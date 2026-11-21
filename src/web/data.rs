//! Web-only queries and small writes.
//!
//! Most reads go through `crate::repo`. The v0.2 repository API is landing in
//! parallel, so this module keeps the web UI working against both the v0.1 and
//! the v0.2 schema: it detects optional tables/columns with `PRAGMA` and falls
//! back to the v0.1 shape instead of failing to boot. When the Data pod lands,
//! the auto-detection lights the v0.2 features up without a second edit here.

use crate::model::{Attachment, TestCase};
use crate::repo::{self, EvidenceOwner};
use anyhow::{Context, Result, bail};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use std::collections::HashMap;
use std::path::Path;

#[derive(Debug, Clone, Default)]
pub struct CaseQuery {
    pub q: Option<String>,
    pub tag: Option<String>,
    pub category: Option<String>,
    pub priority: Option<String>,
    pub status: Option<String>,
}

impl CaseQuery {
    pub fn is_active(&self) -> bool {
        self.q.is_some()
            || self.tag.is_some()
            || self.category.is_some()
            || self.priority.is_some()
            || self.status.is_some()
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct TagCount {
    pub name: String,
    pub category: String,
    pub count: i64,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct Automation {
    pub driver_target: Option<String>,
    pub default_mode: String,
    /// False when migration 0003 has not been applied yet (Data pod pending).
    pub persisted: bool,
    /// True when the case has a case-level target, a driver step target, or a
    /// non-manual default mode. Serialized so templates can read it directly.
    pub automated: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct EvidenceView {
    pub id: i64,
    pub kind: String,
    pub path: String,
    pub url: String,
    pub original_name: String,
    pub caption: String,
    pub role: String,
    pub size: i64,
    pub created_at: String,
    pub is_image: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct LinkView {
    pub id: i64,
    pub source_kind: String,
    pub source_id: String,
    pub target_kind: String,
    pub target_id: String,
    pub relation: String,
    pub note: String,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct EmailLogView {
    pub id: i64,
    pub created_at: String,
    pub to_addr: String,
    pub subject: String,
    pub report_path: String,
    pub status: String,
    pub transport: String,
    pub file_path: String,
    pub error: String,
}

pub fn has_table(conn: &Connection, table: &str) -> bool {
    conn.query_row(
        "SELECT count(*) FROM sqlite_master WHERE type='table' AND name=?1",
        [table],
        |row| row.get::<_, i64>(0).map(|n| n > 0),
    )
    .unwrap_or(false)
}

pub fn has_column(conn: &Connection, table: &str, column: &str) -> bool {
    // `table` is a compile-time literal from our own call sites, never user
    // input; SQLite does not accept PRAGMA arguments as parameters.
    let sql = format!("PRAGMA table_info({table})");
    let Ok(mut stmt) = conn.prepare(&sql) else {
        return false;
    };
    let Ok(rows) = stmt.query_map([], |row| row.get::<_, String>(1)) else {
        return false;
    };
    rows.filter_map(std::result::Result::ok)
        .any(|name| name == column)
}

pub fn case_automation(conn: &Connection, case_id: &str) -> Result<Automation> {
    let driver_steps: i64 = conn.query_row(
        "SELECT count(*) FROM test_step
         WHERE case_id=?1 AND kind='driver'
           AND driver_target IS NOT NULL AND trim(driver_target) <> ''",
        [case_id],
        |row| row.get(0),
    )?;
    if !has_column(conn, "test_case", "default_mode") {
        return Ok(Automation {
            driver_target: None,
            default_mode: "manual".to_string(),
            persisted: false,
            automated: driver_steps > 0,
        });
    }
    let row = conn
        .query_row(
            "SELECT driver_target, default_mode FROM test_case WHERE id=?1",
            [case_id],
            |row| {
                Ok((
                    row.get::<_, Option<String>>(0)?,
                    row.get::<_, Option<String>>(1)?,
                ))
            },
        )
        .optional()?;
    let (driver_target, default_mode) = row.unwrap_or((None, None));
    let default_mode = default_mode.unwrap_or_else(|| "manual".to_string());
    let automated = default_mode != "manual" || driver_target.is_some() || driver_steps > 0;
    Ok(Automation {
        driver_target,
        default_mode,
        persisted: true,
        automated,
    })
}

/// Persist automation fields. Returns false when migration 0003 is not applied.
pub fn set_case_automation(
    conn: &Connection,
    case_id: &str,
    mode: &str,
    driver_target: Option<&str>,
) -> Result<bool> {
    if !has_column(conn, "test_case", "default_mode") {
        return Ok(false);
    }
    let target = driver_target
        .map(str::trim)
        .filter(|value| !value.is_empty());
    conn.execute(
        "UPDATE test_case SET default_mode=?2, driver_target=?3, updated_at=?4 WHERE id=?1",
        params![case_id, mode, target, crate::model::now()],
    )?;
    Ok(true)
}

pub fn list_cases_filtered(conn: &Connection, filter: &CaseQuery) -> Result<Vec<TestCase>> {
    let like = filter
        .q
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(|value| format!("%{value}%"));
    let tag = filter
        .tag
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let category = filter
        .category
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let priority = filter
        .priority
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    let status = filter
        .status
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());

    let mut stmt = conn.prepare(
        "SELECT c.id FROM test_case c
         WHERE (?1 IS NULL OR c.status=?1)
           AND (?2 IS NULL OR c.priority=?2)
           AND (?3 IS NULL OR EXISTS (
                SELECT 1 FROM case_tag ct JOIN tag t ON t.id=ct.tag_id
                WHERE ct.case_id=c.id AND t.name=?3))
           AND (?4 IS NULL OR EXISTS (
                SELECT 1 FROM case_tag ct JOIN tag t ON t.id=ct.tag_id
                WHERE ct.case_id=c.id AND t.category=?4))
           AND (?5 IS NULL
                OR c.id LIKE ?5 COLLATE NOCASE
                OR c.title LIKE ?5 COLLATE NOCASE
                OR c.module LIKE ?5 COLLATE NOCASE
                OR EXISTS (
                    SELECT 1 FROM test_step s
                    WHERE s.case_id=c.id
                      AND (s.action LIKE ?5 COLLATE NOCASE
                           OR s.data LIKE ?5 COLLATE NOCASE
                           OR s.expected LIKE ?5 COLLATE NOCASE)))
         ORDER BY c.id",
    )?;
    let ids = stmt
        .query_map(params![status, priority, tag, category, like], |row| {
            row.get::<_, String>(0)
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;

    let mut cases = Vec::with_capacity(ids.len());
    for id in ids {
        match repo::get_case(conn, &id)? {
            Some(case) => cases.push(case),
            None => bail!("case {id} vanished while listing"),
        }
    }
    Ok(cases)
}

pub fn tag_counts(conn: &Connection) -> Result<Vec<TagCount>> {
    let mut stmt = conn.prepare(
        "SELECT t.name, t.category, count(ct.case_id)
         FROM tag t LEFT JOIN case_tag ct ON ct.tag_id=t.id
         GROUP BY t.id, t.name, t.category
         ORDER BY count(ct.case_id) DESC, t.name",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(TagCount {
            name: row.get(0)?,
            category: row.get(1)?,
            count: row.get(2)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

/// Latest run per case: case id -> (status, started_at).
pub fn latest_runs(conn: &Connection) -> Result<HashMap<String, (String, String)>> {
    let mut stmt = conn.prepare(
        "SELECT r.case_id, r.status, r.started_at FROM test_run r
         WHERE r.started_at = (
            SELECT max(x.started_at) FROM test_run x WHERE x.case_id=r.case_id)
         ORDER BY r.case_id",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, String>(2)?,
        ))
    })?;
    let mut map = HashMap::new();
    for row in rows {
        let (case_id, status, started_at) = row?;
        map.insert(case_id, (status, started_at));
    }
    Ok(map)
}

// ---------------------------------------------------------------------------
// Steps (the v0.1 repo API has add/load only; the UI needs edit/delete/move)
// ---------------------------------------------------------------------------

#[allow(clippy::too_many_arguments)]
pub fn update_step(
    conn: &Connection,
    case_id: &str,
    position: i64,
    action: &str,
    data: &str,
    expected: &str,
    kind: &str,
    driver_target: Option<&str>,
) -> Result<usize> {
    let target = driver_target
        .map(str::trim)
        .filter(|value| !value.is_empty());
    Ok(conn.execute(
        "UPDATE test_step SET action=?3,data=?4,expected=?5,kind=?6,driver_target=?7
         WHERE case_id=?1 AND position=?2",
        params![case_id, position, action, data, expected, kind, target],
    )?)
}

pub fn delete_step(conn: &Connection, case_id: &str, position: i64) -> Result<()> {
    conn.execute(
        "DELETE FROM test_step WHERE case_id=?1 AND position=?2",
        params![case_id, position],
    )?;
    conn.execute(
        "UPDATE test_step SET position=position-1 WHERE case_id=?1 AND position>?2",
        params![case_id, position],
    )?;
    Ok(())
}

pub fn move_step(conn: &Connection, case_id: &str, position: i64, direction: &str) -> Result<bool> {
    let other = match direction {
        "up" => position - 1,
        "down" => position + 1,
        _ => return Ok(false),
    };
    if position < 1 || other < 1 {
        return Ok(false);
    }
    let exists: i64 = conn.query_row(
        "SELECT count(*) FROM test_step WHERE case_id=?1 AND position=?2",
        params![case_id, other],
        |row| row.get(0),
    )?;
    if exists == 0 {
        return Ok(false);
    }
    // Park the moving step on a temporary negative position so the UNIQUE
    // (case_id, position) constraint never trips mid-swap.
    conn.execute(
        "UPDATE test_step SET position=-1 WHERE case_id=?1 AND position=?2",
        params![case_id, position],
    )?;
    conn.execute(
        "UPDATE test_step SET position=?2 WHERE case_id=?1 AND position=?3",
        params![case_id, position, other],
    )?;
    conn.execute(
        "UPDATE test_step SET position=?2 WHERE case_id=?1 AND position=-1",
        params![case_id, other],
    )?;
    Ok(true)
}

pub fn next_position(conn: &Connection, case_id: &str) -> Result<i64> {
    Ok(conn.query_row(
        "SELECT coalesce(max(position),0)+1 FROM test_step WHERE case_id=?1",
        [case_id],
        |row| row.get(0),
    )?)
}

pub fn step_run_id(conn: &Connection, run_id: &str, position: i64) -> Result<Option<i64>> {
    Ok(conn
        .query_row(
            "SELECT id FROM step_run WHERE run_id=?1 AND position=?2",
            params![run_id, position],
            |row| row.get(0),
        )
        .optional()?)
}

// ---------------------------------------------------------------------------
// Evidence
// ---------------------------------------------------------------------------

pub enum EvidenceTarget {
    StepRun(i64),
    Run(String),
    Bug(String),
    Case(String),
}

fn evidence_view(row: &rusqlite::Row<'_>, migrated: bool) -> rusqlite::Result<EvidenceView> {
    let path: String = row.get(2)?;
    let mime: String = row.get(4)?;
    let role: String = if migrated {
        row.get(12)?
    } else {
        "evidence".to_string()
    };
    Ok(EvidenceView {
        id: row.get(0)?,
        kind: row.get(1)?,
        url: format!("/files/{}", path.replace('\\', "/")),
        path,
        original_name: row.get(3)?,
        is_image: mime.starts_with("image/"),
        caption: row.get(7)?,
        size: row.get(6)?,
        created_at: row.get(8)?,
        role,
    })
}

pub fn evidence_for(conn: &Connection, target: &EvidenceTarget) -> Result<Vec<EvidenceView>> {
    let migrated = has_column(conn, "attachment", "role");
    let extra = if migrated { ",role" } else { "" };
    let (column, key) = match target {
        EvidenceTarget::StepRun(id) => ("step_run_id", id.to_string()),
        EvidenceTarget::Run(id) => ("run_id", id.clone()),
        EvidenceTarget::Bug(id) => ("bug_id", id.clone()),
        EvidenceTarget::Case(id) => {
            if !has_column(conn, "attachment", "case_id") {
                return Ok(Vec::new());
            }
            ("case_id", id.clone())
        }
    };
    let sql = format!(
        "SELECT id,kind,path,original_name,mime,sha256,size,caption,created_at,
                step_run_id,run_id,bug_id{extra}
         FROM attachment WHERE {column}=?1 ORDER BY id"
    );
    let mut stmt = conn.prepare(&sql)?;
    let rows = stmt.query_map([key], |row| evidence_view(row, migrated))?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn evidence_for_attachment(conn: &Connection, attachment: &Attachment) -> EvidenceView {
    let _ = conn;
    EvidenceView {
        id: attachment.id,
        kind: attachment.kind.clone(),
        path: attachment.path.clone(),
        url: format!("/files/{}", attachment.path.replace('\\', "/")),
        original_name: attachment.original_name.clone(),
        caption: attachment.caption.clone(),
        role: "evidence".to_string(),
        size: attachment.size,
        created_at: attachment.created_at.clone(),
        is_image: attachment.mime.starts_with("image/"),
    }
}

/// Attach an uploaded file and apply the v0.2 role/owner when available.
pub fn attach_evidence(
    conn: &Connection,
    data_dir: &Path,
    target: EvidenceTarget,
    source: &Path,
    caption: &str,
    role: &str,
) -> Result<()> {
    let role = match role {
        "expected" | "actual" | "artifact" | "evidence" => role,
        _ => "evidence",
    };
    match target {
        EvidenceTarget::Case(case_id) => {
            if !has_column(conn, "attachment", "case_id") {
                bail!(
                    "expected artifacts need migration 0003 (attachment.case_id); \
                     ask the coordinator to apply the v0.2 migration"
                );
            }
            attach_case_file(conn, data_dir, &case_id, source, caption, role)?;
            return Ok(());
        }
        EvidenceTarget::StepRun(id) => {
            let attachment =
                repo::attach_file(conn, data_dir, EvidenceOwner::StepRun(id), source, caption)?;
            set_role(conn, attachment.id, role)?;
        }
        EvidenceTarget::Run(id) => {
            let attachment =
                repo::attach_file(conn, data_dir, EvidenceOwner::Run(id), source, caption)?;
            set_role(conn, attachment.id, role)?;
        }
        EvidenceTarget::Bug(id) => {
            let attachment =
                repo::attach_file(conn, data_dir, EvidenceOwner::Bug(id), source, caption)?;
            set_role(conn, attachment.id, role)?;
        }
    }
    Ok(())
}

fn set_role(conn: &Connection, attachment_id: i64, role: &str) -> Result<()> {
    if has_column(conn, "attachment", "role") {
        conn.execute(
            "UPDATE attachment SET role=?2 WHERE id=?1",
            params![attachment_id, role],
        )?;
    } else if role == "expected" {
        bail!(
            "expected artifacts need migration 0003 (attachment.role); \
             ask the coordinator to apply the v0.2 migration"
        );
    }
    Ok(())
}

fn attach_case_file(
    conn: &Connection,
    data_dir: &Path,
    case_id: &str,
    source: &Path,
    caption: &str,
    role: &str,
) -> Result<()> {
    let bytes = std::fs::read(source).with_context(|| format!("reading {}", source.display()))?;
    let hash = {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(&bytes);
        hasher
            .finalize()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    };
    let original_name = source
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("evidence.bin");
    let safe: String = original_name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect();
    let evidence_dir = data_dir.join("evidence");
    std::fs::create_dir_all(&evidence_dir)?;
    let dest = evidence_dir.join(format!("{}-{}", &hash[..12], safe));
    std::fs::write(&dest, &bytes)?;
    let relative = dest
        .strip_prefix(data_dir)
        .unwrap_or(&dest)
        .to_string_lossy()
        .replace('\\', "/");
    let kind = match source.extension().and_then(|extension| extension.to_str()) {
        Some("png" | "jpg" | "jpeg" | "gif" | "webp") => "screenshot",
        Some("log" | "txt") => "log",
        _ => "file",
    };
    let mime = match source.extension().and_then(|extension| extension.to_str()) {
        Some("png") => "image/png",
        Some("jpg" | "jpeg") => "image/jpeg",
        Some("gif") => "image/gif",
        Some("webp") => "image/webp",
        Some("pdf") => "application/pdf",
        Some("txt" | "log") => "text/plain",
        _ => "application/octet-stream",
    };
    conn.execute(
        "INSERT INTO attachment (kind,path,original_name,mime,sha256,size,caption,created_at,
                                 step_run_id,run_id,bug_id,case_id,role)
         VALUES (?1,?2,?3,?4,?5,?6,?7,?8,NULL,NULL,NULL,?9,?10)",
        params![
            kind,
            relative,
            original_name,
            mime,
            hash,
            bytes.len() as i64,
            caption,
            crate::model::now(),
            case_id,
            role
        ],
    )?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Entity links
// ---------------------------------------------------------------------------

pub fn list_links(conn: &Connection, kind: &str, id: &str) -> Result<Vec<LinkView>> {
    if !has_table(conn, "entity_link") {
        return Ok(Vec::new());
    }
    let mut stmt = conn.prepare(
        "SELECT id,source_kind,source_id,target_kind,target_id,relation,note,created_at
         FROM entity_link
         WHERE (source_kind=?1 AND source_id=?2) OR (target_kind=?1 AND target_id=?2)
         ORDER BY id",
    )?;
    let rows = stmt.query_map(params![kind, id], |row| {
        Ok(LinkView {
            id: row.get(0)?,
            source_kind: row.get(1)?,
            source_id: row.get(2)?,
            target_kind: row.get(3)?,
            target_id: row.get(4)?,
            relation: row.get(5)?,
            note: row.get(6)?,
            created_at: row.get(7)?,
        })
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn add_link(
    conn: &Connection,
    source_kind: &str,
    source_id: &str,
    target_kind: &str,
    target_id: &str,
    relation: &str,
    note: &str,
) -> Result<bool> {
    if !has_table(conn, "entity_link") {
        return Ok(false);
    }
    conn.execute(
        "INSERT OR IGNORE INTO entity_link
            (source_kind,source_id,target_kind,target_id,relation,note,created_at)
         VALUES (?1,?2,?3,?4,?5,?6,?7)",
        params![
            source_kind,
            source_id,
            target_kind,
            target_id,
            relation,
            note,
            crate::model::now()
        ],
    )?;
    Ok(true)
}

pub fn delete_link(conn: &Connection, id: i64) -> Result<()> {
    if has_table(conn, "entity_link") {
        conn.execute("DELETE FROM entity_link WHERE id=?1", [id])?;
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// Email log
// ---------------------------------------------------------------------------

pub fn email_logs(conn: &Connection, limit: i64) -> Result<Vec<EmailLogView>> {
    if !has_table(conn, "email_log") {
        return Ok(Vec::new());
    }
    let mut stmt = conn.prepare(
        "SELECT id,created_at,to_addr,subject,report_path,status,transport,file_path,error
         FROM email_log ORDER BY id DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map([limit], |row| {
        Ok(EmailLogView {
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
    })?;
    Ok(rows.collect::<rusqlite::Result<Vec<_>>>()?)
}

pub fn project_label(root: &Path) -> String {
    root.file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("Laburo project")
        .to_string()
}
