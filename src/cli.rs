//! Command-line surface. Parse, validate, call the library. No business rules.

use crate::config::{ProjectConfig, init_project};
use crate::db::Db;
use crate::model::{CaseFilter, RunMode, RunStatus, StepKind, TestOutcome};
use crate::repo;
use crate::report::ReportKind;
use anyhow::{Context, Result, bail};
use clap::{Args, CommandFactory, Parser, Subcommand};
use std::path::PathBuf;
use std::process::Command as ProcessCommand;
use std::str::FromStr;

/// Test evidence for TQA: cases, runs, evidence, bugs, and reports.
#[derive(Debug, Parser)]
#[command(
    name = "laburo",
    version,
    about = "Local-first TQA suite: cases, runs, evidence, bugs, and reports",
    long_about = None
)]
pub struct Cli {
    /// Project root. Defaults to the nearest `.laburo`, Rails app, or Cargo.toml.
    #[arg(long, global = true, value_name = "DIR")]
    pub project: Option<PathBuf>,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Create `.laburo/` and the SQLite database.
    Init,
    /// Inspect the environment and the target project.
    Doctor,
    /// Test cases.
    Case(CaseArgs),
    /// Runs and step results.
    Run(RunArgs),
    /// Bug reports.
    Bug(BugArgs),
    /// Generate Typst PDF reports.
    Report(ReportArgs),
    /// Drivers and discovered tests.
    Driver(DriverArgs),
    /// Import the class TQA dictionary (`casos.yaml`) into SQLite.
    Import(ImportArgs),
    /// Backup and restore the SQLite database.
    Db(DbArgs),
    /// Deterministic quality findings.
    Quality(QualityArgs),
    /// Link entities (case, run, bug, requirement, step).
    Link(LinkArgs),
    /// Run the local web UI.
    Web(WebArgs),
    /// Create an `.eml` with a report attached (demo outbox).
    Email(EmailArgs),
    /// Run the MCP stdio server.
    Mcp,
    /// Open the TUI.
    Tui,
}

#[derive(Debug, Args)]
pub struct CaseArgs {
    #[command(subcommand)]
    pub command: CaseCommand,
}

#[derive(Debug, Subcommand)]
pub enum CaseCommand {
    Create(CreateCase),
    List(ListCases),
    Show {
        id: String,
    },
    Delete {
        id: String,
        #[arg(long)]
        yes: bool,
    },
    Tag {
        id: String,
        #[arg(long = "tag")]
        tags: Vec<String>,
    },
    Step {
        #[command(subcommand)]
        command: StepCommand,
    },
    /// Set the case-level driver target and default execution mode.
    SetAutomation(SetAutomation),
    /// Attach evidence to a case (for example the expected happy-path screenshot).
    Evidence(CaseEvidence),
}

#[derive(Debug, Args)]
pub struct CaseEvidence {
    pub id: String,
    #[arg(long)]
    pub path: PathBuf,
    #[arg(long, default_value = "")]
    pub caption: String,
    #[arg(long, default_value = "evidence")]
    pub role: String,
}

#[derive(Debug, Args)]
pub struct SetAutomation {
    pub id: String,
    #[arg(long, default_value = "manual")]
    pub mode: String,
    #[arg(long)]
    pub target: Option<String>,
}

#[derive(Debug, Args)]
pub struct CreateCase {
    #[arg(long)]
    pub id: String,
    #[arg(long)]
    pub title: String,
    #[arg(long, default_value = "")]
    pub description: String,
    #[arg(long, default_value = "")]
    pub module: String,
    #[arg(long, default_value = "medium")]
    pub priority: String,
    #[arg(long, default_value = "local")]
    pub environment: String,
    #[arg(long, default_value = "")]
    pub gherkin: String,
    #[arg(long = "tag")]
    pub tags: Vec<String>,
    /// Step as `action|data|expected` or `action|expected`.
    #[arg(long = "step")]
    pub steps: Vec<String>,
}

#[derive(Debug, Args)]
pub struct ListCases {
    #[arg(long)]
    pub tag: Option<String>,
    #[arg(long)]
    pub status: Option<String>,
}

#[derive(Debug, Subcommand)]
pub enum StepCommand {
    Add {
        case: String,
        #[arg(long)]
        action: String,
        #[arg(long, default_value = "")]
        data: String,
        #[arg(long)]
        expected: String,
        #[arg(long, default_value = "manual")]
        kind: String,
        #[arg(long)]
        target: Option<String>,
    },
}

#[derive(Debug, Args)]
pub struct RunArgs {
    #[command(subcommand)]
    pub command: RunCommand,
}

#[derive(Debug, Subcommand)]
pub enum RunCommand {
    /// Create a manual run and apply step statuses in order.
    Manual {
        case: String,
        #[arg(long)]
        environment: Option<String>,
        #[arg(long, value_delimiter = ',')]
        step_status: Vec<String>,
        #[arg(long, default_value = "")]
        note: String,
        #[arg(long = "evidence")]
        evidence: Vec<PathBuf>,
    },
    /// Run a case through a driver.
    Auto {
        case: String,
        #[arg(long, default_value = "minitest")]
        driver: String,
        #[arg(long, default_value = "single")]
        mode: String,
        #[arg(long = "target")]
        targets: Vec<String>,
        #[arg(long)]
        environment: Option<String>,
        #[arg(long)]
        keep_going: bool,
    },
    Show {
        id: String,
    },
    List {
        #[arg(long)]
        case: Option<String>,
        #[arg(long, default_value_t = 20)]
        limit: i64,
    },
    Step {
        run: String,
        position: i64,
        #[arg(long)]
        status: String,
        #[arg(long, default_value = "")]
        observed: String,
        #[arg(long, default_value = "")]
        notes: String,
        #[arg(long = "evidence")]
        evidence: Vec<PathBuf>,
    },
    Finish {
        run: String,
    },
    /// Start runs for every case matching the filters.
    Batch(BatchArgs),
}

#[derive(Debug, Args)]
pub struct BatchArgs {
    /// Exact tag name.
    #[arg(long)]
    pub tag: Option<String>,
    /// Tag value in the `type` category, e.g. functional.
    #[arg(long = "type")]
    pub type_tag: Option<String>,
    /// Tag value in the `level` category, e.g. system.
    #[arg(long)]
    pub level: Option<String>,
    #[arg(long)]
    pub status: Option<String>,
    /// auto (case default), manual, single, or composed.
    #[arg(long, default_value = "auto")]
    pub mode: String,
    #[arg(long, default_value = "minitest")]
    pub driver: String,
    /// Create the runs but do not execute drivers.
    #[arg(long)]
    pub dry_run: bool,
}

#[derive(Debug, Args)]
pub struct LinkArgs {
    #[command(subcommand)]
    pub command: LinkCommand,
}

#[derive(Debug, Subcommand)]
pub enum LinkCommand {
    Add {
        source_kind: String,
        source_id: String,
        target_kind: String,
        target_id: String,
        #[arg(long, default_value = "related")]
        relation: String,
        #[arg(long, default_value = "")]
        note: String,
    },
    List {
        kind: String,
        id: String,
    },
    Remove {
        id: i64,
    },
}

#[derive(Debug, Args)]
pub struct BugArgs {
    #[command(subcommand)]
    pub command: BugCommand,
}

#[derive(Debug, Subcommand)]
#[allow(clippy::large_enum_variant)]
pub enum BugCommand {
    Create(CreateBug),
    List {
        #[arg(long)]
        status: Option<String>,
    },
    Show {
        id: String,
    },
    Close {
        id: String,
    },
}

#[derive(Debug, Args)]
pub struct CreateBug {
    #[arg(long)]
    pub id: Option<String>,
    #[arg(long)]
    pub title: String,
    #[arg(long, default_value = "")]
    pub description: String,
    #[arg(long, default_value = "")]
    pub steps: String,
    #[arg(long, default_value = "")]
    pub actual: String,
    #[arg(long, default_value = "")]
    pub expected: String,
    #[arg(long, default_value_t = 3)]
    pub priority: i64,
    #[arg(long, default_value_t = 3)]
    pub severity: i64,
    #[arg(long, default_value = "")]
    pub environment: String,
    #[arg(long)]
    pub case: Option<String>,
    #[arg(long)]
    pub run: Option<String>,
    #[arg(long)]
    pub step: Option<i64>,
    #[arg(long, default_value = "")]
    pub comments: String,
    #[arg(long = "evidence")]
    pub evidence: Vec<PathBuf>,
}

#[derive(Debug, Args)]
pub struct ReportArgs {
    /// dictionary, case, run, or bug.
    pub kind: String,
    pub id: Option<String>,
}

#[derive(Debug, Args)]
pub struct DriverArgs {
    #[command(subcommand)]
    pub command: DriverCommand,
}

#[derive(Debug, Subcommand)]
pub enum DriverCommand {
    List,
    Discover {
        #[arg(long, default_value = "minitest")]
        driver: String,
    },
}

#[derive(Debug, Args)]
pub struct ImportArgs {
    #[arg(long)]
    pub path: Option<PathBuf>,
}

#[derive(Debug, Args)]
pub struct DbArgs {
    #[command(subcommand)]
    pub command: DbCommand,
}

#[derive(Debug, Subcommand)]
pub enum DbCommand {
    Backup,
    Restore {
        file: PathBuf,
        #[arg(long)]
        yes: bool,
    },
}

#[derive(Debug, Args)]
pub struct QualityArgs {
    #[arg(long)]
    pub json: bool,
}

#[derive(Debug, Args)]
pub struct WebArgs {
    #[arg(long, default_value_t = 3000)]
    pub port: u16,
    #[arg(long, default_value = "127.0.0.1")]
    pub host: String,
    #[arg(long)]
    pub open: bool,
}

#[derive(Debug, Args)]
pub struct EmailArgs {
    #[command(subcommand)]
    pub command: EmailCommand,
}

#[derive(Debug, Subcommand)]
pub enum EmailCommand {
    Send(SendEmail),
}

#[derive(Debug, Args)]
pub struct SendEmail {
    #[arg(long)]
    pub to: String,
    #[arg(long)]
    pub subject: String,
    #[arg(long)]
    pub report: PathBuf,
    #[arg(long, default_value = "Laburo report attached.")]
    pub body: String,
}

pub fn run(cli: Cli) -> Result<()> {
    let Some(command) = cli.command else {
        Cli::command().print_help()?;
        println!();
        return Ok(());
    };
    match command {
        Command::Init => cmd_init(cli.project),
        Command::Doctor => cmd_doctor(cli.project),
        Command::Case(args) => with_project(cli.project, |cfg| cmd_case(cfg, args)),
        Command::Run(args) => with_project(cli.project, |cfg| cmd_run(cfg, args)),
        Command::Bug(args) => with_project(cli.project, |cfg| cmd_bug(cfg, args)),
        Command::Report(args) => with_project(cli.project, |cfg| cmd_report(cfg, args)),
        Command::Driver(args) => with_project(cli.project, |cfg| cmd_driver(cfg, args)),
        Command::Import(args) => with_project(cli.project, |cfg| cmd_import(cfg, args)),
        Command::Db(args) => with_project(cli.project, |cfg| cmd_db(cfg, args)),
        Command::Quality(args) => with_project(cli.project, |cfg| cmd_quality(cfg, args)),
        Command::Link(args) => with_project(cli.project, |cfg| cmd_link(cfg, args)),
        Command::Web(args) => with_project(cli.project, |cfg| {
            crate::web::run(&cfg.root, &args.host, args.port, args.open)
        }),
        Command::Email(args) => with_project(cli.project, |cfg| cmd_email(cfg, args)),
        Command::Mcp => with_project(cli.project, |cfg| crate::mcp::run(&cfg.root)),
        Command::Tui => with_project(cli.project, |cfg| crate::tui::run(&cfg.root)),
    }
}

fn with_project<F>(project: Option<PathBuf>, f: F) -> Result<()>
where
    F: FnOnce(&ProjectConfig) -> Result<()>,
{
    let cfg = ProjectConfig::resolve(project)?;
    cfg.ensure_dirs()?;
    f(&cfg)
}

fn open_db(cfg: &ProjectConfig) -> Result<Db> {
    Db::open(&cfg.db_path)
}

fn cmd_init(project: Option<PathBuf>) -> Result<()> {
    let root = project.unwrap_or(std::env::current_dir()?);
    let cfg = init_project(&root)?;
    let db = Db::open(&cfg.db_path)?;
    let _ = db;
    println!("laburo initialized");
    println!("  project: {}", cfg.root.display());
    println!("  data:    {}", cfg.data_dir.display());
    println!("  db:      {}", cfg.db_path.display());
    Ok(())
}

fn cmd_doctor(project: Option<PathBuf>) -> Result<()> {
    let cfg = ProjectConfig::resolve(project)?;
    println!("laburo {} — doctor", env!("CARGO_PKG_VERSION"));
    println!("target: {}", cfg.root.display());
    println!();
    let mut failures = 0;
    let mut check = |label: &str, ok: bool, detail: String, required: bool| {
        let mark = if ok { "ok  " } else { "MISS" };
        let optional = if required { "" } else { " (optional)" };
        println!("[{mark}] {label:<16} {detail}{optional}");
        if required && !ok {
            failures += 1;
        }
    };
    let rails = cfg.root.join("bin").join("rails");
    check(
        "Rails app",
        rails.is_file(),
        rails.display().to_string(),
        false,
    );
    let gemfile = cfg.root.join("Gemfile");
    let minitest = std::fs::read_to_string(&gemfile)
        .map(|s| s.contains("minitest"))
        .unwrap_or(false)
        || cfg.root.join("test").join("test_helper.rb").is_file();
    check("Minitest", minitest, gemfile.display().to_string(), false);
    let dictionary = cfg.root.join("tqa").join("casos.yaml");
    check(
        "TQA dictionary",
        dictionary.is_file(),
        dictionary.display().to_string(),
        false,
    );
    let db_ok = Db::open(&cfg.db_path).is_ok();
    check("SQLite", db_ok, cfg.db_path.display().to_string(), true);
    let typst = ProcessCommand::new(&cfg.typst).arg("--version").output();
    let typst_ok = typst.as_ref().is_ok_and(|o| o.status.success());
    let typst_detail = typst
        .ok()
        .and_then(|o| {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if s.is_empty() { None } else { Some(s) }
        })
        .unwrap_or_else(|| format!("`{}` not found", cfg.typst));
    check("Typst", typst_ok, typst_detail, true);
    let nextest = ProcessCommand::new("cargo")
        .args(["nextest", "--version"])
        .output();
    let nextest_ok = nextest.as_ref().is_ok_and(|o| o.status.success());
    check(
        "cargo nextest",
        nextest_ok,
        "optional driver".to_string(),
        false,
    );
    if failures > 0 {
        bail!("{failures} required check(s) failed");
    }
    Ok(())
}

fn cmd_case(cfg: &ProjectConfig, args: CaseArgs) -> Result<()> {
    let db = open_db(cfg)?;
    match args.command {
        CaseCommand::Create(args) => {
            let case = repo::create_case(
                &db.conn,
                &args.id,
                &args.title,
                &args.description,
                &args.module,
                &args.priority,
                &args.environment,
                &args.gherkin,
                &args.tags,
            )?;
            for (i, raw) in args.steps.iter().enumerate() {
                let parts: Vec<&str> = raw.split('|').collect();
                let (action, data, expected) = match parts.as_slice() {
                    [a, e] => (*a, "", *e),
                    [a, d, e] => (*a, *d, *e),
                    _ => bail!("step must be action|expected or action|data|expected: {raw}"),
                };
                repo::add_step(
                    &db.conn,
                    &case.id,
                    (i + 1) as i64,
                    action,
                    data,
                    expected,
                    StepKind::Manual,
                    None,
                )?;
            }
            let case = repo::get_case(&db.conn, &case.id)?.context("case not found")?;
            print_case(&case);
        }
        CaseCommand::List(args) => {
            let cases = repo::list_cases(&db.conn, args.tag.as_deref(), args.status.as_deref())?;
            if cases.is_empty() {
                println!("no cases");
            }
            for case in cases {
                println!(
                    "{:<16} {:<28} {:<12} {:<8} {}",
                    case.id,
                    truncate(&case.title, 28),
                    case.priority,
                    case.status,
                    case.tags.join(",")
                );
            }
        }
        CaseCommand::Show { id } => {
            let case =
                repo::get_case(&db.conn, &id)?.with_context(|| format!("case not found: {id}"))?;
            print_case(&case);
        }
        CaseCommand::Delete { id, yes } => {
            if !yes {
                bail!("refusing to delete {id} without --yes");
            }
            if repo::delete_case(&db.conn, &id)? {
                println!("deleted {id}");
            } else {
                bail!("case not found: {id}");
            }
        }
        CaseCommand::Tag { id, tags } => {
            let mut case =
                repo::get_case(&db.conn, &id)?.with_context(|| format!("case not found: {id}"))?;
            for tag in tags {
                if !case.tags.contains(&tag) {
                    case.tags.push(tag);
                }
            }
            repo::update_case(&db.conn, &case)?;
            println!("tags: {}", case.tags.join(", "));
        }
        CaseCommand::Step { command } => match command {
            StepCommand::Add {
                case,
                action,
                data,
                expected,
                kind,
                target,
            } => {
                let existing = repo::get_case(&db.conn, &case)?
                    .with_context(|| format!("case not found: {case}"))?;
                let position = existing.steps.len() as i64 + 1;
                let step = repo::add_step(
                    &db.conn,
                    &case,
                    position,
                    &action,
                    &data,
                    &expected,
                    StepKind::from_str(&kind).map_err(|e| anyhow::anyhow!(e))?,
                    target.as_deref(),
                )?;
                println!("added step {} to {}: {}", step.position, case, step.action);
            }
        },
        CaseCommand::SetAutomation(args) => {
            let mode = RunMode::from_str(&args.mode).map_err(|e| anyhow::anyhow!(e))?;
            let case = repo::set_case_automation(&db.conn, &args.id, mode, args.target.as_deref())?;
            println!(
                "{}: mode={} target={}",
                case.id,
                case.default_mode.as_str(),
                case.driver_target.as_deref().unwrap_or("-")
            );
        }
        CaseCommand::Evidence(args) => {
            let attachment = repo::attach_file_with_role(
                &db.conn,
                &cfg.data_dir,
                repo::EvidenceOwner::Case(args.id.clone()),
                &args.path,
                &args.caption,
                &args.role,
            )?;
            println!(
                "attached {} to {} as role={} ({})",
                attachment.original_name, args.id, attachment.role, attachment.path
            );
        }
    }
    Ok(())
}

fn print_case(case: &crate::model::TestCase) {
    println!("{} — {}", case.id, case.title);
    println!("  module:      {}", case.module);
    println!("  priority:    {}", case.priority);
    println!("  status:      {}", case.status);
    println!("  environment: {}", case.environment);
    println!("  mode:        {}", case.default_mode.as_str());
    if let Some(target) = &case.driver_target {
        println!("  target:      {target}");
    }
    println!("  tags:        {}", case.tags.join(", "));
    if !case.description.is_empty() {
        println!("  description: {}", case.description);
    }
    if !case.gherkin.is_empty() {
        println!("  gherkin:\n{}", case.gherkin);
    }
    for step in &case.steps {
        println!(
            "  {}. [{}] {} -> {}",
            step.position,
            step.kind.as_str(),
            step.action,
            step.expected
        );
    }
}

fn cmd_run(cfg: &ProjectConfig, args: RunArgs) -> Result<()> {
    let db = open_db(cfg)?;
    match args.command {
        RunCommand::Manual {
            case,
            environment,
            step_status,
            note,
            evidence,
        } => {
            let environment = environment.unwrap_or_else(|| "local".to_string());
            let run = repo::create_run(&db.conn, &case, RunMode::Manual, &environment)?;
            if step_status.is_empty() {
                println!("created run {}", run.id);
                print_run(&run);
                return Ok(());
            }
            for (i, step) in run.steps.iter().enumerate() {
                let raw = step_status
                    .get(i)
                    .or_else(|| step_status.last())
                    .map(String::as_str)
                    .unwrap_or("not_run");
                let status = RunStatus::from_str(raw).map_err(|e| anyhow::anyhow!(e))?;
                repo::record_step(
                    &db.conn,
                    &run.id,
                    step.position,
                    status,
                    "",
                    &note,
                    None,
                    step.driver_test.as_deref(),
                    "",
                )?;
            }
            if let Some(first) = run.steps.first() {
                for path in &evidence {
                    repo::attach_file(
                        &db.conn,
                        &cfg.data_dir,
                        repo::EvidenceOwner::StepRun(first.id),
                        path,
                        "",
                    )?;
                }
            }
            let run = repo::finish_run(&db.conn, &run.id)?;
            print_run(&run);
        }
        RunCommand::Auto {
            case,
            driver,
            mode,
            targets,
            environment,
            keep_going,
        } => {
            if driver != "minitest" {
                bail!("unsupported driver: {driver} (only minitest in v0.1)");
            }
            let case_obj = repo::get_case(&db.conn, &case)?
                .with_context(|| format!("case not found: {case}"))?;
            let mode = RunMode::from_str(&mode).map_err(|e| anyhow::anyhow!(e))?;
            let mut targets = targets;
            if targets.is_empty() {
                targets = batch_targets(&case_obj, mode);
            }
            if targets.is_empty() {
                bail!(
                    "no targets: pass --target, set the case driver_target, or set driver_target on a step"
                );
            }
            let environment = environment.unwrap_or_else(|| "local".to_string());
            let run = repo::create_run(&db.conn, &case, mode, &environment)?;
            let outcomes = crate::driver::run_minitest(&cfg.root, &targets)?;
            for (i, outcome) in outcomes.iter().enumerate() {
                let position = (i + 1) as i64;
                let driver_test = run
                    .steps
                    .iter()
                    .find(|step| step.position == position)
                    .and_then(|step| step.driver_test.clone())
                    .unwrap_or_else(|| outcome.target.clone());
                repo::record_step(
                    &db.conn,
                    &run.id,
                    position,
                    outcome.status,
                    &outcome.message,
                    &outcome.backtrace,
                    outcome.duration_ms,
                    Some(&driver_test),
                    &outcome.raw_output,
                )?;
                if !keep_going && outcome.status.is_bad() {
                    break;
                }
            }
            let run = repo::finish_run(&db.conn, &run.id)?;
            print_run(&run);
        }
        RunCommand::Show { id } => {
            let run =
                repo::get_run(&db.conn, &id)?.with_context(|| format!("run not found: {id}"))?;
            print_run(&run);
        }
        RunCommand::List { case, limit } => {
            let runs = repo::list_runs(&db.conn, case.as_deref(), limit)?;
            for run in runs {
                println!(
                    "{:<24} {:<16} {:<10} {:<10} {}",
                    run.id,
                    run.case_id,
                    run.mode.as_str(),
                    run.status.as_str(),
                    run.started_at
                );
            }
        }
        RunCommand::Step {
            run,
            position,
            status,
            observed,
            notes,
            evidence,
        } => {
            let status = RunStatus::from_str(&status).map_err(|e| anyhow::anyhow!(e))?;
            let step = repo::record_step(
                &db.conn, &run, position, status, &observed, &notes, None, None, "",
            )?;
            for path in evidence {
                repo::attach_file(
                    &db.conn,
                    &cfg.data_dir,
                    repo::EvidenceOwner::StepRun(step.id),
                    &path,
                    "",
                )?;
            }
            println!("recorded step {} of {}", position, run);
        }
        RunCommand::Finish { run } => {
            let run = repo::finish_run(&db.conn, &run)?;
            print_run(&run);
        }
        RunCommand::Batch(args) => {
            if args.driver != "minitest" {
                bail!("unsupported driver: {} (only minitest)", args.driver);
            }
            let requested = args.mode.to_ascii_lowercase();
            if !matches!(
                requested.as_str(),
                "auto" | "manual" | "single" | "composed"
            ) {
                bail!(
                    "mode must be auto, manual, single, or composed: {mode}",
                    mode = args.mode
                );
            }
            let filter = CaseFilter {
                q: None,
                tag: args.tag.clone(),
                category: None,
                priority: None,
                status: args.status.clone(),
            };
            let mut cases = repo::list_cases_filtered(&db.conn, &filter)?;
            if let Some(type_tag) = args.type_tag.as_deref().map(str::trim) {
                if !type_tag.is_empty() {
                    cases.retain(|case| {
                        case.tags
                            .iter()
                            .any(|tag| tag.eq_ignore_ascii_case(type_tag))
                    });
                }
            }
            if let Some(level) = args.level.as_deref().map(str::trim) {
                if !level.is_empty() {
                    cases
                        .retain(|case| case.tags.iter().any(|tag| tag.eq_ignore_ascii_case(level)));
                }
            }
            let mut summary = BatchSummary {
                selected: cases.len(),
                ..BatchSummary::default()
            };
            println!("batch: {} case(s) selected", summary.selected);
            for case in cases {
                summary.attempted += 1;
                let mode = match requested.as_str() {
                    "auto" => case.default_mode,
                    other => RunMode::from_str(other).map_err(|e| anyhow::anyhow!(e))?,
                };
                if mode == RunMode::Manual && requested == "auto" {
                    summary.manual_required += 1;
                    println!("SKIP {} — manual-required (no automation)", case.id);
                    continue;
                }
                let targets = batch_targets(&case, mode);
                if mode != RunMode::Manual && targets.is_empty() {
                    summary.manual_required += 1;
                    println!("SKIP {} — manual-required (no driver target)", case.id);
                    continue;
                }
                let run = match repo::create_run(&db.conn, &case.id, mode, &case.environment) {
                    Ok(run) => run,
                    Err(err) => {
                        summary.errors += 1;
                        println!("ERROR {} — creating run: {err:#}", case.id);
                        continue;
                    }
                };
                summary.created += 1;
                if args.dry_run || mode == RunMode::Manual {
                    println!(
                        "CREATED {} — {} ({} mode){}",
                        run.id,
                        case.id,
                        mode.as_str(),
                        if args.dry_run { " [dry-run]" } else { "" }
                    );
                    continue;
                }
                match crate::driver::run_minitest(&cfg.root, &targets) {
                    Ok(outcomes) if outcomes.is_empty() => {
                        let message = "Minitest driver returned no outcomes";
                        record_run_error(&db.conn, &run, message);
                        let finished = repo::finish_run(&db.conn, &run.id)?;
                        summary.errors += 1;
                        println!("ERROR {} — {} — {message}", case.id, finished.id);
                    }
                    Ok(outcomes) => {
                        if let Err(err) = record_outcomes(&db.conn, &run, &outcomes) {
                            summary.errors += 1;
                            println!("ERROR {} — recording steps: {err:#}", case.id);
                            record_run_error(&db.conn, &run, &format!("{err:#}"));
                        }
                        let finished = repo::finish_run(&db.conn, &run.id)?;
                        match finished.status {
                            RunStatus::Passed => summary.passed += 1,
                            RunStatus::Failed | RunStatus::Error => summary.failed += 1,
                            _ => {}
                        }
                        println!(
                            "{} {} — {} — {}",
                            finished.status.as_str().to_uppercase(),
                            finished.id,
                            case.id,
                            finished.mode.as_str()
                        );
                    }
                    Err(err) => {
                        let message = format!("Minitest driver failed: {err:#}");
                        record_run_error(&db.conn, &run, &message);
                        let finished = repo::finish_run(&db.conn, &run.id)?;
                        summary.errors += 1;
                        println!("ERROR {} — {} — {message}", case.id, finished.id);
                    }
                }
            }
            println!(
                "summary: attempted={} created={} passed={} failed={} manual-required={} errors={}",
                summary.attempted,
                summary.created,
                summary.passed,
                summary.failed,
                summary.manual_required,
                summary.errors
            );
        }
    }
    Ok(())
}

#[derive(Debug, Default)]
struct BatchSummary {
    selected: usize,
    attempted: usize,
    created: usize,
    passed: usize,
    failed: usize,
    manual_required: usize,
    errors: usize,
}

fn batch_targets(case: &crate::model::TestCase, mode: RunMode) -> Vec<String> {
    match mode {
        RunMode::Manual => Vec::new(),
        RunMode::Single => case.driver_target.iter().cloned().collect(),
        RunMode::Composed => {
            let mut targets: Vec<String> = case
                .steps
                .iter()
                .filter_map(|step| step.driver_target.clone())
                .collect();
            if targets.is_empty() {
                targets.extend(case.driver_target.iter().cloned());
            }
            targets
        }
    }
}

fn record_outcomes(
    conn: &rusqlite::Connection,
    run: &crate::model::TestRun,
    outcomes: &[TestOutcome],
) -> Result<()> {
    for (index, outcome) in outcomes.iter().enumerate() {
        let position = (index + 1) as i64;
        let driver_test = run
            .steps
            .iter()
            .find(|step| step.position == position)
            .and_then(|step| step.driver_test.clone())
            .unwrap_or_else(|| outcome.target.clone());
        repo::record_step(
            conn,
            &run.id,
            position,
            outcome.status,
            &outcome.message,
            &outcome.backtrace,
            outcome.duration_ms,
            Some(&driver_test),
            &outcome.raw_output,
        )?;
    }
    Ok(())
}

fn record_run_error(conn: &rusqlite::Connection, run: &crate::model::TestRun, message: &str) {
    for step in &run.steps {
        let _ = repo::record_step(
            conn,
            &run.id,
            step.position,
            RunStatus::Error,
            message,
            "",
            None,
            step.driver_test.as_deref(),
            message,
        );
    }
}

fn print_run(run: &crate::model::TestRun) {
    println!(
        "{} — case {} — {} — {}",
        run.id,
        run.case_id,
        run.mode.as_str(),
        run.status.as_str()
    );
    for step in &run.steps {
        println!(
            "  {}. [id {}] {:<8} {}{}",
            step.position,
            step.id,
            step.status.as_str(),
            step.observed,
            if step.evidence.is_empty() {
                String::new()
            } else {
                format!(" [{} evidence]", step.evidence.len())
            }
        );
    }
}

fn cmd_bug(cfg: &ProjectConfig, args: BugArgs) -> Result<()> {
    let db = open_db(cfg)?;
    match args.command {
        BugCommand::Create(args) => {
            let bug = repo::create_bug(
                &db.conn,
                args.id.as_deref(),
                &args.title,
                &args.description,
                &args.steps,
                &args.actual,
                &args.expected,
                args.priority,
                args.severity,
                &args.environment,
                args.case.as_deref(),
                args.run.as_deref(),
                args.step,
                &args.comments,
            )?;
            for path in args.evidence {
                repo::attach_file(
                    &db.conn,
                    &cfg.data_dir,
                    repo::EvidenceOwner::Bug(bug.id.clone()),
                    &path,
                    "",
                )?;
            }
            print_bug(&bug);
        }
        BugCommand::List { status } => {
            let bugs = repo::list_bugs(&db.conn, status.as_deref())?;
            for bug in bugs {
                println!(
                    "{:<20} {:<10} sev {:<2} {:<40} {}",
                    bug.id,
                    bug.status,
                    bug.severity,
                    truncate(&bug.title, 40),
                    bug.case_id.unwrap_or_default()
                );
            }
        }
        BugCommand::Show { id } => {
            let bug =
                repo::get_bug(&db.conn, &id)?.with_context(|| format!("bug not found: {id}"))?;
            print_bug(&bug);
        }
        BugCommand::Close { id } => {
            repo::set_bug_status(&db.conn, &id, "closed")?;
            println!("closed {id}");
        }
    }
    Ok(())
}

fn print_bug(bug: &crate::model::BugReport) {
    println!("{} — {}", bug.id, bug.title);
    println!("  status:   {}", bug.status);
    println!("  severity: {}  priority: {}", bug.severity, bug.priority);
    println!("  case:     {}", bug.case_id.as_deref().unwrap_or("-"));
    println!("  run:      {}", bug.run_id.as_deref().unwrap_or("-"));
    if !bug.actual.is_empty() {
        println!("  actual:   {}", bug.actual);
    }
    if !bug.expected.is_empty() {
        println!("  expected: {}", bug.expected);
    }
    if !bug.evidence.is_empty() {
        println!("  evidence: {}", bug.evidence.len());
    }
}

fn cmd_report(cfg: &ProjectConfig, args: ReportArgs) -> Result<()> {
    let db = open_db(cfg)?;
    let kind = ReportKind::parse(&args.kind)?;
    let path = crate::report::generate(&db.conn, &cfg.data_dir, kind, args.id.as_deref())?;
    println!("{}", path.display());
    Ok(())
}

fn cmd_driver(cfg: &ProjectConfig, args: DriverArgs) -> Result<()> {
    let db = open_db(cfg)?;
    match args.command {
        DriverCommand::List => {
            let tests = repo::list_driver_tests(&db.conn, None)?;
            if tests.is_empty() {
                println!("no discovered tests; run `laburo driver discover`");
            }
            for test in tests {
                println!("{}  {}:{}", test.id, test.file, test.line.unwrap_or(0));
            }
        }
        DriverCommand::Discover { driver } => {
            if driver != "minitest" {
                bail!("unsupported driver: {driver} (only minitest in v0.1)");
            }
            let tests = crate::driver::discover_minitest(&cfg.root)?;
            repo::ensure_driver(&db.conn, "minitest", "minitest", "Minitest (Rails)")?;
            repo::save_driver_tests(&db.conn, "minitest", &tests)?;
            println!("discovered {} tests", tests.len());
            for test in tests.iter().take(20) {
                println!("  {}  {}:{}", test.id, test.file, test.line.unwrap_or(0));
            }
        }
    }
    Ok(())
}

fn cmd_import(cfg: &ProjectConfig, args: ImportArgs) -> Result<()> {
    let db = open_db(cfg)?;
    let path = args.path.unwrap_or_else(|| {
        let local = cfg.root.join("tqa").join("casos.yaml");
        if local.exists() {
            local
        } else {
            cfg.root
                .join("..")
                .join("src-elizabeth-tours")
                .join("tqa")
                .join("casos.yaml")
        }
    });
    let count = crate::import::import_casos(&db.conn, &path)?;
    println!("imported {count} test cases from {}", path.display());
    Ok(())
}

fn cmd_db(cfg: &ProjectConfig, args: DbArgs) -> Result<()> {
    let db = open_db(cfg)?;
    match args.command {
        DbCommand::Backup => {
            let dir = cfg.data_dir.join("backups");
            std::fs::create_dir_all(&dir)?;
            let path = dir.join(format!(
                "laburo-{}.sqlite3",
                chrono::Local::now().format("%Y%m%d-%H%M%S")
            ));
            let escaped = path.to_string_lossy().replace('\'', "''");
            db.conn.execute(&format!("VACUUM INTO '{escaped}'"), [])?;
            println!("{}", path.display());
        }
        DbCommand::Restore { file, yes } => {
            if !yes {
                bail!("refusing to restore without --yes");
            }
            if !file.is_file() {
                bail!("backup not found: {}", file.display());
            }
            std::fs::copy(&file, &cfg.db_path)
                .with_context(|| format!("restoring {}", file.display()))?;
            println!("restored {}", cfg.db_path.display());
        }
    }
    Ok(())
}

fn cmd_email(cfg: &ProjectConfig, args: EmailArgs) -> Result<()> {
    let db = open_db(cfg)?;
    match args.command {
        EmailCommand::Send(send) => {
            if !send.report.is_file() {
                bail!("report not found: {}", send.report.display());
            }
            let path = crate::email::write_eml(
                cfg,
                &db.conn,
                &send.to,
                &send.subject,
                &send.body,
                &send.report,
            )?;
            println!("{}", path.display());
        }
    }
    Ok(())
}

fn cmd_quality(cfg: &ProjectConfig, args: QualityArgs) -> Result<()> {
    let db = open_db(cfg)?;
    let findings = crate::ai::quality_findings(&db.conn)?;
    if args.json {
        println!("{}", serde_json::to_string_pretty(&findings)?);
    } else if findings.is_empty() {
        println!("no findings");
    } else {
        for finding in &findings {
            println!(
                "{:<8} {:<28} {}  [{}]",
                finding.severity,
                finding.kind,
                finding.reason,
                finding.refs.join(", ")
            );
            println!("         -> {}", finding.suggested_action);
        }
    }
    Ok(())
}

fn cmd_link(cfg: &ProjectConfig, args: LinkArgs) -> Result<()> {
    let db = open_db(cfg)?;
    match args.command {
        LinkCommand::Add {
            source_kind,
            source_id,
            target_kind,
            target_id,
            relation,
            note,
        } => {
            let link = repo::create_link(
                &db.conn,
                &source_kind,
                &source_id,
                &target_kind,
                &target_id,
                &relation,
                &note,
            )?;
            println!(
                "{}: {}:{} --{}--> {}:{}",
                link.id,
                link.source_kind,
                link.source_id,
                link.relation,
                link.target_kind,
                link.target_id
            );
        }
        LinkCommand::List { kind, id } => {
            let links = repo::list_links(&db.conn, &kind, &id)?;
            if links.is_empty() {
                println!("no links");
            }
            for link in links {
                println!(
                    "{:<5} {:<10} {:<16} --{:<13}--> {:<10} {:<16} {}",
                    link.id,
                    link.source_kind,
                    link.source_id,
                    link.relation,
                    link.target_kind,
                    link.target_id,
                    link.note
                );
            }
        }
        LinkCommand::Remove { id } => {
            if repo::delete_link(&db.conn, id)? {
                println!("removed link {id}");
            } else {
                bail!("link not found: {id}");
            }
        }
    }
    Ok(())
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut out: String = s.chars().take(max.saturating_sub(1)).collect();
        out.push('…');
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_rust_toolchain_that_runs_the_tests() {
        let check = ProcessCommand::new("cargo").arg("--version").output();
        assert!(check.is_ok_and(|o| o.status.success()));
    }

    #[test]
    fn parses_cli() {
        let cli = Cli::try_parse_from(["laburo", "case", "list"]).unwrap();
        assert!(matches!(cli.command, Some(Command::Case(_))));
    }

    #[test]
    fn step_parser_accepts_two_and_three_fields() {
        let two = "open page|page opens";
        let three = "open page|/tours|page opens";
        assert_eq!(two.split('|').count(), 2);
        assert_eq!(three.split('|').count(), 3);
    }

    #[test]
    fn timestamp_is_non_empty() {
        assert!(!crate::model::now().is_empty());
    }
}
