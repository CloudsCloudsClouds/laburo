# Requirements (historical)

> **Superseded.** This file records the first read-only "collect results and
> emit `casos.yaml`" framing. The current spec is
> [`use-cases.md`](use-cases.md), where Laburo is a TQA suite for a solo dev:
> catalog, run, report, email, non-functional track, defects, traceability, and
> Tier 2 AI assistance. Do not build from the table below.

Status: historical
Last updated: 2026-11-17

Every requirement has a stable ID. `proposed` means the author has not confirmed
it; do not build a large surface on a proposed requirement without asking.

## Functional

| ID | Status | Requirement | Acceptance |
|---|---|---|---|
| LAB-F-001 | proposed | Read a Minitest run from the Rails app. | Given a completed run, `collect` produces a normalized run with one result per test and its status, duration, and location. |
| LAB-F-002 | proposed | Read a `cargo test` run. | Same normalized shape, framework = `cargo test`. |
| LAB-F-003 | proposed | Attach CI evidence to a run. | A run references its log and failed system-test screenshots by path and commit. |
| LAB-F-004 | proposed | Check congruence between the run and `casos.yaml`. | Report rows with no test, tests with no row, and rows whose declared status, path, or actor disagrees. |
| LAB-F-005 | proposed | Update `casos.yaml` without losing author edits. | Only machine-owned fields change; human fields and comments survive; the diff is reviewable. |
| LAB-F-006 | proposed | Recompile the Typst annex. | One command refreshes the data and runs `typst compile`; failure reports Typst's output. |
| LAB-F-007 | proposed | Provide a non-interactive CLI. | Every command works headless and in CI; machine-readable output is available. |
| LAB-F-008 | proposed | Provide a TUI over the same library. | Browse runs, suites, failures, and evidence; no logic lives only in the TUI. |

## Non-functional

| ID | Status | Requirement |
|---|---|---|
| LAB-N-001 | proposed | Deterministic output: stable ordering, and no timestamps in generated data unless the run provides them. |
| LAB-N-002 | proposed | Offline by default; no network calls in the core path. |
| LAB-N-003 | proposed | No silent data loss in YAML round-trips; comments are preserved or the write is refused. |
| LAB-N-004 | proposed | Linux and macOS first; Windows is not a target until needed. |
| LAB-N-005 | proposed | Clear, actionable errors: name the file, line, and expected shape. |
| LAB-N-006 | proposed | Fast enough for the full Rails suite: collection under a second after the run. |
| LAB-N-007 | proposed | No secrets in logs or evidence; redact configured patterns. |

## Constraints

- Rust 2024 edition, stable toolchain.
- The Rails app and its test suite are the source of truth for results.
- The TQA dictionary schema is owned by the Rails project
  (`src-elizabeth-tours/tqa/`), not by Laburo.
- The author confirms domain facts and "done"; see the root `MODELS.md`.

## Out of scope

- Running tests, CI orchestration, bug tracking, and the thesis text.
- Rows for M-04..M-07 until they have use-case diagrams.
- A web UI.
