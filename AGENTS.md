# AGENTS.md — operating contract

This file is for any agent working in `laburo/`. Read it before touching code.

## Read first

1. `GOAL.md` — the objective and acceptance criteria.
2. `README.md`
3. `docs/README.md`
4. `docs/use-cases.md`
5. `docs/architecture.md`
6. `docs/data-model.md`
7. `docs/execution-model.md`
8. `docs/drivers.md`
9. `docs/reports.md`
10. `docs/tui.md`
11. `docs/ai.md`
12. `docs/mcp.md`
13. `docs/status.md` — the live board.

The thesis-level contract still applies only where relevant: `../MODELS.md` and
`../ElizToursWiki/Operandi.md`. Laburo is its own repo; the thesis root stays
untouched.

## Stance

State your stance before working: **superior** (holds the standard, blocks weak
work), **coworker** (plans and reasons), or **junior** (executes a mechanical
task). The author owns direction, domain facts, numbers, and architecture
tradeoffs.

## The loop

1. Take exactly one work unit from `docs/status.md`.
2. Read the relevant docs and the frozen contracts.
3. Write or update the acceptance test first.
4. Implement the smallest thing that passes.
5. Run `cargo fmt --all`, `cargo clippy --all-targets -- -D warnings`,
   `cargo test --all-targets`.
6. Update your handoff note and `docs/status.md`.
7. If you made a real decision, add an ADR under `docs/decisions/`.

If the docs and code disagree, the docs win until the author says otherwise.

## Boundaries

| Area | Owner rule |
|---|---|
| `src/model.rs`, `migrations/`, `Cargo.toml`, `src/lib.rs`, `src/main.rs` | Coordinator only. Do not edit without assignment. |
| `src/db.rs`, `src/repo/` | DB pod. |
| `src/driver/` | Driver pod. |
| `src/report/`, `templates/typst/` | Report pod. |
| `src/tui/` | TUI pod. Split by view files when parallel. |
| `src/ai/` | AI pod. |
| `src/mcp.rs` | MCP pod. |
| `src/cli.rs`, `src/config.rs`, `src/import.rs` | CLI pod. |
| `docs/` | Coordinator; agents may append handoff notes. |

Rules:

- One agent per file/directory at a time. Announce it in `docs/status.md`.
- Do not edit another agent's uncommitted work.
- Local branches/commits/worktrees are allowed for coordination. No pushes.
- Do not rewrite history or force-push.
- Keep generated files (`target/`, `.laburo/`, report PDFs) out of git.
- No secrets. No network in tests. No nondeterministic output.
- No invented numbers or domain facts.
- Tier 2 features are behind interfaces/flags; Tier 1 works without AI.
- Public behavior changes update the docs in the same change.

## Work unit template

Every task states:

- Work unit ID and use case ID.
- One-line goal.
- Files/directories owned.
- Acceptance test names.
- Fixtures used.
- Docs to update.
- Handoff note: changed, verified, unverified, next, do-not-touch.

## Definition of done

- [ ] Acceptance test exists and passes.
- [ ] `fmt`, `clippy -D warnings`, `cargo test` clean.
- [ ] Docs/handoff updated.
- [ ] No secrets, generated files, or unrelated edits.
- [ ] `docs/status.md` has exactly one next action.

## Handoff

End with: what changed, what is verified, what is unverified, the next action,
and any files another agent should not touch.
