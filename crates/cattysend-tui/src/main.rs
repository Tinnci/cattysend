//! Cattysend TUI - 交互式终端界面
//!
//! 使用 ratatui 提供实时设备扫描和传输界面。
//!
//! # 日志
//!
//! 日志默认显示在 TUI 的"日志"标签页中。
//! 如需输出到文件进行调试，设置 RUST_LOG 环境变量：
//!
//! ```bash
//! RUST_LOG=debug cargo run -p cattysend-tui 2>> /tmp/cattysend.log
//! ```

mod app;
mod tui_log;
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
use tracing_subscriber::{EnvFilter, layer::SubscriberExt, util::SubscriberInitExt};

use app::App;
use tui_log::TuiLogLayer;

#[tokio::main]
async fn main() -> Result<()> {
    // Setup terminal
    enable_raw_mode()?;
    let mut stdout = io::stdout();
    execute!(stdout, EnterAlternateScreen, EnableMouseCapture)?;
    let backend = CrosstermBackend::new(stdout);
    let mut terminal = Terminal::new(backend)?;

    // 解析命令行参数（简单的文件路径）
    let args: Vec<String> = std::env::args().collect();
    let file_path = if args.len() > 1 {
        Some(args[1].clone())
    } else {
        None
    };

    // 创建 App（获取日志发送器）
    let mut app = App::new();
    if let Some(path) = file_path {
        app.set_file_to_send(path);
    }

    // 初始化日志系统，发送到 TUI 日志面板
    init_logging(app.event_tx.clone());

    // Run app
    let res = run_app(&mut terminal, app).await;

    // Restore terminal
    disable_raw_mode()?;
    execute!(
        terminal.backend_mut(),
        LeaveAlternateScreen,
        DisableMouseCapture
    )?;
    terminal.show_cursor()?;

    res
}

/// 初始化日志系统
///
/// - 总是将日志发送到 TUI 日志面板
/// - 如果设置了 RUST_LOG，同时输出到 stderr（用于调试）
fn init_logging(log_tx: tokio::sync::mpsc::Sender<app::AppEvent>) {
    // 桥接 log crate（cattysend-core 使用）到 tracing
    let _ = tracing_log::LogTracer::init();

    // TUI 日志层 - 总是启用
    let tui_layer = TuiLogLayer::new(log_tx);

    // 设置过滤器
    let filter = EnvFilter::try_from_default_env().unwrap_or_else(|_| {
        // 默认只显示 info 及以上级别
        EnvFilter::new("info,cattysend_core=debug")
    });

    // 如果设置了 RUST_LOG，同时输出到 stderr
    if std::env::var("RUST_LOG").is_ok() {
        use tracing_subscriber::fmt;

        let stderr_layer = fmt::layer()
            .with_writer(io::stderr)
            .with_target(true)
            .compact();

        let _ = tracing_subscriber::registry()
            .with(filter)
            .with(tui_layer)
            .with(stderr_layer)
            .try_init();
    } else {
        let _ = tracing_subscriber::registry()
            .with(filter)
            .with(tui_layer)
            .try_init();
    }
}

async fn run_app<B: Backend>(terminal: &mut Terminal<B>, mut app: App) -> Result<()> {
    loop {
        terminal.draw(|f| ui::draw(f, &app))?;

        // 使用 poll 避免无限阻塞
        if event::poll(Duration::from_millis(100))?
            && let Event::Key(key) = event::read()?
            && key.kind == KeyEventKind::Press
        {
            // 如果正在显示权限警告弹窗，拦截所有按键以关闭它
            if app.show_perm_warning {
                app.dismiss_warning();
                continue;
            }

            match app.mode {
                app::AppMode::Settings => match key.code {
                    KeyCode::Esc => app.mode = app::AppMode::Idle,
                    KeyCode::Enter => {
                        app.settings.device_name = app.input_buffer.clone();
                        let _ = app.settings.save();
                        app.add_log(
                            app::LogLevel::Info,
                            format!("设备名称已更新为: {}", app.settings.device_name),
                        );
                        app.mode = app::AppMode::Idle;
                    }
                    KeyCode::Char(c) => app.input_buffer.push(c),
                    KeyCode::Backspace => {
                        app.input_buffer.pop();
                    }
                    _ => {}
                },
                app::AppMode::FileSelection => match key.code {
                    KeyCode::Esc => app.mode = app::AppMode::Idle,
                    KeyCode::Up | KeyCode::Char('k') => app.file_selector.previous(),
                    KeyCode::Down | KeyCode::Char('j') => app.file_selector.next(),
                    KeyCode::Enter => {
                        if let Some(path) = app.file_selector.enter() {
                            app.set_file_to_send(path.clone());
                            app.mode = app::AppMode::Idle;

                            // Trigger send immediately if we have a valid device selected
                            // This creates a smoother flow: Enter on Device -> Select File -> Auto Send
                            // We need to check if we can send.
                            if let Some(device) = app.devices.get(app.selected_device).cloned() {
                                app.run_sender(device.address.clone(), path);
                            }
                        }
                    }
                    _ => {}
                },
                _ => match key.code {
                    KeyCode::Char('q') | KeyCode::Esc => {
                        return Ok(());
                    }
                    KeyCode::Char('s') => {
                        app.start_scan();
                    }
                    KeyCode::Char('r') => {
                        app.toggle_receive_mode();
                    }
                    KeyCode::Char('p') => {
                        app.input_buffer = app.settings.device_name.clone();
                        app.mode = app::AppMode::Settings;
                    }
                    KeyCode::Up | KeyCode::Char('k') => app.previous_device(),
                    KeyCode::Down | KeyCode::Char('j') => app.next_device(),
                    KeyCode::Enter => {
                        // Enter Logic priority:
                        // 1. If file is ready -> Send
                        // 2. If NO file -> Enter File Selection
                        if let Some(file_path) = app.file_to_send.clone() {
                            if let Some(device) = app.devices.get(app.selected_device).cloned() {
                                app.run_sender(device.address.clone(), file_path);
                            } else {
                                app.add_log(app::LogLevel::Warn, "无效的设备选择".to_string());
                            }
                        } else {
                            // Only allow file selection if we have devices to send to,
                            // or generally allow it to set the file?
                            // Generally allowing it is better UX.
                            app.mode = app::AppMode::FileSelection;
                            app.file_selector.refresh();
                            app.status_message = "选择文件".to_string();
                            app.add_log(app::LogLevel::Info, "进入文件选择模式...".to_string());
                        }
                    }
                    KeyCode::Tab => app.next_tab(),
                    KeyCode::Char('1') => app.tab = app::Tab::Devices,
                    KeyCode::Char('2') => app.tab = app::Tab::Transfer,
                    KeyCode::Char('3') => app.tab = app::Tab::Log,
                    KeyCode::Char('d') => {
                        app.toggle_log_level();
                    }
                    KeyCode::Char('c') => {
                        app.clear_logs();
                    }
                    _ => {}
                },
            }
        }

        // Update app state (handle async events)
        app.tick();
    }
}
