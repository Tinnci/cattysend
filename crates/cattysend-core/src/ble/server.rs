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

use log::{debug, error, info, trace};

use crate::ble::{
    ADV_SERVICE_UUID, DeviceInfo, MAIN_SERVICE_UUID, P2P_CHAR_UUID, STATUS_CHAR_UUID,
};
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

/// 广播数据中的随机 ID（用于 sender ID）
fn generate_sender_id() -> String {
    let random_bytes: [u8; 2] = rand::random();
    format!("{:02x}{:02x}", random_bytes[0], random_bytes[1])
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
    sender_id: String,
    device_name: String,
    security: Option<Arc<BleSecurityPersistent>>,
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
        let sender_id = generate_sender_id();

        Ok(Self {
            state: Arc::new(Mutex::new(state)),
            p2p_tx,
            p2p_rx: Some(p2p_rx),
            sender_id,
            device_name,
            security: None,
        })
    }

    /// 设置安全上下文，用于自动解密 P2P 信息
    pub fn with_security(mut self, security: Arc<BleSecurityPersistent>) -> Self {
        self.security = Some(security);
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

        // 构造 CatShare 兼容的广播数据
        //
        // 注意: BlueZ 的 Advertisement 接口无法精确控制广播包和扫描响应的分布
        // 我们需要确保总数据量在 31 字节以内，否则可能导致 ADV_SERVICE_UUID 被挤出主包
        //
        // 广播包结构 (最大 31 字节):
        // - Flags (3 bytes): 自动添加
        // - Complete 128-bit Service UUIDs (18 bytes): 2 + 16 = 18
        // - Service Data (最多 10 bytes 剩余空间)

        // 1. 基础服务 UUID - 使用 ADV_SERVICE_UUID (008123456789 后缀)
        let mut service_uuids = BTreeSet::new();
        service_uuids.insert(ADV_SERVICE_UUID);

        // 2. 身份/能力 UUID (0x01FF = 5GHz + Linux)
        // 使用 16-bit UUID 可以节省空间: 0x01FF
        // 但 BlueZ 只支持 128-bit UUID，所以我们需要用完整格式
        let ident_uuid = uuid::Uuid::from_u128(0x000001ff_0000_1000_8000_00805f9b34fb);
        let mut service_data = std::collections::BTreeMap::new();

        // 提供 2 字节识别数据 (与 CatShare RANDOM_DATA 对齐)
        // CatShare 的 RANDOM_DATA 实际上只有 2 字节
        let mut ident_payload = vec![0u8; 2];
        if let Ok(id_val) = u16::from_str_radix(&self.sender_id, 16) {
            let id_bytes = id_val.to_be_bytes();
            ident_payload[0] = id_bytes[0];
            ident_payload[1] = id_bytes[1];
        }
        service_data.insert(ident_uuid, ident_payload);

        // 注意: 我们不再在广播中放入 0xFFFF 的 27 字节数据
        // 因为这会导致总数据超过 31 字节，BlueZ 可能会丢弃关键的 ADV_SERVICE_UUID
        // 设备名称通过 local_name 传递

        let adv = Advertisement {
            advertisement_type: bluer::adv::Type::Peripheral,
            service_uuids: service_uuids.clone(),
            service_data,
            local_name: Some(self.device_name.clone()),
            discoverable: Some(true),
            ..Default::default()
        };

        debug!(
            "Starting BLE advertisement: service_uuid={}, identity_uuid={}, local_name={}",
            ADV_SERVICE_UUID, ident_uuid, self.device_name
        );
        let adv_handle = adapter.advertise(adv).await?;
        debug!("BLE advertisement started successfully");

        info!(
            "GATT Server started, sender_id={}, device_name='{}'",
            self.sender_id, self.device_name
        );
        debug!(
            "Advertising with service_uuid={}, identity_uuid={}",
            ADV_SERVICE_UUID, ident_uuid
        );

        Ok(GattServerHandle {
            _adv_handle: adv_handle,
            _app_handle,
            _session: session,
        })
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

/// GATT Server Handle - 保持服务运行
pub struct GattServerHandle {
    _adv_handle: bluer::adv::AdvertisementHandle,
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
