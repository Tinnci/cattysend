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
use std::time::Duration;

use app::App;

#[tokio::main]
async fn main() -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // Run app
    let app = App::new();
    let res = run_app(&mut terminal, app).await;

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

async fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> Result<()> {
    loop {
        terminal.draw(|f| ui::draw(f, &app))?;

        // 使用 poll 避免无限阻塞，保证 scan 结果能及时更新 UI
        if event::poll(Duration::from_millis(100))? {
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
        }

        // Update app state (handle async events)
        app.tick();
    }
}
