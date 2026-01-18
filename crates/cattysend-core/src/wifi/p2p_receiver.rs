//! WiFi P2P 接收端 (Client)
//!
//! 连接到发送端创建的 P2P 热点。
//!
//! # 实现方式
//!
//! 1. 优先使用 `wpa_cli` 连接（更轻量）
//! 2. 如果失败，退回到 `nmcli`（NetworkManager）
//!
//! # 注意事项
//!
//! - 连接后自动获取 DHCP 分配的 IP 地址
//! - 断开时会清理相关网络配置

use log::info;

use crate::wifi::P2pInfo;
use std::process::Command;
use std::time::Duration;

/// WiFi P2P 接收端
pub struct WiFiP2pReceiver {
    interface: String,
}

impl WiFiP2pReceiver {
    pub fn new(interface: &str) -> Self {
        Self {
            interface: interface.to_string(),
        }
    }

    /// 连接到 P2P 热点
    ///
    /// 返回分配的 IP 地址
    pub async fn connect(&self, info: &P2pInfo) -> anyhow::Result<String> {
        info!(
            "Connecting to WiFi: ssid='{}', interface='{}'",
            info.ssid, self.interface
        );

        // 尝试使用 wpa_cli 连接
        if let Ok(ip) = self.connect_wpa(&info.ssid, &info.psk).await {
            return Ok(ip);
        }

        // 尝试使用 nmcli 连接
        self.connect_nmcli(&info.ssid, &info.psk).await
    }

    /// 使用 wpa_cli 连接
    async fn connect_wpa(&self, ssid: &str, psk: &str) -> anyhow::Result<String> {
        // 添加网络
        let output = Command::new("wpa_cli")
            .args(["-i", &self.interface, "add_network"])
            .output()?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("Failed to add network"));
        }

        let network_id = String::from_utf8_lossy(&output.stdout).trim().to_string();

        // 设置 SSID
        Command::new("wpa_cli")
            .args([
                "-i",
                &self.interface,
                "set_network",
                &network_id,
                "ssid",
                &format!("\"{}\"", ssid),
            ])
            .output()?;

        // 设置 PSK
        Command::new("wpa_cli")
            .args([
                "-i",
                &self.interface,
                "set_network",
                &network_id,
                "psk",
                &format!("\"{}\"", psk),
            ])
            .output()?;

        // 启用网络
        Command::new("wpa_cli")
            .args(["-i", &self.interface, "enable_network", &network_id])
            .output()?;

        // 选择网络
        Command::new("wpa_cli")
            .args(["-i", &self.interface, "select_network", &network_id])
            .output()?;

        // 等待连接
        tokio::time::sleep(Duration::from_secs(3)).await;

        // 获取 IP 地址
        self.get_interface_ip()
    }

    /// 使用 nmcli 连接
    async fn connect_nmcli(&self, ssid: &str, psk: &str) -> anyhow::Result<String> {
        use log::{debug, warn};

        // 先删除旧连接
        let _ = Command::new("nmcli")
            .args(["connection", "delete", ssid])
            .output();

        // WiFi Direct 热点可能需要一些时间才能被发现
        // 我们尝试多次扫描和连接
        let max_retries = 5;
        let mut last_error = String::new();

        for attempt in 1..=max_retries {
            debug!("WiFi connection attempt {}/{}", attempt, max_retries);

            // 触发 WiFi 扫描
            debug!("Triggering WiFi scan on interface {}", self.interface);
            let _ = Command::new("nmcli")
                .args(["device", "wifi", "rescan", "ifname", &self.interface])
                .output();

            // 等待扫描完成
            tokio::time::sleep(Duration::from_secs(2)).await;

            // 列出可见的 WiFi 网络（用于调试）
            if let Ok(list_output) = Command::new("nmcli")
                .args([
                    "-t",
                    "-f",
                    "SSID,SIGNAL",
                    "device",
                    "wifi",
                    "list",
                    "ifname",
                    &self.interface,
                ])
                .output()
            {
                let networks = String::from_utf8_lossy(&list_output.stdout);
                let network_count = networks.lines().count();
                debug!("Visible WiFi networks ({} total):", network_count);

                // 检查目标 SSID 是否在列表中
                let target_found = networks.lines().any(|line| line.starts_with(ssid));
                if target_found {
                    debug!("  ✓ Target SSID '{}' found in scan results", ssid);
                } else {
                    debug!("  ✗ Target SSID '{}' NOT found. Available networks:", ssid);
                    for line in networks.lines().take(10) {
                        debug!("    - {}", line);
                    }
                }
            }

            // 尝试连接
            let output = Command::new("nmcli")
                .args([
                    "device",
                    "wifi",
                    "connect",
                    ssid,
                    "password",
                    psk,
                    "ifname",
                    &self.interface,
                ])
                .output()?;

            if output.status.success() {
                debug!("WiFi connection successful on attempt {}", attempt);
                // 等待 DHCP
                tokio::time::sleep(Duration::from_secs(2)).await;
                return self.get_interface_ip();
            }

            last_error = String::from_utf8_lossy(&output.stderr).to_string();
            warn!(
                "WiFi connection attempt {} failed: {}",
                attempt,
                last_error.trim()
            );

            if attempt < max_retries {
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }

        Err(anyhow::anyhow!(
            "nmcli connection failed after {} attempts: {}",
            max_retries,
            last_error
        ))
    }

    /// 断开连接
    pub async fn disconnect(&self) -> anyhow::Result<()> {
        // 尝试 wpa_cli
        let _ = Command::new("wpa_cli")
            .args(["-i", &self.interface, "disconnect"])
            .output();

        // 尝试 nmcli
        let _ = Command::new("nmcli")
            .args(["device", "disconnect", &self.interface])
            .output();

        Ok(())
    }

    /// 获取接口 IP 地址
    fn get_interface_ip(&self) -> anyhow::Result<String> {
        let output = Command::new("ip")
            .args(["-o", "addr", "show", &self.interface])
            .output()?;

        let stdout = String::from_utf8(output.stdout)?;
        for line in stdout.lines() {
            if line.contains("inet ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if let Some(pos) = parts.iter().position(|&s| s == "inet")
                    && let Some(ip) = parts
                        .get(pos + 1)
                        .and_then(|ip_range| ip_range.split('/').next())
                {
                    return Ok(ip.to_string());
                }
            }
        }
        Err(anyhow::anyhow!(
            "Could not find IP address for {}",
            self.interface
        ))
    }

    /// 检查是否已连接
    pub fn is_connected(&self) -> bool {
        let output = Command::new("wpa_cli")
            .args(["-i", &self.interface, "status"])
            .output();

        if let Ok(output) = output {
            let stdout = String::from_utf8_lossy(&output.stdout);
            return stdout.contains("wpa_state=COMPLETED");
        }

        false
    }
}
