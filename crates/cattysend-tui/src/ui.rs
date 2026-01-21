//! UI rendering module

use ratatui::{
    prelude::*,
    widgets::{Block, Borders, Gauge, List, ListItem, ListState, Paragraph, Tabs, Wrap},
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

    if app.show_perm_warning {
        draw_popup(frame, app);
    }
}

fn draw_popup(frame: &mut Frame, _app: &App) {
    let area = centered_rect(70, 50, frame.area());
    let block = Block::default()
        .title(" 📡 网络配置提示 ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::LightCyan))
        .bg(Color::Black);

    let text = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("💡 提示: ", Style::default().fg(Color::Cyan).bold()),
            Span::raw("本项目已切换至更优雅的 NetworkManager 方案。"),
        ]),
        Line::from(""),
        Line::from("双连接 (Concurrent Mode) 特性现在依赖于系统中的 NetworkManager。"),
        Line::from(""),
        Line::from(vec![Span::styled(
            "✅ 优势: ",
            Style::default().fg(Color::Green).bold(),
        )]),
        Line::from("  • 无需 root/sudo 权限"),
        Line::from("  • 自动管理多网卡并发连接"),
        Line::from("  • 连接更稳健，断开自动恢复"),
        Line::from(""),
        Line::from(vec![
            Span::styled("⚠️ 注意: ", Style::default().fg(Color::Yellow).bold()),
            Span::raw("如果连接失败，请确保已安装 nmcli 并运行 NetworkManager 服务。"),
        ]),
        Line::from(""),
        Line::from(Span::styled(
            " [ 按任意键关闭此提示并继续 ] ",
            Style::default().fg(Color::Gray).italic(),
        )),
    ];

    let paragraph = Paragraph::new(text)
        .block(block)
        .alignment(Alignment::Left)
        .wrap(Wrap { trim: true });

    frame.render_widget(ratatui::widgets::Clear, area); // 这是一个弹窗，需要清除背景
    frame.render_widget(paragraph, area);
}

fn centered_rect(percent_x: u16, percent_y: u16, r: Rect) -> Rect {
    let popup_layout = Layout::default()
        .direction(Direction::Vertical)
        .constraints([
            Constraint::Percentage((100 - percent_y) / 2),
            Constraint::Percentage(percent_y),
            Constraint::Percentage((100 - percent_y) / 2),
        ])
        .split(r);

    Layout::default()
        .direction(Direction::Horizontal)
        .constraints([
            Constraint::Percentage((100 - percent_x) / 2),
            Constraint::Percentage(percent_x),
            Constraint::Percentage((100 - percent_x) / 2),
        ])
        .split(popup_layout[1])[1]
}

fn draw_header(frame: &mut Frame, app: &App, area: Rect) {
    let titles = vec!["设备 [1]", "传输 [2]", "日志 [3]"];
    let selected = match app.tab {
        Tab::Devices => 0,
        Tab::Transfer => 1,
        Tab::Log => 2,
    };

    // 分别显示 NM 和 BLE 权限状态
    let nm_status = if app.has_nmcli {
        Span::styled(" NM:✓ ", Style::default().fg(Color::Green))
    } else {
        Span::styled(" NM:✗ ", Style::default().fg(Color::Red))
    };
    let ble_status = if app.has_net_raw {
        Span::styled("BLE:✓ ", Style::default().fg(Color::Green))
    } else {
        Span::styled("BLE:⚠ ", Style::default().fg(Color::Yellow))
    };

    let tabs = Tabs::new(titles)
        .block(
            Block::default()
                .borders(Borders::ALL)
                .title(Line::from(vec![
                    Span::raw(" Cattysend TUI "),
                    nm_status,
                    ble_status,
                ])),
        )
        .select(selected)
        .style(Style::default().fg(Color::White))
        .highlight_style(Style::default().fg(Color::Yellow).bold());

    frame.render_widget(tabs, area);
}

fn draw_main(frame: &mut Frame, app: &App, area: Rect) {
    if app.mode == AppMode::Settings {
        draw_settings(frame, app, area);
        return;
    }

    if app.mode == AppMode::FileSelection {
        draw_file_selection(frame, app, area);
        return;
    }

    match app.tab {
        Tab::Devices => draw_devices_tab(frame, app, area),
        Tab::Transfer => draw_transfer_tab(frame, app, area),
        Tab::Log => draw_log_tab(frame, app, area),
    }
}

fn draw_settings(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(" ⚙️ 设置 ")
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Yellow));

    let inner_area = centered_rect(60, 30, area);

    let content = vec![
        Line::from(""),
        Line::from(vec![
            Span::styled("修改设备名称: ", Style::default().bold()),
            Span::styled(
                &app.input_buffer,
                Style::default().fg(Color::Cyan).bg(Color::DarkGray),
            ),
            Span::styled("_", Style::default().fg(Color::White).bold()), // 光标模拟
        ]),
        Line::from(""),
        Line::from(vec![
            Span::raw("当前保存值: "),
            Span::styled(&app.settings.device_name, Style::default().fg(Color::Gray)),
        ]),
        Line::from(""),
        Line::from(""),
        Line::from(vec![
            Span::styled(" [Enter] ", Style::default().fg(Color::Green).bold()),
            Span::raw("保存并返回   "),
            Span::styled(" [Esc] ", Style::default().fg(Color::Red).bold()),
            Span::raw("取消"),
        ]),
    ];

    let paragraph = Paragraph::new(content)
        .block(block)
        .alignment(Alignment::Center)
        .wrap(Wrap { trim: true });

    frame.render_widget(paragraph, inner_area);
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
            let brand = &dev.brand;
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
        AppMode::Transferring => format!("正在传输... {}", app.status_message),
        AppMode::Sending => format!("发送模式: {}", app.status_message),
        AppMode::Receiving => format!("接收模式: {}", app.status_message),
        _ => "无活动传输".to_string(),
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
        AppMode::Settings => " ⚙️ 设置中 ",
        AppMode::FileSelection => " 📂 选择文件 ",
    };

    let status = Paragraph::new(format!(
        "{}│ {} │ 设备: {} │ [s]扫描 [r]接收 [p]设置 [Tab]切换 [q]退出",
        mode_text,
        app.status_message,
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

fn draw_file_selection(frame: &mut Frame, app: &App, area: Rect) {
    let block = Block::default()
        .title(format!(
            " 📂 选择文件 - {} ",
            app.file_selector.current_path.to_string_lossy()
        ))
        .borders(Borders::ALL)
        .border_style(Style::default().fg(Color::Cyan));

    let items: Vec<ListItem> = app
        .file_selector
        .entries
        .iter()
        .enumerate()
        .map(|(i, entry)| {
            let icon = if entry.is_dir { "📁" } else { "📄" };
            let style = if i == app.file_selector.selected {
                Style::default()
                    .bg(Color::DarkGray)
                    .fg(Color::White)
                    .add_modifier(Modifier::BOLD)
            } else if entry.is_dir {
                Style::default().fg(Color::Blue)
            } else {
                Style::default()
            };

            ListItem::new(format!("{} {}", icon, entry.name)).style(style)
        })
        .collect();

    let list = List::new(items)
        .block(block)
        .highlight_style(Style::default().add_modifier(Modifier::BOLD));

    // Calculate scroll state to keep selected item in view
    // Since List doesn't manage internal state implicitly without a StatefulWidget,
    // we just rely on the list rendering. To ensure the selected item is visible,
    // we would typically use a ListState. But here we are just redrawing.
    // Ratatui's List will start from top.

    // To properly scroll, we need a ListState or control the offset.
    // For simplicity in this `draw` function (which is stateless), we can't easily auto-scroll
    // without passing a mutable State.
    // However, Ratatui's `List` widget usually works with `Frame::render_stateful_widget`.
    // Since `app` holds the state index, but not a `ListState`, we can construct one temporarily
    // or just render the list roughly centered if we wanted, but sticking to basic List is easiest.
    // Wait, the standard `List` in Ratatui renders all items if they fit, or clips them.
    // Without `render_stateful_widget`, we can't scroll.

    // Let's use `render_stateful_widget` and create a `ListState` on the fly.
    let mut state = ListState::default();
    state.select(Some(app.file_selector.selected));

    frame.render_stateful_widget(list, area, &mut state);
}
