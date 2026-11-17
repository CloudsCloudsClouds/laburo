# Status

Live board. WIP limit: 2. One work unit per agent.

Last updated: 2026-11-17 (Wave 0 complete)

## Current phase

**Wave 0 — contracts: done.** Docs, data model, execution/driver/report/TUI/MCP
contracts, `GOAL.md`, migration, and skeleton are ready.

**Next: `/goal` → Wave 1 (kernel).**

## Wave 1 work units

| ID | Work unit | Owns | Depends on |
|---|---|---|---|
| K1 | SQLite open/migrations/repositories | `src/db.rs`, `src/repo/` | migration |
| K2 | CLI shell, config, project resolution | `src/cli.rs`, `src/config.rs` | K1 |
| K3 | TUI shell + keymap | `src/tui/` | K1/K2 |
| K4 | Driver trait, manual driver, Minitest fixture | `src/driver/`, `tests/fixtures/` | model |
| K5 | Report projection + Typst templates | `src/report/`, `templates/typst/` | K1 |

## Wave 2 (after kernel)

Cases CRUD/import; manual runs/evidence; Minitest execution; bugs; four Typst
reports; TUI views; backup/install/CI; end-to-end fixture.

## Wave 3 (Tier 2)

Quality rules; OpenAI-compatible provider; MCP server; nextest boundary; Gherkin
rendering; dashboard.

## Blocked / author decisions

- None blocking. Report formats: Typst PDF (decided).
- Email: deferred behind a notifier interface.
- Cucumber/Selenium: later; Gherkin text only in v0.1.

## Done

- 2026-11-17 — Wave 0: repo, docs, contracts, goal, migration, skeleton.
