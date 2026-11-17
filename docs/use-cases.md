# Use cases

Status: draft for author confirmation
Last updated: 2026-11-17
Source: author's brief (2026-11-17). This supersedes the earlier read-only
"collector" framing in `requirements.md`.

## Product

Laburo is a local-first TQA suite for a solo developer working with AI. It
collapses the coordination surfaces of Bugzilla, TestRail, and Jira into a small
set of inspectable primitives: test cases, runs, results/evidence, defects,
requirements/use cases, traceability links, scenarios, and non-functional
checks. It executes the project's Minitest suite, records what happened, reports
it, and optionally uses AI to generate, triage, summarize, and maintain the
suite.

The scarce resource is coordination, not features. A solo developer plus AI does
not need roles, assignment queues, watchers, or workflow ceremony. It needs one
local source of truth for: what should be true, what was run, what happened,
what it means, and what to do next. Reports, dashboards, email, and AI are
projections over that source of truth.

## Tiers

- **Tier 1 — minimal full work.** The complete basic project. Meets the class's
  listed requirements; target 100/100 in the TQA project. Must be usable
  without AI.
- **Tier 2 — AI-assisted suite.** Tier 1 plus AI assistance and the extra
  coordination/quality primitives; target 100/100 in the class. Additive and
  behind interfaces. Tier 2 must degrade to Tier 1.

Tier rule: a Tier 2 feature may not be required by a Tier 1 flow.

## Final decisions (2026-11-17)

| Area | Decision |
|---|---|
| Store | SQLite only. No YAML in Laburo; `casos.yaml` is imported once. No JSON store. |
| Reports | Typst CLI, class-format PDFs; no AI needed to render. |
| Execution | Per-case `manual`, `single`, or `composed`; the user decides. One test = one step only in composed mode. |
| Evidence | Optional per step; status + notes is enough; files/screenshots attachable. |
| Tags | Free-form + seeded level/type/technique/quality tags; no hard-coded enum. |
| Drivers | Manual always; Minitest first; cargo-nextest second; Cucumber/Selenium later. |
| Gherkin | Text field on the case; rendered in reports; no parser in v0.1. |
| Email | Deferred behind a notifier interface; not on the v0.1 critical path. |
| MCP | Stdio server so an agent can automate Laburo end to end. |
| AI | Tier 2 quality safeguard: deterministic rules + local OpenAI-compatible (Ollama) ranking; human accepts. |
| Install | `cargo install --path . --locked`. |

## Actors

| Actor | Description |
|---|---|
| Solo dev | Primary human. Selects, runs, reviews, decides. |
| AI assistant | Uses the same core/CLI. Not a privileged UI. Proposes; the human accepts. |
| CI runner | Headless. Runs a saved profile and uploads artifacts. |
| Reader / jury | Consumes exported reports. Never operates the tool. |

## Primitives

| Primitive | What it is | Why it exists |
|---|---|---|
| `TestCase` | ID, title, type, level, priority, preconditions, steps, expected result, automation target, status, source | The unit of testing and the dictionary row. |
| `TestRun` | ID, command, commit, environment, start/end, results, artifacts | The record of one execution. |
| `TestResult` | case, status, duration, message, backtrace, evidence | What happened to one case. |
| `Evidence` | log, screenshot, metric, file, hash | Proof of a result. |
| `Defect` | ID, title, steps, expected/actual, severity, status, links | A reproducible deviation. |
| `Requirement` / `UseCase` | ID, title, source, acceptance | Why the test exists. |
| `TraceLink` | requirement/UC ↔ TC ↔ run ↔ defect | The matrix. |
| `Scenario` | Gherkin feature/scenario | Behavior in business language. |
| `NonFunctionalCheck` | type, tool, command, metrics, expected, result | The separate non-functional track. |
| `ProjectMemory` | indexed runs, defects, cases, decisions | Search and AI context. |

Rule: reports and views are projections. They never become a second source of
truth.

## Tier 1 use cases

Summary:

| ID | Use case | Actor | Depends on | Swarm fit |
|---|---|---|---|---|
| UC-01 | Catalog test cases | Solo dev | K1, K5 | Kernel + catalog pod |
| UC-02 | Select by type and run | Solo dev | UC-01, UC-03 | TUI + runner |
| UC-03 | Execute Minitest | Solo dev, CI | K4, UC-01 | Runner pod |
| UC-04 | Inspect a run and evidence | Solo dev | UC-03, UC-05 | TUI + storage |
| UC-05 | Keep run history | Solo dev, CI | K1 | Kernel |
| UC-06 | Generate a run report | Solo dev | UC-04, UC-05 | Report pod |
| UC-07 | Email the report | Solo dev | UC-06 | Notify pod (small) |
| UC-08 | Separate non-functional track | Solo dev | K5, UC-03 | Non-functional pod |
| UC-09 | Register a defect | Solo dev, AI | UC-04 | Defects pod |
| UC-10 | Trace requirements to tests and runs | Solo dev, jury | UC-01, UC-05 | Traceability pod |
| UC-11 | Run headless in CI | CI | UC-03, UC-06 | CI pod |
| UC-12 | Export the TQA dictionary and annex | Solo dev | UC-01, UC-05 | TQA pod |

### UC-01 — Catalog test cases

- **Tier:** 1
- **Actor:** Solo dev.
- **Goal:** One canonical catalog of at least 15 test cases, with enough detail
  to select, run, report, and trace them.
- **Main flow:**
  1. Import existing cases from `tqa/casos.yaml` and/or create cases in Laburo.
  2. Capture ID, title, type (functional/non-functional), level
     (unit/integration/system), priority, preconditions, steps, expected result,
     automation target, status, and source.
  3. Validate required fields and unique IDs.
  4. Save.
- **Acceptance:**
  - 15+ cases can be stored.
  - Duplicate IDs are rejected with the ID and file.
  - Cases are filterable by type, level, priority, and status.
  - YAML round-trip preserves author fields and comments, or refuses the write.
  - CLI list and TUI list agree.
- **Data:** `TestCase`.
- **Depends on:** K1, K5.
- **Swarm note:** catalog pod owns `src/catalog`; TUI can start with a read-only
  list.

### UC-02 — Select by type and run

- **Tier:** 1
- **Actor:** Solo dev; CI uses the CLI equivalent.
- **Goal:** Run a chosen subset without remembering Rails commands.
- **Main flow:**
  1. Filter the catalog by type, level, priority, and status.
  2. Multi-select cases.
  3. Preview the exact command and targets.
  4. Start the run; stream status per case.
- **Acceptance:**
  - Exactly the selected cases run.
  - Empty selection is rejected with a clear message.
  - The CLI can express the same selection (`--type`, `--level`, `--id`).
  - A selection can be saved as a named profile (optional, but useful for CI).
- **Data:** `TestCase`, selection profile.
- **Depends on:** UC-01, UC-03.
- **Swarm note:** TUI owns selection; runner owns execution. Contract is a
  `Selection` value object.

### UC-03 — Execute Minitest

- **Tier:** 1
- **Actor:** Solo dev; CI.
- **Goal:** Execute selected Minitest tests and normalize the results.
- **Main flow:**
  1. Map each `TestCase` to a Minitest target (file, file:line, or name).
  2. Invoke `bin/rails test` and/or `bin/rails test:system` for the selection.
  3. Capture stdout, stderr, exit code, duration, and environment.
  4. Parse results into one `TestResult` per test.
  5. Link results back to cases.
- **Acceptance:**
  - One result per executed test, with status and duration.
  - Failures keep the message and backtrace.
  - A timeout is a recorded error, not a hang.
  - A missing target is an error naming the case and target.
  - Minitest remains the runner; Laburo does not reimplement it.
- **Data:** `TestRun`, `TestResult`, `Evidence`.
- **Depends on:** K4, UC-01.
- **Swarm note:** this is the highest technical risk. Spike it before the TUI.

### UC-04 — Inspect a run and evidence

- **Tier:** 1
- **Actor:** Solo dev.
- **Goal:** Understand a run quickly and decide what to fix.
- **Main flow:**
  1. Open a run.
  2. See counts and the per-case list.
  3. Filter by status.
  4. Open a failure with message, backtrace, and evidence.
  5. Mark evidence as reviewed (optional).
- **Acceptance:**
  - Pass/fail/skip/error counts match the results.
  - Failure details are readable without leaving the TUI.
  - Evidence links open the file or show the path.
  - Keyboard-only navigation.
- **Data:** `TestRun`, `TestResult`, `Evidence`.
- **Depends on:** UC-03, UC-05.
- **Swarm note:** TUI pod; no business rules in the view.

### UC-05 — Keep run history

- **Tier:** 1
- **Actor:** Solo dev; CI.
- **Goal:** Audit and compare runs.
- **Main flow:**
  1. Persist every run with metadata and results.
  2. List runs in deterministic order.
  3. Compare two runs.
- **Acceptance:**
  - History is append-only.
  - Each run has command, commit, environment, and timestamps.
  - Duplicate run IDs are impossible.
  - A run can be loaded without the Rails app.
- **Data:** `TestRun`, `TestResult`.
- **Depends on:** K1.
- **Swarm note:** kernel; storage choice is a shared contract.

### UC-06 — Generate a run report

- **Tier:** 1
- **Actor:** Solo dev.
- **Goal:** Produce the report the class and the thesis need.
- **Main flow:**
  1. Choose one or more runs.
  2. Render an HTML report and a PDF (Typst first).
  3. Include summary, functional section, non-functional section, failures,
     evidence, and traceability.
  4. Save both artifacts.
- **Acceptance:**
  - Works offline.
  - Same data produces the same report (stable ordering).
  - HTML opens in a browser; PDF compiles with Typst.
  - Evidence links resolve.
  - Non-functional results are not mixed into the functional section.
- **Data:** `TestRun`, `TestResult`, `Evidence`, `NonFunctionalCheck`,
  `TraceLink`.
- **Depends on:** UC-04, UC-05.
- **Swarm note:** report pod; template is a shared asset.

### UC-07 — Email the report (deferred)

- **Tier:** 2 — behind a notifier interface; not on the v0.1 critical path.
- **Actor:** Solo dev.
- **Goal:** Send the report without manual copying.
- **Main flow:**
  1. Configure SMTP from environment/secrets.
  2. Choose recipients and attach the report.
  3. Send; record delivery or failure.
- **Acceptance:**
  - A dry-run mode prints what would be sent.
  - Secrets never appear in logs or the repo.
  - Failure names the SMTP stage and is actionable.
  - The sent artifact is the same file that was generated.
- **Data:** report artifact, delivery record.
- **Depends on:** UC-06.
- **Swarm note:** small, independent pod; can land early with a fake notifier.

### UC-08 — Separate non-functional track

- **Tier:** 1
- **Actor:** Solo dev.
- **Goal:** Keep functional and non-functional tests separate, as the class
  requires.
- **Main flow:**
  1. Switch to the non-functional tab.
  2. Define a check: type (load/stress/performance/security/compatibility/
     recovery/usability/…), tool, command, metrics, expected result.
  3. Run it through an external command or record a manual result.
  4. Attach metrics and evidence.
  5. Include it in the report under its own section.
- **Acceptance:**
  - Separate data model and tab.
  - At least one executable adapter and one manual recording path.
  - Metrics and evidence are stored.
  - The report has a separate non-functional section.
- **Data:** `NonFunctionalCheck`, `TestResult`, `Evidence`.
- **Depends on:** K5, UC-03.
- **Swarm note:** non-functional pod; adapters can be added one at a time.

### UC-09 — Register a defect

- **Tier:** 1
- **Actor:** Solo dev; AI may draft, human accepts.
- **Goal:** Turn a failure or observation into a reproducible defect record.
- **Main flow:**
  1. Start from a failed result or create manually.
  2. Capture ID, title, steps, expected/actual, severity, evidence.
  3. Link the case and run.
  4. Move status (open/resolved/closed).
- **Acceptance:**
  - The class's bug fields are present.
  - A defect links to at least one case or run.
  - No assignment queues or roles; the solo dev is the only owner.
  - Defects are searchable.
- **Data:** `Defect`, `TraceLink`, `Evidence`.
- **Depends on:** UC-04.
- **Swarm note:** defects pod; the lifecycle is deliberately small.

### UC-10 — Trace requirements to tests and runs

- **Tier:** 1
- **Actor:** Solo dev; jury reads the export.
- **Goal:** Show that requirements and use cases are covered.
- **Main flow:**
  1. Import requirements/use cases.
  2. Link them to test cases.
  3. Show the matrix with last run status.
  4. Flag missing links or failing coverage.
  5. Export the matrix into the report.
- **Acceptance:**
  - Each requirement/use case shows its test cases and last status.
  - Missing links are visible, not silent.
  - The matrix is exportable.
- **Data:** `Requirement`, `UseCase`, `TraceLink`, `TestRun`.
- **Depends on:** UC-01, UC-05.
- **Swarm note:** traceability pod; read-mostly and parallel-friendly.

### UC-11 — Run headless in CI

- **Tier:** 1
- **Actor:** CI runner.
- **Goal:** Run the same path without a TUI and keep the evidence.
- **Main flow:**
  1. Read a saved profile or flags.
  2. Execute the selection.
  3. Generate the report.
  4. Exit non-zero on failures.
  5. Upload report and evidence as artifacts.
- **Acceptance:**
  - No interactive prompts.
  - GitHub Actions workflow exists and passes.
  - Artifacts include the report and evidence.
  - The run is recorded in history.
- **Data:** profile, `TestRun`, report artifacts.
- **Depends on:** UC-03, UC-06.
- **Swarm note:** CI pod; can work against a fixture app before the real one.

### UC-12 — Export the TQA dictionary and reports

- **Tier:** 1
- **Actor:** Solo dev.
- **Goal:** Produce the class-format documents from Laburo data.
- **Main flow:**
  1. Import `casos.yaml` into SQLite once (read-only input).
  2. Render the dictionary, case dossier, run sheet, and bug report with Typst.
  3. Keep `.typ` + JSON projection next to each PDF.
- **Acceptance:**
  - Import handles the class dictionary schema.
  - All four report kinds compile with the installed `typst`.
  - No YAML is written by Laburo.
  - A failed compile reports Typst's output.
- **Data:** `TestCase`, `TestRun`, `Defect`, `Requirement`.
- **Depends on:** UC-01, UC-05, UC-06.
- **Swarm note:** TQA/report pod; `casos.yaml` is an input only.

## Tier 2 use cases

Summary:

| ID | Use case | Actor | Depends on | Swarm fit |
|---|---|---|---|---|
| UC-13 | Generate Gherkin from a use case | Solo dev + AI | UC-01, AI provider | AI pod |
| UC-14 | Draft test cases from a requirement | Solo dev + AI | UC-01, AI provider | AI pod |
| UC-15 | Triage a failed run | Solo dev + AI | UC-03, UC-09, AI provider | AI pod |
| UC-16 | Summarize a run for humans | Solo dev + AI | UC-06, AI provider | AI pod |
| UC-17 | Search project memory | Solo dev + AI | UC-05, UC-09, AI provider | AI pod |
| UC-18 | Quality dashboard | Solo dev, jury | UC-05, UC-09 | Metrics pod |
| UC-19 | Maintain tests (flaky/stale) | Solo dev + AI | UC-05, AI provider | AI pod |
| UC-20 | Sync with GitHub/GitLab/Slack | Solo dev | UC-09, config | Integrations pod |
| UC-21 | Non-functional execution adapters | Solo dev | UC-08 | Non-functional pod |
| UC-22 | Plugin API for runners/adapters | Developer/agent | Architecture | Kernel/extension pod |

### UC-13 — Generate Gherkin from a use case

- **Tier:** 2
- **Goal:** Turn a use case or test case into Gherkin for review.
- **Acceptance:** Valid Feature/Scenario/Given/When/Then; human review before
  saving; scenarios link to their source; no auto-execution without approval.
- **Depends on:** UC-01, AI provider.

### UC-14 — Draft test cases from a requirement

- **Tier:** 2
- **Goal:** Draft candidate test cases from a requirement or use case.
- **Acceptance:** Candidates enter the catalog as `proposed`; no auto-accept;
  duplicates are detected; each candidate links to its source.
- **Depends on:** UC-01, AI provider.

### UC-15 — Triage a failed run

- **Tier:** 2
- **Goal:** Cluster failures and propose cause, severity, and next action.
- **Acceptance:** Uses the run's actual logs; cites the failing case and
  evidence; proposes a defect but never closes one; human approval required.
- **Depends on:** UC-03, UC-09, AI provider.

### UC-16 — Summarize a run for humans

- **Tier:** 2
- **Goal:** Add a readable narrative to a report.
- **Acceptance:** Every number comes from the run; the summary is editable; the
  report still renders without AI.
- **Depends on:** UC-06, AI provider.

### UC-17 — Search project memory

- **Tier:** 2
- **Goal:** Ask questions across runs, defects, cases, and decisions.
- **Acceptance:** Local index; answers cite their records; no invented facts;
  works offline or with a configured provider.
- **Depends on:** UC-05, UC-09, AI provider.

### UC-18 — Quality dashboard

- **Tier:** 2
- **Goal:** Show pass rate, coverage, flaky tests, defect density, and trends.
- **Acceptance:** Computed from stored data; time range selectable; exportable;
  no invented numbers.
- **Depends on:** UC-05, UC-09.

### UC-19 — Maintain tests

- **Tier:** 2
- **Goal:** Flag flaky or stale tests and propose updates.
- **Acceptance:** Evidence from history; proposals are reviewed; no auto-merge.
- **Depends on:** UC-05, AI provider.

### UC-20 — Sync with GitHub/GitLab/Slack

- **Tier:** 2
- **Goal:** Mirror defects and run summaries into the tools a small team already
  uses.
- **Acceptance:** Idempotent export/import; secrets outside the repo; offline
  fallback; links preserved.
- **Depends on:** UC-09, configuration.

### UC-21 — Non-functional execution adapters

- **Tier:** 2
- **Goal:** Run load, security, or compatibility tools and import metrics.
- **Acceptance:** At least one real adapter; metrics and evidence stored;
  isolated from the functional path.
- **Depends on:** UC-08.

### UC-22 — Plugin API for runners and adapters

- **Tier:** 2
- **Goal:** Add a new runner (e.g. `cargo test`) or report format without editing
  the core.
- **Acceptance:** Stable traits; one example plugin; tests; docs.
- **Depends on:** architecture.

### UC-23 — MCP automation surface

- **Tier:** 2 (foundation built in this wave)
- **Actor:** AI assistant / external agent.
- **Goal:** Operate Laburo end to end over stdio MCP: discover tests, create
  cases, run them, record evidence, file bugs, generate reports, query quality.
- **Acceptance:** `initialize`, `tools/list`, `tools/call`; at least list/create
  case, run case, create bug, generate report; local only; no shell injection.
- **Depends on:** core library, CLI, repo, reports.
- **Swarm note:** MCP pod; tools call the library, not the CLI.

### UC-24 — Cargo-nextest driver

- **Tier:** 2
- **Goal:** Use the same driver/report model for Rust projects.
- **Acceptance:** `cargo nextest list --message-format json` discovery; run and
  parse outcomes; fixture tests.
- **Depends on:** driver trait, UC-03.

### UC-25 — Quality decision engine

- **Tier:** 2
- **Goal:** Act as a quality safeguard: deterministic rules find weak spots; a
  local OpenAI-compatible model ranks and proposes the next action; the human
  accepts.
- **Acceptance:** rules run with AI disabled; findings cite records; model
  failure falls back to deterministic ranking; no direct writes.
- **Depends on:** UC-05, UC-09, UC-18, AI provider.

## Build order

1. **Wave 0:** docs, contracts, GOAL, migration, skeleton.
2. **Wave 1 (kernel):** DB/repositories, CLI/config, TUI shell, driver trait +
   Minitest fixture, report projection/templates.
3. **Wave 2 (Tier 1):** UC-01/03/05/04/02, then UC-06/08/09/10/11/12 and TUI.
4. **Wave 3 (Tier 2):** UC-23 MCP, UC-25 quality rules, UC-13 Gherkin text,
   UC-24 nextest, UC-18 dashboard. UC-07 email and Cucumber remain later.

## Resolved decisions

All previous open questions are resolved by `docs/architecture.md`,
`docs/data-model.md`, `docs/execution-model.md`, `docs/drivers.md`,
`docs/reports.md`, and `docs/mcp.md`. Remaining deferrals: email (UC-07),
Cucumber/Selenium, remote integrations. Nothing blocks Wave 1.
