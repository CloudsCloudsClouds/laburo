// Shared class style for Laburo reports.
//
// Visual language and colours come from
// `src-elizabeth-tours/tqa/plantilla_tc_tqa.typ` (TQA class template).
// The bug report keeps its own class palette from
// `Bug_report_template_blank.typ` / `Bug-report-template_ejemplo.docx`.
//
// Every active template imports this file and reads `data.json` from the same
// directory. Helpers are defensive: a missing field renders an em dash or an
// omitted block, never a compile error.

// ── Class palette (plantilla_tc_tqa.typ) ─────────────────────────────────────
#let tqa = (
  ink: rgb("#182030"),     // base text and label fill
  blue: rgb("#2D6CDF"),
  green: rgb("#2D9B64"),
  red: rgb("#CD4646"),
  amber: rgb("#EB912D"),
  grey: rgb("#647080"),    // values / placeholders
  zebra: rgb("#F5F8FC"),   // value + even-row fill
  design: rgb("#E8EEF7"),  // "01 · design" box
  exec: rgb("#DDF3EE"),    // "02 · execution" box
)

// ── Class palette (bug report template) ──────────────────────────────────────
#let bugclass = (
  banner: rgb("#4472c4"),
  rule: rgb("#4f81bd"),
  id: rgb("#d9e2f3"),
  label: rgb("#dddddd"),
)

// ── Class fonts ──────────────────────────────────────────────────────────────
#let f-body = ("Aptos", "Calibri", "Carlito", "Liberation Sans", "DejaVu Sans")
#let f-title = ("Aptos Display",) + f-body

// ── Defensive accessors ──────────────────────────────────────────────────────
/// Render a value safely: `none` and blank strings become `default`.
#let field(value, default: "—") = {
  if value == none {
    default
  } else if type(value) == str {
    let text = value.trim()
    if text == "" { default } else { text }
  } else if type(value) == content {
    value
  } else {
    str(value)
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

/// Split a multi-line value into rendered lines (class rows are line-based).
#let multiline(value, default: "—") = {
  let text = field(value, default: default)
  if type(text) != str {
    text
  } else {
    text.split("\n").map(line => [#line]).join(linebreak())
  }
}

/// Class status keys: PASS / FAIL / BLOCKED / NOT RUN.
#let status-key(status) = {
  let key = lower(str(field(status, default: "")))
    .replace("_", "-")
    .replace(" ", "-")
  if key in ("pass", "passed", "green", "ok", "resolved", "closed", "active") {
    "passed"
  } else if key in ("fail", "failed", "error", "red", "open") {
    "failed"
  } else if key in ("blocked", "block", "amber", "reopened", "running") {
    "blocked"
  } else {
    "not-run"
  }
}

#let status-label(status) = {
  let key = status-key(status)
  if key == "passed" { "PASS" }
  else if key == "failed" { "FAIL" }
  else if key == "blocked" { "BLOCKED" }
  else { "NOT RUN" }
}

#let status-color(status) = {
  let key = status-key(status)
  if key == "passed" { tqa.green }
  else if key == "failed" { tqa.red }
  else if key == "blocked" { tqa.amber }
  else { tqa.grey }
}

/// Small class-style status tag (square, white bold text on class colour).
#let status-chip(status) = box(
  fill: status-color(status),
  inset: (x: 4.5pt, y: 1.8pt),
  text(size: 7.8pt, weight: "bold", fill: white, status-label(status)),
)

#let priority-name(priority) = {
  let p = if type(priority) == int { priority } else { 3 }
  if p <= 1 { "Alta" } else if p == 2 { "Media" } else if p == 3 { "Baja" } else { "Baja" }
}

#let priority-chip(priority) = {
  let p = if type(priority) == int { priority } else { 3 }
  let color = if p <= 1 { tqa.red } else if p == 2 { tqa.amber } else { tqa.grey }
  box(
    fill: color,
    inset: (x: 4.5pt, y: 1.8pt),
    text(size: 7.8pt, weight: "bold", fill: white, "P" + str(p)),
  )
}

#let severity-chip(severity) = {
  let s = if type(severity) == int { severity } else { 3 }
  let color = if s <= 1 { tqa.red } else if s == 2 { tqa.amber } else { tqa.grey }
  box(
    fill: color,
    inset: (x: 4.5pt, y: 1.8pt),
    text(size: 7.8pt, weight: "bold", fill: white, "S" + str(s)),
  )
}

// ── Class components ─────────────────────────────────────────────────────────
#let sec(title, color: tqa.blue) = block(
  above: 12pt,
  below: 5pt,
  sticky: true,
  text(font: f-title, size: 17pt, weight: "bold", fill: color, title),
)

#let subsec(title) = block(
  above: 9pt,
  below: 4pt,
  sticky: true,
  text(size: 13pt, weight: "bold", fill: tqa.ink, title),
)

/// Class "ficha": dark label cells + pale value cells.
/// `pairs` is an array of `(label, value)`. `columns` must be even (2 or 4).
#let ficha(pairs, columns: 2, size: 8.5pt) = {
  let cells = ()
  for pair in pairs {
    let label = pair.at(0)
    let value = pair.at(1)
    cells.push(table.cell(
      fill: tqa.ink,
      align: center + horizon,
      inset: (x: 5.4pt, y: 3.2pt),
      text(size: size, weight: "bold", fill: white, upper(str(label))),
    ))
    cells.push(table.cell(
      fill: tqa.zebra,
      align: left + horizon,
      inset: (x: 5.4pt, y: 3.2pt),
      text(size: size, fill: tqa.grey, value),
    ))
  }
  table(
    columns: (1fr,) * columns,
    stroke: 0.5pt + black,
    inset: (x: 5.4pt, y: 0pt),
    align: horizon,
    ..cells,
  )
}

/// Class "registro": coloured header, zebra rows, optional centred columns.
#let registro(
  color,
  header,
  rows,
  columns: none,
  centered: (),
  size: 8pt,
  header-size: 8.5pt,
) = {
  let cols = if columns == none { (1fr,) * header.len() } else { columns }
  let head = header.map(title => table.cell(
    fill: color,
    align: center + horizon,
    inset: (x: 5.4pt, y: 3.6pt),
    text(size: header-size, weight: "bold", fill: white, title),
  ))
  let body = rows
    .enumerate()
    .map(((row-index, row)) => row
      .enumerate()
      .map(((column-index, cell)) => table.cell(
        fill: if calc.even(row-index) { white } else { tqa.zebra },
        align: (if column-index in centered { center } else { left }) + horizon,
        inset: (x: 5.4pt, y: 3.6pt),
        text(size: size, fill: tqa.grey, field(cell)),
      )))
    .flatten()
  table(
    columns: cols,
    stroke: 0.5pt + black,
    inset: (x: 5.4pt, y: 0pt),
    align: horizon,
    table.header(..head),
    ..body,
  )
}

/// Class execution-result strip: four coloured states, current state ticked.
#let status-buttons(status) = {
  let current = status-key(status)
  let states = (
    ("PASS", tqa.green, "passed"),
    ("FAIL", tqa.red, "failed"),
    ("BLOCKED", tqa.amber, "blocked"),
    ("NOT RUN", tqa.grey, "not-run"),
  )
  table(
    columns: (1fr,) * 4,
    stroke: 0.5pt + black,
    inset: 0pt,
    ..states.map(((label, color, key)) => table.cell(
      fill: color,
      align: center + horizon,
      inset: (x: 0pt, y: 5pt),
      text(
        size: 10pt,
        weight: "bold",
        fill: white,
        (if key == current { "☑ " } else { "☐ " }) + label,
      ),
    )),
  )
}

#let tag-list(tags) = {
  let tags = list-of(tags)
  if tags.len() == 0 {
    text(fill: tqa.grey, "—")
  } else {
    tags
      .map(tag => box(
        fill: tqa.design,
        inset: (x: 4pt, y: 1.5pt),
        text(size: 8pt, fill: tqa.blue, field(tag)),
      ))
      .join(h(4pt))
  }
}

#let tag-in(item, names) = {
  let found = list-of(item.at("tags", default: ())).find(tag => tag in names)
  if found == none { "—" } else { found }
}

#let expected-of(item) = {
  let steps = list-of(item.at("steps", default: ()))
  let values = steps
    .map(step => field(step.at("expected", default: ""), default: ""))
    .filter(value => value != "")
  if values.len() == 0 { "—" } else { values.last() }
}

// ── Evidence and links ───────────────────────────────────────────────────────
#let evidence-role(item, default: "evidence") = {
  field(item.at("role", default: default), default: default)
}

/// Case-level expected artifact: `case.expected_evidence` first, then
/// `case.evidence` entries whose role is `expected`.
#let expected-evidence(case, default: ()) = {
  if case == none {
    default
  } else {
    let direct = list-of(case.at("expected_evidence", default: ()))
    if direct.len() > 0 {
      direct
    } else {
      list-of(case.at("evidence", default: ()))
        .filter(item => evidence-role(item, default: "") == "expected")
    }
  }
}

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
  block(
    width: 100%,
    fill: tqa.zebra,
    inset: 6pt,
    stroke: 0.5pt + black,
    breakable: true,
  )[
    #raw(shown, lang: "text", block: true)
  ]
}

#let evidence-item(item, number, default-role: "evidence", image-width: 74%) = {
  let kind = lower(field(item.at("kind", default: "file"), default: "file"))
  let path = field(item.at("path", default: ""), default: "")
  let caption = field(item.at("caption", default: ""), default: "")
  let role = upper(evidence-role(item, default: default-role))
  let exists = item.at("exists", default: false) == true
  let label = if caption == "" {
    role + " " + str(number)
  } else {
    role + " " + str(number) + " — " + caption
  }
  if kind == "screenshot" and exists and path != "" {
    figure(image(path, width: image-width), caption: label)
  } else if kind == "log" and exists and path != "" {
    block(width: 100%, above: 5pt, below: 6pt, breakable: true)[
      #text(size: 8pt, weight: "bold", fill: tqa.grey)[#label]
      #v(3pt)
      #log-block(path)
    ]
  } else {
    block(
      width: 100%,
      above: 4pt,
      below: 4pt,
      fill: tqa.zebra,
      inset: 6pt,
      stroke: 0.5pt + black,
      breakable: true,
    )[
      #text(size: 8pt, weight: "bold", fill: tqa.grey)[#label]
      #if path != "" [
        #v(2pt)
        #text(size: 7.5pt, fill: tqa.grey)[#path]
      ]
    ]
  }
}

/// Render evidence items. Returns `none` when there is nothing to show so the
/// caller can omit the whole block from the clean final report.
#let evidence-gallery(items, default-role: "evidence", image-width: 74%) = {
  let items = list-of(items)
  if items.len() == 0 {
    none
  } else {
    items
      .enumerate()
      .map(((index, item)) => evidence-item(
        item,
        index + 1,
        default-role: default-role,
        image-width: image-width,
      ))
      .join()
  }
}

#let link-kind(link) = field(
  link.at("target_kind", default: link.at("kind", default: link.at("source_kind", default: ""))),
  default: "",
)

#let link-id(link) = field(
  link.at("target_id", default: link.at("id", default: link.at("source_id", default: ""))),
  default: "",
)

#let link-label(link) = {
  let kind = link-kind(link)
  let id = link-id(link)
  let title = field(link.at("title", default: ""), default: "")
  let target = if kind == "" { id } else if id == "" { kind } else { kind + " " + id }
  if title == "" { field(target, default: "—") } else { target + " — " + title }
}

#let links-table(links) = {
  let links = list-of(links)
  if links.len() == 0 {
    text(fill: tqa.grey, "—")
  } else {
    registro(
      tqa.amber,
      ("Relation", "Linked item", "Note"),
      links
        .map(link => (
          field(link.at("relation", default: "related"), default: "related"),
          link-label(link),
          field(link.at("note", default: ""), default: "—"),
        )),
      columns: (auto, 1.25fr, 1fr),
      centered: (0,),
      size: 8pt,
    )
  }
}

#let links-block(links) = {
  let links = list-of(links)
  if links.len() == 0 {
    text(fill: tqa.grey, "—")
  } else {
    links
      .map(link => [
        #text(size: 8.5pt, weight: "bold")[#field(link.at("relation", default: "related"))]
        #h(4pt)
        #text(size: 8.5pt)[#link-label(link)]
        #let note = field(link.at("note", default: ""), default: "")
        #if note != "" [#linebreak() #text(size: 8pt, fill: tqa.grey)[#note]]
      ])
      .join(v(3pt))
  }
}

// ── Shared tables ────────────────────────────────────────────────────────────
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
    text(fill: tqa.grey, "—")
  } else {
    registro(
      tqa.blue,
      ("#", "Paso / acción", "Dato utilizado", "Resultado esperado"),
      steps
        .map(step => (
          field(step.at("position", default: ""), default: ""),
          field(step.at("action", default: ""), default: "—"),
          field(step.at("data", default: ""), default: "—"),
          field(step.at("expected", default: ""), default: "—"),
        )),
      columns: (0.45fr, 1.35fr, 1fr, 1.3fr),
      centered: (0,),
      size: 8pt,
    )
  }
}

#let requirements-table(requirements) = {
  let requirements = list-of(requirements)
  if requirements.len() == 0 {
    text(fill: tqa.grey, "—")
  } else {
    registro(
      tqa.blue,
      ("Requisito", "Título"),
      requirements
        .map(requirement => (
          field(requirement.at("id", default: ""), default: "—"),
          field(requirement.at("title", default: ""), default: "—"),
        )),
      columns: (auto, 1fr),
      centered: (0,),
      size: 8pt,
    )
  }
}

#let bugs-table(bugs) = {
  let bugs = list-of(bugs)
  if bugs.len() == 0 {
    text(fill: tqa.grey, "—")
  } else {
    registro(
      tqa.red,
      ("ID", "Título", "Prioridad", "Severidad", "Estado"),
      bugs
        .map(bug => (
          field(bug.at("id", default: ""), default: "—"),
          field(bug.at("title", default: ""), default: "—"),
          priority-chip(bug.at("priority", default: 3)),
          severity-chip(bug.at("severity", default: 3)),
          status-chip(bug.at("status", default: "open")),
        )),
      columns: (auto, 1fr, auto, auto, auto),
      centered: (0, 2, 3, 4),
      size: 8pt,
    )
  }
}

#let run-summary-table(runs) = {
  let runs = list-of(runs)
  if runs.len() == 0 {
    text(fill: tqa.grey, "—")
  } else {
    registro(
      tqa.green,
      ("Run", "Modo", "Estado", "Inicio", "Fin", "Commit"),
      runs
        .map(run => (
          field(run.at("id", default: ""), default: "—"),
          field(run.at("mode", default: ""), default: "—"),
          status-chip(run.at("status", default: "not_run")),
          field(run.at("started_at", default: ""), default: "—"),
          field(run.at("ended_at", default: ""), default: "—"),
          field(run.at("commit", default: ""), default: "—"),
        )),
      columns: (auto, auto, auto, auto, auto, auto),
      centered: (1, 2),
      size: 7.5pt,
    )
  }
}

// ── Execution sheet body (shared by case + run reports) ──────────────────────
#let run-detail(run, case: none, show-facts: true) = {
  let steps = list-of(run.at("steps", default: ()))
  let run-evidence = list-of(run.at("evidence", default: ()))
  let links = list-of(run.at("links", default: ()))
  block(width: 100%, breakable: true)[
    #if show-facts [
      #grid(
        columns: (1fr, auto),
        column-gutter: 6pt,
        align: (left, right),
        [#text(size: 12pt, weight: "bold", field(run.at("id", default: "")))],
        [#status-chip(run.at("status", default: "not_run"))],
      )
      #v(4pt)
      #ficha(
        (
          ("Modo", field(run.at("mode", default: ""))),
          ("Inicio", field(run.at("started_at", default: ""))),
          ("Fin", field(run.at("ended_at", default: ""))),
          ("Ambiente", field(run.at("environment", default: ""))),
          ("Commit", field(run.at("commit", default: ""))),
          ("Comando", field(run.at("command", default: ""))),
          ("Notas", field(run.at("notes", default: ""))),
          ("Caso", if case == none { "—" } else { field(case.at("id", default: "")) }),
        ),
        columns: 4,
        size: 8pt,
      )
    ]

    #subsec("Registro de ejecución por paso")
    #if steps.len() == 0 [
      #text(fill: tqa.grey, "—")
    ] else [
      #registro(
        tqa.green,
        ("#", "Paso ejecutado", "Resultado observado", "Estado", "Observación"),
        steps
          .map(step => {
            let position = step.at("position", default: 0)
            let info = step-info(case, position)
            let action = if info == none {
              "Paso " + str(position)
            } else {
              field(info.at("action", default: ""), default: "Paso " + str(position))
            }
            (
              position,
              action,
              field(step.at("observed", default: ""), default: "—"),
              status-chip(step.at("status", default: "not_run")),
              field(step.at("notes", default: ""), default: "—"),
            )
          }),
        columns: (auto, 1.25fr, 1.5fr, auto, 1fr),
        centered: (0, 3),
        size: 7.5pt,
      )

      #let steps-with-evidence = steps.filter(
        step => list-of(step.at("evidence", default: ())).len() > 0,
      )
      #if steps-with-evidence.len() > 0 [
        #subsec("Evidencia por paso")
        #for step in steps-with-evidence {
          let position = step.at("position", default: 0)
          let info = step-info(case, position)
          let action = if info == none {
            "Paso " + str(position)
          } else {
            field(info.at("action", default: ""), default: "Paso " + str(position))
          }
          block(width: 100%, above: 5pt, below: 4pt, breakable: true)[
            #text(size: 8.5pt, weight: "bold", fill: tqa.ink)[
              Paso #position · #action #h(4pt) #status-chip(step.at("status", default: "not_run"))
            ]
            #evidence-gallery(step.at("evidence", default: ()), default-role: "actual")
          ]
        }
      ]
    ]

    #if run-evidence.len() > 0 [
      #subsec("Artefactos de la ejecución")
      #evidence-gallery(run-evidence, default-role: "artifact")
    ]

    #if links.len() > 0 [
      #subsec("Elementos vinculados")
      #links-table(links)
    ]
  ]
}

// ── Report shell (class page, fonts, headings) ───────────────────────────────
#let report-shell(kind: "", title: "", subtitle: "", footer: "", body) = {
  set document(title: title, author: "Laburo")
  set page(
    paper: "us-letter",
    margin: (top: 39.6pt, bottom: 45.6pt, x: 43.2pt),
    footer-descent: 0pt,
    footer: align(center, text(size: 8pt, fill: tqa.grey)[TQA • #if footer == "" { kind } else { footer }]),
  )
  set text(
    font: f-body,
    size: 9.5pt,
    fill: tqa.ink,
    lang: "en",
    top-edge: 0.92em,
    bottom-edge: -0.28em,
  )
  set par(leading: 0.18em, spacing: 10pt)
  set heading(numbering: none)
  show heading: it => block(
    above: 8pt,
    below: 4pt,
    sticky: true,
    text(font: f-title, size: 17pt, weight: "bold", fill: tqa.blue, it.body),
  )
  set list(marker: [•], indent: 0pt, body-indent: 14pt, tight: true)

  block(width: 100%, below: 10pt)[
    #text(size: 8.5pt, weight: "bold", fill: tqa.blue, upper(kind))
    #v(2pt)
    #text(font: f-title, size: 22pt, weight: "bold", fill: tqa.ink, title)
    #if subtitle != "" [
      #v(3pt)
      #text(size: 11pt, weight: "bold", fill: tqa.blue, subtitle)
    ]
    #v(7pt)
  ]
  body
}
