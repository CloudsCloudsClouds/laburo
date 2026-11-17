# Decision — Laburo foundation

Date: 2026-11-17
Status: accepted for the v0.1 wave

## Context

Laburo is the TQA final project: a local-first TQA suite for a solo developer
working with AI. It must manage test cases and runs, execute manual and
Minitest-backed tests, record evidence, file bugs, export class-format documents,
and expose an agent automation surface. The class templates are Typst/Word; the
author wants Typst and a Ratatui TUI. The thesis root keeps Laburo as its own
repo.

## Decision

- **SQLite is the store.** `rusqlite` bundled, WAL, foreign keys, explicit
  migrations. No JSON state store. JSON is only a transient Typst projection.
- **No YAML in Laburo.** `casos.yaml` is imported once into SQLite.
- **Typst CLI.** The installed `typst` binary renders PDFs; no Typst crate. Keep
  `.typ` + JSON next to the PDF.
- **Per-case execution mode:** `manual`, `single`, or `composed`. The user
  chooses. One driver test = one step only in composed mode.
- **Evidence is optional.** Status + notes is valid; files/screenshots attach.
- **Tags, not enums**, for level/type/technique/quality.
- **Drivers:** manual always, Minitest first, cargo-nextest second,
  Cucumber/Selenium later.
- **MCP stdio server** exposes the library to agents.
- **Tier 2 AI is a quality safeguard:** deterministic rules + local
  OpenAI-compatible model ranking; the human accepts. No direct writes.
- **Install:** `cargo install --path . --locked`.

## Alternatives

- JSON store: rejected; relations, history, evidence links, and filtering need a
  database.
- HTML/Word reports: rejected; Typst is the author's choice and the class
  templates map cleanly.
- Single execution model: rejected; integration/system tests are not always an
  orchestration of unit tests.
- Hard-coded test-type enum: rejected; the class and project tags vary.
- Email first: rejected; it is a forgettable requirement and belongs behind a
  notifier interface.

## Consequences

- The schema in `migrations/0001_init.sql` is the contract for the swarm.
- Report templates are data-driven; Rust never concatenates Typst strings.
- The driver trait and MCP tools are the extension points.
- Tier 1 must remain usable with AI disabled.
- The thesis root is untouched; local commits/branches in `laburo/` are allowed,
  no pushes.
