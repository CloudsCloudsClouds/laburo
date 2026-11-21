//! Report projection and Typst invocation.
//!
//! SQLite is the source of truth. This module projects one report kind into a
//! transient `data.json`, copies the active Typst template next to it, and runs
//! the installed `typst` binary. No Typst crate and no second store.

use crate::model::{self, Attachment, BugReport, TestCase, TestRun};
use crate::repo::{self, EvidenceOwner};
use anyhow::{Context, Result, bail};
use rusqlite::OptionalExtension;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::process::Command;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReportKind {
    Dictionary,
    Case,
    Run,
    Bug,
}

impl ReportKind {
    pub fn parse(s: &str) -> Result<Self> {
        match s {
            "dictionary" => Ok(Self::Dictionary),
            "case" => Ok(Self::Case),
            "run" => Ok(Self::Run),
            "bug" => Ok(Self::Bug),
            other => bail!("unknown report kind: {other}"),
        }
    }

    const fn slug(self) -> &'static str {
        match self {
            Self::Dictionary => "dictionary",
            Self::Case => "case",
            Self::Run => "run",
            Self::Bug => "bug",
        }
    }
}

const LIB_TYP: &str = include_str!("../../templates/typst/lib.typ");
const DICTIONARY_TYP: &str = include_str!("../../templates/typst/dictionary.typ");
const CASE_TYP: &str = include_str!("../../templates/typst/case.typ");
const RUN_TYP: &str = include_str!("../../templates/typst/run.typ");
const BUG_TYP: &str = include_str!("../../templates/typst/bug.typ");

#[derive(Debug, Serialize)]
struct Projection {
    meta: Meta,
    case: Option<CaseProjection>,
    #[serde(skip_serializing_if = "Option::is_none")]
    cases: Option<Vec<CaseProjection>>,
    runs: Vec<RunProjection>,
    bugs: Vec<BugProjection>,
}

#[derive(Debug, Serialize)]
struct Meta {
    generated_at: String,
    project: String,
    version: String,
}

#[derive(Debug, Serialize)]
struct CaseProjection {
    id: String,
    title: String,
    description: String,
    module: String,
    priority: String,
    status: String,
    last_run_status: Option<String>,
    environment: String,
    gherkin: String,
    driver_target: Option<String>,
    default_mode: String,
    tags: Vec<String>,
    steps: Vec<StepProjection>,
    requirements: Vec<RequirementProjection>,
    expected_evidence: Vec<EvidenceProjection>,
    evidence: Vec<EvidenceProjection>,
    links: Vec<LinkProjection>,
}

#[derive(Debug, Serialize)]
struct StepProjection {
    position: i64,
    action: String,
    data: String,
    expected: String,
    kind: String,
    driver_target: Option<String>,
}

#[derive(Debug, Serialize)]
struct RequirementProjection {
    id: String,
    title: String,
}

#[derive(Debug, Serialize)]
struct RunProjection {
    id: String,
    mode: String,
    status: String,
    started_at: String,
    ended_at: Option<String>,
    environment: String,
    commit: String,
    command: String,
    notes: String,
    steps: Vec<StepRunProjection>,
    evidence: Vec<EvidenceProjection>,
    links: Vec<LinkProjection>,
}

#[derive(Debug, Serialize)]
struct StepRunProjection {
    position: i64,
    status: String,
    observed: String,
    notes: String,
    evidence: Vec<EvidenceProjection>,
}

#[derive(Debug, Serialize)]
struct BugProjection {
    id: String,
    title: String,
    description: String,
    steps: String,
    actual: String,
    expected: String,
    priority: i64,
    severity: i64,
    environment: String,
    status: String,
    comments: String,
    case_id: Option<String>,
    run_id: Option<String>,
    evidence: Vec<EvidenceProjection>,
    links: Vec<LinkProjection>,
}

#[derive(Debug, Clone, Serialize)]
struct EvidenceProjection {
    kind: String,
    path: String,
    caption: String,
    role: String,
    case_id: Option<String>,
    exists: bool,
}

#[derive(Debug, Serialize)]
struct LinkProjection {
    id: i64,
    source_kind: String,
    source_id: String,
    target_kind: String,
    target_id: String,
    relation: String,
    note: String,
    /// Resolved target title when the target is a known entity.
    title: String,
}

impl CaseProjection {
    fn from_case(conn: &rusqlite::Connection, data_dir: &Path, case: &TestCase) -> Result<Self> {
        let mut stmt = conn.prepare(
            "SELECT r.id, r.title
             FROM requirement r
             JOIN case_requirement cr ON cr.requirement_id = r.id
             WHERE cr.case_id = ?1
             ORDER BY r.id",
        )?;
        let rows = stmt.query_map([&case.id], |row| {
            Ok(RequirementProjection {
                id: row.get(0)?,
                title: row.get(1)?,
            })
        })?;
        let requirements = rows.collect::<rusqlite::Result<Vec<_>>>()?;

        let case_evidence = repo::case_evidence(conn, &case.id)?;
        let evidence: Vec<EvidenceProjection> = case_evidence
            .iter()
            .map(|item| EvidenceProjection::from_attachment(data_dir, item))
            .collect();
        let expected_evidence = evidence
            .iter()
            .filter(|item| item.role == "expected")
            .cloned()
            .collect();
        let last_run_status = conn
            .query_row(
                "SELECT status FROM test_run WHERE case_id = ?1
                 ORDER BY started_at DESC, id DESC LIMIT 1",
                [&case.id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;

        Ok(Self {
            id: case.id.clone(),
            title: case.title.clone(),
            description: case.description.clone(),
            module: case.module.clone(),
            priority: case.priority.clone(),
            status: case.status.clone(),
            last_run_status,
            environment: case.environment.clone(),
            gherkin: case.gherkin.clone(),
            driver_target: case.driver_target.clone(),
            default_mode: case.default_mode.as_str().to_string(),
            tags: case.tags.clone(),
            steps: case
                .steps
                .iter()
                .map(|step| StepProjection {
                    position: step.position,
                    action: step.action.clone(),
                    data: step.data.clone(),
                    expected: step.expected.clone(),
                    kind: step.kind.as_str().to_string(),
                    driver_target: step.driver_target.clone(),
                })
                .collect(),
            requirements,
            expected_evidence,
            evidence,
            links: case_links(conn, &case.id)?,
        })
    }
}

fn case_links(conn: &rusqlite::Connection, case_id: &str) -> Result<Vec<LinkProjection>> {
    Ok(repo::list_links(conn, "case", case_id)?
        .iter()
        .map(|link| LinkProjection::from_link(conn, link))
        .collect())
}

impl RunProjection {
    fn from_run(conn: &rusqlite::Connection, data_dir: &Path, run: &TestRun) -> Result<Self> {
        let evidence = repo::load_evidence(conn, EvidenceOwner::Run(run.id.clone()))?
            .iter()
            .map(|item| EvidenceProjection::from_attachment(data_dir, item))
            .collect();

        Ok(Self {
            id: run.id.clone(),
            mode: run.mode.as_str().to_string(),
            status: run.status.as_str().to_string(),
            started_at: run.started_at.clone(),
            ended_at: run.ended_at.clone(),
            environment: run.environment.clone(),
            commit: run.commit_ref.clone(),
            command: run.command.clone(),
            notes: run.notes.clone(),
            steps: run
                .steps
                .iter()
                .map(|step| StepRunProjection {
                    position: step.position,
                    status: step.status.as_str().to_string(),
                    observed: step.observed.clone(),
                    notes: step.notes.clone(),
                    evidence: step
                        .evidence
                        .iter()
                        .map(|item| EvidenceProjection::from_attachment(data_dir, item))
                        .collect(),
                })
                .collect(),
            evidence,
            links: repo::list_links(conn, "run", &run.id)?
                .iter()
                .map(|link| LinkProjection::from_link(conn, link))
                .collect(),
        })
    }
}

impl BugProjection {
    fn from_bug(conn: &rusqlite::Connection, data_dir: &Path, bug: &BugReport) -> Result<Self> {
        Ok(Self {
            id: bug.id.clone(),
            title: bug.title.clone(),
            description: bug.description.clone(),
            steps: bug.steps.clone(),
            actual: bug.actual.clone(),
            expected: bug.expected.clone(),
            priority: bug.priority,
            severity: bug.severity,
            environment: bug.environment.clone(),
            status: bug.status.clone(),
            comments: bug.comments.clone(),
            case_id: bug.case_id.clone(),
            run_id: bug.run_id.clone(),
            evidence: bug
                .evidence
                .iter()
                .map(|item| EvidenceProjection::from_attachment(data_dir, item))
                .collect(),
            links: repo::list_links(conn, "bug", &bug.id)?
                .iter()
                .map(|link| LinkProjection::from_link(conn, link))
                .collect(),
        })
    }
}

impl LinkProjection {
    fn from_link(conn: &rusqlite::Connection, link: &crate::model::EntityLink) -> Self {
        Self {
            id: link.id,
            source_kind: link.source_kind.clone(),
            source_id: link.source_id.clone(),
            target_kind: link.target_kind.clone(),
            target_id: link.target_id.clone(),
            relation: link.relation.clone(),
            note: link.note.clone(),
            title: link_target_title(conn, &link.target_kind, &link.target_id),
        }
    }
}

fn link_target_title(conn: &rusqlite::Connection, kind: &str, id: &str) -> String {
    match kind {
        "case" => repo::get_case(conn, id)
            .ok()
            .flatten()
            .map(|case| case.title)
            .unwrap_or_default(),
        "bug" => repo::get_bug(conn, id)
            .ok()
            .flatten()
            .map(|bug| bug.title)
            .unwrap_or_default(),
        "run" => repo::get_run(conn, id)
            .ok()
            .flatten()
            .map(|run| run.case_id)
            .unwrap_or_default(),
        "requirement" => conn
            .query_row("SELECT title FROM requirement WHERE id=?1", [id], |row| {
                row.get(0)
            })
            .unwrap_or_default(),
        _ => String::new(),
    }
}

impl EvidenceProjection {
    fn from_attachment(data_dir: &Path, item: &Attachment) -> Self {
        let path = PathBuf::from(&item.path);
        let absolute = if path.is_absolute() {
            path
        } else {
            data_dir.join(path)
        };
        let exists = absolute.is_file()
            && std::fs::File::open(&absolute).is_ok()
            && embeddable(&absolute, &item.kind);
        Self {
            kind: item.kind.clone(),
            path: absolute.to_string_lossy().into_owned(),
            caption: item.caption.clone(),
            role: item.role.clone(),
            case_id: item.case_id.clone(),
            exists,
        }
    }
}

fn embeddable(path: &Path, kind: &str) -> bool {
    if kind != "screenshot" {
        return true;
    }
    matches!(
        path.extension()
            .and_then(|extension| extension.to_str())
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("png" | "jpg" | "jpeg" | "gif" | "webp" | "svg")
    )
}

fn project_name(conn: &rusqlite::Connection, data_dir: &Path) -> String {
    for key in ["project_name", "project", "name"] {
        let found: Option<String> = conn
            .query_row(
                "SELECT value FROM schema_meta WHERE key = ?1",
                [key],
                |row| row.get(0),
            )
            .ok();
        if let Some(value) = found {
            if !value.trim().is_empty() {
                return value;
            }
        }
    }
    data_dir
        .parent()
        .and_then(Path::file_name)
        .and_then(|name| name.to_str())
        .filter(|name| !name.is_empty())
        .unwrap_or("Laburo")
        .to_string()
}

fn build_projection(
    conn: &rusqlite::Connection,
    data_dir: &Path,
    kind: ReportKind,
    id: Option<&str>,
) -> Result<Projection> {
    let meta = Meta {
        generated_at: model::now(),
        project: project_name(conn, data_dir),
        version: env!("CARGO_PKG_VERSION").to_string(),
    };

    match kind {
        ReportKind::Dictionary => {
            let cases = repo::list_cases(conn, None, None)?
                .iter()
                .map(|case| CaseProjection::from_case(conn, data_dir, case))
                .collect::<Result<Vec<_>>>()?;
            Ok(Projection {
                meta,
                case: None,
                cases: Some(cases),
                runs: Vec::new(),
                bugs: Vec::new(),
            })
        }
        ReportKind::Case => {
            let id = id.context("report case requires a case id (e.g. TC-001)")?;
            let case =
                repo::get_case(conn, id)?.with_context(|| format!("case not found: {id}"))?;
            let case_projection = CaseProjection::from_case(conn, data_dir, &case)?;
            let runs = repo::list_runs(conn, Some(id), 1000)?
                .iter()
                .map(|run| RunProjection::from_run(conn, data_dir, run))
                .collect::<Result<Vec<_>>>()?;
            let bugs = repo::list_bugs(conn, None)?
                .iter()
                .filter(|bug| bug.case_id.as_deref() == Some(id))
                .map(|bug| BugProjection::from_bug(conn, data_dir, bug))
                .collect::<Result<Vec<_>>>()?;
            Ok(Projection {
                meta,
                case: Some(case_projection),
                cases: None,
                runs,
                bugs,
            })
        }
        ReportKind::Run => {
            let id = id.context("report run requires a run id (e.g. RUN-20261117-180000-1)")?;
            let run = repo::get_run(conn, id)?.with_context(|| format!("run not found: {id}"))?;
            let case = match repo::get_case(conn, &run.case_id)? {
                Some(case) => Some(CaseProjection::from_case(conn, data_dir, &case)?),
                None => None,
            };
            let run_projection = RunProjection::from_run(conn, data_dir, &run)?;
            let bugs = repo::list_bugs(conn, None)?
                .iter()
                .filter(|bug| bug.run_id.as_deref() == Some(id))
                .map(|bug| BugProjection::from_bug(conn, data_dir, bug))
                .collect::<Result<Vec<_>>>()?;
            Ok(Projection {
                meta,
                case,
                cases: None,
                runs: vec![run_projection],
                bugs,
            })
        }
        ReportKind::Bug => {
            let id = id.context("report bug requires a bug id (e.g. BUG-20261117-1)")?;
            let bug = repo::get_bug(conn, id)?.with_context(|| format!("bug not found: {id}"))?;
            let case = match bug.case_id.as_deref() {
                Some(case_id) => match repo::get_case(conn, case_id)? {
                    Some(case) => Some(CaseProjection::from_case(conn, data_dir, &case)?),
                    None => None,
                },
                None => None,
            };
            let runs = match bug.run_id.as_deref() {
                Some(run_id) => match repo::get_run(conn, run_id)? {
                    Some(run) => vec![RunProjection::from_run(conn, data_dir, &run)?],
                    None => Vec::new(),
                },
                None => Vec::new(),
            };
            Ok(Projection {
                meta,
                case,
                cases: None,
                runs,
                bugs: vec![BugProjection::from_bug(conn, data_dir, &bug)?],
            })
        }
    }
}

fn sanitize_component(value: &str) -> String {
    let mut cleaned: String = value
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '-' | '_' | '.') {
                character
            } else {
                '_'
            }
        })
        .collect();
    if cleaned.is_empty() {
        cleaned.push_str("all");
    }
    if cleaned == "." || cleaned == ".." {
        cleaned = "_".to_string();
    }
    cleaned.truncate(64);
    cleaned
}

fn unique_report_dir(reports_dir: &Path, name: &str) -> Result<PathBuf> {
    std::fs::create_dir_all(reports_dir)
        .with_context(|| format!("creating {}", reports_dir.display()))?;
    let mut candidate = reports_dir.join(name);
    let mut suffix = 2;
    while candidate.exists() {
        candidate = reports_dir.join(format!("{name}-{suffix}"));
        suffix += 1;
        if suffix > 1000 {
            bail!(
                "could not allocate a report directory under {}",
                reports_dir.display()
            );
        }
    }
    Ok(candidate)
}

fn embedded_template(name: &str) -> Option<&'static str> {
    match name {
        "lib.typ" => Some(LIB_TYP),
        "dictionary.typ" => Some(DICTIONARY_TYP),
        "case.typ" => Some(CASE_TYP),
        "run.typ" => Some(RUN_TYP),
        "bug.typ" => Some(BUG_TYP),
        _ => None,
    }
}

/// Prefer the template in the source tree (so a developer can edit it without
/// rebuilding); fall back to the copy embedded in the binary for installed use.
fn template_source(name: &str) -> Result<String> {
    let embedded =
        embedded_template(name).with_context(|| format!("unknown report template: {name}"))?;
    let source = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("templates")
        .join("typst")
        .join(name);
    match std::fs::read_to_string(&source) {
        Ok(text) if !text.trim().is_empty() => Ok(text),
        _ => Ok(embedded.to_string()),
    }
}

fn write_template(report_dir: &Path, name: &str) -> Result<()> {
    let text = template_source(name)?;
    let destination = report_dir.join(name);
    std::fs::write(&destination, text)
        .with_context(|| format!("writing report template {}", destination.display()))
}

fn compile_pdf(report_dir: &Path, kind: ReportKind) -> Result<PathBuf> {
    let typst = std::env::var("LABURO_TYPST").unwrap_or_else(|_| "typst".to_string());
    let source = format!("{}.typ", kind.slug());
    let output = Command::new(&typst)
        .current_dir(report_dir)
        .arg("compile")
        // Evidence paths are absolute in data.json, so Typst needs `/` as root.
        .arg("--root")
        .arg("/")
        .arg(&source)
        .arg("report.pdf")
        .output()
        .with_context(|| {
            format!(
                "running `{typst} compile` in {}; install Typst 0.15+ or set LABURO_TYPST to the binary path",
                report_dir.display()
            )
        })?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        let stdout = String::from_utf8_lossy(&output.stdout);
        let detail = if stderr.trim().is_empty() {
            stdout.trim()
        } else {
            stderr.trim()
        };
        bail!(
            "typst failed to compile {source} in {} ({}).\n{detail}\nFix the data/template or set LABURO_TYPST to a working Typst 0.15+ binary.",
            report_dir.display(),
            output.status
        );
    }

    let pdf = report_dir.join("report.pdf");
    if !pdf.is_file() {
        bail!(
            "typst reported success but {} was not created",
            pdf.display()
        );
    }
    Ok(pdf)
}

/// Generate one report and return the PDF path.
///
/// Layout: `<data_dir>/reports/<timestamp>-<kind>-<id-or-all>/` with
/// `data.json`, `<kind>.typ`, `lib.typ`, and `report.pdf`.
pub fn generate(
    conn: &rusqlite::Connection,
    data_dir: &Path,
    kind: ReportKind,
    id: Option<&str>,
) -> Result<PathBuf> {
    let data_dir = if data_dir.is_absolute() {
        data_dir.to_path_buf()
    } else {
        std::env::current_dir()
            .context("resolving the current directory")?
            .join(data_dir)
    };

    let projection = build_projection(conn, &data_dir, kind, id)?;

    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S").to_string();
    let id_part = id
        .map(sanitize_component)
        .filter(|value| !value.is_empty())
        .unwrap_or_else(|| "all".to_string());
    let name = format!("{stamp}-{}-{id_part}", kind.slug());
    let report_dir = unique_report_dir(&data_dir.join("reports"), &name)?;
    std::fs::create_dir_all(&report_dir)
        .with_context(|| format!("creating report directory {}", report_dir.display()))?;

    let json = serde_json::to_string_pretty(&projection)?;
    let json_path = report_dir.join("data.json");
    std::fs::write(&json_path, json)
        .with_context(|| format!("writing report projection {}", json_path.display()))?;

    write_template(&report_dir, "lib.typ")?;
    write_template(&report_dir, &format!("{}.typ", kind.slug()))?;

    compile_pdf(&report_dir, kind)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::db::Db;
    use crate::model::{RunMode, RunStatus, StepKind};
    use tempfile::tempdir;

    fn seed(db: &Db, data_dir: &Path) -> (String, String) {
        repo::create_case(
            &db.conn,
            "TC-001",
            "Book a tour",
            "Verify that a visitor can book a tour.",
            "Bookings",
            "high",
            "local",
            "Feature: Booking",
            &["functional".to_string(), "smoke".to_string()],
        )
        .unwrap();
        repo::add_step(
            &db.conn,
            "TC-001",
            1,
            "Open the tour page",
            "",
            "The page is visible",
            StepKind::Manual,
            None,
        )
        .unwrap();
        repo::add_step(
            &db.conn,
            "TC-001",
            2,
            "Submit the form",
            "seat=12A",
            "A confirmation is shown",
            StepKind::Manual,
            None,
        )
        .unwrap();
        repo::create_requirement(&db.conn, "RF-01", "Reserve a tour", "", "", "functional")
            .unwrap();
        repo::link_case_requirement(&db.conn, "TC-001", "RF-01").unwrap();

        std::fs::create_dir_all(data_dir).unwrap();
        let expected = data_dir.join("expected.log");
        std::fs::write(&expected, "expected page\n").unwrap();
        repo::attach_file_with_role(
            &db.conn,
            data_dir,
            repo::EvidenceOwner::Case("TC-001".to_string()),
            &expected,
            "expected happy path",
            "expected",
        )
        .unwrap();

        let run = repo::create_run(&db.conn, "TC-001", RunMode::Manual, "local").unwrap();
        let step_evidence = data_dir.join("step.log");
        std::fs::write(&step_evidence, "step output\n").unwrap();
        repo::attach_file_with_role(
            &db.conn,
            data_dir,
            repo::EvidenceOwner::StepRun(run.steps[0].id),
            &step_evidence,
            "step actual",
            "actual",
        )
        .unwrap();
        let run_artifact = data_dir.join("artifact.log");
        std::fs::write(&run_artifact, "run artifact\n").unwrap();
        repo::attach_file_with_role(
            &db.conn,
            data_dir,
            repo::EvidenceOwner::Run(run.id.clone()),
            &run_artifact,
            "run artifact",
            "artifact",
        )
        .unwrap();
        repo::record_step(
            &db.conn,
            &run.id,
            1,
            RunStatus::Passed,
            "loaded in 20 ms",
            "",
            Some(20),
            None,
            "",
        )
        .unwrap();
        repo::record_step(
            &db.conn,
            &run.id,
            2,
            RunStatus::Failed,
            "HTTP 500",
            "reproduced twice",
            None,
            None,
            "",
        )
        .unwrap();
        repo::finish_run(&db.conn, &run.id).unwrap();

        let bug = repo::create_bug(
            &db.conn,
            None,
            "Booking returns 500",
            "Seat 12A breaks the form.",
            "1. Open the case\n2. Submit seat 12A",
            "HTTP 500",
            "HTTP 200",
            1,
            2,
            "local",
            Some("TC-001"),
            Some(&run.id),
            None,
            "",
        )
        .unwrap();

        let source = data_dir.join("run.log");
        std::fs::create_dir_all(data_dir).unwrap();
        std::fs::write(&source, "line one\nline two\n").unwrap();
        repo::attach_file(
            &db.conn,
            data_dir,
            repo::EvidenceOwner::Bug(bug.id.clone()),
            &source,
            "server log",
        )
        .unwrap();
        repo::create_link(
            &db.conn,
            "bug",
            &bug.id,
            "case",
            "TC-001",
            "reproduces",
            "reproduced by the case",
        )
        .unwrap();
        repo::create_link(
            &db.conn,
            "run",
            &run.id,
            "case",
            "TC-001",
            "evidence-for",
            "execution evidence",
        )
        .unwrap();

        (run.id, bug.id)
    }

    #[test]
    fn generates_all_four_reports() {
        let db = Db::open_memory().unwrap();
        let workspace = tempdir().unwrap();
        let data_dir = workspace.path().join(".laburo");
        let (run_id, bug_id) = seed(&db, &data_dir);

        let reports = [
            (ReportKind::Dictionary, None, "dictionary.typ"),
            (ReportKind::Case, Some("TC-001"), "case.typ"),
            (ReportKind::Run, Some(run_id.as_str()), "run.typ"),
            (ReportKind::Bug, Some(bug_id.as_str()), "bug.typ"),
        ];

        for (kind, id, template) in reports {
            let pdf = generate(&db.conn, &data_dir, kind, id).unwrap();
            assert!(pdf.is_file(), "missing {}", pdf.display());
            assert_eq!(pdf.file_name().unwrap(), "report.pdf");

            let report_dir = pdf.parent().unwrap();
            assert!(report_dir.starts_with(data_dir.join("reports")));
            assert!(report_dir.join("data.json").is_file());
            assert!(report_dir.join("lib.typ").is_file());
            assert!(report_dir.join(template).is_file());

            let json: serde_json::Value = serde_json::from_str(
                &std::fs::read_to_string(report_dir.join("data.json")).unwrap(),
            )
            .unwrap();
            for key in ["meta", "case", "runs", "bugs"] {
                assert!(json.get(key).is_some(), "data.json lost key {key}");
            }
        }
    }

    fn projection_json(
        db: &Db,
        data_dir: &Path,
        kind: ReportKind,
        id: Option<&str>,
    ) -> serde_json::Value {
        let pdf = generate(&db.conn, data_dir, kind, id).unwrap();
        let report_dir = pdf.parent().unwrap();
        serde_json::from_str(&std::fs::read_to_string(report_dir.join("data.json")).unwrap())
            .unwrap()
    }

    fn assert_evidence(item: &serde_json::Value, role: &str) {
        assert_eq!(item["role"], role);
        assert_eq!(item["exists"], true, "evidence must exist: {item}");
        let path = item["path"].as_str().unwrap();
        assert!(
            path.starts_with('/'),
            "evidence path is not absolute: {path}"
        );
        assert!(Path::new(path).is_file(), "missing evidence file: {path}");
    }

    #[test]
    fn projection_includes_roles_exists_and_links() {
        let db = Db::open_memory().unwrap();
        let workspace = tempdir().unwrap();
        let data_dir = workspace.path().join(".laburo");
        let (run_id, bug_id) = seed(&db, &data_dir);

        let case = projection_json(&db, &data_dir, ReportKind::Case, Some("TC-001"));
        let case = &case["case"];
        assert_evidence(&case["expected_evidence"][0], "expected");
        assert_evidence(&case["evidence"][0], "expected");
        let links = case["links"].as_array().unwrap();
        assert_eq!(links.len(), 2, "{links:?}");
        assert!(links.iter().any(|link| link["relation"] == "reproduces"
            && link["target_kind"] == "case"
            && link["target_id"] == "TC-001"
            && link["note"] == "reproduced by the case"));
        assert!(
            links
                .iter()
                .any(|link| link["relation"] == "evidence-for" && link["target_kind"] == "case")
        );

        let run = projection_json(&db, &data_dir, ReportKind::Run, Some(&run_id));
        let run = &run["runs"][0];
        assert_eq!(run["links"][0]["relation"], "evidence-for");
        assert_evidence(&run["evidence"][0], "artifact");
        assert_evidence(&run["steps"][0]["evidence"][0], "actual");

        let bug = projection_json(&db, &data_dir, ReportKind::Bug, Some(&bug_id));
        let bug = &bug["bugs"][0];
        assert_eq!(bug["case_id"], "TC-001");
        assert_eq!(bug["run_id"].as_str(), Some(run_id.as_str()));
        assert_eq!(bug["links"][0]["relation"], "reproduces");
        assert_eq!(bug["links"][0]["title"], "Book a tour");
        assert_evidence(&bug["evidence"][0], "evidence");
        assert_eq!(
            projection_json(&db, &data_dir, ReportKind::Bug, Some(&bug_id))["case"]["expected_evidence"]
                [0]["role"],
            "expected"
        );
    }

    #[test]
    fn missing_evidence_reports_exists_false_and_absolute_path() {
        let db = Db::open_memory().unwrap();
        let workspace = tempdir().unwrap();
        let data_dir = workspace.path().join(".laburo");
        std::fs::create_dir_all(&data_dir).unwrap();
        repo::create_case(&db.conn, "TC-001", "Case", "", "", "low", "local", "", &[]).unwrap();
        let missing = data_dir.join("missing.log");
        std::fs::write(&missing, "gone\n").unwrap();
        let attachment = repo::attach_file_with_role(
            &db.conn,
            &data_dir,
            repo::EvidenceOwner::Case("TC-001".to_string()),
            &missing,
            "missing later",
            "expected",
        )
        .unwrap();
        std::fs::remove_file(data_dir.join(&attachment.path)).unwrap();

        let case = projection_json(&db, &data_dir, ReportKind::Case, Some("TC-001"));
        let item = &case["case"]["expected_evidence"][0];
        assert_eq!(item["exists"], false);
        assert!(item["path"].as_str().unwrap().starts_with('/'));
    }

    #[test]
    fn requires_an_id_for_single_record_reports() {
        let db = Db::open_memory().unwrap();
        let workspace = tempdir().unwrap();
        let data_dir = workspace.path().join(".laburo");

        let error = generate(&db.conn, &data_dir, ReportKind::Case, None).unwrap_err();
        assert!(error.to_string().contains("requires a case id"));
    }
}
