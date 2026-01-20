//! 传输面板组件

use crate::state::TransferStatus;
use dioxus::prelude::*;
use std::path::PathBuf;

#[derive(Props, Clone, PartialEq)]
pub struct TransferPanelProps {
    pub status: TransferStatus,
    pub selected_files: Vec<PathBuf>,
    pub on_select_files: EventHandler<()>,
    pub on_send: EventHandler<()>,
    pub on_cancel: EventHandler<()>,
}

/// 传输面板
#[component]
pub fn TransferPanel(props: TransferPanelProps) -> Element {
    rsx! {
        div { class: "card",
            div { class: "card-header",
                h2 { class: "card-title", "📦 传输" }
            }

            match &props.status {
                TransferStatus::Idle => rsx! {
                    FileDropzone {
                        files: props.selected_files.clone(),
                        on_click: props.on_select_files,
                    }

                    if !props.selected_files.is_empty() {
                        div { style: "margin-top: 16px;",
                            button {
                                class: "btn btn-primary",
                                style: "width: 100%;",
                                onclick: move |_| props.on_send.call(()),
                                "🚀 发送文件"
                            }
                        }
                    }
                },

                TransferStatus::Scanning => rsx! {
                    TransferProgress {
                        title: "扫描设备中...",
                        subtitle: "正在搜索附近的 CatShare 设备",
                        progress: None,
                    }
                },

                TransferStatus::Connecting => rsx! {
                    TransferProgress {
                        title: "连接中...",
                        subtitle: "正在建立 WiFi P2P 连接",
                        progress: None,
                    }
                },

                TransferStatus::Transferring { current, total, file_name } => rsx! {
                    TransferProgress {
                        title: "传输中...",
                        subtitle: file_name.clone(),
                        progress: Some((*current as f32 / *total as f32) * 100.0),
                    }

                    div { style: "margin-top: 16px;",
                        button {
                            class: "btn btn-secondary",
                            style: "width: 100%;",
                            onclick: move |_| props.on_cancel.call(()),
                            "❌ 取消传输"
                        }
                    }
                },

                TransferStatus::Completed { files } => rsx! {
                    div { class: "empty-state",
                        div { class: "empty-state-icon", "✅" }
                        p { class: "empty-state-text",
                            "成功传输 {files.len()} 个文件！"
                        }
                    }
                },

                TransferStatus::Error(err) => rsx! {
                    div { class: "empty-state",
                        div { class: "empty-state-icon", "❌" }
                        p { class: "empty-state-text",
                            "错误: {err}"
                        }
                    }
                },
            }
        }
    }
}

#[derive(Props, Clone, PartialEq)]
struct FileDropzoneProps {
    files: Vec<PathBuf>,
    on_click: EventHandler<()>,
}

#[component]
fn FileDropzone(props: FileDropzoneProps) -> Element {
    rsx! {
        div {
            class: "dropzone",
            onclick: move |_| props.on_click.call(()),

            if props.files.is_empty() {
                div { class: "dropzone-icon", "📁" }
                p { class: "dropzone-text", "点击选择文件" }
                p { class: "dropzone-hint", "或将文件拖放到此处" }
            } else {
                div { class: "dropzone-icon", "📄" }
                p { class: "dropzone-text",
                    "已选择 {props.files.len()} 个文件"
                }
                div { style: "margin-top: 12px;",
                    for file in props.files.iter().take(3) {
                        p {
                            class: "dropzone-hint",
                            "{file.file_name().unwrap_or_default().to_string_lossy()}"
                        }
                    }
                    if props.files.len() > 3 {
                        p { class: "dropzone-hint", "..." }
                    }
                }
            }
        }
    }
}

#[derive(Props, Clone, PartialEq)]
struct TransferProgressProps {
    title: String,
    subtitle: String,
    progress: Option<f32>,
}

#[component]
fn TransferProgress(props: TransferProgressProps) -> Element {
    rsx! {
        div { class: "progress-container",
            div { style: "text-align: center; margin-bottom: 16px;",
                h3 { style: "font-size: 18px; font-weight: 600; color: #f1f5f9;",
                    "{props.title}"
                }
                p { style: "font-size: 14px; color: #94a3b8; margin-top: 4px;",
                    "{props.subtitle}"
                }
            }

            if let Some(pct) = props.progress {
                div { class: "progress-bar",
                    div {
                        class: "progress-fill",
                        style: "width: {pct:.1}%",
                    }
                }
                div { class: "progress-text",
                    span { "{pct:.1}%" }
                }
            } else {
                // 无限进度动画
                div { class: "progress-bar",
                    div {
                        class: "progress-fill",
                        style: "width: 30%; animation: pulse 1.5s ease-in-out infinite;",
                    }
                }
            }
        }
    }
}
