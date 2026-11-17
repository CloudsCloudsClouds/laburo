# Status

Live board. WIP limit: 3. One work unit per agent.

Last updated: 2026-11-17 18:45

## Phase

**v0.1 Tier 1 complete; Tier 2 foundation complete.** Installed and verified.

## Verified evidence

```text
cargo fmt --all --check              clean
cargo clippy --all-targets -- -D warnings
                                     clean
cargo test --all-targets             48 passed, 0 failed
cargo install --path . --locked      installed laburo 0.1.0
laburo doctor                        Rails/Minitest/SQLite/Typst/nextest
laburo driver discover               2 fixture tests discovered
laburo run auto TC-FIX --target ...  single run passed
laburo tui (pseudo-terminal)         exit 0
laburo mcp initialize/dashboard      valid JSON-RPC results
```

Real flows:

- `/tmp/opencode/laburo-demo` — manual case with 3 steps, 1 failure, notes,
  evidence, bug, and four PDFs (dictionary/case/run/bug).
- `/tmp/opencode/laburo-fixture` — fake Rails app: doctor, Minitest discovery,
  automated single run, MCP dashboard.
- Real import from `src-elizabeth-tours/tqa/casos.yaml`: **21 test cases**.

## What shipped

- SQLite schema, migrations, repositories (cases, steps, tags, runs, step
  results, evidence, bugs, drivers, requirements, traceability).
- CLI: `init`, `doctor`, `case`, `run`, `bug`, `report`, `driver`, `import`,
  `db`, `quality`, `mcp`, `tui`.
- TUI: Cases, Runs, Bugs, Reports, Drivers, Dashboard; manual run flow; case
  create/edit/delete; vim motions; TestBackend test.
- Minitest driver: recursive discovery, targeted execution, ANSI stripping,
  verbose/summary parsing, timeout.
- Typst reports: class-format dictionary, case dossier, run sheet, bug report;
  evidence images/logs.
- `casos.yaml` one-way import into SQLite.
- MCP stdio server: initialize, tools/list, tools/call (13 tools).
- Deterministic quality rules + AI boundary (OpenAI-compatible/Ollama,
  disabled by default).
- Install/doctor, Makefile, CI with Typst, per-pod build helper with sccache.

## Known gaps / deferred

- Email notifier is deferred (behind the notifier boundary).
- Cucumber/Selenium drivers are deferred; Gherkin is stored/rendered, not run.
- Cargo-nextest driver is a boundary/stub, not a real runner.
- TUI Drivers tab is read-only; automated runs and bug creation are CLI/MCP.
- AI network path (live Ollama/OpenAI call) is not exercised in tests.
- No background polling in the TUI; refresh happens on tab/mutation.

## Deadline

Stop by 2026-11-17 19:30. All acceptance criteria are met before the deadline.
