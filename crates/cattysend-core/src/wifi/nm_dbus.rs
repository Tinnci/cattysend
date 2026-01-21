//! NetworkManager D-Bus 客户端
//!
//! 通过 D-Bus 直接与 NetworkManager 守护进程通信，替代 `nmcli` 子进程调用。
//!
//! # 优势
//!
//! - **类型安全**: Rust 类型系统保证接口正确性
//! - **异步**: 原生 async/await 支持
//! - **稳定**: 无需解析 CLI 输出，避免格式变化导致的兼容问题
//! - **高效**: 无需 fork 子进程
//!
//! # 使用
//!
//! ```ignore
//! use cattysend_core::wifi::nm_dbus::NmClient;
//!
//! let client = NmClient::new().await?;
//!
//! // 获取设备列表
//! let devices = client.get_wifi_devices().await?;
//!
//! // 创建热点
//! let conn = client.create_hotspot("DIRECT-abc", "password123", "a").await?;
//!
//! // 激活连接
//! client.activate_connection(&conn, &device).await?;
//! ```

use std::collections::HashMap;
use std::time::Duration;

use std::ops::Deref;

use anyhow::{Context, Result};
use log::{debug, info};
use zbus::Connection;
use zbus::proxy;
use zbus::zvariant::{ObjectPath, OwnedObjectPath, OwnedValue, Value};

// NetworkManager D-Bus 路径由 zbus proxy 宏自动处理

/// NetworkManager 主接口代理
#[proxy(
    interface = "org.freedesktop.NetworkManager",
    default_service = "org.freedesktop.NetworkManager",
    default_path = "/org/freedesktop/NetworkManager"
)]
trait NetworkManager {
    /// 获取所有网络设备
    fn get_devices(&self) -> zbus::Result<Vec<OwnedObjectPath>>;

    /// 激活连接
    fn activate_connection(
        &self,
        connection: &ObjectPath<'_>,
        device: &ObjectPath<'_>,
        specific_object: &ObjectPath<'_>,
    ) -> zbus::Result<OwnedObjectPath>;

    /// 停用连接
    fn deactivate_connection(&self, active_connection: &ObjectPath<'_>) -> zbus::Result<()>;

    /// 添加并激活连接 (简化版)
    fn add_and_activate_connection(
        &self,
        connection: HashMap<&str, HashMap<&str, Value<'_>>>,
        device: &ObjectPath<'_>,
        specific_object: &ObjectPath<'_>,
    ) -> zbus::Result<(OwnedObjectPath, OwnedObjectPath)>;

    /// NetworkManager 版本
    #[zbus(property)]
    fn version(&self) -> zbus::Result<String>;

    /// 当前连接状态
    #[zbus(property)]
    fn state(&self) -> zbus::Result<u32>;

    /// 活动连接列表
    #[zbus(property)]
    fn active_connections(&self) -> zbus::Result<Vec<OwnedObjectPath>>;
}

/// NetworkManager.Settings 接口代理
#[proxy(
    interface = "org.freedesktop.NetworkManager.Settings",
    default_service = "org.freedesktop.NetworkManager",
    default_path = "/org/freedesktop/NetworkManager/Settings"
)]
trait NmSettings {
    /// 添加新连接
    fn add_connection(
        &self,
        connection: HashMap<&str, HashMap<&str, Value<'_>>>,
    ) -> zbus::Result<OwnedObjectPath>;

    /// 列出所有连接
    fn list_connections(&self) -> zbus::Result<Vec<OwnedObjectPath>>;
}

/// NetworkManager.Settings.Connection 接口代理
#[proxy(
    interface = "org.freedesktop.NetworkManager.Settings.Connection",
    default_service = "org.freedesktop.NetworkManager"
)]
trait NmConnection {
    /// 获取连接设置
    fn get_settings(&self) -> zbus::Result<HashMap<String, HashMap<String, OwnedValue>>>;

    /// 删除连接
    fn delete(&self) -> zbus::Result<()>;
}

/// NetworkManager.Device 接口代理
#[proxy(
    interface = "org.freedesktop.NetworkManager.Device",
    default_service = "org.freedesktop.NetworkManager"
)]
trait NmDevice {
    /// 设备接口名 (如 wlan0)
    #[zbus(property)]
    fn interface(&self) -> zbus::Result<String>;

    /// 设备类型 (2=WiFi, 30=WiFi-P2P)
    #[zbus(property)]
    fn device_type(&self) -> zbus::Result<u32>;

    /// 设备状态
    #[zbus(property)]
    fn state(&self) -> zbus::Result<u32>;

    /// 设备是否已管理
    #[zbus(property)]
    fn managed(&self) -> zbus::Result<bool>;

    /// 当前活动连接
    #[zbus(property)]
    fn active_connection(&self) -> zbus::Result<OwnedObjectPath>;

    /// 硬件地址 (MAC)
    #[zbus(property)]
    fn hw_address(&self) -> zbus::Result<String>;

    /// 断开设备连接
    fn disconnect(&self) -> zbus::Result<()>;
}

/// NetworkManager.Device.Wireless 接口代理
#[proxy(
    interface = "org.freedesktop.NetworkManager.Device.Wireless",
    default_service = "org.freedesktop.NetworkManager"
)]
trait NmDeviceWireless {
    /// 触发 WiFi 扫描
    fn request_scan(&self, options: HashMap<&str, Value<'_>>) -> zbus::Result<()>;

    /// 获取所有接入点
    fn get_all_access_points(&self) -> zbus::Result<Vec<OwnedObjectPath>>;
}

/// NetworkManager.Connection.Active 接口代理
#[proxy(
    interface = "org.freedesktop.NetworkManager.Connection.Active",
    default_service = "org.freedesktop.NetworkManager"
)]
trait NmActiveConnection {
    /// 连接 ID
    #[zbus(property)]
    fn id(&self) -> zbus::Result<String>;

    /// 连接状态
    #[zbus(property)]
    fn state(&self) -> zbus::Result<u32>;

    /// IP4 配置对象路径
    #[zbus(property)]
    fn ip4_config(&self) -> zbus::Result<OwnedObjectPath>;
}

/// NetworkManager.IP4Config 接口代理
#[proxy(
    interface = "org.freedesktop.NetworkManager.IP4Config",
    default_service = "org.freedesktop.NetworkManager"
)]
trait NmIp4Config {
    /// 地址数据 (新格式)
    #[zbus(property)]
    fn address_data(&self) -> zbus::Result<Vec<HashMap<String, OwnedValue>>>;
}

// ============================================================================
// 高层封装
// ============================================================================

/// 设备类型常量
pub mod device_type {
    pub const WIFI: u32 = 2;
    pub const WIFI_P2P: u32 = 30;
}

/// 设备状态常量
pub mod device_state {
    pub const DISCONNECTED: u32 = 30;
    pub const ACTIVATED: u32 = 100;
}

/// 连接状态常量
pub mod active_connection_state {
    pub const UNKNOWN: u32 = 0;
    pub const ACTIVATING: u32 = 1;
    pub const ACTIVATED: u32 = 2;
    pub const DEACTIVATING: u32 = 3;
    pub const DEACTIVATED: u32 = 4;

    pub fn name(state: u32) -> &'static str {
        match state {
            UNKNOWN => "UNKNOWN",
            ACTIVATING => "ACTIVATING",
            ACTIVATED => "ACTIVATED",
            DEACTIVATING => "DEACTIVATING",
            DEACTIVATED => "DEACTIVATED",
            _ => "INVALID",
        }
    }
}

/// WiFi 设备信息
#[derive(Debug, Clone)]
pub struct WifiDevice {
    /// D-Bus 对象路径
    pub path: OwnedObjectPath,
    /// 接口名 (如 wlan0)
    pub interface: String,
    /// 设备类型
    pub device_type: u32,
    /// MAC 地址
    pub hw_address: String,
    /// 是否已激活
    pub is_active: bool,
}

/// NetworkManager D-Bus 客户端
pub struct NmClient {
    connection: Connection,
}

impl NmClient {
    /// 创建新的 NM D-Bus 客户端
    pub async fn new() -> Result<Self> {
        let connection = Connection::system()
            .await
            .context("Failed to connect to system D-Bus")?;

        // 验证 NetworkManager 是否可用
        let nm = NetworkManagerProxy::new(&connection).await?;
        let version = nm.version().await?;
        info!("Connected to NetworkManager {}", version);

        Ok(Self { connection })
    }

    /// 获取 NetworkManager 版本
    pub async fn version(&self) -> Result<String> {
        let nm = NetworkManagerProxy::new(&self.connection).await?;
        nm.version().await.context("Failed to get NM version")
    }

    /// 获取所有 WiFi 设备
    pub async fn get_wifi_devices(&self) -> Result<Vec<WifiDevice>> {
        let nm = NetworkManagerProxy::new(&self.connection).await?;
        let device_paths = nm.get_devices().await?;

        let mut wifi_devices = Vec::new();

        for path in device_paths {
            let device = NmDeviceProxy::builder(&self.connection)
                .path(&path)?
                .build()
                .await?;

            let dev_type = device.device_type().await.unwrap_or(0);

            // 只收集 WiFi 和 WiFi-P2P 设备
            if dev_type == device_type::WIFI || dev_type == device_type::WIFI_P2P {
                let interface = device.interface().await.unwrap_or_default();
                let hw_address = device.hw_address().await.unwrap_or_default();
                let state = device.state().await.unwrap_or(0);

                wifi_devices.push(WifiDevice {
                    path,
                    interface,
                    device_type: dev_type,
                    hw_address,
                    is_active: state == device_state::ACTIVATED,
                });
            }
        }

        Ok(wifi_devices)
    }

    /// 查找 P2P 设备
    pub async fn find_p2p_device(&self) -> Result<Option<WifiDevice>> {
        let devices = self.get_wifi_devices().await?;
        Ok(devices
            .into_iter()
            .find(|d| d.device_type == device_type::WIFI_P2P))
    }

    /// 查找主 WiFi 设备
    pub async fn find_wifi_device(&self, interface: Option<&str>) -> Result<Option<WifiDevice>> {
        let devices = self.get_wifi_devices().await?;

        if let Some(iface) = interface {
            Ok(devices.into_iter().find(|d| d.interface == iface))
        } else {
            Ok(devices
                .into_iter()
                .find(|d| d.device_type == device_type::WIFI))
        }
    }

    /// 创建 WiFi 热点连接配置
    pub async fn create_hotspot(
        &self,
        ssid: &str,
        password: &str,
        band: &str,
        interface: &str,
    ) -> Result<OwnedObjectPath> {
        let settings = NmSettingsProxy::new(&self.connection).await?;

        // 构建连接配置
        let connection_settings = self.build_hotspot_settings(ssid, password, band, interface);

        let conn_path = settings
            .add_connection(connection_settings)
            .await
            .context("Failed to create hotspot connection")?;

        info!("Created hotspot connection: {:?}", conn_path);
        Ok(conn_path)
    }

    /// 构建热点连接设置
    fn build_hotspot_settings<'a>(
        &self,
        ssid: &'a str,
        password: &'a str,
        band: &'a str,
        interface: &'a str,
    ) -> HashMap<&'a str, HashMap<&'a str, Value<'a>>> {
        let mut settings: HashMap<&str, HashMap<&str, Value>> = HashMap::new();

        // connection 部分
        let mut connection: HashMap<&str, Value> = HashMap::new();
        let conn_id = format!(
            "cattysend-hotspot-{}",
            &ssid[..std::cmp::min(8, ssid.len())]
        );
        connection.insert("id", Value::Str(conn_id.into()));
        connection.insert("type", Value::Str("802-11-wireless".into()));
        connection.insert("autoconnect", Value::Bool(false));
        connection.insert("interface-name", Value::Str(interface.into()));
        settings.insert("connection", connection);

        // 802-11-wireless 部分
        let mut wireless: HashMap<&str, Value> = HashMap::new();
        wireless.insert("ssid", Value::Array(ssid.as_bytes().into()));
        wireless.insert("mode", Value::Str("ap".into()));
        wireless.insert("band", Value::Str(band.into()));
        settings.insert("802-11-wireless", wireless);

        // 802-11-wireless-security 部分
        let mut wireless_security: HashMap<&str, Value> = HashMap::new();
        wireless_security.insert("key-mgmt", Value::Str("wpa-psk".into()));
        wireless_security.insert("psk", Value::Str(password.into()));
        settings.insert("802-11-wireless-security", wireless_security);

        // ipv4 部分 (共享模式 - 自动 DHCP)
        let mut ipv4: HashMap<&str, Value> = HashMap::new();
        ipv4.insert("method", Value::Str("shared".into()));
        settings.insert("ipv4", ipv4);

        // ipv6 部分
        let mut ipv6: HashMap<&str, Value> = HashMap::new();
        ipv6.insert("method", Value::Str("ignore".into()));
        settings.insert("ipv6", ipv6);

        settings
    }

    /// 创建 WiFi 客户端连接配置
    pub async fn create_wifi_connection(
        &self,
        ssid: &str,
        password: &str,
        interface: Option<&str>,
    ) -> Result<OwnedObjectPath> {
        let settings = NmSettingsProxy::new(&self.connection).await?;

        let connection_settings = self.build_wifi_client_settings(ssid, password, interface);

        let conn_path = settings
            .add_connection(connection_settings)
            .await
            .context("Failed to create WiFi connection")?;

        info!("Created WiFi connection: {:?}", conn_path);
        Ok(conn_path)
    }

    /// 构建 WiFi 客户端连接设置
    fn build_wifi_client_settings<'a>(
        &self,
        ssid: &'a str,
        password: &'a str,
        interface: Option<&'a str>,
    ) -> HashMap<&'a str, HashMap<&'a str, Value<'a>>> {
        let mut settings: HashMap<&str, HashMap<&str, Value>> = HashMap::new();

        // connection 部分
        let mut connection: HashMap<&str, Value> = HashMap::new();
        let conn_id = format!("cattysend-wifi-{}", &ssid[..std::cmp::min(8, ssid.len())]);
        connection.insert("id", Value::Str(conn_id.into()));
        connection.insert("type", Value::Str("802-11-wireless".into()));
        connection.insert("autoconnect", Value::Bool(false));
        if let Some(iface) = interface {
            connection.insert("interface-name", Value::Str(iface.into()));
        }
        settings.insert("connection", connection);

        // 802-11-wireless 部分
        let mut wireless: HashMap<&str, Value> = HashMap::new();
        wireless.insert("ssid", Value::Array(ssid.as_bytes().into()));
        wireless.insert("mode", Value::Str("infrastructure".into()));
        settings.insert("802-11-wireless", wireless);

        // 802-11-wireless-security 部分
        let mut wireless_security: HashMap<&str, Value> = HashMap::new();
        wireless_security.insert("key-mgmt", Value::Str("wpa-psk".into()));
        wireless_security.insert("psk", Value::Str(password.into()));
        settings.insert("802-11-wireless-security", wireless_security);

        // ipv4 部分
        let mut ipv4: HashMap<&str, Value> = HashMap::new();
        ipv4.insert("method", Value::Str("auto".into()));
        settings.insert("ipv4", ipv4);

        // ipv6 部分
        let mut ipv6: HashMap<&str, Value> = HashMap::new();
        ipv6.insert("method", Value::Str("auto".into()));
        settings.insert("ipv6", ipv6);

        settings
    }

    /// 激活连接
    pub async fn activate_connection(
        &self,
        connection_path: &ObjectPath<'_>,
        device: &WifiDevice,
    ) -> Result<OwnedObjectPath> {
        let nm = NetworkManagerProxy::new(&self.connection).await?;

        let device_path = device.path.as_ref();
        let active_conn = nm
            .activate_connection(
                connection_path,
                &device_path,
                &ObjectPath::from_static_str_unchecked("/"),
            )
            .await
            .context("Failed to activate connection")?;

        info!("Activated connection: {:?}", active_conn);
        Ok(active_conn)
    }

    /// 停用连接
    pub async fn deactivate_connection(&self, active_connection: &ObjectPath<'_>) -> Result<()> {
        let nm = NetworkManagerProxy::new(&self.connection).await?;
        nm.deactivate_connection(active_connection).await?;
        Ok(())
    }

    /// 删除连接配置
    pub async fn delete_connection(&self, connection_path: &ObjectPath<'_>) -> Result<()> {
        let conn = NmConnectionProxy::builder(&self.connection)
            .path(connection_path)?
            .build()
            .await?;

        conn.delete().await.context("Failed to delete connection")?;
        debug!("Deleted connection: {:?}", connection_path);
        Ok(())
    }

    /// 删除连接（通过名称）
    pub async fn delete_connection_by_name(&self, name: &str) -> Result<bool> {
        let settings = NmSettingsProxy::new(&self.connection).await?;
        let connections = settings.list_connections().await?;

        for conn_path in connections {
            let conn = NmConnectionProxy::builder(&self.connection)
                .path(&conn_path)?
                .build()
                .await?;

            if let Ok(conn_settings) = conn.get_settings().await
                && let Some(connection_section) = conn_settings.get("connection")
                && let Some(id_value) = connection_section.get("id")
                && let Value::Str(id_str) = id_value.deref()
                && id_str.as_str() == name
            {
                conn.delete().await?;
                debug!("Deleted connection by name: {}", name);
                return Ok(true);
            }
        }

        Ok(false)
    }

    /// 触发 WiFi 扫描
    pub async fn request_wifi_scan(&self, device: &WifiDevice) -> Result<()> {
        let wireless = NmDeviceWirelessProxy::builder(&self.connection)
            .path(&device.path)?
            .build()
            .await?;

        wireless
            .request_scan(HashMap::new())
            .await
            .context("Failed to request WiFi scan")?;

        Ok(())
    }

    /// 等待连接激活（不等待IP配置，适用于热点模式）
    pub async fn wait_for_activation(
        &self,
        active_connection: &ObjectPath<'_>,
        timeout: Duration,
    ) -> Result<()> {
        let start = std::time::Instant::now();
        let mut last_state = 0u32;

        loop {
            if start.elapsed() > timeout {
                return Err(anyhow::anyhow!(
                    "Timeout waiting for connection activation (last state: {})",
                    active_connection_state::name(last_state)
                ));
            }

            let active = NmActiveConnectionProxy::builder(&self.connection)
                .path(active_connection)?
                .build()
                .await?;

            let state = active.state().await.unwrap_or(0);

            // 状态变化时记录日志
            if state != last_state {
                debug!(
                    "Connection state changed: {} -> {}",
                    active_connection_state::name(last_state),
                    active_connection_state::name(state)
                );
                last_state = state;
            }

            match state {
                active_connection_state::ACTIVATED => {
                    info!("Connection activated successfully");
                    return Ok(());
                }
                active_connection_state::DEACTIVATED | active_connection_state::DEACTIVATING => {
                    return Err(anyhow::anyhow!(
                        "Connection failed to activate (state: {})",
                        active_connection_state::name(state)
                    ));
                }
                _ => {
                    // UNKNOWN, ACTIVATING - 继续等待
                }
            }

            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }

    /// 等待连接激活并获取 IP
    pub async fn wait_for_ip(
        &self,
        active_connection: &ObjectPath<'_>,
        timeout: Duration,
    ) -> Result<String> {
        let start = std::time::Instant::now();

        loop {
            if start.elapsed() > timeout {
                return Err(anyhow::anyhow!("Timeout waiting for IP address"));
            }

            let active = NmActiveConnectionProxy::builder(&self.connection)
                .path(active_connection)?
                .build()
                .await?;

            let state = active.state().await.unwrap_or(0);

            if state == active_connection_state::ACTIVATED {
                // 获取 IP4 配置
                if let Ok(ip4_path) = active.ip4_config().await
                    && ip4_path.as_str() != "/"
                {
                    let ip4 = NmIp4ConfigProxy::builder(&self.connection)
                        .path(&ip4_path)?
                        .build()
                        .await?;

                    if let Ok(addresses) = ip4.address_data().await {
                        for addr in addresses {
                            if let Some(address_value) = addr.get("address")
                                && let Value::Str(ip_str) = address_value.deref()
                            {
                                return Ok(ip_str.to_string());
                            }
                        }
                    }
                }
            }

            tokio::time::sleep(Duration::from_millis(500)).await;
        }
    }

    /// 断开设备连接
    pub async fn disconnect_device(&self, device: &WifiDevice) -> Result<()> {
        let dev = NmDeviceProxy::builder(&self.connection)
            .path(&device.path)?
            .build()
            .await?;

        dev.disconnect()
            .await
            .context("Failed to disconnect device")?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // 注意: 这些测试需要系统 D-Bus 和 NetworkManager 运行
    // 在 CI 环境中可能需要跳过

    #[tokio::test]
    #[ignore = "requires system D-Bus and NetworkManager"]
    async fn test_nm_client_version() {
        let client = NmClient::new().await.unwrap();
        let version = client.version().await.unwrap();
        assert!(!version.is_empty());
        println!("NetworkManager version: {}", version);
    }

    #[tokio::test]
    #[ignore = "requires system D-Bus and NetworkManager"]
    async fn test_get_wifi_devices() {
        let client = NmClient::new().await.unwrap();
        let devices = client.get_wifi_devices().await.unwrap();

        for device in &devices {
            println!(
                "Device: {} (type={}, mac={})",
                device.interface, device.device_type, device.hw_address
            );
        }
    }
}
