//! WiFi P2P 模块
//!
//! 提供与 CatShare 兼容的 WiFi Direct 功能。
//!
//! # 模块
//!
//! - `nm_dbus`: NetworkManager D-Bus 客户端 (推荐)
//! - `p2p_sender`: P2P 热点创建（发送端）
//! - `p2p_receiver`: P2P 连接（接收端）
//!
//! # P2pInfo
//!
//! 核心数据结构，用于在 BLE 握手时交换 WiFi 连接信息。
//! 敏感字段（SSID、PSK、MAC）可以使用 AES-CTR 加密。

pub mod nm_dbus;
pub mod p2p_receiver;
pub mod p2p_sender;

#[cfg(test)]
mod tests;

pub use nm_dbus::NmClient;
pub use p2p_receiver::{P2pReceiverConfig, WiFiP2pReceiver};
pub use p2p_sender::{P2pConfig, WiFiP2pSender};

/// 检查进程是否具有必要的权限
///
/// 返回 (has_nmcli, has_net_raw)
/// - has_nmcli: 系统中是否安装了 NetworkManager (nmcli)
/// - has_net_raw: 是否有 CAP_NET_RAW (用于 BLE 扫描)
pub fn check_capabilities() -> (bool, bool) {
    let mut has_nmcli = false;
    let mut has_net_raw = false;

    // 检查是否是 root
    unsafe {
        if libc::geteuid() == 0 {
            return (true, true);
        }
    }

    // 检查 CAP_NET_RAW (用于 BLE 扫描)
    if let Ok(status) = std::fs::read_to_string("/proc/self/status") {
        for line in status.lines() {
            if line.starts_with("CapEff:")
                && let Some(hex) = line.split_whitespace().nth(1)
                && let Ok(caps) = u64::from_str_radix(hex, 16)
            {
                // CAP_NET_RAW = 13
                has_net_raw = (caps & (1 << 13)) != 0;
            }
        }
    }

    // 检查 nmcli 是否可用
    if let Ok(output) = std::process::Command::new("nmcli")
        .arg("--version")
        .output()
    {
        has_nmcli = output.status.success();
    }

    (has_nmcli, has_net_raw)
}

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
