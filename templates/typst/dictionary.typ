// Class test-case dictionary: every case, with type, level, module, objective,
// and expected result. Reads `data.json` from the report directory.
#import "lib.typ": *

#let data = json("data.json")
#let meta = data.at("meta", default: (:))
#let cases = list-of(data.at("cases", default: ()))
#let project = field(meta.at("project", default: ""), default: "Project")
#let generated = field(meta.at("generated_at", default: ""), default: "")

#let count-status(status) = {
  cases.filter(item => field(item.at("status", default: "")) == status).len()
}

#let tags-of(item) = list-of(item.at("tags", default: ()))

#let tag-in(item, names) = {
  let found = tags-of(item).find(tag => tag in names)
  if found == none { "—" } else { found }
}

#let expected-of(item) = {
  let steps = list-of(item.at("steps", default: ()))
  let values = steps
    .map(step => field(step.at("expected", default: ""), default: ""))
    .filter(value => value != "")
  if values.len() == 0 { "—" } else { values.last() }
}

#show: report-shell.with(
  kind: "Test-case dictionary",
  title: project,
  subtitle: if generated == "" { "Test-case dictionary" } else { "Generated " + generated },
)

#facts((
  ("Project", project),
  ("Cases", str(cases.len())),
  ("Active", str(count-status("active"))),
  ("Draft", str(count-status("draft"))),
  ("Archived", str(count-status("archived"))),
  ("Generated", if generated == "" { "—" } else { generated }),
))

#report-section("Dictionary", [
  #if cases.len() == 0 [
    #gap(note: "No test cases in the catalogue.")
  ] else [
    #table(
      columns: (auto, 1.1fr, auto, auto, auto, 1.1fr, 1.1fr, auto),
      inset: (x: 4pt, y: 4pt),
      stroke: 0.5pt + palette.rule,
      table.header(
        [*ID*],
        [*Title*],
        [*Type*],
        [*Level*],
        [*Module*],
        [*Objective*],
        [*Expected result*],
        [*Status*],
      ),
      ..cases
        .map(item => (
          text(size: 8pt, weight: "bold", field(item.at("id", default: ""))),
          text(size: 8pt, field(item.at("title", default: ""))),
          text(
            size: 8pt,
            tag-in(item, ("functional", "non-functional")),
          ),
          text(size: 8pt, tag-in(item, ("unit", "integration", "system"))),
          text(size: 8pt, field(item.at("module", default: ""))),
          text(size: 8pt, field(item.at("description", default: ""))),
          text(size: 8pt, expected-of(item)),
          status-chip(item.at("status", default: "draft")),
        ))
        .flatten(),
    )
  ]
])

#let traced = cases.filter(item => list-of(item.at("requirements", default: ())).len() > 0)

#report-section("Traceability", [
  #if traced.len() == 0 [
    #gap(note: "No case is linked to a requirement.")
  ] else [
    #table(
      columns: (auto, 1fr, 1fr),
      inset: (x: 4pt, y: 4pt),
      stroke: 0.5pt + palette.rule,
      table.header([*Case*], [*Title*], [*Requirements*]),
      ..traced
        .map(item => (
          text(size: 8pt, weight: "bold", field(item.at("id", default: ""))),
          text(size: 8pt, field(item.at("title", default: ""))),
          text(
            size: 8pt,
            list-of(item.at("requirements", default: ()))
              .map(requirement => field(requirement.at("id", default: "")))
              .join(", "),
          ),
        ))
        .flatten(),
    )
  ]
])
