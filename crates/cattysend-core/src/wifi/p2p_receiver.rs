//! WiFi P2P 接收端 (Client)
//!
//! 连接到发送端创建的 P2P 热点。
//!
//! # 连接策略（优先级从高到低）
//!
//! 1. **nmcli wifi-p2p** (推荐): 使用 NetworkManager 原生 P2P 支持，
//!    不需要 sudo，不会断开现有 WiFi。
//! 2. **普通 WiFi 连接**: 如果 P2P 不可用，退回到普通连接模式，
//!    会临时断开现有 WiFi。
//!
//! # 注意事项
//!
//! - 连接后自动获取 DHCP 分配的 IP 地址
//! - 断开时会清理相关网络配置

use log::{debug, info, warn};

use crate::wifi::P2pInfo;
use std::process::Command;
use std::time::Duration;

/// WiFi P2P 接收端配置
#[derive(Debug, Clone)]
pub struct P2pReceiverConfig {
    /// 主 WiFi 接口名（如 wlan0）
    pub main_interface: String,
    /// P2P 设备接口名（如 p2p-dev-wlan0）
    pub p2p_device: Option<String>,
    /// 是否优先保持原有 WiFi 连接
    pub preserve_wifi: bool,
}

impl Default for P2pReceiverConfig {
    fn default() -> Self {
        Self {
            main_interface: "wlan0".to_string(),
            p2p_device: None,
            preserve_wifi: true,
        }
    }
}

/// WiFi P2P 接收端
pub struct WiFiP2pReceiver {
    config: P2pReceiverConfig,
    /// 实际使用的接口名
    active_interface: Option<String>,
    /// NetworkManager 连接名（用于断开时清理）
    nm_connection_name: Option<String>,
    /// 是否使用了 P2P 模式（true = 双连接成功）
    used_p2p_mode: bool,
}

impl WiFiP2pReceiver {
    pub fn new(interface: &str) -> Self {
        Self {
            config: P2pReceiverConfig {
                main_interface: interface.to_string(),
                ..Default::default()
            },
            active_interface: None,
            nm_connection_name: None,
            used_p2p_mode: false,
        }
    }

    pub fn with_config(config: P2pReceiverConfig) -> Self {
        Self {
            config,
            active_interface: None,
            nm_connection_name: None,
            used_p2p_mode: false,
        }
    }

    /// 连接到 P2P 热点
    ///
    /// 返回分配的 IP 地址
    pub async fn connect(&mut self, info: &P2pInfo) -> anyhow::Result<String> {
        info!(
            "Connecting to WiFi Direct: ssid='{}', preserve_wifi={}",
            info.ssid, self.config.preserve_wifi
        );

        // 策略1: 尝试使用 nmcli P2P 模式（不需要 sudo，不会断开现有 WiFi）
        if self.config.preserve_wifi {
            if let Some(p2p_dev) = self.find_p2p_device() {
                info!("Found P2P device: {}, attempting dual connection", p2p_dev);

                match self
                    .connect_nmcli_p2p(&p2p_dev, &info.ssid, &info.psk)
                    .await
                {
                    Ok(ip) => {
                        self.used_p2p_mode = true;
                        return Ok(ip);
                    }
                    Err(e) => {
                        warn!("P2P connection failed: {}, falling back to normal WiFi", e);
                    }
                }
            } else {
                debug!("No P2P device found, using normal WiFi connection");
            }
        }

        // 策略2: 普通 WiFi 连接（会断开现有 WiFi）
        info!(
            "Using main interface '{}' (original WiFi will disconnect)",
            self.config.main_interface
        );
        self.active_interface = Some(self.config.main_interface.clone());

        self.connect_nmcli_wifi(&self.config.main_interface.clone(), &info.ssid, &info.psk)
            .await
    }

    /// 查找 P2P 设备接口（如 p2p-dev-wlan0）
    fn find_p2p_device(&self) -> Option<String> {
        // 使用 nmcli 查找 wifi-p2p 类型的设备
        if let Ok(output) = Command::new("nmcli")
            .args(["-t", "-f", "DEVICE,TYPE", "device"])
            .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            for line in stdout.lines() {
                let parts: Vec<&str> = line.split(':').collect();
                if parts.len() >= 2 && parts[1] == "wifi-p2p" {
                    return Some(parts[0].to_string());
                }
            }
        }

        // 回退: 尝试标准命名
        let default_p2p = format!("p2p-dev-{}", self.config.main_interface);
        if let Ok(output) = Command::new("nmcli")
            .args(["device", "show", &default_p2p])
            .output()
            && output.status.success()
        {
            return Some(default_p2p);
        }

        None
    }

    /// 使用 nmcli wifi-p2p 连接（不需要 sudo，保持现有 WiFi）
    async fn connect_nmcli_p2p(
        &mut self,
        p2p_device: &str,
        ssid: &str,
        psk: &str,
    ) -> anyhow::Result<String> {
        debug!("Connecting via nmcli wifi-p2p on device {}", p2p_device);

        let conn_name = format!("cattysend-p2p-{}", &ssid[..std::cmp::min(8, ssid.len())]);

        // 先删除可能存在的旧连接
        let _ = Command::new("nmcli")
            .args(["connection", "delete", &conn_name])
            .output();

        // 先扫描
        let _ = Command::new("nmcli")
            .args(["device", "wifi", "rescan"])
            .output();

        tokio::time::sleep(Duration::from_secs(2)).await;

        // 尝试创建连接
        // 注意：这里不再强制绑定 ifname。
        // 因为 NetworkManager 的 P2P 虚拟设备 (p2p-dev-wlan0) 目前不支持 WPA-PSK (密码) 连接。
        // 不绑定 ifname 会让 NM 的物理网卡 (wlan0) 接管请求，虽然会导致断网，但能确保连接成功。
        let output = Command::new("nmcli")
            .args([
                "connection",
                "add",
                "type",
                "wifi",
                "con-name",
                &conn_name,
                "ssid",
                ssid,
                "wifi-sec.key-mgmt",
                "wpa-psk",
                "wifi-sec.psk",
                psk,
                "connection.autoconnect",
                "no",
            ])
            .output()?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("Failed to create connection: {}", err));
        }

        self.nm_connection_name = Some(conn_name.clone());

        // 激活连接
        let output = Command::new("nmcli")
            .args(["connection", "up", &conn_name])
            .output()?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            // 清理失败的连接
            let _ = Command::new("nmcli")
                .args(["connection", "delete", &conn_name])
                .output();
            return Err(anyhow::anyhow!("Failed to activate connection: {}", err));
        }

        // 等待连接建立和 IP 获取
        for i in 1..=10 {
            tokio::time::sleep(Duration::from_secs(1)).await;

            if let Ok(ip) = self.get_connection_ip(&conn_name) {
                info!("P2P connection established on attempt {}", i);
                return Ok(ip);
            }
        }

        Err(anyhow::anyhow!("Timeout waiting for IP address"))
    }

    /// 使用普通 nmcli wifi 连接（会断开现有 WiFi）
    async fn connect_nmcli_wifi(
        &mut self,
        interface: &str,
        ssid: &str,
        psk: &str,
    ) -> anyhow::Result<String> {
        debug!("Connecting via nmcli wifi on interface {}", interface);

        let conn_name = format!("cattysend-wifi-{}", &ssid[..std::cmp::min(8, ssid.len())]);

        // 先删除可能存在的旧连接
        let _ = Command::new("nmcli")
            .args(["connection", "delete", &conn_name])
            .output();

        // WiFi Direct 热点可能需要一些时间才能被发现
        let max_retries = 5;

        for attempt in 1..=max_retries {
            debug!("WiFi connection attempt {}/{}", attempt, max_retries);

            // 触发 WiFi 扫描
            let _ = Command::new("nmcli")
                .args(["device", "wifi", "rescan", "ifname", interface])
                .output();

            tokio::time::sleep(Duration::from_secs(2)).await;

            // 尝试直接连接
            let output = Command::new("nmcli")
                .args([
                    "device", "wifi", "connect", ssid, "password", psk, "ifname", interface,
                ])
                .output()?;

            if output.status.success() {
                info!("✅ WiFi connection successful on attempt {}", attempt);
                self.nm_connection_name = Some(ssid.to_string());
                tokio::time::sleep(Duration::from_secs(2)).await;
                return self.get_interface_ip(interface);
            }

            let err = String::from_utf8_lossy(&output.stderr);
            warn!("WiFi connection attempt {} failed: {}", attempt, err.trim());
        }

        Err(anyhow::anyhow!(
            "WiFi connection failed after {} attempts",
            max_retries
        ))
    }

    /// 获取连接的 IP 地址
    fn get_connection_ip(&self, conn_name: &str) -> anyhow::Result<String> {
        let output = Command::new("nmcli")
            .args(["-t", "-f", "IP4.ADDRESS", "connection", "show", conn_name])
            .output()?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        for line in stdout.lines() {
            if line.starts_with("IP4.ADDRESS")
                && let Some(ip_cidr) = line.split(':').nth(1)
                && let Some(ip) = ip_cidr.split('/').next()
            {
                return Ok(ip.to_string());
            }
        }

        Err(anyhow::anyhow!(
            "No IP address found for connection {}",
            conn_name
        ))
    }

    /// 断开连接并清理
    pub async fn disconnect(&mut self) -> anyhow::Result<()> {
        info!("Disconnecting WiFi P2P connection");

        // 删除 NetworkManager 连接
        if let Some(conn_name) = &self.nm_connection_name {
            debug!("Removing NM connection: {}", conn_name);
            let _ = Command::new("nmcli")
                .args(["connection", "delete", conn_name])
                .output();
        }

        // 如果有活动接口，断开它
        if let Some(iface) = &self.active_interface {
            let _ = Command::new("nmcli")
                .args(["device", "disconnect", iface])
                .output();
        }

        self.active_interface = None;
        self.nm_connection_name = None;
        self.used_p2p_mode = false;

        Ok(())
    }

    /// 获取接口 IP 地址
    fn get_interface_ip(&self, interface: &str) -> anyhow::Result<String> {
        let output = Command::new("ip")
            .args(["-o", "addr", "show", interface])
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
            interface
        ))
    }

    /// 检查是否已连接
    pub fn is_connected(&self) -> bool {
        if let Some(conn_name) = &self.nm_connection_name
            && let Ok(output) = Command::new("nmcli")
                .args(["-t", "-f", "STATE", "connection", "show", conn_name])
                .output()
        {
            let stdout = String::from_utf8_lossy(&output.stdout);
            return stdout.contains("activated");
        }
        false
    }

    /// 获取当前使用的接口名
    pub fn active_interface(&self) -> Option<&str> {
        self.active_interface.as_deref()
    }

    /// 原有 WiFi 是否保持连接
    pub fn is_dual_connected(&self) -> bool {
        self.used_p2p_mode
    }
}

impl Drop for WiFiP2pReceiver {
    fn drop(&mut self) {
        // 尝试清理（同步版本）
        if let Some(conn_name) = &self.nm_connection_name {
            let _ = Command::new("nmcli")
                .args(["connection", "delete", conn_name])
                .output();
        }
    }
}
