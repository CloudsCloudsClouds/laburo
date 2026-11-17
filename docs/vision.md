# Vision

Status: draft — author to confirm against the class requirements
Last updated: 2026-11-17

## Problem

The TQA final project asks for more than a green suite. It asks for a test
automation system over the integrador: a screen to choose test cases by type, a
way to run them, a report of what ran, a way to email that report, a separate
track for non-functional tests, CI/CD, and at least 15 test cases. The TQA
document additionally needs the cases, execution evidence, traceability, bug
reports, and Gherkin scenarios.

Today the Elizabeth Tours tests live in the Rails app, are run from a terminal,
and produce output that is read once and lost. There is no single place to
select a subset, see what happened, keep the evidence, or produce the class
artifacts. The dictionary (`tqa/casos.yaml`) is maintained by hand and drifts
from the suite.

## Users

- **Author (primary)** — selects and runs tests, records evidence, produces the
  report, emails it, and defends the project.
- **Jury (indirect)** — reads the report and the TQA document; may watch a run.
- **Agents (secondary)** — need a deterministic CLI and a documented library to
  extend the tool without guessing.

## Jobs

1. When I open Laburo, I want to choose test cases by type (unit, integration,
   system; positive, negative, regression) and run them, so I do not have to
   remember Rails commands.
2. When a run finishes, I want to see what passed, failed, or was skipped, with
   duration and evidence, so I can decide what to fix.
3. When I need the TQA deliverable, I want a report file of the run and a way to
   email it, so the class requirement is met without manual copying.
4. When I test non-functional behavior, I want a separate tab for load, stress,
   performance, security, compatibility, and similar checks, so functional and
   non-functional results are not mixed.
5. When I update the TQA document, I want the cases, evidence, traceability, and
   bugs to come from the same data, so the document cannot drift from reality.
6. When I push to CI, I want a headless run that records the execution, so the
   evidence is reproducible.
7. Optionally, when I write a scenario in natural language, I want Gherkin
   generated and automated, so I can reach the class's 100-grade extension.

## Principles

- **Minitest is the runner.** Laburo orchestrates; it does not reimplement
  Minitest.
- **CLI core, TUI on top.** The TUI is the intended interface, but every action
  must be possible headless and testable.
- **Local-first.** No network in the core path; email and optional agents are
  explicit, configured side effects.
- **Deterministic.** Same inputs and same run produce the same report.
- **Traceable.** Every result names the test, the command, the run, and the
  artifact it came from.
- **The author's data wins.** Generated artifacts never silently overwrite
  hand-written test cases or decisions.
- **No secrets.** SMTP credentials and tokens live outside the repo.

## Scope

In:

- Minitest test discovery and selection from the Rails app.
- Running selected unit, integration, and system tests; recording results.
- A functional TUI track and a separate non-functional track.
- Report generation (HTML/PDF first; Word/Excel optional).
- Emailing the report.
- TQA artifacts: dictionary, execution evidence, traceability, bugs, Gherkin.
- CI/CD execution and recorded runs.
- The class's minimum of 15 test cases; the project already has around 47.

Out (for now):

- Replacing Minitest, Rails, or GitHub Actions.
- A general-purpose CI server.
- A web UI.
- Automatic test writing without author review.
- The thesis text itself.

## Success

- At least 15 test cases can be selected by type and executed from Laburo.
- A run produces a report that can be emailed and attached to the TQA document.
- Functional and non-functional results are separate and both recorded.
- CI runs the same headless path and keeps the evidence.
- A second agent can implement an adapter from the docs alone.
- The author stops maintaining the same fact in two places.

## Sources

- Class notes: `~/Documents/MisNotas/10 Proyectos/15 Unifranz SIS/TQA-711 Testing y QA/TQA-711 Clase 8.md`, `TQA-711 Clase 9.md` (final project).
- Class notes: `TQA-711 Clase 2.md`, `Clase 3.md`, `Clase 4.md`, `Clase 6.md` (test cases, bugs, functional/non-functional, coverage).
- Official template: `~/Downloads/uni/Documento_Plantilla_TQA_FINAL.docx` (Section 6).
- Official template: `~/Downloads/Plantilla_TC_y_Ejecucion_TQA.docx` (TC design and execution).
- Hito 3 instructions: `~/Downloads/Examen_Practico_Hito_3_TQA_Instrucciones.pptx`.

> The class notes are a record of what the instructor said, not an official
> syllabus. The author must confirm this scope. See
> [`ElizToursWiki/Research/TQA-711 final project requirements.md`](../../ElizToursWiki/Research/TQA-711%20final%20project%20requirements.md).
