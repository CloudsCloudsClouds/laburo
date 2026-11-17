// Class bug report: one defect, its evidence, and its context.
#import "lib.typ": *

#let data = json("data.json")
#let meta = data.at("meta", default: (:))
#let case = data.at("case", default: none)
#let runs = list-of(data.at("runs", default: ()))
#let bugs = list-of(data.at("bugs", default: ()))
#let bug = if bugs.len() > 0 { bugs.first() } else { none }
#let run = if runs.len() > 0 { runs.first() } else { none }
#let generated = field(meta.at("generated_at", default: ""), default: "")

#let title = if bug == none {
  "Bug report"
} else {
  field(bug.at("id", default: ""), default: "Bug") + " — " + field(bug.at("title", default: ""), default: "Bug report")
}

#show: report-shell.with(
  kind: "Bug report",
  title: title,
  subtitle: if generated == "" { "" } else { "Generated " + generated },
)

#if bug == none [
  #gap(note: "No bug found in data.json.")
] else [
  #facts((
    ("Bug ID", field(bug.at("id", default: ""))),
    ("Status", status-chip(bug.at("status", default: "open"))),
    ("Priority", priority-chip(bug.at("priority", default: 3))),
    ("Severity", severity-chip(bug.at("severity", default: 3))),
    ("Environment", field(bug.at("environment", default: ""))),
    ("Case", if case == none {
      "—"
    } else {
      field(case.at("id", default: "")) + " — " + field(case.at("title", default: ""))
    }),
    ("Run", if run == none { "—" } else { field(run.at("id", default: "")) }),
    ("Generated", if generated == "" { "—" } else { generated }),
  ))

  #let description = field(bug.at("description", default: ""), default: "")
  #report-section("Description", [
    #if description == "" [
      #gap(note: "No description recorded.")
    ] else [
      #description
    ]
  ])

  #let steps = field(bug.at("steps", default: ""), default: "")
  #report-section("Steps to reproduce", [
    #if steps == "" [
      #gap(note: "No reproduction steps recorded.")
    ] else [
      #code-block(steps)
    ]
  ])

  #report-section("Expected vs actual", [
    #table(
      columns: (1fr, 1fr),
      stroke: 0.5pt + palette.rule,
      inset: 8pt,
      [*Expected*], [*Actual*],
      [
        #text(
          size: 9pt,
          fill: palette.good,
          field(bug.at("expected", default: ""), default: "—"),
        )
      ],
      [
        #text(
          size: 9pt,
          fill: palette.bad,
          field(bug.at("actual", default: ""), default: "—"),
        )
      ],
    )
  ])

  #report-section("Evidence", [
    #evidence-list(bug.at("evidence", default: ()))
  ])

  #let comments = field(bug.at("comments", default: ""), default: "")
  #report-section("Comments", [
    #if comments == "" [
      #gap(note: "No comments recorded.")
    ] else [
      #comments
    ]
  ])
]
