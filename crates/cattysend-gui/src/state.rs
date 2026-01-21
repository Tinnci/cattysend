//! 应用状态管理
//!
//! 使用 Dioxus signals 管理应用状态

use std::path::PathBuf;

/// 应用模式
#[derive(Debug, Clone, PartialEq, Default)]
pub enum AppMode {
    #[default]
    Home,
    #[expect(dead_code, reason = "发送模式功能规划中，当前合并到 Home 模式")]
    Sending,
    Receiving,
    Settings,
}

/// 发现的设备
#[derive(Debug, Clone, PartialEq)]
pub struct DiscoveredDeviceInfo {
    pub name: String,
    pub address: String,
    pub rssi: i16,
    pub brand: Option<String>,
    pub sender_id: String,
    pub supports_5ghz: bool,
}

/// 传输状态
#[derive(Debug, Clone, PartialEq, Default)]
pub enum TransferStatus {
    #[default]
    Idle,
    Scanning,
    Connecting,
    Transferring {
        current: u64,
        total: u64,
        file_name: String,
    },
    Completed {
        files: Vec<PathBuf>,
    },
    Error(String),
}

impl TransferStatus {
    /// 获取进度百分比
    #[expect(dead_code, reason = "reserved for UI progress bar display")]
    pub fn progress_percent(&self) -> Option<f32> {
        match self {
            TransferStatus::Transferring { current, total, .. } => {
                if *total > 0 {
                    Some((*current as f32 / *total as f32) * 100.0)
                } else {
                    Some(0.0)
                }
            }
            _ => None,
        }
    }

    /// 是否正在进行中
    pub fn is_busy(&self) -> bool {
        matches!(
            self,
            TransferStatus::Scanning
                | TransferStatus::Connecting
                | TransferStatus::Transferring { .. }
        )
    }
}
