use crate::model::{
    Attachment, BugReport, CaseFilter, Dashboard, DiscoveredTest, EmailLog, EntityLink, RunMode,
    RunStatus, StepKind, StepRun, TestCase, TestRun, TestStep, now,
};
use anyhow::{Context, Result, bail};
use rusqlite::types::Value;
use rusqlite::{Connection, OptionalExtension, Row, params, params_from_iter};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::str::FromStr;

const CASE_COLS: &str = "id,title,description,module,priority,status,environment,gherkin,created_at,updated_at,driver_target,default_mode";

const ATTACHMENT_COLS: &str = "id,kind,path,original_name,mime,sha256,size,caption,created_at,step_run_id,run_id,bug_id,case_id,role";

const ATTACHMENT_ROLES: [&str; 4] = ["expected", "actual", "artifact", "evidence"];

fn row_to_case(row: &Row<'_>) -> rusqlite::Result<TestCase> {
    let default_mode: String = row.get(11)?;
    Ok(TestCase {
        id: row.get(0)?,
        title: row.get(1)?,
        description: row.get(2)?,
        module: row.get(3)?,
        priority: row.get(4)?,
        status: row.get(5)?,
        environment: row.get(6)?,
        gherkin: row.get(7)?,
        created_at: row.get(8)?,
        updated_at: row.get(9)?,
        driver_target: row.get(10)?,
        default_mode: RunMode::from_str(&default_mode).unwrap_or(RunMode::Manual),
        tags: Vec::new(),
        steps: Vec::new(),
    })
}

fn load_tags(conn: &Connection, case_id: &str) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT t.name FROM tag t JOIN case_tag ct ON ct.tag_id=t.id
         WHERE ct.case_id=?1 ORDER BY t.name",
    )?;
    let rows = stmt.query_map([case_id], |r| r.get::<_, String>(0))?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

fn load_steps(conn: &Connection, case_id: &str) -> Result<Vec<TestStep>> {
    let mut stmt = conn.prepare(
        "SELECT id,case_id,position,action,data,expected,kind,driver_target
         FROM test_step WHERE case_id=?1 ORDER BY position",
    )?;
    let rows = stmt.query_map([case_id], |r| {
        let kind: String = r.get(6)?;
        Ok(TestStep {
            id: r.get(0)?,
            case_id: r.get(1)?,
            position: r.get(2)?,
            action: r.get(3)?,
            data: r.get(4)?,
            expected: r.get(5)?,
            kind: StepKind::from_str(&kind).unwrap_or(StepKind::Manual),
            driver_target: r.get(7)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

fn hydrate_case(conn: &Connection, mut case: TestCase) -> Result<TestCase> {
    case.tags = load_tags(conn, &case.id)?;
    case.steps = load_steps(conn, &case.id)?;
    Ok(case)
}

pub fn set_tags(conn: &Connection, case_id: &str, tags: &[String]) -> Result<()> {
    conn.execute("DELETE FROM case_tag WHERE case_id=?1", [case_id])?;
    for tag in tags {
        let tag = tag.trim();
        if tag.is_empty() {
            continue;
        }
        conn.execute(
            "INSERT OR IGNORE INTO tag (name,category) VALUES (?1,'custom')",
            [tag],
        )?;
        conn.execute(
            "INSERT OR IGNORE INTO case_tag (case_id,tag_id)
             SELECT ?1,id FROM tag WHERE name=?2",
            params![case_id, tag],
        )?;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
pub fn create_case(
    conn: &Connection,
    id: &str,
    title: &str,
    description: &str,
    module: &str,
    priority: &str,
    environment: &str,
    gherkin: &str,
    tags: &[String],
) -> Result<TestCase> {
    let stamp = now();
    conn.execute(
        "INSERT INTO test_case (id,title,description,module,priority,status,environment,gherkin,created_at,updated_at)
         VALUES (?1,?2,?3,?4,?5,'active',?6,?7,?8,?8)",
        params![id, title, description, module, priority, environment, gherkin, stamp],
    )
    .with_context(|| format!("creating case {id}"))?;
    set_tags(conn, id, tags)?;
    get_case(conn, id)?.context("case disappeared after insert")
}

pub fn update_case(conn: &Connection, case: &TestCase) -> Result<()> {
    conn.execute(
        "UPDATE test_case SET title=?2,description=?3,module=?4,priority=?5,status=?6,
         environment=?7,gherkin=?8,updated_at=?9,driver_target=?10,default_mode=?11 WHERE id=?1",
        params![
            case.id,
            case.title,
            case.description,
            case.module,
            case.priority,
            case.status,
            case.environment,
            case.gherkin,
            now(),
            case.driver_target,
            case.default_mode.as_str()
        ],
    )?;
    set_tags(conn, &case.id, &case.tags)?;
    Ok(())
}

/// Point a case at a discovered test and choose its default execution mode.
pub fn set_case_automation(
    conn: &Connection,
    case_id: &str,
    mode: RunMode,
    driver_target: Option<&str>,
) -> Result<TestCase> {
    let target = driver_target
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let changed = conn.execute(
        "UPDATE test_case SET default_mode=?2,driver_target=?3,updated_at=?4 WHERE id=?1",
        params![case_id, mode.as_str(), target, now()],
    )?;
    if changed == 0 {
        bail!("case not found: {case_id}");
    }
    get_case(conn, case_id)?.context("case disappeared after setting automation")
}

pub fn delete_case(conn: &Connection, id: &str) -> Result<bool> {
    Ok(conn.execute("DELETE FROM test_case WHERE id=?1", [id])? > 0)
}

pub fn get_case(conn: &Connection, id: &str) -> Result<Option<TestCase>> {
    let sql = format!("SELECT {CASE_COLS} FROM test_case WHERE id=?1");
    let case = conn.query_row(&sql, [id], row_to_case).optional()?;
    match case {
        Some(case) => Ok(Some(hydrate_case(conn, case)?)),
        None => Ok(None),
    }
}

pub fn list_cases(
    conn: &Connection,
    tag: Option<&str>,
    status: Option<&str>,
) -> Result<Vec<TestCase>> {
    let sql = format!(
        "SELECT {CASE_COLS} FROM test_case c
         WHERE (?1 IS NULL OR c.status=?1)
           AND (?2 IS NULL OR EXISTS (
                SELECT 1 FROM case_tag ct JOIN tag t ON t.id=ct.tag_id
                WHERE ct.case_id=c.id AND t.name=?2))
         ORDER BY c.id"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![status, tag], row_to_case)?;
    let cases = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    cases.into_iter().map(|c| hydrate_case(conn, c)).collect()
}

fn non_empty_filter(value: &Option<String>) -> Option<String> {
    value
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

/// List cases matching every provided filter. Empty filters are ignored.
///
/// `q` matches the id, title, module, or step action/expected text
/// case-insensitively. `category` matches the tag category (level, type,
/// technique, quality).
pub fn list_cases_filtered(conn: &Connection, filter: &CaseFilter) -> Result<Vec<TestCase>> {
    let mut sql = format!("SELECT {CASE_COLS} FROM test_case c WHERE 1=1");
    let mut values: Vec<Value> = Vec::new();

    if let Some(q) = non_empty_filter(&filter.q) {
        let pattern = format!("%{}%", q.to_lowercase());
        sql.push_str(
            " AND (lower(c.id) LIKE ?
                OR lower(c.title) LIKE ?
                OR lower(c.module) LIKE ?
                OR EXISTS (
                    SELECT 1 FROM test_step s
                    WHERE s.case_id=c.id
                      AND (lower(s.action) LIKE ? OR lower(s.expected) LIKE ?)))",
        );
        for _ in 0..5 {
            values.push(Value::Text(pattern.clone()));
        }
    }
    if let Some(tag) = non_empty_filter(&filter.tag) {
        sql.push_str(
            " AND EXISTS (
                SELECT 1 FROM case_tag ct JOIN tag t ON t.id=ct.tag_id
                WHERE ct.case_id=c.id AND t.name=?)",
        );
        values.push(Value::Text(tag));
    }
    if let Some(category) = non_empty_filter(&filter.category) {
        sql.push_str(
            " AND EXISTS (
                SELECT 1 FROM case_tag ct JOIN tag t ON t.id=ct.tag_id
                WHERE ct.case_id=c.id AND t.category=?)",
        );
        values.push(Value::Text(category));
    }
    if let Some(priority) = non_empty_filter(&filter.priority) {
        sql.push_str(" AND c.priority=?");
        values.push(Value::Text(priority));
    }
    if let Some(status) = non_empty_filter(&filter.status) {
        sql.push_str(" AND c.status=?");
        values.push(Value::Text(status));
    }
    sql.push_str(" ORDER BY c.id");

    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params_from_iter(values.iter()), row_to_case)?;
    let cases = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    cases.into_iter().map(|c| hydrate_case(conn, c)).collect()
}

/// `(name, category, count)` for every tag, including unused tags.
pub fn tag_counts(conn: &Connection) -> Result<Vec<(String, String, i64)>> {
    let mut stmt = conn.prepare(
        "SELECT t.name, t.category, count(ct.case_id)
         FROM tag t LEFT JOIN case_tag ct ON ct.tag_id=t.id
         GROUP BY t.id
         ORDER BY t.category, t.name",
    )?;
    let rows = stmt.query_map([], |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)))?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

#[allow(clippy::too_many_arguments)]
pub fn add_step(
    conn: &Connection,
    case_id: &str,
    position: i64,
    action: &str,
    data: &str,
    expected: &str,
    kind: StepKind,
    driver_target: Option<&str>,
) -> Result<TestStep> {
    conn.execute(
        "INSERT INTO test_step (case_id,position,action,data,expected,kind,driver_target)
         VALUES (?1,?2,?3,?4,?5,?6,?7)",
        params![
            case_id,
            position,
            action,
            data,
            expected,
            kind.as_str(),
            driver_target
        ],
    )?;
    let id = conn.last_insert_rowid();
    Ok(TestStep {
        id,
        case_id: case_id.to_string(),
        position,
        action: action.to_string(),
        data: data.to_string(),
        expected: expected.to_string(),
        kind,
        driver_target: driver_target.map(str::to_string),
    })
}

fn row_to_run(row: &Row<'_>) -> rusqlite::Result<TestRun> {
    let mode: String = row.get(2)?;
    let status: String = row.get(3)?;
    Ok(TestRun {
        id: row.get(0)?,
        case_id: row.get(1)?,
        mode: RunMode::from_str(&mode).unwrap_or(RunMode::Manual),
        status: RunStatus::from_str(&status).unwrap_or(RunStatus::NotRun),
        started_at: row.get(4)?,
        ended_at: row.get(5)?,
        environment: row.get(6)?,
        commit_ref: row.get(7)?,
        command: row.get(8)?,
        notes: row.get(9)?,
        steps: Vec::new(),
    })
}

fn new_run_id(conn: &Connection) -> Result<String> {
    let base = chrono::Local::now().format("RUN-%Y%m%d-%H%M%S").to_string();
    let mut n = 1;
    loop {
        let id = format!("{base}-{n}");
        let exists: i64 =
            conn.query_row("SELECT count(*) FROM test_run WHERE id=?1", [&id], |r| {
                r.get(0)
            })?;
        if exists == 0 {
            return Ok(id);
        }
        n += 1;
    }
}

pub fn create_run(
    conn: &Connection,
    case_id: &str,
    mode: RunMode,
    environment: &str,
) -> Result<TestRun> {
    let case = get_case(conn, case_id)?.with_context(|| format!("case not found: {case_id}"))?;
    let id = new_run_id(conn)?;
    let stamp = now();
    conn.execute(
        "INSERT INTO test_run (id,case_id,mode,status,started_at,environment)
         VALUES (?1,?2,?3,'running',?4,?5)",
        params![id, case_id, mode.as_str(), stamp, environment],
    )?;
    if mode == RunMode::Single {
        // `single` runs the case-level driver target as one synthetic step, so
        // the driver outcome maps to position 1 regardless of the case steps.
        conn.execute(
            "INSERT INTO step_run (run_id,step_id,position,status,driver_test)
             VALUES (?1,NULL,1,'not_run',?2)",
            params![id, case.driver_target],
        )?;
    } else if case.steps.is_empty() {
        conn.execute(
            "INSERT INTO step_run (run_id,position,status) VALUES (?1,1,'not_run')",
            [&id],
        )?;
    } else {
        for step in &case.steps {
            conn.execute(
                "INSERT INTO step_run (run_id,step_id,position,status,driver_test)
                 VALUES (?1,?2,?3,'not_run',?4)",
                params![id, step.id, step.position, step.driver_target],
            )?;
        }
    }
    get_run(conn, &id)?.context("run disappeared after insert")
}

pub fn get_run(conn: &Connection, id: &str) -> Result<Option<TestRun>> {
    let run = conn
        .query_row(
            "SELECT id,case_id,mode,status,started_at,ended_at,environment,commit_ref,command,notes
             FROM test_run WHERE id=?1",
            [id],
            row_to_run,
        )
        .optional()?;
    let Some(mut run) = run else { return Ok(None) };
    run.steps = load_step_runs(conn, &run.id)?;
    Ok(Some(run))
}

fn load_step_runs(conn: &Connection, run_id: &str) -> Result<Vec<StepRun>> {
    let mut stmt = conn.prepare(
        "SELECT id,run_id,step_id,position,status,observed,notes,duration_ms,driver_test,raw_output
         FROM step_run WHERE run_id=?1 ORDER BY position",
    )?;
    let rows = stmt.query_map([run_id], |r| {
        let status: String = r.get(4)?;
        Ok(StepRun {
            id: r.get(0)?,
            run_id: r.get(1)?,
            step_id: r.get(2)?,
            position: r.get(3)?,
            status: RunStatus::from_str(&status).unwrap_or(RunStatus::NotRun),
            observed: r.get(5)?,
            notes: r.get(6)?,
            duration_ms: r.get(7)?,
            driver_test: r.get(8)?,
            raw_output: r.get(9)?,
            evidence: Vec::new(),
        })
    })?;
    let mut steps = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    for step in &mut steps {
        step.evidence = load_evidence(conn, EvidenceOwner::StepRun(step.id))?;
    }
    Ok(steps)
}

pub fn list_runs(conn: &Connection, case_id: Option<&str>, limit: i64) -> Result<Vec<TestRun>> {
    let mut stmt = conn.prepare(
        "SELECT id,case_id,mode,status,started_at,ended_at,environment,commit_ref,command,notes
         FROM test_run WHERE (?1 IS NULL OR case_id=?1) ORDER BY started_at DESC LIMIT ?2",
    )?;
    let rows = stmt.query_map(params![case_id, limit], row_to_run)?;
    let mut runs = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    for run in &mut runs {
        run.steps = load_step_runs(conn, &run.id)?;
    }
    Ok(runs)
}

#[allow(clippy::too_many_arguments)]
pub fn record_step(
    conn: &Connection,
    run_id: &str,
    position: i64,
    status: RunStatus,
    observed: &str,
    notes: &str,
    duration_ms: Option<i64>,
    driver_test: Option<&str>,
    raw_output: &str,
) -> Result<StepRun> {
    let changed = conn.execute(
        "UPDATE step_run SET status=?3,observed=?4,notes=?5,duration_ms=?6,driver_test=?7,raw_output=?8
         WHERE run_id=?1 AND position=?2",
        params![run_id, position, status.as_str(), observed, notes, duration_ms, driver_test, raw_output],
    )?;
    if changed == 0 {
        conn.execute(
            "INSERT INTO step_run (run_id,position,status,observed,notes,duration_ms,driver_test,raw_output)
             VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
            params![run_id, position, status.as_str(), observed, notes, duration_ms, driver_test, raw_output],
        )?;
    }
    get_step_run(conn, run_id, position)?.context("step run not found after record")
}

pub fn get_step_run(conn: &Connection, run_id: &str, position: i64) -> Result<Option<StepRun>> {
    let mut steps = load_step_runs(conn, run_id)?;
    Ok(steps.drain(..).find(|s| s.position == position))
}

pub fn finish_run(conn: &Connection, run_id: &str) -> Result<TestRun> {
    let run = get_run(conn, run_id)?.with_context(|| format!("run not found: {run_id}"))?;
    let status = derive_run_status(&run.steps);
    conn.execute(
        "UPDATE test_run SET status=?2,ended_at=?3 WHERE id=?1",
        params![run_id, status.as_str(), now()],
    )?;
    get_run(conn, run_id)?.context("run disappeared after finish")
}

fn derive_run_status(steps: &[StepRun]) -> RunStatus {
    if steps.is_empty() {
        return RunStatus::NotRun;
    }
    if steps.iter().any(|s| s.status.is_bad()) {
        return RunStatus::Failed;
    }
    if steps.iter().any(|s| s.status == RunStatus::Blocked) {
        return RunStatus::Blocked;
    }
    if steps.iter().any(|s| s.status == RunStatus::Running) {
        return RunStatus::Running;
    }
    if steps.iter().all(|s| s.status == RunStatus::Passed) {
        return RunStatus::Passed;
    }
    RunStatus::NotRun
}

fn new_bug_id(conn: &Connection) -> Result<String> {
    let base = chrono::Local::now().format("BUG-%Y%m%d").to_string();
    let mut n = 1;
    loop {
        let id = format!("{base}-{n}");
        let exists: i64 =
            conn.query_row("SELECT count(*) FROM bug_report WHERE id=?1", [&id], |r| {
                r.get(0)
            })?;
        if exists == 0 {
            return Ok(id);
        }
        n += 1;
    }
}

#[allow(clippy::too_many_arguments)]
pub fn create_bug(
    conn: &Connection,
    id: Option<&str>,
    title: &str,
    description: &str,
    steps: &str,
    actual: &str,
    expected: &str,
    priority: i64,
    severity: i64,
    environment: &str,
    case_id: Option<&str>,
    run_id: Option<&str>,
    step_run_id: Option<i64>,
    comments: &str,
) -> Result<BugReport> {
    let id = match id {
        Some(id) if !id.trim().is_empty() => id.to_string(),
        _ => new_bug_id(conn)?,
    };
    let stamp = now();
    conn.execute(
        "INSERT INTO bug_report (id,title,description,steps,actual,expected,priority,severity,environment,status,case_id,run_id,step_run_id,comments,created_at,updated_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,'open',?10,?11,?12,?13,?14,?14)",
        params![id, title, description, steps, actual, expected, priority, severity, environment, case_id, run_id, step_run_id, comments, stamp],
    )?;
    get_bug(conn, &id)?.context("bug disappeared after insert")
}

pub fn get_bug(conn: &Connection, id: &str) -> Result<Option<BugReport>> {
    let bug = conn
        .query_row(
            "SELECT id,title,description,steps,actual,expected,priority,severity,environment,status,case_id,run_id,step_run_id,comments,created_at,updated_at
             FROM bug_report WHERE id=?1",
            [id],
            row_to_bug,
        )
        .optional()?;
    let Some(mut bug) = bug else { return Ok(None) };
    bug.evidence = load_evidence(conn, EvidenceOwner::Bug(bug.id.clone()))?;
    Ok(Some(bug))
}

fn row_to_bug(row: &Row<'_>) -> rusqlite::Result<BugReport> {
    Ok(BugReport {
        id: row.get(0)?,
        title: row.get(1)?,
        description: row.get(2)?,
        steps: row.get(3)?,
        actual: row.get(4)?,
        expected: row.get(5)?,
        priority: row.get(6)?,
        severity: row.get(7)?,
        environment: row.get(8)?,
        status: row.get(9)?,
        case_id: row.get(10)?,
        run_id: row.get(11)?,
        step_run_id: row.get(12)?,
        comments: row.get(13)?,
        created_at: row.get(14)?,
        updated_at: row.get(15)?,
        evidence: Vec::new(),
    })
}

pub fn list_bugs(conn: &Connection, status: Option<&str>) -> Result<Vec<BugReport>> {
    let mut stmt = conn.prepare(
        "SELECT id,title,description,steps,actual,expected,priority,severity,environment,status,case_id,run_id,step_run_id,comments,created_at,updated_at
         FROM bug_report WHERE (?1 IS NULL OR status=?1) ORDER BY created_at DESC",
    )?;
    let rows = stmt.query_map([status], row_to_bug)?;
    let bugs = rows.collect::<rusqlite::Result<Vec<_>>>()?;
    Ok(bugs)
}

pub fn set_bug_status(conn: &Connection, id: &str, status: &str) -> Result<()> {
    conn.execute(
        "UPDATE bug_report SET status=?2,updated_at=?3 WHERE id=?1",
        params![id, status, now()],
    )?;
    Ok(())
}

#[derive(Debug, Clone)]
pub enum EvidenceOwner {
    StepRun(i64),
    Run(String),
    Bug(String),
    Case(String),
}

fn row_to_attachment(row: &Row<'_>) -> rusqlite::Result<Attachment> {
    Ok(Attachment {
        id: row.get(0)?,
        kind: row.get(1)?,
        path: row.get(2)?,
        original_name: row.get(3)?,
        mime: row.get(4)?,
        sha256: row.get(5)?,
        size: row.get(6)?,
        caption: row.get(7)?,
        created_at: row.get(8)?,
        step_run_id: row.get(9)?,
        run_id: row.get(10)?,
        bug_id: row.get(11)?,
        case_id: row.get(12)?,
        role: row.get(13)?,
    })
}

pub fn load_evidence(conn: &Connection, owner: EvidenceOwner) -> Result<Vec<Attachment>> {
    let (column, key) = match &owner {
        EvidenceOwner::StepRun(id) => ("step_run_id", id.to_string()),
        EvidenceOwner::Run(id) => ("run_id", id.clone()),
        EvidenceOwner::Bug(id) => ("bug_id", id.clone()),
        EvidenceOwner::Case(id) => ("case_id", id.clone()),
    };
    let sql = format!("SELECT {ATTACHMENT_COLS} FROM attachment WHERE {column}=?1 ORDER BY id");
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([key], row_to_attachment)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn case_evidence(conn: &Connection, case_id: &str) -> Result<Vec<Attachment>> {
    load_evidence(conn, EvidenceOwner::Case(case_id.to_string()))
}

pub fn attach_file(
    conn: &Connection,
    data_dir: &Path,
    owner: EvidenceOwner,
    source: &Path,
    caption: &str,
) -> Result<Attachment> {
    attach_file_with_role(conn, data_dir, owner, source, caption, "evidence")
}

pub fn attach_file_with_role(
    conn: &Connection,
    data_dir: &Path,
    owner: EvidenceOwner,
    source: &Path,
    caption: &str,
    role: &str,
) -> Result<Attachment> {
    let role = role.trim();
    let role = if role.is_empty() { "evidence" } else { role };
    if !ATTACHMENT_ROLES.contains(&role) {
        bail!(
            "unknown attachment role: {role}; expected one of {}",
            ATTACHMENT_ROLES.join(", ")
        );
    }
    let bytes =
        std::fs::read(source).with_context(|| format!("reading evidence {}", source.display()))?;
    let mut hasher = Sha256::new();
    hasher.update(&bytes);
    let digest = hasher.finalize();
    let hash: String = digest.iter().map(|b| format!("{b:02x}")).collect();
    let name = source
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("evidence.bin");
    let safe: String = name
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || c == '.' || c == '-' || c == '_' {
                c
            } else {
                '_'
            }
        })
        .collect();
    let evidence_dir = data_dir.join("evidence");
    std::fs::create_dir_all(&evidence_dir)?;
    let dest = evidence_dir.join(format!("{}-{}", &hash[..12], safe));
    std::fs::write(&dest, &bytes)?;
    let rel = dest
        .strip_prefix(data_dir)
        .unwrap_or(&dest)
        .to_string_lossy()
        .to_string();
    let kind = match source.extension().and_then(|e| e.to_str()) {
        Some("png" | "jpg" | "jpeg" | "gif" | "webp") => "screenshot",
        Some("log" | "txt") => "log",
        _ => "file",
    };
    let mime = mime_for(&safe);
    let (step_run_id, run_id, bug_id, case_id) = match owner {
        EvidenceOwner::StepRun(id) => (Some(id), None, None, None),
        EvidenceOwner::Run(id) => (None, Some(id), None, None),
        EvidenceOwner::Bug(id) => (None, None, Some(id), None),
        EvidenceOwner::Case(id) => (None, None, None, Some(id)),
    };
    conn.execute(
        "INSERT INTO attachment (kind,path,original_name,mime,sha256,size,caption,created_at,step_run_id,run_id,bug_id,case_id,role)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11,?12,?13)",
        params![
            kind,
            rel,
            name,
            mime,
            hash,
            bytes.len() as i64,
            caption,
            now(),
            step_run_id,
            run_id,
            bug_id,
            case_id,
            role
        ],
    )?;
    let id = conn.last_insert_rowid();
    let sql = format!("SELECT {ATTACHMENT_COLS} FROM attachment WHERE id=?1");
    conn.query_row(&sql, [id], row_to_attachment)
        .map_err(Into::into)
}

fn mime_for(name: &str) -> &'static str {
    match name.rsplit('.').next().unwrap_or("") {
        "png" => "image/png",
        "jpg" | "jpeg" => "image/jpeg",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "txt" | "log" => "text/plain",
        "pdf" => "application/pdf",
        _ => "application/octet-stream",
    }
}

fn row_to_link(row: &Row<'_>) -> rusqlite::Result<EntityLink> {
    Ok(EntityLink {
        id: row.get(0)?,
        source_kind: row.get(1)?,
        source_id: row.get(2)?,
        target_kind: row.get(3)?,
        target_id: row.get(4)?,
        relation: row.get(5)?,
        note: row.get(6)?,
        created_at: row.get(7)?,
    })
}

const LINK_COLS: &str = "id,source_kind,source_id,target_kind,target_id,relation,note,created_at";

/// Create an edge, or return the existing edge for the same relation.
pub fn create_link(
    conn: &Connection,
    source_kind: &str,
    source_id: &str,
    target_kind: &str,
    target_id: &str,
    relation: &str,
    note: &str,
) -> Result<EntityLink> {
    for (label, value) in [
        ("source kind", source_kind),
        ("source id", source_id),
        ("target kind", target_kind),
        ("target id", target_id),
        ("relation", relation),
    ] {
        if value.trim().is_empty() {
            bail!("{label} must not be empty");
        }
    }
    conn.execute(
        "INSERT OR IGNORE INTO entity_link
         (source_kind,source_id,target_kind,target_id,relation,note,created_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7)",
        params![
            source_kind.trim(),
            source_id.trim(),
            target_kind.trim(),
            target_id.trim(),
            relation.trim(),
            note,
            now()
        ],
    )?;
    let sql = format!(
        "SELECT {LINK_COLS} FROM entity_link
         WHERE source_kind=?1 AND source_id=?2 AND target_kind=?3 AND target_id=?4 AND relation=?5"
    );
    conn.query_row(
        &sql,
        params![
            source_kind.trim(),
            source_id.trim(),
            target_kind.trim(),
            target_id.trim(),
            relation.trim()
        ],
        row_to_link,
    )
    .map_err(Into::into)
}

/// List every edge that touches `kind`/`id`, in either direction.
pub fn list_links(conn: &Connection, kind: &str, id: &str) -> Result<Vec<EntityLink>> {
    let sql = format!(
        "SELECT {LINK_COLS} FROM entity_link
         WHERE (source_kind=?1 AND source_id=?2) OR (target_kind=?1 AND target_id=?2)
         ORDER BY id"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map(params![kind, id], row_to_link)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn delete_link(conn: &Connection, id: i64) -> Result<bool> {
    Ok(conn.execute("DELETE FROM entity_link WHERE id=?1", [id])? > 0)
}

fn row_to_email_log(row: &Row<'_>) -> rusqlite::Result<EmailLog> {
    Ok(EmailLog {
        id: row.get(0)?,
        created_at: row.get(1)?,
        to_addr: row.get(2)?,
        subject: row.get(3)?,
        report_path: row.get(4)?,
        status: row.get(5)?,
        transport: row.get(6)?,
        file_path: row.get(7)?,
        error: row.get(8)?,
    })
}

const EMAIL_LOG_COLS: &str =
    "id,created_at,to_addr,subject,report_path,status,transport,file_path,error";

#[allow(clippy::too_many_arguments)]
pub fn create_email_log(
    conn: &Connection,
    to: &str,
    subject: &str,
    report_path: &str,
    status: &str,
    transport: &str,
    file_path: &str,
    error: &str,
) -> Result<EmailLog> {
    conn.execute(
        "INSERT INTO email_log
         (created_at,to_addr,subject,report_path,status,transport,file_path,error)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8)",
        params![
            now(),
            to,
            subject,
            report_path,
            status,
            transport,
            file_path,
            error
        ],
    )?;
    let id = conn.last_insert_rowid();
    let sql = format!("SELECT {EMAIL_LOG_COLS} FROM email_log WHERE id=?1");
    conn.query_row(&sql, [id], row_to_email_log)
        .map_err(Into::into)
}

pub fn list_email_logs(conn: &Connection, limit: i64) -> Result<Vec<EmailLog>> {
    let sql = format!(
        "SELECT {EMAIL_LOG_COLS} FROM email_log ORDER BY created_at DESC, id DESC LIMIT ?1"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([limit], row_to_email_log)?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn recent_email_recipients(conn: &Connection, limit: i64) -> Result<Vec<String>> {
    let mut stmt = conn.prepare(
        "SELECT to_addr FROM email_log
         GROUP BY to_addr
         ORDER BY max(created_at) DESC, max(id) DESC
         LIMIT ?1",
    )?;
    let rows = stmt.query_map([limit], |row| row.get::<_, String>(0))?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn ensure_driver(conn: &Connection, id: &str, kind: &str, name: &str) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO driver (id,kind,name) VALUES (?1,?2,?3)",
        params![id, kind, name],
    )?;
    Ok(())
}

pub fn save_driver_tests(
    conn: &Connection,
    driver_id: &str,
    tests: &[DiscoveredTest],
) -> Result<()> {
    conn.execute("DELETE FROM driver_test WHERE driver_id=?1", [driver_id])?;
    for test in tests {
        conn.execute(
            "INSERT OR REPLACE INTO driver_test (id,driver_id,name,file,line,metadata_json,discovered_at)
             VALUES (?1,?2,?3,?4,?5,?6,?7)",
            params![
                test.id,
                driver_id,
                test.name,
                test.file,
                test.line,
                test.metadata.to_string(),
                now()
            ],
        )?;
    }
    Ok(())
}

pub fn list_driver_tests(
    conn: &Connection,
    driver_id: Option<&str>,
) -> Result<Vec<DiscoveredTest>> {
    let mut stmt = conn.prepare(
        "SELECT id,driver_id,name,file,line,metadata_json FROM driver_test
         WHERE (?1 IS NULL OR driver_id=?1) ORDER BY file,name",
    )?;
    let rows = stmt.query_map([driver_id], |r| {
        let meta: String = r.get(5)?;
        Ok(DiscoveredTest {
            id: r.get(0)?,
            driver_id: r.get(1)?,
            name: r.get(2)?,
            file: r.get(3)?,
            line: r.get(4)?,
            metadata: serde_json::from_str(&meta).unwrap_or(serde_json::Value::Null),
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn create_requirement(
    conn: &Connection,
    id: &str,
    title: &str,
    source: &str,
    acceptance: &str,
    kind: &str,
) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO requirement (id,title,source,acceptance,kind) VALUES (?1,?2,?3,?4,?5)",
        params![id, title, source, acceptance, kind],
    )?;
    Ok(())
}

pub fn link_case_requirement(conn: &Connection, case_id: &str, requirement_id: &str) -> Result<()> {
    conn.execute(
        "INSERT OR IGNORE INTO case_requirement (case_id,requirement_id) VALUES (?1,?2)",
        params![case_id, requirement_id],
    )?;
    Ok(())
}

pub fn dashboard(conn: &Connection) -> Result<Dashboard> {
    let cases = count(conn, "SELECT count(*) FROM test_case")?;
    let runs = count(conn, "SELECT count(*) FROM test_run")?;
    let bugs_open = count(conn, "SELECT count(*) FROM bug_report WHERE status='open'")?;
    let bugs_total = count(conn, "SELECT count(*) FROM bug_report")?;
    let failed_runs = count(conn, "SELECT count(*) FROM test_run WHERE status='failed'")?;
    let passed_runs = count(conn, "SELECT count(*) FROM test_run WHERE status='passed'")?;
    let evidence = count(conn, "SELECT count(*) FROM attachment")?;
    Ok(Dashboard {
        cases,
        runs,
        bugs_open,
        bugs_total,
        failed_runs,
        passed_runs,
        evidence,
    })
}

fn count(conn: &Connection, sql: &str) -> Result<i64> {
    Ok(conn.query_row(sql, [], |r| r.get(0))?)
}

pub fn default_data_dir(project: &Path) -> PathBuf {
    project.join(".laburo")
}
