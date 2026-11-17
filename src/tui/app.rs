//! TUI state, data loading, and keyboard actions.
//!
//! Views stay dumb: [`crate::tui::ui`] only reads this state. All database
//! access and business decisions live here so the shell can be exercised with
//! `TestBackend` and no terminal.

use std::path::{Path, PathBuf};

use anyhow::{Result, anyhow};
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind, KeyModifiers};
use ratatui::Frame;
use ratatui::widgets::TableState;

use crate::config::ProjectConfig;
use crate::db::Db;
use crate::model::{
    Attachment, BugReport, Dashboard, DiscoveredTest, RunMode, RunStatus, TestCase, TestRun,
};
use crate::repo;
use crate::report::{self, ReportKind};

use super::ui;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Tab {
    Cases,
    Runs,
    Bugs,
    Reports,
    Dashboard,
    Drivers,
}

impl Tab {
    pub(crate) const ALL: [Self; 6] = [
        Self::Cases,
        Self::Runs,
        Self::Bugs,
        Self::Reports,
        Self::Dashboard,
        Self::Drivers,
    ];

    pub(crate) const fn title(self) -> &'static str {
        match self {
            Self::Cases => "Cases",
            Self::Runs => "Runs",
            Self::Bugs => "Bugs",
            Self::Reports => "Reports",
            Self::Dashboard => "Dashboard",
            Self::Drivers => "Drivers",
        }
    }

    pub(crate) const fn prev(self) -> Self {
        match self {
            Self::Cases => Self::Drivers,
            Self::Runs => Self::Cases,
            Self::Bugs => Self::Runs,
            Self::Reports => Self::Bugs,
            Self::Dashboard => Self::Reports,
            Self::Drivers => Self::Dashboard,
        }
    }

    pub(crate) const fn next(self) -> Self {
        match self {
            Self::Cases => Self::Runs,
            Self::Runs => Self::Bugs,
            Self::Bugs => Self::Reports,
            Self::Reports => Self::Dashboard,
            Self::Dashboard => Self::Drivers,
            Self::Drivers => Self::Cases,
        }
    }

    fn from_number(c: char) -> Option<Self> {
        match c {
            '1' => Some(Self::Cases),
            '2' => Some(Self::Runs),
            '3' => Some(Self::Bugs),
            '4' => Some(Self::Reports),
            '5' => Some(Self::Dashboard),
            '6' => Some(Self::Drivers),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Screen {
    Browse,
    CaseDetail,
    RunDetail,
    BugDetail,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum MessageLevel {
    Info,
    Ok,
    Error,
}

#[derive(Debug, Clone)]
pub(crate) struct Message {
    pub(crate) text: String,
    pub(crate) level: MessageLevel,
}

#[derive(Debug, Clone)]
pub(crate) struct CaseRow {
    pub(crate) case: TestCase,
    pub(crate) last_status: Option<RunStatus>,
    pub(crate) last_run_at: Option<String>,
}

#[derive(Debug, Clone)]
pub(crate) struct CaseDetail {
    pub(crate) case: TestCase,
    pub(crate) runs: Vec<TestRun>,
    pub(crate) bugs: Vec<BugReport>,
}

#[derive(Debug, Clone)]
pub(crate) struct RunDetail {
    pub(crate) run: TestRun,
    pub(crate) evidence: Vec<Attachment>,
}

#[derive(Debug, Clone)]
pub(crate) enum FormKind {
    NewCase,
    EditTitle { case_id: String },
}

#[derive(Debug, Clone)]
pub(crate) struct FormField {
    pub(crate) label: &'static str,
    pub(crate) value: String,
}

#[derive(Debug, Clone)]
pub(crate) struct FormState {
    pub(crate) kind: FormKind,
    pub(crate) fields: Vec<FormField>,
    pub(crate) focus: usize,
    pub(crate) error: Option<String>,
}

impl FormState {
    fn new_case() -> Self {
        Self {
            kind: FormKind::NewCase,
            fields: vec![
                FormField {
                    label: "ID",
                    value: String::new(),
                },
                FormField {
                    label: "Title",
                    value: String::new(),
                },
            ],
            focus: 0,
            error: None,
        }
    }

    fn edit_title(case_id: &str, title: &str) -> Self {
        Self {
            kind: FormKind::EditTitle {
                case_id: case_id.to_string(),
            },
            fields: vec![FormField {
                label: "Title",
                value: title.to_string(),
            }],
            focus: 0,
            error: None,
        }
    }

    pub(crate) fn title(&self) -> &'static str {
        match self.kind {
            FormKind::NewCase => "New case",
            FormKind::EditTitle { .. } => "Edit case title",
        }
    }
}

#[derive(Debug, Clone)]
pub(crate) struct ConfirmDelete {
    pub(crate) id: String,
    pub(crate) title: String,
}

#[derive(Debug, Clone)]
pub(crate) struct NoteInput {
    pub(crate) run_id: String,
    pub(crate) position: i64,
    pub(crate) step_label: String,
    pub(crate) value: String,
}

#[derive(Debug, Clone)]
pub(crate) struct ManualState {
    pub(crate) run: TestRun,
    pub(crate) cursor: usize,
}

pub(crate) struct App {
    pub(crate) cfg: ProjectConfig,
    pub(crate) db: Db,
    pub(crate) tab: Tab,
    pub(crate) screen: Screen,
    pub(crate) cases: Vec<CaseRow>,
    pub(crate) case_state: TableState,
    pub(crate) runs: Vec<TestRun>,
    pub(crate) run_state: TableState,
    pub(crate) bugs: Vec<BugReport>,
    pub(crate) bug_state: TableState,
    pub(crate) drivers: Vec<DiscoveredTest>,
    pub(crate) driver_state: TableState,
    pub(crate) dashboard: Option<Dashboard>,
    pub(crate) case_detail: Option<CaseDetail>,
    pub(crate) run_detail: Option<RunDetail>,
    pub(crate) bug_detail: Option<BugReport>,
    pub(crate) scroll: u16,
    pub(crate) form: Option<FormState>,
    pub(crate) confirm_delete: Option<ConfirmDelete>,
    pub(crate) note: Option<NoteInput>,
    pub(crate) manual: Option<ManualState>,
    pub(crate) help: bool,
    pub(crate) message: Option<Message>,
    pub(crate) last_report: Option<PathBuf>,
    pub(crate) quit: bool,
}

impl App {
    pub(crate) fn open(project: &Path) -> Result<Self> {
        let cfg = ProjectConfig::resolve(Some(project.to_path_buf()))?;
        cfg.ensure_dirs()?;
        let db = Db::open(&cfg.db_path)?;
        let mut app = Self {
            cfg,
            db,
            tab: Tab::Cases,
            screen: Screen::Browse,
            cases: Vec::new(),
            case_state: TableState::default(),
            runs: Vec::new(),
            run_state: TableState::default(),
            bugs: Vec::new(),
            bug_state: TableState::default(),
            drivers: Vec::new(),
            driver_state: TableState::default(),
            dashboard: None,
            case_detail: None,
            run_detail: None,
            bug_detail: None,
            scroll: 0,
            form: None,
            confirm_delete: None,
            note: None,
            manual: None,
            help: false,
            message: None,
            last_report: None,
            quit: false,
        };
        app.refresh_all();
        if app.message.is_none() {
            app.set_message("Ready — press ? for keys", MessageLevel::Info);
        }
        Ok(app)
    }

    pub(crate) fn draw(&mut self, frame: &mut Frame<'_>) {
        ui::draw(self, frame);
    }

    pub(crate) fn should_quit(&self) -> bool {
        self.quit
    }

    pub(crate) fn set_tab(&mut self, tab: Tab) {
        if self.tab != tab {
            self.tab = tab;
            self.screen = Screen::Browse;
            self.case_detail = None;
            self.run_detail = None;
            self.bug_detail = None;
            self.scroll = 0;
        }
        if tab == Tab::Dashboard {
            self.reload_dashboard();
        }
    }

    // ----------------------------------------------------------------- keys

    pub(crate) fn on_key(&mut self, key: KeyEvent) {
        if key.kind == KeyEventKind::Release {
            return;
        }
        if self.help {
            if matches!(
                key.code,
                KeyCode::Esc | KeyCode::Char('?') | KeyCode::Char('q')
            ) {
                self.help = false;
            }
            return;
        }
        if self.form.is_some() {
            self.form_key(key);
            return;
        }
        if self.confirm_delete.is_some() {
            self.confirm_key(key);
            return;
        }
        if self.note.is_some() {
            self.note_key(key);
            return;
        }
        if self.manual.is_some() {
            self.manual_key(key);
            return;
        }
        if key.modifiers.contains(KeyModifiers::CONTROL) {
            match key.code {
                KeyCode::Char('c') => self.quit = true,
                KeyCode::Char('d') => self.scroll_or_move(8),
                KeyCode::Char('u') => self.scroll_or_move(-8),
                _ => {}
            }
            return;
        }
        match key.code {
            KeyCode::Esc => self.escape(),
            KeyCode::Char('?') => self.help = true,
            KeyCode::Char('q') => self.quit = true,
            KeyCode::Char('h') | KeyCode::Left => self.set_tab(self.tab.prev()),
            KeyCode::Char('l') | KeyCode::Right => self.set_tab(self.tab.next()),
            KeyCode::Char(c @ '1'..='6') => {
                if let Some(tab) = Tab::from_number(c) {
                    self.set_tab(tab);
                }
            }
            KeyCode::Char('j') | KeyCode::Down => self.move_down(),
            KeyCode::Char('k') | KeyCode::Up => self.move_up(),
            KeyCode::Char('g') => self.go_first(),
            KeyCode::Char('G') => self.go_last(),
            KeyCode::Enter => self.activate(),
            KeyCode::Char('r') => self.run_action(),
            KeyCode::Char('c') => self.new_case_action(),
            KeyCode::Char('e') => self.edit_action(),
            KeyCode::Char('d') => self.delete_or_dictionary(),
            KeyCode::Char('x') => self.report_action(),
            _ => {}
        }
    }

    fn escape(&mut self) {
        if self.screen != Screen::Browse {
            self.screen = Screen::Browse;
            self.case_detail = None;
            self.run_detail = None;
            self.bug_detail = None;
            self.scroll = 0;
        } else {
            self.message = None;
        }
    }

    fn move_down(&mut self) {
        match self.screen {
            Screen::Browse => self.move_list(1),
            _ => self.scroll = self.scroll.saturating_add(1),
        }
    }

    fn move_up(&mut self) {
        match self.screen {
            Screen::Browse => self.move_list(-1),
            _ => self.scroll = self.scroll.saturating_sub(1),
        }
    }

    fn scroll_or_move(&mut self, delta: i32) {
        match self.screen {
            Screen::Browse => self.move_list(delta),
            _ => {
                let next = i32::from(self.scroll) + delta;
                self.scroll = u16::try_from(next.max(0)).unwrap_or(u16::MAX);
            }
        }
    }

    fn move_list(&mut self, delta: i32) {
        match self.tab {
            Tab::Cases | Tab::Reports => {
                if self.cases.is_empty() {
                    return;
                }
                let current = self.case_index() as i32;
                let last = self.cases.len() as i32 - 1;
                self.case_state
                    .select(Some((current + delta).clamp(0, last) as usize));
            }
            Tab::Runs => {
                if self.runs.is_empty() {
                    return;
                }
                let current = self.run_index() as i32;
                let last = self.runs.len() as i32 - 1;
                self.run_state
                    .select(Some((current + delta).clamp(0, last) as usize));
            }
            Tab::Bugs => {
                if self.bugs.is_empty() {
                    return;
                }
                let current = self.bug_index() as i32;
                let last = self.bugs.len() as i32 - 1;
                self.bug_state
                    .select(Some((current + delta).clamp(0, last) as usize));
            }
            Tab::Drivers => {
                if self.drivers.is_empty() {
                    return;
                }
                let current = self.driver_index() as i32;
                let last = self.drivers.len() as i32 - 1;
                self.driver_state
                    .select(Some((current + delta).clamp(0, last) as usize));
            }
            Tab::Dashboard => {}
        }
    }

    fn go_first(&mut self) {
        match self.screen {
            Screen::Browse => match self.tab {
                Tab::Cases | Tab::Reports if !self.cases.is_empty() => {
                    self.case_state.select(Some(0));
                }
                Tab::Runs if !self.runs.is_empty() => self.run_state.select(Some(0)),
                Tab::Bugs if !self.bugs.is_empty() => self.bug_state.select(Some(0)),
                Tab::Drivers if !self.drivers.is_empty() => {
                    self.driver_state.select(Some(0));
                }
                _ => {}
            },
            _ => self.scroll = 0,
        }
    }

    fn go_last(&mut self) {
        match self.screen {
            Screen::Browse => match self.tab {
                Tab::Cases | Tab::Reports if !self.cases.is_empty() => {
                    self.case_state.select(Some(self.cases.len() - 1));
                }
                Tab::Runs if !self.runs.is_empty() => {
                    self.run_state.select(Some(self.runs.len() - 1));
                }
                Tab::Bugs if !self.bugs.is_empty() => {
                    self.bug_state.select(Some(self.bugs.len() - 1));
                }
                Tab::Drivers if !self.drivers.is_empty() => {
                    self.driver_state.select(Some(self.drivers.len() - 1));
                }
                _ => {}
            },
            _ => self.scroll = u16::MAX,
        }
    }

    fn activate(&mut self) {
        if self.screen != Screen::Browse {
            return;
        }
        match self.tab {
            Tab::Cases => self.open_case_detail(),
            Tab::Runs => self.open_run_detail(),
            Tab::Bugs => self.open_bug_detail(),
            Tab::Reports | Tab::Dashboard | Tab::Drivers => {}
        }
    }

    fn run_action(&mut self) {
        let allowed = self.screen == Screen::CaseDetail
            || (self.screen == Screen::Browse && self.tab == Tab::Cases);
        if allowed {
            self.start_manual_run();
        }
    }

    fn new_case_action(&mut self) {
        let allowed = self.screen == Screen::CaseDetail
            || (self.screen == Screen::Browse && matches!(self.tab, Tab::Cases | Tab::Reports));
        if allowed {
            self.form = Some(FormState::new_case());
        }
    }

    fn edit_action(&mut self) {
        let allowed = self.screen == Screen::CaseDetail
            || (self.screen == Screen::Browse && self.tab == Tab::Cases);
        if !allowed {
            return;
        }
        let Some(case) = self.current_case() else {
            self.set_message("No case selected", MessageLevel::Info);
            return;
        };
        let form = FormState::edit_title(&case.id, &case.title);
        self.form = Some(form);
    }

    fn delete_or_dictionary(&mut self) {
        if self.screen == Screen::Browse && self.tab == Tab::Reports {
            self.generate_dictionary();
            return;
        }
        let allowed = self.screen == Screen::CaseDetail
            || (self.screen == Screen::Browse && self.tab == Tab::Cases);
        if !allowed {
            return;
        }
        let Some(case) = self.current_case() else {
            self.set_message("No case selected", MessageLevel::Info);
            return;
        };
        self.confirm_delete = Some(ConfirmDelete {
            id: case.id.clone(),
            title: case.title.clone(),
        });
    }

    fn report_action(&mut self) {
        let allowed = self.screen == Screen::CaseDetail
            || (self.screen == Screen::Browse && matches!(self.tab, Tab::Cases | Tab::Reports));
        if allowed {
            self.generate_case_report();
        }
    }

    // ------------------------------------------------------------- manual run

    fn manual_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Char('p') => self.record_manual(RunStatus::Passed),
            KeyCode::Char('f') => self.record_manual(RunStatus::Failed),
            KeyCode::Char('m') => self.record_manual(RunStatus::Blocked),
            KeyCode::Char('n') => self.open_note(),
            KeyCode::Char('F') | KeyCode::Esc | KeyCode::Char('q') => self.finish_manual(),
            KeyCode::Enter => self.advance_manual(),
            KeyCode::Char('j') | KeyCode::Down => self.move_manual(1),
            KeyCode::Char('k') | KeyCode::Up => self.move_manual(-1),
            KeyCode::Char('g') => {
                if let Some(manual) = &mut self.manual {
                    manual.cursor = 0;
                }
            }
            KeyCode::Char('G') => {
                if let Some(manual) = &mut self.manual {
                    manual.cursor = manual.run.steps.len().saturating_sub(1);
                }
            }
            _ => {}
        }
    }

    fn start_manual_run(&mut self) {
        let Some(case) = self.current_case().cloned() else {
            self.set_message("No case selected", MessageLevel::Info);
            return;
        };
        match repo::create_run(&self.db.conn, &case.id, RunMode::Manual, &case.environment) {
            Ok(run) => {
                let steps = run.steps.len();
                self.manual = Some(ManualState { run, cursor: 0 });
                self.set_message(
                    format!("Manual run started · {steps} step(s) · p/f/m records a result"),
                    MessageLevel::Info,
                );
            }
            Err(err) => self.error("starting run", err),
        }
    }

    fn current_manual_step(&self) -> Option<(String, crate::model::StepRun)> {
        self.manual.as_ref().and_then(|manual| {
            manual
                .run
                .steps
                .get(manual.cursor)
                .map(|step| (manual.run.id.clone(), step.clone()))
        })
    }

    fn record_manual(&mut self, status: RunStatus) {
        let Some((run_id, step)) = self.current_manual_step() else {
            self.set_message("There is no current step", MessageLevel::Info);
            return;
        };
        match repo::record_step(
            &self.db.conn,
            &run_id,
            step.position,
            status,
            &step.observed,
            &step.notes,
            step.duration_ms,
            step.driver_test.as_deref(),
            &step.raw_output,
        ) {
            Ok(_) => {
                self.reload_manual_run(&run_id, true);
                let level = if status == RunStatus::Passed {
                    MessageLevel::Ok
                } else {
                    MessageLevel::Info
                };
                self.set_message(
                    format!("Step {} · {}", step.position, status.as_str()),
                    level,
                );
            }
            Err(err) => self.error("recording step", err),
        }
    }

    fn reload_manual_run(&mut self, run_id: &str, advance: bool) {
        match repo::get_run(&self.db.conn, run_id) {
            Ok(Some(run)) => {
                if let Some(manual) = &mut self.manual {
                    manual.run = run;
                    if advance && manual.cursor + 1 < manual.run.steps.len() {
                        manual.cursor += 1;
                    }
                }
            }
            Ok(None) => {
                self.manual = None;
                self.set_message("Run disappeared", MessageLevel::Error);
            }
            Err(err) => self.error("refreshing run", err),
        }
    }

    fn advance_manual(&mut self) {
        let mut at_end = false;
        if let Some(manual) = &mut self.manual {
            if manual.cursor + 1 < manual.run.steps.len() {
                manual.cursor += 1;
            } else {
                at_end = true;
            }
        }
        if at_end {
            self.set_message("Last step · press F to finish the run", MessageLevel::Info);
        }
    }

    fn move_manual(&mut self, delta: i32) {
        if let Some(manual) = &mut self.manual {
            if manual.run.steps.is_empty() {
                return;
            }
            let current = manual.cursor as i32;
            let last = manual.run.steps.len() as i32 - 1;
            manual.cursor = (current + delta).clamp(0, last) as usize;
        }
    }

    fn open_note(&mut self) {
        let Some((run_id, step)) = self.current_manual_step() else {
            self.set_message("There is no current step", MessageLevel::Info);
            return;
        };
        self.note = Some(NoteInput {
            run_id,
            position: step.position,
            step_label: format!("step {}", step.position),
            value: step.notes,
        });
    }

    fn note_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.note = None,
            KeyCode::Enter => self.submit_note(),
            _ => {
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    || key.modifiers.contains(KeyModifiers::ALT)
                {
                    return;
                }
                if let Some(note) = &mut self.note {
                    match key.code {
                        KeyCode::Backspace => {
                            note.value.pop();
                        }
                        KeyCode::Char(c) => note.value.push(c),
                        _ => {}
                    }
                }
            }
        }
    }

    fn submit_note(&mut self) {
        let Some(note) = self.note.take() else {
            return;
        };
        let step = self.manual.as_ref().and_then(|manual| {
            manual
                .run
                .steps
                .iter()
                .find(|step| step.position == note.position)
                .cloned()
        });
        let Some(step) = step else {
            self.set_message("The step is no longer available", MessageLevel::Error);
            return;
        };
        match repo::record_step(
            &self.db.conn,
            &note.run_id,
            note.position,
            step.status,
            &step.observed,
            &note.value,
            step.duration_ms,
            step.driver_test.as_deref(),
            &step.raw_output,
        ) {
            Ok(_) => {
                self.reload_manual_run(&note.run_id, false);
                self.set_message("Note saved", MessageLevel::Ok);
            }
            Err(err) => {
                self.note = Some(note);
                self.error("saving note", err);
            }
        }
    }

    fn finish_manual(&mut self) {
        let Some(manual) = self.manual.take() else {
            return;
        };
        match repo::finish_run(&self.db.conn, &manual.run.id) {
            Ok(run) => {
                let level = match run.status {
                    RunStatus::Passed => MessageLevel::Ok,
                    RunStatus::Failed | RunStatus::Error => MessageLevel::Error,
                    _ => MessageLevel::Info,
                };
                self.set_message(
                    format!("Run {} finished · {}", run.id, run.status.as_str()),
                    level,
                );
                self.refresh_all();
                self.reload_case_detail();
            }
            Err(err) => {
                self.manual = Some(manual);
                self.error("finishing run", err);
            }
        }
    }

    // ---------------------------------------------------------------- forms

    fn form_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Esc => self.form = None,
            KeyCode::Enter => self.submit_form(),
            KeyCode::Tab | KeyCode::Down => {
                if let Some(form) = &mut self.form {
                    if !form.fields.is_empty() {
                        form.focus = (form.focus + 1) % form.fields.len();
                    }
                }
            }
            KeyCode::BackTab | KeyCode::Up => {
                if let Some(form) = &mut self.form {
                    if !form.fields.is_empty() {
                        form.focus = (form.focus + form.fields.len() - 1) % form.fields.len();
                    }
                }
            }
            _ => {
                if key.modifiers.contains(KeyModifiers::CONTROL)
                    || key.modifiers.contains(KeyModifiers::ALT)
                {
                    return;
                }
                if let Some(form) = &mut self.form {
                    match key.code {
                        KeyCode::Backspace => {
                            if let Some(field) = form.fields.get_mut(form.focus) {
                                field.value.pop();
                            }
                        }
                        KeyCode::Char(c) => {
                            if let Some(field) = form.fields.get_mut(form.focus) {
                                field.value.push(c);
                            }
                        }
                        _ => {}
                    }
                }
            }
        }
    }

    fn submit_form(&mut self) {
        let Some(form) = self.form.clone() else {
            return;
        };
        match form.kind {
            FormKind::NewCase => {
                let id = form
                    .fields
                    .first()
                    .map(|field| field.value.trim().to_string())
                    .unwrap_or_default();
                let title = form
                    .fields
                    .get(1)
                    .map(|field| field.value.trim().to_string())
                    .unwrap_or_default();
                if id.is_empty() || title.is_empty() {
                    self.form_error("ID and title are required");
                    return;
                }
                match repo::create_case(&self.db.conn, &id, &title, "", "", "medium", "", "", &[]) {
                    Ok(case) => {
                        self.form = None;
                        self.screen = Screen::Browse;
                        self.tab = Tab::Cases;
                        self.case_detail = None;
                        self.refresh_all();
                        self.select_case_id(&case.id);
                        self.set_message(format!("Case {} created", case.id), MessageLevel::Ok);
                    }
                    Err(err) => self.form_error(format!("{err:#}")),
                }
            }
            FormKind::EditTitle { case_id } => {
                let title = form
                    .fields
                    .first()
                    .map(|field| field.value.trim().to_string())
                    .unwrap_or_default();
                if title.is_empty() {
                    self.form_error("Title is required");
                    return;
                }
                match repo::get_case(&self.db.conn, &case_id) {
                    Ok(Some(mut case)) => {
                        case.title.clone_from(&title);
                        match repo::update_case(&self.db.conn, &case) {
                            Ok(()) => {
                                self.form = None;
                                if let Some(detail) = &mut self.case_detail {
                                    if detail.case.id == case_id {
                                        detail.case.title = title;
                                    }
                                }
                                self.refresh_all();
                                self.reload_case_detail();
                                self.set_message(
                                    format!("Case {case_id} updated"),
                                    MessageLevel::Ok,
                                );
                            }
                            Err(err) => self.form_error(format!("{err:#}")),
                        }
                    }
                    Ok(None) => self.form_error(format!("case not found: {case_id}")),
                    Err(err) => self.form_error(format!("{err:#}")),
                }
            }
        }
    }

    fn form_error(&mut self, text: impl Into<String>) {
        let text = text.into();
        if let Some(form) = &mut self.form {
            form.error = Some(text.clone());
        }
        self.set_message(text, MessageLevel::Error);
    }

    // -------------------------------------------------------------- confirm

    fn confirm_key(&mut self, key: KeyEvent) {
        match key.code {
            KeyCode::Enter | KeyCode::Char('y') | KeyCode::Char('Y') => self.delete_confirmed(),
            KeyCode::Esc | KeyCode::Char('n') | KeyCode::Char('N') | KeyCode::Char('q') => {
                self.confirm_delete = None
            }
            _ => {}
        }
    }

    fn delete_confirmed(&mut self) {
        let Some(confirm) = self.confirm_delete.take() else {
            return;
        };
        match repo::delete_case(&self.db.conn, &confirm.id) {
            Ok(true) => {
                self.screen = Screen::Browse;
                self.tab = Tab::Cases;
                self.case_detail = None;
                self.refresh_all();
                self.set_message(format!("Case {} deleted", confirm.id), MessageLevel::Ok);
            }
            Ok(false) => {
                self.refresh_all();
                self.set_message(
                    format!("Case {} was not found", confirm.id),
                    MessageLevel::Error,
                );
            }
            Err(err) => {
                self.confirm_delete = Some(confirm);
                self.error("deleting case", err);
            }
        }
    }

    // -------------------------------------------------------------- reports

    fn generate_dictionary(&mut self) {
        self.generate_report(ReportKind::Dictionary, None);
    }

    fn generate_case_report(&mut self) {
        let Some(case) = self.current_case() else {
            self.set_message("No case selected", MessageLevel::Info);
            return;
        };
        let id = case.id.clone();
        self.generate_report(ReportKind::Case, Some(id));
    }

    fn generate_report(&mut self, kind: ReportKind, id: Option<String>) {
        match report::generate(&self.db.conn, &self.cfg.data_dir, kind, id.as_deref()) {
            Ok(path) => {
                self.last_report = Some(path.clone());
                self.set_message(
                    format!("Report written to {}", path.display()),
                    MessageLevel::Ok,
                );
            }
            Err(err) => self.set_message(format!("Report failed: {err:#}"), MessageLevel::Error),
        }
    }

    // ----------------------------------------------------------- load data

    fn refresh_all(&mut self) {
        self.reload_cases();
        self.reload_runs();
        self.reload_bugs();
        self.reload_drivers();
        self.reload_dashboard();
    }

    fn reload_cases(&mut self) {
        let previous = self
            .cases
            .get(self.case_index())
            .map(|row| row.case.id.clone());
        match repo::list_cases(&self.db.conn, None, None) {
            Ok(cases) => {
                let mut rows = Vec::with_capacity(cases.len());
                for case in cases {
                    let last = match repo::list_runs(&self.db.conn, Some(&case.id), 1) {
                        Ok(mut runs) => runs.pop(),
                        Err(_) => None,
                    };
                    rows.push(CaseRow {
                        case,
                        last_status: last.as_ref().map(|run| run.status),
                        last_run_at: last.map(|run| run.started_at),
                    });
                }
                self.cases = rows;
                let index = previous
                    .and_then(|id| self.cases.iter().position(|row| row.case.id == id))
                    .unwrap_or(0);
                self.select_case(index);
            }
            Err(err) => self.error("loading cases", err),
        }
    }

    fn reload_runs(&mut self) {
        let previous = self.runs.get(self.run_index()).map(|run| run.id.clone());
        match repo::list_runs(&self.db.conn, None, 100) {
            Ok(runs) => {
                self.runs = runs;
                let index = previous
                    .and_then(|id| self.runs.iter().position(|run| run.id == id))
                    .unwrap_or(0);
                self.select_run(index);
            }
            Err(err) => self.error("loading runs", err),
        }
    }

    fn reload_bugs(&mut self) {
        let previous = self.bugs.get(self.bug_index()).map(|bug| bug.id.clone());
        match repo::list_bugs(&self.db.conn, None) {
            Ok(bugs) => {
                self.bugs = bugs;
                let index = previous
                    .and_then(|id| self.bugs.iter().position(|bug| bug.id == id))
                    .unwrap_or(0);
                self.select_bug(index);
            }
            Err(err) => self.error("loading bugs", err),
        }
    }

    fn reload_dashboard(&mut self) {
        match repo::dashboard(&self.db.conn) {
            Ok(dashboard) => self.dashboard = Some(dashboard),
            Err(err) => self.error("loading dashboard", err),
        }
    }

    fn reload_drivers(&mut self) {
        let previous = self
            .drivers
            .get(self.driver_index())
            .map(|test| test.id.clone());
        match repo::list_driver_tests(&self.db.conn, None) {
            Ok(drivers) => {
                self.drivers = drivers;
                let index = previous
                    .and_then(|id| self.drivers.iter().position(|test| test.id == id))
                    .unwrap_or(0);
                self.select_driver(index);
            }
            Err(err) => self.error("loading drivers", err),
        }
    }

    fn load_case_detail(&self, case_id: &str) -> Result<CaseDetail> {
        let case = repo::get_case(&self.db.conn, case_id)?
            .ok_or_else(|| anyhow!("case not found: {case_id}"))?;
        let runs = repo::list_runs(&self.db.conn, Some(case_id), 20)?;
        let mut bugs = repo::list_bugs(&self.db.conn, None)?;
        bugs.retain(|bug| bug.case_id.as_deref() == Some(case_id));
        Ok(CaseDetail { case, runs, bugs })
    }

    fn open_case_detail(&mut self) {
        let Some(case_id) = self
            .cases
            .get(self.case_index())
            .map(|row| row.case.id.clone())
        else {
            self.set_message("No cases yet · press c to create one", MessageLevel::Info);
            return;
        };
        match self.load_case_detail(&case_id) {
            Ok(detail) => {
                self.case_detail = Some(detail);
                self.screen = Screen::CaseDetail;
                self.scroll = 0;
            }
            Err(err) => self.error("loading case", err),
        }
    }

    fn reload_case_detail(&mut self) {
        let Some(case_id) = self
            .case_detail
            .as_ref()
            .map(|detail| detail.case.id.clone())
        else {
            return;
        };
        match self.load_case_detail(&case_id) {
            Ok(detail) => self.case_detail = Some(detail),
            Err(err) => self.error("refreshing case", err),
        }
    }

    fn open_run_detail(&mut self) {
        let Some(run_id) = self.runs.get(self.run_index()).map(|run| run.id.clone()) else {
            self.set_message("No runs recorded yet", MessageLevel::Info);
            return;
        };
        match repo::get_run(&self.db.conn, &run_id) {
            Ok(Some(run)) => {
                let evidence = match repo::load_evidence(
                    &self.db.conn,
                    repo::EvidenceOwner::Run(run_id.clone()),
                ) {
                    Ok(evidence) => evidence,
                    Err(err) => {
                        self.error("loading run evidence", err);
                        Vec::new()
                    }
                };
                self.run_detail = Some(RunDetail { run, evidence });
                self.screen = Screen::RunDetail;
                self.scroll = 0;
            }
            Ok(None) => self.set_message(format!("run not found: {run_id}"), MessageLevel::Error),
            Err(err) => self.error("loading run", err),
        }
    }

    fn open_bug_detail(&mut self) {
        let Some(bug_id) = self.bugs.get(self.bug_index()).map(|bug| bug.id.clone()) else {
            self.set_message("No bugs recorded yet", MessageLevel::Info);
            return;
        };
        match repo::get_bug(&self.db.conn, &bug_id) {
            Ok(Some(bug)) => {
                self.bug_detail = Some(bug);
                self.screen = Screen::BugDetail;
                self.scroll = 0;
            }
            Ok(None) => self.set_message(format!("bug not found: {bug_id}"), MessageLevel::Error),
            Err(err) => self.error("loading bug", err),
        }
    }

    // --------------------------------------------------------------- helpers

    fn current_case(&self) -> Option<&TestCase> {
        if let Some(detail) = &self.case_detail {
            return Some(&detail.case);
        }
        self.cases.get(self.case_index()).map(|row| &row.case)
    }

    fn case_index(&self) -> usize {
        self.case_state.selected().unwrap_or(0)
    }

    fn run_index(&self) -> usize {
        self.run_state.selected().unwrap_or(0)
    }

    fn bug_index(&self) -> usize {
        self.bug_state.selected().unwrap_or(0)
    }

    fn driver_index(&self) -> usize {
        self.driver_state.selected().unwrap_or(0)
    }

    fn select_case(&mut self, index: usize) {
        if self.cases.is_empty() {
            self.case_state.select(None);
        } else {
            self.case_state
                .select(Some(index.min(self.cases.len() - 1)));
        }
    }

    fn select_run(&mut self, index: usize) {
        if self.runs.is_empty() {
            self.run_state.select(None);
        } else {
            self.run_state.select(Some(index.min(self.runs.len() - 1)));
        }
    }

    fn select_bug(&mut self, index: usize) {
        if self.bugs.is_empty() {
            self.bug_state.select(None);
        } else {
            self.bug_state.select(Some(index.min(self.bugs.len() - 1)));
        }
    }

    fn select_driver(&mut self, index: usize) {
        if self.drivers.is_empty() {
            self.driver_state.select(None);
        } else {
            self.driver_state
                .select(Some(index.min(self.drivers.len() - 1)));
        }
    }

    fn select_case_id(&mut self, id: &str) {
        if let Some(index) = self.cases.iter().position(|row| row.case.id == id) {
            self.case_state.select(Some(index));
        }
    }

    fn set_message(&mut self, text: impl Into<String>, level: MessageLevel) {
        self.message = Some(Message {
            text: text.into(),
            level,
        });
    }

    fn error(&mut self, context: &str, err: anyhow::Error) {
        self.set_message(format!("{context}: {err:#}"), MessageLevel::Error);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::model::StepKind;
    use ratatui::Terminal;
    use ratatui::backend::TestBackend;

    fn buffer_text(terminal: &Terminal<TestBackend>) -> String {
        terminal
            .backend()
            .buffer()
            .content
            .iter()
            .map(ratatui::buffer::Cell::symbol)
            .collect()
    }

    #[test]
    fn tui_smoke_constructs_app_and_draws_without_terminal() {
        let dir = tempfile::tempdir().unwrap();
        let mut app = App::open(dir.path()).unwrap();

        let case = repo::create_case(
            &app.db.conn,
            "TC-001",
            "Login works",
            "Users can sign in.",
            "auth",
            "high",
            "test",
            "",
            &["smoke".to_string()],
        )
        .unwrap();
        repo::add_step(
            &app.db.conn,
            &case.id,
            1,
            "Open the login page",
            "",
            "The form is visible",
            StepKind::Manual,
            None,
        )
        .unwrap();
        let run = repo::create_run(&app.db.conn, &case.id, RunMode::Manual, "test").unwrap();
        repo::record_step(
            &app.db.conn,
            &run.id,
            1,
            RunStatus::Passed,
            "Form visible",
            "looks right",
            None,
            None,
            "",
        )
        .unwrap();
        repo::finish_run(&app.db.conn, &run.id).unwrap();
        repo::create_bug(
            &app.db.conn,
            Some("BUG-1"),
            "Form misaligned",
            "",
            "",
            "",
            "Aligned",
            2,
            2,
            "test",
            Some("TC-001"),
            None,
            None,
            "",
        )
        .unwrap();
        repo::ensure_driver(&app.db.conn, "minitest", "minitest", "Minitest").unwrap();
        repo::save_driver_tests(
            &app.db.conn,
            "minitest",
            &[DiscoveredTest {
                id: "test/models/user_test.rb#UserTest#test_valid".to_string(),
                driver_id: "minitest".to_string(),
                name: "UserTest#test_valid".to_string(),
                file: "test/models/user_test.rb".to_string(),
                line: Some(12),
                metadata: serde_json::json!({}),
            }],
        )
        .unwrap();
        app.refresh_all();

        let mut terminal = Terminal::new(TestBackend::new(120, 40)).unwrap();
        for tab in Tab::ALL {
            app.set_tab(tab);
            terminal.draw(|frame| app.draw(frame)).unwrap();
            let text = buffer_text(&terminal);
            assert!(
                !text.trim().is_empty(),
                "tab {} rendered nothing",
                tab.title()
            );
        }

        app.set_tab(Tab::Cases);
        app.open_case_detail();
        terminal.draw(|frame| app.draw(frame)).unwrap();
        assert!(buffer_text(&terminal).contains("Login works"));

        app.set_tab(Tab::Cases);
        app.on_key(KeyEvent::new(KeyCode::Char('6'), KeyModifiers::NONE));
        assert_eq!(app.tab, Tab::Drivers);
        terminal.draw(|frame| app.draw(frame)).unwrap();
        assert!(buffer_text(&terminal).contains("UserTest#test_valid"));
        assert!(buffer_text(&terminal).contains("test/models/user_test.rb"));

        app.set_tab(Tab::Runs);
        if let Some(index) = app.runs.iter().position(|item| item.id == run.id) {
            app.run_state.select(Some(index));
        }
        app.open_run_detail();
        terminal.draw(|frame| app.draw(frame)).unwrap();
        assert!(buffer_text(&terminal).contains("RUN-"));

        app.set_tab(Tab::Bugs);
        if let Some(index) = app.bugs.iter().position(|bug| bug.id == "BUG-1") {
            app.bug_state.select(Some(index));
        }
        app.open_bug_detail();
        terminal.draw(|frame| app.draw(frame)).unwrap();
        assert!(buffer_text(&terminal).contains("BUG-1"));

        app.set_tab(Tab::Cases);
        if let Some(index) = app.cases.iter().position(|row| row.case.id == "TC-001") {
            app.case_state.select(Some(index));
        }
        app.start_manual_run();
        terminal.draw(|frame| app.draw(frame)).unwrap();
        assert!(buffer_text(&terminal).contains("Manual run"));
        app.on_key(KeyEvent::new(KeyCode::Char('p'), KeyModifiers::NONE));
        app.on_key(KeyEvent::new(KeyCode::Char('n'), KeyModifiers::NONE));
        assert!(app.note.is_some());
        app.on_key(KeyEvent::new(KeyCode::Char('x'), KeyModifiers::NONE));
        app.on_key(KeyEvent::new(KeyCode::Enter, KeyModifiers::NONE));
        assert!(app.note.is_none());
        assert!(
            app.manual
                .as_ref()
                .unwrap()
                .run
                .steps
                .iter()
                .any(|step| step.notes.contains('x'))
        );
        app.on_key(KeyEvent::new(KeyCode::Char('F'), KeyModifiers::NONE));
        assert!(app.manual.is_none());
        assert!(!app.should_quit());
    }
}
