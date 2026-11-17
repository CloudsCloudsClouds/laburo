# Swarm plan

Status: draft — depends on the author freezing `use-cases.md`
Last updated: 2026-11-17

How several agents can build Laburo without stepping on each other.

## Principles

- **Kernel first.** The domain, storage, interfaces, CLI shell, TUI shell, and
  fixture harness land before feature pods.
- **Freeze contracts, not code.** Traits and data schemas are the cross-pod
  coupling. Code inside a pod is free to move.
- **Vertical slices.** Each use case lands with its data, logic, test, and UI or
  CLI surface. No "layer for later".
- **Fixtures before integrations.** A fake Rails app and sample Minitest outputs
  let agents work without the real project or a database.
- **One owner per area.** Announce the area in `docs/status.md` before editing.
- **Tier 1 stays green.** Tier 2 work is behind interfaces and feature flags; it
  may not break a Tier 1 flow.
- **No invented facts or numbers.** AI output is a proposal, never a source.

## Kernel (must land before feature pods)

| Unit | Deliverable | Owns |
|---|---|---|
| K1 | Domain model + storage schema + migrations/serialization | `src/domain.rs`, `src/store/` |
| K2 | CLI shell, config, error model, exit codes | `src/cli.rs`, `src/config.rs` |
| K3 | TUI shell: tabs, navigation, vim motions, keymap | `src/tui/` |
| K4 | Minitest fixture harness: fake Rails app, sample runs, golden files | `tests/fixtures/` |
| K5 | Interfaces: `Runner`, `Catalog`, `Report`, `Notifier`, `AiProvider`, `Store` | `src/ports.rs` or `src/ports/` |

Kernel output: `laburo doctor` still works; `laburo run` can execute one fixture
case end to end; the TUI opens and navigates the fixture.

## Feature pods

| Pod | Use cases | Owns | Depends on | Parallel? |
|---|---|---|---|---|
| Catalog / Traceability | UC-01, UC-10, UC-12 | `src/catalog/`, `src/trace/`, `src/tqa/` | K1, K5 | Yes, after K1/K5 |
| Runner / Evidence | UC-03, UC-04, UC-05 | `src/runner/`, `src/evidence/` | K4, K5 | Yes; start with the fixture |
| TUI | UC-02, UC-04, UC-08 tabs | `src/tui/` | K3, catalog, runner | Yes, after K3 |
| Report / Notify | UC-06, UC-07 | `src/report/`, `src/notify/` | K1, UC-05 | Yes |
| Non-functional | UC-08, UC-21 | `src/nonfunctional/` | K5, UC-03 | Yes |
| Defects | UC-09 | `src/defects/` | K1 | Yes |
| CI | UC-11 | `.github/workflows/`, `scripts/` | CLI, runner, report | Yes, after CLI |
| AI | UC-13..UC-17, UC-19 | `src/ai/` | K5, catalog, defects | Tier 2; after Tier 1 spine |
| Metrics / Integrations | UC-18, UC-20, UC-22 | `src/metrics/`, `src/integrations/` | store, defects | Tier 2 |

## Suggested first ten work units

1. **K1** — domain + storage schema, with serialization tests.
2. **K4** — fixture harness: fake Rails app with one passing and one failing
   test, plus captured Minitest output.
3. **K5** — ports and data contracts.
4. **K2** — CLI shell: `doctor`, `list`, `run`, `report`, `version`.
5. **K3** — TUI shell: tabs, list, detail, keymap, quit.
6. **UC-01** — catalog import/list from a fixture YAML.
7. **UC-03** — run one selected fixture case through the runner port.
8. **UC-05** — persist and list a run.
9. **UC-04** — inspect the run in the TUI.
10. **UC-02** — filter, select, and run a subset from the TUI and CLI.

After those, the report/notify and non-functional pods can run in parallel.

## Work-unit template

Every swarm task states:

- Use case ID and a one-line goal.
- Files/directories owned.
- Acceptance test names, not just descriptions.
- Fixtures used.
- Docs updated (`docs/status.md`, and an ADR if a decision was made).
- Handoff note: what changed, what is verified, what is unverified, what is next,
  and what not to touch.

## Swarm rules

- At most two agents in shared files (`Cargo.toml`, `src/domain.rs`,
  `src/lib.rs`, `docs/status.md`) at once.
- Freeze the schema before pods start; schema changes need an ADR and a
  migration.
- New dependencies require a note in the handoff.
- No agent edits another agent's uncommitted files.
- No agent commits unless the author asks.
- Tests must be deterministic and offline.
- Tier 2 features are behind a feature flag or a provider trait.

## Risks

| Risk | Mitigation |
|---|---|
| Minitest selection/parsing is harder than expected. | Spike UC-03 first with a fixture; keep a fallback to JUnit XML. |
| The TUI becomes the product and the core rots. | All logic in the library; TUI calls it. CLI parity tests. |
| The catalog and the suite drift. | UC-10 traceability flags gaps; UC-12 round-trip is reviewable. |
| Tier 2 AI invents facts. | Every AI output cites records; human accepts; Tier 1 works without AI. |
| Agents conflict in shared files. | Kernel first; one owner per area; status board. |
| Scope creep beyond the class. | Tier 1 acceptance list; Tier 2 additive. |
