# Status — v0.2 (Demo Edition)

Last updated: 2026-11-21 05:45

## Phase

**P0 complete and verified.** The web UI, evidence graph, class PDFs, `.eml`
outbox, MCP quality gate, Rails suite uplift, and demo seed are implemented.
The installed binary is running at <http://127.0.0.1:3000>.

## Evidence

```text
cargo fmt --all --check                  clean
cargo clippy --all-targets -- -D warnings
                                         clean
cargo test --all-targets                 83 passed, 0 failed
cargo install --path . --locked          laburo 0.1.0 installed
bin/rails test                           71 runs, 358 assertions, 0 failures
bin/rails test:system                    1 run, 13 assertions, 0 failures
```

Real seeded flows:

- Web routes `/`, `/search`, `/cases`, `/runs`, `/bugs`, `/reports`, `/drivers`,
  `/quality`, `/email` all return 200.
- **Run all filtered** (`tag=smoke`, mode auto): `attempted=2 created=2 passed=2
  failed=0 manual-required=0 errors=0`; batch results page works.
- **Web automation**: POST `/cases/TC-M01-01-1/automate` sets `single` +
  `driver_target`; the case badge changes from manual to single.
- **Evidence**: expected happy-path screenshot attached to two cases; per-step
  log/artifact evidence; links (case ↔ bug/run) created from web and CLI.
- **Email**: web and CLI create a standards-compliant `.eml` with the PDF
  attached in `.laburo/outbox`; web lists/downloads it.
- **Class PDFs**: dictionary (4 pages), case dossier (expected screenshot,
  traceability, run history), run sheet (per-step evidence), bug report
  (linked case/run, evidence); class layout/colors, no guide text.
- **MCP**: 24 tools. `laburo_check_in tag smoke` starts `fail 40` (failed run +
  critical bug); after the smoke batch and closing the bug it returns
  `pass 100`.
- **Rails uplift**: 71 tests / 358 assertions + 1 system test / 13 assertions;
  real Chromium screenshot at
  `src-elizabeth-tours/tmp/laburo-evidence/happy-path.png`.
- **Demo seed**: `scripts/demo-seed.sh` resets only `.laburo`, imports 21 cases,
  discovers 72 Minitest tests, maps 7 real targets, creates 3 runs, one bug,
  expected artifacts, four PDFs, and one `.eml`.
- **Web stale-DB fix**: the server reopens SQLite when `.laburo` changes.

## Remaining (author)

- Record the 15-minute video using `DEMO.md`.
- Create/push the public repo remote (repo is publishable; no secrets).
- P1 shipped: README screenshot gallery (`docs/screenshots/`), the dashboard
  run trend (last 12 runs, no JS/external chart), and
  `scripts/verify-demo.sh` (DB, PDFs, `.eml`, MCP, web).
- Optional P1 left: TUI rework, SMTP transport, GIF, ngrok share token.
- P2: Cucumber/Selenium, real nextest driver, Robot Framework.

## Notes

- SQLite is the only store; `casos.yaml` is import-only; reports are Typst
  projections.
- `.laburo/` is local and ignored; no secrets in the repo.
- The demo server is running with the installed binary; stop with
  `fuser -k 3000/tcp` if needed.
