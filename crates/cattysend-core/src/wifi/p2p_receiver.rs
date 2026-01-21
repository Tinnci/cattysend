//! WiFi P2P 接收端 (Client)
//!
//! 连接到发送端创建的 P2P 热点。
//!
//! # 连接策略（优先级从高到低）
//!
//! 1. **NmClient D-Bus**: 使用 NetworkManager 原生 D-Bus 接口
//! 2. **普通 WiFi 连接**: 退回到简单命令行（仅作为备用）
//!
//! # 注意事项
//!
//! - 连接后自动获取 DHCP 分配的 IP 地址
//! - 断开时会清理相关网络配置

use std::process::Command;
use std::sync::Arc;
use std::time::Duration;

use log::{debug, info, warn};
use tokio::sync::Mutex;

use crate::wifi::P2pInfo;
use crate::wifi::nm_dbus::NmClient;

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

/// 活动连接状态
struct ActiveConnection {
    connection_name: String,
    _connection_path: Option<String>,
    used_p2p_mode: bool,
}

/// WiFi P2P 接收端
pub struct WiFiP2pReceiver {
    config: P2pReceiverConfig,
    nm_client: Arc<Mutex<Option<NmClient>>>,
    active_connection: Arc<Mutex<Option<ActiveConnection>>>,
}

impl WiFiP2pReceiver {
    pub fn new(interface: &str) -> Self {
        Self {
            config: P2pReceiverConfig {
                main_interface: interface.to_string(),
                ..Default::default()
            },
            nm_client: Arc::new(Mutex::new(None)),
            active_connection: Arc::new(Mutex::new(None)),
        }
    }

    pub fn with_config(config: P2pReceiverConfig) -> Self {
        Self {
            config,
            nm_client: Arc::new(Mutex::new(None)),
            active_connection: Arc::new(Mutex::new(None)),
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

    /// 连接到 P2P 热点
    ///
    /// 返回分配的 IP 地址
    pub async fn connect(&mut self, info: &P2pInfo) -> anyhow::Result<String> {
        info!(
            "Connecting to WiFi Direct: ssid='{}', preserve_wifi={}",
            info.ssid, self.config.preserve_wifi
        );

        // 尝试使用 NmClient D-Bus
        match self.connect_nm_dbus(info).await {
            Ok(ip) => {
                info!("Connected via NetworkManager D-Bus, IP: {}", ip);
                return Ok(ip);
            }
            Err(e) => {
                warn!("NM D-Bus connection failed: {}, trying fallback", e);
            }
        }

        // 退回到简单的 nmcli 命令
        self.connect_nmcli_fallback(info).await
    }

    /// 使用 NmClient D-Bus 连接
    async fn connect_nm_dbus(&self, info: &P2pInfo) -> anyhow::Result<String> {
        self.ensure_nm_client().await?;

        let client_guard = self.nm_client.lock().await;
        let client = client_guard
            .as_ref()
            .ok_or_else(|| anyhow::anyhow!("NM client not initialized"))?;

        let conn_name = format!(
            "cattysend-wifi-{}",
            &info.ssid[..std::cmp::min(8, info.ssid.len())]
        );

        // 删除可能存在的旧连接
        let _ = client.delete_connection_by_name(&conn_name).await;

        // 触发 WiFi 扫描
        if let Some(device) = client
            .find_wifi_device(Some(&self.config.main_interface))
            .await?
        {
            let _ = client.request_wifi_scan(&device).await;
            tokio::time::sleep(Duration::from_secs(2)).await;
        }

        // 创建连接
        let conn_path = client
            .create_wifi_connection(&info.ssid, &info.psk, Some(&self.config.main_interface))
            .await?;

        // 查找设备
        let device = client
            .find_wifi_device(Some(&self.config.main_interface))
            .await?
            .ok_or_else(|| {
                anyhow::anyhow!("WiFi device {} not found", self.config.main_interface)
            })?;

        let active_conn = client
            .activate_connection(&conn_path.as_ref(), &device)
            .await?;

        // 等待 IP 分配
        let ip = client
            .wait_for_ip(&active_conn.as_ref(), Duration::from_secs(20))
            .await?;

        // 记录活动连接
        let mut active = self.active_connection.lock().await;
        *active = Some(ActiveConnection {
            connection_name: conn_name,
            _connection_path: Some(conn_path.to_string()),
            used_p2p_mode: false,
        });

        Ok(ip)
    }

    /// 使用 nmcli 命令行连接（备用）
    async fn connect_nmcli_fallback(&self, info: &P2pInfo) -> anyhow::Result<String> {
        debug!("Connecting via nmcli fallback");

        // 触发扫描
        let _ = Command::new("nmcli")
            .args([
                "device",
                "wifi",
                "rescan",
                "ifname",
                &self.config.main_interface,
            ])
            .output();

        tokio::time::sleep(Duration::from_secs(2)).await;

        // 尝试连接
        let output = Command::new("nmcli")
            .args([
                "device",
                "wifi",
                "connect",
                &info.ssid,
                "password",
                &info.psk,
                "ifname",
                &self.config.main_interface,
            ])
            .output()?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("nmcli connection failed: {}", err));
        }

        // 记录活动连接
        let mut active = self.active_connection.lock().await;
        *active = Some(ActiveConnection {
            connection_name: info.ssid.clone(),
            _connection_path: None,
            used_p2p_mode: false,
        });

        // 等待并获取 IP
        tokio::time::sleep(Duration::from_secs(2)).await;
        self.get_interface_ip(&self.config.main_interface)
    }

    /// 断开连接并清理
    pub async fn disconnect(&mut self) -> anyhow::Result<()> {
        info!("Disconnecting WiFi P2P connection");

        let active = self.active_connection.lock().await.take();

        if let Some(conn) = active {
            // 尝试使用 NM D-Bus 删除
            if let Ok(()) = self.ensure_nm_client().await {
                let client_guard = self.nm_client.lock().await;
                if let Some(client) = client_guard.as_ref() {
                    let _ = client
                        .delete_connection_by_name(&conn.connection_name)
                        .await;
                }
            }

            // 也尝试 nmcli 删除（备用）
            let _ = Command::new("nmcli")
                .args(["connection", "delete", &conn.connection_name])
                .output();
        }

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
    pub async fn is_connected(&self) -> bool {
        let active = self.active_connection.lock().await;
        active.is_some()
    }

    /// 获取当前使用的接口名
    pub fn active_interface(&self) -> &str {
        &self.config.main_interface
    }

    /// 原有 WiFi 是否保持连接
    pub async fn is_dual_connected(&self) -> bool {
        let active = self.active_connection.lock().await;
        active.as_ref().map(|a| a.used_p2p_mode).unwrap_or(false)
    }
}

impl Drop for WiFiP2pReceiver {
    fn drop(&mut self) {
        // 由于 Drop 是同步的，我们只能尝试使用 nmcli 清理
        if let Ok(active) = self.active_connection.try_lock()
            && let Some(conn) = active.as_ref()
        {
            let _ = Command::new("nmcli")
                .args(["connection", "delete", &conn.connection_name])
                .output();
        }
    }
}
