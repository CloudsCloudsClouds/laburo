//! Command-line surface. Parse, validate, call the library. No business rules.

use std::path::PathBuf;
use std::process::Command as ProcessCommand;

use anyhow::{Result, bail};
use clap::{CommandFactory, Parser, Subcommand};

/// Test evidence for TQA: collect results, check congruence, publish the annex.
#[derive(Debug, Parser)]
#[command(name = "laburo", version, about, long_about = None)]
pub struct Cli {
    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Debug, Subcommand)]
pub enum Command {
    /// Inspect the environment and the target project.
    Doctor(DoctorArgs),
}

#[derive(Debug, clap::Args)]
pub struct DoctorArgs {
    /// Project root to inspect. Defaults to the current directory.
    #[arg(long, value_name = "DIR")]
    pub path: Option<PathBuf>,
}

pub fn run(cli: Cli) -> Result<()> {
    match cli.command {
        Some(Command::Doctor(args)) => doctor(&args),
        None => {
            Cli::command().print_help()?;
            println!();
            Ok(())
        }
    }
}

struct Check {
    label: &'static str,
    detail: String,
    ok: bool,
    required: bool,
}

fn doctor(args: &DoctorArgs) -> Result<()> {
    let root = match &args.path {
        Some(path) => path.clone(),
        None => std::env::current_dir()?,
    };
    let root = root.canonicalize().unwrap_or(root);
    println!("laburo {} — doctor", env!("CARGO_PKG_VERSION"));
    println!("target: {}", root.display());
    println!();

    let rails = root.join("bin").join("rails");
    let gemfile = root.join("Gemfile");
    let dictionary = root.join("tqa").join("casos.yaml");

    let checks = vec![
        Check {
            label: "Rails app",
            detail: rails.display().to_string(),
            ok: rails.is_file(),
            required: false,
        },
        Check {
            label: "Minitest",
            detail: gemfile.display().to_string(),
            ok: std::fs::read_to_string(&gemfile)
                .map(|text| text.contains("minitest"))
                .unwrap_or(false),
            required: false,
        },
        Check {
            label: "TQA dictionary",
            detail: dictionary.display().to_string(),
            ok: dictionary.is_file(),
            required: false,
        },
        tool_check("Typst", "typst", &["--version"], true),
        tool_check("Rust", "cargo", &["--version"], true),
        tool_check("Git", "git", &["--version"], true),
    ];

    let failures = checks
        .iter()
        .filter(|check| check.required && !check.ok)
        .count();

    for check in &checks {
        let mark = if check.ok { "ok  " } else { "MISS" };
        let optional = if check.required { "" } else { " (optional)" };
        println!("[{mark}] {:<16} {}{optional}", check.label, check.detail);
    }

    if failures > 0 {
        bail!("{failures} required check(s) failed");
    }
    Ok(())
}

fn tool_check(label: &'static str, program: &str, args: &[&str], required: bool) -> Check {
    match ProcessCommand::new(program).args(args).output() {
        Ok(output) if output.status.success() => {
            let stdout = String::from_utf8_lossy(&output.stdout);
            let stderr = String::from_utf8_lossy(&output.stderr);
            let text = if stdout.trim().is_empty() {
                stderr
            } else {
                stdout
            };
            Check {
                label,
                detail: text.lines().next().unwrap_or(program).trim().to_string(),
                ok: true,
                required,
            }
        }
        _ => Check {
            label,
            detail: format!("`{program}` not found"),
            ok: false,
            required,
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_the_rust_toolchain_that_runs_the_tests() {
        let check = tool_check("Rust", "cargo", &["--version"], true);
        assert!(check.ok, "cargo must be available while running cargo test");
    }
}
