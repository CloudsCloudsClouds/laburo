//! Minimal stdio MCP server.
//!
//! JSON-RPC 2.0 over stdin/stdout, one message per line. `Content-Length`
//! framing is also accepted for clients that use it. Diagnostics go to stderr;
//! stdout carries protocol messages only.

use std::io::{self, BufRead, Write};
use std::path::Path;
use std::str::FromStr;

use anyhow::{Context, Result, bail};
use rusqlite::Connection;
use serde_json::{Map, Value, json};

use crate::ai;
use crate::config::ProjectConfig;
use crate::db::Db;
use crate::driver;
use crate::model::{RunMode, RunStatus, StepKind, TestCase, TestOutcome, TestRun};
use crate::repo::{self, EvidenceOwner};
use crate::report::{self, ReportKind};

const MAX_FRAME_BYTES: usize = 8 * 1024 * 1024;
const MAX_ID_CHARS: usize = 128;

struct Server {
    config: ProjectConfig,
    conn: Connection,
}

/// Run the MCP server for `project` until stdin closes.
pub fn run(project: &Path) -> Result<()> {
    let config = ProjectConfig::resolve(Some(project.to_path_buf()))?;
    if !config.root.is_dir() {
        bail!("project root is not a directory: {}", config.root.display());
    }
    config.ensure_dirs()?;
    let db = Db::open(&config.db_path)?;
    let server = Server {
        config,
        conn: db.conn,
    };
    eprintln!(
        "laburo mcp: serving {} (db {})",
        server.config.root.display(),
        server.config.db_path.display()
    );
    let stdin = io::stdin();
    let stdout = io::stdout();
    let mut writer = stdout.lock();
    serve(stdin.lock(), &mut writer, &server)
}

fn serve<R: BufRead, W: Write>(mut reader: R, writer: &mut W, server: &Server) -> Result<()> {
    loop {
        let payload = match next_payload(&mut reader) {
            Ok(Some(payload)) => payload,
            Ok(None) => break,
            Err(err) => {
                eprintln!("laburo mcp: transport error: {err}");
                break;
            }
        };
        let payload = payload.trim();
        if payload.is_empty() {
            continue;
        }
        let response = match serde_json::from_str::<Value>(payload) {
            Ok(message) => dispatch(server, &message),
            Err(err) => Some(rpc_error(
                Value::Null,
                -32700,
                format!("parse error: {err}"),
            )),
        };
        if let Some(response) = response {
            let line = serde_json::to_string(&response)?;
            writeln!(writer, "{line}").context("writing MCP response")?;
            writer.flush().context("flushing MCP response")?;
        }
    }
    Ok(())
}

fn next_payload<R: BufRead>(reader: &mut R) -> io::Result<Option<String>> {
    loop {
        let mut line = String::new();
        if reader.read_line(&mut line)? == 0 {
            return Ok(None);
        }
        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }
        if trimmed.starts_with('{') || trimmed.starts_with('[') {
            return Ok(Some(line));
        }
        let mut length = content_length(trimmed)?;
        loop {
            let mut header = String::new();
            if reader.read_line(&mut header)? == 0 {
                return Ok(None);
            }
            let header = header.trim();
            if header.is_empty() {
                break;
            }
            if length.is_none() {
                length = content_length(header)?;
            }
        }
        let length = length.ok_or_else(|| {
            io::Error::new(io::ErrorKind::InvalidData, "missing Content-Length header")
        })?;
        if length > MAX_FRAME_BYTES {
            return Err(io::Error::new(
                io::ErrorKind::InvalidData,
                "Content-Length exceeds the 8 MiB limit",
            ));
        }
        let mut frame = vec![0_u8; length];
        reader.read_exact(&mut frame)?;
        return String::from_utf8(frame)
            .map(Some)
            .map_err(|err| io::Error::new(io::ErrorKind::InvalidData, err.to_string()));
    }
}

fn content_length(line: &str) -> io::Result<Option<usize>> {
    let Some((name, value)) = line.split_once(':') else {
        return Ok(None);
    };
    if !name.trim().eq_ignore_ascii_case("content-length") {
        return Ok(None);
    }
    value
        .trim()
        .parse::<usize>()
        .map(Some)
        .map_err(|_| io::Error::new(io::ErrorKind::InvalidData, "invalid Content-Length header"))
}

fn dispatch(server: &Server, message: &Value) -> Option<Value> {
    let Some(object) = message.as_object() else {
        return Some(rpc_error(
            Value::Null,
            -32600,
            "invalid request: expected a JSON object",
        ));
    };
    let id = object.get("id").cloned().unwrap_or(Value::Null);
    if !object.contains_key("id") {
        let method = object
            .get("method")
            .and_then(Value::as_str)
            .unwrap_or("<unknown>");
        if method != "notifications/initialized" && method != "initialized" {
            eprintln!("laburo mcp: ignoring notification {method}");
        }
        return None;
    }
    if object.get("jsonrpc").and_then(Value::as_str) != Some("2.0") {
        return Some(rpc_error(
            id,
            -32600,
            "invalid request: jsonrpc must be \"2.0\"",
        ));
    }
    let Some(method) = object.get("method").and_then(Value::as_str) else {
        return Some(rpc_error(
            id,
            -32600,
            "invalid request: method must be a string",
        ));
    };
    let params = object.get("params").unwrap_or(&Value::Null);
    match method {
        "initialize" => Some(rpc_result(id, initialize_result())),
        "ping" => Some(rpc_result(id, json!({}))),
        "tools/list" => Some(rpc_result(id, json!({"tools": tool_definitions()}))),
        "tools/call" => match call_tool(server, params) {
            Ok(result) => Some(rpc_result(id, result)),
            Err(message) => Some(rpc_error(id, -32602, message)),
        },
        other => Some(rpc_error(id, -32601, format!("method not found: {other}"))),
    }
}

fn initialize_result() -> Value {
    json!({
        "protocolVersion": "2025-06-18",
        "serverInfo": {
            "name": "laburo",
            "version": env!("CARGO_PKG_VERSION"),
        },
        "capabilities": {
            "tools": {},
        },
    })
}

fn rpc_result(id: Value, result: Value) -> Value {
    json!({"jsonrpc": "2.0", "id": id, "result": result})
}

fn rpc_error(id: Value, code: i64, message: impl Into<String>) -> Value {
    let message = message.into();
    json!({
        "jsonrpc": "2.0",
        "id": id,
        "error": {"code": code, "message": message},
    })
}

fn call_tool(server: &Server, params: &Value) -> std::result::Result<Value, String> {
    let Some(params) = params.as_object() else {
        return Err("tools/call params must be an object".to_string());
    };
    let Some(name) = params.get("name").and_then(Value::as_str) else {
        return Err("tools/call requires a string name".to_string());
    };
    let empty = Map::new();
    let args = match params.get("arguments") {
        None | Some(Value::Null) => &empty,
        Some(Value::Object(map)) => map,
        Some(_) => return Err("tools/call arguments must be an object".to_string()),
    };

    let outcome = match name {
        "laburo_doctor" => tool_doctor(server),
        "laburo_list_cases" => tool_list_cases(server, args),
        "laburo_get_case" => tool_get_case(server, args),
        "laburo_create_case" => tool_create_case(server, args),
        "laburo_discover_tests" => tool_discover_tests(server, args),
        "laburo_run_case" => tool_run_case(server, args),
        "laburo_record_step" => tool_record_step(server, args),
        "laburo_attach_evidence" => tool_attach_evidence(server, args),
        "laburo_finish_run" => tool_finish_run(server, args),
        "laburo_list_runs" => tool_list_runs(server, args),
        "laburo_get_run" => tool_get_run(server, args),
        "laburo_create_bug" => tool_create_bug(server, args),
        "laburo_list_bugs" => tool_list_bugs(server, args),
        "laburo_generate_report" => tool_generate_report(server, args),
        "laburo_quality_check" => tool_quality_check(server, args),
        "laburo_dashboard" => tool_dashboard(server),
        other => {
            return Err(format!("unknown tool: {other}"));
        }
    };

    Ok(match outcome {
        Ok(value) => tool_result(value),
        Err(message) => tool_error_result(&message),
    })
}

fn tool_result(value: Value) -> Value {
    json!({
        "content": [{"type": "text", "text": value.to_string()}],
        "isError": false,
    })
}

fn tool_error_result(message: &str) -> Value {
    let text = json!({"error": message});
    json!({
        "content": [{"type": "text", "text": text.to_string()}],
        "isError": true,
    })
}

fn tool_doctor(server: &Server) -> std::result::Result<Value, String> {
    let counts = repo::dashboard(&server.conn).map_err(repo_error)?;
    let gemfile = server.config.root.join("Gemfile");
    let minitest = std::fs::read_to_string(&gemfile)
        .map(|text| text.contains("minitest"))
        .unwrap_or(false);
    let checks = vec![
        json!({
            "name": "project root",
            "ok": server.config.root.is_dir(),
            "detail": server.config.root.display().to_string(),
        }),
        json!({
            "name": "data directory",
            "ok": server.config.data_dir.is_dir(),
            "detail": server.config.data_dir.display().to_string(),
        }),
        json!({
            "name": "database",
            "ok": server.config.db_path.is_file(),
            "detail": server.config.db_path.display().to_string(),
        }),
        json!({
            "name": "minitest",
            "ok": minitest,
            "detail": gemfile.display().to_string(),
        }),
    ];
    let ok = checks
        .iter()
        .all(|check| check.get("ok").and_then(Value::as_bool).unwrap_or(false));
    Ok(json!({
        "ok": ok,
        "server": {"name": "laburo", "version": env!("CARGO_PKG_VERSION")},
        "project": server.config.root.display().to_string(),
        "checks": checks,
        "counts": counts,
    }))
}

fn tool_list_cases(
    server: &Server,
    args: &Map<String, Value>,
) -> std::result::Result<Value, String> {
    let tag = optional_string(args, "tag")?;
    let status = optional_string(args, "status")?;
    if let Some(status) = &status {
        validate_choice("case status", status, &["draft", "active", "archived"])?;
    }
    if let Some(tag) = &tag {
        validate_tag(tag)?;
    }
    let cases =
        repo::list_cases(&server.conn, tag.as_deref(), status.as_deref()).map_err(repo_error)?;
    let summaries: Vec<Value> = cases.iter().map(case_summary).collect();
    Ok(json!({"count": summaries.len(), "cases": summaries}))
}

fn tool_get_case(server: &Server, args: &Map<String, Value>) -> std::result::Result<Value, String> {
    let id = required_string(args, "id")?;
    validate_id("case", &id)?;
    let case = repo::get_case(&server.conn, &id)
        .map_err(repo_error)?
        .ok_or_else(|| format!("case not found: {id}"))?;
    serde_json::to_value(&case).map_err(|err| format!("serializing case: {err}"))
}

fn tool_create_case(
    server: &Server,
    args: &Map<String, Value>,
) -> std::result::Result<Value, String> {
    let id = required_string(args, "id")?;
    validate_id("case", &id)?;
    let title = required_string(args, "title")?;
    let description = optional_string(args, "description")?.unwrap_or_default();
    let tags = optional_string_array(args, "tags")?;
    for tag in &tags {
        validate_tag(tag)?;
    }
    let steps = parse_steps(args)?;

    repo::create_case(
        &server.conn,
        &id,
        &title,
        &description,
        "",
        "medium",
        "",
        "",
        &tags,
    )
    .map_err(|err| format!("creating case {id}: {err:#}"))?;
    for (index, step) in steps.iter().enumerate() {
        let position = index as i64 + 1;
        if let Err(err) = repo::add_step(
            &server.conn,
            &id,
            position,
            &step.action,
            &step.data,
            &step.expected,
            step.kind,
            step.driver_target.as_deref(),
        ) {
            let _ = repo::delete_case(&server.conn, &id);
            return Err(format!("adding step {position} to case {id}: {err:#}"));
        }
    }

    let case = repo::get_case(&server.conn, &id)
        .map_err(repo_error)?
        .ok_or_else(|| format!("case disappeared after creation: {id}"))?;
    serde_json::to_value(&case).map_err(|err| format!("serializing case: {err}"))
}

fn tool_discover_tests(
    server: &Server,
    args: &Map<String, Value>,
) -> std::result::Result<Value, String> {
    let driver_id = optional_string(args, "driver")?.unwrap_or_else(|| "minitest".to_string());
    if driver_id != "minitest" {
        return Err(format!(
            "unsupported driver: {driver_id}; only minitest is available"
        ));
    }
    let tests = driver::discover_minitest(&server.config.root)
        .map_err(|err| format!("Minitest discovery failed: {err:#}"))?;
    let cached = if tests.is_empty() {
        false
    } else {
        repo::ensure_driver(&server.conn, "minitest", "minitest", "Minitest")
            .map_err(repo_error)?;
        repo::save_driver_tests(&server.conn, "minitest", &tests).map_err(repo_error)?;
        true
    };
    Ok(json!({
        "driver": "minitest",
        "count": tests.len(),
        "cached": cached,
        "tests": tests,
    }))
}

fn tool_run_case(server: &Server, args: &Map<String, Value>) -> std::result::Result<Value, String> {
    let id = required_string(args, "id")?;
    validate_id("case", &id)?;
    let mode_text = optional_string(args, "mode")?.unwrap_or_else(|| "manual".to_string());
    let mode = RunMode::from_str(&mode_text).map_err(|err| format!("invalid mode: {err}"))?;
    let keep_going = optional_bool(args, "keep_going")?.unwrap_or(false);
    let case = repo::get_case(&server.conn, &id)
        .map_err(repo_error)?
        .ok_or_else(|| format!("case not found: {id}"))?;
    let run = repo::create_run(&server.conn, &id, mode, &case.environment)
        .map_err(|err| format!("creating run for {id}: {err:#}"))?;
    match mode {
        RunMode::Manual => Ok(manual_run_payload(&run)),
        RunMode::Single | RunMode::Composed => execute_automated(server, &case, &run, keep_going),
    }
}

fn tool_record_step(
    server: &Server,
    args: &Map<String, Value>,
) -> std::result::Result<Value, String> {
    let run_id = required_string(args, "run_id")?;
    validate_id("run", &run_id)?;
    let position =
        optional_i64(args, "position")?.ok_or_else(|| "position is required".to_string())?;
    if position < 1 {
        return Err("position must be >= 1".to_string());
    }
    let status_text = required_string(args, "status")?;
    let status =
        RunStatus::from_str(&status_text).map_err(|err| format!("invalid status: {err}"))?;
    let observed = optional_string(args, "observed")?.unwrap_or_default();
    let notes = optional_string(args, "notes")?.unwrap_or_default();
    let duration_ms = optional_i64(args, "duration_ms")?;
    let driver_test = optional_string(args, "driver_test")?;
    let raw_output = optional_string(args, "raw_output")?.unwrap_or_default();
    let step = repo::record_step(
        &server.conn,
        &run_id,
        position,
        status,
        &observed,
        &notes,
        duration_ms,
        driver_test.as_deref(),
        &raw_output,
    )
    .map_err(repo_error)?;
    serde_json::to_value(step).map_err(|err| format!("serializing step: {err}"))
}

fn tool_finish_run(
    server: &Server,
    args: &Map<String, Value>,
) -> std::result::Result<Value, String> {
    let run_id = required_string(args, "run_id")?;
    validate_id("run", &run_id)?;
    let run = repo::finish_run(&server.conn, &run_id).map_err(repo_error)?;
    serde_json::to_value(run).map_err(|err| format!("serializing run: {err}"))
}

fn tool_attach_evidence(
    server: &Server,
    args: &Map<String, Value>,
) -> std::result::Result<Value, String> {
    let path = required_string(args, "path")?;
    let caption = optional_string(args, "caption")?.unwrap_or_default();
    let run_id = optional_string(args, "run_id")?;
    let bug_id = optional_string(args, "bug_id")?;
    let step_run_id = optional_i64(args, "step_run_id")?;
    let owners = usize::from(run_id.is_some())
        + usize::from(bug_id.is_some())
        + usize::from(step_run_id.is_some());
    if owners != 1 {
        return Err("provide exactly one of run_id, step_run_id, or bug_id".to_string());
    }
    if let Some(id) = &run_id {
        validate_id("run", id)?;
    }
    if let Some(id) = &bug_id {
        validate_id("bug", id)?;
    }
    let source = Path::new(&path);
    if !source.is_file() {
        return Err(format!("evidence file not found: {path}"));
    }
    let canonical = source
        .canonicalize()
        .map_err(|err| format!("resolving evidence path: {err}"))?;
    let root = server
        .config
        .root
        .canonicalize()
        .unwrap_or_else(|_| server.config.root.clone());
    let temp = std::env::temp_dir()
        .canonicalize()
        .unwrap_or_else(|_| std::env::temp_dir());
    if !canonical.starts_with(&root) && !canonical.starts_with(&temp) {
        return Err("evidence must be inside the project or the temp directory".to_string());
    }
    let owner = if let Some(id) = run_id {
        EvidenceOwner::Run(id)
    } else if let Some(id) = bug_id {
        EvidenceOwner::Bug(id)
    } else {
        EvidenceOwner::StepRun(step_run_id.expect("checked above"))
    };
    let attachment = repo::attach_file(
        &server.conn,
        &server.config.data_dir,
        owner,
        &canonical,
        &caption,
    )
    .map_err(repo_error)?;
    serde_json::to_value(attachment).map_err(|err| format!("serializing attachment: {err}"))
}

fn manual_run_payload(run: &TestRun) -> Value {
    let steps: Vec<Value> = run
        .steps
        .iter()
        .map(|step| {
            json!({
                "step_run_id": step.id,
                "step_id": step.step_id,
                "position": step.position,
                "status": step.status.as_str(),
            })
        })
        .collect();
    json!({
        "run_id": run.id,
        "case_id": run.case_id,
        "mode": run.mode.as_str(),
        "status": run.status.as_str(),
        "steps": steps,
        "next_action": "Record each step result, then finish the run.",
    })
}

fn execute_automated(
    server: &Server,
    case: &TestCase,
    run: &TestRun,
    keep_going: bool,
) -> std::result::Result<Value, String> {
    let targets: Vec<String> = case
        .steps
        .iter()
        .filter_map(|step| step.driver_target.clone())
        .collect();
    match driver::run_minitest(&server.config.root, &targets) {
        Ok(outcomes) if outcomes.is_empty() => {
            let message =
                "Minitest driver returned no outcomes; automated execution is not available yet";
            record_run_error(server, run, message);
            Err(format!("{message} (run {})", run.id))
        }
        Ok(outcomes) => record_outcomes(server, run, &outcomes, keep_going),
        Err(err) => {
            let message = format!("Minitest driver failed: {err:#}");
            record_run_error(server, run, &message);
            Err(format!("{message} (run {})", run.id))
        }
    }
}

fn record_run_error(server: &Server, run: &TestRun, message: &str) {
    for step in &run.steps {
        if let Err(err) = repo::record_step(
            &server.conn,
            &run.id,
            step.position,
            RunStatus::Error,
            message,
            "",
            None,
            step.driver_test.as_deref(),
            message,
        ) {
            eprintln!(
                "laburo mcp: failed to record error for run {} step {}: {err:#}",
                run.id, step.position
            );
        }
    }
    if let Err(err) = repo::finish_run(&server.conn, &run.id) {
        eprintln!(
            "laburo mcp: failed to finish errored run {}: {err:#}",
            run.id
        );
    }
}

fn record_outcomes(
    server: &Server,
    run: &TestRun,
    outcomes: &[TestOutcome],
    keep_going: bool,
) -> std::result::Result<Value, String> {
    let mut used = vec![false; outcomes.len()];
    let mut unmatched = 0_usize;
    for step in &run.steps {
        let Some(index) = match_outcome(outcomes, &used, step.driver_test.as_deref()) else {
            unmatched += 1;
            continue;
        };
        used[index] = true;
        let outcome = &outcomes[index];
        repo::record_step(
            &server.conn,
            &run.id,
            step.position,
            outcome.status,
            &outcome.message,
            &outcome.backtrace,
            outcome.duration_ms,
            Some(outcome.target.as_str()),
            &outcome.raw_output,
        )
        .map_err(|err| format!("recording step {}: {err:#}", step.position))?;
    }

    let finished = repo::finish_run(&server.conn, &run.id)
        .map_err(|err| format!("finishing run {}: {err:#}", run.id))?;
    let steps: Vec<Value> = finished
        .steps
        .iter()
        .map(|step| {
            json!({
                "step_run_id": step.id,
                "step_id": step.step_id,
                "position": step.position,
                "status": step.status.as_str(),
                "driver_test": step.driver_test,
                "duration_ms": step.duration_ms,
            })
        })
        .collect();
    Ok(json!({
        "run_id": finished.id,
        "case_id": finished.case_id,
        "mode": finished.mode.as_str(),
        "status": finished.status.as_str(),
        "keep_going": keep_going,
        "unmatched_targets": unmatched,
        "steps": steps,
    }))
}

fn match_outcome(outcomes: &[TestOutcome], used: &[bool], target: Option<&str>) -> Option<usize> {
    match target {
        Some(target) => outcomes
            .iter()
            .enumerate()
            .position(|(index, outcome)| !used[index] && outcome.target == target),
        None => used.iter().position(|used| !used),
    }
}

fn tool_list_runs(
    server: &Server,
    args: &Map<String, Value>,
) -> std::result::Result<Value, String> {
    let case_id = optional_string(args, "case")?;
    if let Some(id) = &case_id {
        validate_id("case", id)?;
    }
    let limit = optional_i64(args, "limit")?.unwrap_or(20);
    if !(1..=200).contains(&limit) {
        return Err("limit must be between 1 and 200".to_string());
    }
    let runs = repo::list_runs(&server.conn, case_id.as_deref(), limit).map_err(repo_error)?;
    let summaries: Vec<Value> = runs.iter().map(run_summary).collect();
    Ok(json!({"count": summaries.len(), "runs": summaries}))
}

fn tool_get_run(server: &Server, args: &Map<String, Value>) -> std::result::Result<Value, String> {
    let id = required_string(args, "id")?;
    validate_id("run", &id)?;
    let run = repo::get_run(&server.conn, &id)
        .map_err(repo_error)?
        .ok_or_else(|| format!("run not found: {id}"))?;
    serde_json::to_value(&run).map_err(|err| format!("serializing run: {err}"))
}

fn tool_create_bug(
    server: &Server,
    args: &Map<String, Value>,
) -> std::result::Result<Value, String> {
    let id = optional_string(args, "id")?;
    if let Some(id) = &id {
        validate_id("bug", id)?;
    }
    let title = required_string(args, "title")?;
    let description = optional_string(args, "description")?.unwrap_or_default();
    let steps = optional_string(args, "steps")?.unwrap_or_default();
    let actual = optional_string(args, "actual")?.unwrap_or_default();
    let expected = optional_string(args, "expected")?.unwrap_or_default();
    let environment = optional_string(args, "environment")?.unwrap_or_default();
    let comments = optional_string(args, "comments")?.unwrap_or_default();
    let priority = optional_i64(args, "priority")?.unwrap_or(3);
    let severity = optional_i64(args, "severity")?.unwrap_or(3);
    if !(1..=5).contains(&priority) {
        return Err("priority must be between 1 and 5".to_string());
    }
    if !(1..=5).contains(&severity) {
        return Err("severity must be between 1 and 5".to_string());
    }
    let case_id = optional_string(args, "case_id")?;
    if let Some(case_id) = &case_id {
        validate_id("case", case_id)?;
        if repo::get_case(&server.conn, case_id)
            .map_err(repo_error)?
            .is_none()
        {
            return Err(format!("case not found: {case_id}"));
        }
    }
    let run_id = optional_string(args, "run_id")?;
    if let Some(run_id) = &run_id {
        validate_id("run", run_id)?;
        if repo::get_run(&server.conn, run_id)
            .map_err(repo_error)?
            .is_none()
        {
            return Err(format!("run not found: {run_id}"));
        }
    }
    let step_run_id = optional_i64(args, "step_run_id")?;
    if let Some(step_run_id) = step_run_id {
        let exists: i64 = server
            .conn
            .query_row(
                "SELECT count(*) FROM step_run WHERE id=?1",
                [step_run_id],
                |row| row.get(0),
            )
            .map_err(|err| format!("checking step run {step_run_id}: {err:#}"))?;
        if exists == 0 {
            return Err(format!("step run not found: {step_run_id}"));
        }
    }

    let bug = repo::create_bug(
        &server.conn,
        id.as_deref(),
        &title,
        &description,
        &steps,
        &actual,
        &expected,
        priority,
        severity,
        &environment,
        case_id.as_deref(),
        run_id.as_deref(),
        step_run_id,
        &comments,
    )
    .map_err(|err| format!("creating bug: {err:#}"))?;
    serde_json::to_value(&bug).map_err(|err| format!("serializing bug: {err}"))
}

fn tool_list_bugs(
    server: &Server,
    args: &Map<String, Value>,
) -> std::result::Result<Value, String> {
    let status = optional_string(args, "status")?;
    if let Some(status) = &status {
        validate_choice(
            "bug status",
            status,
            &["open", "resolved", "closed", "reopened"],
        )?;
    }
    let severity = optional_i64(args, "severity")?;
    if let Some(severity) = severity {
        if !(1..=5).contains(&severity) {
            return Err("severity must be between 1 and 5".to_string());
        }
    }
    let bugs = repo::list_bugs(&server.conn, status.as_deref()).map_err(repo_error)?;
    let summaries: Vec<Value> = bugs
        .iter()
        .filter(|bug| severity.is_none_or(|value| bug.severity == value))
        .map(|bug| {
            json!({
                "id": bug.id,
                "title": bug.title,
                "status": bug.status,
                "priority": bug.priority,
                "severity": bug.severity,
                "case_id": bug.case_id,
                "run_id": bug.run_id,
                "created_at": bug.created_at,
            })
        })
        .collect();
    Ok(json!({"count": summaries.len(), "bugs": summaries}))
}

fn tool_generate_report(
    server: &Server,
    args: &Map<String, Value>,
) -> std::result::Result<Value, String> {
    let kind_text = required_string(args, "kind")?;
    let kind = ReportKind::parse(&kind_text).map_err(|err| err.to_string())?;
    let id = optional_string(args, "id")?;
    if let Some(id) = &id {
        validate_id("report", id)?;
    }
    let path = report::generate(&server.conn, &server.config.data_dir, kind, id.as_deref())
        .map_err(|err| format!("generating {kind_text} report: {err:#}"))?;
    if !path.starts_with(&server.config.data_dir) {
        return Err(format!(
            "report path escaped the data directory: {}",
            path.display()
        ));
    }
    Ok(json!({
        "kind": kind_text,
        "id": id,
        "path": path.display().to_string(),
    }))
}

fn tool_quality_check(
    server: &Server,
    args: &Map<String, Value>,
) -> std::result::Result<Value, String> {
    let severity = optional_string(args, "severity")?;
    let findings = ai::quality_findings(&server.conn)
        .map_err(|err| format!("quality check failed: {err:#}"))?;
    let findings: Vec<_> = findings
        .into_iter()
        .filter(|finding| {
            severity
                .as_deref()
                .is_none_or(|value| finding.severity.eq_ignore_ascii_case(value))
        })
        .collect();
    let next_action = if findings.is_empty() {
        "No findings; keep recording evidence.".to_string()
    } else {
        format!(
            "Review {} finding(s), highest severity first.",
            findings.len()
        )
    };
    Ok(json!({
        "count": findings.len(),
        "findings": findings,
        "next_action": next_action,
    }))
}

fn tool_dashboard(server: &Server) -> std::result::Result<Value, String> {
    let dashboard = repo::dashboard(&server.conn).map_err(repo_error)?;
    let runs = repo::list_runs(&server.conn, None, 50).map_err(repo_error)?;
    let recent_failures: Vec<Value> = runs
        .iter()
        .filter(|run| run.status.is_bad())
        .take(5)
        .map(run_summary)
        .collect();
    Ok(json!({
        "cases": dashboard.cases,
        "runs": dashboard.runs,
        "bugs_open": dashboard.bugs_open,
        "bugs_total": dashboard.bugs_total,
        "failed_runs": dashboard.failed_runs,
        "passed_runs": dashboard.passed_runs,
        "evidence": dashboard.evidence,
        "pass_rate": dashboard.pass_rate(),
        "recent_failures": recent_failures,
    }))
}

fn case_summary(case: &TestCase) -> Value {
    json!({
        "id": case.id,
        "title": case.title,
        "status": case.status,
        "priority": case.priority,
        "module": case.module,
        "tags": case.tags,
        "steps": case.steps.len(),
    })
}

fn run_summary(run: &TestRun) -> Value {
    json!({
        "id": run.id,
        "case_id": run.case_id,
        "mode": run.mode.as_str(),
        "status": run.status.as_str(),
        "started_at": run.started_at,
        "ended_at": run.ended_at,
        "steps": run.steps.len(),
        "failed_steps": run.steps.iter().filter(|step| step.status.is_bad()).count(),
    })
}

struct NewStep {
    action: String,
    data: String,
    expected: String,
    kind: StepKind,
    driver_target: Option<String>,
}

fn parse_steps(args: &Map<String, Value>) -> std::result::Result<Vec<NewStep>, String> {
    let Some(value) = args.get("steps") else {
        return Ok(Vec::new());
    };
    if value.is_null() {
        return Ok(Vec::new());
    }
    let Some(items) = value.as_array() else {
        return Err("steps must be an array".to_string());
    };
    if items.len() > 200 {
        return Err("too many steps (max 200)".to_string());
    }
    let mut steps = Vec::with_capacity(items.len());
    for (index, item) in items.iter().enumerate() {
        let Some(object) = item.as_object() else {
            return Err(format!("steps[{index}] must be an object"));
        };
        let action =
            required_string(object, "action").map_err(|err| format!("steps[{index}]: {err}"))?;
        let data = optional_string(object, "data")?.unwrap_or_default();
        let expected = optional_string(object, "expected")?.unwrap_or_default();
        let kind_text = optional_string(object, "kind")?.unwrap_or_else(|| "manual".to_string());
        let kind =
            StepKind::from_str(&kind_text).map_err(|err| format!("steps[{index}]: {err}"))?;
        let driver_target = optional_string(object, "driver_target")?;
        if let Some(target) = &driver_target {
            if target.trim().is_empty() {
                return Err(format!("steps[{index}]: driver_target must not be empty"));
            }
        }
        steps.push(NewStep {
            action,
            data,
            expected,
            kind,
            driver_target,
        });
    }
    Ok(steps)
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

fn optional_bool(
    args: &Map<String, Value>,
    key: &str,
) -> std::result::Result<Option<bool>, String> {
    match args.get(key) {
        None | Some(Value::Null) => Ok(None),
        Some(Value::Bool(value)) => Ok(Some(*value)),
        Some(_) => Err(format!("argument {key} must be a boolean")),
    }
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

fn optional_string_array(
    args: &Map<String, Value>,
    key: &str,
) -> std::result::Result<Vec<String>, String> {
    let Some(value) = args.get(key) else {
        return Ok(Vec::new());
    };
    if value.is_null() {
        return Ok(Vec::new());
    }
    let Some(items) = value.as_array() else {
        return Err(format!("argument {key} must be an array of strings"));
    };
    items
        .iter()
        .map(|item| {
            item.as_str()
                .map(str::to_string)
                .ok_or_else(|| format!("argument {key} must be an array of strings"))
        })
        .collect()
}

fn validate_id(kind: &str, id: &str) -> std::result::Result<(), String> {
    if id.is_empty() {
        return Err(format!("{kind} id must not be empty"));
    }
    if id.chars().count() > MAX_ID_CHARS {
        return Err(format!(
            "{kind} id is longer than {MAX_ID_CHARS} characters"
        ));
    }
    if id.trim() != id {
        return Err(format!("{kind} id must not have surrounding whitespace"));
    }
    if !id
        .chars()
        .all(|ch| ch.is_ascii_alphanumeric() || matches!(ch, '-' | '_' | '.' | ':' | '#'))
    {
        return Err(format!("{kind} id contains unsupported characters: {id}"));
    }
    Ok(())
}

fn validate_tag(tag: &str) -> std::result::Result<(), String> {
    let tag = tag.trim();
    if tag.is_empty() {
        return Err("tag must not be empty".to_string());
    }
    if tag.chars().count() > 64 {
        return Err(format!("tag is too long: {tag}"));
    }
    if tag.chars().any(char::is_control) {
        return Err("tag contains control characters".to_string());
    }
    Ok(())
}

fn validate_choice(field: &str, value: &str, allowed: &[&str]) -> std::result::Result<(), String> {
    if allowed.contains(&value) {
        Ok(())
    } else {
        Err(format!(
            "invalid {field}: {value}; expected one of {}",
            allowed.join(", ")
        ))
    }
}

fn repo_error(err: anyhow::Error) -> String {
    format!("database error: {err:#}")
}

fn tool_definitions() -> Vec<Value> {
    vec![
        json!({
            "name": "laburo_doctor",
            "description": "Inspect the project, database, and Minitest environment.",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }
        }),
        json!({
            "name": "laburo_list_cases",
            "description": "List test cases, optionally filtered by tag or status.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "tag": {"type": "string", "description": "Tag name to filter by."},
                    "status": {
                        "type": "string",
                        "enum": ["draft", "active", "archived"]
                    }
                },
                "additionalProperties": false
            }
        }),
        json!({
            "name": "laburo_get_case",
            "description": "Read one test case with its steps and tags.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": {"type": "string", "description": "Case id, e.g. TC-001."}
                },
                "required": ["id"],
                "additionalProperties": false
            }
        }),
        json!({
            "name": "laburo_create_case",
            "description": "Create a test case with optional tags and steps.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": {"type": "string", "description": "Case id, e.g. TC-001."},
                    "title": {"type": "string"},
                    "description": {"type": "string"},
                    "tags": {
                        "type": "array",
                        "items": {"type": "string"}
                    },
                    "steps": {
                        "type": "array",
                        "items": {
                            "type": "object",
                            "properties": {
                                "action": {"type": "string"},
                                "data": {"type": "string"},
                                "expected": {"type": "string"},
                                "kind": {"type": "string", "enum": ["manual", "driver"]},
                                "driver_target": {"type": "string"}
                            },
                            "required": ["action"],
                            "additionalProperties": false
                        }
                    }
                },
                "required": ["id", "title"],
                "additionalProperties": false
            }
        }),
        json!({
            "name": "laburo_discover_tests",
            "description": "Discover Minitest tests and cache them in the database.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "driver": {"type": "string", "enum": ["minitest"]}
                },
                "additionalProperties": false
            }
        }),
        json!({
            "name": "laburo_run_case",
            "description": "Create a run for a case. Manual runs return step ids; automated runs execute the Minitest driver.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": {"type": "string", "description": "Case id."},
                    "mode": {
                        "type": "string",
                        "enum": ["manual", "single", "composed"]
                    },
                    "keep_going": {"type": "boolean"}
                },
                "required": ["id"],
                "additionalProperties": false
            }
        }),
        json!({
            "name": "laburo_record_step",
            "description": "Record one manual step result: pass/fail/blocked/error, observed text, notes, optional driver metadata.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "run_id": {"type": "string"},
                    "position": {"type": "integer", "minimum": 1},
                    "status": {
                        "type": "string",
                        "enum": ["not_run", "running", "passed", "failed", "blocked", "error"]
                    },
                    "observed": {"type": "string"},
                    "notes": {"type": "string"},
                    "duration_ms": {"type": "integer", "minimum": 0},
                    "driver_test": {"type": "string"},
                    "raw_output": {"type": "string"}
                },
                "required": ["run_id", "position", "status"],
                "additionalProperties": false
            }
        }),
        json!({
            "name": "laburo_attach_evidence",
            "description": "Copy a file into the evidence store and link it to a run, step run, or bug.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "run_id": {"type": "string"},
                    "step_run_id": {"type": "integer", "minimum": 1},
                    "bug_id": {"type": "string"},
                    "path": {"type": "string", "description": "Existing file inside the project or temp directory."},
                    "caption": {"type": "string"}
                },
                "required": ["path"],
                "additionalProperties": false
            }
        }),
        json!({
            "name": "laburo_finish_run",
            "description": "Finalize a run and derive its status from the recorded steps.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "run_id": {"type": "string"}
                },
                "required": ["run_id"],
                "additionalProperties": false
            }
        }),
        json!({
            "name": "laburo_list_runs",
            "description": "List recent runs, optionally for one case.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "case": {"type": "string", "description": "Case id to filter by."},
                    "limit": {"type": "integer", "minimum": 1, "maximum": 200}
                },
                "additionalProperties": false
            }
        }),
        json!({
            "name": "laburo_get_run",
            "description": "Read one run with its step results and evidence.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": {"type": "string", "description": "Run id, e.g. RUN-20260101-120000-1."}
                },
                "required": ["id"],
                "additionalProperties": false
            }
        }),
        json!({
            "name": "laburo_create_bug",
            "description": "File a bug report, optionally linked to a case, run, or step run.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "id": {"type": "string", "description": "Optional user-supplied bug id."},
                    "case_id": {"type": "string"},
                    "run_id": {"type": "string"},
                    "step_run_id": {"type": "integer"},
                    "title": {"type": "string"},
                    "description": {"type": "string"},
                    "steps": {"type": "string"},
                    "actual": {"type": "string"},
                    "expected": {"type": "string"},
                    "priority": {"type": "integer", "minimum": 1, "maximum": 5},
                    "severity": {"type": "integer", "minimum": 1, "maximum": 5},
                    "environment": {"type": "string"},
                    "comments": {"type": "string"}
                },
                "required": ["title"],
                "additionalProperties": false
            }
        }),
        json!({
            "name": "laburo_list_bugs",
            "description": "List bug reports, optionally filtered by status and severity.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "status": {
                        "type": "string",
                        "enum": ["open", "resolved", "closed", "reopened"]
                    },
                    "severity": {"type": "integer", "minimum": 1, "maximum": 5}
                },
                "additionalProperties": false
            }
        }),
        json!({
            "name": "laburo_generate_report",
            "description": "Generate a PDF report and return its path.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "kind": {
                        "type": "string",
                        "enum": ["dictionary", "case", "run", "bug"]
                    },
                    "id": {"type": "string", "description": "Case, run, or bug id when the kind needs one."}
                },
                "required": ["kind"],
                "additionalProperties": false
            }
        }),
        json!({
            "name": "laburo_quality_check",
            "description": "Run deterministic quality rules and return findings plus a next action.",
            "inputSchema": {
                "type": "object",
                "properties": {
                    "severity": {"type": "string", "description": "Only return findings with this severity."}
                },
                "additionalProperties": false
            }
        }),
        json!({
            "name": "laburo_dashboard",
            "description": "Return counts, pass rate, and recent failures.",
            "inputSchema": {
                "type": "object",
                "properties": {},
                "additionalProperties": false
            }
        }),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use tempfile::tempdir;

    fn test_server(root: &Path) -> Server {
        let data_dir = root.join(".laburo");
        std::fs::create_dir_all(&data_dir).unwrap();
        let db = Db::open_memory().unwrap();
        Server {
            config: ProjectConfig {
                root: root.to_path_buf(),
                data_dir: data_dir.clone(),
                db_path: data_dir.join("laburo.sqlite3"),
                typst: "typst".to_string(),
            },
            conn: db.conn,
        }
    }

    fn rpc(id: i64, method: &str, params: Value) -> Value {
        json!({"jsonrpc": "2.0", "id": id, "method": method, "params": params})
    }

    fn call(server: &Server, id: i64, tool: &str, arguments: Value) -> Value {
        let response = dispatch(
            server,
            &rpc(
                id,
                "tools/call",
                json!({"name": tool, "arguments": arguments}),
            ),
        )
        .expect("request must produce a response");
        response["result"].clone()
    }

    fn result_json(result: &Value) -> Value {
        let text = result["content"][0]["text"]
            .as_str()
            .expect("tool result must contain JSON text");
        serde_json::from_str(text).expect("tool result text must be JSON")
    }

    #[test]
    fn parses_line_and_content_length_frames() {
        let body = r#"{"jsonrpc":"2.0","id":2,"method":"ping"}"#;
        let input = format!(
            "{}\nContent-Length: {}\r\nContent-Type: application/json\r\n\r\n{}",
            r#"{"jsonrpc":"2.0","id":1,"method":"ping"}"#,
            body.len(),
            body
        );
        let mut reader = std::io::BufReader::new(Cursor::new(input.into_bytes()));

        let first = next_payload(&mut reader).unwrap().unwrap();
        assert!(first.contains(r#""id":1"#));
        let second = next_payload(&mut reader).unwrap().unwrap();
        assert_eq!(second, body);
        assert!(next_payload(&mut reader).unwrap().is_none());
    }

    #[test]
    fn initializes_and_lists_tools() {
        let dir = tempdir().unwrap();
        let server = test_server(dir.path());

        let response = dispatch(&server, &rpc(1, "initialize", json!({}))).unwrap();
        assert_eq!(response["result"]["protocolVersion"], "2025-06-18");
        assert_eq!(response["result"]["serverInfo"]["name"], "laburo");
        assert!(response["result"]["capabilities"]["tools"].is_object());

        let response = dispatch(&server, &rpc(2, "tools/list", json!({}))).unwrap();
        let tools = response["result"]["tools"].as_array().unwrap();
        assert_eq!(tools.len(), 16);
        for name in [
            "laburo_doctor",
            "laburo_list_cases",
            "laburo_get_case",
            "laburo_create_case",
            "laburo_discover_tests",
            "laburo_run_case",
            "laburo_record_step",
            "laburo_attach_evidence",
            "laburo_finish_run",
            "laburo_list_runs",
            "laburo_get_run",
            "laburo_create_bug",
            "laburo_list_bugs",
            "laburo_generate_report",
            "laburo_quality_check",
            "laburo_dashboard",
        ] {
            assert!(
                tools.iter().any(|tool| tool["name"] == name),
                "missing tool {name}"
            );
        }

        assert!(
            dispatch(
                &server,
                &json!({"jsonrpc": "2.0", "method": "notifications/initialized"})
            )
            .is_none()
        );
        let unknown = dispatch(&server, &rpc(3, "nope", json!({}))).unwrap();
        assert_eq!(unknown["error"]["code"], -32601);
    }

    #[test]
    fn creates_case_and_starts_manual_run() {
        let dir = tempdir().unwrap();
        let server = test_server(dir.path());

        let created = call(
            &server,
            1,
            "laburo_create_case",
            json!({
                "id": "TC-001",
                "title": "Book a tour",
                "description": "Smoke the booking form.",
                "tags": ["smoke"],
                "steps": [
                    {"action": "Open the booking form.", "expected": "Form is visible."}
                ]
            }),
        );
        assert_eq!(created["isError"], false, "{created}");
        let case = result_json(&created);
        assert_eq!(case["id"], "TC-001");
        assert_eq!(case["steps"].as_array().unwrap().len(), 1);
        assert_eq!(case["tags"][0], "smoke");

        let listed = call(&server, 2, "laburo_list_cases", json!({}));
        assert_eq!(listed["isError"], false, "{listed}");
        assert_eq!(result_json(&listed)["count"], 1);

        let started = call(
            &server,
            3,
            "laburo_run_case",
            json!({"id": "TC-001", "mode": "manual"}),
        );
        assert_eq!(started["isError"], false, "{started}");
        let run = result_json(&started);
        assert!(run["run_id"].as_str().unwrap().starts_with("RUN-"));
        assert_eq!(run["mode"], "manual");
        assert_eq!(run["steps"][0]["status"], "not_run");
        let run_id = run["run_id"].as_str().unwrap().to_string();
        let step_run_id = run["steps"][0]["step_run_id"].as_i64().unwrap();

        let recorded = call(
            &server,
            4,
            "laburo_record_step",
            json!({
                "run_id": run_id,
                "position": 1,
                "status": "passed",
                "observed": "form visible",
                "notes": "ok"
            }),
        );
        assert_eq!(recorded["isError"], false, "{recorded}");
        assert_eq!(result_json(&recorded)["status"], "passed");

        let evidence = dir.path().join("evidence.log");
        std::fs::write(&evidence, b"step 1 ok\n").unwrap();
        let attached = call(
            &server,
            5,
            "laburo_attach_evidence",
            json!({
                "step_run_id": step_run_id,
                "path": evidence.to_str().unwrap(),
                "caption": "log"
            }),
        );
        assert_eq!(attached["isError"], false, "{attached}");
        let attachment = result_json(&attached);
        assert_eq!(attachment["kind"], "log");
        assert!(attachment["sha256"].as_str().unwrap().len() >= 16);

        let finished = call(&server, 6, "laburo_finish_run", json!({"run_id": run_id}));
        assert_eq!(finished["isError"], false, "{finished}");
        assert_eq!(result_json(&finished)["status"], "passed");
    }

    #[test]
    fn rejects_invalid_ids_and_unknown_tools() {
        let dir = tempdir().unwrap();
        let server = test_server(dir.path());

        let bad = call(
            &server,
            1,
            "laburo_get_case",
            json!({"id": "TC 001; rm -rf /"}),
        );
        assert_eq!(bad["isError"], true);
        let error = result_json(&bad);
        assert!(
            error["error"]
                .as_str()
                .unwrap()
                .contains("unsupported characters")
        );

        let unknown = dispatch(
            &server,
            &rpc(
                2,
                "tools/call",
                json!({"name": "laburo_nope", "arguments": {}}),
            ),
        )
        .unwrap();
        assert_eq!(unknown["error"]["code"], -32602);
    }
}
