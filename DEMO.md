# Laburo demo — Elizabeth Tours

Prepared: 2026-11-17. Timebox: 5 minutes.

The demo environment is already seeded in `src-elizabeth-tours/.laburo/`:

- SQLite database: `src-elizabeth-tours/.laburo/laburo.sqlite3`
- 21 test cases imported from `src-elizabeth-tours/tqa/casos.yaml`
- 41 Minitest tests discovered
- One generated dictionary PDF:
  `src-elizabeth-tours/.laburo/reports/20261117-190720-dictionary-all/report.pdf`
- `.laburo/` is ignored by the Rails repository.

All commands below run from the thesis root unless noted.

## 0. Sanity check (15 s)

```bash
laburo --project src-elizabeth-tours doctor
```

Expected: Rails app, Minitest, SQLite, Typst and nextest marked `ok`; TQA
dictionary marked `ok`.

## 1. Show the TQA catalogue (30 s)

```bash
laburo --project src-elizabeth-tours case list | head -8
laburo --project src-elizabeth-tours case show TC-M02-01-1
```

This is the class dictionary imported into SQLite. Point out the ID, module,
priority, tags (`unit`, `Funcional`, `imported`) and the three steps.

## 2. Show Minitest discovery (30 s)

```bash
laburo --project src-elizabeth-tours driver list | head -8
```

41 real Minitest tests from `test/**/*_test.rb`, with file and line. This is the
bridge between the catalogue and the Rails suite.

## 3. Run a case manually with evidence (60 s)

```bash
laburo --project src-elizabeth-tours run manual TC-M02-01-1 \
  --step-status passed,failed,passed \
  --note "step 2 fails in the demo" \
  --evidence laburo/tests/fixtures/evidence/log.txt
```

Copy the `RUN-...` id from the output, then:

```bash
laburo --project src-elizabeth-tours run show RUN-...
```

Point out: per-step status, notes, evidence attachment.

## 4. File the bug and export the class reports (90 s)

```bash
laburo --project src-elizabeth-tours bug create \
  --title "Booking step 2 returns 500" \
  --case TC-M02-01-1 \
  --run RUN-... \
  --severity 1 \
  --actual 500 \
  --expected "booking created"
```

```bash
laburo --project src-elizabeth-tours report case TC-M02-01-1
laburo --project src-elizabeth-tours report dictionary
```

Open the printed PDF paths. The case dossier includes the design, the failed
run, the evidence log, and the linked bug; the dictionary is the class format.

## 5. TUI (45 s)

```bash
laburo --project src-elizabeth-tours tui
```

Navigate with `h/l` or `1–6`: Cases, Runs, Bugs, Reports, Drivers, Dashboard.
`j/k` moves, `Enter` opens, `Esc` back, `?` help, `q` quits. Open a case and
press `r` to start a manual run; `p/f/m` records pass/fail/blocked, `n` adds a
note, `F`/`Esc` finishes.

## 6. Agent surface (20 s)

```bash
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/list","params":{}}' |
  laburo --project src-elizabeth-tours mcp
```

18 MCP tools; an agent can create cases, run them, record steps, attach
evidence, file bugs, generate reports, and query quality.

## 7. Quality findings (20 s)

```bash
laburo --project src-elizabeth-tours quality
```

Deterministic findings: cases with no steps, failed runs without bugs, missing
expected results. No AI required.

## Two-minute version

```bash
laburo --project src-elizabeth-tours doctor
laburo --project src-elizabeth-tours case show TC-M02-01-1
laburo --project src-elizabeth-tours driver list | head -5
laburo --project src-elizabeth-tours report dictionary
```

## Reset the demo data

```bash
rm -rf src-elizabeth-tours/.laburo
laburo --project src-elizabeth-tours init
laburo --project src-elizabeth-tours import
laburo --project src-elizabeth-tours driver discover
```

## Notes

- `laburo` is installed in `~/.cargo/bin/laburo`.
- Typst is invoked as an external command; no Rust dependency.
- No YAML is written by Laburo; `casos.yaml` is imported once.
- Reports keep `.typ`, `data.json` and `report.pdf` together.
