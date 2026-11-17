# Architecture

Status: accepted for the wave; author may veto details
Last updated: 2026-11-17

## Product

Laburo is a local-first TQA suite for a solo developer working with AI. It
collapses Bugzilla/TestRail/Jira coordination into a small set of primitives:
cases, steps, runs, results, evidence, bugs, requirements, traceability, tags,
and reports. The TUI is the main interface; the CLI is the core and the CI/MCP
surface.

## Components

```text
TUI (ratatui) ─┐
CLI (clap) ────┼──> core library ──> SQLite
MCP (stdio) ───┘        │
                        ├──> drivers: manual, minitest, cargo-nextest
                        ├──> reports: projection -> Typst CLI -> PDF
                        └──> quality: rules + optional local model (Tier 2)
```

The library owns all behavior. TUI, CLI, and MCP are thin adapters. Reports are
projections. SQLite is the only source of truth.

## Repository layout

| Path | Responsibility |
|---|---|
| `src/lib.rs` | Public library surface; module wiring. |
| `src/model.rs` | Domain structs and enums. |
| `src/db.rs` | Connection, pragmas, migrations, transaction helpers. |
| `src/repo/` | CRUD and queries per aggregate (`cases`, `runs`, `bugs`, `drivers`, `reports`). |
| `src/driver/` | Driver trait, manual driver, Minitest driver, nextest driver. |
| `src/report/` | SQLite -> report projection -> Typst invocation. |
| `src/ai/` | Quality rules, provider trait, OpenAI-compatible client, Ollama config. |
| `src/mcp.rs` | MCP stdio server exposing the core to agents. |
| `src/cli.rs` | Commands and flags. |
| `src/tui/` | Ratatui app, views, forms, keymap. |
| `migrations/` | SQL migrations, applied by `PRAGMA user_version`. |
| `templates/typst/` | Active fillable templates. |
| `tests/` | Integration tests, fixtures, golden output. |
| `.laburo/` | Local runtime data (DB, evidence, reports, temp). Never committed. |

## Data flow

1. The user creates a case with steps and tags.
2. A run is created in `manual`, `single`, or `composed` mode.
3. Manual steps are marked by the user; automated steps call a driver.
4. Results, notes, and optional evidence are written to SQLite.
5. A failed step can produce a bug report linked to the case/run/step.
6. Reports read SQLite, write a transient JSON projection, and invoke the
   installed `typst` binary.
7. The MCP server exposes the same operations to an agent.

## Key decisions

- **SQLite** (`rusqlite`, bundled): relational data, transactions, foreign keys,
  history, filters, evidence links, migrations. No JSON state store.
- **Typst CLI**: the binary is installed; do not add a Rust Typst crate. The
  templates stay human-editable.
- **No YAML in Laburo**: `casos.yaml` is imported once into SQLite. No YAML
  output. JSON is only a transient Typst projection.
- **Per-case execution mode**: manual, single automated, or composed automated.
  The user decides; integration tests are not forced into unit-test steps.
- **Evidence optional**: status + notes is a valid result; files/screenshots are
  attachable, copied into `.laburo/evidence/`, hashed.
- **Tags, not enums**: level, type, technique, quality categories are seeded
  tags; free-form tags are allowed.
- **Minitest first**: manual driver always; Minitest is the first real driver;
  cargo-nextest is second; Cucumber/Selenium are later plugins.
- **MCP server**: stdio JSON-RPC server that exposes the core so an agent can
  operate Laburo end to end.
- **AI is Tier 2**: deterministic rules produce candidate findings; a local
  OpenAI-compatible model (Ollama) ranks/chooses; the human accepts. No direct
  writes from the model.

## Non-goals

- Replacing Minitest/Rails, CI, or the thesis document.
- A web UI.
- Full Cucumber/Selenium drivers in this wave.
- Email on the critical path (behind a notifier interface, last).
- Multi-user roles, watchers, assignment queues. Solo dev + AI only.

## Verification

- `cargo fmt --all --check`
- `cargo clippy --all-targets -- -D warnings`
- `cargo test --all-targets`
- `laburo doctor`
- A fixture-backed end-to-end flow: create case -> run manual -> create bug ->
  generate Typst PDFs.
- MCP smoke test over stdio.
