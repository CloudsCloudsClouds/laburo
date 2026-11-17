# Typst report templates

Active, Laburo-owned templates. They render the class-format annex from the
transient `data.json` projection described in `docs/reports.md`. The originals
in `src-elizabeth-tours/tqa/` stay read-only reference material.

## Files

| File | Report kind | Class reference / purpose |
|---|---|---|
| `lib.typ` | shared | Colors, page frame, status/priority chips, facts tables, evidence helpers, step/run/bug tables. Every template imports this file. |
| `dictionary.typ` | `laburo report dictionary` | Class test-case dictionary: all cases with ID, title, type, level, module, objective, expected result, plus a requirement traceability table. |
| `case.typ` | `laburo report case <TC-ID>` | Case dossier: design fields, tags, Gherkin, requirements, all runs, per-step execution detail, evidence, and linked bugs. |
| `run.typ` | `laburo report run <RUN-ID>` | Execution sheet: run metadata, the case under test, per-step status/observation/notes, evidence, and defects found during the run. |
| `bug.typ` | `laburo report bug <BUG-ID>` | Class bug report: ID, title, description, reproduction steps, expected vs actual, priority, severity, environment, evidence, comments. |

## Data contract

Each report directory contains:

```text
<timestamp>-<kind>-<id-or-all>/
├── data.json
├── <kind>.typ
├── lib.typ
└── report.pdf
```

`data.json` follows `docs/reports.md`. Two additive fields keep the templates
defensive:

- `cases` is present for the dictionary (array of case objects, same shape as
  `case`).
- every evidence object carries `exists` (boolean), computed by the generator
  from the absolute path on disk. `exists` is true only for a readable file in a
  format Typst can embed (images: png/jpg/jpeg/gif/webp/svg). Missing evidence
  renders as a dashed gap instead of failing the compile.

The generator writes absolute evidence paths so Typst can load them; compilation
uses `--root /` for that reason.

## Manual rendering

From a report directory:

```bash
typst compile --root / case.typ report.pdf
```

Set `LABURO_TYPST` to use a different Typst binary. Typst 0.15.1 is the pinned
version (see `docs/install.md`).
