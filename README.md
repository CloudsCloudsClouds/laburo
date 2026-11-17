# Laburo

A local-first TQA suite for a solo developer working with AI.

Laburo manages test cases and runs, executes manual and Minitest-backed tests,
records evidence, files bug reports, exports the class-format Typst PDFs, and
exposes an MCP server so an agent can operate it end to end. The TUI is the main
interface; the CLI is the core and the CI/MCP surface.

> **Status: v0.1 in progress.** The contracts are frozen in `docs/`; the swarm
> builds against them. See [`GOAL.md`](GOAL.md) and
> [`docs/status.md`](docs/status.md).

## Install

```bash
cd laburo
cargo install --path . --locked
laburo doctor
```

## Quick start

```bash
make install                 # cargo install --path . --locked
laburo init --project ../src-elizabeth-tours
laburo case create --id TC-001 --title "Booking happy path" --tag integration
laburo run manual TC-001
laburo bug create --case TC-001 --title "Booking fails" --severity 1
laburo report dictionary
laburo tui
laburo mcp
```

For development:

```bash
make test                    # cargo test --all-targets
make test-fast               # cargo nextest run --all-targets
make e2e                     # fixture flow: driver + four PDFs + MCP
scripts/pod.sh tui check     # per-pod target dir + sccache, for parallel agents
```

## Documentation

- [`GOAL.md`](GOAL.md) — objective and acceptance criteria.
- [`docs/architecture.md`](docs/architecture.md) — components and boundaries.
- [`docs/data-model.md`](docs/data-model.md) — SQLite schema.
- [`docs/execution-model.md`](docs/execution-model.md) — manual, single, composed.
- [`docs/drivers.md`](docs/drivers.md) — manual, Minitest, nextest.
- [`docs/reports.md`](docs/reports.md) — Typst projections and templates.
- [`docs/tui.md`](docs/tui.md) — layout and keymap.
- [`docs/ai.md`](docs/ai.md) — Tier 2 quality engine.
- [`docs/mcp.md`](docs/mcp.md) — agent automation surface.
- [`docs/install.md`](docs/install.md) — runtime dirs, config, backup.
- [`docs/use-cases.md`](docs/use-cases.md) — Tier 1 and Tier 2 use cases.
- [`docs/status.md`](docs/status.md) — live board.

## Principles

- SQLite is the only source of truth. JSON only as a transient report projection;
  no YAML in Laburo.
- Typst is invoked as a CLI; no Typst crate.
- The library owns behavior; TUI/CLI/MCP are thin.
- Evidence is optional; status + notes is a valid result.
- Tags, not hard-coded test-type enums.
- Minitest first; cargo-nextest second; Cucumber/Selenium later.
- Tier 2 AI is a quality safeguard: rules propose, the model ranks, the human
  accepts.

## Provenance

Laburo is its own git repository inside the Elizabeth Tours thesis workspace.
The thesis root ignores `laburo/`. The wiki note is
`ElizToursWiki/Software/Laburo.md`.
