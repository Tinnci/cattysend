//! GATT Server - 接收端 BLE 广播和服务
//!
//! 提供与 CatShare (Android) 兼容的 BLE GATT 服务器实现。
//!
//! # 功能
//!
//! - 发布 BLE 广播（与 CatShare 广播格式兼容）
//! - 提供 GATT 服务包含 STATUS 和 P2P 特征
//! - 处理发送端的 P2P 信息写入
//!
//! # 广播数据格式
//!
//! CatShare 兼容的广播包含：
//! - Service UUID: `00003331-0000-1000-8000-008123456789`
//! - Service Data (0x01FF): 6 字节身份数据
//! - Scan Response (0xFFFF): 27 字节，包含设备名称和协议版本

use log::{debug, error, info, trace, warn};

use crate::ble::{
    ADV_SERVICE_UUID, DeviceInfo, MAIN_SERVICE_UUID, P2P_CHAR_UUID, STATUS_CHAR_UUID,
    mgmt_advertiser::{LegacyAdvConfig, MgmtLegacyAdvertiser},
};
use crate::config::{AppSettings, BrandId};
use crate::crypto::BleSecurityPersistent;
use crate::wifi::P2pInfo;
use bluer::{
    adv::Advertisement,
    gatt::local::{
        Application, Characteristic, CharacteristicRead, CharacteristicWrite,
        CharacteristicWriteMethod, ReqError, Service,
    },
};
use futures_util::FutureExt;
use std::collections::BTreeSet;
use std::sync::Arc;
use tokio::sync::{Mutex, mpsc};

/// 从随机数据生成 sender ID
fn sender_id_from_random_data(random_data: &[u8; 2]) -> String {
    format!("{:02x}{:02x}", random_data[0], random_data[1])
}

/// P2P 信息接收事件
#[derive(Debug, Clone)]
pub struct P2pReceiveEvent {
    pub p2p_info: P2pInfo,
    pub sender_public_key: Option<String>,
}

/// GATT Server 状态
pub struct GattServerState {
    pub device_info: DeviceInfo,
    pub device_info_bytes: Vec<u8>,
}

impl GattServerState {
    pub fn new(mac_address: String, public_key: String) -> anyhow::Result<Self> {
        let device_info = DeviceInfo::new(public_key, mac_address);
        let device_info_bytes = serde_json::to_vec(&device_info)?;

        Ok(Self {
            device_info,
            device_info_bytes,
        })
    }

    pub fn update_mac(&mut self, mac: String) -> anyhow::Result<()> {
        self.device_info.mac = mac;
        self.device_info_bytes = serde_json::to_vec(&self.device_info)?;
        Ok(())
    }
}

/// GATT Server
pub struct GattServer {
    state: Arc<Mutex<GattServerState>>,
    p2p_tx: mpsc::Sender<P2pReceiveEvent>,
    p2p_rx: Option<mpsc::Receiver<P2pReceiveEvent>>,
    /// 随机数据 (2 bytes)，用于 sender ID 和广播身份
    random_data: [u8; 2],
    sender_id: String,
    device_name: String,
    security: Option<Arc<BleSecurityPersistent>>,
    /// 厂商 ID
    brand_id: BrandId,
    /// 是否支持 5GHz
    supports_5ghz: bool,
}

impl GattServer {
    /// 创建新的 GATT Server
    pub fn new(
        mac_address: String,
        device_name: String,
        public_key: String,
    ) -> anyhow::Result<Self> {
        let state = GattServerState::new(mac_address, public_key)?;

        let (p2p_tx, p2p_rx) = mpsc::channel(16);
        // 生成随机数据 (2 bytes)，在整个 GATT Server 生命周期内保持不变
        let random_data: [u8; 2] = rand::random();
        let sender_id = sender_id_from_random_data(&random_data);

        Ok(Self {
            state: Arc::new(Mutex::new(state)),
            p2p_tx,
            p2p_rx: Some(p2p_rx),
            random_data,
            sender_id,
            device_name,
            security: None,
            brand_id: BrandId::Linux,
            supports_5ghz: true,
        })
    }

    /// 从 AppSettings 创建 GattServer
    pub fn from_settings(
        mac_address: String,
        public_key: String,
        settings: &AppSettings,
    ) -> anyhow::Result<Self> {
        let mut server = Self::new(mac_address, settings.device_name.clone(), public_key)?;
        server.brand_id = settings.brand_id;
        server.supports_5ghz = settings.supports_5ghz;
        Ok(server)
    }

    /// 设置安全上下文，用于自动解密 P2P 信息
    pub fn with_security(mut self, security: Arc<BleSecurityPersistent>) -> Self {
        self.security = Some(security);
        self
    }

    /// 设置厂商 ID
    pub fn with_brand(mut self, brand_id: BrandId) -> Self {
        self.brand_id = brand_id;
        self
    }

    /// 设置 5GHz 支持
    pub fn with_5ghz_support(mut self, supports_5ghz: bool) -> Self {
        self.supports_5ghz = supports_5ghz;
        self
    }

    /// 获取 sender ID
    pub fn sender_id(&self) -> &str {
        &self.sender_id
    }

    /// 获取 P2P 信息接收通道
    pub fn take_p2p_receiver(&mut self) -> Option<mpsc::Receiver<P2pReceiveEvent>> {
        self.p2p_rx.take()
    }

    /// 启动 GATT 服务
    pub async fn start(&self) -> anyhow::Result<GattServerHandle> {
        debug!("Initializing BLE session...");
        let session = bluer::Session::new().await?;

        debug!("Getting default adapter...");
        let adapter = session.default_adapter().await?;

        let adapter_name = adapter.name().to_string();
        debug!("Powering on adapter: {}", adapter_name);
        adapter.set_powered(true).await?;

        let state = self.state.clone();
        let p2p_tx = self.p2p_tx.clone();

        // STATUS 特征 - 只读，返回 DeviceInfo JSON
        let state_for_read = state.clone();
        let status_char = Characteristic {
            uuid: STATUS_CHAR_UUID,
            read: Some(CharacteristicRead {
                read: true,
                fun: Box::new(move |req| {
                    let state = state_for_read.clone();
                    async move {
                        let s = state.lock().await;
                        let offset = req.offset as usize;
                        debug!(
                            "STATUS characteristic read: offset={}, data_len={}",
                            offset,
                            s.device_info_bytes.len()
                        );
                        if offset >= s.device_info_bytes.len() {
                            return Ok(vec![]);
                        }
                        Ok(s.device_info_bytes[offset..].to_vec())
                    }
                    .boxed()
                }),
                ..Default::default()
            }),
            ..Default::default()
        };

        // P2P 特征 - 可写，接收 P2pInfo JSON
        let p2p_tx_clone = p2p_tx.clone();
        let security_clone = self.security.clone();
        let p2p_char = Characteristic {
            uuid: P2P_CHAR_UUID,
            write: Some(CharacteristicWrite {
                write: true,
                write_without_response: true,
                method: CharacteristicWriteMethod::Fun(Box::new(move |data, _req| {
                    let p2p_tx = p2p_tx_clone.clone();
                    let security = security_clone.clone();
                    async move {
                        match process_p2p_write(&data, security.as_deref()) {
                            Ok(event) => {
                                let _ = p2p_tx.send(event).await;
                                Ok(())
                            }
                            Err(e) => {
                                error!("Failed to process P2P write: {}", e);
                                Err(ReqError::Failed)
                            }
                        }
                    }
                    .boxed()
                })),
                ..Default::default()
            }),
            ..Default::default()
        };

        // 创建 GATT 应用
        let app = Application {
            services: vec![Service {
                uuid: MAIN_SERVICE_UUID,
                primary: true,
                characteristics: vec![status_char, p2p_char],
                ..Default::default()
            }],
            ..Default::default()
        };

        debug!(
            "Registering GATT application with service_uuid={}",
            MAIN_SERVICE_UUID
        );
        let _app_handle = adapter.serve_gatt_application(app).await?;
        debug!("GATT application registered successfully");

        // 广播策略：
        // 1. 尝试 MGMT API (Legacy 模式，需要 CAP_NET_ADMIN)
        // 2. 如果失败，回退到 bluer (D-Bus，可能使用 Extended Advertising)
        let random_data = self.random_data;
        let flag_5ghz: u8 = if self.supports_5ghz { 0x01 } else { 0x00 };
        let brand = self.brand_id.id();
        let ident_uuid_short = ((flag_5ghz as u16) << 8) | (brand as u16);

        // 尝试 MGMT Legacy 广播
        let adv_backend = match self
            .try_mgmt_advertising(ident_uuid_short, random_data)
            .await
        {
            Ok(mgmt_adv) => {
                debug!("Using MGMT Legacy advertising (optimal compatibility)");
                AdvertisingBackend::Mgmt(mgmt_adv)
            }
            Err(e) => {
                warn!(
                    "MGMT advertising failed ({}), falling back to bluer D-Bus",
                    e
                );
                warn!("Note: This may use Extended Advertising which some devices don't support.");
                warn!("For Legacy mode, run: sudo setcap 'cap_net_admin+eip' <binary>");

                // 回退到 bluer
                let adv_handle = self.start_bluer_advertising(&adapter, random_data).await?;
                AdvertisingBackend::Bluer(adv_handle)
            }
        };

        info!(
            "GATT Server started, sender_id={}, device_name='{}'",
            self.sender_id, self.device_name
        );

        Ok(GattServerHandle {
            _adv_backend: adv_backend,
            _app_handle,
            _session: session,
        })
    }

    /// 尝试使用 MGMT API 启动 Legacy 广播
    async fn try_mgmt_advertising(
        &self,
        ident_uuid: u16,
        random_data: [u8; 2],
    ) -> anyhow::Result<MgmtLegacyAdvertiser> {
        let adv_config = LegacyAdvConfig::catshare_compatible(
            0x3331, // CatShare Service UUID
            ident_uuid,
            &[random_data[0], random_data[1], 0, 0, 0, 0],
            &self.device_name,
            random_data,
        );

        debug!(
            "Trying MGMT Legacy advertising: service=0x3331, ident=0x{:04x}",
            ident_uuid
        );

        let mut mgmt_adv = MgmtLegacyAdvertiser::new(adv_config).await?;
        mgmt_adv.start().await?;
        debug!("MGMT Legacy advertising started successfully");

        Ok(mgmt_adv)
    }

    /// 使用 bluer D-Bus 启动广播（回退方案）
    async fn start_bluer_advertising(
        &self,
        adapter: &bluer::Adapter,
        random_data: [u8; 2],
    ) -> anyhow::Result<bluer::adv::AdvertisementHandle> {
        // 构造精简的广播数据（尽量保持在 31 字节内以触发 Legacy 模式）
        let mut service_uuids = BTreeSet::new();
        service_uuids.insert(ADV_SERVICE_UUID);

        // 只放简短的身份数据，不放 27 字节的名称数据
        let flag_5ghz: u8 = if self.supports_5ghz { 0x01 } else { 0x00 };
        let brand = self.brand_id.id();
        let capability_short = ((flag_5ghz as u16) << 8) | (brand as u16);
        let ident_uuid = uuid::Uuid::from_u128(
            ((capability_short as u128) << 96) | 0x0000_1000_8000_0080_5f9b_34fb_u128,
        );

        let mut ident_payload = vec![0u8; 6];
        ident_payload[0] = random_data[0];
        ident_payload[1] = random_data[1];

        let mut service_data = std::collections::BTreeMap::new();
        service_data.insert(ident_uuid, ident_payload);

        let adv = Advertisement {
            advertisement_type: bluer::adv::Type::Peripheral,
            service_uuids,
            service_data,
            local_name: Some(self.device_name.clone()),
            discoverable: Some(true),
            ..Default::default()
        };

        debug!("Starting bluer D-Bus advertising (fallback mode)");
        let adv_handle = adapter.advertise(adv).await?;
        debug!("Bluer advertising started");

        Ok(adv_handle)
    }
}

/// 处理 P2P 特征写入
///
/// 如果提供 security 且 P2pInfo 包含发送端公钥 (key 字段)，则自动解密 SSID/PSK/MAC 字段。
fn process_p2p_write(
    data: &[u8],
    security: Option<&BleSecurityPersistent>,
) -> anyhow::Result<P2pReceiveEvent> {
    let json_str = std::str::from_utf8(data)?;
    let mut p2p_info: P2pInfo = serde_json::from_str(json_str)?;

    let is_encrypted = p2p_info.key.is_some();
    let sender_public_key = p2p_info.key.clone();

    if let (Some(sender_key), Some(sec)) = (&sender_public_key, security) {
        debug!("Sender provided public key, decrypting P2P info...");
        match sec.derive_session_key(sender_key) {
            Ok(cipher) => {
                p2p_info.ssid = cipher.decrypt(&p2p_info.ssid).unwrap_or(p2p_info.ssid);
                p2p_info.psk = cipher.decrypt(&p2p_info.psk).unwrap_or(p2p_info.psk);
                p2p_info.mac = cipher.decrypt(&p2p_info.mac).unwrap_or(p2p_info.mac);
                p2p_info.key = None; // 表示已解密
                info!("Successfully decrypted P2P info");
            }
            Err(e) => {
                error!("Failed to derive session key: {}", e);
            }
        }
    }

    info!(
        "Received P2P info from sender, ssid='{}', port={}, decrypted={}",
        p2p_info.ssid,
        p2p_info.port,
        is_encrypted && p2p_info.key.is_none()
    );
    trace!("Full P2P info: {:?}", p2p_info);

    Ok(P2pReceiveEvent {
        p2p_info,
        sender_public_key,
    })
}

/// 广播后端枚举
#[allow(dead_code)]
enum AdvertisingBackend {
    /// MGMT API (Legacy 模式，推荐)
    Mgmt(MgmtLegacyAdvertiser),
    /// bluer D-Bus (回退，可能 Extended)
    Bluer(bluer::adv::AdvertisementHandle),
}

/// GATT Server Handle - 保持服务运行
pub struct GattServerHandle {
    _adv_backend: AdvertisingBackend,
    _app_handle: bluer::gatt::local::ApplicationHandle,
    _session: bluer::Session,
}

impl GattServerHandle {
    /// 等待服务关闭信号
    pub async fn wait_for_shutdown(&self) {
        // 永远等待，直到被 drop
        std::future::pending::<()>().await;
    }
}
