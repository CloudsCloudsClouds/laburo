//! Local web UI: axum 0.8 + minijinja 3 + tower-http.
//!
//! Server-rendered, offline, no CDN and no build step. Templates live in
//! `templates/web/` and are read from disk at startup (with embedded fallbacks
//! so an installed binary still renders). The database connection is a single
//! `Mutex<Connection>`; every mutation redirects with a `?flash=` message.

mod bugs;
mod cases;
mod data;
mod misc;
mod runs;
#[cfg(test)]
mod tests;

use crate::config::ProjectConfig;
use crate::db::Db;
use anyhow::{Context, Result};
use axum::Router;
use axum::body::Body;
use axum::extract::{DefaultBodyLimit, Multipart};
use axum::http::{StatusCode, header};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use minijinja::Environment;
use rusqlite::Connection;
use serde_json::{Map, Value};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, MutexGuard};
use tower_http::services::ServeDir;

pub const MAX_UPLOAD_BYTES: usize = 64 * 1024 * 1024;

#[derive(Clone)]
pub struct AppState {
    pub cfg: Arc<ProjectConfig>,
    pub db: Arc<Mutex<Connection>>,
    pub env: Arc<Environment<'static>>,
    db_path: Option<PathBuf>,
    db_identity: Arc<Mutex<Option<DbIdentity>>>,
}

/// Cheap fingerprint of the SQLite file. Used to reopen the connection after
/// `demo-seed.sh` deletes and recreates `.laburo/` while the server is running.
#[derive(Debug, Clone, PartialEq, Eq)]
struct DbIdentity {
    len: u64,
    modified_ns: u128,
    inode: u64,
}

fn db_identity(path: &Path) -> Option<DbIdentity> {
    let metadata = std::fs::metadata(path).ok()?;
    let modified_ns = metadata
        .modified()
        .ok()
        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
        .map(|duration| duration.as_nanos())
        .unwrap_or(0);
    #[cfg(unix)]
    let inode = {
        use std::os::unix::fs::MetadataExt;
        metadata.ino()
    };
    #[cfg(not(unix))]
    let inode = 0;
    Some(DbIdentity {
        len: metadata.len(),
        modified_ns,
        inode,
    })
}

impl AppState {
    pub fn new(cfg: ProjectConfig, conn: Connection) -> Result<Self> {
        // Tests use an in-memory connection: no file is created, so the
        // identity stays None and no reopen is attempted.
        let (db_path, identity) = if cfg.db_path.is_file() {
            (Some(cfg.db_path.clone()), db_identity(&cfg.db_path))
        } else {
            (None, None)
        };
        Ok(Self {
            cfg: Arc::new(cfg),
            db: Arc::new(Mutex::new(conn)),
            env: Arc::new(load_templates()?),
            db_path,
            db_identity: Arc::new(Mutex::new(identity)),
        })
    }
}

/// Run the local web UI until the process is stopped.
pub fn run(project: &Path, host: &str, port: u16, open: bool) -> Result<()> {
    let cfg = ProjectConfig::resolve(Some(project.to_path_buf()))?;
    cfg.ensure_dirs()?;
    let db = Db::open(&cfg.db_path)?;
    let state = AppState::new(cfg.clone(), db.conn)?;

    let shown_host = if host == "0.0.0.0" { "127.0.0.1" } else { host };
    let url = format!("http://{shown_host}:{port}/");
    println!("Laburo web UI: {url}");
    println!("Project:       {}", cfg.root.display());
    println!("Data:          {}", cfg.data_dir.display());
    println!("Reseed:        the database is reopened automatically when .laburo changes");
    if open {
        open_browser(&url);
    }

    let runtime = tokio::runtime::Builder::new_multi_thread()
        .enable_all()
        .build()
        .context("building the Tokio runtime")?;
    runtime.block_on(async move {
        let listener = tokio::net::TcpListener::bind(format!("{host}:{port}"))
            .await
            .with_context(|| format!("binding {host}:{port}"))?;
        axum::serve(listener, router(state))
            .with_graceful_shutdown(async {
                let _ = tokio::signal::ctrl_c().await;
            })
            .await
            .context("running the web server")?;
        Ok::<(), anyhow::Error>(())
    })?;
    Ok(())
}

fn open_browser(url: &str) {
    let candidates: &[&str] = if cfg!(target_os = "macos") {
        &["open"]
    } else {
        &["xdg-open", "gio", "sensible-browser"]
    };
    for command in candidates {
        let mut child = std::process::Command::new(command);
        if *command == "gio" {
            child.args(["open", url]);
        } else {
            child.arg(url);
        }
        child
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null());
        if child.spawn().is_ok() {
            return;
        }
    }
}

pub fn router(state: AppState) -> Router {
    let static_dir = assets_dir();
    let files = ServeDir::new(state.cfg.data_dir.clone());
    Router::new()
        .route("/", get(misc::dashboard))
        .route("/search", get(misc::search))
        .route("/cases", get(cases::list).post(cases::create))
        .route("/cases/new", get(cases::new_form))
        .route("/cases/export", get(cases::export_csv))
        .route("/cases/run-filtered", post(cases::run_filtered))
        .route("/cases/bulk", post(cases::bulk))
        .route("/cases/{id}", get(cases::detail))
        .route("/cases/{id}/edit", get(cases::edit_form))
        .route("/cases/{id}", post(cases::update))
        .route("/cases/{id}/delete", post(cases::delete))
        .route("/cases/{id}/run", post(cases::start_run))
        .route("/cases/{id}/automate", post(cases::automate))
        .route("/cases/{id}/steps", post(cases::add_step))
        .route("/cases/{id}/steps/{pos}", post(cases::update_step))
        .route("/cases/{id}/steps/{pos}/delete", post(cases::delete_step))
        .route("/cases/{id}/steps/{pos}/move", post(cases::move_step))
        .route("/cases/{id}/evidence", post(cases::upload_evidence))
        .route("/runs", get(runs::list))
        .route("/runs/{id}", get(runs::detail))
        .route("/runs/{id}/steps/{pos}", post(runs::record_step))
        .route(
            "/runs/{id}/steps/{pos}/evidence",
            post(runs::upload_step_evidence),
        )
        .route("/runs/{id}/finish", post(runs::finish))
        .route("/runs/{id}/evidence", post(runs::upload_evidence))
        .route("/bugs", get(bugs::list))
        .route("/bugs/new", get(bugs::new_form))
        .route("/bugs", post(bugs::create))
        .route("/bugs/{id}", get(bugs::detail))
        .route("/bugs/{id}/status", post(bugs::set_status))
        .route("/bugs/{id}/evidence", post(bugs::upload_evidence))
        .route("/links", post(misc::create_link))
        .route("/links/{id}/delete", post(misc::delete_link))
        .route("/drivers", get(misc::drivers))
        .route("/drivers/discover", post(misc::discover_drivers))
        .route("/reports", get(misc::reports))
        .route("/reports/{kind}/{id}", post(misc::generate_report))
        .route("/quality", get(misc::quality))
        .route("/email", get(misc::email).post(misc::send_email))
        .route("/outbox/{file}", get(misc::download_outbox))
        .route("/static/app.css", get(misc::app_css))
        .route("/static/app.js", get(misc::app_js))
        .route("/health", get(misc::health))
        .nest_service("/files", files)
        .nest_service("/static/assets", ServeDir::new(static_dir.clone()))
        .layer(DefaultBodyLimit::max(MAX_UPLOAD_BYTES))
        .with_state(state)
}

// ---------------------------------------------------------------------------
// Templates
// ---------------------------------------------------------------------------

const TEMPLATES: &[(&str, &str)] = &[
    ("base.html", include_str!("../../templates/web/base.html")),
    (
        "dashboard.html",
        include_str!("../../templates/web/dashboard.html"),
    ),
    (
        "search.html",
        include_str!("../../templates/web/search.html"),
    ),
    ("cases.html", include_str!("../../templates/web/cases.html")),
    ("batch.html", include_str!("../../templates/web/batch.html")),
    (
        "case_form.html",
        include_str!("../../templates/web/case_form.html"),
    ),
    (
        "case_detail.html",
        include_str!("../../templates/web/case_detail.html"),
    ),
    ("runs.html", include_str!("../../templates/web/runs.html")),
    (
        "run_detail.html",
        include_str!("../../templates/web/run_detail.html"),
    ),
    ("bugs.html", include_str!("../../templates/web/bugs.html")),
    (
        "bug_form.html",
        include_str!("../../templates/web/bug_form.html"),
    ),
    (
        "bug_detail.html",
        include_str!("../../templates/web/bug_detail.html"),
    ),
    (
        "drivers.html",
        include_str!("../../templates/web/drivers.html"),
    ),
    (
        "reports.html",
        include_str!("../../templates/web/reports.html"),
    ),
    (
        "quality.html",
        include_str!("../../templates/web/quality.html"),
    ),
    ("email.html", include_str!("../../templates/web/email.html")),
    ("error.html", include_str!("../../templates/web/error.html")),
];

fn load_templates() -> Result<Environment<'static>> {
    let mut env = Environment::new();
    let dir = templates_dir();
    for (name, embedded) in TEMPLATES {
        let from_disk = dir
            .as_ref()
            .map(|base| base.join(name))
            .and_then(|path| std::fs::read_to_string(path).ok())
            .filter(|source| !source.trim().is_empty());
        let source = from_disk.unwrap_or_else(|| (*embedded).to_string());
        env.add_template_owned(*name, source)
            .with_context(|| format!("loading web template {name}"))?;
    }
    env.add_filter("urlencode", |value: String| {
        urlencoding::encode(&value).into_owned()
    });
    Ok(env)
}

fn templates_dir() -> Option<PathBuf> {
    let mut candidates = vec![Path::new(env!("CARGO_MANIFEST_DIR")).join("templates/web")];
    if let Ok(cwd) = std::env::current_dir() {
        candidates.push(cwd.join("templates/web"));
    }
    candidates.into_iter().find(|path| path.is_dir())
}

fn assets_dir() -> PathBuf {
    let from_source = Path::new(env!("CARGO_MANIFEST_DIR")).join("static");
    if from_source.is_dir() {
        return from_source;
    }
    std::env::current_dir()
        .map(|cwd| cwd.join("static"))
        .unwrap_or(from_source)
}

// ---------------------------------------------------------------------------
// Rendering, errors, redirects
// ---------------------------------------------------------------------------

pub struct WebError {
    status: StatusCode,
    message: String,
}

impl WebError {
    pub fn internal(error: impl std::fmt::Display) -> Self {
        Self {
            status: StatusCode::INTERNAL_SERVER_ERROR,
            message: error.to_string(),
        }
    }

    pub fn not_found(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::NOT_FOUND,
            message: message.into(),
        }
    }

    pub fn bad_request(message: impl Into<String>) -> Self {
        Self {
            status: StatusCode::BAD_REQUEST,
            message: message.into(),
        }
    }
}

impl From<anyhow::Error> for WebError {
    fn from(error: anyhow::Error) -> Self {
        Self::internal(format!("{error:#}"))
    }
}

impl From<minijinja::Error> for WebError {
    fn from(error: minijinja::Error) -> Self {
        Self::internal(error)
    }
}

impl From<rusqlite::Error> for WebError {
    fn from(error: rusqlite::Error) -> Self {
        Self::internal(error)
    }
}

impl From<std::io::Error> for WebError {
    fn from(error: std::io::Error) -> Self {
        Self::internal(error)
    }
}

impl From<serde_json::Error> for WebError {
    fn from(error: serde_json::Error) -> Self {
        Self::internal(error)
    }
}

impl std::fmt::Display for WebError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str(&self.message)
    }
}

impl IntoResponse for WebError {
    fn into_response(self) -> Response {
        let title = match self.status {
            StatusCode::NOT_FOUND => "Not found",
            StatusCode::BAD_REQUEST => "That request needs attention",
            _ => "Something went wrong",
        };
        let body = format!(
            "<!doctype html><html lang=\"en\"><head><meta charset=\"utf-8\">\
             <meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
             <title>{title} · Laburo</title><link rel=\"stylesheet\" href=\"/static/app.css\">\
             </head><body><main class=\"page narrow\"><section class=\"card error-card\">\
             <h1>{title}</h1><p class=\"error-detail\">{message}</p>\
             <p><a class=\"btn\" href=\"/\">Back to dashboard</a></p></section></main></body></html>",
            message = escape_html(&self.message)
        );
        (self.status, Html(body)).into_response()
    }
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

pub(crate) fn render(
    state: &AppState,
    template: &str,
    active: &str,
    flash: Option<&str>,
    extra: Value,
) -> Result<Html<String>, WebError> {
    let mut context = Map::new();
    context.insert("active".to_string(), Value::String(active.to_string()));
    context.insert(
        "page_title".to_string(),
        Value::String(default_title(active).to_string()),
    );
    context.insert(
        "project".to_string(),
        Value::String(data::project_label(&state.cfg.root)),
    );
    context.insert(
        "version".to_string(),
        Value::String(env!("CARGO_PKG_VERSION").to_string()),
    );
    context.insert(
        "flash".to_string(),
        flash.map_or(Value::Null, |value| Value::String(value.to_string())),
    );
    context.insert(
        "data_dir".to_string(),
        Value::String(state.cfg.data_dir.display().to_string()),
    );
    if let Value::Object(map) = extra {
        for (key, value) in map {
            context.insert(key, value);
        }
    }
    let html = state
        .env
        .get_template(template)?
        .render(minijinja::value::Serde(Value::Object(context)))?;
    Ok(Html(html))
}

fn default_title(active: &str) -> &'static str {
    match active {
        "dashboard" => "Dashboard",
        "cases" => "Cases",
        "runs" => "Runs",
        "bugs" => "Bugs",
        "drivers" => "Drivers",
        "reports" => "Reports",
        "quality" => "Quality",
        "email" => "Email",
        _ => "Laburo",
    }
}

pub(crate) fn redirect(to: &str, flash: &str) -> Response {
    let (path, fragment) = match to.split_once('#') {
        Some((path, fragment)) => (path, Some(fragment)),
        None => (to, None),
    };
    let separator = if path.contains('?') { '&' } else { '?' };
    let location = match fragment {
        Some(fragment) => format!(
            "{path}{separator}flash={}#{fragment}",
            urlencoding::encode(flash)
        ),
        None => format!("{path}{separator}flash={}", urlencoding::encode(flash)),
    };
    (
        StatusCode::SEE_OTHER,
        [(header::LOCATION, location)],
        Body::empty(),
    )
        .into_response()
}

pub(crate) fn db(state: &AppState) -> Result<MutexGuard<'_, Connection>, WebError> {
    let mut connection = state
        .db
        .lock()
        .map_err(|_| WebError::internal("database lock poisoned"))?;
    if let Some(path) = state.db_path.as_deref() {
        let current = db_identity(path);
        let mut known = state
            .db_identity
            .lock()
            .map_err(|_| WebError::internal("database identity lock poisoned"))?;
        if current != *known {
            let fresh = Db::open(path).map_err(WebError::internal)?;
            *connection = fresh.conn;
            *known = current;
        }
    }
    Ok(connection)
}

pub(crate) fn clean(value: Option<&str>) -> Option<String> {
    value
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_string)
}

pub(crate) fn clean_or(value: Option<String>, fallback: &str) -> String {
    clean(value.as_deref()).unwrap_or_else(|| fallback.to_string())
}

pub(crate) fn flash_param(flash: Option<&str>) -> Option<String> {
    clean(flash)
}

// ---------------------------------------------------------------------------
// Multipart uploads
// ---------------------------------------------------------------------------

pub struct Upload {
    pub filename: String,
    pub bytes: Vec<u8>,
    pub caption: String,
    pub role: String,
    pub fields: std::collections::HashMap<String, String>,
}

pub(crate) async fn collect_multipart(mut multipart: Multipart) -> Result<Upload, WebError> {
    let mut filename = String::new();
    let mut bytes = Vec::new();
    let mut caption = String::new();
    let mut role = String::new();
    let mut fields = std::collections::HashMap::new();
    while let Some(field) = multipart
        .next_field()
        .await
        .map_err(|error| WebError::bad_request(format!("reading upload: {error}")))?
    {
        let name = field.name().unwrap_or_default().to_string();
        if name == "file" {
            filename = field.file_name().unwrap_or("upload.bin").to_string();
            bytes = field
                .bytes()
                .await
                .map_err(|error| WebError::bad_request(format!("reading file: {error}")))?
                .to_vec();
        } else {
            let value = field
                .text()
                .await
                .map_err(|error| WebError::bad_request(format!("reading field: {error}")))?;
            match name.as_str() {
                "caption" => caption = value.trim().to_string(),
                "role" => role = value.trim().to_string(),
                _ => {
                    fields.insert(name, value);
                }
            }
        }
    }
    if bytes.is_empty() {
        return Err(WebError::bad_request("Choose a file to upload."));
    }
    Ok(Upload {
        filename,
        bytes,
        caption,
        role,
        fields,
    })
}

/// Write the upload to `.laburo/tmp`, run the closure (usually the attachment
/// insert), and remove the temporary file again.
pub(crate) fn with_upload<T>(
    state: &AppState,
    upload: &Upload,
    f: impl FnOnce(&Path) -> anyhow::Result<T>,
) -> Result<T, WebError> {
    let tmp_dir = state.cfg.data_dir.join("tmp");
    std::fs::create_dir_all(&tmp_dir)?;
    let stamp = chrono::Local::now().format("%Y%m%d%H%M%S%f").to_string();
    let name = format!("{stamp}-{}", safe_upload_name(&upload.filename));
    let path = tmp_dir.join(name);
    std::fs::write(&path, &upload.bytes)?;
    let result = f(&path);
    let _ = std::fs::remove_file(&path);
    result.map_err(WebError::from)
}

fn safe_upload_name(name: &str) -> String {
    let base = Path::new(name)
        .file_name()
        .and_then(|value| value.to_str())
        .unwrap_or("upload.bin");
    let cleaned: String = base
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect();
    if cleaned.is_empty() {
        "upload.bin".to_string()
    } else {
        cleaned
    }
}

pub(crate) fn json_value<T: serde::Serialize>(value: &T) -> Result<Value, WebError> {
    Ok(serde_json::to_value(value)?)
}
