//! 头部组件

use crate::state::TransferStatus;
use dioxus::prelude::*;

/// 应用头部
#[component]
pub fn Header(status: TransferStatus) -> Element {
    let status_class = match status {
        TransferStatus::Scanning => "status-badge scanning",
        TransferStatus::Error(_) => "status-badge error",
        _ => "status-badge",
    };

    let status_text = match status {
        TransferStatus::Idle => "系统就绪",
        TransferStatus::Scanning => "正在探测周边设备...",
        TransferStatus::Connecting => "建立安全通道...",
        TransferStatus::Transferring { .. } => "数据传输中",
        TransferStatus::Completed { .. } => "传输已完成",
        TransferStatus::Error(_) => "系统异常",
    };

    rsx! {
        div { class: "logo",
            h1 { "CATTYSEND 2026" }
        }

        div { class: "{status_class}",
            if matches!(status, TransferStatus::Scanning) {
                span { style: "display: inline-block; width: 10; height: 10; background: black; margin-right: 8px;", "■" }
            }
            "{status_text}"
        }
    }
}
