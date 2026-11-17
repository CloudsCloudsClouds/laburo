# Execution model

Status: frozen for the wave
Last updated: 2026-11-17

A **test case** is designed once. A **run** is one execution of it. The user
chooses how the case is executed; Laburo does not force integration tests into a
unit-test chain.

## Modes

| Mode | Use when | Behavior |
|---|---|---|
| `manual` | A human must follow steps, or no driver exists | The user walks the steps, marks each pass/fail/blocked, adds notes and optional evidence. |
| `single` | One automated test/command covers the whole case | Laburo runs one driver target; one result is recorded; optional evidence attaches to the run. Good for integration/system tests. |
| `composed` | The case is an orchestration of smaller tests | Each step can map to one driver test/command. Laburo runs them in order and records a result per step. One test = one step. |

The user picks the mode per case. The default is `manual`.

## Step kinds

- `manual`: action + expected result; the user records the observed result.
- `driver`: action + expected result + `driver_target` (e.g.
  `UserTest#test_valid`, `test/models/user_test.rb:12`, or a nextest id).

A case may mix them. In `single` mode, a case-level `driver_target` is used and
steps are design documentation only.

## Manual flow

1. Create the run.
2. For each step: pass, fail, blocked, or skip; add observed result and notes.
3. Attach evidence if useful (screenshot, log, file, text).
4. Finish. The run status is derived (see `data-model.md`).
5. If a step failed, offer to create a bug report pre-filled with the case,
   step, run, expected result, and observed result.

## Automated flow

1. Resolve the case/step targets.
2. Run the driver in a worker thread; stream progress to the TUI.
3. Parse results into `TestOutcome` values.
4. Map outcomes to the case (`single`) or its steps (`composed`).
5. Store raw output as evidence; attach screenshots when the driver reports
   them.
6. Stop on first failure by default; `--keep-going` runs the rest.
7. A timeout is an `error`, not a hang.

## Evidence

Optional per step and per run:

- `kind`: screenshot, log, file, text.
- Files are copied into `.laburo/evidence/` with a hash and size.
- The original path/name is kept as metadata.
- The report embeds images; logs render as monospace; missing evidence is shown
  as a gap.
- A result is valid with just a status and notes. Evidence is never mandatory.

## Gherkin

Tier 1 stores a Gherkin text block on the case and renders it in reports. It does
not parse or execute Gherkin. Tier 2 may generate it and a Cucumber driver may
consume it.

## Discovery

Drivers can list tests they know about:

- Minitest: parse `test/**/*_test.rb` for `class ...Test` and `def test_*` /
  `test "..." do`; store file and line.
- Cargo nextest: `cargo nextest list --message-format json`.
- The TUI shows discovered tests and lets the user attach one to a step or to a
  `single` case. Discovery never creates a case automatically.

## Case ↔ test identity

- The case ID is stable and user-facing (`TC-...`).
- The driver test ID is stable within the driver (`file#Class#method` for
  Minitest; nextest test id for Cargo).
- The link is stored explicitly on the step or case; no name-convention magic is
  required.
