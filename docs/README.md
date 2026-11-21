# Documentation

Docs are the spec. Code follows them; the author owns direction.

## Index

| Document | What it answers |
|---|---|
| [`../GOAL-v0.2.md`](../GOAL-v0.2.md) | The current v0.2 objective and acceptance criteria. |
| [`v0.2-contracts.md`](v0.2-contracts.md) | Frozen v0.2 schema, repo API, web routes, email, MCP gate. |
| [`../GOAL.md`](../GOAL.md) | Historical v0.1 objective. |
| [`vision.md`](vision.md) | Why Laburo exists, for whom, and what it refuses to be. |
| [`use-cases.md`](use-cases.md) | Tier 1 and Tier 2 use cases, actors, primitives, acceptance. |
| [`architecture.md`](architecture.md) | Components, boundaries, data flow, key decisions. |
| [`data-model.md`](data-model.md) | SQLite tables, statuses, tags, migrations. |
| [`execution-model.md`](execution-model.md) | Manual, single, composed; steps, evidence, discovery. |
| [`drivers.md`](drivers.md) | Driver trait, manual, Minitest, cargo-nextest, Cucumber later. |
| [`reports.md`](reports.md) | Typst templates, JSON projection, outputs. |
| [`tui.md`](tui.md) | Layout, views, keymap, forms, async. |
| [`ai.md`](ai.md) | Tier 2 quality engine, rules, Ollama/OpenAI-compatible provider. |
| [`mcp.md`](mcp.md) | MCP stdio server tools for agent automation. |
| [`install.md`](install.md) | Install, runtime dirs, config, backup, CI. |
| [`roadmap.md`](roadmap.md) | Wave plan and later work. |
| [`status.md`](status.md) | The live board. |
| [`swarm-plan.md`](swarm-plan.md) | Historical swarm plan; wave 0 supersedes it. |
| [`requirements.md`](requirements.md) | Historical read-only framing. |
| [`decisions/`](decisions/) | ADRs. |

## Rules

- One fact, one home. Link instead of copying.
- Use-case IDs (`UC-…`) and work-unit IDs (`K1`, `T1-…`) are stable.
- `status.md` is the only live board.
- ADRs are append-only.
- Mark assumptions and unverified claims explicitly.
- SQLite is the source of truth; JSON is only a transient report projection; no
  YAML in Laburo.
