//! BLE 模块
//!
//! 包含:
//! - UUID 常量定义
//! - BLE 扫描器 (发现接收端设备)
//! - BLE 客户端 (连接接收端并交换 P2P 信息)
//! - GATT 服务器 (作为接收端等待连接)
//! - 广播器 (发布接收端广播)

pub mod advertiser;
pub mod client;
pub mod gatt;
pub mod scanner;
pub mod server;

use uuid::Uuid;

/// 广播 Service UUID (用于设备发现)
pub const SERVICE_UUID: Uuid = Uuid::from_u128(0x00003331_0000_1000_8000_008123456789);

/// GATT Main Service UUID
pub const MAIN_SERVICE_UUID: Uuid = Uuid::from_u128(0x00009955_0000_1000_8000_00805f9b34fb);

/// STATUS 特征 UUID (读取 DeviceInfo)
pub const STATUS_CHAR_UUID: Uuid = Uuid::from_u128(0x00009954_0000_1000_8000_00805f9b34fb);

/// P2P 特征 UUID (写入 P2pInfo)
pub const P2P_CHAR_UUID: Uuid = Uuid::from_u128(0x00009953_0000_1000_8000_00805f9b34fb);

/// DeviceInfo - 必须与 CatShare 的 DeviceInfo 字段完全一致
/// CatShare: data class DeviceInfo(val state: Int, val key: String?, val mac: String, val catShare: Int? = null)
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    pub state: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    pub mac: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cat_share: Option<i32>,
}

impl DeviceInfo {
    pub fn new(public_key: String, mac: String) -> Self {
        Self {
            state: 0,
            key: Some(public_key),
            mac,
            cat_share: Some(1), // Protocol version
        }
    }
}

// Re-exports
pub use client::BleClient;
pub use scanner::{BleScanner, DiscoveredDevice};
pub use server::{GattServer, GattServerHandle, P2pReceiveEvent};
