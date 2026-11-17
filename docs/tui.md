# TUI

Status: design contract for the wave
Last updated: 2026-11-17

Ratatui + crossterm. Keyboard-first, vim motions, clean dark theme. Chamba is a
reference for the event loop, background runner, and Minitest parser — not for
the visual design.

## Layout

```text
┌ header: Laburo · project · db · status ─────────────────────────────┐
│ tabs: Cases  Runs  Bugs  Reports  Drivers  Quality  Dashboard        │
├───────────────────────────┬──────────────────────────────────────────┤
│ list / table              │ detail / form / preview                  │
│                           │                                          │
├───────────────────────────┴──────────────────────────────────────────┤
│ status bar: mode · keys · messages · progress                        │
└──────────────────────────────────────────────────────────────────────┘
```

- Two-pane list/detail where it helps; full-width form for create/edit.
- Header shows the project, DB path, and current mode.
- Status bar shows context keys and the last message.
- Use color for status only; borders subtle; no emoji overload.

## Views

| View | Content |
|---|---|
| Cases | ID, title, tags, priority, last status, last run. Create/edit/delete, run, bug, export. |
| Runs | Run history, status, mode, date, case. Open a run to see steps/evidence. |
| Bugs | ID, title, severity, status, linked case/run. Create/edit, link, close. |
| Reports | Generate dictionary/case/run/bug, open the output directory. |
| Drivers | Registered drivers, discovered tests, attach a test to a step/case. |
| Quality | Tier 2 findings: weak cases, missing evidence, flaky tests, open bugs without repro. |
| Dashboard | Counts, pass rate, recent failures, open bugs, untested cases. |

## Keymap

| Key | Action |
|---|---|
| `h` / `l` or `1..7` | Previous/next tab |
| `j` / `k` | Down/up |
| `g` / `G` | First/last |
| `Ctrl-d` / `Ctrl-u` | Half-page down/up |
| `Enter` | Open / confirm |
| `Esc` | Back / cancel |
| `/` | Search |
| `n` / `N` | Next/previous match |
| `c` | Create |
| `e` | Edit |
| `d` | Delete (confirm) |
| `r` | Run |
| `b` | Bug report |
| `x` | Export/report |
| `a` | Attach evidence |
| `p` / `f` / `m` | Pass / fail / blocked in a manual step |
| `n` | Note on the current step |
| `?` | Help |
| `:` | Command palette (optional, if time) |
| `q` | Back/quit (confirm if running) |

Long text (descriptions, steps, Gherkin, bug report) opens `$EDITOR` on a temp
file. Short fields use in-TUI input. No mouse requirement.

## Manual run flow

1. Open a case, press `r`.
2. Step list with the action and expected result.
3. `p`/`f`/`m` sets the status; `n` adds notes; `a` attaches evidence.
4. `Enter` advances; the run status is derived.
5. On failure, `b` offers a pre-filled bug report.

## Automated run flow

- The driver runs in a worker thread; progress events go through `mpsc`.
- The UI stays responsive; a spinner/progress bar is shown.
- Results map to the case (`single`) or steps (`composed`).
- `Ctrl-c` cancels the child process; the run is marked `error` with a note.

## Forms

- Create/edit case: ID, title, description, module, priority, status,
  environment, tags, Gherkin.
- Steps: add/remove/reorder; kind manual/driver; driver target picker.
- Bug: title, description, steps, actual/expected, priority, severity,
  environment, comments; link to case/run/step.
- Validation errors stay in the form; never lose input.

## Testing

- `ratatui::backend::TestBackend` snapshot tests for the shell and key views.
- No test requires a real terminal.
- The TUI calls the library; no business logic in views.
