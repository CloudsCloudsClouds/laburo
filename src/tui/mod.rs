//! Keyboard-first terminal UI.
//!
//! `run` owns the terminal for the duration of the session; everything else in
//! this module is terminal-free and testable with `TestBackend`.

mod app;
mod ui;

use std::path::Path;
use std::time::Duration;

use anyhow::{Context, Result};
use crossterm::event::{self, Event, KeyEventKind};
use ratatui::Terminal;
use ratatui::backend::Backend;

use app::App;

pub fn run(project: &Path) -> Result<()> {
    let mut app = App::open(project)?;
    let mut terminal = ratatui::try_init().context("initializing the terminal")?;
    let result = event_loop(&mut terminal, &mut app);
    ratatui::restore();
    result
}

fn event_loop<B: Backend>(terminal: &mut Terminal<B>, app: &mut App) -> Result<()> {
    loop {
        terminal.draw(|frame| app.draw(frame))?;
        if app.should_quit() {
            break;
        }
        if event::poll(Duration::from_millis(250))? {
            match event::read()? {
                Event::Key(key)
                    if matches!(key.kind, KeyEventKind::Press | KeyEventKind::Repeat) =>
                {
                    app.on_key(key);
                }
                _ => {}
            }
        }
    }
    Ok(())
}
