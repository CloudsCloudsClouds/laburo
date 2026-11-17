# Roadmap

Status: v0.1 wave plan
Last updated: 2026-11-17

## Wave 0 — contracts (done)

Docs, data model, execution model, report/driver/TUI/MCP contracts, GOAL,
migration, skeleton.

## Wave 1 — kernel

| Unit | Deliverable | Acceptance |
|---|---|---|
| K1 | SQLite open/migrations/repositories | CRUD tests on a temp DB; foreign keys and cascades work. |
| K2 | CLI shell, config, project resolution, install | `laburo doctor`, `init`, `--help`; `cargo install --path .` works. |
| K3 | TUI shell | Launches; tabs, list/detail, keymap; TestBackend snapshot. |
| K4 | Driver trait + manual + Minitest fixture | Fixture discovery and pass/fail parse. |
| K5 | Report projection + Typst templates | Golden JSON; four PDFs compile. |

## Wave 2 — Tier 1

- Case CRUD + tags + import `casos.yaml`.
- Manual run with per-step status, notes, optional evidence.
- Minitest targeted execution for unit/integration/system.
- Run history and step results.
- Bug CRUD linked to case/run/step.
- Reports: dictionary, case dossier, run sheet, bug report.
- TUI views: cases, runs, bugs, reports, drivers, dashboard.
- Backup/restore, install script, CI, end-to-end fixture test.

## Wave 3 — Tier 2

- Deterministic quality rules + quality view.
- OpenAI-compatible provider + Ollama config; no network in tests.
- MCP stdio server with tools for cases/runs/bugs/reports/quality.
- Cargo-nextest driver boundary (real implementation if time).
- Gherkin text field and report rendering; generation later.
- Dashboard metrics and project memory (FTS5) if time.

## Later

- Cucumber driver, Selenium/system-test integration.
- Email notifier (behind interface; not critical).
- GitHub/GitLab/Slack integrations.
- Plugin API for third-party drivers/reports.
