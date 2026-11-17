# Data model

Status: frozen for the wave
Last updated: 2026-11-17

SQLite is the store. The migration is `migrations/0001_init.sql`. JSON is only a
transient report projection; YAML is only a one-time import.

## Conventions

- Case IDs are user-facing: `TC-001`, `TC-M02-01-1`, etc. Primary key `TEXT`.
- Run IDs are generated: `RUN-YYYYMMDD-HHMMSS-N`.
- Bug IDs are generated: `BUG-YYYYMMDD-N`, or user-supplied.
- Timestamps are ISO-8601 local strings (`2026-11-17 18:30:00`).
- Status values are lower-case in SQLite; the UI can display them capitalised.
- Evidence files live in `.laburo/evidence/`; SQLite stores the relative path,
  hash, size, MIME, and caption.
- Foreign keys are enabled; deletes cascade where ownership is clear.

## Tables

| Table | Purpose | Key columns |
|---|---|---|
| `test_case` | Test case design | `id`, `title`, `description`, `module`, `priority`, `status`, `environment`, `gherkin`, timestamps |
| `test_step` | Step of a case | `case_id`, `position`, `action`, `data`, `expected`, `kind` (`manual`/`driver`), `driver_target` |
| `tag` | Tag dictionary | `name`, `category` |
| `case_tag` | Case ↔ tag | `case_id`, `tag_id` |
| `test_run` | One execution of a case | `id`, `case_id`, `mode` (`manual`/`single`/`composed`), `status`, start/end, environment, commit, command, notes |
| `step_run` | Result of one step | `run_id`, `step_id`, `position`, `status`, `observed`, `notes`, `duration_ms`, `driver_test`, `raw_output` |
| `bug_report` | Defect | `id`, title/description/steps, actual/expected, priority, severity, environment, status, links to case/run/step |
| `attachment` | Evidence | `kind`, `path`, original name, MIME, hash, size, caption; linked to step/run/bug |
| `driver` | Registered driver | `id`, `kind`, `name`, `config_json`, `enabled` |
| `driver_test` | Discovered test cache | `id`, `driver_id`, `name`, `file`, `line`, `metadata_json`, `discovered_at` |
| `requirement` | Requirement/use case | `id`, `title`, `source`, `acceptance`, `kind` |
| `case_requirement` | Traceability | `case_id`, `requirement_id` |

## Statuses

- Run/step: `not_run`, `running`, `passed`, `failed`, `blocked`, `error`.
- Case: `draft`, `active`, `archived`.
- Bug: `open`, `resolved`, `closed`, `reopened`.
- Priority: `low`, `medium`, `high`, `critical` for cases; `1..5` for bugs.
- Severity: `1..5` for bugs.

Derivation for a case run:

- Any `failed`/`error` -> `failed`.
- Else any `blocked` -> `blocked`.
- Else all steps `passed` -> `passed`.
- Else `not_run`/`running` -> not complete.

## Tags

Seeded categories:

- `level`: unit, integration, system.
- `type`: functional, non-functional.
- `technique`: black-box, white-box.
- `quality`: regression, smoke, sanity, acceptance, security, performance,
  load, stress, compatibility, installation, recovery, usability, localization.
- `custom`: anything else.

## Migrations

- `PRAGMA user_version` tracks the applied schema.
- Migrations are append-only files in `migrations/`.
- The DB layer applies them on open, inside a transaction.
- `laburo db backup` uses `VACUUM INTO`; `laburo db restore <file>` copies back
  after a confirmation.

## Report projection

The report layer reads SQLite and writes a temporary JSON file with the exact
shape each Typst template expects. It never writes JSON back to SQLite.
