//! 传输面板组件

use crate::state::TransferStatus;
use dioxus::prelude::*;
use std::path::PathBuf;

/// 传输面板
#[component]
pub fn TransferPanel(
    status: TransferStatus,
    selected_files: Vec<PathBuf>,
    on_select_files: EventHandler<()>,
    on_send: EventHandler<()>,
    on_cancel: EventHandler<()>,
) -> Element {
    rsx! {
        div {
            h2 { "传输控制" }

            match status {
                TransferStatus::Idle => rsx! {
                    div {
                        class: "dropzone",
                        onclick: move |_| on_select_files.call(()),
                        div { class: "dropzone-icon", "📁" }
                        div { class: "dropzone-text", "点击选择要传输的文件" }
                        div { class: "dropzone-hint", "支持任意格式文件" }
                    }

                    if !selected_files.is_empty() {
                        div { style: "margin-top: 24px;",
                            h3 { style: "font-weight: 800; font-size: 14px; margin-bottom: 12px; text-transform: uppercase;", "待发送项目" }
                            div { style: "display: flex; flex-direction: column; gap: 8px;",
                                for file in selected_files.iter() {
                                    div {
                                        style: "padding: 10px; border: 2px solid black; background: white; font-weight: 700; font-size: 13px;",
                                        "📄 {file.file_name().unwrap_or_default().to_string_lossy()}"
                                    }
                                }
                            }

                            button {
                                class: "btn btn-primary",
                                style: "width: 100%; margin-top: 24px;",
                                onclick: move |_| on_send.call(()),
                                "开始传输"
                            }
                        }
                    }
                },

                TransferStatus::Connecting | TransferStatus::Scanning => rsx! {
                    div { style: "text-align: center; padding: 40px;",
                        div { style: "font-size: 40px; margin-bottom: 20px; animation: pulse 1s infinite;", "📡" }
                        p { style: "font-weight: 800;", "正在建立握手..." }
                    }
                },

                TransferStatus::Transferring { current, total, file_name } => {
                    let progress = if total > 0 { (current as f32 / total as f32) * 100.0 } else { 0.0 };
                    rsx! {
                        div {
                            h3 { style: "font-weight: 800; margin-bottom: 16px;", "正在发送: {file_name}" }
                            div { class: "progress-container",
                                div {
                                    class: "progress-fill",
                                    style: "width: {progress}%;"
                                }
                                div { class: "progress-text", "{progress:.1}%" }
                            }
                        }
                    }
                },

                TransferStatus::Completed { .. } => rsx! {
                    div { style: "text-align: center; padding: 40px;",
                        div { style: "font-size: 48px; margin-bottom: 16px;", "📦" }
                        p { style: "font-weight: 800; color: var(--success);", "任务成功交付！" }
                        button {
                            class: "btn btn-secondary",
                            style: "margin-top: 24px;",
                            onclick: move |_| on_cancel.call(()),
                            "返回"
                        }
                    }
                },

                TransferStatus::Error(e) => rsx! {
                    div { style: "text-align: center; padding: 40px; border: 3px solid var(--error); background: #FFF1F2;",
                        h3 { style: "color: var(--error); font-weight: 900;", "传输中断" }
                        p { style: "margin-top: 10px; font-weight: 600;", "{e}" }
                        button {
                            class: "btn btn-primary",
                            style: "margin-top: 24px;",
                            onclick: move |_| on_cancel.call(()),
                            "重试"
                        }
                    }
                },
            }
        }
    }
}
