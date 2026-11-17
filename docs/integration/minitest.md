# Minitest integration

Status: research — no format chosen
Last updated: 2026-11-17

Laburo must read what the Rails test suite did. The app currently runs Minitest
through `bin/rails test` (unit and integration) and `bin/rails test:system`
(browser). CI runs both and uploads the screenshots of failed system tests.

## What a run must provide

- One record per test: stable identity, status, duration, and source location.
- Enough context to attribute the run: suite, command, commit, and timestamp.
- Failures with the assertion message and backtrace.
- A machine-readable artifact that survives CI and can be checked in as a
  fixture.

## Candidate artifacts

| Option | Pros | Cons | Status |
|---|---|---|---|
| Default Minitest stdout | No new dependency; already produced. | Human-oriented, unstable, hard to parse reliably. | Fallback only. |
| JUnit XML via a reporter gem | Standard shape; CI-friendly; many parsers. | Adds a gem; exact gem and version not chosen; XML shapes vary. | **Preferred candidate**; needs a spike. |
| Custom Minitest plugin → JSON | Exact fields; no XML; small. | We own the plugin and its compatibility with Minitest/Rails. | Viable. |
| Rails' own test report | Possibly built in. | Not verified for this Rails version. | Check first. |

> Unverified: the built-in reporting options of Rails 8.1 and the exact
> reporter gem to use. Verify against the actual app; do not assert a flag or
> version from memory.

## Proposed spike (M1)

1. Run the existing suite in `src-elizabeth-tours/` and capture the default
   output.
2. Check whether Rails/Minitest can emit JUnit XML without a new dependency.
3. If not, add a reporter on a branch and capture one XML file.
4. Build a fixture from that real run (redact secrets), then implement the
   parser against the fixture.
5. Record the decision as an ADR.

## Identity and the dictionary

Minitest gives `ClassName#test_method`; the dictionary gives `TC-M02-01-1`.
Nothing today maps one to the other. Options:

- **Name convention:** test methods carry the TC id, e.g.
  `test_tc_m02_01_1_persists_full_name`.
- **Location mapping:** the dictionary's `ubicacion` plus the test name.
- **Sidecar map:** a small file mapping TC ids to test ids.

The author decides. Until then, `check` can report by file and line, which is
useful without a mapping.

## Evidence

- `bin/rails test` output: capture stdout, stderr, and the exit code.
- `bin/rails test:system`: Rails writes failure screenshots under
  `tmp/screenshots` (verify the path in this app).
- CI: download the `system-test` artifact and keep the file names.
- Never store secrets, cookies, or database dumps as evidence.

## Open questions

- Which command is the canonical run: the two Rails commands, or a wrapper?
- Is "green" per test, per suite, or per run?
- Do we keep historical runs, or only the latest?
- Can test names carry TC ids, or is the mapping external?
