//! UI rendering module

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Gauge, List, ListItem, Paragraph, Tabs, Wrap},
};

use crate::app::{App, AppMode, Tab};

pub fn draw(frame: &mut Frame, app: &App) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(3), // Header
            Constraint::Min(10),   // Main content
            Constraint::Length(3), // Status bar
        ])
        .split(frame.area());

    draw_header(frame, app, chunks[0]);
    draw_main(frame, app, chunks[1]);
    draw_status_bar(frame, app, chunks[2]);
}

fn draw_header(frame: &mut Frame, app: &App, area: Rect) {
    let titles = vec!["设备 [1]", "传输 [2]", "日志 [3]"];
    let selected = match app.tab {
        Tab::Devices => 0,
        Tab::Transfer => 1,
        Tab::Log => 2,
    };

    let tabs = Tabs::new(titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" Cattysend TUI "),
        )
        .select(selected)
        .style(Style::default().fg(Color::White))
        .highlight_style(Style::default().fg(Color::Yellow).bold());

    frame.render_widget(tabs, area);
}

fn draw_main(frame: &mut Frame, app: &App, area: Rect) {
    match app.tab {
        Tab::Devices => draw_devices_tab(frame, app, area),
        Tab::Transfer => draw_transfer_tab(frame, app, area),
        Tab::Log => draw_log_tab(frame, app, area),
    }
}

fn draw_devices_tab(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Horizontal)
        .constraints([Constraint::Percentage(60), Constraint::Percentage(40)])
        .split(area);

    // Device list
    let items: Vec<ListItem> = app
        .devices
        .iter()
        .enumerate()
        .map(|(i, dev)| {
            let rssi_bar = rssi_to_bar(dev.rssi);
            let content = format!("{} {} [{}]", dev.name, rssi_bar, dev.brand);
            let style = if i == app.selected_device {
                Style::default().bg(Color::DarkGray).fg(Color::White)
            } else {
                Style::default()
            };
            ListItem::new(content).style(style)
        })
        .collect();

    let title = match app.mode {
        AppMode::Scanning => " 🔍 扫描中... ",
        _ => " 📱 附近设备 ",
    };

    let list = List::new(items)
        .block(Block::default().borders(Borders::ALL).title(title))
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));

    frame.render_widget(list, chunks[0]);

    // Device details / help
    let help_text = if app.devices.is_empty() {
        "按 's' 开始扫描\n按 'r' 进入接收模式\n按 'q' 退出"
    } else {
        "↑/↓ 选择设备\nEnter 连接\nTab 切换标签\n\n按 's' 重新扫描"
    };

    let help = Paragraph::new(help_text)
        .block(Block::default().borders(Borders::ALL).title(" 帮助 "))
        .wrap(Wrap { trim: true });

    frame.render_widget(help, chunks[1]);
}

fn draw_transfer_tab(frame: &mut Frame, app: &App, area: Rect) {
    let chunks = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Length(5), // Progress
            Constraint::Length(4), // Speed
            Constraint::Min(5),    // File info
        ])
        .split(area);

    // Progress bar
    let progress_percent = (app.progress * 100.0) as u16;
    let gauge = Gauge::default()
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(" 📦 传输进度 "),
        )
        .gauge_style(Style::default().fg(Color::Green).bg(Color::Black))
        .percent(progress_percent)
        .label(format!("{}%", progress_percent));

    frame.render_widget(gauge, chunks[0]);

    // Speed
    let speed_text = if app.mode == AppMode::Transferring {
        format!("⚡ 传输速度: {:.1} MB/s", app.transfer_speed)
    } else {
        "⚡ 传输速度: --".to_string()
    };

    let speed =
        Paragraph::new(speed_text).block(Block::default().borders(Borders::ALL).title(" 速度 "));

    frame.render_widget(speed, chunks[1]);

    // File info
    let file_info = match app.mode {
        AppMode::Transferring => "正在传输: document.pdf (10.5 MB)",
        AppMode::Sending => "准备发送...",
        AppMode::Receiving => "等待接收...",
        _ => "无活动传输",
    };

    let info =
        Paragraph::new(file_info).block(Block::default().borders(Borders::ALL).title(" 文件信息 "));

    frame.render_widget(info, chunks[2]);
}

fn draw_log_tab(frame: &mut Frame, app: &App, area: Rect) {
    let items: Vec<ListItem> = app
        .logs
        .iter()
        .rev()
        .take(20)
        .map(|log| ListItem::new(log.as_str()))
        .collect();

    let list = List::new(items).block(Block::default().borders(Borders::ALL).title(" 📋 日志 "));

    frame.render_widget(list, area);
}

fn draw_status_bar(frame: &mut Frame, app: &App, area: Rect) {
    let mode_text = match app.mode {
        AppMode::Idle => " ⏸️  空闲 ",
        AppMode::Scanning => " 🔍 扫描中 ",
        AppMode::Receiving => " 📥 接收模式 ",
        AppMode::Sending => " 📤 发送中 ",
        AppMode::Transferring => " 🔄 传输中 ",
    };

    let status = Paragraph::new(format!(
        "{}│ 设备: {} │ [s]扫描 [r]接收 [Tab]切换 [q]退出",
        mode_text,
        app.devices.len()
    ))
    .block(Block::default().borders(Borders::ALL));

    frame.render_widget(status, area);
}

fn rssi_to_bar(rssi: i16) -> &'static str {
    if rssi > -50 {
        "████"
    } else if rssi > -60 {
        "███░"
    } else if rssi > -70 {
        "██░░"
    } else if rssi > -80 {
        "█░░░"
    } else {
        "░░░░"
    }
}
