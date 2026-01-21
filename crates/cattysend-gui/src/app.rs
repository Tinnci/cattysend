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
            // 头部 (Bento Row 1)
            div { class: "bento-tile header-tile",
                Header { status: status.read().clone() }
            }

            // 模式选择（首页显示）
            if *mode.read() == AppMode::Home {
                div { class: "mode-tile",
                    ModeSelector {
                        current_mode: mode.read().clone(),
                        on_change: on_mode_change,
                    }
                }
            }

            // 主内容区 (Bento Row 2)
            match *mode.read() {
                AppMode::Home | AppMode::Sending => rsx! {
                    // 设备列表 (Left Box)
                    div { class: "bento-tile main-left",
                        DeviceList {
                            devices: devices.read().clone(),
                            selected: selected_device.read().clone(),
                            on_select: on_device_select,
                            on_refresh: on_refresh_devices,
                            is_scanning: matches!(*status.read(), TransferStatus::Scanning),
                        }
                    }

                    // 传输面板 (Right Box)
                    div { class: "bento-tile main-right",
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
                    div { class: "bento-tile", style: "grid-column: span 12; display: flex; flex-direction: column; min-height: 500px;",
                        div { class: "card-header",
                            h2 { "📥 接收模式" }
                            button {
                                class: "btn btn-secondary",
                                onclick: on_stop_receive,
                                "停止接收"
                            }
                        }

                        // 状态显示
                        div { style: "padding: 32px; text-align: center; background: white; border: 3px solid black; margin-bottom: 24px;",
                            match receive_state.read().clone() {
                                ReceiveState::Idle | ReceiveState::Starting => rsx! {
                                    div { style: "font-size: 48px; margin-bottom: 16px;", "⏳" }
                                    p { style: "font-weight: 800; font-size: 20px;", "正在启动系统..." }
                                },
                                ReceiveState::Advertising { device_name } => rsx! {
                                    div { style: "font-size: 48px; margin-bottom: 16px;", "📡" }
                                    p { style: "font-weight: 800; font-size: 20px;", "正在广播为: {device_name}" }
                                    p { style: "color: #64748b; font-weight: 600; margin-top: 8px;",
                                        "等待其他设备发送文件..."
                                    }
                                },
                                ReceiveState::Connecting { ssid } => rsx! {
                                    div { style: "font-size: 48px; margin-bottom: 16px;", "📶" }
                                    p { style: "font-weight: 800; font-size: 20px;", "正在建立连接..." }
                                    p { style: "font-weight: 600;", "SSID: {ssid}" }
                                },
                                ReceiveState::Receiving { progress, file_name } => rsx! {
                                    div { style: "font-size: 48px; margin-bottom: 16px;", "📥" }
                                    p { style: "font-weight: 800; font-size: 20px;", "正在接收: {file_name}" }
                                    div { class: "progress-container", style: "margin-top: 24px; width: 100%;",
                                        div {
                                            class: "progress-fill",
                                            style: "width: {progress}%;"
                                        }
                                        div { class: "progress-text", "{progress:.1}%" }
                                    }
                                },
                                ReceiveState::Completed { files } => rsx! {
                                    div { style: "font-size: 48px; margin-bottom: 16px;", "✅" }
                                    p { style: "font-weight: 800; font-size: 24px; color: var(--success);", "传输快如闪电！" }
                                    p { style: "font-weight: 600; margin-top: 8px;",
                                        "共接收 {files.len()} 个项目"
                                    }
                                    div { style: "margin-top: 20px; display: flex; gap: 10px; justify-content: center;",
                                        button { class: "btn btn-primary", "查看文件夹" }
                                    }
                                },
                                ReceiveState::Error(err) => rsx! {
                                    div { style: "font-size: 48px; margin-bottom: 16px;", "❌" }
                                    p { style: "font-weight: 800; font-size: 20px; color: var(--error);", "拦截到异常" }
                                    p { style: "font-weight: 600; margin-top: 8px;", "{err}" }
                                },
                            }
                        }

                        // 控制台日志
                        h3 { style: "font-weight: 900; margin-bottom: 10px; text-transform: uppercase;", "System Monitor" }
                        div { class: "receive-log",
                            for log in receive_logs.read().iter().rev().take(50) {
                                p { "{log}" }
                            }
                        }
                    }
                },

                AppMode::Settings => rsx! {
                    div { class: "bento-tile", style: "grid-column: span 12;",
                        div { class: "card-header",
                            h2 { "⚙️ 系统设置" }
                        }
                        div { style: "display: grid; grid-template-columns: repeat(2, 1fr); gap: 40px; padding: 20px;",
                            div {
                                h3 { "设备身份" }
                                p { "修改您的 Linux 设备在网络中的名称" }
                                // TODO: Input field
                            }
                            div {
                                h3 { "偏好设置" }
                                p { "自动接受下载，开启 5GHz 直连等" }
                            }
                        }
                        button {
                            class: "btn btn-primary",
                            style: "margin-top: 40px;",
                            onclick: move |_| mode.set(AppMode::Home),
                            "保存并返回"
                        }
                    }
                },
            }
        }
    }
}
