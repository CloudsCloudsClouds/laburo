//! MCP tool handlers for the v0.2 quality loop.
//!
//! Kept separate from the transport in `src/mcp.rs` so the data pod can evolve
//! the repository without merge conflicts. The coordinator wires these helpers
//! into the MCP dispatch.

use crate::ai::{CheckInFilter, check_in};
use crate::repo::{self, EvidenceOwner};
use rusqlite::Connection;
use serde_json::{Map, Value, json};
use std::path::{Path, PathBuf};

type ToolResult = std::result::Result<Value, String>;

pub fn check_in_tool(conn: &Connection, args: &Map<String, Value>) -> ToolResult {
    let filter = CheckInFilter {
        q: optional_string(args, "q")?,
        case: optional_string(args, "case")?,
        tag: optional_string(args, "tag")?,
    };
    let result = check_in(conn, &filter).map_err(|err| format!("quality check failed: {err:#}"))?;
    serde_json::to_value(result).map_err(|err| format!("serializing check-in: {err}"))
}

pub fn find_tests_tool(conn: &Connection, args: &Map<String, Value>) -> ToolResult {
    let q = optional_string(args, "q")?;
    let tag = optional_string(args, "tag")?;
    let limit = optional_i64(args, "limit")?.unwrap_or(20).clamp(1, 100);

    let mut stmt = conn
        .prepare(
            "SELECT c.id, c.title, c.module, c.status
             FROM test_case c
             WHERE (?1 IS NULL OR c.id LIKE '%' || ?1 || '%'
                    OR c.title LIKE '%' || ?1 || '%'
                    OR c.module LIKE '%' || ?1 || '%')
               AND (?2 IS NULL OR EXISTS (
                    SELECT 1 FROM case_tag ct JOIN tag t ON t.id = ct.tag_id
                    WHERE ct.case_id = c.id AND t.name = ?2))
             ORDER BY c.id LIMIT ?3",
        )
        .map_err(|err| format!("searching cases: {err}"))?;
    let cases: Vec<Value> = stmt
        .query_map(rusqlite::params![q, tag, limit], |row| {
            Ok(json!({
                "id": row.get::<_, String>(0)?,
                "title": row.get::<_, String>(1)?,
                "module": row.get::<_, String>(2)?,
                "status": row.get::<_, String>(3)?,
            }))
        })
        .map_err(|err| format!("searching cases: {err}"))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|err| format!("searching cases: {err}"))?;

    let mut stmt = conn
        .prepare(
            "SELECT id, driver_id, name, file, line
             FROM driver_test
             WHERE (?1 IS NULL OR id LIKE '%' || ?1 || '%'
                    OR name LIKE '%' || ?1 || '%'
                    OR file LIKE '%' || ?1 || '%')
             ORDER BY file, line LIMIT ?2",
        )
        .map_err(|err| format!("searching driver tests: {err}"))?;
    let tests: Vec<Value> = stmt
        .query_map(rusqlite::params![q, limit], |row| {
            Ok(json!({
                "id": row.get::<_, String>(0)?,
                "driver": row.get::<_, String>(1)?,
                "name": row.get::<_, String>(2)?,
                "file": row.get::<_, String>(3)?,
                "line": row.get::<_, Option<i64>>(4)?,
            }))
        })
        .map_err(|err| format!("searching driver tests: {err}"))?
        .collect::<rusqlite::Result<Vec<_>>>()
        .map_err(|err| format!("searching driver tests: {err}"))?;

    Ok(json!({
        "query": q,
        "tag": tag,
        "cases": cases,
        "tests": tests,
    }))
}

pub fn link_tool(conn: &Connection, args: &Map<String, Value>) -> ToolResult {
    let source_kind = required_string(args, "source_kind")?;
    let source_id = required_string(args, "source_id")?;
    let target_kind = required_string(args, "target_kind")?;
    let target_id = required_string(args, "target_id")?;
    let relation = optional_string(args, "relation")?.unwrap_or_else(|| "related".to_string());
    let note = optional_string(args, "note")?.unwrap_or_default();
    let link = repo::create_link(
        conn,
        &source_kind,
        &source_id,
        &target_kind,
        &target_id,
        &relation,
        &note,
    )
    .map_err(|err| format!("creating link: {err:#}"))?;
    serde_json::to_value(link).map_err(|err| format!("serializing link: {err}"))
}

pub fn unlink_tool(conn: &Connection, args: &Map<String, Value>) -> ToolResult {
    let id = required_i64(args, "id")?;
    let deleted = repo::delete_link(conn, id).map_err(|err| format!("deleting link: {err:#}"))?;
    Ok(json!({"id": id, "deleted": deleted}))
}

pub fn list_links_tool(conn: &Connection, args: &Map<String, Value>) -> ToolResult {
    let kind = required_string(args, "kind")?;
    let id = required_string(args, "id")?;
    let links =
        repo::list_links(conn, &kind, &id).map_err(|err| format!("listing links: {err:#}"))?;
    serde_json::to_value(links).map_err(|err| format!("serializing links: {err}"))
}

pub fn record_evidence_tool(
    project_root: &Path,
    data_dir: &Path,
    conn: &Connection,
    args: &Map<String, Value>,
) -> ToolResult {
    let path = required_string(args, "path")?;
    let caption = optional_string(args, "caption")?.unwrap_or_default();
    let role = optional_string(args, "role")?.unwrap_or_else(|| "evidence".to_string());
    let case_id = optional_string(args, "case_id")?;
    let run_id = optional_string(args, "run_id")?;
    let bug_id = optional_string(args, "bug_id")?;
    let step_run_id = optional_i64(args, "step_run_id")?;
    let owners = usize::from(case_id.is_some())
        + usize::from(run_id.is_some())
        + usize::from(bug_id.is_some())
        + usize::from(step_run_id.is_some());
    if owners != 1 {
        return Err("provide exactly one of case_id, run_id, step_run_id, or bug_id".to_string());
    }
    let source = PathBuf::from(&path);
    if !source.is_file() {
        return Err(format!("evidence file not found: {path}"));
    }
    let canonical = source
        .canonicalize()
        .map_err(|err| format!("resolving evidence path: {err}"))?;
    let root = project_root
        .canonicalize()
        .unwrap_or_else(|_| project_root.to_path_buf());
    let temp = std::env::temp_dir()
        .canonicalize()
        .unwrap_or_else(|_| std::env::temp_dir());
    if !canonical.starts_with(&root) && !canonical.starts_with(&temp) {
        return Err("evidence must be inside the project or the temp directory".to_string());
    }
    let owner = if let Some(id) = case_id {
        EvidenceOwner::Case(id)
    } else if let Some(id) = run_id {
        EvidenceOwner::Run(id)
    } else if let Some(id) = bug_id {
        EvidenceOwner::Bug(id)
    } else {
        EvidenceOwner::StepRun(step_run_id.expect("checked above"))
    };
    let attachment =
        repo::attach_file_with_role(conn, data_dir, owner, &canonical, &caption, &role)
            .map_err(|err| format!("attaching evidence: {err:#}"))?;
    serde_json::to_value(attachment).map_err(|err| format!("serializing attachment: {err}"))
}

fn required_string(args: &Map<String, Value>, key: &str) -> std::result::Result<String, String> {
    match args.get(key) {
        Some(Value::String(value)) if !value.trim().is_empty() => Ok(value.clone()),
        Some(Value::String(_)) | None => Err(format!("missing required string argument: {key}")),
        Some(_) => Err(format!("argument {key} must be a string")),
    }
}

fn optional_string(
    args: &Map<String, Value>,
    key: &str,
) -> std::result::Result<Option<String>, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(value)) => Ok(Some(value.clone())),
        Some(_) => Err(format!("argument {key} must be a string")),
    }
}

fn required_i64(args: &Map<String, Value>, key: &str) -> std::result::Result<i64, String> {
    optional_i64(args, key)?.ok_or_else(|| format!("missing required integer argument: {key}"))
}

fn optional_i64(args: &Map<String, Value>, key: &str) -> std::result::Result<Option<i64>, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Number(value)) => value
            .as_i64()
            .map(Some)
            .ok_or_else(|| format!("argument {key} must be an integer")),
        Some(_) => Err(format!("argument {key} must be an integer")),
    }
}

pub fn tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "laburo_check_in",
            "description": "Quality gate: relevant cases, mapped/unmapped coverage, last results, evidence gaps, open bugs, next actions, pass/warn/fail.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "q": {"type": "string", "description": "Feature text, case title, module, or step text."},
                    "case": {"type": "string", "description": "Exact case id."},
                    "tag": {"type": "string", "description": "Tag such as smoke, security, system."}
                },
                "additionalProperties": false
            }
        }),
        json!({
            "name": "laburo_find_tests",
            "description": "Search test cases and discovered driver tests by text/tag.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "q": {"type": "string"},
                    "tag": {"type": "string"},
                    "limit": {"type": "integer", "minimum": 1, "maximum": 100}
                },
                "additionalProperties": false
            }
        }),
        json!({
            "name": "laburo_link",
            "description": "Link two records, e.g. a bug to the case that reproduces it.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "source_kind": {"type": "string"},
                    "source_id": {"type": "string"},
                    "target_kind": {"type": "string"},
                    "target_id": {"type": "string"},
                    "relation": {"type": "string"},
                    "note": {"type": "string"}
                },
                "required": ["source_kind", "source_id", "target_kind", "target_id"],
                "additionalProperties": false
            }
        }),
        json!({
            "name": "laburo_unlink",
            "description": "Remove an entity link by id.",
            "inputSchema": {
                "type": "object",
                "properties": {"id": {"type": "integer", "minimum": 1}},
                "required": ["id"],
                "additionalProperties": false
            }
        }),
        json!({
            "name": "laburo_list_links",
            "description": "List links touching a record.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "kind": {"type": "string"},
                    "id": {"type": "string"}
                },
                "required": ["kind", "id"],
                "additionalProperties": false
            }
        }),
        json!({
            "name": "laburo_record_evidence",
            "description": "Attach a file to a case, step run, run, or bug.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "case_id": {"type": "string"},
                    "run_id": {"type": "string"},
                    "step_run_id": {"type": "integer"},
                    "bug_id": {"type": "string"},
                    "path": {"type": "string"},
                    "caption": {"type": "string"},
                    "role": {"type": "string", "enum": ["expected", "actual", "artifact", "evidence"]}
                },
                "required": ["path"],
                "additionalProperties": false
            }
        }),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;

    #[test]
    fn check_in_empty_db_fails() {
        let db = Db::open_memory().unwrap();
        let value = check_in_tool(&db.conn, &Map::new()).unwrap();
        assert_eq!(value["status"], "fail");
    }

    #[test]
    fn find_tests_returns_empty_arrays() {
        let db = Db::open_memory().unwrap();
        let value = find_tests_tool(&db.conn, &Map::new()).unwrap();
        assert_eq!(value["cases"].as_array().unwrap().len(), 0);
        assert_eq!(value["tests"].as_array().unwrap().len(), 0);
    }

    #[test]
    fn tool_definitions_include_the_quality_loop() {
        let names: Vec<String> = tool_definitions()
            .iter()
            .filter_map(|tool| tool["name"].as_str().map(str::to_string))
            .collect();
        assert!(names.contains(&"laburo_check_in".to_string()));
        assert!(names.contains(&"laburo_record_evidence".to_string()));
    }
}
