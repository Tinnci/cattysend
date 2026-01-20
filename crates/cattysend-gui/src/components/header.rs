//! 头部组件

use crate::state::TransferStatus;
use dioxus::prelude::*;

#[derive(Props, Clone, PartialEq)]
pub struct HeaderProps {
    pub status: TransferStatus,
}

/// 应用头部
#[component]
pub fn Header(props: HeaderProps) -> Element {
    let status_class = match &props.status {
        TransferStatus::Idle => "",
        TransferStatus::Scanning => "scanning",
        TransferStatus::Error(_) => "error",
        _ => "",
    };

    let status_text = match &props.status {
        TransferStatus::Idle => "就绪",
        TransferStatus::Scanning => "扫描中...",
        TransferStatus::Connecting => "连接中...",
        TransferStatus::Transferring { .. } => "传输中...",
        TransferStatus::Completed { .. } => "已完成",
        TransferStatus::Error(e) => e.as_str(),
    };

    rsx! {
        header { class: "header",
            div { class: "logo",
                span { class: "logo-icon", "🐱" }
                h1 { "Cattysend" }
            }

            div { class: "status-badge {status_class}",
                span { class: "status-dot" }
                span { "{status_text}" }
            }
        }
    }
}
