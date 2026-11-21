// Class test-case dictionary: cover, summary, dictionary by module, and
// traceability. Visual language from `plantilla_tc_tqa.typ` /
// `diccionario_casos_de_prueba.typ`. No legend, instructions, or working notes.
#import "lib.typ": *

#let data = json("data.json")

#let meta = data.at("meta", default: (:))
#let cases = list-of(data.at("cases", default: ()))
#let project = field(meta.at("project", default: ""), default: "Project")
#let generated = field(meta.at("generated_at", default: ""), default: "")
#let version = field(meta.at("version", default: ""), default: "")

#let count-status(status) = {
  cases.filter(item => lower(field(item.at("status", default: ""), default: "")) == status).len()
}

#let with-expected-artifact = {
  cases.filter(item => expected-evidence(item).len() > 0).len()
}

#let modules = {
  let seen = ()
  for item in cases {
    let name = field(item.at("module", default: ""), default: "General")
    if not seen.contains(name) { seen.push(name) }
  }
  seen
}

#let traced = cases.filter(item => {
  let requirements = list-of(item.at("requirements", default: ()))
  let links = list-of(item.at("links", default: ()))
  requirements.len() + links.len() > 0
})

#show: report-shell.with(
  kind: "Test-case dictionary",
  title: "Test-case dictionary",
  subtitle: project,
  footer: "Diccionario de casos de prueba",
)

// ── Class cover ──────────────────────────────────────────────────────────────
#v(16pt)
#block(width: 100%, below: 12pt)[
  #text(size: 9pt, fill: tqa.grey)[#project]
  #if generated != "" [#h(6pt) #text(size: 9pt, fill: tqa.grey)[· #generated]]
  #if version != "" [#h(6pt) #text(size: 9pt, fill: tqa.grey)[· v#version]]
]

#table(
  columns: (1fr,) * 3,
  stroke: 0.5pt + black,
  inset: 0pt,
  table.cell(
    fill: tqa.design,
    align: center + horizon,
    inset: (x: 6pt, y: 14pt),
    text(size: 11pt, weight: "bold")[01 \ DISEÑO],
  ),
  table.cell(
    fill: tqa.exec,
    align: center + horizon,
    inset: (x: 6pt, y: 14pt),
    text(size: 11pt, weight: "bold")[02 \ EJECUCIÓN],
  ),
  table.cell(
    fill: tqa.amber,
    align: center + horizon,
    inset: (x: 6pt, y: 14pt),
    text(size: 11pt, weight: "bold")[03 \ TRAZABILIDAD],
  ),
)

#pagebreak()

// ── 1 · Summary ──────────────────────────────────────────────────────────────
#sec("1 · Resumen")

#ficha(
  (
    ("Proyecto", project),
    ("Casos de prueba", str(cases.len())),
    ("Activos", str(count-status("active"))),
    ("Borradores", str(count-status("draft"))),
    ("Archivados", str(count-status("archived"))),
    ("Con evidencia esperada", str(with-expected-artifact)),
    ("Módulos", str(modules.len())),
    ("Generado", if generated == "" { "—" } else { generated }),
  ),
  columns: 4,
)

// ── 2 · Dictionary ───────────────────────────────────────────────────────────
#sec("2 · Diccionario de casos de prueba")

#if cases.len() == 0 [
  #text(fill: tqa.grey, "—")
] else [
  #for module in modules {
    let rows = cases
      .filter(item => field(item.at("module", default: ""), default: "General") == module)
      .map(item => (
        field(item.at("id", default: ""), default: "—"),
        field(item.at("title", default: ""), default: "—"),
        tag-in(item, ("functional", "non-functional", "negativa", "negative", "regression")),
        tag-in(item, ("unit", "integration", "system")),
        field(item.at("priority", default: ""), default: "—"),
        field(item.at("description", default: ""), default: "—"),
        expected-of(item),
        status-chip(item.at("last_run_status", default: "not_run")),
      ))
    subsec(module)
    registro(
      tqa.blue,
      ("TC", "Título", "Tipo", "Nivel", "Prioridad", "Objetivo", "Resultado esperado", "Estado"),
      rows,
      columns: (auto, 1.1fr, auto, auto, auto, 1.15fr, 1.15fr, auto),
      centered: (0, 2, 3, 4, 7),
      size: 7.5pt,
    )
  }
]

// ── 3 · Traceability ────────────────────────────────────────────────────────
#sec("3 · Trazabilidad")

#if traced.len() == 0 [
  #text(fill: tqa.grey, "—")
] else [
  #registro(
    tqa.amber,
    ("TC", "Título", "Requisitos", "Elementos vinculados"),
    traced.map(item => (
      field(item.at("id", default: ""), default: "—"),
      field(item.at("title", default: ""), default: "—"),
      list-of(item.at("requirements", default: ()))
        .map(requirement => field(requirement.at("id", default: ""), default: "—"))
        .join(", "),
      list-of(item.at("links", default: ()))
        .map(link => field(link.at("relation", default: "related"), default: "related")
          + ": "
          + link-label(link))
        .join(", "),
    )),
    columns: (auto, 1.1fr, 1.2fr, 1.2fr),
    centered: (0,),
    size: 8pt,
  )
]
