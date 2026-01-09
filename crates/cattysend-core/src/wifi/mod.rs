//! WiFi P2P 模块
//!
//! 提供与 CatShare 兼容的 WiFi Direct 功能。
//!
//! # 模块
//!
//! - `p2p_sender`: P2P 热点创建（发送端）
//! - `p2p_receiver`: P2P 连接（接收端）
//!
//! # P2pInfo
//!
//! 核心数据结构，用于在 BLE 握手时交换 WiFi 连接信息。
//! 敏感字段（SSID、PSK、MAC）可以使用 AES-CTR 加密。

pub mod p2p_receiver;
pub mod p2p_sender;

pub use p2p_receiver::WiFiP2pReceiver;
pub use p2p_sender::{P2pConfig, WiFiP2pSender};

/// P2pInfo - 与 CatShare 的 P2pInfo 完全兼容
///
/// CatShare Kotlin 定义:
/// ```kotlin
/// data class P2pInfo(
///     val id: String?,
///     val ssid: String,
///     val psk: String,
///     val mac: String,
///     val port: Int,
///     val key: String? = null,
///     val catShare: Int? = null,
/// )
/// ```
///
/// # 字段
///
/// - `id`: 发送端 ID（可选）
/// - `ssid`: WiFi SSID（可加密）
/// - `psk`: WiFi 密码（可加密）
/// - `mac`: 发送端 MAC 地址（可加密）
/// - `port`: HTTPS 服务端口
/// - `key`: 发送端 ECDH 公钥（用于解密上述字段）
/// - `cat_share`: 协议版本号
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone, PartialEq)]
#[serde(rename_all = "camelCase")]
pub struct P2pInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub ssid: String,
    pub psk: String,
    pub mac: String,
    pub port: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cat_share: Option<i32>,
}

impl P2pInfo {
    /// 创建未加密的 P2pInfo
    pub fn new(ssid: String, psk: String, mac: String, port: i32) -> Self {
        Self {
            id: None,
            ssid,
            psk,
            mac,
            port,
            key: None,
            cat_share: Some(1),
        }
    }

    /// 创建带加密字段的 P2pInfo
    ///
    /// # 参数
    ///
    /// - `id`: 发送端 ID
    /// - `ssid_encrypted`: 加密后的 SSID (Base64)
    /// - `psk_encrypted`: 加密后的密码 (Base64)
    /// - `mac_encrypted`: 加密后的 MAC 地址 (Base64)
    /// - `port`: 服务端口
    /// - `sender_public_key`: 发送端公钥（接收端用此解密）
    pub fn with_encryption(
        id: String,
        ssid_encrypted: String,
        psk_encrypted: String,
        mac_encrypted: String,
        port: i32,
        sender_public_key: String,
    ) -> Self {
        Self {
            id: Some(id),
            ssid: ssid_encrypted,
            psk: psk_encrypted,
            mac: mac_encrypted,
            port,
            key: Some(sender_public_key),
            cat_share: Some(1),
        }
    }

    /// 获取发送端的 HTTPS 地址
    pub fn get_server_url(&self, host_ip: &str) -> String {
        format!("https://{}:{}", host_ip, self.port)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证 P2pInfo 序列化与 CatShare 兼容
    #[test]
    fn test_p2p_info_serialization() {
        let info = P2pInfo::new(
            "DIRECT-abc".to_string(),
            "password123".to_string(),
            "AA:BB:CC:DD:EE:FF".to_string(),
            8443,
        );

        let json = serde_json::to_string(&info).unwrap();

        // 验证 camelCase 命名
        assert!(json.contains("\"ssid\":"));
        assert!(json.contains("\"psk\":"));
        assert!(json.contains("\"mac\":"));
        assert!(json.contains("\"port\":"));
        assert!(json.contains("\"catShare\":")); // 不是 cat_share

        // 验证值
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["ssid"], "DIRECT-abc");
        assert_eq!(parsed["psk"], "password123");
        assert_eq!(parsed["mac"], "AA:BB:CC:DD:EE:FF");
        assert_eq!(parsed["port"], 8443);
        assert_eq!(parsed["catShare"], 1);
    }

    /// 验证 P2pInfo 反序列化与 CatShare 兼容
    #[test]
    fn test_p2p_info_deserialization() {
        // 模拟 CatShare 发送的加密 P2pInfo
        let json = r#"{
            "id": "abcd",
            "ssid": "ENCRYPTED_SSID",
            "psk": "ENCRYPTED_PSK",
            "mac": "ENCRYPTED_MAC",
            "port": 9000,
            "key": "SENDER_PUBLIC_KEY",
            "catShare": 2
        }"#;

        let info: P2pInfo = serde_json::from_str(json).unwrap();

        assert_eq!(info.id, Some("abcd".to_string()));
        assert_eq!(info.ssid, "ENCRYPTED_SSID");
        assert_eq!(info.psk, "ENCRYPTED_PSK");
        assert_eq!(info.mac, "ENCRYPTED_MAC");
        assert_eq!(info.port, 9000);
        assert_eq!(info.key, Some("SENDER_PUBLIC_KEY".to_string()));
        assert_eq!(info.cat_share, Some(2));
    }

    /// 验证 P2pInfo 可选字段被正确跳过
    #[test]
    fn test_p2p_info_skip_none() {
        let info = P2pInfo::new(
            "SSID".to_string(),
            "PSK".to_string(),
            "MAC".to_string(),
            8080,
        );

        let json = serde_json::to_string(&info).unwrap();

        // id 和 key 是 None，应该被跳过
        assert!(!json.contains("\"id\":"));
        assert!(!json.contains("\"key\":"));
    }

    /// 验证 with_encryption 构造函数
    #[test]
    fn test_p2p_info_with_encryption() {
        let info = P2pInfo::with_encryption(
            "sender123".to_string(),
            "encrypted_ssid".to_string(),
            "encrypted_psk".to_string(),
            "encrypted_mac".to_string(),
            8443,
            "public_key_base64".to_string(),
        );

        assert_eq!(info.id, Some("sender123".to_string()));
        assert_eq!(info.key, Some("public_key_base64".to_string()));
        assert_eq!(info.cat_share, Some(1));
    }

    /// 验证 get_server_url 方法
    #[test]
    fn test_p2p_info_get_server_url() {
        let info = P2pInfo::new(
            "SSID".to_string(),
            "PSK".to_string(),
            "MAC".to_string(),
            8443,
        );

        assert_eq!(
            info.get_server_url("192.168.1.1"),
            "https://192.168.1.1:8443"
        );
        assert_eq!(info.get_server_url("10.42.0.1"), "https://10.42.0.1:8443");
    }
}
