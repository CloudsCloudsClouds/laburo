# Drivers

Status: frozen for the wave
Last updated: 2026-11-17

A driver turns "a test the user can select" into a runnable target and a result.
The manual path is always available. Minitest is the first real driver;
cargo-nextest is second; Cucumber/Selenium are later plugins.

## Trait

```rust
pub trait Driver {
    fn id(&self) -> &str;
    fn name(&self) -> &str;
    fn discover(&self, project: &Project) -> Result<Vec<DiscoveredTest>>;
    fn run(&self, project: &Project, targets: &[String], options: &RunOptions)
        -> Result<Vec<TestOutcome>>;
    fn capabilities(&self) -> Capabilities;
}

pub struct DiscoveredTest {
    pub id: String,
    pub name: String,
    pub file: String,
    pub line: Option<u32>,
    pub metadata: serde_json::Value,
}

pub struct TestOutcome {
    pub target: String,
    pub status: RunStatus,
    pub duration_ms: Option<u64>,
    pub message: String,
    pub backtrace: String,
    pub raw_output: String,
    pub evidence: Vec<EvidenceRef>,
}
```

Built-in drivers are registered in code. No dynamic library loading in this wave.

## Manual

- No discovery, no execution.
- The TUI owns the step loop.
- Used for security, installation, usability, exploratory, and any step that
  needs a human.

## Minitest (first)

Project root: the Rails app (`src-elizabeth-tours`), resolved by config or
`--project`.

Discovery:

- Parse `test/**/*_test.rb`.
- Match `class FooTest < ...` and `def test_...` / `test "..." do`.
- Capture class, method, file, line.
- ID: `relative/file.rb#Class#method`.
- Fallback: allow the user to paste a target manually.

Run:

- Unit/integration: `bin/rails test <file>:<line>` or `bin/rails test <file>`.
- System: `bin/rails test:system <file>` when the case is tagged `system`.
- Capture stdout/stderr/exit code.
- Parse verbose lines (`Class#test = .`) and the summary
  (`N runs, N assertions, N failures, N errors, N skips`).
- Strip ANSI codes.
- If parsing fails, the run is an `error` with the raw output as evidence.
- Do not require a Rails plugin or a new gem in v1.

## Cargo nextest (second)

- Discovery: `cargo nextest list --message-format json`.
- Run: `cargo nextest run --message-format json <targets>`.
- Parse the JSON events; store the raw output.
- This is the second driver because it is the same class of problem as Minitest
  and useful for Laburo itself.

## Cucumber / Selenium (later)

- Cucumber is a feature/scenario runner, not a Minitest runner. It needs a
  separate driver that discovers `.feature` files and scenarios and parses
  Cucumber JSON.
- Selenium is an implementation detail inside Rails system tests; do not model it
  as a separate driver in v1.
- Tier 1 only stores a Gherkin text block for reports. Tier 2 may add generation
  and a Cucumber driver.

## Fixture harness

`tests/fixtures/minitest/` contains:

- A fake `test/` tree with two test classes.
- Captured passing/failing output with ANSI codes.
- A golden JSON projection for Typst.

No real Rails app, database, or browser is required for unit/integration tests.

## Safety

- Never pass user text through a shell. Use `Command` with argv.
- Validate project paths and driver targets.
- Timeouts are mandatory.
- Do not store secrets in driver config; use environment variables.
