# Typst report templates

Active, Laburo-owned class-format templates. They render the annex from the
transient `data.json` projection described in `docs/reports.md` and
`docs/v0.2-contracts.md`. The originals in `src-elizabeth-tours/tqa/` stay
read-only reference material.

## Files

| File | Report kind | Class reference / purpose |
|---|---|---|
| `lib.typ` | shared | Class palette, fonts, page shell, `ficha`, `registro`, status strip, evidence, and link helpers. Every template imports this file. |
| `dictionary.typ` | `laburo report dictionary` | Class dictionary: cover, summary, cases grouped by module, and requirement/link traceability. |
| `case.typ` | `laburo report case <TC-ID>` | Case dossier: class design ficha, objective, steps, expected result + expected artifact, traceability, execution history/detail, and defects. |
| `run.typ` | `laburo report run <RUN-ID>` | Execution sheet: class execution ficha, result strip, case under test and its expected artifact, per-step results and actual evidence, run artifacts, links, and defects. |
| `bug.typ` | `laburo report bug <BUG-ID>` | Class bug report: blue banner, top rule, label/value table, linked TC/run, linked items, expected artifact, role-based attachments, and comments. |

## Class visual language

`lib.typ` uses the palette and page furniture of
`src-elizabeth-tours/tqa/plantilla_tc_tqa.typ`; `bug.typ` uses the palette of
`Bug_report_template_blank.typ` / `Bug-report-template_ejemplo.docx`.

| Token | Value | Class use |
|---|---|---|
| ink | `#182030` | base text, label cells |
| blue | `#2D6CDF` | headings, dictionary header row |
| green | `#2D9B64` | execution headings, PASS, execution header row |
| red | `#CD4646` | FAIL / defects, defect header row |
| amber | `#EB912D` | BLOCKED, traceability header row, traceability cover box |
| grey | `#647080` | values and secondary text |
| zebra | `#F5F8FC` | value cells and even rows |
| design | `#E8EEF7` | “01 · design” cover box, tags |
| exec | `#DDF3EE` | “02 · execution” cover box |
| bug banner | `#4472c4` | bug report banner |
| bug rule | `#4f81bd` | bug report top rule |
| bug id | `#d9e2f3` | bug ID/header cells |
| bug label | `#dddddd` | bug label column |

- Fonts: `Aptos`, `Calibri`, `Carlito`, `Liberation Sans`, `DejaVu Sans`
  (headings prepend `Aptos Display`). Typst warns about unavailable families and
  falls back to the next installed one; `Liberation Sans` is present in the
  demo environment.
- Page: US Letter, class margins (top 39.6 pt, bottom 45.6 pt, x 43.2 pt),
  centred `TQA • …` footer. The bug report keeps the class A4 / 2.54 cm page and
  top rule.
- Components: dark-label `ficha`, coloured-header zebra `registro`, the
  four-state PASS / FAIL / BLOCKED / NOT RUN result strip, and class-style
  status tags.

## What each report renders

- **Dictionary**: cover boxes, summary counts, cases grouped by module (ID,
  title, type, level, priority, objective, expected result, status), and a
  traceability table with requirements and linked items.
- **Case dossier**: class design ficha, objective/tags, design steps, Gherkin,
  final expected result, case-level expected artifact, requirements, case links,
  run history, full execution detail with per-step actual evidence, and defects.
- **Run sheet**: run ficha, result strip, case under test, Gherkin, expected
  artifact, per-step results, per-step actual evidence, run-level artifacts,
  run links, and defects.
- **Bug report**: class bug form, linked TC and run, linked items, expected vs
  actual, role-based attachments, and comments. A bug without a linked case/run
  still renders cleanly with `—`.

**Clean output:** the active templates never render a legend, instructions,
“Reglas de uso”, working notes, or “fill me” placeholders. Empty optional
blocks are omitted; missing scalar values render as `—`. Missing evidence is
omitted rather than shown as a guide box.

## Data contract

```json
{
  "meta": { "generated_at": "...", "project": "...", "version": "..." },
  "case": {
    "id": "TC-001", "title": "...", "description": "...", "module": "...",
    "priority": "high", "status": "active", "environment": "local",
    "gherkin": "...", "tags": ["functional", "smoke"],
    "steps": [{"position": 1, "action": "...", "data": "...", "expected": "..."}],
    "requirements": [{"id": "RF-01", "title": "..."}],
    "expected_evidence": [{"kind": "screenshot", "path": "/abs/expected.png", "caption": "...", "role": "expected", "exists": true}],
    "links": [{"relation": "covers", "target_kind": "requirement", "target_id": "RF-01", "note": "...", "title": "..."}]
  },
  "cases": ["same case shape, for the dictionary"],
  "runs": [{
    "id": "RUN-...", "mode": "manual", "status": "passed",
    "started_at": "...", "ended_at": "...", "environment": "...",
    "commit": "...", "command": "...", "notes": "...",
    "steps": [{"position": 1, "status": "passed", "observed": "...", "notes": "...",
               "evidence": [{"kind": "screenshot", "path": "/abs/actual.png", "caption": "...", "role": "actual", "exists": true}]}],
    "evidence": [{"kind": "log", "path": "/abs/run.log", "caption": "...", "role": "artifact", "exists": true}],
    "links": [{"relation": "found-in", "target_kind": "bug", "target_id": "BUG-...", "note": "..."}]
  }],
  "bugs": [{
    "id": "BUG-...", "title": "...", "description": "...", "steps": "...",
    "actual": "...", "expected": "...", "priority": 1, "severity": 2,
    "environment": "...", "status": "open", "comments": "...",
    "case_id": "TC-001", "run_id": "RUN-...",
    "evidence": [{"kind": "screenshot", "path": "/abs/error.png", "caption": "...", "role": "actual", "exists": true}],
    "links": [{"relation": "reproduces", "target_kind": "case", "target_id": "TC-001", "note": "..."}]
  }]
}
```

### Required projection fields (Data pod)

The templates are defensive, but these names are the contract for the full
report:

| Field | Used by | Meaning |
|---|---|---|
| `meta.generated_at`, `meta.project`, `meta.version` | all | Header/footer and summary. |
| `case.expected_evidence` | case, run, bug | Case-level expected screenshot/photo/document. Falls back to `case.evidence` entries with `role == "expected"`. |
| `case.links` | case, dictionary | Case-level linked requirements, cases, runs, bugs, documents. |
| `runs[].steps[].evidence` | case, run | Per-step actual artifacts. |
| `runs[].evidence` | case, run | Run-level artifacts. |
| `runs[].links` | case, run | Run links to cases/bugs/requirements. |
| `bugs[].evidence` | bug | Attachments; `role == "expected"` goes under expected result, everything else under attachments. |
| `bugs[].links` | bug | Linked TC/run/requirement/document rows. |
| `bugs[].case_id`, `bugs[].run_id` | bug | Fallback when the projected `case`/`runs` objects are absent. |
| attachment `role` | all | `expected`, `actual`, `artifact`, or `evidence`. Defaults to `evidence`. |
| attachment `kind`, `path`, `caption`, `exists` | all | `screenshot` embeds an image; `log` embeds monospace text; other kinds show a file reference. `path` is absolute; `exists` must be computed by the generator because Typst cannot test a missing image. |

Link objects may use `target_kind`/`target_id` (entity-link shape) or
`kind`/`id`; `source_kind`/`source_id` are accepted as a fallback. `title` and
`note` are optional.

## Output layout

```text
<timestamp>-<kind>-<id-or-all>/
├── data.json
├── <kind>.typ
├── lib.typ
└── report.pdf
```

Evidence paths are absolute in `data.json`; compilation uses `--root /` so Typst
can read them.

## Manual rendering

```bash
typst compile --root / case.typ report.pdf
```

Set `LABURO_TYPST` to use a different Typst binary. Typst 0.15.1 is the pinned
version (see `docs/install.md`).
