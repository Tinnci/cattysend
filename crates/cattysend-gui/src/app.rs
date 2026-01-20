//! 主应用组件

use dioxus::prelude::*;
use std::path::PathBuf;

use crate::components::{DeviceList, Header, ModeSelector, TransferPanel};
use crate::state::{AppMode, AppSettings, DiscoveredDeviceInfo, TransferStatus};
use crate::styles::GLOBAL_CSS;

/// 主应用
#[component]
pub fn App() -> Element {
    // 应用状态
    let mut mode = use_signal(|| AppMode::Home);
    let mut status = use_signal(|| TransferStatus::Idle);
    let mut devices = use_signal(Vec::<DiscoveredDeviceInfo>::new);
    let mut selected_device = use_signal(|| Option::<String>::None);
    let mut selected_files = use_signal(Vec::<PathBuf>::new);
    let settings = use_signal(AppSettings::default);

    // 事件处理器
    let on_mode_change = move |new_mode: AppMode| {
        mode.set(new_mode);
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
                    div { class: "card", style: "flex: 1;",
                        div { class: "card-header",
                            h2 { class: "card-title", "📥 接收模式" }
                            button {
                                class: "btn btn-secondary",
                                onclick: move |_| mode.set(AppMode::Home),
                                "返回"
                            }
                        }

                        div { class: "empty-state",
                            div { class: "empty-state-icon", "📡" }
                            p { class: "empty-state-text",
                                "正在广播为 \"{settings.read().device_name}\"..."
                            }
                            p { style: "color: #64748b; font-size: 12px; margin-top: 8px;",
                                "等待其他设备发送文件"
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
