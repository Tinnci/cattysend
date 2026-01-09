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
            let rssi_bar = rssi_to_bar(dev.rssi.unwrap_or(-100)); // Default to weak signal
            let brand = get_brand_name(dev.brand_id.unwrap_or(0));
            let wifi_5g = if dev.supports_5ghz { "⚡5G" } else { "" };
            let content = format!(
                "{} ({}) {} {} [{}]",
                dev.name, dev.sender_id, rssi_bar, wifi_5g, brand
            );
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
    let logs = app.filtered_logs();
    // 将日志合并为多行文本，最近的在下面（或者最近的在上面，取决于习惯，这里保持最近在最前）
    let log_text: Vec<Line> = logs
        .iter()
        .rev()
        .take(100) // 增加可显示的日志数
        .map(|log| Line::from(log.as_str()))
        .collect();

    let title = format!(" 📋 日志 [{}] - [d]级别 [c]清空 ", app.log_filter.name());

    let paragraph = Paragraph::new(log_text)
        .block(Block::default().borders(Borders::ALL).title(title))
        .wrap(Wrap { trim: true }); // 开启自动换行

    frame.render_widget(paragraph, area);
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

fn get_brand_name(id: u16) -> String {
    match id {
        1 => "Xiaomi",
        2 => "Vivo",
        3 => "Oppo",
        4 => "Huawei",
        5 => "Samsung",
        6 => "Honor",
        7 => "Lenovo",
        8 => "Meizu",
        9 => "ZTE",
        10 => "Nubia",
        11 => "OnePlus",
        12 => "Hisense",
        13 => "Asus",
        14 => "Coolpad",
        15 => "BlackShark",
        16 => "Redmi",
        17 => "Realme",
        18 => "IQOO",
        _ => "Unknown",
    }
    .to_string()
}
