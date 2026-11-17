use crate::model::{
    Attachment, BugReport, Dashboard, DiscoveredTest, RunMode, RunStatus, StepKind, StepRun,
    TestCase, TestRun, TestStep, now,
};
use anyhow::{Context, Result};
use rusqlite::{Connection, OptionalExtension, Row, params};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};
use std::str::FromStr;

const CASE_COLS: &str =
    "id,title,description,module,priority,status,environment,gherkin,created_at,updated_at";

fn row_to_case(row: &Row<'_>) -> rusqlite::Result<TestCase> {
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
         environment=?7,gherkin=?8,updated_at=?9 WHERE id=?1",
        params![
            case.id,
            case.title,
            case.description,
            case.module,
            case.priority,
            case.status,
            case.environment,
            case.gherkin,
            now()
        ],
    )?;
    set_tags(conn, &case.id, &case.tags)?;
    Ok(())
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
    if case.steps.is_empty() {
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
}

pub fn load_evidence(conn: &Connection, owner: EvidenceOwner) -> Result<Vec<Attachment>> {
    let (sql, key) = match &owner {
        EvidenceOwner::StepRun(id) => (
            "SELECT id,kind,path,original_name,mime,sha256,size,caption,created_at,step_run_id,run_id,bug_id FROM attachment WHERE step_run_id=?1 ORDER BY id",
            id.to_string(),
        ),
        EvidenceOwner::Run(id) => (
            "SELECT id,kind,path,original_name,mime,sha256,size,caption,created_at,step_run_id,run_id,bug_id FROM attachment WHERE run_id=?1 ORDER BY id",
            id.clone(),
        ),
        EvidenceOwner::Bug(id) => (
            "SELECT id,kind,path,original_name,mime,sha256,size,caption,created_at,step_run_id,run_id,bug_id FROM attachment WHERE bug_id=?1 ORDER BY id",
            id.clone(),
        ),
    };
    let mut stmt = conn.prepare(sql)?;
    let rows = stmt.query_map([key], |r| {
        Ok(Attachment {
            id: r.get(0)?,
            kind: r.get(1)?,
            path: r.get(2)?,
            original_name: r.get(3)?,
            mime: r.get(4)?,
            sha256: r.get(5)?,
            size: r.get(6)?,
            caption: r.get(7)?,
            created_at: r.get(8)?,
            step_run_id: r.get(9)?,
            run_id: r.get(10)?,
            bug_id: r.get(11)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn attach_file(
    conn: &Connection,
    data_dir: &Path,
    owner: EvidenceOwner,
    source: &Path,
    caption: &str,
) -> Result<Attachment> {
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
    let (step_run_id, run_id, bug_id) = match owner {
        EvidenceOwner::StepRun(id) => (Some(id), None, None),
        EvidenceOwner::Run(id) => (None, Some(id), None),
        EvidenceOwner::Bug(id) => (None, None, Some(id)),
    };
    conn.execute(
        "INSERT INTO attachment (kind,path,original_name,mime,sha256,size,caption,created_at,step_run_id,run_id,bug_id)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,?9,?10,?11)",
        params![kind, rel, name, mime, hash, bytes.len() as i64, caption, now(), step_run_id, run_id, bug_id],
    )?;
    let id = conn.last_insert_rowid();
    conn.query_row(
        "SELECT id,kind,path,original_name,mime,sha256,size,caption,created_at,step_run_id,run_id,bug_id
         FROM attachment WHERE id=?1",
        [id],
        |r| {
            Ok(Attachment {
                id: r.get(0)?,
                kind: r.get(1)?,
                path: r.get(2)?,
                original_name: r.get(3)?,
                mime: r.get(4)?,
                sha256: r.get(5)?,
                size: r.get(6)?,
                caption: r.get(7)?,
                created_at: r.get(8)?,
                step_run_id: r.get(9)?,
                run_id: r.get(10)?,
                bug_id: r.get(11)?,
            })
        },
    )
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
