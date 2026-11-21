// Execution sheet: one run, its steps, per-step actual evidence, run
// artifacts, links, and defects. Class Part 2 visual language.
#import "lib.typ": *

#let data = json("data.json")
#let meta = data.at("meta", default: (:))
#let case = data.at("case", default: none)
#let runs = list-of(data.at("runs", default: ()))
#let bugs = list-of(data.at("bugs", default: ()))
#let run = if runs.len() > 0 { runs.first() } else { none }
#let generated = field(meta.at("generated_at", default: ""), default: "")

#let title = if run == none {
  "Execution sheet"
} else {
  field(run.at("id", default: ""), default: "Execution sheet")
}

#let subtitle = if case == none {
  if generated == "" { "" } else { "Generado " + generated }
} else {
  "Caso " + field(case.at("id", default: ""), default: "") + " · " + field(case.at("title", default: ""), default: "")
}

#show: report-shell.with(
  kind: "Hoja de ejecución",
  title: title,
  subtitle: subtitle,
  footer: "Hoja de ejecución",
)

#if run == none [
  #text(fill: tqa.grey, "—")
] else [
  #let expected = expected-evidence(case)
  #let gherkin = if case == none { "" } else { field(case.at("gherkin", default: ""), default: "") }
  #let case-requirements = if case == none {
    ()
  } else {
    list-of(case.at("requirements", default: ()))
  }

  // ── Class execution ficha ─────────────────────────────────────────────────
  #ficha(
    (
      ("ID del run", field(run.at("id", default: ""))),
      ("Estado", status-chip(run.at("status", default: "not_run"))),
      ("Caso", if case == none {
        "—"
      } else {
        field(case.at("id", default: "")) + " · " + field(case.at("title", default: ""))
      }),
      ("Modo", field(run.at("mode", default: ""))),
      ("Inicio", field(run.at("started_at", default: ""))),
      ("Fin", field(run.at("ended_at", default: ""))),
      ("Ambiente", field(run.at("environment", default: ""))),
      ("Commit", field(run.at("commit", default: ""))),
      ("Comando", field(run.at("command", default: ""))),
      ("Notas", field(run.at("notes", default: ""))),
      ("Generado", if generated == "" { "—" } else { generated }),
      ("Proyecto", field(meta.at("project", default: ""), default: "—")),
    ),
    columns: 4,
  )

  // ── 1 · Result ────────────────────────────────────────────────────────────
  #sec("1 · Resultado de la ejecución", color: tqa.green)
  #status-buttons(run.at("status", default: "not_run"))

  // ── 2 · Case under test ───────────────────────────────────────────────────
  #if case != none [
    #sec("2 · Caso bajo prueba")
    #ficha(
      (
        ("ID del TC", field(case.at("id", default: ""))),
        ("Título", field(case.at("title", default: ""))),
        ("Módulo", field(case.at("module", default: ""))),
        ("Prioridad", field(case.at("priority", default: ""))),
        ("Estado", status-chip(case.at("last_run_status", default: "not_run"))),
        ("Nivel", tag-in(case, ("unit", "integration", "system"))),
        ("Ambiente", field(case.at("environment", default: ""))),
        ("Tags", tag-list(case.at("tags", default: ()))),
      ),
      columns: 4,
    )
    #if case-requirements.len() > 0 [
      #subsec("Requisitos vinculados")
      #requirements-table(case-requirements)
    ]
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
    #if expected.len() > 0 [
      #subsec("Artefacto esperado")
      #evidence-gallery(expected, default-role: "expected")
    ]
  ]

  // ── 3 · Execution ─────────────────────────────────────────────────────────
  #sec("3 · Registro de ejecución", color: tqa.green)
  #run-detail(run, case: case, show-facts: false)

  // ── 4 · Defects ───────────────────────────────────────────────────────────
  #sec("4 · Defectos", color: tqa.red)
  #bugs-table(bugs)
]
