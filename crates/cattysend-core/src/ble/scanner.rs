//! BLE Scanner - 发现 CatShare 兼容设备
//!
//! 使用 bluer (BlueZ D-Bus) 扫描广播 CatShare 服务 UUID 的设备。
//!
//! # 设备识别
//!
//! 扫描器通过以下方式识别 CatShare 设备：
//! - Service UUID: `00003331-0000-1000-8000-008123456789`
//! - Scan Response (0xFFFF): 包含设备名称和 sender ID
//!
//! # 品牌 ID
//!
//! 广播数据中包含品牌 ID：
//! - 0x01: 小米
//! - 0x02: OPPO/一加
//! - 0x03: vivo
//! - 0xFF: 未知/Linux

use log::{debug, info, trace};

use crate::ble::SERVICE_UUID;
use bluer::AdapterEvent;
use futures_util::StreamExt;
use std::time::Duration;
use tokio::time;

#[derive(Debug, Clone)]
pub struct DiscoveredDevice {
    pub name: String,
    pub address: String,
    pub sender_id: String,
    pub brand_id: Option<u16>,
    pub rssi: Option<i16>,
    pub supports_5ghz: bool,
}

pub struct BleScanner {
    session: bluer::Session,
}

impl BleScanner {
    pub async fn new() -> anyhow::Result<Self> {
        let session = bluer::Session::new().await?;
        Ok(Self { session })
    }

    pub async fn scan(&self, timeout: Duration) -> anyhow::Result<Vec<DiscoveredDevice>> {
        debug!("Getting default adapter for scan");
        let adapter = self.session.default_adapter().await?;

        let adapter_name = adapter.name().to_string();
        debug!("Powering on adapter '{}' for scan", adapter_name);
        adapter.set_powered(true).await?;

        let filter = bluer::DiscoveryFilter {
            ..Default::default()
        };
        adapter.set_discovery_filter(filter).await?;

        let mut discoverer = adapter.discover_devices().await?;
        let mut discovered_map = std::collections::HashMap::new();

        info!("Starting BLE scan for {} seconds", timeout.as_secs());

        let timeout_fut = time::sleep(timeout);
        tokio::pin!(timeout_fut);

        loop {
            tokio::select! {
                _ = &mut timeout_fut => {
                    break;
                }
                event = discoverer.next() => {
                    match event {
                        Some(AdapterEvent::DeviceAdded(addr)) => {
                            trace!("Device discovered: {}", addr);
                            let device = adapter.device(addr)?;
                            if let Ok(Some(dev)) = self.parse_device(&device).await {
                                debug!(
                                    "CatShare-compatible device found: addr={}, name='{}', sender_id={}",
                                    addr, dev.name, dev.sender_id
                                );
                                discovered_map.insert(addr, dev);
                            }
                        }
                        None => break,
                        _ => {}
                    }
                }
            }
        }

        // 也检查已经发现的设备
        let device_addrs = adapter.device_addresses().await?;
        for addr in device_addrs {
            if !discovered_map.contains_key(&addr) {
                let device = adapter.device(addr)?;
                if let Ok(Some(dev)) = self.parse_device(&device).await {
                    discovered_map.insert(addr, dev);
                }
            }
        }

        Ok(discovered_map.into_values().collect())
    }

    async fn parse_device(
        &self,
        device: &bluer::Device,
    ) -> anyhow::Result<Option<DiscoveredDevice>> {
        let uuids = device.uuids().await?.unwrap_or_default();
        let service_data = device.service_data().await?.unwrap_or_default();

        let scan_resp_uuid = uuid::Uuid::from_u128(0x0000ffff_0000_1000_8000_00805f9b34fb);
        let has_base_service = uuids.contains(&SERVICE_UUID);
        let identity_data = service_data.get(&scan_resp_uuid);

        if has_base_service || identity_data.is_some() {
            let mut name = device
                .name()
                .await?
                .unwrap_or_else(|| "Unknown".to_string());
            let rssi = device.rssi().await?;
            let mut brand_id = None;
            let mut sender_id = "0000".to_string();
            let mut supports_5ghz = false;

            if let Some(data) = identity_data {
                if data.len() >= 26 {
                    let id_raw = ((data[8] as u16) << 8) | (data[9] as u16);
                    sender_id = format!("{:04x}", id_raw);

                    let mut name_bytes = Vec::new();
                    for i in 10..26 {
                        if data[i] != 0 {
                            name_bytes.push(data[i]);
                        } else {
                            break;
                        }
                    }
                    if let Ok(n) = String::from_utf8(name_bytes) {
                        name = n;
                    }
                }
            }

            for (uuid, data) in &service_data {
                if data.len() == 6 {
                    let bytes = uuid.as_bytes();
                    if bytes[0] == 0 && bytes[1] == 0 && bytes[4] == 0 && bytes[5] == 0 {
                        supports_5ghz = bytes[2] == 1;
                        brand_id = Some(bytes[3] as u16);
                    }
                }
            }

            if brand_id.is_none() {
                let mfdata = device.manufacturer_data().await?.unwrap_or_default();
                brand_id = mfdata.keys().next().map(|&k| k as u16);
            }

            return Ok(Some(DiscoveredDevice {
                name,
                address: device.address().to_string(),
                sender_id,
                brand_id,
                rssi,
                supports_5ghz,
            }));
        }
        Ok(None)
    }
}
