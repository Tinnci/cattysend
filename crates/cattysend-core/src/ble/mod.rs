//! BLE 模块
//!
//! 提供与 CatShare (Android) 兼容的 BLE 功能。
//!
//! # 模块
//!
//! - `scanner`: BLE 扫描器（发现接收端设备）
//! - `client`: BLE 客户端（连接接收端并交换 P2P 信息）
//! - `server`: GATT 服务器（作为接收端等待连接）
//! - `advertiser`: 广播器（发布接收端广播）
//!
//! # UUID 常量
//!
//! 所有 UUID 均与 CatShare 保持一致：
//! - `SERVICE_UUID`: 广播服务 UUID，用于设备发现
//! - `MAIN_SERVICE_UUID`: GATT 主服务 UUID
//! - `STATUS_CHAR_UUID`: 读取 DeviceInfo 的特征
//! - `P2P_CHAR_UUID`: 写入 P2pInfo 的特征

pub mod advertiser;
pub mod client;
pub mod gatt;
pub mod scanner;
pub mod server;

use uuid::Uuid;

/// CatShare/MTA 使用 16-bit UUID: 0x3331
///
/// 注意：广播发现使用自定义基底 008123456789，而不是标准蓝牙基底
pub const ADV_SERVICE_UUID: Uuid = Uuid::from_u128(0x00003331_0000_1000_8000_008123456789);

/// Service UUID (用于扫描时匹配，使用标准蓝牙基底)
///
/// 某些设备可能使用标准蓝牙基底 UUID: 00003331-0000-1000-8000-00805f9b34fb
/// 扫描时应同时检查 ADV_SERVICE_UUID 和 SERVICE_UUID
pub const SERVICE_UUID: Uuid = Uuid::from_u128(0x00003331_0000_1000_8000_00805f9b34fb);

/// GATT Main Service UUID
///
/// CatShare: `00009955-0000-1000-8000-00805f9b34fb`
/// 用于 GATT 服务注册，包含 STATUS 和 P2P 特征
pub const MAIN_SERVICE_UUID: Uuid = Uuid::from_u128(0x00009955_0000_1000_8000_00805f9b34fb);

/// STATUS 特征 UUID (读取 DeviceInfo)
///
/// CatShare: `00009954-0000-1000-8000-00805f9b34fb`
pub const STATUS_CHAR_UUID: Uuid = Uuid::from_u128(0x00009954_0000_1000_8000_00805f9b34fb);

/// P2P 特征 UUID (写入 P2pInfo)
///
/// CatShare: `00009953-0000-1000-8000-00805f9b34fb`
pub const P2P_CHAR_UUID: Uuid = Uuid::from_u128(0x00009953_0000_1000_8000_00805f9b34fb);

/// DeviceInfo - 与 CatShare 的 DeviceInfo 完全兼容
///
/// CatShare Kotlin 定义:
/// ```kotlin
/// data class DeviceInfo(val state: Int, val key: String?, val mac: String, val catShare: Int? = null)
/// ```
///
/// # 字段
///
/// - `state`: 设备状态 (通常为 0)
/// - `key`: Base64 编码的 ECDH 公钥 (SPKI 格式)
/// - `mac`: 设备 MAC 地址
/// - `cat_share`: 协议版本号 (序列化为 `catShare`)
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
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
    /// 创建新的 DeviceInfo
    ///
    /// # 参数
    ///
    /// - `public_key`: Base64 编码的 ECDH 公钥
    /// - `mac`: 设备 MAC 地址
    pub fn new(public_key: String, mac: String) -> Self {
        Self {
            state: 0,
            key: Some(public_key),
            mac,
            cat_share: Some(1),
        }
    }
}

// Re-exports
pub use client::BleClient;
pub use scanner::{BleScanner, DiscoveredDevice, ScanCallback};
pub use server::{GattServer, GattServerHandle, P2pReceiveEvent};

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证 UUID 常量与 CatShare 一致
    #[test]
    fn test_uuid_constants() {
        // 广播服务 UUID (CatShare ADV_SERVICE_UUID)
        assert_eq!(
            ADV_SERVICE_UUID.to_string(),
            "00003331-0000-1000-8000-008123456789"
        );

        // 标准蓝牙基底版本的服务 UUID
        assert_eq!(
            SERVICE_UUID.to_string(),
            "00003331-0000-1000-8000-00805f9b34fb"
        );

        // GATT 主服务 UUID
        assert_eq!(
            MAIN_SERVICE_UUID.to_string(),
            "00009955-0000-1000-8000-00805f9b34fb"
        );

        // STATUS 特征 UUID
        assert_eq!(
            STATUS_CHAR_UUID.to_string(),
            "00009954-0000-1000-8000-00805f9b34fb"
        );

        // P2P 特征 UUID
        assert_eq!(
            P2P_CHAR_UUID.to_string(),
            "00009953-0000-1000-8000-00805f9b34fb"
        );
    }

    /// 验证 DeviceInfo 序列化与 CatShare 兼容
    #[test]
    fn test_device_info_serialization() {
        let info = DeviceInfo::new("BASE64KEY".to_string(), "AA:BB:CC:DD:EE:FF".to_string());

        let json = serde_json::to_string(&info).unwrap();

        // 验证 camelCase 命名
        assert!(json.contains("\"state\":"));
        assert!(json.contains("\"key\":"));
        assert!(json.contains("\"mac\":"));
        assert!(json.contains("\"catShare\":")); // 不是 cat_share

        // 验证值
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["state"], 0);
        assert_eq!(parsed["key"], "BASE64KEY");
        assert_eq!(parsed["mac"], "AA:BB:CC:DD:EE:FF");
        assert_eq!(parsed["catShare"], 1);
    }

    /// 验证 DeviceInfo 反序列化与 CatShare 兼容
    #[test]
    fn test_device_info_deserialization() {
        // 模拟 CatShare 发送的 JSON
        let json = r#"{"state":0,"key":"ABC123","mac":"11:22:33:44:55:66","catShare":2}"#;

        let info: DeviceInfo = serde_json::from_str(json).unwrap();

        assert_eq!(info.state, 0);
        assert_eq!(info.key, Some("ABC123".to_string()));
        assert_eq!(info.mac, "11:22:33:44:55:66");
        assert_eq!(info.cat_share, Some(2));
    }

    /// 验证 DeviceInfo 可选字段处理
    #[test]
    fn test_device_info_optional_fields() {
        // 没有 key 和 catShare
        let json = r#"{"state":1,"mac":"00:00:00:00:00:00"}"#;

        let info: DeviceInfo = serde_json::from_str(json).unwrap();

        assert_eq!(info.state, 1);
        assert_eq!(info.key, None);
        assert_eq!(info.mac, "00:00:00:00:00:00");
        assert_eq!(info.cat_share, None);
    }

    /// 验证空 key 序列化时被跳过
    #[test]
    fn test_device_info_skip_none() {
        let info = DeviceInfo {
            state: 0,
            key: None,
            mac: "AA:BB:CC:DD:EE:FF".to_string(),
            cat_share: None,
        };

        let json = serde_json::to_string(&info).unwrap();

        // None 字段应该被跳过
        assert!(!json.contains("key"));
        assert!(!json.contains("catShare"));
    }
}
