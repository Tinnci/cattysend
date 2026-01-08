//! Cattysend TUI - 交互式终端界面
//!
//! 使用 ratatui 提供：
//! - 实时设备扫描列表
//! - 传输进度条
//! - RSSI 信号强度显示

mod app;
mod ui;

use anyhow::Result;
use crossterm::{
    event::{self, DisableMouseCapture, EnableMouseCapture, Event, KeyCode, KeyEventKind},
    execute,
    terminal::{EnterAlternateScreen, LeaveAlternateScreen, disable_raw_mode, enable_raw_mode},
};
use ratatui::prelude::*;
use std::io;

use app::App;

fn main() -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run app
    let app = App::new();
    let res = run_app(&mut terminal, app);

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    if let Err(err) = res {
        eprintln!("Error: {:?}", err);
    }

    Ok(())
}

fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> Result<()> {
    loop {
        terminal.draw(|f| ui::draw(f, &app))?;

        if let Event::Key(key) = event::read()? {
            if key.kind == KeyEventKind::Press {
                match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => return Ok(()),
                    KeyCode::Char('s') => app.start_scan(),
                    KeyCode::Char('r') => app.toggle_receive_mode(),
                    KeyCode::Up | KeyCode::Char('k') => app.previous_device(),
                    KeyCode::Down | KeyCode::Char('j') => app.next_device(),
                    KeyCode::Enter => app.select_device(),
                    KeyCode::Tab => app.next_tab(),
                    _ => {}
                }
            }
        }

        // Update app state (simulated for now)
        app.tick();
    }
}
