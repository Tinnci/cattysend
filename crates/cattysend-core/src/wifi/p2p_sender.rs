//! WiFi P2P 发送端 (Group Owner / Hotspot)
//!
//! 使用 wpa_supplicant 或 NetworkManager 创建 P2P 热点。
//!
//! # 实现方式
//!
//! 1. 优先使用 `wpa_cli p2p_group_add` 创建真正的 Wi-Fi Direct 组
//! 2. 如果失败，退回到 `nmcli` 创建普通热点
//!
//! # 注意事项
//!
//! - 需要 `CAP_NET_ADMIN` 权限或 root
//! - 5GHz 频段优先(更快速度)

use log::warn;

use crate::wifi::P2pInfo;
use std::process::Command;

/// WiFi P2P 配置
pub struct P2pConfig {
    /// 网络接口名称 (通常是 p2p-dev-wlan0 或 p2p0)
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

pub struct WiFiP2pSender {
    config: P2pConfig,
}

impl WiFiP2pSender {
    pub fn new(interface: &str) -> Self {
        Self {
            config: P2pConfig {
                interface: interface.to_string(),
                ..Default::default()
            },
        }
    }

    pub fn with_config(config: P2pConfig) -> Self {
        Self { config }
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

        // 尝试使用 wpa_cli 创建 P2P 组
        // 如果失败，使用 nmcli 创建热点
        if let Err(e) = self.create_p2p_group_wpa(&ssid, &psk).await {
            warn!("wpa_cli P2P group creation failed: {}, trying nmcli", e);
            self.create_hotspot_nmcli(&ssid, &psk).await?;
        }

        Ok(P2pInfo::new(ssid, psk, mac, port))
    }

    /// 使用 wpa_cli 创建 P2P 组
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
        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        Ok(())
    }

    /// 使用 nmcli 创建热点 (备用方案)
    async fn create_hotspot_nmcli(&self, ssid: &str, psk: &str) -> anyhow::Result<()> {
        // 先尝试删除旧的热点
        let _ = Command::new("nmcli")
            .args(["connection", "delete", "cattysend-hotspot"])
            .output();

        let band = if self.config.use_5ghz { "a" } else { "bg" };

        let output = Command::new("nmcli")
            .args([
                "device",
                "wifi",
                "hotspot",
                "ifname",
                &self.config.interface,
                "con-name",
                "cattysend-hotspot",
                "ssid",
                ssid,
                "password",
                psk,
                "band",
                band,
            ])
            .output()?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("nmcli hotspot creation failed: {}", err));
        }

        Ok(())
    }

    /// 停止 P2P 组
    pub async fn stop_group(&self) -> anyhow::Result<()> {
        // 尝试使用 wpa_cli 停止
        let _ = Command::new("wpa_cli")
            .args(["-i", &self.config.interface, "p2p_group_remove", "*"])
            .output();

        // 尝试使用 nmcli 停止
        let _ = Command::new("nmcli")
            .args(["connection", "down", "cattysend-hotspot"])
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
