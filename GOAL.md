# GOAL — Laburo v0.1

Status: ready for `/goal`
Last updated: 2026-11-17

## Objective

Build, install, and verify **Laburo v0.1**: a local-first TQA suite for a solo
developer that manages test cases and runs, executes manual and Minitest-backed
tests, records evidence, files bug reports, exports the class-format Typst PDFs,
and exposes an MCP server so an agent can operate it end to end. Tier 1 must be
complete; Tier 2 includes the deterministic quality engine and a local
OpenAI-compatible (Ollama) provider hook.

(User note, mind the time too pls. At most, one hour and a half now - or on 2026-11-17, until 19:30 at most. If 19:30 comes, just stop. Or stop if you finish early, whichever comes first.)

## Scope

Tier 1:

- SQLite store with migrations, cases, steps, tags, runs, step results,
  evidence, bugs, drivers, requirements, traceability.
- CLI: `doctor`, `init`, `case`, `run`, `bug`, `report`, `driver`, `import`,
  `db`, `mcp`, `tui`.
- TUI: cases, runs, bugs, reports, drivers, dashboard; manual run flow; create/
  edit forms; vim motions.
- Minitest driver: discover tests from `test/**/*_test.rb`, run selected tests,
  parse verbose output and summary.
- Manual runs: per-step pass/fail/blocked, notes, optional evidence.
- Automated modes: `single` and `composed`.
- Typst reports: dictionary, case dossier, run sheet, bug report.
- Import from `casos.yaml` into SQLite (one-way; no YAML output).
- Install with `cargo install --path . --locked`.

Tier 2 (this wave, foundation only):

- Deterministic quality rules + findings.
- OpenAI-compatible provider interface with Ollama-compatible config.
- MCP server tools for agent automation.
- Cargo-nextest driver stub/boundary.
- Gherkin text field and report rendering; no parser.

## Acceptance criteria

- [x] `cargo fmt --all --check` passes.
- [x] `cargo clippy --all-targets -- -D warnings` passes.
- [x] `cargo test --all-targets` passes (48 tests).
- [x] `cargo install --path . --locked` succeeds and `laburo doctor` works.
- [x] `laburo init` creates `.laburo/laburo.sqlite3` and applies migrations.
- [x] Case CRUD works from CLI and TUI.
- [x] Manual run with three steps, one failure, notes, and one evidence file is
      persisted and readable.
- [x] A bug can be created from the failed step, linked to the case/run/step.
- [x] Minitest fixture tests are discovered and a passing/failing result is
      parsed correctly.
- [x] `laburo report dictionary`, `case`, `run`, and `bug` produce PDFs with the
      installed `typst` binary.
- [x] `laburo import casos.yaml` imports the class dictionary into SQLite
      (real dictionary: 21 cases).
- [x] `laburo tui` launches against the fixture DB (TestBackend test + real
      pseudo-terminal launch, exit 0).
- [x] `laburo mcp` responds to `initialize`, `tools/list`, and `tools/call` for
      list/create/run/report.
- [x] Quality rules produce deterministic findings without AI.
- [x] AI provider is disabled by default; config points to Ollama; no network in
      tests.
- [x] `docs/status.md` reflects the final state; no YAML store, no JSON store.

## Verification

```bash
cd laburo
cargo fmt --all --check
cargo clippy --all-targets -- -D warnings
cargo test --all-targets
cargo install --path . --locked
laburo doctor
# fixture E2E
laburo init --project tests/fixtures/project
laburo case create --id TC-001 --title "Fixture case" --tag unit
laburo run manual TC-001 --step-status passed,failed --note "step 2 broke" \
  --evidence tests/fixtures/evidence/log.txt
laburo bug create --case TC-001 --title "Fixture bug" --severity 1
laburo report dictionary
laburo report case TC-001
laburo report bug BUG-...
printf '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}\n' | laburo mcp
```

## Non-goals this wave

- Full Cucumber/Selenium execution.
- Email delivery.
- Web UI.
- Multi-user roles or assignment queues.
- Remote sync/push.
- Full AI generation pipeline; only the provider boundary and deterministic
  quality rules are required.

## Swarm waves

- **Wave 0 (done):** docs, contracts, objective, migration, skeleton.
- **Wave 1 (kernel):** DB/repositories, CLI/config/import, TUI shell, driver
  trait + Minitest fixture, Typst projection/templates.
- **Wave 2 (Tier 1):** case/run/bug/report/TUI pods, install/CI, end-to-end.
- **Wave 3 (Tier 2):** quality rules, AI provider, MCP tools, nextest boundary,
  Gherkin rendering, dashboard.

Coordination: one work unit per agent, disjoint files, local branches/worktrees
allowed, no pushes, coordinator reviews and merges. Handoff format in
`AGENTS.md`.
