//! BLE Scanner for CatShare (MTA) Compatible Devices
//!
//! Handles discovery of devices broadcasting the CatShare/MTA protocol via BLE.
//! Uses `bluer` to interface with BlueZ.
//!
//! # Logic
//!
//! Devices are identified by:
//! 1. Service UUIDs in the range `00003331` to `00003334` (base `00805f9b34fb`).
//! 2. Manufacturer Data (specifically Xiaomi `0x038F`).
//! 3. Service Data for specific UUIDs containing legacy device info.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use bluer::{Adapter, AdapterEvent, Device, Session};
use futures_util::{StreamExt, pin_mut};
use log::{debug, info, warn};
use uuid::Uuid;

/// Manufacturer ID for Xiaomi
const MANUF_ID_XIAOMI: u16 = 0x038F;

/// Scan Response UUID (Legacy)
const SCAN_RESP_UUID_STR: &str = "0000ffff-0000-1000-8000-00805f9b34fb";

/// The Base UUID suffix for Bluetooth SIG
const BASE_UUID_SUFFIX: [u8; 12] = [
    0x00, 0x00, 0x10, 0x00, 0x80, 0x00, 0x00, 0x80, 0x5f, 0x9b, 0x34, 0xfb,
];

/// Known brands found in CatShare/MTA protocol
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Brand {
    Xiaomi,
    BlackShark,
    Oppo,
    Realme,
    OnePlus,
    Vivo,
    Meizu,
    Nubia,
    Samsung,
    Zte,
    Smartisan,
    Lenovo,
    Motorola,
    Nio,
    Honor,
    Hisense,
    Asus,
    Rog,
    Unknown(i16),
}

impl std::fmt::Display for Brand {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Xiaomi => write!(f, "Xiaomi"),
            Self::BlackShark => write!(f, "Black Shark"),
            Self::Oppo => write!(f, "OPPO"),
            Self::Realme => write!(f, "realme"),
            Self::OnePlus => write!(f, "OnePlus"),
            Self::Vivo => write!(f, "vivo"),
            Self::Meizu => write!(f, "Meizu"),
            Self::Nubia => write!(f, "Nubia"),
            Self::Samsung => write!(f, "Samsung"),
            Self::Zte => write!(f, "ZTE"),
            Self::Smartisan => write!(f, "Smartisan"),
            Self::Lenovo => write!(f, "Lenovo"),
            Self::Motorola => write!(f, "Motorola"),
            Self::Nio => write!(f, "Nio"),
            Self::Honor => write!(f, "Honor"),
            Self::Hisense => write!(f, "Hisense"),
            Self::Asus => write!(f, "ASUS"),
            Self::Rog => write!(f, "ROG"),
            Self::Unknown(id) => write!(f, "Unknown ({})", id),
        }
    }
}

impl From<i16> for Brand {
    fn from(id: i16) -> Self {
        // Original logic derived from decompiled Java/Smali code.
        // Some negative values correspond to signed byte interpretations of high keys.
        match id {
            11 => Self::Realme,
            10..=19 => Self::Oppo,
            20..=29 => Self::Vivo,
            32 => Self::BlackShark,
            30..=39 => Self::Xiaomi,
            41..=45 => Self::OnePlus,
            50..=59 => Self::Meizu,
            60..=69 => Self::Nubia,
            70..=75 => Self::Samsung,
            80..=89 => Self::Zte,
            90..=95 => Self::Smartisan,
            100..=109 => Self::Lenovo,
            110..=119 => Self::Motorola,
            120..=129 => Self::Nio,
            140..=149 => Self::Honor,
            // Java signed byte: -86 (0xAA) .. -77
            -86..=-77 | 170..=179 => Self::Hisense,
            // Java signed byte: -96 (0xA0) .. -87
            -96 | 160 => Self::Rog,
            -95..=-87 | 161..=169 => Self::Asus,
            _ => Self::Unknown(id),
        }
    }
}

/// Helper for backward compatibility with existing code
pub fn get_vendor_name(id: i16) -> String {
    Brand::from(id).to_string()
}

#[derive(Debug, Clone)]
pub struct DiscoveredDevice {
    pub name: String,
    pub address: String,
    pub sender_id: String,
    pub brand: String,
    pub brand_id: Option<i16>,
    pub rssi: Option<i16>,
    pub supports_5ghz: bool,
}

#[async_trait]
pub trait ScanCallback: Send + Sync {
    async fn on_device_found(&self, device: DiscoveredDevice);
}

/// 通用的基于 Channel 的扫描回调
///
/// 将发现的设备转换为指定类型的事件并发送到 channel。
pub struct ChannelScanCallback<F, T> {
    tx: tokio::sync::mpsc::Sender<T>,
    map_fn: F,
}

impl<F, T> ChannelScanCallback<F, T>
where
    F: Fn(DiscoveredDevice) -> T + Send + Sync,
    T: Send,
{
    pub fn new(tx: tokio::sync::mpsc::Sender<T>, map_fn: F) -> Self {
        Self { tx, map_fn }
    }
}

#[async_trait]
impl<F, T> ScanCallback for ChannelScanCallback<F, T>
where
    F: Fn(DiscoveredDevice) -> T + Send + Sync,
    T: Send,
{
    async fn on_device_found(&self, device: DiscoveredDevice) {
        let event = (self.map_fn)(device);
        let _ = self.tx.send(event).await;
    }
}

/// Parses raw byte sequences to find the "best" human-readable device name.
/// Heuristically prefers strings containing known brand names.
fn extract_ascii_name(data: &[u8]) -> Option<String> {
    let known_brands = [
        "Redmi", "Xiaomi", "Mi", "POCO", "OnePlus", "OPPO", "vivo", "Realme",
    ];

    // Split data into chunks of potential ASCII strings
    let candidates = data
        .split(|&b| !matches!(b, 32..=126)) // Split on non-printable ASCII
        .filter(|chunk| chunk.len() >= 4) // Only consider reasonable lengths
        .filter_map(|chunk| String::from_utf8(chunk.to_vec()).ok())
        .map(|s| s.trim().to_string());

    // Select the best candidate
    candidates.max_by(|a, b| {
        let score = |s: &str| {
            let mut val = s.len();
            if known_brands.iter().any(|brand| s.contains(brand)) {
                val += 100; // Boost score if it contains a brand name
            }
            val
        };
        score(a).cmp(&score(b))
    })
}

pub struct BleScanner {
    session: Session,
}

impl BleScanner {
    pub async fn new() -> anyhow::Result<Self> {
        let session = Session::new().await?;
        Ok(Self { session })
    }

    pub async fn scan(
        &self,
        timeout: Duration,
        callback: Option<Arc<dyn ScanCallback>>,
    ) -> anyhow::Result<Vec<DiscoveredDevice>> {
        let adapter = self.init_adapter().await?;
        let mut discovered_map = HashMap::new();

        info!(
            "Starting BLE scan for {}s on {}",
            timeout.as_secs(),
            adapter.name()
        );

        let mut device_events = adapter.discover_devices().await?;
        let timeout_fut = tokio::time::sleep(timeout);
        pin_mut!(timeout_fut);

        // Process incoming events and timeout
        loop {
            tokio::select! {
                _ = &mut timeout_fut => break,
                Some(event) = device_events.next() => {
                    if let AdapterEvent::DeviceAdded(addr) = event {
                        if let Ok(device) = adapter.device(addr) {
                            self.process_device(&device, &mut discovered_map, callback.as_ref()).await;
                        }
                    }
                }
                else => break,
            }
        }

        // Post-scan: Check any already cached devices we might have missed events for
        // or that were already present.
        if let Ok(cached_addrs) = adapter.device_addresses().await {
            debug!("Checking {} cached devices", cached_addrs.len());
            for addr in cached_addrs {
                if !discovered_map.contains_key(&addr) {
                    if let Ok(device) = adapter.device(addr) {
                        self.process_device(&device, &mut discovered_map, callback.as_ref())
                            .await;
                    }
                }
            }
        }

        info!("Scan complete. Found {} devices.", discovered_map.len());
        Ok(discovered_map.into_values().collect())
    }

    async fn init_adapter(&self) -> bluer::Result<Adapter> {
        let adapter = self.session.default_adapter().await?;
        adapter.set_powered(true).await?;
        // Ensure discovery filter is reset/set to defaults to catch everything
        adapter.set_discovery_filter(Default::default()).await?;
        Ok(adapter)
    }

    async fn process_device(
        &self,
        device: &Device,
        discovered_map: &mut HashMap<bluer::Address, DiscoveredDevice>,
        callback: Option<&Arc<dyn ScanCallback>>,
    ) {
        let addr = device.address();
        // Skip if already processed
        if discovered_map.contains_key(&addr) {
            return;
        }

        match self.parse_device(device).await {
            Ok(Some(dev)) => {
                debug!("Matched CatShare device: {} ({})", dev.name, addr);
                if let Some(cb) = callback {
                    cb.on_device_found(dev.clone()).await;
                }
                discovered_map.insert(addr, dev);
            }
            Ok(None) => { /* Not a target device */ }
            Err(e) => {
                warn!("Error parsing device {}: {:?}", addr, e);
            }
        }
    }

    async fn parse_device(&self, device: &Device) -> anyhow::Result<Option<DiscoveredDevice>> {
        let uuids = device.uuids().await?.unwrap_or_default();
        let service_data = device.service_data().await?.unwrap_or_default();
        let manuf_data = device.manufacturer_data().await?.unwrap_or_default();

        // 1. Check if device matches CatShare/MTA characteristics
        let is_mta = self.is_mta_device(&uuids, &service_data, &manuf_data);
        if !is_mta {
            return Ok(None);
        }

        // 2. Extract Device Name
        let name = self.resolve_device_name(device, &manuf_data).await?;

        // 3. Extract Metadata (Sender ID, Brand, etc.)
        let (sender_id, brand_id, supports_5ghz) =
            self.parse_service_metadata(&service_data, &manuf_data);

        let brand = brand_id
            .map(|id| Brand::from(id).to_string())
            .unwrap_or_else(|| "Unknown".to_string());

        let rssi = device.rssi().await?;

        Ok(Some(DiscoveredDevice {
            name,
            address: device.address().to_string(),
            sender_id,
            brand,
            brand_id,
            rssi,
            supports_5ghz,
        }))
    }

    fn is_mta_device(
        &self,
        uuids: &HashSet<Uuid>,
        service_data: &HashMap<Uuid, Vec<u8>>,
        manuf_data: &HashMap<u16, Vec<u8>>,
    ) -> bool {
        let has_mta_uuid = uuids.iter().any(Self::is_mta_uuid);
        let has_mta_service_data = service_data.keys().any(Self::is_mta_uuid);
        let has_xiaomi_manuf = manuf_data.contains_key(&MANUF_ID_XIAOMI);

        // Standard Scan Response UUID: 0000ffff-...
        // We check if it exists in service data keys
        let scan_resp_uuid = Uuid::parse_str(SCAN_RESP_UUID_STR).unwrap_or_default();
        let has_scan_resp = service_data.contains_key(&scan_resp_uuid);

        has_mta_uuid || has_mta_service_data || has_xiaomi_manuf || has_scan_resp
    }

    /// Checks if a UUID matches the MTA range: 0000333x-0000-1000-8000-00805f9b34fb
    fn is_mta_uuid(u: &Uuid) -> bool {
        let b = u.as_bytes();
        // Check standard base matching 0000xxxx-0000-1000-8000-00805f9b34fb
        if b[4..] != BASE_UUID_SUFFIX {
            return false;
        }
        // Check specific prefix 0000333x
        b[0] == 0 && b[1] == 0 && b[2] == 0x33 && matches!(b[3], 0x31..=0x34)
    }

    async fn resolve_device_name(
        &self,
        device: &Device,
        manuf_data: &HashMap<u16, Vec<u8>>,
    ) -> anyhow::Result<String> {
        let system_name = device
            .name()
            .await?
            .unwrap_or_else(|| "<unknown>".to_string());

        // Try to find a better name in Manufacturer Data
        // 1. Priority: Xiaomi Manuf Data (0x038F)
        if let Some(data) = manuf_data.get(&MANUF_ID_XIAOMI) {
            if let Some(name) = extract_ascii_name(data) {
                return Ok(name);
            }
        }

        // 2. If system name looks bad, search other Manuf Data
        if self.is_name_suspicious(&system_name) {
            for (id, data) in manuf_data {
                if *id == MANUF_ID_XIAOMI {
                    continue;
                }
                if let Some(name) = extract_ascii_name(data) {
                    return Ok(name);
                }
            }
        }

        // 3. Fallback to system name, cleaned
        Ok(self.clean_name(&system_name))
    }

    fn is_name_suspicious(&self, name: &str) -> bool {
        name == "<unknown>" || name.starts_with('(') || name.ends_with('$') || name.ends_with('\t')
    }

    fn clean_name(&self, name: &str) -> String {
        name.trim_matches(|c| c == '(' || c == '$' || c == '\t')
            .to_string()
    }

    fn parse_service_metadata(
        &self,
        service_data: &HashMap<Uuid, Vec<u8>>,
        manuf_data: &HashMap<u16, Vec<u8>>,
    ) -> (String, Option<i16>, bool) {
        let mut sender_id = "0000".to_string();
        let mut brand_id = None;
        let mut supports_5ghz = false;

        for (uuid, data) in service_data {
            match data.len() {
                // 27-byte data: typical CatShare payload with ID and partial name
                27 => {
                    // ID at offset 8 (big endian u16)
                    let id_val = u16::from_be_bytes([data[8], data[9]]);
                    sender_id = format!("{:04x}", id_val);
                    // Name is at data[10..] but we usually prefer the one from manuf data or GAP
                }
                // 6-byte data: often contains capability flags in UUID + data
                6 => {
                    let u_bytes = uuid.as_bytes();
                    if u_bytes[0..2] == [0, 0] {
                        supports_5ghz = u_bytes[2] == 1;
                        // Brand ID is often in the UUID byte 3
                        brand_id = Some(u_bytes[3] as i16);
                    }
                }
                _ => {}
            }
        }

        // If Brand ID not found in Service UUID, infer from Manufacturer Data key
        if brand_id.is_none() {
            if let Some(key) = manuf_data.keys().next() {
                // Heuristic: Take the first manufacturer ID as brand ID
                // Note: casting u16 to i16 to match legacy signed logic
                // Note: casting u16 to i16 to match legacy signed logic
                #[allow(clippy::cast_possible_wrap)]
                let signed_id = *key as i16;
                brand_id = Some(signed_id);
            }
        }

        (sender_id, brand_id, supports_5ghz)
    }
}
