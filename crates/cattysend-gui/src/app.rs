//! 主应用组件

use dioxus::prelude::*;
use std::path::PathBuf;

use crate::components::{DeviceList, Header, ModeSelector, TransferPanel};
use crate::state::{AppMode, DiscoveredDeviceInfo, TransferStatus};
use crate::styles::GLOBAL_CSS;

use cattysend_core::{AppSettings, ReceiveEvent, ReceiveOptions, Receiver, SimpleReceiveCallback};

/// 接收状态
#[derive(Debug, Clone, PartialEq)]
pub enum ReceiveState {
    Idle,
    Starting,
    Advertising { device_name: String },
    Connecting { ssid: String },
    Receiving { progress: f32, file_name: String },
    Completed { files: Vec<PathBuf> },
    Error(String),
}

/// 主应用
#[component]
pub fn App() -> Element {
    // 应用状态
    let mut mode = use_signal(|| AppMode::Home);
    let mut status = use_signal(|| TransferStatus::Idle);
    let mut devices = use_signal(Vec::<DiscoveredDeviceInfo>::new);
    let mut selected_device = use_signal(|| Option::<String>::None);
    let mut selected_files = use_signal(Vec::<PathBuf>::new);
    let settings = use_signal(AppSettings::load);

    // 接收状态
    let mut receive_state = use_signal(|| ReceiveState::Idle);
    let mut receive_logs = use_signal(Vec::<String>::new);

    // 事件处理器
    let on_mode_change = move |new_mode: AppMode| {
        mode.set(new_mode.clone());

        // 当切换到接收模式时启动接收
        if new_mode == AppMode::Receiving {
            let current_settings = settings.read().clone();
            let device_name = current_settings.device_name.clone();

            receive_state.set(ReceiveState::Starting);
            receive_logs.set(vec!["正在启动接收模式...".to_string()]);
            receive_logs.with_mut(|logs| {
                logs.push(format!(
                    "配置已加载: 设备名='{}', 厂商='{}', 5GHz={}",
                    device_name,
                    current_settings.brand_id.name(),
                    current_settings.supports_5ghz
                ));
            });

            spawn(async move {
                let options = ReceiveOptions {
                    device_name: device_name.clone(),
                    brand_id: current_settings.brand_id,
                    supports_5ghz: current_settings.supports_5ghz,
                    ..Default::default()
                };

                match Receiver::new(options) {
                    Ok(receiver) => {
                        let (callback, mut rx) = SimpleReceiveCallback::new(true);

                        receive_state.set(ReceiveState::Advertising {
                            device_name: device_name.clone(),
                        });
                        receive_logs.with_mut(|logs| {
                            logs.push(format!("📡 正在广播为 '{}'", device_name));
                        });

                        // 使用 spawn 来处理事件（Dioxus 的 spawn 不要求 Send）
                        let mut logs_for_events = receive_logs;
                        let mut state_for_events = receive_state;

                        // 在另一个 Dioxus spawn 中处理事件
                        spawn(async move {
                            while let Some(event) = rx.recv().await {
                                match event {
                                    ReceiveEvent::Status(s) => {
                                        logs_for_events.with_mut(|logs| {
                                            logs.push(format!("ℹ️ {}", s));
                                        });
                                        // 检测连接状态并提取 SSID
                                        if (s.contains("连接到 WiFi") || s.contains("Connecting"))
                                            && let Some(ssid) = s
                                                .split("WiFi: ")
                                                .nth(1)
                                                .or(s.split("ssid='").nth(1))
                                        {
                                            let ssid = ssid
                                                .split(['\'', '"', ','])
                                                .next()
                                                .unwrap_or("")
                                                .to_string();
                                            state_for_events.set(ReceiveState::Connecting { ssid });
                                        }
                                    }
                                    ReceiveEvent::Progress { received, total } => {
                                        let progress = if total > 0 {
                                            (received as f32 / total as f32) * 100.0
                                        } else {
                                            0.0
                                        };
                                        state_for_events.set(ReceiveState::Receiving {
                                            progress,
                                            file_name: "文件".to_string(),
                                        });
                                    }
                                    ReceiveEvent::Complete(files) => {
                                        logs_for_events.with_mut(|logs| {
                                            logs.push(format!(
                                                "✅ 接收完成，共 {} 个文件",
                                                files.len()
                                            ));
                                        });
                                        state_for_events.set(ReceiveState::Completed { files });
                                    }
                                    ReceiveEvent::Error(e) => {
                                        logs_for_events.with_mut(|logs| {
                                            logs.push(format!("❌ 错误: {}", e));
                                        });
                                        state_for_events.set(ReceiveState::Error(e));
                                    }
                                    _ => {}
                                }
                            }
                        });

                        // 启动接收
                        if let Err(e) = receiver.start(&callback).await {
                            receive_logs.with_mut(|logs| {
                                logs.push(format!("❌ 接收失败: {}", e));
                            });
                            receive_state.set(ReceiveState::Error(e.to_string()));
                        }
                    }
                    Err(e) => {
                        receive_logs.with_mut(|logs| {
                            logs.push(format!("❌ 初始化失败: {}", e));
                        });
                        receive_state.set(ReceiveState::Error(e.to_string()));
                    }
                }
            });
        } else {
            // 离开接收模式时重置状态
            receive_state.set(ReceiveState::Idle);
        }
    };

    let on_device_select = move |address: String| {
        selected_device.set(Some(address));
    };

    let on_refresh_devices = move |_| {
        // 模拟扫描
        status.set(TransferStatus::Scanning);

        // 在实际实现中，这里会调用 BLE 扫描
        // 这里用模拟数据演示
        spawn(async move {
            tokio::time::sleep(std::time::Duration::from_secs(2)).await;

            devices.set(vec![
                DiscoveredDeviceInfo {
                    name: "Xiaomi 14 Pro".to_string(),
                    address: "AA:BB:CC:DD:EE:01".to_string(),
                    rssi: -45,
                    brand: Some("xiaomi".to_string()),
                },
                DiscoveredDeviceInfo {
                    name: "OPPO Find X7".to_string(),
                    address: "AA:BB:CC:DD:EE:02".to_string(),
                    rssi: -62,
                    brand: Some("oppo".to_string()),
                },
                DiscoveredDeviceInfo {
                    name: "Galaxy S24".to_string(),
                    address: "AA:BB:CC:DD:EE:03".to_string(),
                    rssi: -78,
                    brand: Some("samsung".to_string()),
                },
            ]);

            status.set(TransferStatus::Idle);
        });
    };

    let on_select_files = move |_| {
        // 在实际实现中，这里会调用文件选择对话框
        // 这里用模拟数据演示
        selected_files.set(vec![
            PathBuf::from("/home/user/document.pdf"),
            PathBuf::from("/home/user/photo.jpg"),
        ]);
    };

    let on_send = move |_| {
        if selected_device.read().is_some() && !selected_files.read().is_empty() {
            status.set(TransferStatus::Connecting);

            spawn(async move {
                tokio::time::sleep(std::time::Duration::from_secs(1)).await;

                // 模拟传输进度
                for i in 0..=100 {
                    status.set(TransferStatus::Transferring {
                        current: i * 1024 * 1024,
                        total: 100 * 1024 * 1024,
                        file_name: "document.pdf".to_string(),
                    });
                    tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                }

                status.set(TransferStatus::Completed {
                    files: selected_files.read().clone(),
                });
            });
        }
    };

    let on_cancel = move |_| {
        status.set(TransferStatus::Idle);
    };

    // 接收模式的停止处理
    let on_stop_receive = move |_| {
        mode.set(AppMode::Home);
        receive_state.set(ReceiveState::Idle);
        receive_logs.set(vec![]);
    };

    rsx! {
        style { "{GLOBAL_CSS}" }

        div { class: "app-container",
            // 头部
            Header { status: status.read().clone() }

            // 模式选择（仅在首页显示）
            if *mode.read() == AppMode::Home {
                ModeSelector {
                    current_mode: mode.read().clone(),
                    on_change: on_mode_change,
                }
            }

            // 主内容区
            match *mode.read() {
                AppMode::Home => rsx! {
                    div { class: "main-content",
                        // 设备列表
                        DeviceList {
                            devices: devices.read().clone(),
                            selected: selected_device.read().clone(),
                            on_select: on_device_select,
                            on_refresh: on_refresh_devices,
                            is_scanning: matches!(*status.read(), TransferStatus::Scanning),
                        }

                        // 传输面板
                        TransferPanel {
                            status: status.read().clone(),
                            selected_files: selected_files.read().clone(),
                            on_select_files: on_select_files,
                            on_send: on_send,
                            on_cancel: on_cancel,
                        }
                    }
                },

                AppMode::Sending => rsx! {
                    div { class: "main-content",
                        DeviceList {
                            devices: devices.read().clone(),
                            selected: selected_device.read().clone(),
                            on_select: on_device_select,
                            on_refresh: on_refresh_devices,
                            is_scanning: matches!(*status.read(), TransferStatus::Scanning),
                        }

                        TransferPanel {
                            status: status.read().clone(),
                            selected_files: selected_files.read().clone(),
                            on_select_files: on_select_files,
                            on_send: on_send,
                            on_cancel: on_cancel,
                        }
                    }
                },

                AppMode::Receiving => rsx! {
                    div { class: "card", style: "flex: 1; display: flex; flex-direction: column;",
                        div { class: "card-header",
                            h2 { class: "card-title", "📥 接收模式" }
                            button {
                                class: "btn btn-secondary",
                                onclick: on_stop_receive,
                                "停止接收"
                            }
                        }

                        // 状态显示
                        div { style: "padding: 16px; text-align: center;",
                            match receive_state.read().clone() {
                                ReceiveState::Idle | ReceiveState::Starting => rsx! {
                                    div { class: "empty-state-icon", style: "animation: pulse 2s infinite;", "⏳" }
                                    p { class: "empty-state-text", "正在启动..." }
                                },
                                ReceiveState::Advertising { device_name } => rsx! {
                                    div { class: "empty-state-icon", style: "animation: pulse 2s infinite;", "📡" }
                                    p { class: "empty-state-text", "正在广播为 \"{device_name}\"" }
                                    p { style: "color: #64748b; font-size: 12px; margin-top: 8px;",
                                        "等待其他设备发送文件"
                                    }
                                },
                                ReceiveState::Connecting { ssid } => rsx! {
                                    div { class: "empty-state-icon", style: "animation: pulse 1s infinite;", "📶" }
                                    p { class: "empty-state-text", "正在连接到 WiFi: {ssid}" }
                                },
                                ReceiveState::Receiving { progress, file_name } => rsx! {
                                    div { class: "empty-state-icon", "📥" }
                                    p { class: "empty-state-text", "正在接收: {file_name}" }
                                    div { class: "progress-bar", style: "margin-top: 12px; width: 80%; margin-left: auto; margin-right: auto;",
                                        div {
                                            class: "progress-fill",
                                            style: "width: {progress}%;"
                                        }
                                    }
                                    p { style: "color: #64748b; font-size: 12px; margin-top: 8px;",
                                        "{progress:.1}%"
                                    }
                                },
                                ReceiveState::Completed { files } => rsx! {
                                    div { class: "empty-state-icon", "✅" }
                                    p { class: "empty-state-text", "接收完成！" }
                                    p { style: "color: #64748b; font-size: 12px; margin-top: 8px;",
                                        "共接收 {files.len()} 个文件"
                                    }
                                },
                                ReceiveState::Error(err) => rsx! {
                                    div { class: "empty-state-icon", "❌" }
                                    p { class: "empty-state-text", style: "color: #ef4444;", "发生错误" }
                                    p { style: "color: #64748b; font-size: 12px; margin-top: 8px;",
                                        "{err}"
                                    }
                                },
                            }
                        }

                        // 日志区域
                        div {
                            style: "flex: 1; overflow-y: auto; padding: 16px; background: #0f172a; border-radius: 8px; margin: 16px; font-family: monospace; font-size: 12px;",
                            for log in receive_logs.read().iter().rev().take(50) {
                                p { style: "margin: 4px 0; color: #94a3b8;", "{log}" }
                            }
                        }
                    }
                },

                AppMode::Settings => rsx! {
                    div { class: "card", style: "flex: 1;",
                        div { class: "card-header",
                            h2 { class: "card-title", "⚙️ 设置" }
                        }
                        // TODO: 设置页面内容
                    }
                },
            }
        }
    }
}
