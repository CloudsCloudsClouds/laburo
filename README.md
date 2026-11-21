# Laburo

A local-first TQA suite for a solo developer working with AI.

Laburo manages test cases and runs, executes manual and Minitest-backed tests,
records evidence, files bug reports, exports the class-format Typst PDFs, and
exposes an MCP server so an agent can operate it end to end. The TUI is the main
interface; the CLI is the core and the CI/MCP surface.

> **Status: v0.2 (Demo Edition) in progress.** The v0.1 core is installed; the
> v0.2 wave adds the web UI, evidence graph, class-format reports, `.eml` email,
> and the MCP quality gate. See [`GOAL-v0.2.md`](GOAL-v0.2.md) and
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
./scripts/demo-seed.sh       # reset/seed src-elizabeth-tours/.laburo
laburo web --port 3000       # the demo centerpiece
laburo case list --tag smoke
laburo run batch --tag smoke --mode auto
laburo report dictionary
laburo email send --to demo@example.com --subject "TQA report" --report <pdf>
laburo mcp                   # agent surface
```

## MCP quality contract (for coding swarms)

A coding agent should check in with Laburo before and after a change:

1. `laburo_check_in` with a feature, case, or tag: relevant cases, mapped/
   unmapped coverage, last results, evidence gaps, open bugs, next actions, and
   `pass`/`warn`/`fail`.
2. Run the mapped cases; record evidence/artifacts.
3. File a bug on failure, linked to the reproducing case and run.
4. Re-run the check-in until it is `pass` or the remaining warnings are known.

The AI orchestrates; Laburo remembers the quality truth. No model call is
required for the gate.

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
