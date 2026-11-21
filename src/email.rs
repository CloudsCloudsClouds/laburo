//! Email delivery for v0.2: writes standards-compliant `.eml` files to the
//! project outbox. No SMTP transport is used in P0.

use crate::config::ProjectConfig;
use crate::repo;
use anyhow::{Context, Result, bail};
use mail_builder::MessageBuilder;
use rusqlite::Connection;
use std::path::{Path, PathBuf};

const DEFAULT_FROM_NAME: &str = "Laburo";
const DEFAULT_FROM_ADDR: &str = "laburo@localhost";
const RECENT_RECIPIENTS_LIMIT: i64 = 20;
const SLUG_MAX_LEN: usize = 60;

pub fn outbox_dir(cfg: &ProjectConfig) -> PathBuf {
    cfg.data_dir.join("outbox")
}

/// Recent distinct recipients, newest first.
pub fn recent_recipients(conn: &Connection) -> Result<Vec<String>> {
    repo::recent_email_recipients(conn, RECENT_RECIPIENTS_LIMIT)
}

/// Build and write an `.eml` with the report PDF attached, then record the
/// attempt in `email_log`.
pub fn write_eml(
    cfg: &ProjectConfig,
    conn: &Connection,
    to: &str,
    subject: &str,
    body: &str,
    report: &Path,
) -> Result<PathBuf> {
    let to = to.trim();
    if to.is_empty() {
        bail!("recipient is required");
    }
    let subject = subject.trim();
    if subject.is_empty() {
        bail!("subject is required");
    }

    let bytes = read_pdf(report)?;
    let filename = report
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("report.pdf");

    let (from_name, from_addr) = from_address();
    let eml = MessageBuilder::new()
        .from((from_name.as_str(), from_addr.as_str()))
        .to(to)
        .subject(subject)
        .text_body(body)
        .attachment("application/pdf", filename, bytes.as_slice())
        .write_to_vec()
        .context("building email message")?;

    let dir = outbox_dir(cfg);
    std::fs::create_dir_all(&dir).with_context(|| format!("creating outbox {}", dir.display()))?;

    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S-%3f").to_string();
    let slug = slugify(subject);
    let path = unique_path(&dir, &stamp, &slug);
    std::fs::write(&path, &eml).with_context(|| format!("writing {}", path.display()))?;

    let file_path = path
        .strip_prefix(&cfg.data_dir)
        .unwrap_or(&path)
        .to_string_lossy()
        .to_string();
    let report_path = report.to_string_lossy();

    repo::create_email_log(
        conn,
        to,
        subject,
        &report_path,
        "written",
        "eml",
        &file_path,
        "",
    )?;

    Ok(path)
}

fn read_pdf(report: &Path) -> Result<Vec<u8>> {
    let meta =
        std::fs::metadata(report).with_context(|| format!("reading {}", report.display()))?;
    if !meta.is_file() {
        bail!("report is not a file: {}", report.display());
    }
    let bytes = std::fs::read(report).with_context(|| format!("reading {}", report.display()))?;
    if !bytes.starts_with(b"%PDF-") {
        bail!("report is not a PDF: {}", report.display());
    }
    Ok(bytes)
}

/// Parse `LABURO_SMTP_FROM` into a display name and address. Accepts either
/// `name <addr@host>` or a bare address. Defaults to `Laburo <laburo@localhost>`.
fn from_address() -> (String, String) {
    let raw = std::env::var("LABURO_SMTP_FROM")
        .ok()
        .map(|value| value.trim().to_string())
        .filter(|value| !value.is_empty());

    let Some(raw) = raw else {
        return (DEFAULT_FROM_NAME.to_string(), DEFAULT_FROM_ADDR.to_string());
    };

    if let (Some(open), Some(close)) = (raw.find('<'), raw.rfind('>')) {
        if open < close {
            let name = raw[..open].trim().trim_matches('"').trim();
            let addr = raw[open + 1..close].trim();
            if !addr.is_empty() {
                let name = if name.is_empty() {
                    DEFAULT_FROM_NAME
                } else {
                    name
                };
                return (name.to_string(), addr.to_string());
            }
        }
    }

    (DEFAULT_FROM_NAME.to_string(), raw)
}

fn unique_path(dir: &Path, stamp: &str, slug: &str) -> PathBuf {
    let base = format!("{stamp}-{slug}");
    let mut candidate = dir.join(format!("{base}.eml"));
    let mut n = 2;
    while candidate.exists() {
        candidate = dir.join(format!("{base}-{n}.eml"));
        n += 1;
    }
    candidate
}

fn slugify(subject: &str) -> String {
    let mut slug = String::new();
    let mut pending_dash = false;
    for ch in subject.chars() {
        if ch.is_ascii_alphanumeric() {
            if pending_dash && !slug.is_empty() {
                slug.push('-');
            }
            pending_dash = false;
            slug.push(ch.to_ascii_lowercase());
            if slug.len() >= SLUG_MAX_LEN {
                break;
            }
        } else {
            pending_dash = true;
        }
    }
    let slug = slug.trim_matches('-').to_string();
    if slug.is_empty() {
        "report".to_string()
    } else {
        slug
    }
}
