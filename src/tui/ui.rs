//! Rendering for the Laburo TUI. Views read `App`; they never touch the DB.

use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Color, Modifier, Style};
use ratatui::text::{Line, Span};
use ratatui::widgets::{
    Block, Borders, Cell, Clear, Gauge, List, ListItem, ListState, Padding, Paragraph, Row, Table,
    Wrap,
};

use crate::model::{
    Attachment, BugReport, DiscoveredTest, RunStatus, StepRun, TestCase, TestRun, TestStep,
};
use crate::tui::app::{App, CaseDetail, CaseRow, FormState, MessageLevel, RunDetail, Screen, Tab};

const BG: Color = Color::Rgb(13, 15, 20);
const HEADER_BG: Color = Color::Rgb(18, 21, 28);
const PANEL_BG: Color = Color::Rgb(21, 24, 32);
const SELECTED: Color = Color::Rgb(33, 41, 56);
const BORDER: Color = Color::Rgb(45, 51, 64);
const TEXT: Color = Color::Rgb(216, 220, 231);
const DIM: Color = Color::Rgb(122, 130, 150);
const ACCENT: Color = Color::Rgb(112, 202, 255);
const ACCENT2: Color = Color::Rgb(242, 184, 108);
const GREEN: Color = Color::Rgb(120, 210, 150);
const RED: Color = Color::Rgb(242, 112, 124);
const MAGENTA: Color = Color::Rgb(204, 134, 226);
const BLUE: Color = Color::Rgb(138, 170, 250);

fn base() -> Style {
    Style::default().bg(BG).fg(TEXT)
}

fn dim() -> Style {
    Style::default().fg(DIM)
}

fn panel() -> Style {
    Style::default().bg(PANEL_BG).fg(TEXT)
}

fn pane_block(title: &str) -> Block<'static> {
    Block::default()
        .borders(Borders::TOP)
        .border_style(Style::default().fg(BORDER))
        .style(panel())
        .padding(Padding::horizontal(1))
        .title(Line::from(Span::styled(
            format!(" {title} "),
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        )))
}

pub(crate) fn draw(app: &mut App, frame: &mut Frame<'_>) {
    let area = frame.area();
    frame.render_widget(Block::default().style(base()), area);

    if area.width < 56 || area.height < 15 {
        frame.render_widget(
            Paragraph::new("Laburo needs a larger terminal\nresize to at least 56 x 15")
                .alignment(Alignment::Center)
                .style(dim()),
            area,
        );
        return;
    }

    let layout = Layout::vertical([
        Constraint::Length(2),
        Constraint::Min(1),
        Constraint::Length(2),
    ])
    .split(area);

    draw_header(app, frame, layout[0]);

    if app.manual.is_some() {
        draw_manual(app, frame, layout[1]);
    } else if app.screen == Screen::CaseDetail && app.case_detail.is_some() {
        draw_case_detail(app, frame, layout[1]);
    } else if app.screen == Screen::RunDetail && app.run_detail.is_some() {
        draw_run_detail(app, frame, layout[1]);
    } else if app.screen == Screen::BugDetail && app.bug_detail.is_some() {
        draw_bug_detail(app, frame, layout[1]);
    } else {
        match app.tab {
            Tab::Cases => draw_cases_browse(app, frame, layout[1]),
            Tab::Runs => draw_runs_browse(app, frame, layout[1]),
            Tab::Bugs => draw_bugs_browse(app, frame, layout[1]),
            Tab::Reports => draw_reports(app, frame, layout[1]),
            Tab::Dashboard => draw_dashboard(app, frame, layout[1]),
            Tab::Drivers => draw_drivers(app, frame, layout[1]),
        }
    }

    draw_footer(app, frame, layout[2]);

    if app.note.is_some() {
        draw_note(app, frame, area);
    }
    if let Some(confirm) = &app.confirm_delete {
        draw_confirm(frame, area, confirm);
    }
    if let Some(form) = &app.form {
        draw_form(frame, area, form);
    }
    if app.help {
        draw_help(frame, area);
    }
}

fn draw_header(app: &App, frame: &mut Frame<'_>, area: Rect) {
    frame.render_widget(Block::default().style(Style::default().bg(HEADER_BG)), area);
    let rows = Layout::vertical([Constraint::Length(1), Constraint::Length(1)]).split(area);

    let project = app
        .cfg
        .root
        .file_name()
        .and_then(|name| name.to_str())
        .unwrap_or("project");
    let db = app
        .cfg
        .db_path
        .strip_prefix(&app.cfg.root)
        .map(|path| path.display().to_string())
        .unwrap_or_else(|_| app.cfg.db_path.display().to_string());
    let line = Line::from(vec![
        Span::styled(
            " LABURO ",
            Style::default()
                .fg(BG)
                .bg(ACCENT)
                .add_modifier(Modifier::BOLD),
        ),
        Span::raw("  "),
        Span::styled(
            project.to_string(),
            Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
        ),
        Span::styled("  ·  ", dim()),
        Span::styled(db, dim()),
    ]);
    frame.render_widget(Paragraph::new(line), rows[0]);

    let mut spans = vec![Span::raw(" ")];
    for (index, tab) in Tab::ALL.iter().enumerate() {
        let label = format!(" {} {} ", index + 1, tab.title());
        if *tab == app.tab {
            spans.push(Span::styled(
                label,
                Style::default()
                    .fg(BG)
                    .bg(ACCENT)
                    .add_modifier(Modifier::BOLD),
            ));
        } else {
            spans.push(Span::styled(label, dim()));
        }
        spans.push(Span::raw(" "));
    }
    frame.render_widget(
        Paragraph::new(Line::from(spans)).style(Style::default().bg(HEADER_BG)),
        rows[1],
    );
}

fn draw_footer(app: &App, frame: &mut Frame<'_>, area: Rect) {
    frame.render_widget(Block::default().style(Style::default().bg(HEADER_BG)), area);
    let message = match &app.message {
        Some(message) => Line::from(vec![
            Span::styled(
                format!(" {} ", level_label(message.level)),
                Style::default()
                    .fg(BG)
                    .bg(level_color(message.level))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(message.text.clone(), Style::default().fg(TEXT)),
        ]),
        None => Line::from(Span::styled(" Ready", dim())),
    };
    let hints = Line::from(Span::styled(format!(" {}", hints(app)), dim()));
    frame.render_widget(Paragraph::new(vec![message, hints]), area);
}

fn level_label(level: MessageLevel) -> &'static str {
    match level {
        MessageLevel::Info => "INFO",
        MessageLevel::Ok => "OK",
        MessageLevel::Error => "ERR",
    }
}

fn level_color(level: MessageLevel) -> Color {
    match level {
        MessageLevel::Info => ACCENT,
        MessageLevel::Ok => GREEN,
        MessageLevel::Error => RED,
    }
}

fn hints(app: &App) -> &'static str {
    if app.help {
        return "Esc close help";
    }
    if app.form.is_some() {
        return "Tab next field · Enter save · Esc cancel";
    }
    if app.confirm_delete.is_some() {
        return "Enter/y delete · Esc/n cancel";
    }
    if app.note.is_some() {
        return "Enter save note · Esc cancel";
    }
    if app.manual.is_some() {
        return "p pass · f fail · m blocked · Enter next · n note · F/Esc finish · j/k step";
    }
    if app.screen != Screen::Browse {
        return "j/k scroll · g/G top/bottom · Esc back · ? help · q quit";
    }
    match app.tab {
        Tab::Cases => {
            "j/k move · g/G ends · Enter open · c new · e title · d delete · r run · x report · h/l tabs · ? help · q quit"
        }
        Tab::Runs => "j/k move · g/G ends · Enter results · h/l tabs · ? help · q quit",
        Tab::Bugs => "j/k move · g/G ends · Enter detail · h/l tabs · ? help · q quit",
        Tab::Reports => {
            "j/k select case · d dictionary · x case report · h/l tabs · ? help · q quit"
        }
        Tab::Dashboard => "h/l tabs · ? help · q quit",
        Tab::Drivers => "j/k move · g/G ends · h/l tabs · ? help · q quit",
    }
}

fn split_main(area: Rect) -> (Rect, Rect) {
    let chunks = Layout::horizontal([
        Constraint::Percentage(52),
        Constraint::Length(1),
        Constraint::Min(0),
    ])
    .split(area);
    (chunks[0], chunks[2])
}

fn draw_cases_browse(app: &mut App, frame: &mut Frame<'_>, area: Rect) {
    let (left, right) = split_main(area);
    render_cases_table(app, frame, left, "Cases");
    let row = app
        .cases
        .get(app.case_state.selected().unwrap_or(0))
        .cloned();
    match row {
        Some(row) => draw_case_preview(frame, right, &row),
        None => draw_empty_pane(
            frame,
            right,
            "Preview",
            "No case selected\n\nPress c to create the first case.",
        ),
    }
}

fn render_cases_table(app: &mut App, frame: &mut Frame<'_>, area: Rect, title: &str) {
    let rows: Vec<Row<'static>> = app
        .cases
        .iter()
        .map(|row| {
            let tags = if row.case.tags.is_empty() {
                "—".to_string()
            } else {
                shorten(&row.case.tags.join(" · "), 14)
            };
            let last = match row.last_status {
                Some(status) => Cell::from(Line::from(status_chip(status))),
                None => Cell::from(Span::styled("—", dim())),
            };
            Row::new(vec![
                Cell::from(Span::styled(
                    row.case.id.clone(),
                    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
                )),
                Cell::from(Span::raw(shorten(&row.case.title, 48))),
                Cell::from(Span::styled(tags, dim())),
                Cell::from(Span::styled(
                    row.case.priority.to_ascii_uppercase(),
                    Style::default().fg(priority_color(&row.case.priority)),
                )),
                last,
            ])
        })
        .collect();

    let widths = [
        Constraint::Length(9),
        Constraint::Min(12),
        Constraint::Length(14),
        Constraint::Length(6),
        Constraint::Length(10),
    ];
    let table = Table::new(rows, widths)
        .header(
            Row::new(["ID", "TITLE", "TAGS", "PRI", "LAST"])
                .style(Style::default().fg(DIM).add_modifier(Modifier::BOLD)),
        )
        .block(pane_block(title))
        .column_spacing(1)
        .row_highlight_style(Style::default().bg(SELECTED).fg(TEXT))
        .highlight_symbol("› ");
    frame.render_stateful_widget(table, area, &mut app.case_state);
    if app.cases.is_empty() {
        draw_table_hint(frame, area, "No cases yet\npress c to create one");
    }
}

fn draw_case_preview(frame: &mut Frame<'_>, area: Rect, row: &CaseRow) {
    let mut lines = vec![
        Line::from(Span::styled(
            row.case.title.clone(),
            Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled(
                row.case.id.clone(),
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            priority_span(&row.case.priority),
            Span::raw(" "),
            case_status_span(&row.case.status),
        ]),
        Line::default(),
    ];
    lines.extend(field_lines("Module", &row.case.module));
    lines.extend(field_lines("Environment", &row.case.environment));
    lines.extend(field_lines("Tags", &row.case.tags.join(", ")));
    lines.extend(field_lines("Description", &row.case.description));
    lines.push(Line::default());
    lines.push(section(&format!("STEPS · {}", row.case.steps.len())));
    if row.case.steps.is_empty() {
        lines.push(Line::from(Span::styled("  No steps designed yet", dim())));
    } else {
        for step in row.case.steps.iter().take(6) {
            lines.push(Line::from(vec![
                Span::styled(format!("  {:>2}. ", step.position), dim()),
                Span::raw(shorten(&step.action, 48)),
            ]));
        }
        if row.case.steps.len() > 6 {
            lines.push(Line::from(Span::styled(
                format!("  … {} more", row.case.steps.len() - 6),
                dim(),
            )));
        }
    }
    lines.push(Line::default());
    match row.last_status {
        Some(status) => lines.push(Line::from(vec![
            Span::styled("  Last run  ", dim()),
            status_chip(status),
            Span::styled(
                format!("  {}", row.last_run_at.as_deref().unwrap_or_default()),
                dim(),
            ),
        ])),
        None => lines.push(Line::from(Span::styled("  Last run  not run yet", dim()))),
    }
    lines.push(Line::default());
    lines.push(Line::from(Span::styled(
        "  Enter opens the case · r starts a manual run",
        dim(),
    )));
    let paragraph = Paragraph::new(lines)
        .block(pane_block("Preview"))
        .style(panel())
        .wrap(Wrap { trim: false });
    frame.render_widget(paragraph, area);
}

fn draw_runs_browse(app: &mut App, frame: &mut Frame<'_>, area: Rect) {
    let (left, right) = split_main(area);
    let rows: Vec<Row<'static>> = app
        .runs
        .iter()
        .map(|run| {
            Row::new(vec![
                Cell::from(Span::styled(
                    shorten(&run.id, 22),
                    Style::default().fg(ACCENT),
                )),
                Cell::from(Span::raw(run.case_id.clone())),
                Cell::from(Line::from(status_chip(run.status))),
                Cell::from(Span::styled(shorten(&run.started_at, 19), dim())),
            ])
        })
        .collect();
    let widths = [
        Constraint::Min(16),
        Constraint::Length(9),
        Constraint::Length(10),
        Constraint::Length(19),
    ];
    let table = Table::new(rows, widths)
        .header(
            Row::new(["RUN", "CASE", "STATUS", "STARTED"])
                .style(Style::default().fg(DIM).add_modifier(Modifier::BOLD)),
        )
        .block(pane_block("Runs"))
        .column_spacing(1)
        .row_highlight_style(Style::default().bg(SELECTED).fg(TEXT))
        .highlight_symbol("› ");
    frame.render_stateful_widget(table, area, &mut app.run_state);
    if app.runs.is_empty() {
        draw_table_hint(frame, left, "No runs yet\npress r on a case to start one");
    }

    let run = app.runs.get(app.run_state.selected().unwrap_or(0)).cloned();
    match run {
        Some(run) => draw_run_preview(app, frame, right, &run),
        None => draw_empty_pane(frame, right, "Preview", "No run selected"),
    }
}

fn draw_run_preview(app: &App, frame: &mut Frame<'_>, area: Rect, run: &TestRun) {
    let mut lines = vec![
        Line::from(vec![
            Span::styled(
                run.id.clone(),
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            status_chip(run.status),
        ]),
        Line::default(),
    ];
    lines.extend(field_lines("Case", &run.case_id));
    lines.extend(field_lines("Mode", run.mode.as_str()));
    lines.extend(field_lines("Started", &run.started_at));
    lines.extend(field_lines(
        "Ended",
        run.ended_at.as_deref().unwrap_or("not finished"),
    ));
    lines.extend(field_lines("Environment", &run.environment));
    lines.push(Line::default());
    lines.push(section(&format!("STEPS · {}", run.steps.len())));
    if run.steps.is_empty() {
        lines.push(Line::from(Span::styled("  No step results", dim())));
    } else {
        for step in run.steps.iter().take(8) {
            let action = step_design(app, run, step)
                .map(|design| design.action.clone())
                .unwrap_or_else(|| format!("step {}", step.position));
            lines.push(Line::from(vec![
                Span::styled(format!("  {:>2} ", step.position), dim()),
                status_chip(step.status),
                Span::raw(" "),
                Span::raw(shorten(&action, 34)),
            ]));
        }
    }
    if !run.notes.trim().is_empty() {
        lines.push(Line::default());
        lines.extend(field_lines("Notes", &run.notes));
    }
    lines.push(Line::default());
    lines.push(Line::from(Span::styled(
        "  Enter opens step results and evidence",
        dim(),
    )));
    frame.render_widget(
        Paragraph::new(lines)
            .block(pane_block("Preview"))
            .style(panel())
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn draw_bugs_browse(app: &mut App, frame: &mut Frame<'_>, area: Rect) {
    let (left, right) = split_main(area);
    let rows: Vec<Row<'static>> = app
        .bugs
        .iter()
        .map(|bug| {
            Row::new(vec![
                Cell::from(Span::styled(
                    shorten(&bug.id, 20),
                    Style::default().fg(ACCENT),
                )),
                Cell::from(Span::raw(shorten(&bug.title, 44))),
                Cell::from(Span::styled(
                    format!("S{}", bug.severity),
                    Style::default().fg(severity_color(bug.severity)),
                )),
                Cell::from(Line::from(bug_status_chip(&bug.status))),
            ])
        })
        .collect();
    let widths = [
        Constraint::Length(20),
        Constraint::Min(12),
        Constraint::Length(4),
        Constraint::Length(9),
    ];
    let table = Table::new(rows, widths)
        .header(
            Row::new(["BUG", "TITLE", "SEV", "STATUS"])
                .style(Style::default().fg(DIM).add_modifier(Modifier::BOLD)),
        )
        .block(pane_block("Bugs"))
        .column_spacing(1)
        .row_highlight_style(Style::default().bg(SELECTED).fg(TEXT))
        .highlight_symbol("› ");
    frame.render_stateful_widget(table, area, &mut app.bug_state);
    if app.bugs.is_empty() {
        draw_table_hint(frame, left, "No bugs recorded");
    }

    let bug = app.bugs.get(app.bug_state.selected().unwrap_or(0)).cloned();
    match bug {
        Some(bug) => draw_bug_preview(frame, right, &bug),
        None => draw_empty_pane(frame, right, "Preview", "No bug selected"),
    }
}

fn draw_bug_preview(frame: &mut Frame<'_>, area: Rect, bug: &BugReport) {
    let mut lines = vec![
        Line::from(Span::styled(
            bug.title.clone(),
            Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
        )),
        Line::from(vec![
            Span::styled(
                bug.id.clone(),
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            bug_status_chip(&bug.status),
            Span::raw(" "),
            number_chip("P", bug.priority),
            Span::raw(" "),
            number_chip("S", bug.severity),
        ]),
        Line::default(),
    ];
    lines.extend(field_lines("Case", bug.case_id.as_deref().unwrap_or("—")));
    lines.extend(field_lines("Run", bug.run_id.as_deref().unwrap_or("—")));
    lines.extend(field_lines("Environment", &bug.environment));
    lines.extend(field_lines("Description", &bug.description));
    lines.extend(field_lines("Actual", &bug.actual));
    lines.extend(field_lines("Expected", &bug.expected));
    lines.push(Line::default());
    lines.push(Line::from(vec![
        Span::styled("  Evidence  ", dim()),
        Span::styled(format!("{} attachment(s)", bug.evidence.len()), dim()),
    ]));
    frame.render_widget(
        Paragraph::new(lines)
            .block(pane_block("Preview"))
            .style(panel())
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn draw_drivers(app: &mut App, frame: &mut Frame<'_>, area: Rect) {
    let (left, right) = split_main(area);
    let rows: Vec<Row<'static>> = app
        .drivers
        .iter()
        .map(|test| {
            let location = match test.line {
                Some(line) => format!("{}:{}", test.file, line),
                None => test.file.clone(),
            };
            Row::new(vec![
                Cell::from(Span::styled(
                    shorten(&test.id, 36),
                    Style::default().fg(ACCENT),
                )),
                Cell::from(Span::styled(shorten(&location, 30), dim())),
                Cell::from(Span::raw(shorten(&test.name, 40))),
            ])
        })
        .collect();
    let widths = [
        Constraint::Min(16),
        Constraint::Length(28),
        Constraint::Min(12),
    ];
    let table = Table::new(rows, widths)
        .header(
            Row::new(["ID", "FILE:LINE", "NAME"])
                .style(Style::default().fg(DIM).add_modifier(Modifier::BOLD)),
        )
        .block(pane_block("Drivers"))
        .column_spacing(1)
        .row_highlight_style(Style::default().bg(SELECTED).fg(TEXT))
        .highlight_symbol("› ");
    frame.render_stateful_widget(table, left, &mut app.driver_state);
    if app.drivers.is_empty() {
        draw_table_hint(
            frame,
            left,
            "No discovered tests\nrun `laburo driver discover`",
        );
    }

    let test = app
        .drivers
        .get(app.driver_state.selected().unwrap_or(0))
        .cloned();
    match test {
        Some(test) => draw_driver_preview(frame, right, &test),
        None => draw_empty_pane(
            frame,
            right,
            "Preview",
            "No discovered tests\n\nrun `laburo driver discover`",
        ),
    }
}

fn draw_driver_preview(frame: &mut Frame<'_>, area: Rect, test: &DiscoveredTest) {
    let location = match test.line {
        Some(line) => format!("{}:{}", test.file, line),
        None => test.file.clone(),
    };
    let mut lines = vec![
        Line::from(Span::styled(
            test.name.clone(),
            Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
        )),
        Line::from(Span::styled(test.id.clone(), Style::default().fg(ACCENT))),
        Line::default(),
    ];
    lines.extend(field_lines("Driver", &test.driver_id));
    lines.extend(field_lines("File", &test.file));
    lines.extend(field_lines(
        "Line",
        &test
            .line
            .map(|line| line.to_string())
            .unwrap_or_else(|| "—".to_string()),
    ));
    lines.extend(field_lines("Location", &location));
    if !test.metadata.is_null() {
        lines.push(Line::default());
        lines.push(section("METADATA"));
        for line in test.metadata.to_string().lines().take(12) {
            lines.push(Line::from(Span::styled(
                format!("  {line}"),
                Style::default().fg(DIM),
            )));
        }
    }
    lines.push(Line::default());
    lines.push(Line::from(Span::styled(
        "  Read-only · run `laburo driver discover` to refresh",
        dim(),
    )));
    frame.render_widget(
        Paragraph::new(lines)
            .block(pane_block("Preview"))
            .style(panel())
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn draw_reports(app: &mut App, frame: &mut Frame<'_>, area: Rect) {
    let (left, right) = split_main(area);
    render_cases_table(app, frame, left, "Cases");

    let selected = app
        .cases
        .get(app.case_state.selected().unwrap_or(0))
        .map(|row| (row.case.id.clone(), row.case.title.clone()));
    let mut lines = vec![
        section("GENERATORS"),
        Line::default(),
        Line::from(vec![
            Span::styled(
                "  d  ",
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "Dictionary",
                Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
            ),
        ]),
        Line::from(Span::styled(
            "     every case, step, tag, and requirement link",
            dim(),
        )),
        Line::default(),
        Line::from(vec![
            Span::styled(
                "  x  ",
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
            Span::styled(
                "Case report",
                Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
            ),
        ]),
    ];
    match &selected {
        Some((id, title)) => {
            lines.push(Line::from(Span::styled(
                format!("     {id} · {}", shorten(title, 40)),
                dim(),
            )));
            lines.push(Line::from(Span::styled(
                "     selects the case highlighted on the left",
                dim(),
            )));
        }
        None => lines.push(Line::from(Span::styled("     no case selected", dim()))),
    }
    lines.push(Line::default());
    lines.push(section("OUTPUT"));
    let output = app
        .cfg
        .data_dir
        .join("reports")
        .strip_prefix(&app.cfg.root)
        .map(|path| path.display().to_string())
        .unwrap_or_else(|_| app.cfg.data_dir.join("reports").display().to_string());
    lines.push(Line::from(vec![
        Span::styled("  directory  ", dim()),
        Span::raw(output),
    ]));
    lines.push(Line::default());
    lines.push(section("LAST GENERATED"));
    match &app.last_report {
        Some(path) => lines.push(Line::from(Span::styled(
            format!("  {}", path.display()),
            Style::default().fg(GREEN),
        ))),
        None => lines.push(Line::from(Span::styled(
            "  Nothing generated in this session",
            dim(),
        ))),
    }
    lines.push(Line::default());
    lines.push(Line::from(Span::styled(
        "  Results appear in the status bar.",
        dim(),
    )));

    frame.render_widget(
        Paragraph::new(lines)
            .block(pane_block("Reports"))
            .style(panel())
            .wrap(Wrap { trim: false }),
        right,
    );
}

fn draw_dashboard(app: &App, frame: &mut Frame<'_>, area: Rect) {
    let Some(dashboard) = &app.dashboard else {
        draw_empty_pane(
            frame,
            area,
            "Dashboard",
            "Dashboard is not available right now.\nSee the status bar for the error.",
        );
        return;
    };

    let layout = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(1),
        Constraint::Length(3),
        Constraint::Length(1),
        Constraint::Min(1),
    ])
    .split(area);

    let cards_top = three_columns(layout[0]);
    let pass_rate = dashboard.pass_rate();
    let rate_color = if pass_rate >= 90.0 {
        GREEN
    } else if pass_rate >= 70.0 {
        ACCENT2
    } else {
        RED
    };
    draw_metric(
        frame,
        cards_top[0],
        "CASES",
        &dashboard.cases.to_string(),
        "designed test cases",
        ACCENT,
    );
    draw_metric(
        frame,
        cards_top[2],
        "RUNS",
        &dashboard.runs.to_string(),
        "recorded executions",
        BLUE,
    );
    draw_metric(
        frame,
        cards_top[4],
        "PASS RATE",
        &format!("{pass_rate:.0}%"),
        &format!(
            "{} passed · {} failed",
            dashboard.passed_runs, dashboard.failed_runs
        ),
        rate_color,
    );

    let cards_bottom = three_columns(layout[2]);
    draw_metric(
        frame,
        cards_bottom[0],
        "OPEN BUGS",
        &dashboard.bugs_open.to_string(),
        &format!("{} bugs total", dashboard.bugs_total),
        if dashboard.bugs_open > 0 {
            ACCENT2
        } else {
            GREEN
        },
    );
    draw_metric(
        frame,
        cards_bottom[2],
        "FAILED RUNS",
        &dashboard.failed_runs.to_string(),
        "need attention",
        if dashboard.failed_runs > 0 {
            RED
        } else {
            GREEN
        },
    );
    draw_metric(
        frame,
        cards_bottom[4],
        "EVIDENCE",
        &dashboard.evidence.to_string(),
        "attached files",
        ACCENT2,
    );

    let bottom = Layout::horizontal([
        Constraint::Percentage(52),
        Constraint::Length(1),
        Constraint::Min(0),
    ])
    .split(layout[4]);

    let mut failures = Vec::new();
    for run in app.runs.iter().filter(|run| run.status.is_bad()).take(8) {
        failures.push(Line::from(vec![
            Span::styled(
                format!(" {} ", run.status.as_str()),
                Style::default()
                    .fg(BG)
                    .bg(status_color(run.status))
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(shorten(&run.id, 24), Style::default().fg(TEXT)),
            Span::styled(format!("  {}", shorten(&run.started_at, 19)), dim()),
        ]));
    }
    if failures.is_empty() {
        failures.push(Line::from(Span::styled(" No failing runs", dim())));
    }
    frame.render_widget(
        Paragraph::new(failures)
            .block(pane_block("Recent failures"))
            .style(panel()),
        bottom[0],
    );

    let mut open_bugs = Vec::new();
    for bug in app.bugs.iter().filter(|bug| bug.status == "open").take(8) {
        open_bugs.push(Line::from(vec![
            Span::styled(
                " BUG ",
                Style::default().fg(BG).bg(RED).add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(shorten(&bug.id, 18), Style::default().fg(TEXT)),
            Span::raw(" "),
            Span::styled(shorten(&bug.title, 28), dim()),
        ]));
    }
    if open_bugs.is_empty() {
        open_bugs.push(Line::from(Span::styled(" No open bugs", dim())));
    }
    frame.render_widget(
        Paragraph::new(open_bugs)
            .block(pane_block("Open bugs"))
            .style(panel()),
        bottom[2],
    );
}

fn three_columns(area: Rect) -> Vec<Rect> {
    Layout::horizontal([
        Constraint::Percentage(33),
        Constraint::Length(1),
        Constraint::Percentage(33),
        Constraint::Length(1),
        Constraint::Min(0),
    ])
    .split(area)
    .to_vec()
}

fn draw_metric(
    frame: &mut Frame<'_>,
    area: Rect,
    label: &str,
    value: &str,
    note: &str,
    color: Color,
) {
    let block = Block::default()
        .borders(Borders::TOP)
        .border_style(Style::default().fg(BORDER))
        .style(panel())
        .padding(Padding::horizontal(1))
        .title(Line::from(Span::styled(
            format!(" {label} "),
            Style::default().fg(DIM).add_modifier(Modifier::BOLD),
        )));
    let inner = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                value.to_string(),
                Style::default().fg(color).add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(note.to_string(), dim())),
        ]),
        inner,
    );
}

fn draw_case_detail(app: &mut App, frame: &mut Frame<'_>, area: Rect) {
    let chunks = Layout::vertical([Constraint::Length(3), Constraint::Min(1)]).split(area);
    {
        let Some(detail) = &app.case_detail else {
            return;
        };
        draw_case_header(frame, chunks[0], detail);
    }

    let lines = case_detail_lines(app);
    let (left, right) = split_main(chunks[1]);
    scrollable_panel(frame, left, "Case detail", lines, &mut app.scroll);

    let side = Layout::vertical([
        Constraint::Percentage(52),
        Constraint::Length(1),
        Constraint::Min(0),
    ])
    .split(right);
    let Some(detail) = &app.case_detail else {
        return;
    };
    let runs = case_runs_lines(&detail.runs);
    let bugs = case_bugs_lines(&detail.bugs);
    frame.render_widget(
        Paragraph::new(runs)
            .block(pane_block("Runs"))
            .style(panel())
            .wrap(Wrap { trim: false }),
        side[0],
    );
    frame.render_widget(
        Paragraph::new(bugs)
            .block(pane_block("Bugs"))
            .style(panel())
            .wrap(Wrap { trim: false }),
        side[2],
    );
}

fn draw_case_header(frame: &mut Frame<'_>, area: Rect, detail: &CaseDetail) {
    let tags = if detail.case.tags.is_empty() {
        vec![Span::styled("—", dim())]
    } else {
        detail
            .case
            .tags
            .iter()
            .map(|tag| {
                Span::styled(
                    format!(" {tag} "),
                    Style::default().fg(ACCENT2).bg(SELECTED),
                )
            })
            .collect::<Vec<_>>()
    };
    let mut second = vec![
        priority_span(&detail.case.priority),
        Span::raw(" "),
        case_status_span(&detail.case.status),
        Span::raw("  "),
    ];
    second.extend(tags);
    second.push(Span::styled(
        format!("  updated {}", detail.case.updated_at),
        dim(),
    ));

    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled(
                    detail.case.id.clone(),
                    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
                Span::styled(
                    detail.case.title.clone(),
                    Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(second),
        ])
        .style(base()),
        area,
    );
}

fn case_detail_lines(app: &App) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    let Some(detail) = &app.case_detail else {
        return lines;
    };
    let case = &detail.case;
    lines.push(section("FIELDS"));
    lines.extend(field_lines("Description", &case.description));
    lines.extend(field_lines("Module", &case.module));
    lines.extend(field_lines("Priority", &case.priority));
    lines.extend(field_lines("Status", &case.status));
    lines.extend(field_lines("Environment", &case.environment));
    lines.extend(field_lines("Tags", &case.tags.join(", ")));
    lines.extend(field_lines("Created", &case.created_at));
    lines.extend(field_lines("Updated", &case.updated_at));
    if !case.gherkin.trim().is_empty() {
        lines.extend(field_lines("Gherkin", &case.gherkin));
    }
    lines.push(Line::default());
    lines.push(section(&format!("STEPS · {}", case.steps.len())));
    if case.steps.is_empty() {
        lines.push(Line::from(Span::styled("  No steps designed yet", dim())));
    } else {
        for step in &case.steps {
            lines.push(Line::from(vec![
                Span::styled(format!("  {:>2}. ", step.position), dim()),
                Span::styled(
                    step.action.clone(),
                    Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
                ),
            ]));
            if !step.expected.trim().is_empty() {
                lines.push(Line::from(vec![
                    Span::raw("      "),
                    Span::styled("expected  ", dim()),
                    Span::raw(step.expected.clone()),
                ]));
            }
            if !step.data.trim().is_empty() {
                lines.push(Line::from(vec![
                    Span::raw("      "),
                    Span::styled("data      ", dim()),
                    Span::raw(step.data.clone()),
                ]));
            }
            let mut meta = vec![
                Span::raw("      "),
                Span::styled(step.kind.as_str().to_string(), Style::default().fg(BLUE)),
            ];
            if let Some(target) = &step.driver_target {
                meta.push(Span::styled(format!("  {target}"), dim()));
            }
            lines.push(Line::from(meta));
        }
    }
    lines.push(Line::default());
    lines.push(Line::from(Span::styled(
        "  r manual run · x report · e edit title · d delete · Esc back",
        dim(),
    )));
    lines
}

fn case_runs_lines(runs: &[TestRun]) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    if runs.is_empty() {
        lines.push(Line::from(Span::styled("  No runs yet", dim())));
        return lines;
    }
    for run in runs.iter().take(8) {
        lines.push(Line::from(vec![
            status_chip(run.status),
            Span::raw(" "),
            Span::styled(shorten(&run.id, 24), Style::default().fg(TEXT)),
        ]));
        lines.push(Line::from(Span::styled(
            format!("  {} · {}", shorten(&run.started_at, 19), run.mode.as_str()),
            dim(),
        )));
    }
    lines
}

fn case_bugs_lines(bugs: &[BugReport]) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    if bugs.is_empty() {
        lines.push(Line::from(Span::styled("  No linked bugs", dim())));
        return lines;
    }
    for bug in bugs.iter().take(8) {
        lines.push(Line::from(vec![
            bug_status_chip(&bug.status),
            Span::raw(" "),
            Span::styled(shorten(&bug.id, 20), Style::default().fg(TEXT)),
        ]));
        lines.push(Line::from(Span::styled(
            format!("  {}", shorten(&bug.title, 32)),
            dim(),
        )));
    }
    lines
}

fn draw_run_detail(app: &mut App, frame: &mut Frame<'_>, area: Rect) {
    let chunks = Layout::vertical([Constraint::Length(3), Constraint::Min(1)]).split(area);
    let lines = {
        let Some(detail) = &app.run_detail else {
            return;
        };
        draw_run_header(frame, chunks[0], &detail.run);
        run_detail_lines(app, detail)
    };
    scrollable_panel(frame, chunks[1], "Run detail", lines, &mut app.scroll);
}

fn draw_run_header(frame: &mut Frame<'_>, area: Rect, run: &TestRun) {
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                status_chip(run.status),
                Span::raw(" "),
                Span::styled(
                    run.id.clone(),
                    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
                Span::styled(format!("case {}", run.case_id), dim()),
                Span::raw("  "),
                Span::styled(run.mode.as_str().to_string(), Style::default().fg(BLUE)),
            ]),
            Line::from(vec![
                Span::styled(format!("  started {}", run.started_at), dim()),
                Span::styled(
                    format!(
                        "  ·  ended {}",
                        run.ended_at.as_deref().unwrap_or("not finished")
                    ),
                    dim(),
                ),
                Span::styled(
                    format!("  ·  env {}", display_or_dash(&run.environment)),
                    dim(),
                ),
            ]),
        ])
        .style(base()),
        area,
    );
}

fn run_detail_lines(app: &App, detail: &RunDetail) -> Vec<Line<'static>> {
    let run = &detail.run;
    let mut lines = vec![section("RUN")];
    lines.extend(field_lines("Case", &run.case_id));
    lines.extend(field_lines("Mode", run.mode.as_str()));
    lines.extend(field_lines("Status", run.status.as_str()));
    lines.extend(field_lines("Started", &run.started_at));
    lines.extend(field_lines(
        "Ended",
        run.ended_at.as_deref().unwrap_or("not finished"),
    ));
    lines.extend(field_lines("Environment", &run.environment));
    lines.extend(field_lines("Commit", &run.commit_ref));
    lines.extend(field_lines("Command", &run.command));
    lines.extend(field_lines("Notes", &run.notes));
    if !detail.evidence.is_empty() {
        lines.push(Line::default());
        lines.push(section("RUN EVIDENCE"));
        for attachment in &detail.evidence {
            lines.push(evidence_line(attachment));
        }
    }

    lines.push(Line::default());
    lines.push(section(&format!("STEP RESULTS · {}", run.steps.len())));
    if run.steps.is_empty() {
        lines.push(Line::from(Span::styled("  No step results", dim())));
    }
    for step in &run.steps {
        let design = step_design(app, run, step);
        let action = design
            .map(|design| design.action.clone())
            .unwrap_or_else(|| format!("step {}", step.position));
        lines.push(Line::from(vec![
            Span::styled(format!("  {:>2} ", step.position), dim()),
            status_chip(step.status),
            Span::raw(" "),
            Span::styled(
                action,
                Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
            ),
        ]));
        if let Some(design) = design {
            if !design.expected.trim().is_empty() {
                lines.push(Line::from(vec![
                    Span::raw("      "),
                    Span::styled("expected  ", dim()),
                    Span::raw(design.expected.clone()),
                ]));
            }
        }
        if !step.observed.trim().is_empty() {
            lines.push(Line::from(vec![
                Span::raw("      "),
                Span::styled("observed  ", dim()),
                Span::raw(step.observed.clone()),
            ]));
        }
        if !step.notes.trim().is_empty() {
            lines.push(Line::from(vec![
                Span::raw("      "),
                Span::styled("notes     ", dim()),
                Span::raw(step.notes.clone()),
            ]));
        }
        if let Some(driver_test) = &step.driver_test {
            lines.push(Line::from(vec![
                Span::raw("      "),
                Span::styled("driver    ", dim()),
                Span::styled(driver_test.clone(), Style::default().fg(BLUE)),
            ]));
        }
        for attachment in &step.evidence {
            lines.push(evidence_line(attachment));
        }
        if !step.raw_output.trim().is_empty() {
            lines.push(Line::from(Span::styled("      raw output", dim())));
            for line in step.raw_output.lines().take(12) {
                lines.push(Line::from(Span::styled(
                    format!("        {line}"),
                    Style::default().fg(DIM),
                )));
            }
            if step.raw_output.lines().count() > 12 {
                lines.push(Line::from(Span::styled("        … truncated", dim())));
            }
        }
    }
    lines.push(Line::default());
    lines.push(Line::from(Span::styled("  Esc back · j/k scroll", dim())));
    lines
}

fn draw_bug_detail(app: &mut App, frame: &mut Frame<'_>, area: Rect) {
    let chunks = Layout::vertical([Constraint::Length(3), Constraint::Min(1)]).split(area);
    let lines = {
        let Some(bug) = &app.bug_detail else {
            return;
        };
        draw_bug_header(frame, chunks[0], bug);
        bug_detail_lines(bug)
    };
    scrollable_panel(frame, chunks[1], "Bug detail", lines, &mut app.scroll);
}

fn draw_bug_header(frame: &mut Frame<'_>, area: Rect, bug: &BugReport) {
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(vec![
                Span::styled(
                    bug.id.clone(),
                    Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
                ),
                Span::raw("  "),
                Span::styled(
                    bug.title.clone(),
                    Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
                ),
            ]),
            Line::from(vec![
                bug_status_chip(&bug.status),
                Span::raw(" "),
                number_chip("P", bug.priority),
                Span::raw(" "),
                number_chip("S", bug.severity),
                Span::raw("  "),
                Span::styled(
                    format!("case {}", bug.case_id.as_deref().unwrap_or("—")),
                    dim(),
                ),
                Span::raw("  "),
                Span::styled(
                    format!("run {}", bug.run_id.as_deref().unwrap_or("—")),
                    dim(),
                ),
            ]),
        ])
        .style(base()),
        area,
    );
}

fn bug_detail_lines(bug: &BugReport) -> Vec<Line<'static>> {
    let mut lines = vec![section("BUG")];
    lines.extend(field_lines("Description", &bug.description));
    lines.extend(field_lines("Steps", &bug.steps));
    lines.extend(field_lines("Actual", &bug.actual));
    lines.extend(field_lines("Expected", &bug.expected));
    lines.extend(field_lines("Priority", &bug.priority.to_string()));
    lines.extend(field_lines("Severity", &bug.severity.to_string()));
    lines.extend(field_lines("Status", &bug.status));
    lines.extend(field_lines("Environment", &bug.environment));
    lines.extend(field_lines("Case", bug.case_id.as_deref().unwrap_or("—")));
    lines.extend(field_lines("Run", bug.run_id.as_deref().unwrap_or("—")));
    lines.extend(field_lines(
        "Step run",
        &bug.step_run_id
            .map(|id| id.to_string())
            .unwrap_or_else(|| "—".to_string()),
    ));
    lines.extend(field_lines("Comments", &bug.comments));
    lines.extend(field_lines("Created", &bug.created_at));
    lines.extend(field_lines("Updated", &bug.updated_at));
    lines.push(Line::default());
    lines.push(section(&format!("EVIDENCE · {}", bug.evidence.len())));
    if bug.evidence.is_empty() {
        lines.push(Line::from(Span::styled("  No evidence attached", dim())));
    } else {
        for attachment in &bug.evidence {
            lines.push(evidence_line(attachment));
        }
    }
    lines.push(Line::default());
    lines.push(Line::from(Span::styled("  Esc back · j/k scroll", dim())));
    lines
}

fn draw_manual(app: &App, frame: &mut Frame<'_>, area: Rect) {
    let Some(manual) = &app.manual else {
        return;
    };
    let run = &manual.run;
    let layout = Layout::vertical([
        Constraint::Length(2),
        Constraint::Length(2),
        Constraint::Min(1),
    ])
    .split(area);

    let case = find_case(app, &run.case_id);
    frame.render_widget(
        Paragraph::new(vec![Line::from(vec![
            Span::styled(
                " MANUAL ",
                Style::default()
                    .fg(BG)
                    .bg(ACCENT2)
                    .add_modifier(Modifier::BOLD),
            ),
            Span::raw(" "),
            Span::styled(
                run.id.clone(),
                Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
            ),
            Span::raw("  "),
            Span::styled(
                format!(
                    "{} · {}",
                    run.case_id,
                    case.map(|case| case.title.as_str())
                        .unwrap_or("unknown case")
                ),
                Style::default().fg(TEXT),
            ),
            Span::raw("  "),
            status_chip(run.status),
        ])])
        .style(base()),
        layout[0],
    );

    let total = run.steps.len();
    let done = run
        .steps
        .iter()
        .filter(|step| step.status.is_terminal())
        .count();
    let ratio = if total == 0 {
        0.0
    } else {
        done as f64 / total as f64
    };
    let gauge = Gauge::default()
        .ratio(ratio.clamp(0.0, 1.0))
        .label(format!(" {done}/{total} steps · {:.0}% ", ratio * 100.0))
        .gauge_style(Style::default().fg(ACCENT).bg(SELECTED))
        .style(Style::default().bg(PANEL_BG).fg(TEXT));
    frame.render_widget(gauge, layout[1]);

    let (left, right) = split_main(layout[2]);
    let mut items = Vec::new();
    for (index, step) in run.steps.iter().enumerate() {
        let action = step_design(app, run, step)
            .map(|design| design.action.clone())
            .unwrap_or_else(|| "ad-hoc step".to_string());
        items.push(ListItem::new(Line::from(vec![
            Span::styled(format!(" {:>2} ", index + 1), dim()),
            status_chip(step.status),
            Span::raw(" "),
            Span::raw(shorten(&action, 34)),
        ])));
    }
    let list = List::new(items)
        .block(pane_block("Steps"))
        .style(panel())
        .highlight_style(Style::default().bg(SELECTED).fg(TEXT))
        .highlight_symbol("› ");
    let mut state = ListState::default();
    if total > 0 {
        state.select(Some(manual.cursor.min(total - 1)));
    }
    frame.render_stateful_widget(list, left, &mut state);

    let mut lines = Vec::new();
    if let Some((index, step)) = run
        .steps
        .iter()
        .enumerate()
        .nth(manual.cursor.min(total.saturating_sub(1)))
    {
        lines.push(Line::from(Span::styled(
            format!("Step {} of {}", index + 1, total),
            dim(),
        )));
        lines.push(Line::default());
        let design = step_design(app, run, step);
        lines.push(Line::from(Span::styled(
            design
                .map(|design| design.action.clone())
                .unwrap_or_else(|| "Ad-hoc step".to_string()),
            Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
        )));
        lines.push(Line::default());
        if let Some(design) = design {
            lines.push(Line::from(vec![
                Span::styled("Expected  ", dim()),
                Span::raw(design.expected.clone()),
            ]));
            if !design.data.trim().is_empty() {
                lines.push(Line::from(vec![
                    Span::styled("Data      ", dim()),
                    Span::raw(design.data.clone()),
                ]));
            }
            lines.push(Line::from(vec![
                Span::styled("Kind      ", dim()),
                Span::styled(design.kind.as_str().to_string(), Style::default().fg(BLUE)),
            ]));
            if let Some(target) = &design.driver_target {
                lines.push(Line::from(vec![
                    Span::styled("Target    ", dim()),
                    Span::raw(target.clone()),
                ]));
            }
            lines.push(Line::default());
        }
        lines.push(Line::from(vec![
            Span::styled("Observed  ", dim()),
            Span::raw(display_or_dash(&step.observed)),
        ]));
        lines.push(Line::from(vec![
            Span::styled("Notes     ", dim()),
            Span::raw(display_or_dash(&step.notes)),
        ]));
        lines.push(Line::default());
        lines.push(Line::from(vec![
            Span::styled("Result    ", dim()),
            status_chip(step.status),
        ]));
    } else {
        lines.push(Line::from(Span::styled("Run has no steps", dim())));
    }
    lines.push(Line::default());
    lines.push(Line::from(Span::styled(
        "p pass   f fail   m blocked   Enter next",
        dim(),
    )));
    lines.push(Line::from(Span::styled(
        "n note   F/Esc finish   j/k step",
        dim(),
    )));
    frame.render_widget(
        Paragraph::new(lines)
            .block(pane_block("Current step"))
            .style(panel())
            .wrap(Wrap { trim: false }),
        right,
    );
}

fn scrollable_panel(
    frame: &mut Frame<'_>,
    area: Rect,
    title: &str,
    lines: Vec<Line<'static>>,
    scroll: &mut u16,
) {
    let text_width = area.width.saturating_sub(4).max(1) as usize;
    let visual_lines: usize = lines
        .iter()
        .map(|line| {
            let width = line.width();
            if width == 0 {
                1
            } else {
                width.div_ceil(text_width)
            }
        })
        .sum();
    let visible = area.height.saturating_sub(3) as usize;
    let max_scroll = visual_lines.saturating_sub(visible);
    *scroll = (*scroll).min(u16::try_from(max_scroll).unwrap_or(u16::MAX));
    frame.render_widget(
        Paragraph::new(lines)
            .block(pane_block(title))
            .style(panel())
            .scroll((*scroll, 0))
            .wrap(Wrap { trim: false }),
        area,
    );
}

fn draw_empty_pane(frame: &mut Frame<'_>, area: Rect, title: &str, text: &str) {
    let block = pane_block(title);
    let inner = block.inner(area);
    frame.render_widget(block, area);
    frame.render_widget(
        Paragraph::new(text)
            .alignment(Alignment::Center)
            .style(dim()),
        inner,
    );
}

fn draw_table_hint(frame: &mut Frame<'_>, area: Rect, text: &str) {
    if area.height < 5 {
        return;
    }
    let hint = Rect {
        x: area.x + 2,
        y: area.y + 3,
        width: area.width.saturating_sub(4),
        height: 2,
    };
    frame.render_widget(
        Paragraph::new(text)
            .alignment(Alignment::Center)
            .style(dim()),
        hint,
    );
}

fn draw_note(app: &App, frame: &mut Frame<'_>, area: Rect) {
    let Some(note) = &app.note else {
        return;
    };
    let width = area.width.saturating_sub(4).max(20);
    let popup = Rect {
        x: area.x + 2,
        y: area.y + area.height.saturating_sub(5),
        width,
        height: 3,
    };
    frame.render_widget(Clear, popup);
    let block = Block::default()
        .borders(Borders::TOP | Borders::BOTTOM)
        .border_style(Style::default().fg(ACCENT))
        .style(panel())
        .padding(Padding::horizontal(1))
        .title(Line::from(Span::styled(
            format!(" Note · {} · Enter save · Esc cancel ", note.step_label),
            Style::default().fg(ACCENT2).add_modifier(Modifier::BOLD),
        )));
    frame.render_widget(
        Paragraph::new(Line::from(vec![
            Span::styled("› ", Style::default().fg(ACCENT)),
            Span::raw(note.value.clone()),
            Span::styled("▏", Style::default().fg(ACCENT)),
        ]))
        .block(block),
        popup,
    );
}

fn draw_confirm(frame: &mut Frame<'_>, area: Rect, confirm: &crate::tui::app::ConfirmDelete) {
    let popup = centered(area, 62, 7);
    frame.render_widget(Clear, popup);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(RED))
        .style(panel())
        .padding(Padding::new(2, 2, 1, 1))
        .title(Line::from(Span::styled(
            " Confirm delete ",
            Style::default().fg(RED).add_modifier(Modifier::BOLD),
        )));
    frame.render_widget(
        Paragraph::new(vec![
            Line::from(Span::styled(
                format!("Delete {}?", confirm.id),
                Style::default().fg(TEXT).add_modifier(Modifier::BOLD),
            )),
            Line::from(Span::styled(shorten(&confirm.title, 54), dim())),
            Line::default(),
            Line::from(Span::styled("Enter/y delete · Esc/n cancel", dim())),
        ])
        .block(block),
        popup,
    );
}

fn draw_form(frame: &mut Frame<'_>, area: Rect, form: &FormState) {
    let height = (form.fields.len() as u16 * 2 + 6).max(8);
    let popup = centered(area, 68, height);
    frame.render_widget(Clear, popup);
    let block = Block::default()
        .borders(Borders::ALL)
        .border_style(Style::default().fg(ACCENT))
        .style(panel())
        .padding(Padding::new(2, 2, 1, 1))
        .title(Line::from(Span::styled(
            format!(" {} ", form.title()),
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        )));

    let input_width = popup.width.saturating_sub(8) as usize;
    let mut lines = Vec::new();
    for (index, field) in form.fields.iter().enumerate() {
        let active = index == form.focus;
        lines.push(Line::from(Span::styled(
            format!(" {} ", field.label.to_ascii_uppercase()),
            Style::default()
                .fg(if active { ACCENT } else { DIM })
                .add_modifier(Modifier::BOLD),
        )));
        let mut value = format!(" {:<width$} ", field.value, width = input_width);
        if active {
            value.push('▏');
        }
        lines.push(Line::from(Span::styled(
            shorten(&value, input_width + 3),
            if active {
                Style::default().fg(TEXT).bg(SELECTED)
            } else {
                Style::default().fg(TEXT).bg(PANEL_BG)
            },
        )));
    }
    match &form.error {
        Some(error) => lines.push(Line::from(Span::styled(
            format!(" {error}"),
            Style::default().fg(RED),
        ))),
        None => lines.push(Line::from(Span::styled(
            " Tab next · Enter save · Esc cancel",
            dim(),
        ))),
    }
    frame.render_widget(Paragraph::new(lines).block(block), popup);
}

fn draw_help(frame: &mut Frame<'_>, area: Rect) {
    let popup = centered(area, 84, 30);
    frame.render_widget(Clear, popup);
    let lines = vec![
        Line::from(Span::styled(
            "Navigation",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        )),
        help_line("h / l", "previous / next tab"),
        help_line("1 – 6", "jump to a tab"),
        help_line("j / k", "move down / up · scroll in detail"),
        help_line("g / G", "first / last"),
        help_line("Enter", "open the selected item"),
        help_line("Esc", "back / cancel"),
        help_line("q", "quit"),
        Line::default(),
        Line::from(Span::styled(
            "Cases",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        )),
        help_line("c / e / d", "create · edit title · delete (confirm)"),
        help_line("r", "start a manual run"),
        help_line("x", "generate a case report"),
        Line::default(),
        Line::from(Span::styled(
            "Manual run",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        )),
        help_line("p / f / m", "pass · fail · blocked"),
        help_line("Enter", "advance to the next step"),
        help_line("n", "note on the current step"),
        help_line("F / Esc", "finish the run and derive its status"),
        Line::default(),
        Line::from(Span::styled(
            "Reports",
            Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
        )),
        help_line("d", "dictionary report"),
        help_line("x", "case report for the selected case"),
        Line::default(),
        Line::from(Span::styled("Esc or ? closes this help", dim())),
    ];
    frame.render_widget(
        Paragraph::new(lines)
            .block(
                Block::default()
                    .borders(Borders::ALL)
                    .border_style(Style::default().fg(BORDER))
                    .style(panel())
                    .padding(Padding::new(2, 2, 1, 1))
                    .title(Line::from(Span::styled(
                        " Keys ",
                        Style::default().fg(ACCENT2).add_modifier(Modifier::BOLD),
                    ))),
            )
            .style(panel()),
        popup,
    );
}

fn help_line(keys: &str, description: &str) -> Line<'static> {
    Line::from(vec![
        Span::styled(format!("  {keys:<12}"), Style::default().fg(ACCENT2)),
        Span::styled(description.to_string(), Style::default().fg(TEXT)),
    ])
}

fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width);
    let height = height.min(area.height);
    Rect {
        x: area.x + area.width.saturating_sub(width) / 2,
        y: area.y + area.height.saturating_sub(height) / 2,
        width,
        height,
    }
}

fn section(title: &str) -> Line<'static> {
    Line::from(Span::styled(
        format!("▌ {title}"),
        Style::default().fg(ACCENT).add_modifier(Modifier::BOLD),
    ))
}

fn field_lines(label: &str, value: &str) -> Vec<Line<'static>> {
    let mut lines = Vec::new();
    if value.trim().is_empty() {
        lines.push(Line::from(vec![
            Span::styled(format!("  {label:<14}"), dim()),
            Span::styled("—", dim()),
        ]));
        return lines;
    }
    for (index, text) in value.lines().enumerate() {
        if index == 0 {
            lines.push(Line::from(vec![
                Span::styled(format!("  {label:<14}"), dim()),
                Span::raw(text.to_string()),
            ]));
        } else {
            lines.push(Line::from(vec![
                Span::raw(" ".repeat(16)),
                Span::raw(text.to_string()),
            ]));
        }
    }
    lines
}

fn display_or_dash(value: &str) -> &str {
    if value.trim().is_empty() {
        "—"
    } else {
        value
    }
}

fn evidence_line(attachment: &Attachment) -> Line<'static> {
    let caption = if attachment.caption.trim().is_empty() {
        String::new()
    } else {
        format!("  · {}", attachment.caption)
    };
    Line::from(vec![
        Span::raw("      "),
        Span::styled("-> ", dim()),
        Span::styled(attachment.path.clone(), Style::default().fg(ACCENT2)),
        Span::styled(format!("  {}{caption}", attachment.kind), dim()),
    ])
}

fn step_design<'a>(app: &'a App, run: &TestRun, step: &StepRun) -> Option<&'a TestStep> {
    find_case(app, &run.case_id).and_then(|case| {
        case.steps
            .iter()
            .find(|design| design.position == step.position)
    })
}

fn find_case<'a>(app: &'a App, case_id: &str) -> Option<&'a TestCase> {
    if let Some(detail) = &app.case_detail {
        if detail.case.id == case_id {
            return Some(&detail.case);
        }
    }
    app.cases
        .iter()
        .find(|row| row.case.id == case_id)
        .map(|row| &row.case)
}

fn shorten(value: &str, max: usize) -> String {
    if max == 0 {
        return String::new();
    }
    if value.chars().count() <= max {
        return value.to_string();
    }
    let mut out: String = value.chars().take(max.saturating_sub(1)).collect();
    out.push('…');
    out
}

fn status_color(status: RunStatus) -> Color {
    match status {
        RunStatus::NotRun => Color::Rgb(78, 84, 102),
        RunStatus::Running => ACCENT,
        RunStatus::Passed => GREEN,
        RunStatus::Failed => RED,
        RunStatus::Blocked => ACCENT2,
        RunStatus::Error => MAGENTA,
    }
}

fn status_chip(status: RunStatus) -> Span<'static> {
    Span::styled(
        format!(
            " {} ",
            status.as_str().replace('_', " ").to_ascii_uppercase()
        ),
        Style::default()
            .fg(BG)
            .bg(status_color(status))
            .add_modifier(Modifier::BOLD),
    )
}

fn priority_color(priority: &str) -> Color {
    match priority.to_ascii_lowercase().as_str() {
        "critical" => RED,
        "high" => ACCENT2,
        "medium" => ACCENT,
        "low" => DIM,
        _ => DIM,
    }
}

fn priority_span(priority: &str) -> Span<'static> {
    Span::styled(
        format!(" {} ", priority.to_ascii_uppercase()),
        Style::default()
            .fg(BG)
            .bg(priority_color(priority))
            .add_modifier(Modifier::BOLD),
    )
}

fn case_status_span(status: &str) -> Span<'static> {
    let color = match status {
        "active" => GREEN,
        "draft" => ACCENT2,
        "archived" => DIM,
        _ => DIM,
    };
    Span::styled(
        status.to_ascii_uppercase(),
        Style::default().fg(color).add_modifier(Modifier::BOLD),
    )
}

fn bug_status_chip(status: &str) -> Span<'static> {
    let color = match status {
        "open" => RED,
        "reopened" => MAGENTA,
        "resolved" => GREEN,
        "closed" => DIM,
        _ => DIM,
    };
    Span::styled(
        format!(" {} ", status.to_ascii_uppercase()),
        Style::default()
            .fg(BG)
            .bg(color)
            .add_modifier(Modifier::BOLD),
    )
}

fn severity_color(severity: i64) -> Color {
    match severity {
        5 => RED,
        4 => ACCENT2,
        3 => ACCENT,
        2 => BLUE,
        _ => DIM,
    }
}

fn number_chip(prefix: &str, number: i64) -> Span<'static> {
    Span::styled(
        format!(" {prefix}{number} "),
        Style::default().fg(TEXT).bg(SELECTED),
    )
}
