//! WiFi P2P 发送端 (Group Owner / Hotspot)
//!
//! 使用 NetworkManager D-Bus API 创建 WiFi 热点。
//!
//! # 实现方式
//!
//! 1. 优先使用 `NmClient` (D-Bus) 创建热点
//! 2. 如果 NM 不可用，退回到 `wpa_cli` 创建 P2P 组
//!
//! # 注意事项
//!
//! - 使用 NM 时不需要额外权限（依赖 PolicyKit）
//! - 5GHz 频段优先（更快速度）

use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use log::{debug, info, warn};
use tokio::sync::Mutex;

use crate::wifi::P2pInfo;
use crate::wifi::nm_dbus::NmClient;

/// WiFi P2P 配置
pub struct P2pConfig {
    /// 网络接口名称 (通常是 wlan0)
    pub interface: String,
    /// SSID 前缀
    pub ssid_prefix: String,
    /// 是否使用 5GHz
    pub use_5ghz: bool,
}

impl Default for P2pConfig {
    fn default() -> Self {
        Self {
            interface: "wlan0".to_string(),
            ssid_prefix: "DIRECT-".to_string(),
            use_5ghz: true,
        }
    }
}

/// 活动连接信息（用于清理）
struct ActiveHotspot {
    connection_name: String,
    _connection_path: Option<String>,
}

pub struct WiFiP2pSender {
    config: P2pConfig,
    nm_client: Arc<Mutex<Option<NmClient>>>,
    active_hotspot: Arc<Mutex<Option<ActiveHotspot>>>,
}

impl WiFiP2pSender {
    pub fn new(interface: &str) -> Self {
        Self {
            config: P2pConfig {
                interface: interface.to_string(),
                ..Default::default()
            },
            nm_client: Arc::new(Mutex::new(None)),
            active_hotspot: Arc::new(Mutex::new(None)),
        }
    }

    pub fn with_config(config: P2pConfig) -> Self {
        Self {
            config,
            nm_client: Arc::new(Mutex::new(None)),
            active_hotspot: Arc::new(Mutex::new(None)),
        }
    }

    /// 初始化 NM 客户端
    async fn ensure_nm_client(&self) -> anyhow::Result<()> {
        let mut client = self.nm_client.lock().await;
        if client.is_none() {
            match NmClient::new().await {
                Ok(c) => {
                    info!("NetworkManager D-Bus client initialized");
                    *client = Some(c);
                }
                Err(e) => {
                    warn!("Failed to initialize NM client: {}", e);
                    return Err(e);
                }
            }
        }
        Ok(())
    }

    /// 生成随机 SSID 和 PSK
    fn generate_credentials(&self) -> (String, String) {
        let random_chars: String = (0..8)
            .map(|_| {
                let idx = rand::random::<usize>() % 36;
                if idx < 10 {
                    (b'0' + idx as u8) as char
                } else {
                    (b'a' + (idx - 10) as u8) as char
                }
            })
            .collect();

        let ssid = format!("{}{}", self.config.ssid_prefix, random_chars);
        let psk: String = (0..8)
            .map(|_| {
                let idx = rand::random::<usize>() % 36;
                if idx < 10 {
                    (b'0' + idx as u8) as char
                } else {
                    (b'a' + (idx - 10) as u8) as char
                }
            })
            .collect();

        (ssid, psk)
    }

    /// 创建 WiFi P2P 组（热点模式）
    ///
    /// 返回 P2P 信息，包含 SSID、密码和端口
    pub async fn create_group(&self, port: i32) -> anyhow::Result<P2pInfo> {
        let (ssid, psk) = self.generate_credentials();

        // 获取 MAC 地址
        let mac = self.get_mac_address()?;

        // 尝试使用 NmClient (D-Bus) 创建热点
        match self.create_hotspot_nm(&ssid, &psk).await {
            Ok(_) => {
                info!("Hotspot created via NetworkManager D-Bus");
            }
            Err(e) => {
                warn!("NM D-Bus hotspot failed: {}, trying wpa_cli", e);
                // 退回到 wpa_cli
                if let Err(wpa_err) = self.create_p2p_group_wpa(&ssid, &psk).await {
                    warn!("wpa_cli also failed: {}", wpa_err);
                    return Err(anyhow::anyhow!(
                        "Failed to create hotspot: NM={}, wpa_cli={}",
                        e,
                        wpa_err
                    ));
                }
            }
        }

        Ok(P2pInfo::new(ssid, psk, mac, port))
    }

    /// 使用 NetworkManager D-Bus 创建热点
    async fn create_hotspot_nm(&self, ssid: &str, psk: &str) -> anyhow::Result<()> {
        self.ensure_nm_client().await?;

        let client_guard = self.nm_client.lock().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("NM client not initialized"))?;

        // 先删除可能存在的旧连接
        let conn_name = format!(
            "cattysend-hotspot-{}",
            &ssid[..std::cmp::min(8, ssid.len())]
        );
        let _ = client.delete_connection_by_name(&conn_name).await;

        let band = if self.config.use_5ghz { "a" } else { "bg" };

        // 创建热点连接配置
        let conn_path = client
            .create_hotspot(ssid, psk, band, &self.config.interface)
            .await?;

        // 查找设备
        let device = client
            .find_wifi_device(Some(&self.config.interface))
            .await?
            .ok_or_else(|| anyhow::anyhow!("WiFi device {} not found", self.config.interface))?;

        // 激活连接
        let active_conn = client
            .activate_connection(&conn_path.as_ref(), &device)
            .await?;

        // 等待激活完成
        let ip = client
            .wait_for_ip(&active_conn.as_ref(), Duration::from_secs(15))
            .await?;
        info!("Hotspot active with IP: {}", ip);

        // 记录活动热点信息（用于清理）
        let mut hotspot = self.active_hotspot.lock().await;
        *hotspot = Some(ActiveHotspot {
            connection_name: conn_name,
            _connection_path: Some(conn_path.to_string()),
        });

        Ok(())
    }

    /// 使用 wpa_cli 创建 P2P 组 (备用方案)
    async fn create_p2p_group_wpa(&self, ssid: &str, psk: &str) -> anyhow::Result<()> {
        let output = Command::new("wpa_cli")
            .args([
                "-i",
                &self.config.interface,
                "p2p_group_add",
                &format!("persistent ssid={} passphrase={}", ssid, psk),
            ])
            .output()?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("wpa_cli p2p_group_add failed: {}", err));
        }

        // 等待组创建完成
        tokio::time::sleep(Duration::from_secs(2)).await;

        // 记录活动热点
        let mut hotspot = self.active_hotspot.lock().await;
        *hotspot = Some(ActiveHotspot {
            connection_name: ssid.to_string(),
            _connection_path: None,
        });

        Ok(())
    }

    /// 停止 P2P 组
    pub async fn stop_group(&self) -> anyhow::Result<()> {
        debug!("Stopping P2P group/hotspot");

        let hotspot = self.active_hotspot.lock().await.take();

        if let Some(info) = hotspot {
            // 使用 NM D-Bus 删除连接
            if let Ok(()) = self.ensure_nm_client().await {
                let client_guard = self.nm_client.lock().await;
                if let Some(client) = client_guard.as_ref() {
                    let _ = client
                        .delete_connection_by_name(&info.connection_name)
                        .await;
                }
            }
        }

        // 也尝试 wpa_cli 停止（兼容性）
        let _ = Command::new("wpa_cli")
            .args(["-i", &self.config.interface, "p2p_group_remove", "*"])
            .output();

        Ok(())
    }

    /// 获取接口 MAC 地址
    fn get_mac_address(&self) -> anyhow::Result<String> {
        // 尝试从 sysfs 读取
        let path = format!("/sys/class/net/{}/address", self.config.interface);
        if let Ok(mac) = std::fs::read_to_string(&path) {
            return Ok(mac.trim().to_uppercase());
        }

        // 尝试读取 p2p 接口
        let p2p_path = format!("/sys/class/net/p2p-dev-{}/address", self.config.interface);
        if let Ok(mac) = std::fs::read_to_string(&p2p_path) {
            return Ok(mac.trim().to_uppercase());
        }

        // 返回默认 MAC
        Ok("02:00:00:00:00:00".to_string())
    }

    /// 获取热点的 IP 地址
    pub fn get_hotspot_ip(&self) -> anyhow::Result<String> {
        // 通常热点的 IP 是 10.42.0.1 (nmcli) 或 192.168.49.1 (wpa_supplicant)
        let output = Command::new("ip").args(["-o", "addr", "show"]).output()?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if line.contains(&self.config.interface) && line.contains("inet ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if let Some(pos) = parts.iter().position(|&s| s == "inet")
                    && let Some(ip) = parts
                        .get(pos + 1)
                        .and_then(|ip_cidr| ip_cidr.split('/').next())
                {
                    return Ok(ip.to_string());
                }
            }
        }

        // 返回默认 IP
        Ok("10.42.0.1".to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_generate_credentials() {
        let sender = WiFiP2pSender::new("wlan0");
        let (ssid, psk) = sender.generate_credentials();

        assert!(ssid.starts_with("DIRECT-"));
        assert_eq!(ssid.len(), 15); // "DIRECT-" (7) + 8 chars
        assert_eq!(psk.len(), 8);
    }
}
