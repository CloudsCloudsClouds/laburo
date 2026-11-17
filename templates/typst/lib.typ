// Shared style for Laburo reports.
//
// Every active template imports this file and reads `data.json` from the same
// directory. The projection contract lives in docs/reports.md; keep helpers
// defensive so a missing field or an empty list renders a gap, never an error.

#let palette = (
  ink: rgb("#1f2937"),
  brand: rgb("#1f3a5f"),
  muted: rgb("#6b7280"),
  rule: rgb("#d1d5db"),
  surface: rgb("#f3f4f6"),
  accent: rgb("#e8eef7"),
  good: rgb("#166534"),
  bad: rgb("#991b1b"),
)

#let status-palette = (
  passed: (bg: rgb("#dcfce7"), fg: rgb("#166534")),
  failed: (bg: rgb("#fee2e2"), fg: rgb("#991b1b")),
  blocked: (bg: rgb("#fef3c7"), fg: rgb("#92400e")),
  error: (bg: rgb("#fee2e2"), fg: rgb("#991b1b")),
  running: (bg: rgb("#dbeafe"), fg: rgb("#1e40af")),
  "not-run": (bg: rgb("#e5e7eb"), fg: rgb("#374151")),
  open: (bg: rgb("#fee2e2"), fg: rgb("#991b1b")),
  reopened: (bg: rgb("#ffedd5"), fg: rgb("#9a3412")),
  resolved: (bg: rgb("#dcfce7"), fg: rgb("#166534")),
  closed: (bg: rgb("#e5e7eb"), fg: rgb("#374151")),
  draft: (bg: rgb("#e5e7eb"), fg: rgb("#374151")),
  active: (bg: rgb("#dcfce7"), fg: rgb("#166534")),
  archived: (bg: rgb("#e5e7eb"), fg: rgb("#374151")),
  low: (bg: rgb("#dcfce7"), fg: rgb("#166534")),
  medium: (bg: rgb("#fef3c7"), fg: rgb("#92400e")),
  high: (bg: rgb("#ffedd5"), fg: rgb("#9a3412")),
  critical: (bg: rgb("#fee2e2"), fg: rgb("#991b1b")),
)

#let status-colors(status) = {
  let key = lower(str(status)).replace("_", "-").replace(" ", "-")
  status-palette.at(key, default: (bg: palette.surface, fg: palette.muted))
}

/// Render a value safely: `none` and blank strings become `default`.
#let field(value, default: "—") = {
  if value == none {
    default
  } else {
    let text = str(value).trim()
    if text == "" { default } else { text }
  }
}

/// Coerce a projected list into an array.
#let list-of(value) = {
  if value == none {
    ()
  } else if type(value) == array {
    value
  } else {
    (value,)
  }
}

#let chip(label, status: "") = {
  let colors = status-colors(status)
  box(
    fill: colors.bg,
    stroke: 0.4pt + colors.fg,
    radius: 2.5pt,
    inset: (x: 5pt, y: 2pt),
    text(
      size: 8pt,
      weight: "medium",
      fill: colors.fg,
      upper(str(label).replace("_", " ")),
    ),
  )
}

#let status-chip(status) = chip(field(status, default: "unknown"), status: status)

#let priority-chip(priority) = {
  let p = if priority == none {
    3
  } else if type(priority) == int {
    priority
  } else {
    3
  }
  let colors = if p <= 1 {
    (bg: rgb("#fee2e2"), fg: rgb("#991b1b"))
  } else if p == 2 {
    (bg: rgb("#ffedd5"), fg: rgb("#9a3412"))
  } else if p == 3 {
    (bg: rgb("#fef3c7"), fg: rgb("#92400e"))
  } else {
    (bg: rgb("#e5e7eb"), fg: rgb("#374151"))
  }
  box(
    fill: colors.bg,
    stroke: 0.4pt + colors.fg,
    radius: 2.5pt,
    inset: (x: 5pt, y: 2pt),
    text(size: 8pt, weight: "medium", fill: colors.fg, "P" + str(p)),
  )
}

#let severity-chip(severity) = {
  let s = if severity == none {
    3
  } else if type(severity) == int {
    severity
  } else {
    3
  }
  let colors = if s <= 1 {
    (bg: rgb("#fee2e2"), fg: rgb("#991b1b"))
  } else if s == 2 {
    (bg: rgb("#ffedd5"), fg: rgb("#9a3412"))
  } else if s == 3 {
    (bg: rgb("#fef3c7"), fg: rgb("#92400e"))
  } else {
    (bg: rgb("#e5e7eb"), fg: rgb("#374151"))
  }
  box(
    fill: colors.bg,
    stroke: 0.4pt + colors.fg,
    radius: 2.5pt,
    inset: (x: 5pt, y: 2pt),
    text(size: 8pt, weight: "medium", fill: colors.fg, "S" + str(s)),
  )
}

#let tag-list(tags) = {
  let tags = list-of(tags)
  if tags.len() == 0 {
    text(fill: palette.muted, "—")
  } else {
    tags
      .map(tag => box(
        fill: palette.accent,
        inset: (x: 4pt, y: 1pt),
        radius: 2pt,
        text(size: 8pt, fill: palette.brand, field(tag)),
      ))
      .join(h(4pt))
  }
}

/// A compact label/value table. `rows` is an array of `(label, value)` pairs.
#let facts(rows, label-width: 28%) = {
  let cells = ()
  for (label, value) in rows {
    cells.push(text(size: 8.5pt, fill: palette.muted, upper(str(label))))
    cells.push(value)
  }
  table(
    columns: (label-width, 1fr),
    stroke: none,
    inset: (x: 0pt, y: 2.8pt),
    ..cells,
  )
}

#let report-section(title, body) = block(
  width: 100%,
  above: 13pt,
  below: 2pt,
  breakable: true,
)[
  #text(size: 9pt, weight: "bold", fill: palette.brand, upper(title))
  #v(3pt)
  #line(length: 100%, stroke: 0.7pt + palette.rule)
  #v(5pt)
  #body
]

#let code-block(body, lang: "text") = block(
  width: 100%,
  fill: palette.surface,
  inset: 7pt,
  radius: 3pt,
  stroke: 0.5pt + palette.rule,
  breakable: true,
)[
  #raw(body, lang: lang, block: true)
]

#let gap(note: "Evidence missing.", path: "") = block(
  width: 100%,
  fill: palette.surface,
  inset: 10pt,
  radius: 3pt,
  stroke: (paint: palette.rule, thickness: 0.7pt, dash: "dashed"),
  breakable: false,
)[
  #text(size: 8.5pt, weight: "bold", fill: palette.muted)[GAP]
  #h(5pt)
  #text(size: 9pt, fill: palette.ink)[#note]
  #if path != "" [
    #linebreak()
    #text(size: 8pt, fill: palette.muted)[#path]
  ]
]

#let log-block(path, limit: 4000) = {
  let body = read(path)
  let shown = if body.len() > limit {
    body.slice(0, limit)
      + "\n\n… ["
      + str(body.len() - limit)
      + " characters truncated]"
  } else {
    body
  }
  code-block(shown)
}

/// Render projected evidence. Images load only when `exists` is true and the
/// path is non-empty; missing evidence becomes an explicit dashed gap.
#let evidence-list(items) = {
  let items = list-of(items)
  if items.len() == 0 {
    gap(note: "No evidence attached.")
  } else {
    for (index, item) in items.enumerate() {
      let kind = field(item.at("kind", default: "file"), default: "file")
      let path = field(item.at("path", default: ""), default: "")
      let caption = field(item.at("caption", default: ""), default: "")
      let exists = item.at("exists", default: false) == true
      let number = index + 1
      block(width: 100%, above: 6pt, below: 6pt, breakable: false)[
        #if kind == "screenshot" and exists and path != "" {
          figure(
            image(path, width: 80%),
            caption: if caption == "" {
              "Evidence " + str(number)
            } else {
              "Evidence " + str(number) + ": " + caption
            },
          )
        } else if kind == "log" and exists and path != "" {
          block(width: 100%)[
            #text(
              size: 8.5pt,
              weight: "bold",
              fill: palette.muted,
            )[EVIDENCE #number — LOG#if caption != "" [ (#caption)]]
            #v(4pt)
            #log-block(path)
          ]
        } else {
          gap(
            note: if kind == "screenshot" {
              "Screenshot not available on disk."
            } else if kind == "log" {
              "Log not available on disk."
            } else {
              "Attachment (" + kind + ") is recorded but not embedded."
            },
            path: path,
          )
        }
      ]
    }
  }
}

/// Find the design step that belongs to a run step by position.
#let step-info(case, position) = {
  let steps = if case == none {
    ()
  } else {
    list-of(case.at("steps", default: ()))
  }
  steps.find(step => step.at("position", default: -1) == position)
}

#let case-steps-table(steps) = {
  let steps = list-of(steps)
  if steps.len() == 0 {
    gap(note: "No design steps recorded.")
  } else {
    table(
      columns: (auto, 1.35fr, 1fr, 1.25fr),
      inset: (x: 4pt, y: 4pt),
      stroke: 0.5pt + palette.rule,
      table.header([*No.*], [*Action*], [*Data*], [*Expected result*]),
      ..steps
        .map(step => (
          text(size: 8pt, weight: "bold", field(step.at("position", default: ""))),
          text(size: 8pt, field(step.at("action", default: ""))),
          text(size: 8pt, field(step.at("data", default: ""))),
          text(size: 8pt, field(step.at("expected", default: ""))),
        ))
        .flatten(),
    )
  }
}

#let requirements-table(requirements) = {
  let requirements = list-of(requirements)
  if requirements.len() == 0 {
    gap(note: "No requirements linked.")
  } else {
    table(
      columns: (auto, 1fr),
      inset: (x: 4pt, y: 4pt),
      stroke: 0.5pt + palette.rule,
      table.header([*Requirement*], [*Title*]),
      ..requirements
        .map(requirement => (
          text(size: 8pt, weight: "bold", field(requirement.at("id", default: ""))),
          text(size: 8pt, field(requirement.at("title", default: ""))),
        ))
        .flatten(),
    )
  }
}

#let bugs-table(bugs) = {
  let bugs = list-of(bugs)
  if bugs.len() == 0 {
    gap(note: "No bug reports linked.")
  } else {
    table(
      columns: (auto, 1fr, auto, auto, auto),
      inset: (x: 4pt, y: 4pt),
      stroke: 0.5pt + palette.rule,
      table.header(
        [*ID*],
        [*Title*],
        [*Priority*],
        [*Severity*],
        [*Status*],
      ),
      ..bugs
        .map(bug => (
          text(size: 8pt, weight: "bold", field(bug.at("id", default: ""))),
          text(size: 8pt, field(bug.at("title", default: ""))),
          priority-chip(bug.at("priority", default: 3)),
          severity-chip(bug.at("severity", default: 3)),
          status-chip(bug.at("status", default: "open")),
        ))
        .flatten(),
    )
  }
}

#let run-summary-table(runs) = {
  let runs = list-of(runs)
  if runs.len() == 0 {
    gap(note: "No runs recorded for this case.")
  } else {
    table(
      columns: (auto, auto, auto, auto, auto, auto),
      inset: (x: 4pt, y: 4pt),
      stroke: 0.5pt + palette.rule,
      table.header(
        [*Run*],
        [*Mode*],
        [*Status*],
        [*Started*],
        [*Ended*],
        [*Commit*],
      ),
      ..runs
        .map(run => (
          text(size: 8pt, weight: "bold", field(run.at("id", default: ""))),
          text(size: 8pt, field(run.at("mode", default: ""))),
          status-chip(run.at("status", default: "not_run")),
          text(size: 8pt, field(run.at("started_at", default: ""))),
          text(size: 8pt, field(run.at("ended_at", default: ""))),
          text(size: 8pt, field(run.at("commit", default: ""))),
        ))
        .flatten(),
    )
  }
}

/// One execution sheet: metadata, per-step results, and evidence.
/// `show-facts: false` is used when the caller already printed the run header.
#let run-detail(run, case: none, show-facts: true) = {
  let steps = list-of(run.at("steps", default: ()))
  block(width: 100%, breakable: true)[
    #if show-facts [
      #grid(
        columns: (1fr, auto),
        column-gutter: 6pt,
        align: (left, right),
        [#text(size: 11pt, weight: "bold", field(run.at("id", default: "")))],
        [#status-chip(run.at("status", default: "not_run"))],
      )
      #v(4pt)
      #facts((
        ("Mode", field(run.at("mode", default: ""))),
        ("Started", field(run.at("started_at", default: ""))),
        ("Ended", field(run.at("ended_at", default: ""))),
        ("Environment", field(run.at("environment", default: ""))),
        ("Commit", field(run.at("commit", default: ""))),
        ("Command", field(run.at("command", default: ""))),
        ("Notes", field(run.at("notes", default: ""))),
      ))
    ]

    #report-section("Steps", [
      #if steps.len() == 0 [
        #gap(note: "No step results recorded.")
      ] else [
        #for (index, step) in steps.enumerate() {
          let position = step.at("position", default: index + 1)
          let info = step-info(case, position)
          let action = if info == none {
            "Step " + str(position)
          } else {
            field(info.at("action", default: ""), default: "Step " + str(position))
          }
          let data = if info == none { "" } else { field(info.at("data", default: ""), default: "") }
          let expected = if info == none { "" } else { field(info.at("expected", default: ""), default: "") }
          let observed = field(step.at("observed", default: ""), default: "")
          let notes = field(step.at("notes", default: ""), default: "")
          let evidence = list-of(step.at("evidence", default: ()))
          block(
            width: 100%,
            stroke: 0.5pt + palette.rule,
            radius: 2pt,
            inset: 7pt,
            above: 5pt,
            below: 5pt,
            breakable: true,
          )[
            #grid(
              columns: (auto, 1fr, auto),
              column-gutter: 6pt,
              align: (left, left, right),
              [#text(size: 9pt, weight: "bold", str(position) + ".")],
              [#text(size: 9pt, weight: "medium", action)],
              [#status-chip(step.at("status", default: "not_run"))],
            )
            #if data != "" [
              #v(2pt)
              #text(size: 8.5pt)[*Data:* #data]
            ]
            #if expected != "" [
              #v(2pt)
              #text(size: 8.5pt)[*Expected:* #expected]
            ]
            #if observed != "" [
              #v(3pt)
              #text(size: 8.5pt)[*Observed:* #observed]
            ]
            #if notes != "" [
              #v(3pt)
              #text(size: 8.5pt)[*Notes:* #notes]
            ]
            #v(4pt)
            #evidence-list(evidence)
          ]
        }
      ]
    ])

    #let run-evidence = list-of(run.at("evidence", default: ()))
    #if run-evidence.len() > 0 [
      #report-section("Run evidence", [#evidence-list(run-evidence)])
    ]
  ]
}

/// Full-page frame shared by all reports.
#let report-shell(kind: "", title: "", subtitle: "", body) = {
  set document(title: title, author: "Laburo")
  set page(
    paper: "a4",
    margin: (x: 1.9cm, y: 1.7cm),
    footer: context [
      #set text(size: 8pt, fill: palette.muted)
      #kind
      #h(1fr)
      Page #counter(page).display("1 / 1", both: true)
    ],
  )
  set text(size: 10pt, fill: palette.ink)
  set par(leading: 0.65em)
  block(width: 100%, below: 10pt)[
    #text(size: 8.5pt, weight: "medium", fill: palette.brand, upper(kind))
    #v(2pt)
    #text(size: 19pt, weight: "bold", fill: palette.ink, title)
    #if subtitle != "" [
      #v(3pt)
      #text(size: 9.5pt, fill: palette.muted, subtitle)
    ]
    #v(8pt)
    #line(length: 100%, stroke: 1.2pt + palette.brand)
  ]
  body
}
