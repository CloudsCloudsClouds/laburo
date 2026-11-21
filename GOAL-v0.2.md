# GOAL — Laburo v0.2 (Demo Edition)

Status: P0 implemented and verified — author records the video and pushes the repo
Last updated: 2026-11-21 05:45

## Objective

Turn Laburo v0.1 into a **demo-ready, tag-driven TQA suite with a friendly web
UI, linked evidence, class-format reports, email delivery, and an MCP quality
gate for AI coding swarms**. The demo also **improves the Rails test suite in
`src-elizabeth-tours` into a diverse, genuinely useful cast of tests** and maps
those tests to Laburo cases.

Deliverable: a 15-minute video plus a public repo link.

## Timebox and stop rule

- **Maximum 5 hours of agent work.** Stop at 5 hours or when P0 is met.
- P0 first. P1 only if P0 is green. P2 is out.
- The demo must be recordable from a clean seed with one command.
- If the Rails suite uplift threatens the 5 hours, cap it at the highest-value
  booking/validation/auth tests plus one system flow. The web/evidence/MCP path
  still ships.
- If it looks good on a 1080p recording and the flows work, stop.

## Baseline (v0.1)

- Rust repo `laburo/`, installed as `laburo 0.1.0`.
- SQLite cases/steps/tags/runs/results/evidence/bugs/drivers/requirements.
- CLI, Ratatui TUI, Minitest driver, Typst reports, `casos.yaml` import, MCP
  (18 tools), deterministic quality rules, AI boundary.
- 48 tests green; demo env seeded in `src-elizabeth-tours/.laburo` (21 imported
  cases, 41 discovered Minitest tests, dictionary PDF).
- Rails app `src-elizabeth-tours/` is a nested repo; its test suite is the
  source of truth for Minitest results.

## Product framing

- **Web UI** = human-facing presentation and the main demo surface.
- **MCP** = AI-facing surface: the coding swarm asks Laburo “are we doing this
  right, and with quality?”
- **TUI** = power interface. It stays functional; a rework is P1.
- **Laburo as quality memory**: dictionary, runs, evidence, links, and bugs are
  the shared truth for a solo dev + swarm.

### The 1% idea

A coding agent can write code fast; it cannot know what “done” means for this
project. Laburo gives the swarm a standardized check-in:

1. What cases/tests cover this feature?
2. What was run, what passed, what has evidence?
3. What is missing, stale, or unfixed?
4. What is the next quality action?

That check-in is the 1% that makes the other 99% trustworthy. The AI is the
client; Laburo is the quality memory and the definition of done.

## P0 — must ship for the demo

### W0. Rails suite uplift (new, demo-critical)

The demo is not just Laburo looking at decorators. It includes making the Rails
test suite genuinely useful.

- Audit `src-elizabeth-tours/test/` and the imported TQA dictionary.
- Identify decorative/weak tests: no real assertion, vague name, duplicate
  coverage, missing negative/boundary case.
- Improve and add tests so the suite has **at least 15 meaningful cases**
  across:
  - unit/model validation (full name, phone, travellers 1–20, pickup time,
    addon totals);
  - integration (booking flow, admin roles/403, CSV reports, login);
  - system (public catalogue → booking → confirmation; the browser environment
    is runnable);
  - smoke/sanity happy path: catalogue → booking → confirmation, admin sees the
    booking, login; fast and repeatable;
  - negative/security (unauthorized admin, invalid pickup, missing fields, tour
    not found);
  - boundary values (travellers 0/1/20/21; pickup 04:59/05:00/22:59/23:00).
- Every test must have a clear purpose and a meaningful assertion. No
  decorative tests.
- If a test exposes a product bug, file a Laburo bug with the reproducing case;
  fix product code only if it blocks the demo.
- Keep `bin/rails test` and `bin/rails test:system` green; capture a real
  system-test screenshot.
- Map the improved tests to Laburo cases with driver targets; capture real logs
  and at least one real happy-path screenshot as evidence.
- Update the TQA dictionary/import so the demo’s cases reflect the real tests.

### W1. Web application shell

- `laburo web [--port 3000] [--host 127.0.0.1] [--open]`.
- Server-rendered Rust (`axum` + templates + CSS); no SPA/Node; offline; no
  external CDNs.
- Pages: Dashboard, Cases, Runs, Bugs, Drivers, Reports, Quality, Email.
- Layout: light, content-first, restrained. Windows 7-era sensibilities:
  pale gray chrome, white content cards, one soft blue accent, subtle
  gradients/shadows, rounded corners, crisp tables, clear typography. No dark
  neon, no glossy excess. Discrete.
- Responsive at 1280–1440; keyboard-friendly forms.
- Local-only by default; `--host 0.0.0.0` for ngrok sharing; optional
  `LABURO_WEB_TOKEN` gate.

### W2. Case catalogue: search, filter, automation, full CRUD

- Search by ID, title, module, step text.
- Filter by tag, tag category, level/type, priority, status.
- Tag chips with counts; clear filters; URL-encoded filter state.
- Create/edit/delete:
  - ID, title, description, module, priority, status, environment.
  - Tags (picker + free text + autocomplete).
  - Gherkin text.
  - Steps: add/remove/reorder, action/data/expected.
  - Step kind: manual or driver.
  - Step driver target: pick a discovered Minitest test with search.
  - Case-level driver target for `single` mode.
  - Default execution mode: `manual` / `single` / `composed`.
- **Automate a case**: choose a discovered test, set the target, switch mode,
  save, run. This is how a decorative dictionary row becomes a real automated
  case.
- Bulk actions on the filtered set: add/remove tag, archive, run, export.

### W3. Evidence, artifacts, and links

Evidence is a graph, not just a file on a run.

**Case design**

- Optional **expected artifact** per case: reference screenshot/photo, mockup,
  or document showing what “good” looks like (the happy-path photo).
- Optional links to requirements, other cases, or a source document.

**Step execution**

- Optional evidence per step: file upload (screenshot/log/pdf), caption, role
  (`actual`, `artifact`, `evidence`).
- Optional links from a step/run to another case, run, bug, or requirement.

**Run**

- Per-step artifacts and run-level artifacts.
- Links to the case, bugs found, and related runs.

**Bug report**

- Links to the test case that reproduces it (when one exists) and the run/step
  where it was found. **A bug with no standard trigger is allowed**; the UI
  warns and offers “create a case from this bug.”
- Evidence attachments and class fields.

**Data**

- `attachment.case_id` nullable + `role` (`expected`, `actual`, `artifact`,
  `evidence`); update the owner check.
- Generic `entity_link` table: source/target kind+id, relation
  (`reproduces`, `found-in`, `covers`, `related`, `evidence-for`), note,
  created_at; unique per relation.
- **Local per project:** DB, evidence, reports, outbox live under
  `<project>/.laburo/`; no global data store. Global config only for SMTP/AI.
- Web: expected-artifact block on the case page; evidence upload on step/run/
  bug pages; “linked items” picker/list with search/autocomplete.
- Reports render expected vs actual artifacts and linked items.

### W4. Runs and evidence

- Manual run: per-step pass/fail/blocked/error, observed text, notes, evidence.
- Automated run: Minitest targets, per-step results, raw output, artifacts.
- Run list/detail: status, mode, steps, evidence thumbnails/logs, linked bugs.
- Retry a run; file a bug from a failed step with case/run/step prefilled.
- Manual modes are first-class for white-box, stress, installation, security,
  usability, and exploratory testing.

### W5. Bugs and standardized reproduction

- Bug form: ID, title, description, steps, actual, expected, priority, severity,
  environment, comments.
- Strongly prompted: the case that reproduces it and the run/step where found.
  Optional; if absent, show a warning and offer “create a case from this bug.”
- Attach evidence; show linked case/run in the bug page and PDF.
- Lifecycle: open/resolved/closed/reopened; no assignment queues.
- Quality finding for bugs without a reproducing case.

### W6. Tag-driven and type-driven actions

- CLI: `laburo run batch --tag smoke --mode auto`, `--type security`,
  `--level system`, `--status active`, `--dry-run`.
- Web: “Run all filtered” from Cases, with mode choice and batch results.
- Batch summary: attempted, created, passed/failed/skipped, errors.
- Cases with no driver target are `manual-required`, not silently ignored.
- Search/filter and run-by-filter are first-class.

### W7. Class-format reports

- Use the class-given templates for dictionary, test case, run/execution sheet,
  and bug report.
- Reference originals: `src-elizabeth-tours/tqa/plantilla_tc_tqa.typ`,
  `diccionario_casos_de_prueba.typ`, `Bug_report_template_blank.typ`, and the
  root bug-report `.docx`.
- Active fillable templates in `laburo/templates/typst/`; keep class originals
  as reference.
- **Keep the class layout and colors.** Extend only with expected artifacts,
  per-step evidence, and linked TC/run.
- **Remove the legend and instructions** from the rendered reports; keep the
  layout. Final build must not carry guide text.
- Generate dictionary, case dossier, run sheet, bug report; report history with
  download links.

### W8. Email delivery

- **P0 is the `.eml` outbox.** No SMTP credentials or provider are required for
  the demo.
- CLI: `laburo email send --to ... --subject ... --report <path>` writes a
  standards-compliant `.eml` with the PDF attached to `.laburo/outbox/`.
- Web: Email page with recipient, subject, body, report picker, "Create .eml",
  outbox table, download/open link, and SMTP status = "not configured (demo
  mode)".
- `email_log` records recipient, subject, report, status, and file path.
- Optional SMTP transport stays behind env vars (`LABURO_SMTP_*`) and is P1; it
  is never required for the demo.
- No secrets are needed. ngrok, if used, is only for sharing the web UI.

### W9. Data migration and CLI surface

- Migration `0003_evidence_links_email.sql`:
  - `test_case.driver_target`, `test_case.default_mode`.
  - `attachment.case_id`, `attachment.role`.
  - `entity_link` table.
  - `email_log` table.
  - Indexes for tag/type filters.
- Model/repo/API updates for all callers.
- New CLI: `web`, `run batch`, `email send`, `case set-automation`, `link`,
  `unlink`.
- Existing CLI/TUI/MCP keep working.

### W10. MCP quality loop (AI surface)

- MCP tools:
  - `laburo_check_in`: given a feature/case/tag/run, return a standardized
    quality report: relevant cases, mapped/unmapped coverage, last results,
    evidence gaps, open bugs, next actions, status (`pass`/`warn`/`fail`).
  - `laburo_find_tests`: search cases and discovered tests by text/tag/feature.
  - `laburo_link` / `laburo_unlink` / `laburo_list_links`.
  - `laburo_record_evidence` with case/step/run/bug roles.
  - Keep existing 18 tools.
- MCP resources: `laburo://quality`, `laburo://cases`, `laburo://runs`,
  `laburo://bugs`, `laburo://coverage`.
- Gate semantics (proposal):
  - `pass`: mapped cases have recent green runs with evidence; no open critical
    bugs.
  - `warn`: unmapped/manual-only cases, stale runs, missing evidence, or medium
    bugs.
  - `fail`: a mapped run failed/errored, an open critical bug, or no mapped
    coverage for the requested feature.
- Deterministic; no live model required. **MCP only for now**; a local model
  summary is P1.
- `AGENTS.md`/README get a “Laburo quality contract” snippet for coding swarms:
  check in before/after changes, run relevant cases, record evidence, file a bug
  on failure.
- Web Quality page mirrors the check-in.

### W11. Autocomplete and quality-of-life

- Tag autocomplete (existing tags + categories).
- Searchable pickers for cases, runs, bugs, and driver tests in link/automation
  forms.
- Recipient autocomplete from recent email recipients.
- Search debounce; filter state in URL; clear-filters control.
- Flash messages/toasts; inline validation; unsaved-changes warning.
- Copy ID / copy link buttons.
- Empty states with the next action.
- Keyboard: `/` focus search, `n` new case, `g d` dashboard (optional).
- Bulk selection count and “select all filtered.”

### W12. Demo seed and 15-minute script

- Demo seed uses the improved real Rails tests and the real Laburo data.
- The video shows the improved suite **through Laburo** (cases, tags, runs,
  evidence, reports), not a walkthrough of Rails code diffs.
- Highlight the smoke/sanity happy-path cases and the real system-test run.
- **Do not invent decorative tests for the demo.** The Rails uplift is real.
- Map 2–3 improved real tests to real cases; leave the rest visibly
  manual/unmapped. The demo shows the decorator problem and how Laburo turns a
  decorative row into a real automated case.
- Capture one real happy-path screenshot from the Rails app as the expected
  artifact; capture real run logs/screenshots as actual artifacts.
- Seed Laburo data only: tags, mappings, 3–5 real runs, one bug with evidence,
  linked case/run, generated reports.
- `laburo demo seed [--reset]` or `scripts/demo-seed.sh`; one-command reset.
- `DEMO.md` rewritten around the web UI + MCP segment, with timestamps.
- Shot list: clean DB, no secrets, browser 1440p, terminal font, reset command,
  PDFs openable.

### W13. Verification

- `cargo fmt --all --check`, `cargo clippy --all-targets -- -D warnings`,
  `cargo test --all-targets` green.
- Existing 48 tests pass; new web/email/link tests.
- `bin/rails test` and `bin/rails test:system` green.
- Email `.eml` outbox contains the PDF attachment.
- `cargo install --path . --locked`; `laburo web` serves the seeded demo.
- HTTP smoke tests: dashboard, cases list/detail, run detail, reports, email
  dry-run, evidence upload, link creation.
- MCP smoke: `laburo_check_in`, `laburo_find_tests`, `laburo_link`, report.
- Full demo rehearsal.

## P1 — only if P0 is green

- TUI: tag/type filter, “run filtered”, full step editor, driver picker.
- TUI rework: simplify toward a read-only demo/dashboard + run controls.
- Web: driver discovery page with attach-to-case/step.
- Web: dashboard charts/trends.
- Optional local AI summary of the MCP check-in.
- Optional SMTP transport if credentials ever exist.
- Screenshots/GIF in README.
- Share token for ngrok.

## P2 — out of scope this wave

- Multi-user auth/teams/cloud.
- Full Cucumber/Selenium drivers.
- Real cargo-nextest runner.
- Robot Framework driver (white-box/stress/installation automation); keep the
  driver boundary ready for it later.
- Mobile-first design.

## Swarm plan (5 hours)

### Wave 0 — contracts, scaffold, Rails audit (45 min)

- Migration, model/repo changes, web stack, design tokens, route map, template
  parity checklist, demo seed contract.
- Rails test audit: list weak/decorative tests and the gap set.
- Freeze routes, template data shapes, evidence/link schema, email config,
  tag/type semantics, MCP gate semantics.
- `cargo check` green before pods start.

### Wave 1 — web core, cases, Rails uplift (2 h)

- Pod A: web server, layout, static assets, dashboard.
- Pod B: cases list/search/filter/tag chips.
- Pod C: case detail/edit/step editor/automation picker.
- Pod D: data migration + repo/automation/evidence/link API + CLI batch.
- Pod E: report web pages/downloads + class-template parity.
- Pod F: Rails suite uplift (tests + mapping + evidence).

### Wave 2 — execution, evidence, email, MCP (1.5 h)

- Pod G: manual/automated run pages, evidence upload, bug-from-step.
- Pod H: batch runs + batch results.
- Pod I: email service, dry-run `.eml`, outbox, CLI email.
- Pod J: MCP check-in/find/link tools + quality page.
- Pod K: demo seed using real cases/tests + real screenshot + curated runs.
- Pod L: TUI filters if time.

### Wave 3 — demo polish and verification (45 min)

- Pod M: visual polish, empty states, responsive checks.
- Pod N: `DEMO.md`, shot list, 15-minute script, README/repo checklist.
- Pod O: full tests/install/rehearsal; P0 bugs only.
- Coordinator integrates and stops at 5 hours.

## 15-minute demo outline

- 0:00 — problem: TQA is decoration; Laburo makes it the definition of done.
- 1:00 — dashboard: cases, runs, bugs, quality.
- 2:30 — cases, tags, filters: smoke/security/happy-path.
- 4:00 — the Rails suite uplift: show the real improved tests.
- 5:30 — automate a decorative happy-path case with a real Minitest test.
- 7:00 — expected artifact: the happy-path screenshot on the case.
- 8:00 — run all `smoke`/`security` cases by tag; batch results.
- 9:00 — evidence: actual screenshot/log per step; link the run.
- 10:00 — failure → bug linked to the reproducing case + run; evidence.
- 11:00 — class-format reports: dictionary, case, run, bug.
- 12:00 — email/outbox.
- 13:00 — the 1%: opencode swarm + Laburo MCP `check_in`, find tests, run,
  quality gate, bug.
- 14:30 — close: quality memory for solo dev + AI.

## Deliverable checklist

- [ ] Public repo link (author provides remote/credentials last; repo
      publishable, README, demo guide, no secrets, no `.laburo/`).
- [ ] 15-minute video recorded from the seeded demo.
- [ ] `DEMO.md` + README explain install, seed, demo path, MCP quality contract.
- [ ] P0 met; P1 marked optional.
- [ ] No live secrets in repo or video.

## Resolved decisions

- System tests are runnable; `bin/rails test:system` is in P0.
- Email is the `.eml` outbox only; no SMTP credentials or provider required.
  SMTP transport is optional P1.
- Rails uplift focus: smoke/sanity happy path plus the existing
  unit/integration coverage.
- The demo shows the improved suite through Laburo, not a Rails code diff.
- Windows 7 aesthetic, MCP gate, autocomplete scope, class templates, and repo
  link are accepted as written.

No blocking questions. Run `/goal` against this file.

## Final verification — 2026-11-21

- [x] `cargo fmt --all --check` clean; `clippy -D warnings` clean.
- [x] `cargo test --all-targets`: **83 passed, 0 failed**.
- [x] `bin/rails test`: **71 runs, 358 assertions, 0 failures**.
- [x] `bin/rails test:system`: **1 run, 13 assertions, 0 failures**; real
      Chromium screenshot captured.
- [x] `cargo install --path . --locked`: `laburo 0.1.0` installed.
- [x] Web UI running at `http://127.0.0.1:3000`; all primary pages return 200.
- [x] Cases: search/filter by tag/category/priority/status; full CRUD; step
      editor; driver picker; case-level target + default mode; "automate" POST.
- [x] Run all filtered: `tag=smoke`, auto -> `attempted=2 created=2 passed=2
      failed=0 manual-required=0 errors=0`; batch results page.
- [x] Manual/automated runs, evidence upload, expected artifact, entity links,
      bug-from-step.
- [x] Class-format PDFs: dictionary, case dossier, run sheet, bug report;
      expected screenshot, traceability, per-step evidence, linked case/run; no
      legend/instructions.
- [x] `.eml` outbox: web + CLI create/download a PDF-attached `.eml`; log row.
- [x] MCP 24 tools; `laburo_check_in tag smoke` goes **fail 40 -> pass 100**
      after the smoke batch and closing the bug.
- [x] Demo seed: 21 imported cases, 72 discovered tests, 7 real targets, 3 runs,
      one bug, expected artifacts, four PDFs, one `.eml`.
- [x] Web auto-reopens SQLite after `.laburo` reseed.
- [ ] **Author:** record the 15-minute video from `DEMO.md`.
- [ ] **Author:** create/push the public repo and add the link.

## Definition of done

P0 green, Rails suite genuinely improved and green (`test` + `test:system`),
demo rehearsal works from a clean seed, expected/actual artifacts and links
render in the UI and class-format PDFs, the `.eml` outbox works without secrets,
MCP `check_in` gives a useful quality verdict, 83 tests green, installed
binary, repo publishable, and the 15-minute video can be recorded without
touching the terminal except `laburo demo seed` and `laburo web`.
