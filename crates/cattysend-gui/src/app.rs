//! 主应用组件

use async_trait::async_trait;
use dioxus::prelude::*;
use futures_util::StreamExt;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc;

use crate::components::{DeviceList, Header, ModeSelector, TransferPanel};
use crate::state::{AppMode, DiscoveredDeviceInfo, TransferStatus};
use crate::styles::GLOBAL_CSS;

use cattysend_core::{
    AppSettings, BleScanner, DiscoveredDevice, ReceiveEvent, ReceiveOptions, Receiver,
    ScanCallback, SendEvent, SendOptions, Sender, SimpleReceiveCallback, SimpleSendCallback,
};

/// 异步事件，用于从后台任务更新 UI
#[derive(Debug, Clone)]
enum GuiEvent {
    DeviceFound(DiscoveredDevice),
    ScanFinished,
    TransferStatusUpdate(TransferStatus),
    ReceiveStatusUpdate(ReceiveState),
    Log(LogLevel, String),
    Error(String),
}

/// 接收状态
#[derive(Debug, Clone, PartialEq)]
pub enum ReceiveState {
    Idle,
    #[expect(dead_code, reason = "接收流程中间状态，保留用于未来状态机完善")]
    Starting,
    Advertising {
        device_name: String,
    },
    #[expect(dead_code, reason = "Wi-Fi连接中间状态，保留用于未来连接状态显示")]
    Connecting {
        ssid: String,
    },
    Receiving {
        progress: f32,
        file_name: String,
    },
    Completed {
        files: Vec<PathBuf>,
    },
    Error(String),
}

/// 日志级别
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
enum LogLevel {
    Error = 0,
    Warn = 1,
    Info = 2,
    #[expect(dead_code, reason = "保留用于未来调试级别日志")]
    Debug = 3,
}

/// 日志条目
#[derive(Debug, Clone, PartialEq)]
struct LogEntry {
    level: LogLevel,
    message: String,
}

impl LogLevel {
    fn icon(&self) -> &'static str {
        match self {
            LogLevel::Error => "❌",
            LogLevel::Warn => "⚠️",
            LogLevel::Info => "ℹ️",
            LogLevel::Debug => "🔍",
        }
    }
}

/// 主应用
#[component]
pub fn App() -> Element {
    // === 核心状态 ===
    let mut mode = use_signal(|| AppMode::Home);
    let mut status = use_signal(|| TransferStatus::Idle);
    let mut devices = use_signal(Vec::<DiscoveredDeviceInfo>::new);
    let mut selected_device = use_signal(|| Option::<String>::None);
    let mut selected_files = use_signal(Vec::<PathBuf>::new);
    let settings = use_signal(AppSettings::load);

    // === 接收 & 日志状态 ===
    let mut receive_state = use_signal(|| ReceiveState::Idle);
    let mut logs = use_signal(Vec::<LogEntry>::new);
    let log_filter = use_signal(|| LogLevel::Info);

    // === 任务管理 ===
    let mut active_receive_task = use_signal(|| Option::<dioxus::prelude::Task>::None);

    // === 权限检查 ===
    let permissions = use_signal(|| {
        let (has_nmcli, has_net_raw) = cattysend_core::wifi::check_capabilities();
        (has_nmcli, has_net_raw)
    });

    // === 事件处理循环 (协程) ===
    let event_handler = use_coroutine(move |mut rx: UnboundedReceiver<GuiEvent>| async move {
        while let Some(event) = rx.next().await {
            match event {
                GuiEvent::DeviceFound(device) => {
                    devices.with_mut(|devs| {
                        if !devs.iter().any(|d| d.address == device.address) {
                            devs.push(DiscoveredDeviceInfo {
                                name: device.name.clone(),
                                address: device.address.clone(),
                                rssi: device.rssi.unwrap_or(-100),
                                brand: device.brand_id.map(|b| b.to_string()),
                            });
                        }
                    });
                }
                GuiEvent::ScanFinished => {
                    status.set(TransferStatus::Idle);
                }
                GuiEvent::TransferStatusUpdate(s) => {
                    status.set(s);
                }
                GuiEvent::ReceiveStatusUpdate(s) => {
                    receive_state.set(s);
                }
                GuiEvent::Log(level, msg) => {
                    logs.with_mut(|l| {
                        l.push(LogEntry {
                            level,
                            message: msg,
                        });
                        if l.len() > 100 {
                            l.remove(0);
                        }
                    });
                }
                GuiEvent::Error(msg) => {
                    status.set(TransferStatus::Error(msg.clone()));
                    logs.with_mut(|l| {
                        l.push(LogEntry {
                            level: LogLevel::Error,
                            message: msg,
                        })
                    });
                }
            }
        }
    });

    // 初始化日志
    use_effect(move || {
        event_handler.send(GuiEvent::Log(
            LogLevel::Info,
            "Cattysend GUI 已启动".to_string(),
        ));
    });

    // === 扫描逻辑 ===
    let on_refresh_devices = move |_| {
        devices.set(vec![]);
        status.set(TransferStatus::Scanning);

        let tx_coroutine = event_handler;
        spawn(async move {
            let (tx_mpsc, mut rx_mpsc) = mpsc::unbounded_channel();

            struct GuiScanCallback(mpsc::UnboundedSender<GuiEvent>);
            #[async_trait]
            impl ScanCallback for GuiScanCallback {
                async fn on_device_found(&self, device: DiscoveredDevice) {
                    let _ = self.0.send(GuiEvent::DeviceFound(device));
                }
            }

            let tx_fwd = tx_coroutine;
            spawn(async move {
                while let Some(ev) = rx_mpsc.recv().await {
                    tx_fwd.send(ev);
                }
            });

            match BleScanner::new().await {
                Ok(scanner) => {
                    let _ = scanner
                        .scan(
                            Duration::from_secs(10),
                            Some(Arc::new(GuiScanCallback(tx_mpsc))),
                        )
                        .await;
                    tx_coroutine.send(GuiEvent::ScanFinished);
                }
                Err(e) => tx_coroutine.send(GuiEvent::Error(format!("扫描失败: {}", e))),
            }
        });
    };

    // === 文件选择逻辑 ===
    let on_select_files = move |_| {
        spawn(async move {
            if let Some(files) = rfd::AsyncFileDialog::new()
                .set_title("选择文件")
                .pick_files()
                .await
            {
                let paths: Vec<PathBuf> = files.iter().map(|f| f.path().to_path_buf()).collect();
                selected_files.set(paths);
            }
        });
    };

    // === 发送逻辑 ===
    let on_send = move |_| {
        if let (Some(addr), false) = (
            selected_device.read().clone(),
            selected_files.read().is_empty(),
        ) {
            let files = selected_files.read().clone();
            let tx = event_handler;
            let current_settings = settings.read().clone();
            let device_info = devices.read().iter().find(|d| d.address == *addr).cloned();

            if let Some(dev) = device_info {
                status.set(TransferStatus::Connecting);
                spawn(async move {
                    let options = SendOptions {
                        wifi_interface: "wlan0".to_string(),
                        use_5ghz: current_settings.supports_5ghz,
                        sender_name: current_settings.device_name.clone(),
                    };

                    let (callback, mut rx) = SimpleSendCallback::new();
                    let tx_ev = tx;
                    let files_for_events = files.clone();

                    spawn(async move {
                        while let Some(event) = rx.recv().await {
                            match event {
                                SendEvent::Status(s) => {
                                    tx_ev.send(GuiEvent::Log(LogLevel::Info, s))
                                }
                                SendEvent::Progress { sent, total, .. } => {
                                    tx_ev.send(GuiEvent::TransferStatusUpdate(
                                        TransferStatus::Transferring {
                                            current: sent,
                                            total,
                                            file_name: files_for_events
                                                .first()
                                                .map(|p| {
                                                    p.file_name()
                                                        .unwrap_or_default()
                                                        .to_string_lossy()
                                                        .into_owned()
                                                })
                                                .unwrap_or_default(),
                                        },
                                    ));
                                }
                                SendEvent::Complete => {
                                    tx_ev.send(GuiEvent::TransferStatusUpdate(
                                        TransferStatus::Completed {
                                            files: files_for_events.clone(),
                                        },
                                    ));
                                }
                                SendEvent::Error(e) => tx_ev.send(GuiEvent::Error(e)),
                            }
                        }
                    });

                    let target = DiscoveredDevice {
                        address: dev.address,
                        name: dev.name,
                        rssi: Some(dev.rssi),
                        brand_id: dev.brand.and_then(|b| b.parse().ok()),
                        sender_id: String::new(),
                        supports_5ghz: false,
                    };

                    if let Ok(sender) = Sender::new(options) {
                        let _ = sender.send_to_device(&target, files, &callback).await;
                    }
                });
            }
        }
    };

    // === 接收逻辑 ===
    let mut on_mode_change = move |new_mode: AppMode| {
        // 如果切换到接收模式
        if new_mode == AppMode::Receiving {
            // 检查是否已经在接收模式（防止重复启动）
            if *mode.read() == AppMode::Receiving {
                event_handler.send(GuiEvent::Log(
                    LogLevel::Warn,
                    "已在接收模式中，忽略重复请求".to_string(),
                ));
                return;
            }

            // 清除之前的任务引用（Task drop时会取消）
            active_receive_task.set(None);

            mode.set(AppMode::Receiving);

            let tx = event_handler;
            let current_settings = settings.read().clone();

            event_handler.send(GuiEvent::Log(
                LogLevel::Info,
                format!(
                    "正在启动接收模式，设备名: '{}'",
                    current_settings.device_name
                ),
            ));

            // 启动新的接收任务
            let handle = spawn(async move {
                let options = ReceiveOptions {
                    device_name: current_settings.device_name.clone(),
                    brand_id: current_settings.brand_id,
                    supports_5ghz: current_settings.supports_5ghz,
                    ..Default::default()
                };

                match Receiver::new(options) {
                    Ok(receiver) => {
                        let (callback, mut rx) = SimpleReceiveCallback::new(true);

                        tx.send(GuiEvent::ReceiveStatusUpdate(ReceiveState::Advertising {
                            device_name: current_settings.device_name.clone(),
                        }));

                        tx.send(GuiEvent::Log(
                            LogLevel::Info,
                            "GATT Server 已启动，等待连接...".to_string(),
                        ));

                        let tx_ev = tx;
                        spawn(async move {
                            while let Some(event) = rx.recv().await {
                                match event {
                                    ReceiveEvent::Status(s) => {
                                        tx_ev.send(GuiEvent::Log(LogLevel::Info, s))
                                    }
                                    ReceiveEvent::Progress { received, total } => {
                                        tx_ev.send(GuiEvent::ReceiveStatusUpdate(
                                            ReceiveState::Receiving {
                                                progress: if total > 0 {
                                                    (received as f32 / total as f32) * 100.0
                                                } else {
                                                    0.0
                                                },
                                                file_name: "正在接收...".to_string(),
                                            },
                                        ));
                                    }
                                    ReceiveEvent::Complete(files) => {
                                        tx_ev.send(GuiEvent::ReceiveStatusUpdate(
                                            ReceiveState::Completed { files },
                                        ));
                                    }
                                    ReceiveEvent::Error(e) => tx_ev.send(
                                        GuiEvent::ReceiveStatusUpdate(ReceiveState::Error(e)),
                                    ),
                                    _ => {}
                                }
                            }
                        });

                        let _ = receiver.start(&callback).await;
                    }
                    Err(e) => {
                        tx.send(GuiEvent::Error(format!("无法启动接收器: {}", e)));
                        tx.send(GuiEvent::ReceiveStatusUpdate(ReceiveState::Error(format!(
                            "初始化失败: {}",
                            e
                        ))));
                    }
                }
            });

            // 保存任务句柄
            active_receive_task.set(Some(handle));
        } else {
            // 切换到其他模式时，清除任务引用（Task drop时会取消）
            active_receive_task.set(None);
            receive_state.set(ReceiveState::Idle);
            event_handler.send(GuiEvent::Log(LogLevel::Info, "已停止接收模式".to_string()));
            mode.set(new_mode);
        }
    };

    let filtered_logs = use_memo(move || {
        let filter = *log_filter.read();
        logs.read()
            .iter()
            .filter(|e| e.level <= filter)
            .cloned()
            .collect::<Vec<LogEntry>>()
    });

    rsx! {
        style { "{GLOBAL_CSS}" }
        div { class: "app-container",
            div { class: "bento-tile header-tile", Header { status: status.read().clone() } }
            if *mode.read() == AppMode::Home {
                div { class: "mode-tile", ModeSelector { current_mode: mode.read().clone(), on_change: on_mode_change } }
            }
            match *mode.read() {
                AppMode::Home | AppMode::Sending => rsx! {
                    div { class: "bento-tile main-left",
                        DeviceList {
                            devices: devices.read().clone(),
                            selected: selected_device.read().clone(),
                            on_select: move |a| selected_device.set(Some(a)),
                            on_refresh: on_refresh_devices,
                            is_scanning: matches!(*status.read(), TransferStatus::Scanning),
                        }
                    }
                    div { class: "bento-tile main-right",
                        TransferPanel {
                            status: status.read().clone(),
                            selected_files: selected_files.read().clone(),
                            on_select_files: on_select_files,
                            on_send: on_send,
                            on_cancel: move |_| status.set(TransferStatus::Idle),
                        }
                    }
                },
                AppMode::Receiving => rsx! {
                    div { class: "bento-tile", style: "grid-column: span 12; display: flex; flex-direction: column; min-height: 500px;",
                        div { class: "card-header", h2 { "📥 接收模式" } button { class: "btn btn-secondary", onclick: move |_| on_mode_change(AppMode::Home), "停止" } }
                        div {
                            style: "padding: 32px; text-align: center; background: white; border: 3px solid black; margin-bottom: 24px;",
                            match receive_state.read().clone() {
                                ReceiveState::Idle | ReceiveState::Starting => rsx! { p { "准备中..." } },
                                ReceiveState::Advertising { device_name } => rsx! { p { "广播为: {device_name}" } },
                                ReceiveState::Connecting { ssid } => rsx! { p { "连接中: {ssid}" } },
                                ReceiveState::Receiving { progress, .. } => rsx! { p { "正在接收: {progress:.1}%" } },
                                ReceiveState::Completed { files } => rsx! { p { "完成: {files.len()} 个文件" } },
                                ReceiveState::Error(e) => rsx! { p { "错误: {e}" } },
                            }
                        }
                        div { class: "receive-log", for log in filtered_logs.read().iter().rev().take(10) { p { "{log.level.icon()} {log.message}" } } }
                    }
                },
                AppMode::Settings => {
                    let s = settings.read().clone();
                    let p = *permissions.read();
                    let supports_5g = if s.supports_5ghz { "开启" } else { "关闭" };
                    let nmcli_status = if p.0 { "✅ NM 就绪" } else { "❌ NM 缺失" };
                    let net_raw_status = if p.1 { "✅ RAW 正常" } else { "❌ 权限不足" };

                    rsx! {
                        div { class: "bento-tile", style: "grid-column: span 12;",
                            h2 { "⚙️ 设置" }
                            p { "设备: {s.device_name}" }
                            p { "5GHz: {supports_5g}" }
                            p { "{nmcli_status}" }
                            p { "{net_raw_status}" }
                            button { class: "btn btn-primary", onclick: move |_| mode.set(AppMode::Home), "返回" }
                        }
                    }
                },
            }
        }
    }
}
