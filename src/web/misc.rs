//! Dashboard, drivers, reports, quality, email, links and static assets.

use super::data::{self, CaseQuery};
use super::{AppState, WebError, clean, clean_or, db, flash_param, json_value, redirect, render};
use crate::ai::{CheckInFilter, Finding, check_in, quality_findings, suggest_next};
use crate::config::ProjectConfig;
use crate::driver;
use crate::email;
use crate::repo;
use crate::report::{self, ReportKind};
use axum::extract::{Form, Path, Query, State};
use axum::http::{StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::path::{Path as FsPath, PathBuf};

// ---------------------------------------------------------------------------
// Dashboard
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Default)]
pub struct PageParams {
    pub flash: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct Gate {
    pub status: String,
    pub score: i64,
    pub label: String,
    pub critical: i64,
    pub high: i64,
    pub medium: i64,
    pub low: i64,
}

fn gate(findings: &[Finding]) -> Gate {
    let mut critical = 0;
    let mut high = 0;
    let mut medium = 0;
    let mut low = 0;
    for finding in findings {
        match finding.severity.as_str() {
            "critical" => critical += 1,
            "high" => high += 1,
            "medium" => medium += 1,
            _ => low += 1,
        }
    }
    let (status, label) = if critical > 0 || high > 0 {
        ("fail", "Quality gate failing")
    } else if medium > 0 || low > 0 {
        ("warn", "Quality gate warning")
    } else {
        ("pass", "Quality gate passing")
    };
    let score = (100 - critical * 30 - high * 14 - medium * 5 - low * 2).clamp(0, 100);
    Gate {
        status: status.to_string(),
        score,
        label: label.to_string(),
        critical,
        high,
        medium,
        low,
    }
}

pub async fn dashboard(
    State(state): State<AppState>,
    Query(params): Query<PageParams>,
) -> Result<Html<String>, WebError> {
    let conn = db(&state)?;
    let dashboard = repo::dashboard(&conn)?;
    let runs = repo::list_runs(&conn, None, 8)?;
    let bugs = repo::list_bugs(&conn, None)?;
    let open_bugs: Vec<_> = bugs
        .iter()
        .filter(|bug| bug.status == "open" || bug.status == "reopened")
        .take(6)
        .collect();
    let findings = quality_findings(&conn)?;
    let next = suggest_next(&findings).map(json_value).transpose()?;
    let tags = data::tag_counts(&conn)?;
    let active_tags: Vec<_> = tags.iter().filter(|tag| tag.count > 0).take(14).collect();
    let cases = repo::list_cases(&conn, None, None)?;
    let latest = data::latest_runs(&conn)?;
    let flash = flash_param(params.flash.as_deref());
    render(
        &state,
        "dashboard.html",
        "dashboard",
        flash.as_deref(),
        json!({
            "stats": json_value(&dashboard)?,
            "pass_rate": format!("{:.0}", dashboard.pass_rate()),
            "runs": runs,
            "bugs": open_bugs,
            "bugs_total": bugs.len(),
            "findings": findings.iter().take(5).map(json_value).collect::<Result<Vec<_>, _>>()?,
            "finding_total": findings.len(),
            "next_action": next,
            "gate": json_value(&gate(&findings))?,
            "tags": active_tags,
            "cases": cases,
            "latest": latest,
        }),
    )
}

// ---------------------------------------------------------------------------
// Global search
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Default)]
pub struct SearchParams {
    pub q: Option<String>,
    pub flash: Option<String>,
}

pub async fn search(
    State(state): State<AppState>,
    Query(params): Query<SearchParams>,
) -> Result<Html<String>, WebError> {
    let conn = db(&state)?;
    let query = clean(params.q.as_deref()).unwrap_or_default();
    let (cases, runs, bugs) = if query.is_empty() {
        (Vec::new(), Vec::new(), Vec::new())
    } else {
        let needle = query.to_lowercase();
        let cases = data::list_cases_filtered(
            &conn,
            &CaseQuery {
                q: Some(query.clone()),
                ..CaseQuery::default()
            },
        )?;
        let runs: Vec<_> = repo::list_runs(&conn, None, 500)?
            .into_iter()
            .filter(|run| {
                run.id.to_lowercase().contains(&needle)
                    || run.case_id.to_lowercase().contains(&needle)
                    || run.environment.to_lowercase().contains(&needle)
                    || run.notes.to_lowercase().contains(&needle)
            })
            .collect();
        let bugs: Vec<_> = repo::list_bugs(&conn, None)?
            .into_iter()
            .filter(|bug| {
                bug.id.to_lowercase().contains(&needle)
                    || bug.title.to_lowercase().contains(&needle)
                    || bug.description.to_lowercase().contains(&needle)
                    || bug
                        .case_id
                        .as_deref()
                        .is_some_and(|id| id.to_lowercase().contains(&needle))
            })
            .collect();
        (cases, runs, bugs)
    };
    let case_total = cases.len();
    let run_total = runs.len();
    let bug_total = bugs.len();
    let cases: Vec<_> = cases
        .iter()
        .take(30)
        .map(json_value)
        .collect::<Result<_, _>>()?;
    let runs: Vec<_> = runs
        .iter()
        .take(30)
        .map(json_value)
        .collect::<Result<_, _>>()?;
    let bugs: Vec<_> = bugs
        .iter()
        .take(30)
        .map(json_value)
        .collect::<Result<_, _>>()?;
    let flash = flash_param(params.flash.as_deref());
    render(
        &state,
        "search.html",
        "search",
        flash.as_deref(),
        json!({
            "page_title": "Search",
            "query": query,
            "cases": cases,
            "runs": runs,
            "bugs": bugs,
            "case_total": case_total,
            "run_total": run_total,
            "bug_total": bug_total,
            "has_query": !query.is_empty(),
            "empty": case_total + run_total + bug_total == 0,
        }),
    )
}

// ---------------------------------------------------------------------------
// Quality
// ---------------------------------------------------------------------------

pub async fn quality(
    State(state): State<AppState>,
    Query(params): Query<PageParams>,
) -> Result<Html<String>, WebError> {
    let conn = db(&state)?;
    let findings = quality_findings(&conn)?;
    let gate = gate(&findings);
    let check = check_in(&conn, &CheckInFilter::default())?;
    let next = suggest_next(&findings).map(json_value).transpose()?;
    let dashboard = repo::dashboard(&conn)?;
    let cases = repo::list_cases(&conn, None, None)?;
    let mut automated = 0_i64;
    let mut manual = 0_i64;
    for case in &cases {
        if data::case_automation(&conn, &case.id)?.automated {
            automated += 1;
        } else {
            manual += 1;
        }
    }
    let flash = flash_param(params.flash.as_deref());
    render(
        &state,
        "quality.html",
        "quality",
        flash.as_deref(),
        json!({
            "findings": findings.iter().map(json_value).collect::<Result<Vec<_>, _>>()?,
            "finding_total": findings.len(),
            "gate": json_value(&gate)?,
            "check": json_value(&check)?,
            "next_action": next,
            "stats": json_value(&dashboard)?,
            "automated": automated,
            "manual": manual,
        }),
    )
}

// ---------------------------------------------------------------------------
// Drivers
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Default)]
pub struct DriverParams {
    pub q: Option<String>,
    pub flash: Option<String>,
}

pub async fn drivers(
    State(state): State<AppState>,
    Query(params): Query<DriverParams>,
) -> Result<Html<String>, WebError> {
    let conn = db(&state)?;
    let tests = repo::list_driver_tests(&conn, None)?;
    let query = clean(params.q.as_deref());
    let needle = query.as_deref().map(str::to_lowercase);
    let filtered: Vec<_> = tests
        .iter()
        .filter(|test| {
            needle.as_deref().is_none_or(|needle| {
                test.id.to_lowercase().contains(needle)
                    || test.name.to_lowercase().contains(needle)
                    || test.file.to_lowercase().contains(needle)
            })
        })
        .collect();
    let mut by_driver = std::collections::BTreeMap::<String, i64>::new();
    for test in &tests {
        *by_driver.entry(test.driver_id.clone()).or_default() += 1;
    }
    let by_driver: Vec<_> = by_driver
        .into_iter()
        .map(|(name, count)| json!({ "name": name, "count": count }))
        .collect();
    let cases = repo::list_cases(&conn, None, None)?;
    let flash = flash_param(params.flash.as_deref());
    render(
        &state,
        "drivers.html",
        "drivers",
        flash.as_deref(),
        json!({
            "tests": filtered,
            "total": tests.len(),
            "shown": filtered.len(),
            "by_driver": by_driver,
            "cases": cases.into_iter().map(|case| json!({
                "id": case.id, "title": case.title
            })).collect::<Vec<_>>(),
            "project_root": state.cfg.root.display().to_string(),
            "filters": { "q": params.q },
        }),
    )
}

pub async fn discover_drivers(State(state): State<AppState>) -> Response {
    let root = state.cfg.root.clone();
    let discovered = tokio::task::spawn_blocking(move || driver::discover_minitest(&root)).await;
    let flash = match discovered {
        Ok(Ok(tests)) => {
            let count = tests.len();
            let stored = db(&state).and_then(|conn| {
                repo::ensure_driver(&conn, "minitest", "minitest", "Minitest")?;
                repo::save_driver_tests(&conn, "minitest", &tests)?;
                Ok(())
            });
            match stored {
                Ok(()) => format!("Discovered {count} Minitest test(s)."),
                Err(error) => {
                    format!("Discovered {count} test(s) but could not save them: {error}")
                }
            }
        }
        Ok(Err(error)) => format!("Discovery failed: {error}"),
        Err(error) => format!("Discovery task failed: {error}"),
    };
    redirect("/drivers", &flash)
}

// ---------------------------------------------------------------------------
// Reports
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct ReportEntry {
    pub dir: String,
    pub kind: String,
    pub id: String,
    pub created: String,
    pub size: i64,
    pub url: String,
}

fn scan_reports(data_dir: &FsPath) -> Vec<ReportEntry> {
    let reports_dir = data_dir.join("reports");
    let Ok(entries) = std::fs::read_dir(&reports_dir) else {
        return Vec::new();
    };
    let mut reports = Vec::new();
    for entry in entries.flatten() {
        if !entry.path().is_dir() {
            continue;
        }
        let name = entry.file_name().to_string_lossy().to_string();
        let Some((kind, id)) = split_report_name(&name) else {
            continue;
        };
        let pdf = entry.path().join("report.pdf");
        let Ok(metadata) = std::fs::metadata(&pdf) else {
            continue;
        };
        let created = metadata
            .modified()
            .ok()
            .map(|time| {
                let stamp: chrono::DateTime<chrono::Local> = time.into();
                stamp.format("%Y-%m-%d %H:%M").to_string()
            })
            .unwrap_or_else(|| "—".to_string());
        reports.push(ReportEntry {
            dir: name.clone(),
            kind,
            id,
            created,
            size: i64::try_from(metadata.len()).unwrap_or(0),
            url: format!("/files/reports/{name}/report.pdf"),
        });
    }
    reports.sort_by(|left, right| right.dir.cmp(&left.dir));
    reports
}

fn split_report_name(name: &str) -> Option<(String, String)> {
    for kind in ["dictionary", "case", "run", "bug"] {
        let marker = format!("-{kind}-");
        if let Some(position) = name.find(&marker) {
            let id = &name[position + marker.len()..];
            return Some((kind.to_string(), id.to_string()));
        }
    }
    None
}

pub async fn reports(
    State(state): State<AppState>,
    Query(params): Query<PageParams>,
) -> Result<Html<String>, WebError> {
    let reports = scan_reports(&state.cfg.data_dir);
    let flash = flash_param(params.flash.as_deref());
    render(
        &state,
        "reports.html",
        "reports",
        flash.as_deref(),
        json!({
            "reports": reports,
            "reports_dir": state.cfg.data_dir.join("reports").display().to_string(),
            "kinds": ["dictionary", "case", "run", "bug"],
        }),
    )
}

pub async fn generate_report(
    State(state): State<AppState>,
    Path((kind, id)): Path<(String, String)>,
) -> Response {
    let parsed = match ReportKind::parse(&kind) {
        Ok(parsed) => parsed,
        Err(error) => return redirect("/reports", &format!("Unknown report kind: {error}")),
    };
    let case_id = if id == "all" || id == "-" {
        None
    } else {
        Some(id.as_str())
    };
    let result = {
        let Ok(conn) = db(&state) else {
            return redirect("/reports", "Database lock poisoned.");
        };
        report::generate(&conn, &state.cfg.data_dir, parsed, case_id)
    };
    match result {
        Ok(pdf) => {
            let relative = pdf
                .strip_prefix(&state.cfg.data_dir)
                .map(|path| path.to_string_lossy().to_string())
                .unwrap_or_else(|_| pdf.display().to_string());
            redirect("/reports", &format!("Report generated: /files/{relative}"))
        }
        Err(error) => {
            let message = format!("{error:#}").replace('\n', " ");
            let message: String = message.chars().take(300).collect();
            redirect("/reports", &format!("Report failed: {message}"))
        }
    }
}

// ---------------------------------------------------------------------------
// Email
// ---------------------------------------------------------------------------

#[derive(Debug, Serialize)]
pub struct OutboxEntry {
    pub name: String,
    pub created: String,
    pub size: i64,
    pub url: String,
}

fn scan_outbox(cfg: &ProjectConfig) -> Vec<OutboxEntry> {
    let outbox = email::outbox_dir(cfg);
    let Ok(entries) = std::fs::read_dir(&outbox) else {
        return Vec::new();
    };
    let mut files = Vec::new();
    for entry in entries.flatten() {
        let path = entry.path();
        let is_eml = path
            .extension()
            .and_then(|value| value.to_str())
            .is_some_and(|extension| extension.eq_ignore_ascii_case("eml"));
        if !path.is_file() || !is_eml {
            continue;
        }
        let Ok(metadata) = std::fs::metadata(&path) else {
            continue;
        };
        let name = entry.file_name().to_string_lossy().to_string();
        let created = metadata
            .modified()
            .or_else(|_| metadata.created())
            .ok()
            .map(|time| {
                let stamp: chrono::DateTime<chrono::Local> = time.into();
                stamp.format("%Y-%m-%d %H:%M").to_string()
            })
            .unwrap_or_else(|| "—".to_string());
        files.push(OutboxEntry {
            url: format!("/outbox/{}", urlencoding::encode(&name)),
            name,
            created,
            size: i64::try_from(metadata.len()).unwrap_or(0),
        });
    }
    files.sort_by(|left, right| right.name.cmp(&left.name));
    files
}

pub async fn email(
    State(state): State<AppState>,
    Query(params): Query<PageParams>,
) -> Result<Html<String>, WebError> {
    let conn = db(&state)?;
    let mut recipients = email::recent_recipients(&conn).unwrap_or_default();
    for log in data::email_logs(&conn, 25)? {
        if !log.to_addr.trim().is_empty() && !recipients.contains(&log.to_addr) {
            recipients.push(log.to_addr);
        }
    }
    let reports = scan_reports(&state.cfg.data_dir);
    let outbox = scan_outbox(&state.cfg);
    let logs = data::email_logs(&conn, 25)?;
    let smtp = if std::env::var("LABURO_SMTP_HOST").is_ok() {
        "SMTP configured (optional)"
    } else {
        "not configured (demo mode — writes .eml to the outbox)"
    };
    let flash = flash_param(params.flash.as_deref());
    render(
        &state,
        "email.html",
        "email",
        flash.as_deref(),
        json!({
            "recipients": recipients,
            "reports": reports,
            "outbox": outbox,
            "logs": logs,
            "smtp": smtp,
            "outbox_dir": email::outbox_dir(&state.cfg).display().to_string(),
        }),
    )
}

#[derive(Debug, Deserialize, Default)]
pub struct EmailForm {
    pub to: Option<String>,
    pub subject: Option<String>,
    pub body: Option<String>,
    pub report: Option<String>,
}

pub async fn send_email(
    State(state): State<AppState>,
    Form(form): Form<EmailForm>,
) -> Result<Response, WebError> {
    let to = form.to.as_deref().map(str::trim).unwrap_or("").to_string();
    let subject = form
        .subject
        .as_deref()
        .map(str::trim)
        .unwrap_or("")
        .to_string();
    if !to.contains('@') || to.starts_with('@') || to.ends_with('@') {
        return Ok(redirect("/email", "Enter a valid recipient address."));
    }
    if subject.is_empty() {
        return Ok(redirect("/email", "Subject is required."));
    }
    let Some(report_field) = clean(form.report.as_deref()) else {
        return Ok(redirect("/email", "Pick a generated report to attach."));
    };
    let data_dir = &state.cfg.data_dir;
    let candidate = if FsPath::new(&report_field).is_absolute() {
        PathBuf::from(&report_field)
    } else {
        data_dir.join(&report_field)
    };
    let report_path = match candidate.canonicalize() {
        Ok(path) => path,
        Err(_) => {
            return Ok(redirect(
                "/email",
                "That report file does not exist. Generate it first.",
            ));
        }
    };
    let reports_root = data_dir.join("reports");
    let allowed = reports_root
        .canonicalize()
        .map(|root| report_path.starts_with(root))
        .unwrap_or(false);
    if !allowed {
        return Ok(redirect(
            "/email",
            "Reports must come from .laburo/reports.",
        ));
    }
    let body = form.body.as_deref().unwrap_or("");
    let result = {
        let conn = db(&state)?;
        email::write_eml(&state.cfg, &conn, &to, &subject, body, &report_path)
    };
    match result {
        Ok(path) => {
            let name = path
                .file_name()
                .and_then(|name| name.to_str())
                .unwrap_or("message.eml");
            Ok(redirect(
                "/email",
                &format!("Created .eml in the outbox: {name}"),
            ))
        }
        Err(error) => {
            let message = format!("{error:#}").replace('\n', " ");
            let message: String = message.chars().take(300).collect();
            Ok(redirect("/email", &format!("Email failed: {message}")))
        }
    }
}

pub async fn download_outbox(
    State(state): State<AppState>,
    Path(file): Path<String>,
) -> Result<Response, WebError> {
    let name = FsPath::new(&file)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("");
    if name != file || name.is_empty() || !name.ends_with(".eml") {
        return Err(WebError::not_found("outbox file not found"));
    }
    let path = email::outbox_dir(&state.cfg).join(name);
    let bytes = tokio::fs::read(&path)
        .await
        .map_err(|_| WebError::not_found(format!("outbox file {name} not found")))?;
    let mut response = axum::body::Body::from(bytes).into_response();
    response.headers_mut().insert(
        header::CONTENT_TYPE,
        axum::http::HeaderValue::from_static("message/rfc822"),
    );
    if let Ok(disposition) =
        axum::http::HeaderValue::from_str(&format!("attachment; filename=\"{name}\""))
    {
        response
            .headers_mut()
            .insert(header::CONTENT_DISPOSITION, disposition);
    }
    Ok(response)
}

// ---------------------------------------------------------------------------
// Links, health, static assets
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize, Default)]
pub struct LinkForm {
    pub source_kind: Option<String>,
    pub source_id: Option<String>,
    pub target_kind: Option<String>,
    pub target_id: Option<String>,
    pub relation: Option<String>,
    pub note: Option<String>,
    pub return_to: Option<String>,
}

fn return_to(raw: Option<&str>) -> String {
    raw.map(str::trim)
        .filter(|value| value.starts_with('/') && !value.starts_with("//"))
        .unwrap_or("/")
        .to_string()
}

pub async fn create_link(
    State(state): State<AppState>,
    Form(form): Form<LinkForm>,
) -> Result<Response, WebError> {
    let source_kind = clean_or(form.source_kind.clone(), "case");
    let target_kind = clean_or(form.target_kind.clone(), "case");
    let relation = clean_or(form.relation.clone(), "related");
    let source_id = clean(form.source_id.as_deref()).unwrap_or_default();
    let target_id = clean(form.target_id.as_deref()).unwrap_or_default();
    let back = return_to(form.return_to.as_deref());
    if source_id.is_empty() || target_id.is_empty() {
        return Ok(redirect(
            &back,
            "A link needs both a source and a target id.",
        ));
    }
    let conn = db(&state)?;
    let stored = data::add_link(
        &conn,
        &source_kind,
        &source_id,
        &target_kind,
        &target_id,
        &relation,
        form.note.as_deref().unwrap_or(""),
    )?;
    let message = if stored {
        format!("Linked {source_kind} {source_id} → {target_kind} {target_id} ({relation}).")
    } else {
        "Entity links need migration 0003; the link was not stored.".to_string()
    };
    Ok(redirect(&back, &message))
}

pub async fn delete_link(
    State(state): State<AppState>,
    Path(id): Path<i64>,
    Form(form): Form<LinkForm>,
) -> Result<Response, WebError> {
    let conn = db(&state)?;
    data::delete_link(&conn, id)?;
    Ok(redirect(
        &return_to(form.return_to.as_deref()),
        &format!("Link {id} removed."),
    ))
}

pub async fn health() -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "text/plain; charset=utf-8")], "ok")
}

pub async fn app_css() -> Response {
    asset(
        "app.css",
        include_str!("../../static/app.css"),
        "text/css; charset=utf-8",
    )
}

pub async fn app_js() -> Response {
    asset(
        "app.js",
        include_str!("../../static/app.js"),
        "application/javascript; charset=utf-8",
    )
}

fn asset(name: &str, embedded: &'static str, content_type: &'static str) -> Response {
    let from_disk = std::env::current_dir()
        .ok()
        .map(|cwd| cwd.join("static").join(name))
        .and_then(|path| std::fs::read_to_string(path).ok());
    let body = from_disk.unwrap_or_else(|| embedded.to_string());
    (
        StatusCode::OK,
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, "no-cache"),
        ],
        body,
    )
        .into_response()
}
