// Execution sheet: one run, its steps, observations, evidence, and defects.
#import "lib.typ": *

#let data = json("data.json")
#let meta = data.at("meta", default: (:))
#let case = data.at("case", default: none)
#let runs = list-of(data.at("runs", default: ()))
#let bugs = list-of(data.at("bugs", default: ()))
#let run = if runs.len() > 0 { runs.first() } else { none }
#let generated = field(meta.at("generated_at", default: ""), default: "")

#let title = if run == none {
  "Run sheet"
} else {
  field(run.at("id", default: ""), default: "Run sheet")
}

#let subtitle = if case == none {
  if generated == "" { "" } else { "Generated " + generated }
} else {
  "Case " + field(case.at("id", default: ""), default: "") + " — " + field(case.at("title", default: ""), default: "")
}

#show: report-shell.with(
  kind: "Execution sheet",
  title: title,
  subtitle: subtitle,
)

#if run == none [
  #gap(note: "No run found in data.json.")
] else [
  #facts((
    ("Run ID", field(run.at("id", default: ""))),
    ("Status", status-chip(run.at("status", default: "not_run"))),
    ("Case", if case == none {
      "—"
    } else {
      field(case.at("id", default: "")) + " — " + field(case.at("title", default: ""))
    }),
    ("Mode", field(run.at("mode", default: ""))),
    ("Started", field(run.at("started_at", default: ""))),
    ("Ended", field(run.at("ended_at", default: ""))),
    ("Environment", field(run.at("environment", default: ""))),
    ("Commit", field(run.at("commit", default: ""))),
    ("Command", field(run.at("command", default: ""))),
    ("Notes", field(run.at("notes", default: ""))),
    ("Generated", if generated == "" { "—" } else { generated }),
  ))

  #if case != none [
    #report-section("Case under test", [
      #facts((
        ("Case ID", field(case.at("id", default: ""))),
        ("Title", field(case.at("title", default: ""))),
        ("Module", field(case.at("module", default: ""))),
        ("Priority", chip(field(case.at("priority", default: "")), status: case.at("priority", default: ""))),
        ("Tags", tag-list(case.at("tags", default: ()))),
      ))
      #let gherkin = field(case.at("gherkin", default: ""), default: "")
      #if gherkin != "" [
        #v(4pt)
        #code-block(gherkin)
      ]
    ])
  ]

  #report-section("Execution", [
    #run-detail(run, case: case, show-facts: false)
  ])

  #report-section("Defects", [
    #bugs-table(bugs)
  ])
]
