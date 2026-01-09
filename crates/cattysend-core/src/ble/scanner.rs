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
use std::sync::Arc;
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

/// 扫描回调接口，用于实时汇报发现的设备
#[async_trait::async_trait]
pub trait ScanCallback: Send + Sync {
    async fn on_device_found(&self, device: DiscoveredDevice);
}

pub struct BleScanner {
    session: bluer::Session,
}

impl BleScanner {
    pub async fn new() -> anyhow::Result<Self> {
        let session = bluer::Session::new().await?;
        Ok(Self { session })
    }

    pub async fn scan(
        &self,
        timeout: Duration,
        callback: Option<Arc<dyn ScanCallback>>,
    ) -> anyhow::Result<Vec<DiscoveredDevice>> {
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

        let timeout_fut = tokio::time::sleep(timeout);
        tokio::pin!(timeout_fut);

        loop {
            tokio::select! {
                _ = &mut timeout_fut => {
                    break;
                }
                event = discoverer.next() => {
                    match event {
                        Some(AdapterEvent::DeviceAdded(addr)) => {
                            let device = adapter.device(addr)?;
                            if let Ok(Some(dev)) = self.parse_device(&device).await {
                                if !discovered_map.contains_key(&addr) {
                                    debug!(
                                        "CatShare device found: addr={}, name='{}'",
                                        addr, dev.name
                                    );

                                    // 实时汇报
                                    if let Some(ref cb) = callback {
                                        cb.on_device_found(dev.clone()).await;
                                    }

                                    discovered_map.insert(addr, dev);
                                }
                            }
                        }
                        None => break,
                        _ => {}
                    }
                }
            }
        }

        // 也检查已经发现的设备（可能是扫描开始前已存在的）
        let device_addrs = adapter.device_addresses().await?;
        debug!("Checking {} cached devices", device_addrs.len());
        for addr in device_addrs {
            if !discovered_map.contains_key(&addr) {
                let device = adapter.device(addr)?;
                if let Ok(Some(dev)) = self.parse_device(&device).await {
                    debug!(
                        "Found CatShare device in cache: addr={}, name='{}'",
                        addr, dev.name
                    );

                    // 实时汇报
                    if let Some(ref cb) = callback {
                        cb.on_device_found(dev.clone()).await;
                    }

                    discovered_map.insert(addr, dev);
                }
            }
        }

        info!(
            "Scan complete: found {} CatShare-compatible device(s)",
            discovered_map.len()
        );
        Ok(discovered_map.into_values().collect())
    }

    async fn parse_device(
        &self,
        device: &bluer::Device,
    ) -> anyhow::Result<Option<DiscoveredDevice>> {
        // 检查设备的 UUID 和 Service Data
        let addr = device.address();
        let manufacturer_data = device.manufacturer_data().await?.unwrap_or_default();
        let mut name = device
            .name()
            .await?
            .unwrap_or_else(|| "<unknown>".to_string());

        // 如果名称未知，尝试从厂商数据中提取
        if name == "<unknown>" {
            let mut candidates = Vec::new();
            for (_, data) in &manufacturer_data {
                let mut current_seq = Vec::new();
                for &b in data {
                    if (32..=126).contains(&b) {
                        current_seq.push(b);
                    } else {
                        if current_seq.len() >= 3 {
                            if let Ok(s) = String::from_utf8(current_seq.clone()) {
                                candidates.push(s.trim().to_string());
                            }
                        }
                        current_seq.clear();
                    }
                }
                if current_seq.len() >= 3 {
                    if let Ok(s) = String::from_utf8(current_seq) {
                        candidates.push(s.trim().to_string());
                    }
                }
            }

            if !candidates.is_empty() {
                // 优先选择包含已知品牌的
                let brands = [
                    "REDMI", "XIAOMI", "ONEPLUS", "OPPO", "VIVO", "REALME", "MI", "HUAWEI", "HONOR",
                ];
                let mut branded: Vec<_> = candidates
                    .iter()
                    .filter(|s| {
                        let s_up = s.to_uppercase();
                        brands.iter().any(|&b| s_up.contains(b))
                    })
                    .collect();

                if !branded.is_empty() {
                    branded.sort_by_key(|s| s.len());
                    name = branded.last().unwrap().to_string();
                } else {
                    candidates.sort_by_key(|s| s.len());
                    name = candidates.last().unwrap().clone();
                }
            }
        }

        let uuids = device.uuids().await?.unwrap_or_default();
        let service_data = device.service_data().await?.unwrap_or_default();

        let scan_resp_uuid = uuid::Uuid::from_u128(0x0000ffff_0000_1000_8000_00805f9b34fb);
        // MTA/CatShare 相关的 16-bit UUID 范围 (0x3331 - 0x3334)
        let is_mta_uuid = |u: &uuid::Uuid| {
            let b = u.as_bytes();
            // 检查 16-bit 格式: 0000xxxx-0000-1000-8000-00805f9b34fb
            // xxxx 应该在 0x3331 和 0x3334 之间
            b[0] == 0 && b[1] == 0 && b[4] == 0 && b[5] == 0 &&
            b[2] == 0x33 && (0x31..=0x34).contains(&b[3]) &&
            b[6] == 0x10 && b[7] == 0x00 && // 标准蓝牙基准 UUID 的一部分
            &b[8..] == &[0x80, 0x00, 0x00, 0x80, 0x5f, 0x9b, 0x34, 0xfb]
        };

        let mta_uuid_in_list = uuids.iter().any(is_mta_uuid);
        let identity_data = service_data.get(&scan_resp_uuid).or_else(|| {
            // 如果没有全域 0xFFFF 键，尝试寻找 0x3331..0x3334 的 service data
            service_data
                .iter()
                .find(|(k, _)| is_mta_uuid(k))
                .map(|(_, v)| v)
        });

        // 检查厂商数据 (Xiaomi ID: 0x038F)
        let has_mta_manufacturer = manufacturer_data.contains_key(&0x038f);

        // 只要满足其中之一就被认为是 CatShare 设备
        let has_base_service = mta_uuid_in_list || identity_data.is_some() || has_mta_manufacturer;

        // 详细日志
        let interesting_name = name.contains("一加")
            || name.contains("OnePlus")
            || name.contains("Redmi")
            || name.contains("Xiaomi")
            || name.contains("小米");

        if has_base_service || interesting_name {
            debug!(
                "Device {}: name='{}', uuids={}, s_data={}, m_data={}",
                addr,
                name,
                uuids.len(),
                service_data.len(),
                manufacturer_data.len()
            );
            if !uuids.is_empty() {
                debug!("  -> All UUIDs: {:?}", uuids);
            }
            if !manufacturer_data.is_empty() {
                let m_keys: Vec<String> = manufacturer_data
                    .keys()
                    .map(|k| format!("0x{:04X}", k))
                    .collect();
                debug!("  -> Manufacturer IDs: {:?}", m_keys);
                for (id, data) in &manufacturer_data {
                    let hex_data: String = data.iter().map(|b| format!("{:02x}", b)).collect();
                    debug!("  -> [0x{:04X}] Data: {}", id, hex_data);
                }
            }
            debug!(
                "  -> Match result: mta_uuid_list={}, has_id_data={}, mta_combined={}",
                mta_uuid_in_list,
                identity_data.is_some(),
                has_base_service
            );
        }

        if has_base_service {
            debug!(
                "Device {} matches CatShare: mta_uuid_list={}, has_id_data={}",
                addr,
                mta_uuid_in_list,
                identity_data.is_some()
            );
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

            info!(
                "Discovered CatShare device: name='{}', addr={}, rssi={:?}, 5GHz={}",
                name,
                device.address(),
                rssi,
                supports_5ghz
            );
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
