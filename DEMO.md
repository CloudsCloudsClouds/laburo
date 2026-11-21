# Laburo v0.2 — demo walkthrough

Target: 15 minutes. Web UI is the centerpiece; MCP is the AI quality loop.

## Prepare (once)

```bash
cd laburo
cargo install --path . --locked
./scripts/demo-seed.sh          # resets only <project>/.laburo
laburo web --port 3000
```

Open <http://127.0.0.1:3000>. The demo project is
`../src-elizabeth-tours` by default.

## 15-minute script

### 0:00 — the problem

TQA documentation is usually decoration. Laburo makes the dictionary, tests,
runs, evidence, and bugs one queryable source of truth for a solo dev + AI.

### 1:00 — dashboard

Show cases, runs, bugs, pass rate, recent failures, quality findings. Point at
the seeded demo: the Rails suite is real, the evidence is real.

### 2:30 — cases, tags, filters

Open **Cases**. Filter `smoke`, then `security`, then `system`; search a title;
clear filters. Show tag chips and counts. This is the “find the right test
without remembering commands” moment.

### 4:00 — the Rails suite uplift

Open a smoke/sanity happy-path case. Show its real Minitest target
(`test/...`), the tags, and the last run. Say: the dictionary row is no longer a
decorator; it points at a real test.

### 5:30 — automate a decorative case

Open an imported case that is still manual. Press **Automate**, search a
discovered Minitest test, pick it, set mode `single`, save. Run it. Show the
status change.

### 7:00 — expected artifact

On the case page, show the **expected artifact** (the real happy-path
screenshot). Explain: the case says what “good” looks like; the run says what
actually happened.

### 8:00 — run by tag

Back to Cases, filter `smoke`, choose **Run all filtered**, mode `auto`. Show
the batch summary: attempted, passed, failed, manual-required.

### 9:00 — evidence and links

Open a run. Show per-step actual artifacts/logs, linked case, and the
evidence upload. Show a bug linked to the reproducing case and run.

### 10:00 — bug from a failure

Open a failed step, **File bug**. Show prefilled case/run/step, severity,
evidence. Save. Open the bug; show the reproduction link.

### 11:00 — class-format reports

Open **Reports**. Generate/download the dictionary, case dossier, run sheet,
and bug report. Open the PDF: class layout/colors, expected/actual artifacts,
linked TC/run, no legend/instructions.

### 12:00 — email/outbox

Open **Email**. Pick the report, recipient `demo@example.com`, create the
`.eml`. Show the outbox and download the `.eml`. No SMTP credentials needed.

### 13:00 — the 1%: MCP quality loop

Show the README quality contract. In a terminal, simulate the swarm check-in:

```bash
printf '%s\n' \
  '{"jsonrpc":"2.0","id":1,"method":"initialize","params":{}}' \
  '{"jsonrpc":"2.0","id":2,"method":"tools/call","params":{"name":"laburo_check_in","arguments":{"tag":"smoke"}}}' |
  laburo --project ../src-elizabeth-tours mcp
```

The seeded check-in is **fail 40**: the smoke cases have a failed manual run and
an open critical bug. Run the smoke batch (web **Run all filtered** or
`laburo run batch --tag smoke --mode auto`), close the bug, and check in again:

```bash
laburo --project ../src-elizabeth-tours bug close BUG-...
laburo --project ../src-elizabeth-tours quality
```

The same check-in now returns **pass 100**: every smoke case has a recent green
run and an expected artifact, and no critical bug is open. Point out: the AI
orchestrates; Laburo remembers and judges.

### 14:30 — close

Web for humans, MCP for agents, SQLite for truth. `laburo web`, `laburo_check_in`,
class-format PDFs, no secrets.

## Two-minute version

1. Dashboard.
2. Filter `smoke`, run all filtered.
3. Open a case with expected artifact and a real run.
4. Generate the dictionary PDF.
5. Show the `.eml` outbox.
6. Show `laburo_check_in` over MCP.

## Video checklist

- [ ] `./scripts/demo-seed.sh` from a clean state.
- [ ] `laburo web --port 3000` and browser at 1440p.
- [ ] No secrets, no `.laburo/` paths with personal data.
- [ ] Rail test suite green (`bin/rails test` + `bin/rails test:system`).
- [ ] PDFs open cleanly; no legend/instruction text.
- [ ] `.eml` downloads and contains the PDF attachment.
- [ ] MCP check-in returns a readable verdict.
- [ ] Close with the repo link and the quality-contract idea.

## Reset

```bash
./scripts/demo-seed.sh
```

Only `<project>/.laburo` is reset; the Rails repo and its tests are untouched.
