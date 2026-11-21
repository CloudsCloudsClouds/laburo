// Class bug report. Layout and colours follow
// `src-elizabeth-tours/tqa/Bug_report_template_blank.typ` and
// `Bug-report-template_ejemplo.docx`: blue banner, thin top rule, label/value
// table, class priority/severity wording. Extended with linked TC/run,
// linked items, expected artifact, and evidence-by-role attachments.
#import "lib.typ": *

#let data = json("data.json")
#let meta = data.at("meta", default: (:))
#let case = data.at("case", default: none)
#let runs = list-of(data.at("runs", default: ()))
#let bugs = list-of(data.at("bugs", default: ()))
#let bug = if bugs.len() > 0 { bugs.first() } else { none }
#let bug-run-id = if bug == none {
  ""
} else {
  field(bug.at("run_id", default: ""), default: "")
}
#let run = {
  let direct = runs.find(r => field(r.at("id", default: ""), default: "") == bug-run-id)
  if direct != none {
    direct
  } else if runs.len() > 0 {
    runs.first()
  } else {
    none
  }
}
#let generated = field(meta.at("generated_at", default: ""), default: "")

#let bug-title = if bug == none {
  "Bug report"
} else {
  field(bug.at("id", default: ""), default: "Bug") + " — " + field(bug.at("title", default: ""), default: "Bug report")
}

#set document(title: bug-title, author: "Laburo")
#set page(
  paper: "a4",
  margin: 2.54cm,
  background: place(
    top + left,
    dx: 2.40cm,
    dy: 2.12cm,
    line(length: 16.8cm, stroke: 0.75pt + bugclass.rule),
  ),
)
#set text(
  font: ("Calibri", "Carlito", "Aptos", "Liberation Sans", "DejaVu Sans"),
  size: 10pt,
  lang: "en",
  hyphenate: false,
)
#set par(justify: true, leading: 0.76em)

#v(4.6pt)
#move(dx: -3pt, block(
  width: 100% + 6pt,
  fill: bugclass.banner,
  inset: (x: 3pt, y: 6.2pt),
)[
  #text(size: 11pt, fill: white)[B#text(size: 8.8pt)[UG REPORT]]
])
#v(15.9pt)

#let label-cell(body) = table.cell(
  fill: bugclass.label,
  inset: (x: 5.4pt, y: 4.2pt),
  align: left + horizon,
)[
  #set text(size: 8pt, weight: "bold")
  #body
]

#let value-cell(body) = table.cell(
  inset: (x: 5.4pt, y: 4.2pt),
  align: left + horizon,
)[
  #set text(size: 9.5pt)
  #body
]

#let bug-row(label, body) = (label-cell(label), value-cell(body))

#if bug == none [
  #table(
    columns: (3.1cm, 1fr),
    stroke: 0.5pt + black,
    inset: (x: 0.19cm, y: 4.05pt),
    table.cell(
      inset: (x: 0.19cm, y: 0.5pt),
    )[#block(
      width: 100%,
      fill: bugclass.id,
      inset: (x: 2.5pt, y: 5.2pt),
    )[#text(size: 9pt, weight: "bold")[\#BUG-XX-F]]],
    table.cell(
      inset: (x: 0.19cm, y: 0.5pt),
    )[#block(
      width: 100%,
      fill: bugclass.id,
      inset: (x: 2.5pt, y: 5.2pt),
    )[#text(size: 11pt)[B#text(size: 8.8pt)[UG REPORT]]]],
    ..bug-row("Estado", "—"),
  )
] else [
  #let bug-evidence = list-of(bug.at("evidence", default: ()))
  #let expected-artifact = (
    expected-evidence(case)
      + bug-evidence.filter(item => evidence-role(item, default: "") == "expected")
  )
  #let attachments = bug-evidence.filter(
    item => evidence-role(item, default: "evidence") != "expected",
  )
  #let bug-links = list-of(bug.at("links", default: ()))

  #let linked-case-id = if case != none {
    field(case.at("id", default: ""), default: "")
  } else {
    field(bug.at("case_id", default: ""), default: "")
  }
  #let linked-case-title = if case != none {
    field(case.at("title", default: ""), default: "")
  } else {
    ""
  }
  #let linked-run-id = if run != none {
    field(run.at("id", default: ""), default: "")
  } else {
    field(bug.at("run_id", default: ""), default: "")
  }

  #let reference = {
    let parts = ()
    if linked-case-id != "" { parts.push(linked-case-id) }
    if linked-run-id != "" { parts.push(linked-run-id) }
    parts.join(" · ")
  }

  #table(
    columns: (3.1cm, 1fr),
    stroke: 0.5pt + black,
    inset: (x: 0.19cm, y: 4.05pt),

    table.cell(
      inset: (x: 0.19cm, y: 0.5pt),
    )[#block(
      width: 100%,
      fill: bugclass.id,
      inset: (x: 2.5pt, y: 5.2pt),
    )[#text(size: 9pt, weight: "bold")[#field(bug.at("id", default: ""), default: "#BUG-XX-F")]]],
    table.cell(
      inset: (x: 0.19cm, y: 0.5pt),
    )[#block(
      width: 100%,
      fill: bugclass.id,
      inset: (x: 2.5pt, y: 5.2pt),
    )[#text(size: 11pt)[B#text(size: 8.8pt)[UG REPORT]]]],

    ..bug-row("Title", multiline(bug.at("title", default: ""), default: "—")),
    ..bug-row("Descripción", multiline(bug.at("description", default: ""), default: "—")),
    ..bug-row("Pasos", {
      let steps = multiline(bug.at("steps", default: ""), default: "—")
      if reference == "" {
        steps
      } else {
        [#steps #linebreak() Referencia: #reference]
      }
    }),
    ..bug-row("RESULTADO ACTUAL", multiline(bug.at("actual", default: ""), default: "—")),
    ..bug-row("RESULTADO ESPERADO", {
      let expected-text = multiline(bug.at("expected", default: ""), default: "—")
      if expected-artifact.len() == 0 {
        expected-text
      } else {
        [
          #expected-text
          #v(4pt)
          #evidence-gallery(expected-artifact, default-role: "expected", image-width: 58%)
        ]
      }
    }),
    ..bug-row("PRIORIDAD", "Alta 1-5 → " + str(if type(bug.at("priority", default: 3)) == int { bug.at("priority", default: 3) } else { 3 })),
    ..bug-row("SEVERIDAD", "Crítica 1-5 → " + str(if type(bug.at("severity", default: 3)) == int { bug.at("severity", default: 3) } else { 3 })),
    ..bug-row("AMBIENTE", multiline(bug.at("environment", default: ""), default: "—")),
    ..bug-row("ESTADO", status-chip(bug.at("status", default: "open"))),
    ..bug-row("CASO DE PRUEBA", if linked-case-id == "" {
      "—"
    } else if linked-case-title == "" {
      linked-case-id
    } else {
      [#linked-case-id — #linked-case-title]
    }),
    ..bug-row("EJECUCIÓN", if linked-run-id == "" {
      "—"
    } else {
      [#linked-run-id #h(4pt) #if run != none { status-chip(run.at("status", default: "not_run")) }]
    }),
    ..bug-row("VINCULADOS", if bug-links.len() == 0 {
      "—"
    } else {
      links-block(bug-links)
    }),
    ..bug-row("ADJUNTOS", if attachments.len() == 0 {
      "—"
    } else {
      evidence-gallery(attachments, default-role: "actual", image-width: 62%)
    }),
    ..bug-row("COMENTARIOS", multiline(bug.at("comments", default: ""), default: "—")),
  )
]
