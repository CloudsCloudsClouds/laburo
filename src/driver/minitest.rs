//! Minitest driver.
//!
//! Discovery parses `test/**/*_test.rb` without loading Ruby. Execution spawns
//! `bin/rails test` (or `bin/rails test:system`) with an argv list, captures
//! stdout/stderr, enforces a timeout, strips ANSI codes, and parses verbose
//! result lines plus the Minitest summary.

use super::{Capabilities, Driver, RunOptions};
use crate::model::{DiscoveredTest, RunStatus, TestOutcome};
use anyhow::{Context, Result, bail};
use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant};

/// Target used by [`parse_minitest_output`] when the output has a summary but
/// no per-test verbose lines.
pub const SUMMARY_TARGET: &str = "minitest";

#[derive(Debug, Default, Clone, Copy)]
pub struct MinitestDriver;

impl MinitestDriver {
    /// Associated-function form of [`Driver::discover`].
    pub fn discover(project: &Path) -> Result<Vec<DiscoveredTest>> {
        discover(project)
    }

    /// Associated-function form of [`Driver::run`].
    pub fn run(
        project: &Path,
        targets: &[String],
        options: &RunOptions,
    ) -> Result<Vec<TestOutcome>> {
        run(project, targets, options)
    }
}

impl Driver for MinitestDriver {
    fn id(&self) -> &str {
        "minitest"
    }

    fn name(&self) -> &str {
        "Minitest"
    }

    fn capabilities(&self) -> Capabilities {
        Capabilities::MINITEST
    }

    fn discover(&self, project: &Path) -> Result<Vec<DiscoveredTest>> {
        discover(project)
    }

    fn run(
        &self,
        project: &Path,
        targets: &[String],
        options: &RunOptions,
    ) -> Result<Vec<TestOutcome>> {
        run(project, targets, options)
    }
}

/// Recursively scan `test/**/*_test.rb` and return the tests found.
///
/// IDs are stable within the driver: `relative/path.rb#Class#method`.
pub fn discover(project: &Path) -> Result<Vec<DiscoveredTest>> {
    let test_root = project.join("test");
    if !test_root.is_dir() {
        bail!(
            "no test directory found at {}; point --project at the Rails app root",
            test_root.display()
        );
    }

    let mut files = Vec::new();
    collect_test_files(&test_root, &mut files)?;
    files.sort();

    let mut tests = Vec::new();
    for file in files {
        let relative = relative_path(project, &file);
        let source = fs::read_to_string(&file)
            .with_context(|| format!("reading Minitest file {}", file.display()))?;
        tests.extend(parse_test_file(&relative, &source));
    }
    Ok(tests)
}

/// Run the given targets in one Rails test invocation.
///
/// Accepted target shapes:
///
/// - a discovered id (`test/models/user_test.rb#UserTest#test_valid`)
/// - a Rails class/method (`UserTest#test_valid`), resolved by discovery
/// - a file or file:line (`test/models/user_test.rb:12`)
///
/// `--verbose` is passed so every test has a parseable result line.
pub fn run(project: &Path, targets: &[String], options: &RunOptions) -> Result<Vec<TestOutcome>> {
    if targets.is_empty() {
        bail!("minitest run needs at least one target (file, file:line, or discovered test id)");
    }

    let resolved = resolve_targets(project, targets)?;
    let system_targets = resolved
        .iter()
        .filter(|target| is_system_test_path(&target.file))
        .count();
    let system = options.system || system_targets > 0;

    if !options.system && system_targets > 0 && system_targets != resolved.len() {
        bail!(
            "cannot mix system and non-system Minitest targets in one run; \
             run test/system targets separately or set RunOptions::system for all"
        );
    }

    let subcommand = if system { "test:system" } else { "test" };
    let rails = project.join("bin").join("rails");
    if !rails.is_file() {
        bail!(
            "{} not found; expected a Rails app with bin/rails at the project root",
            rails.display()
        );
    }

    let mut command = Command::new(&rails);
    command
        .current_dir(project)
        .arg(subcommand)
        .arg("--verbose");
    for target in &resolved {
        command.arg(&target.arg);
    }
    command.stdout(Stdio::piped()).stderr(Stdio::piped());

    let started = Instant::now();
    let mut child = command
        .spawn()
        .with_context(|| format!("running {} {subcommand}", rails.display()))?;

    let stdout_handle = child.stdout.take().map(spawn_reader);
    let stderr_handle = child.stderr.take().map(spawn_reader);

    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Some(status),
            Ok(None) => {
                if started.elapsed() >= options.timeout {
                    let _ = child.kill();
                    let _ = child.wait();
                    break None;
                }
                std::thread::sleep(Duration::from_millis(25));
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(error)
                    .with_context(|| format!("waiting for {} {subcommand}", rails.display()));
            }
        }
    };

    let timed_out = status.is_none();
    let (stdout, stderr) = if timed_out {
        // The reader threads can stay blocked on a killed child's pipe if the
        // child spawned grandchildren. Do not join them on the timeout path:
        // the timeout result must return immediately.
        drop(stdout_handle);
        drop(stderr_handle);
        (Vec::new(), Vec::new())
    } else {
        (join_reader(stdout_handle), join_reader(stderr_handle))
    };
    let elapsed_ms = duration_ms(started.elapsed());

    if timed_out {
        let raw_output = combine_output(&stdout, &stderr);
        let message = format!(
            "minitest timed out after {:.1}s; the child process was killed",
            options.timeout.as_secs_f64()
        );
        return Ok(resolved
            .iter()
            .map(|target| TestOutcome {
                target: target.original.clone(),
                status: RunStatus::Error,
                duration_ms: Some(elapsed_ms),
                message: message.clone(),
                backtrace: String::new(),
                raw_output: raw_output.clone(),
                evidence: Vec::new(),
            })
            .collect());
    }

    let status = status.unwrap_or_else(|| unreachable!("checked above"));
    let combined = combine_output(&stdout, &stderr);
    let mut outcomes = parse_minitest_output(&combined);
    enrich_targets(project, &resolved, &mut outcomes);

    if !status.success() && !outcomes.iter().any(|outcome| outcome.status.is_bad()) {
        outcomes.push(TestOutcome {
            target: resolved
                .iter()
                .map(|target| target.original.as_str())
                .collect::<Vec<_>>()
                .join(", "),
            status: RunStatus::Error,
            duration_ms: Some(elapsed_ms),
            message: format!(
                "{} {subcommand} exited with {status}; see raw output",
                rails.display()
            ),
            backtrace: String::new(),
            raw_output: combined,
            evidence: Vec::new(),
        });
    }

    Ok(outcomes)
}

/// Parse Minitest output into outcomes.
///
/// This is a pure function so fixtures can exercise it without Rails. It strips
/// ANSI codes, reads verbose lines (`Class#test = 0.12 s = .`), attaches failure
/// details, and falls back to the `N runs, N assertions, ...` summary when no
/// per-test lines are present.
pub fn parse_minitest_output(output: &str) -> Vec<TestOutcome> {
    let clean = strip_ansi(output);
    let mut outcomes: Vec<TestOutcome> = Vec::new();
    let mut summary = None;
    let mut summary_line = String::new();

    for line in clean.lines() {
        if let Some(parsed) = parse_verbose_line(line) {
            outcomes.push(TestOutcome {
                target: parsed.target.to_string(),
                status: parsed.status,
                duration_ms: parsed.duration_ms,
                message: String::new(),
                backtrace: String::new(),
                raw_output: clean.clone(),
                evidence: Vec::new(),
            });
            continue;
        }
        if let Some(parsed_summary) = parse_summary_line(line) {
            summary = Some(parsed_summary);
            summary_line = line.trim().to_string();
        }
    }

    let failure_blocks = parse_failure_blocks(&clean);
    for outcome in &mut outcomes {
        if let Some(block) = failure_blocks
            .iter()
            .find(|block| block.target == outcome.target)
        {
            outcome.message.clone_from(&block.message);
            outcome.backtrace.clone_from(&block.backtrace);
        }
    }

    if outcomes.is_empty() {
        if let Some(summary) = summary {
            if summary.runs > 0 {
                let status = if summary.errors > 0 {
                    RunStatus::Error
                } else if summary.failures > 0 {
                    RunStatus::Failed
                } else {
                    RunStatus::Passed
                };
                outcomes.push(TestOutcome {
                    target: SUMMARY_TARGET.to_string(),
                    status,
                    duration_ms: None,
                    message: summary_line,
                    backtrace: String::new(),
                    raw_output: clean,
                    evidence: Vec::new(),
                });
            }
        } else if !clean.trim().is_empty() {
            let detail = clean
                .lines()
                .rev()
                .find(|line| !line.trim().is_empty())
                .map(str::trim)
                .unwrap_or_default();
            let message = if detail.is_empty() {
                "could not parse minitest output".to_string()
            } else {
                format!("could not parse minitest output: {detail}")
            };
            outcomes.push(TestOutcome {
                target: SUMMARY_TARGET.to_string(),
                status: RunStatus::Error,
                duration_ms: None,
                message,
                backtrace: String::new(),
                raw_output: clean,
                evidence: Vec::new(),
            });
        }
    }

    outcomes
}

/// Remove ANSI CSI/OSC escape sequences so parsed output is stable in reports.
pub fn strip_ansi(input: &str) -> String {
    let bytes = input.as_bytes();
    let mut output = Vec::with_capacity(bytes.len());
    let mut index = 0;

    while index < bytes.len() {
        if bytes[index] != 0x1b {
            output.push(bytes[index]);
            index += 1;
            continue;
        }

        index += 1;
        if index >= bytes.len() {
            break;
        }

        match bytes[index] {
            b'[' => {
                index += 1;
                while index < bytes.len() && !(0x40..=0x7e).contains(&bytes[index]) {
                    index += 1;
                }
                if index < bytes.len() {
                    index += 1;
                }
            }
            b']' => {
                index += 1;
                while index < bytes.len() {
                    if bytes[index] == 0x07 {
                        index += 1;
                        break;
                    }
                    if bytes[index] == 0x1b && index + 1 < bytes.len() && bytes[index + 1] == b'\\'
                    {
                        index += 2;
                        break;
                    }
                    index += 1;
                }
            }
            _ => index += 1,
        }
    }

    String::from_utf8_lossy(&output).into_owned()
}

fn collect_test_files(directory: &Path, files: &mut Vec<PathBuf>) -> Result<()> {
    let mut entries = fs::read_dir(directory)
        .with_context(|| format!("reading directory {}", directory.display()))?
        .collect::<std::io::Result<Vec<_>>>()
        .with_context(|| format!("reading directory {}", directory.display()))?;
    entries.sort_by_key(|entry| entry.file_name());

    for entry in entries {
        let path = entry.path();
        let file_type = entry
            .file_type()
            .with_context(|| format!("reading file type of {}", path.display()))?;
        if file_type.is_dir() {
            collect_test_files(&path, files)?;
        } else if file_type.is_file()
            && path
                .file_name()
                .and_then(|name| name.to_str())
                .is_some_and(|name| name.ends_with("_test.rb"))
        {
            files.push(path);
        }
    }
    Ok(())
}

fn relative_path(project: &Path, file: &Path) -> String {
    file.strip_prefix(project)
        .unwrap_or(file)
        .to_string_lossy()
        .replace('\\', "/")
}

fn parse_test_file(relative: &str, source: &str) -> Vec<DiscoveredTest> {
    let mut tests = Vec::new();
    let mut current_class: Option<(String, usize)> = None;
    let mut in_block_comment = false;

    for (index, raw_line) in source.lines().enumerate() {
        let line_number = index + 1;
        let trimmed = raw_line.trim();

        if in_block_comment {
            if trimmed.starts_with("=end") {
                in_block_comment = false;
            }
            continue;
        }
        if trimmed.starts_with("=begin") {
            in_block_comment = true;
            continue;
        }
        if trimmed.is_empty() || trimmed.starts_with('#') {
            continue;
        }

        let indent = raw_line.len() - raw_line.trim_start().len();
        if trimmed == "end" {
            if let Some((_, class_indent)) = &current_class {
                if indent <= *class_indent {
                    current_class = None;
                }
            }
            continue;
        }

        if let Some(class_name) = parse_class_declaration(trimmed) {
            current_class = Some((class_name, indent));
            continue;
        }

        let Some((class_name, _)) = &current_class else {
            continue;
        };
        if let Some(declaration) = parse_test_declaration(trimmed) {
            tests.push(discovered_test(
                relative,
                line_number,
                class_name,
                &declaration,
            ));
        }
    }

    tests
}

fn parse_class_declaration(line: &str) -> Option<String> {
    let rest = line.strip_prefix("class ")?;
    if rest.starts_with("<<") {
        return None;
    }

    let name: String = rest
        .chars()
        .take_while(|character| {
            character.is_ascii_alphanumeric() || *character == '_' || *character == ':'
        })
        .collect();
    if name.is_empty() || !name.ends_with("Test") {
        return None;
    }
    if !name
        .chars()
        .next()
        .is_some_and(|character| character.is_ascii_uppercase())
    {
        return None;
    }

    let tail = rest[name.len()..].trim_start();
    if !tail.starts_with('<') {
        return None;
    }
    Some(name)
}

struct TestDeclaration {
    method: String,
    kind: &'static str,
    description: Option<String>,
}

fn parse_test_declaration(line: &str) -> Option<TestDeclaration> {
    if let Some(rest) = line.strip_prefix("def ") {
        let method: String = rest
            .chars()
            .take_while(|character| {
                character.is_ascii_alphanumeric() || matches!(character, '_' | '?' | '!')
            })
            .collect();
        if !method.starts_with("test_") || method.len() <= "test_".len() {
            return None;
        }
        return Some(TestDeclaration {
            method,
            kind: "def",
            description: None,
        });
    }

    let rest = line.strip_prefix("test")?.trim_start();
    let rest = rest.strip_prefix('(').unwrap_or(rest).trim_start();
    let quote = rest.chars().next()?;
    if quote != '"' && quote != '\'' {
        return None;
    }

    let mut escaped = false;
    let mut closing = None;
    for (index, character) in rest.char_indices().skip(1) {
        if escaped {
            escaped = false;
            continue;
        }
        if character == '\\' && quote == '"' {
            escaped = true;
            continue;
        }
        if character == quote {
            closing = Some(index);
            break;
        }
    }
    let closing = closing?;
    let description = rest[1..closing].trim();
    if description.is_empty() {
        return None;
    }

    let after = rest[closing + 1..].trim_start();
    let after = after.strip_prefix(')').unwrap_or(after).trim_start();
    if !(after.starts_with("do") || after.starts_with('{')) {
        return None;
    }

    Some(TestDeclaration {
        method: minitest_method_name(description),
        kind: "test",
        description: Some(description.to_string()),
    })
}

fn minitest_method_name(description: &str) -> String {
    let slug = description.split_whitespace().collect::<Vec<_>>().join("_");
    format!("test_{slug}")
}

fn discovered_test(
    relative: &str,
    line_number: usize,
    class_name: &str,
    declaration: &TestDeclaration,
) -> DiscoveredTest {
    let id = format!("{relative}#{class_name}#{}", declaration.method);
    let name = format!("{class_name}#{}", declaration.method);
    let mut metadata = serde_json::json!({
        "class": class_name,
        "method": declaration.method,
        "kind": declaration.kind,
        "system": is_system_test_path(relative),
    });
    if let Some(description) = &declaration.description {
        metadata["description"] = serde_json::Value::String(description.clone());
    }

    DiscoveredTest {
        id,
        driver_id: "minitest".to_string(),
        name,
        file: relative.to_string(),
        line: Some(line_number as i64),
        metadata,
    }
}

pub(crate) fn is_system_test_path(path: &str) -> bool {
    path.replace('\\', "/").contains("test/system")
}

fn target_file(target: &str) -> &str {
    if let Some((file, line)) = target.rsplit_once(':')
        && !line.is_empty()
        && line.chars().all(|character| character.is_ascii_digit())
    {
        return file;
    }
    target
}

struct ResolvedTarget {
    original: String,
    arg: String,
    file: String,
}

fn resolve_targets(project: &Path, targets: &[String]) -> Result<Vec<ResolvedTarget>> {
    let mut discovered: Option<Vec<DiscoveredTest>> = None;
    let mut resolved = Vec::with_capacity(targets.len());

    for raw in targets {
        let target = raw.trim();
        if target.is_empty() {
            bail!("empty Minitest target");
        }

        if let Some((file, _, _)) = parse_discovery_id(target) {
            let id = target.to_string();
            let tests = discovered_or_load(&mut discovered, project)?;
            let found = tests.iter().find(|test| test.id == id);
            let arg = match found.and_then(|test| test.line) {
                Some(line) => format!("{file}:{line}"),
                None => file.to_string(),
            };
            resolved.push(ResolvedTarget {
                original: id,
                arg,
                file: file.to_string(),
            });
        } else if target.contains('#') {
            let tests = discovered_or_load(&mut discovered, project)?;
            let Some(found) = tests.iter().find(|test| test.name == target) else {
                bail!(
                    "Minitest target '{target}' was not found by discovery; \
                     use a discovered test id, file:line, or file"
                );
            };
            let arg = match found.line {
                Some(line) => format!("{}:{line}", found.file),
                None => found.file.clone(),
            };
            resolved.push(ResolvedTarget {
                original: target.to_string(),
                arg,
                file: found.file.clone(),
            });
        } else {
            resolved.push(ResolvedTarget {
                original: target.to_string(),
                arg: target.to_string(),
                file: target_file(target).to_string(),
            });
        }
    }

    Ok(resolved)
}

fn parse_discovery_id(value: &str) -> Option<(&str, &str, &str)> {
    let mut parts = value.rsplitn(3, '#');
    let method = parts.next()?;
    let class_name = parts.next()?;
    let file = parts.next()?;
    if file.is_empty() || method.is_empty() || class_name.is_empty() || !file.ends_with(".rb") {
        return None;
    }
    Some((file, class_name, method))
}

fn discovered_or_load<'a>(
    cache: &'a mut Option<Vec<DiscoveredTest>>,
    project: &Path,
) -> Result<&'a [DiscoveredTest]> {
    if cache.is_none() {
        *cache = Some(discover(project)?);
    }
    Ok(cache.as_deref().unwrap_or_default())
}

fn enrich_targets(project: &Path, resolved: &[ResolvedTarget], outcomes: &mut [TestOutcome]) {
    let files: Vec<&str> = resolved.iter().map(|target| target.file.as_str()).collect();
    let Ok(tests) = discover(project) else {
        return;
    };

    for outcome in outcomes {
        if outcome.target == SUMMARY_TARGET {
            continue;
        }
        if let Some(found) = tests
            .iter()
            .find(|test| test.name == outcome.target && files.iter().any(|file| *file == test.file))
        {
            outcome.target.clone_from(&found.id);
        }
    }
}

fn spawn_reader(mut pipe: impl Read + Send + 'static) -> std::thread::JoinHandle<Vec<u8>> {
    std::thread::spawn(move || {
        let mut buffer = Vec::new();
        let _ = pipe.read_to_end(&mut buffer);
        buffer
    })
}

fn join_reader(handle: Option<std::thread::JoinHandle<Vec<u8>>>) -> Vec<u8> {
    handle
        .and_then(|handle| handle.join().ok())
        .unwrap_or_default()
}

fn combine_output(stdout: &[u8], stderr: &[u8]) -> String {
    let mut combined = String::from_utf8_lossy(stdout).into_owned();
    if !stderr.is_empty() {
        if !combined.is_empty() && !combined.ends_with('\n') {
            combined.push('\n');
        }
        combined.push_str(&String::from_utf8_lossy(stderr));
    }
    combined
}

fn duration_ms(duration: Duration) -> i64 {
    i64::try_from(duration.as_millis()).unwrap_or(i64::MAX)
}

struct VerboseLine<'a> {
    target: &'a str,
    duration_ms: Option<i64>,
    status: RunStatus,
}

fn parse_verbose_line(line: &str) -> Option<VerboseLine<'_>> {
    let parts: Vec<&str> = line.trim().split(" = ").collect();
    if parts.len() < 2 || parts.len() > 3 {
        return None;
    }

    let target = parts[0].trim();
    if !target.contains('#') || target.contains(char::is_whitespace) {
        return None;
    }

    let (duration, marker) = if parts.len() == 3 {
        (Some(parts[1]), parts[2].trim())
    } else {
        (None, parts[1].trim())
    };
    let status = match marker {
        "." => RunStatus::Passed,
        "F" => RunStatus::Failed,
        "E" => RunStatus::Error,
        "S" => RunStatus::Blocked,
        _ => return None,
    };
    let duration_ms = duration.and_then(parse_duration_ms);

    Some(VerboseLine {
        target,
        duration_ms,
        status,
    })
}

fn parse_duration_ms(raw: &str) -> Option<i64> {
    let seconds = raw.trim().strip_suffix(" s")?.trim().parse::<f64>().ok()?;
    if !seconds.is_finite() || seconds < 0.0 {
        return None;
    }
    Some((seconds * 1000.0).round() as i64)
}

#[derive(Debug, Default)]
struct Summary {
    runs: u64,
    assertions: u64,
    failures: u64,
    errors: u64,
    skips: u64,
}

fn parse_summary_line(line: &str) -> Option<Summary> {
    let mut summary = Summary::default();
    let mut saw_runs = false;
    let mut saw_assertions = false;

    for part in line.split(',') {
        let mut words = part.split_whitespace();
        let Some(count) = words.next().and_then(|word| word.parse::<u64>().ok()) else {
            continue;
        };
        let Some(label) = words.next() else {
            continue;
        };
        match label {
            "runs" => {
                summary.runs = count;
                saw_runs = true;
            }
            "assertions" => {
                summary.assertions = count;
                saw_assertions = true;
            }
            "failures" => summary.failures = count,
            "errors" => summary.errors = count,
            "skips" => summary.skips = count,
            _ => {}
        }
    }

    (saw_runs && saw_assertions).then_some(summary)
}

struct FailureBlock {
    target: String,
    message: String,
    backtrace: String,
}

fn parse_failure_blocks(output: &str) -> Vec<FailureBlock> {
    let lines: Vec<&str> = output.lines().collect();
    let mut blocks = Vec::new();
    let mut index = 0;

    while index < lines.len() {
        let heading = lines[index].trim();
        if !matches!(heading, "Failure:" | "Error:" | "Skipped:") {
            index += 1;
            continue;
        }

        let mut cursor = index + 1;
        while cursor < lines.len() && lines[cursor].trim().is_empty() {
            cursor += 1;
        }
        if cursor >= lines.len() {
            break;
        }

        let header = lines[cursor].trim();
        let target = header
            .split(" [")
            .next()
            .unwrap_or(header)
            .trim_end_matches(':')
            .trim();
        if !target.contains('#') {
            index += 1;
            continue;
        }
        cursor += 1;

        let mut body = Vec::new();
        while cursor < lines.len() {
            let body_line = lines[cursor].trim();
            if body_line.is_empty()
                || matches!(body_line, "Failure:" | "Error:" | "Skipped:")
                || body_line.starts_with("Finished in")
                || parse_summary_line(body_line).is_some()
            {
                break;
            }
            body.push(body_line.to_string());
            cursor += 1;
        }

        blocks.push(FailureBlock {
            target: target.to_string(),
            message: body.first().cloned().unwrap_or_default(),
            backtrace: body.iter().skip(1).cloned().collect::<Vec<_>>().join("\n"),
        });
        index = cursor;
    }

    blocks
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(unix)]
    use std::os::unix::fs::PermissionsExt;
    use tempfile::{TempDir, tempdir};

    const PASS_OUTPUT: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/minitest/output_pass.txt"
    ));
    const FAIL_OUTPUT: &str = include_str!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/fixtures/minitest/output_fail.txt"
    ));

    fn fixture_project() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/minitest")
    }

    fn write_project(files: &[(&str, &str)]) -> TempDir {
        let directory = tempdir().expect("tempdir");
        for &(relative, contents) in files {
            let path = directory.path().join(relative);
            if let Some(parent) = path.parent() {
                fs::create_dir_all(parent).expect("create fixture dir");
            }
            fs::write(&path, contents).expect("write fixture file");
            if relative == "bin/rails" {
                let mut permissions = fs::metadata(&path).expect("metadata").permissions();
                permissions.set_mode(0o755);
                fs::set_permissions(&path, permissions).expect("chmod fake rails");
            }
        }
        directory
    }

    #[test]
    fn discovers_fixture_tests() {
        let tests = discover(&fixture_project()).expect("discover fixture");

        let ids: Vec<&str> = tests.iter().map(|test| test.id.as_str()).collect();
        assert_eq!(
            ids,
            vec![
                "test/models/user_test.rb#UserTest#test_valid_user",
                "test/models/user_test.rb#UserTest#test_rejects_blank_name",
            ]
        );
        assert_eq!(tests[0].file, "test/models/user_test.rb");
        assert_eq!(tests[0].driver_id, "minitest");
        assert_eq!(tests[0].name, "UserTest#test_valid_user");
        assert_eq!(tests[0].line, Some(4));
        assert_eq!(tests[0].metadata["class"], "UserTest");
        assert_eq!(tests[0].metadata["method"], "test_valid_user");
        assert_eq!(tests[0].metadata["kind"], "test");
        assert_eq!(tests[1].metadata["kind"], "def");
        assert_eq!(tests[1].line, Some(8));
    }

    #[test]
    fn discovery_ignores_comments_and_tracks_classes() {
        let project = write_project(&[(
            "test/alpha_test.rb",
            r#"# class CommentedTest < ActiveSupport::TestCase
#   def test_commented
=begin
class BlockTest < ActiveSupport::TestCase
  def test_blocked
=end

class AlphaTest < ActiveSupport::TestCase
  def test_one
  end

  test "does the thing" do
  end

  def helper_method
  end
end

class BetaTest < ActiveSupport::TestCase
  def test_two
  end
end
"#,
        )]);

        let tests = discover(project.path()).expect("discover");
        let found: Vec<(&str, Option<i64>)> = tests
            .iter()
            .map(|test| (test.id.as_str(), test.line))
            .collect();
        assert_eq!(
            found,
            vec![
                ("test/alpha_test.rb#AlphaTest#test_one", Some(9)),
                ("test/alpha_test.rb#AlphaTest#test_does_the_thing", Some(12)),
                ("test/alpha_test.rb#BetaTest#test_two", Some(20)),
            ]
        );
        assert_eq!(
            tests[1].metadata["description"],
            serde_json::Value::String("does the thing".to_string())
        );
    }

    #[test]
    fn discovery_marks_system_tests() {
        let project = write_project(&[(
            "test/system/checkout_test.rb",
            "class CheckoutTest < ApplicationSystemTestCase\n  def test_pays\n  end\nend\n",
        )]);

        let tests = discover(project.path()).expect("discover");
        assert_eq!(tests.len(), 1);
        assert_eq!(tests[0].metadata["system"], true);
        assert!(is_system_test_path(&tests[0].file));
    }

    #[test]
    fn discovery_errors_without_test_directory() {
        let project = tempdir().expect("tempdir");
        let error = discover(project.path()).expect_err("missing test/ must fail");
        assert!(error.to_string().contains("no test directory"));
    }

    #[test]
    fn parses_passing_output() {
        let outcomes = parse_minitest_output(PASS_OUTPUT);

        assert_eq!(outcomes.len(), 2);
        assert_eq!(outcomes[0].target, "UserTest#test_valid_user");
        assert_eq!(outcomes[0].status, RunStatus::Passed);
        assert_eq!(outcomes[0].duration_ms, Some(12));
        assert_eq!(outcomes[1].target, "UserTest#test_rejects_blank_name");
        assert_eq!(outcomes[1].status, RunStatus::Passed);
        assert!(
            outcomes
                .iter()
                .all(|outcome| !outcome.raw_output.contains('\u{1b}'))
        );
    }

    #[test]
    fn parses_failing_output_with_message() {
        let outcomes = parse_minitest_output(FAIL_OUTPUT);

        assert_eq!(outcomes.len(), 2);
        assert_eq!(outcomes[0].status, RunStatus::Passed);
        assert_eq!(outcomes[1].status, RunStatus::Failed);
        assert_eq!(outcomes[1].target, "UserTest#test_rejects_blank_name");
        assert_eq!(outcomes[1].message, "Expected true to be false.");
        assert!(outcomes[1].raw_output.contains("1 failures"));
    }

    #[test]
    fn parses_error_and_skip_markers() {
        let output = "\
UserTest#test_error = 0.010 s = E
UserTest#test_skip = 0.001 s = S

Error:
UserTest#test_error:
RuntimeError: boom

2 runs, 2 assertions, 1 failures, 1 errors, 0 skips
";
        let outcomes = parse_minitest_output(output);

        assert_eq!(outcomes.len(), 2);
        assert_eq!(outcomes[0].status, RunStatus::Error);
        assert_eq!(outcomes[0].message, "RuntimeError: boom");
        assert_eq!(outcomes[1].status, RunStatus::Blocked);
    }

    #[test]
    fn parses_summary_without_verbose_lines() {
        let output = "Run options: --seed 42\n\nFinished in 0.010000s\n1 runs, 1 assertions, 1 failures, 0 errors, 0 skips\n";
        let outcomes = parse_minitest_output(output);

        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].target, SUMMARY_TARGET);
        assert_eq!(outcomes[0].status, RunStatus::Failed);
        assert!(outcomes[0].message.contains("1 runs"));
    }

    #[test]
    fn unparseable_output_is_an_error() {
        let outcomes = parse_minitest_output("Could not find test file\n");

        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].status, RunStatus::Error);
        assert!(outcomes[0].message.contains("could not parse"));
    }

    #[cfg(unix)]
    #[test]
    fn runs_targets_with_argv_and_parses_outcomes() {
        let project = write_project(&[
            (
                "test/models/user_test.rb",
                "class UserTest < ActiveSupport::TestCase\n  def test_valid_user\n  end\n\n  def test_rejects_blank_name\n  end\nend\n",
            ),
            (
                "bin/rails",
                "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$(dirname \"$0\")/../args.txt\"\ncat \"$(dirname \"$0\")/../output.txt\"\n",
            ),
            ("output.txt", PASS_OUTPUT),
        ]);

        let targets = vec!["test/models/user_test.rb#UserTest#test_valid_user".to_string()];
        let outcomes = MinitestDriver
            .run(project.path(), &targets, &RunOptions::default())
            .expect("run fake rails");

        assert_eq!(outcomes.len(), 2);
        assert!(
            outcomes
                .iter()
                .all(|outcome| outcome.status == RunStatus::Passed)
        );
        assert_eq!(
            outcomes[0].target,
            "test/models/user_test.rb#UserTest#test_valid_user"
        );
        let args = fs::read_to_string(project.path().join("args.txt")).expect("args");
        let args: Vec<&str> = args.lines().collect();
        assert_eq!(args[0], "test");
        assert!(args.contains(&"--verbose"));
        assert!(args.contains(&"test/models/user_test.rb:2"));
    }

    #[cfg(unix)]
    #[test]
    fn system_option_selects_test_system_task() {
        let project = write_project(&[
            (
                "test/models/user_test.rb",
                "class UserTest < ActiveSupport::TestCase\n  def test_valid_user\n  end\nend\n",
            ),
            (
                "bin/rails",
                "#!/bin/sh\nprintf '%s\\n' \"$@\" > \"$(dirname \"$0\")/../args.txt\"\nexit 0\n",
            ),
        ]);

        let targets = vec!["test/models/user_test.rb".to_string()];
        let options = RunOptions::default().system();
        MinitestDriver
            .run(project.path(), &targets, &options)
            .expect("run fake rails");

        let args = fs::read_to_string(project.path().join("args.txt")).expect("args");
        assert_eq!(args.lines().next(), Some("test:system"));
    }

    #[cfg(unix)]
    #[test]
    fn timeout_kills_child_and_reports_error() {
        let project = write_project(&[
            (
                "test/models/user_test.rb",
                "class UserTest < ActiveSupport::TestCase\n  def test_slow\n  end\nend\n",
            ),
            ("bin/rails", "#!/bin/sh\nsleep 10\n"),
        ]);

        let targets = vec!["test/models/user_test.rb".to_string()];
        let options = RunOptions::default().with_timeout(Duration::from_millis(150));
        let started = Instant::now();
        let outcomes = MinitestDriver
            .run(project.path(), &targets, &options)
            .expect("timeout is a result, not an error");

        assert!(started.elapsed() < Duration::from_secs(5));
        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].status, RunStatus::Error);
        assert!(outcomes[0].message.contains("timed out"));
    }
}
