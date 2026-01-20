//! Legacy BLE Advertising via BlueZ MGMT API
//!
//! 直接使用 BlueZ Management Socket API 发送 Legacy Advertising，
//! 绕过 bluer 的 D-Bus 接口，获得对广播参数的完全控制权。
//!
//! # Legacy vs Extended Advertising
//!
//! - **Legacy**: 数据限制 31 字节，兼容所有 BLE 设备
//! - **Extended**: 数据可达 255 字节，但部分旧设备不支持
//!
//! 通过不设置 `SecondaryChannelWithLe*` 标志，强制使用 Legacy 模式。
//!
//! # 权限要求
//!
//! 需要 `CAP_NET_ADMIN` 权限：
//! ```bash
//! sudo setcap 'cap_net_admin+eip' your_binary
//! ```

use btmgmt::Client;
use btmgmt::command::{AddAdvertising, RemoveAdvertising};
use btmgmt::packet::{AdvDataScanResp, AdvertiseInstance, AdvertisingFlag};
use log::{debug, error, info, trace};
use std::sync::Arc;
use tokio::sync::Mutex;

/// Legacy 广播配置
#[derive(Debug, Clone)]
pub struct LegacyAdvConfig {
    /// 控制器索引 (通常为 0 = hci0)
    pub controller_index: u16,
    /// 广播实例 ID (1-255)
    pub instance: u8,
    /// 是否可连接
    pub connectable: bool,
    /// 是否可发现
    pub discoverable: bool,
    /// 广播数据 (最大 31 字节)
    pub adv_data: Vec<u8>,
    /// 扫描响应数据 (最大 31 字节)
    pub scan_rsp_data: Vec<u8>,
    /// 持续时间 (秒, 0=无限)
    pub duration: u16,
    /// 超时 (秒, 0=无限)
    pub timeout: u16,
}

impl Default for LegacyAdvConfig {
    fn default() -> Self {
        Self {
            controller_index: 0,
            instance: 1,
            connectable: true,
            discoverable: true,
            adv_data: Vec::new(),
            scan_rsp_data: Vec::new(),
            duration: 0,
            timeout: 0,
        }
    }
}

impl LegacyAdvConfig {
    /// 创建 CatShare 兼容的广播配置
    ///
    /// # 参数
    /// - `service_uuid`: 16-bit 服务 UUID (e.g., 0x3331)
    /// - `ident_uuid`: 16-bit 身份/能力 UUID (e.g., 0x011e for 5GHz + Xiaomi)
    /// - `ident_data`: 身份数据 (最多 6 字节)
    /// - `device_name`: 设备名称
    /// - `sender_id`: 发送者 ID (2 字节)
    pub fn catshare_compatible(
        service_uuid: u16,
        ident_uuid: u16,
        ident_data: &[u8],
        device_name: &str,
        _sender_id: [u8; 2],
    ) -> Self {
        // 构建广播数据 (Adv Data)
        // 格式: [len, type, data...]
        let mut adv_data = Vec::with_capacity(31);

        // 1. Flags: LE General Discoverable + BR/EDR Not Supported
        adv_data.push(2); // length
        adv_data.push(0x01); // type: Flags
        adv_data.push(0x06); // LE General Discoverable + BR/EDR Not Supported

        // 2. Complete 16-bit Service UUIDs
        adv_data.push(3); // length
        adv_data.push(0x03); // type: Complete List of 16-bit Service UUIDs
        adv_data.push((service_uuid & 0xFF) as u8); // UUID low byte
        adv_data.push((service_uuid >> 8) as u8); // UUID high byte

        // 3. Service Data (身份/能力)
        let ident_data_len = ident_data.len().min(6);
        adv_data.push((3 + ident_data_len) as u8); // length = UUID (2) + type (1) + data
        adv_data.push(0x16); // type: Service Data - 16 bit UUID
        adv_data.push((ident_uuid & 0xFF) as u8);
        adv_data.push((ident_uuid >> 8) as u8);
        adv_data.extend_from_slice(&ident_data[..ident_data_len]);

        // 构建扫描响应数据 (Scan Response)
        let mut scan_rsp = Vec::with_capacity(31);

        // 1. Complete Local Name (如果空间足够)
        let name_bytes = device_name.as_bytes();
        let max_name_len = 29; // 31 - 2 (length + type)
        let name_len = name_bytes.len().min(max_name_len);
        if name_len > 0 {
            scan_rsp.push((1 + name_len) as u8); // length
            scan_rsp.push(0x09); // type: Complete Local Name
            scan_rsp.extend_from_slice(&name_bytes[..name_len]);
        }

        debug!(
            "Legacy adv config: adv_data={} bytes, scan_rsp={} bytes",
            adv_data.len(),
            scan_rsp.len()
        );
        trace!("Adv data: {:02x?}", adv_data);
        trace!("Scan rsp: {:02x?}", scan_rsp);

        Self {
            adv_data,
            scan_rsp_data: scan_rsp,
            connectable: true,
            discoverable: true,
            ..Default::default()
        }
    }
}

/// MGMT Legacy 广播器
pub struct MgmtLegacyAdvertiser {
    client: Arc<Mutex<Client>>,
    config: LegacyAdvConfig,
    active: bool,
}

impl MgmtLegacyAdvertiser {
    /// 创建新的 MGMT 广播器
    pub async fn new(config: LegacyAdvConfig) -> anyhow::Result<Self> {
        debug!("Opening MGMT socket...");
        let client = Client::open()?;
        info!("MGMT socket opened successfully");

        Ok(Self {
            client: Arc::new(Mutex::new(client)),
            config,
            active: false,
        })
    }

    /// 启动 Legacy 广播
    pub async fn start(&mut self) -> anyhow::Result<()> {
        if self.active {
            debug!("Advertising already active, stopping first...");
            self.stop().await?;
        }

        let client = self.client.lock().await;

        // 构建 flags - **不设置 SecondaryChannel* 标志以强制 Legacy 模式**
        let mut flags = AdvertisingFlag::empty();
        if self.config.connectable {
            flags |= AdvertisingFlag::SwitchIntoConnectableMode;
        }
        if self.config.discoverable {
            flags |= AdvertisingFlag::AdvertiseAsDiscoverable;
        }
        // 添加 Flags 字段到广播数据
        flags |= AdvertisingFlag::AddFlagsFieldToAdvData;

        debug!(
            "Starting Legacy advertising with flags: {:?} (bits: 0x{:08x})",
            flags,
            flags.bits()
        );

        // 构建广播数据
        let adv_data_scan_resp = AdvDataScanResp::new(
            self.config.adv_data.clone(),
            self.config.scan_rsp_data.clone(),
        );

        // 发送 AddAdvertising 命令
        let cmd = AddAdvertising::new(
            AdvertiseInstance::new(self.config.instance),
            flags,
            self.config.duration,
            self.config.timeout,
            adv_data_scan_resp,
        );

        let index = Some(self.config.controller_index);
        let reply = client.call(index, cmd).await?;

        info!(
            "Legacy advertising started: instance={}, reply={:?}",
            self.config.instance, reply
        );

        drop(client);
        self.active = true;

        Ok(())
    }

    /// 停止广播
    pub async fn stop(&mut self) -> anyhow::Result<()> {
        if !self.active {
            return Ok(());
        }

        let client = self.client.lock().await;

        let cmd = RemoveAdvertising::new(AdvertiseInstance::new(self.config.instance));
        let index = Some(self.config.controller_index);

        match client.call(index, cmd).await {
            Ok(_) => {
                info!("Advertising instance {} removed", self.config.instance);
            }
            Err(e) => {
                error!("Failed to remove advertising: {}", e);
            }
        }

        drop(client);
        self.active = false;

        Ok(())
    }

    /// 检查广播是否活动
    pub fn is_active(&self) -> bool {
        self.active
    }
}

impl Drop for MgmtLegacyAdvertiser {
    fn drop(&mut self) {
        if self.active {
            // 注意: 这里无法 async，广播实例会在 MGMT socket 关闭时自动清理
            debug!("MgmtLegacyAdvertiser dropped, advertising will be cleaned up");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_catshare_config() {
        let config = LegacyAdvConfig::catshare_compatible(
            0x3331,         // service UUID
            0x011e,         // ident UUID (5GHz + Xiaomi)
            &[0x12, 0x75],  // ident data (sender ID)
            "thinkpad-z13", // device name
            [0x12, 0x75],   // sender ID
        );

        // 验证广播数据不超过 31 字节
        assert!(
            config.adv_data.len() <= 31,
            "adv_data too long: {}",
            config.adv_data.len()
        );
        assert!(
            config.scan_rsp_data.len() <= 31,
            "scan_rsp too long: {}",
            config.scan_rsp_data.len()
        );

        // 验证 Flags 存在
        assert_eq!(config.adv_data[0], 2); // Flags length
        assert_eq!(config.adv_data[1], 0x01); // Flags type
        assert_eq!(config.adv_data[2], 0x06); // LE General Discoverable + BR/EDR Not Supported
    }
}
