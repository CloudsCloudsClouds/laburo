// Case dossier: one case, its design steps, all runs, and linked bugs.
#import "lib.typ": *

#let data = json("data.json")
#let meta = data.at("meta", default: (:))
#let case = data.at("case", default: none)
#let runs = list-of(data.at("runs", default: ()))
#let bugs = list-of(data.at("bugs", default: ()))
#let generated = field(meta.at("generated_at", default: ""), default: "")

#let title = if case == none {
  "Case dossier"
} else {
  field(case.at("id", default: ""), default: "") + " — " + field(case.at("title", default: ""), default: "Case dossier")
}

#show: report-shell.with(
  kind: "Case dossier",
  title: title,
  subtitle: if generated == "" { "" } else { "Generated " + generated },
)

#if case == none [
  #gap(note: "No case found in data.json.")
] else [
  #facts((
    ("Case ID", field(case.at("id", default: ""))),
    ("Status", status-chip(case.at("status", default: ""))),
    ("Module", field(case.at("module", default: ""))),
    ("Priority", chip(field(case.at("priority", default: "")), status: case.at("priority", default: ""))),
    ("Environment", field(case.at("environment", default: ""))),
    ("Tags", tag-list(case.at("tags", default: ()))),
  ))

  #let description = field(case.at("description", default: ""), default: "")
  #if description != "" [
    #report-section("Objective", [#description])
  ]

  #report-section("Design steps", [
    #case-steps-table(case.at("steps", default: ()))
  ])

  #report-section("Gherkin", [
    #let gherkin = field(case.at("gherkin", default: ""), default: "")
    #if gherkin == "" [
      #gap(note: "No Gherkin scenario recorded.")
    ] else [
      #code-block(gherkin)
    ]
  ])

  #report-section("Requirements", [
    #requirements-table(case.at("requirements", default: ()))
  ])

  #report-section("Runs", [
    #run-summary-table(runs)
  ])

  #if runs.len() > 0 [
    #report-section("Execution detail", [
      #for run in runs [
        #run-detail(run, case: case)
        #v(10pt)
      ]
    ])
  ]

  #report-section("Defects", [
    #bugs-table(bugs)
  ])
]
