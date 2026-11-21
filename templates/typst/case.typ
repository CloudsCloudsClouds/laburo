// Case dossier: design (class Part 1) and execution (class Part 2) for one
// case, with expected artifact, per-step actual evidence, links, and defects.
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
  field(case.at("id", default: ""), default: "") + " · " + field(case.at("title", default: ""), default: "Case dossier")
}

#show: report-shell.with(
  kind: "Ficha de caso de prueba",
  title: title,
  subtitle: if generated == "" { "" } else { "Generado " + generated },
  footer: "Ficha de caso de prueba",
)

#if case == none [
  #text(fill: tqa.grey, "—")
] else [
  #let expected = expected-evidence(case)
  #let links = list-of(case.at("links", default: ()))

  // ── Class design ficha ────────────────────────────────────────────────────
  #ficha(
    (
      ("ID del TC", field(case.at("id", default: ""))),
      ("Prioridad", field(case.at("priority", default: ""))),
      ("Título", field(case.at("title", default: ""))),
      ("Tipo", tag-in(case, ("functional", "non-functional", "negativa", "negative", "regression"))),
      ("Módulo", field(case.at("module", default: ""))),
      ("Nivel", tag-in(case, ("unit", "integration", "system"))),
      ("Ambiente", field(case.at("environment", default: ""))),
      ("Estado", status-chip(case.at("last_run_status", default: "not_run"))),
    ),
    columns: 4,
  )

  // ── 1 · Objective ─────────────────────────────────────────────────────────
  #sec("1 · Objetivo")
  #let description = field(case.at("description", default: ""), default: "")
  #if description == "" [
    #text(fill: tqa.grey, "—")
  ] else [
    #description
  ]
  #if list-of(case.at("tags", default: ())).len() > 0 [
    #v(4pt)
    #tag-list(case.at("tags", default: ()))
  ]

  // ── 2 · Design steps ──────────────────────────────────────────────────────
  #sec("2 · Pasos y resultados esperados")
  #case-steps-table(case.at("steps", default: ()))

  #let gherkin = field(case.at("gherkin", default: ""), default: "")
  #if gherkin != "" [
    #subsec("Gherkin")
    #block(
      width: 100%,
      fill: tqa.zebra,
      inset: 7pt,
      stroke: 0.5pt + black,
      breakable: true,
    )[
      #raw(gherkin, lang: "gherkin", block: true)
    ]
  ]

  // ── 3 · Expected result + expected artifact ───────────────────────────────
  #sec("3 · Resultado esperado final")
  #ficha(
    (
      ("Criterio de éxito", expected-of(case)),
      ("Evidencia esperada", str(expected.len()) + " artefacto(s)"),
    ),
    columns: 2,
  )
  #if expected.len() > 0 [
    #v(6pt)
    #evidence-gallery(expected, default-role: "expected")
  ]

  // ── 4 · Traceability ──────────────────────────────────────────────────────
  #sec("4 · Trazabilidad")
  #requirements-table(case.at("requirements", default: ()))
  #if links.len() > 0 [
    #v(8pt)
    #subsec("Elementos vinculados")
    #links-table(links)
  ]

  // ── 5 · Execution history ─────────────────────────────────────────────────
  #sec("5 · Historial de ejecución")
  #run-summary-table(runs)

  // ── 6 · Execution detail (class Part 2, new page) ─────────────────────────
  #if runs.len() > 0 [
    #pagebreak()
    #sec("Ejecución · " + field(case.at("id", default: ""), default: ""), color: tqa.green)
    #for run in runs [
      #run-detail(run, case: case)
      #v(12pt)
    ]
  ]

  // ── 7 · Defects ───────────────────────────────────────────────────────────
  #sec("7 · Defectos", color: tqa.red)
  #bugs-table(bugs)
]
