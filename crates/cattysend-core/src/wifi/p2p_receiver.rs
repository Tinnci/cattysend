//! WiFi P2P 接收端 (Client)
//!
//! 连接到发送端创建的 P2P 热点。
//!
//! # 双连接支持
//!
//! 在支持多接口的网卡上，优先使用独立的 P2P 接口连接，
//! 这样不会断开原有的 WiFi 连接。
//!
//! # 实现方式
//!
//! 1. 检测是否支持多接口（双连接）
//! 2. 如果支持，创建虚拟 P2P 接口进行连接
//! 3. 如果不支持或失败，回退到使用主接口
//!
//! # 注意事项
//!
//! - 连接后自动获取 DHCP 分配的 IP 地址
//! - 断开时会清理相关网络配置和虚拟接口

use log::{debug, info, warn};

use crate::wifi::P2pInfo;
use std::process::Command;
use std::time::Duration;

/// WiFi P2P 接收端配置
#[derive(Debug, Clone)]
pub struct P2pReceiverConfig {
    /// 主 WiFi 接口名（如 wlan0）
    pub main_interface: String,
    /// P2P 接口名（如 p2p0, 会自动创建）
    pub p2p_interface: Option<String>,
    /// 是否优先保持原有 WiFi 连接
    pub preserve_wifi: bool,
}

impl Default for P2pReceiverConfig {
    fn default() -> Self {
        Self {
            main_interface: "wlan0".to_string(),
            p2p_interface: None,
            preserve_wifi: true,
        }
    }
}

/// WiFi P2P 接收端
pub struct WiFiP2pReceiver {
    config: P2pReceiverConfig,
    /// 实际使用的接口名
    active_interface: Option<String>,
    /// 是否创建了虚拟接口
    created_virtual_interface: bool,
    /// 保存的 wpa_supplicant 网络 ID
    network_id: Option<String>,
}

impl WiFiP2pReceiver {
    pub fn new(interface: &str) -> Self {
        Self {
            config: P2pReceiverConfig {
                main_interface: interface.to_string(),
                ..Default::default()
            },
            active_interface: None,
            created_virtual_interface: false,
            network_id: None,
        }
    }

    pub fn with_config(config: P2pReceiverConfig) -> Self {
        Self {
            config,
            active_interface: None,
            created_virtual_interface: false,
            network_id: None,
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

        // 如果配置了保持原有 WiFi，尝试使用双连接
        if self.config.preserve_wifi {
            // 检查是否支持双连接
            if self.supports_multi_interface().await? {
                info!("Multi-interface supported, attempting dual connection");

                // 尝试使用 wpa_supplicant P2P 接口
                if let Ok(ip) = self.connect_p2p_interface(&info.ssid, &info.psk).await {
                    return Ok(ip);
                }

                // 尝试创建虚拟接口
                if let Ok(ip) = self.connect_virtual_interface(&info.ssid, &info.psk).await {
                    return Ok(ip);
                }

                warn!("Dual connection failed, falling back to main interface");
            } else {
                debug!("Multi-interface not supported, using main interface");
            }
        }

        // 回退：使用主接口（会断开原有 WiFi）
        self.active_interface = Some(self.config.main_interface.clone());

        // 尝试使用 wpa_cli 连接
        if let Ok(ip) = self
            .connect_wpa(&self.config.main_interface.clone(), &info.ssid, &info.psk)
            .await
        {
            return Ok(ip);
        }

        // 尝试使用 nmcli 连接
        self.connect_nmcli(&self.config.main_interface.clone(), &info.ssid, &info.psk)
            .await
    }

    /// 检查是否支持多接口（双连接）
    async fn supports_multi_interface(&self) -> anyhow::Result<bool> {
        // 方法1: 检查 iw list 的 interface combinations
        let output = Command::new("iw").args(["list"]).output()?;

        let stdout = String::from_utf8_lossy(&output.stdout);

        // 查找 "valid interface combinations" 部分
        // 如果 total <= N 且 N >= 2，则支持多接口
        if stdout.contains("valid interface combinations") {
            // 检查是否有 "total <= 2" 或更高
            for line in stdout.lines() {
                if line.contains("total <=")
                    && let Some(num_str) = line.split("total <=").nth(1)
                    && let Ok(num) = num_str
                        .trim()
                        .split(',')
                        .next()
                        .unwrap_or("0")
                        .trim()
                        .parse::<u32>()
                    && num >= 2
                {
                    debug!("Multi-interface supported: total <= {}", num);
                    return Ok(true);
                }
            }
        }

        // 方法2: 检查是否存在 p2p-dev 接口
        let output = Command::new("iw").args(["dev"]).output()?;

        let stdout = String::from_utf8_lossy(&output.stdout);
        if stdout.contains("p2p-dev-") || stdout.contains("p2p0") {
            debug!("P2P device interface detected");
            return Ok(true);
        }

        Ok(false)
    }

    /// 使用 wpa_supplicant 的 P2P 接口连接
    async fn connect_p2p_interface(&mut self, ssid: &str, psk: &str) -> anyhow::Result<String> {
        // 查找 P2P 设备接口
        let p2p_dev = self.find_p2p_device_interface()?;
        debug!("Using P2P device interface: {}", p2p_dev);

        // 使用 wpa_cli 的 P2P_CONNECT 或普通连接
        // 对于 DIRECT-* 网络，我们使用普通扫描+连接方式

        // 创建新的 P2P 组接口
        let output = Command::new("wpa_cli")
            .args(["-i", &p2p_dev, "p2p_group_add"])
            .output()?;

        if output.status.success() {
            // 等待 p2p-wlan0-* 接口创建
            tokio::time::sleep(Duration::from_secs(1)).await;

            // 查找新创建的 P2P 组接口
            if let Ok(p2p_group) = self.find_p2p_group_interface() {
                self.active_interface = Some(p2p_group.clone());

                // 在 P2P 组接口上连接
                let ip = self.connect_wpa(&p2p_group, ssid, psk).await?;
                return Ok(ip);
            }
        }

        Err(anyhow::anyhow!("P2P interface connection failed"))
    }

    /// 创建虚拟接口并连接
    async fn connect_virtual_interface(&mut self, ssid: &str, psk: &str) -> anyhow::Result<String> {
        let virt_iface = format!("{}_p2p", self.config.main_interface);

        info!("Creating virtual interface: {}", virt_iface);

        // 先删除可能存在的旧接口
        let _ = Command::new("sudo")
            .args(["iw", "dev", &virt_iface, "del"])
            .output();

        // 创建虚拟接口 (managed 模式可以连接到 AP)
        let output = Command::new("sudo")
            .args([
                "iw",
                "dev",
                &self.config.main_interface,
                "interface",
                "add",
                &virt_iface,
                "type",
                "managed",
            ])
            .output()?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            warn!("Failed to create virtual interface: {}", err);
            return Err(anyhow::anyhow!(
                "Failed to create virtual interface: {}",
                err
            ));
        }

        self.created_virtual_interface = true;
        self.active_interface = Some(virt_iface.clone());

        // 启动接口
        let _ = Command::new("sudo")
            .args(["ip", "link", "set", &virt_iface, "up"])
            .output();

        tokio::time::sleep(Duration::from_millis(500)).await;

        // 启动 wpa_supplicant 在虚拟接口上
        // 创建临时配置文件
        let conf_content = format!(
            r#"ctrl_interface=/run/wpa_supplicant
update_config=1

network={{
    ssid="{}"
    psk="{}"
    key_mgmt=WPA-PSK
    scan_ssid=1
}}
"#,
            ssid, psk
        );

        let conf_path = format!("/tmp/wpa_supplicant_{}.conf", virt_iface);
        std::fs::write(&conf_path, conf_content)?;

        // 启动 wpa_supplicant
        let _ = Command::new("sudo")
            .args(["pkill", "-f", &format!("wpa_supplicant.*{}", virt_iface)])
            .output();

        let wpa_output = Command::new("sudo")
            .args([
                "wpa_supplicant",
                "-B",
                "-i",
                &virt_iface,
                "-c",
                &conf_path,
                "-D",
                "nl80211",
            ])
            .output()?;

        if !wpa_output.status.success() {
            let err = String::from_utf8_lossy(&wpa_output.stderr);
            warn!("Failed to start wpa_supplicant on {}: {}", virt_iface, err);
            return Err(anyhow::anyhow!("wpa_supplicant failed: {}", err));
        }

        // 等待连接
        for i in 1..=10 {
            tokio::time::sleep(Duration::from_secs(1)).await;

            let status = Command::new("wpa_cli")
                .args(["-i", &virt_iface, "status"])
                .output()?;

            let stdout = String::from_utf8_lossy(&status.stdout);
            if stdout.contains("wpa_state=COMPLETED") {
                debug!("Virtual interface connected on attempt {}", i);

                // 请求 DHCP
                let _ = Command::new("sudo")
                    .args(["dhclient", "-v", &virt_iface])
                    .output();

                tokio::time::sleep(Duration::from_secs(2)).await;

                return self.get_interface_ip(&virt_iface);
            }
        }

        Err(anyhow::anyhow!("Virtual interface connection timeout"))
    }

    /// 查找 P2P 设备接口
    fn find_p2p_device_interface(&self) -> anyhow::Result<String> {
        let output = Command::new("iw").args(["dev"]).output()?;

        let stdout = String::from_utf8_lossy(&output.stdout);

        // 查找 p2p-dev-wlan0 或类似接口
        for line in stdout.lines() {
            if line.trim().starts_with("Interface") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if let Some(iface) = parts.get(1)
                    && (iface.starts_with("p2p-dev-") || *iface == "p2p0")
                {
                    return Ok(iface.to_string());
                }
            }
        }

        // 尝试标准名称
        Ok(format!("p2p-dev-{}", self.config.main_interface))
    }

    /// 查找 P2P 组接口
    fn find_p2p_group_interface(&self) -> anyhow::Result<String> {
        let output = Command::new("iw").args(["dev"]).output()?;

        let stdout = String::from_utf8_lossy(&output.stdout);

        for line in stdout.lines() {
            if line.trim().starts_with("Interface") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if let Some(iface) = parts.get(1) {
                    // P2P 组接口格式: p2p-wlan0-0 或 p2p0-0
                    if iface.starts_with("p2p-")
                        && iface.contains('-')
                        && !iface.starts_with("p2p-dev-")
                    {
                        return Ok(iface.to_string());
                    }
                }
            }
        }

        Err(anyhow::anyhow!("No P2P group interface found"))
    }

    /// 使用 wpa_cli 连接
    async fn connect_wpa(
        &mut self,
        interface: &str,
        ssid: &str,
        psk: &str,
    ) -> anyhow::Result<String> {
        debug!("Connecting via wpa_cli on interface {}", interface);

        // 添加网络
        let output = Command::new("wpa_cli")
            .args(["-i", interface, "add_network"])
            .output()?;

        if !output.status.success() {
            return Err(anyhow::anyhow!("Failed to add network"));
        }

        let network_id = String::from_utf8_lossy(&output.stdout).trim().to_string();
        self.network_id = Some(network_id.clone());

        // 设置 SSID
        Command::new("wpa_cli")
            .args([
                "-i",
                interface,
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
                interface,
                "set_network",
                &network_id,
                "psk",
                &format!("\"{}\"", psk),
            ])
            .output()?;

        // 设置 scan_ssid=1 (对于 DIRECT- 前缀的热点很重要)
        Command::new("wpa_cli")
            .args([
                "-i",
                interface,
                "set_network",
                &network_id,
                "scan_ssid",
                "1",
            ])
            .output()?;

        // 启用网络
        Command::new("wpa_cli")
            .args(["-i", interface, "enable_network", &network_id])
            .output()?;

        // 选择网络
        Command::new("wpa_cli")
            .args(["-i", interface, "select_network", &network_id])
            .output()?;

        // 触发扫描
        let _ = Command::new("wpa_cli")
            .args(["-i", interface, "scan"])
            .output();

        // 等待连接
        for i in 1..=10 {
            tokio::time::sleep(Duration::from_secs(1)).await;

            let status = Command::new("wpa_cli")
                .args(["-i", interface, "status"])
                .output()?;

            let stdout = String::from_utf8_lossy(&status.stdout);
            if stdout.contains("wpa_state=COMPLETED") {
                info!("✅ WiFi connection successful on attempt {}", i);

                // 请求 DHCP (如果需要)
                if !stdout.contains("ip_address=") {
                    let _ = Command::new("sudo").args(["dhclient", interface]).output();
                    tokio::time::sleep(Duration::from_secs(2)).await;
                }

                return self.get_interface_ip(interface);
            }
        }

        Err(anyhow::anyhow!("wpa_cli connection timeout"))
    }

    /// 使用 nmcli 连接
    async fn connect_nmcli(
        &mut self,
        interface: &str,
        ssid: &str,
        psk: &str,
    ) -> anyhow::Result<String> {
        debug!("Connecting via nmcli on interface {}", interface);

        // 先删除旧连接
        let _ = Command::new("nmcli")
            .args(["connection", "delete", ssid])
            .output();

        // WiFi Direct 热点可能需要一些时间才能被发现
        let max_retries = 5;
        let mut last_error = String::new();

        for attempt in 1..=max_retries {
            debug!("WiFi connection attempt {}/{}", attempt, max_retries);

            // 触发 WiFi 扫描
            let _ = Command::new("nmcli")
                .args(["device", "wifi", "rescan", "ifname", interface])
                .output();

            tokio::time::sleep(Duration::from_secs(2)).await;

            // 尝试连接
            let output = Command::new("nmcli")
                .args([
                    "device", "wifi", "connect", ssid, "password", psk, "ifname", interface,
                ])
                .output()?;

            if output.status.success() {
                info!("✅ WiFi connection successful on attempt {}", attempt);
                tokio::time::sleep(Duration::from_secs(2)).await;
                return self.get_interface_ip(interface);
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

    /// 断开连接并清理
    pub async fn disconnect(&mut self) -> anyhow::Result<()> {
        if let Some(iface) = &self.active_interface {
            info!("Disconnecting from WiFi on interface {}", iface);

            // 断开 wpa_supplicant 连接
            if let Some(net_id) = &self.network_id {
                let _ = Command::new("wpa_cli")
                    .args(["-i", iface, "disable_network", net_id])
                    .output();
                let _ = Command::new("wpa_cli")
                    .args(["-i", iface, "remove_network", net_id])
                    .output();
            }

            let _ = Command::new("wpa_cli")
                .args(["-i", iface, "disconnect"])
                .output();

            // nmcli 断开
            let _ = Command::new("nmcli")
                .args(["device", "disconnect", iface])
                .output();
        }

        // 如果创建了虚拟接口，删除它
        if self.created_virtual_interface
            && let Some(iface) = &self.active_interface
        {
            info!("Removing virtual interface {}", iface);

            // 停止 wpa_supplicant
            let _ = Command::new("sudo")
                .args(["pkill", "-f", &format!("wpa_supplicant.*{}", iface)])
                .output();

            // 删除接口
            let _ = Command::new("sudo")
                .args(["iw", "dev", iface, "del"])
                .output();

            // 删除临时配置
            let conf_path = format!("/tmp/wpa_supplicant_{}.conf", iface);
            let _ = std::fs::remove_file(conf_path);
        }

        self.active_interface = None;
        self.created_virtual_interface = false;
        self.network_id = None;

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
        if let Some(iface) = &self.active_interface {
            let output = Command::new("wpa_cli")
                .args(["-i", iface, "status"])
                .output();

            if let Ok(output) = output {
                let stdout = String::from_utf8_lossy(&output.stdout);
                return stdout.contains("wpa_state=COMPLETED");
            }
        }
        false
    }

    /// 获取当前使用的接口名
    pub fn active_interface(&self) -> Option<&str> {
        self.active_interface.as_deref()
    }

    /// 原有 WiFi 是否保持连接
    pub fn is_dual_connected(&self) -> bool {
        self.created_virtual_interface
            || self.active_interface.as_deref() != Some(&self.config.main_interface)
    }
}

impl Drop for WiFiP2pReceiver {
    fn drop(&mut self) {
        // 尝试清理（同步版本）
        if self.created_virtual_interface
            && let Some(iface) = &self.active_interface
        {
            let _ = Command::new("sudo")
                .args(["pkill", "-f", &format!("wpa_supplicant.*{}", iface)])
                .output();
            let _ = Command::new("sudo")
                .args(["iw", "dev", iface, "del"])
                .output();
        }
    }
}
