//! GATT Server - 接收端 BLE 广播和服务
//!
//! 功能:
//! - 发布 BLE 广播（与 CatShare 格式兼容）
//! - 提供 GATT 服务包含 STATUS 和 P2P 特征
//! - 处理发送端的 P2P 信息写入

use crate::ble::{DeviceInfo, MAIN_SERVICE_UUID, P2P_CHAR_UUID, SERVICE_UUID, STATUS_CHAR_UUID};
// use crate::crypto::BleSecurity; // Removed
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
        })
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
        let session = bluer::Session::new().await?;
        let adapter = session.default_adapter().await?;
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
        let p2p_char = Characteristic {
            uuid: P2P_CHAR_UUID,
            write: Some(CharacteristicWrite {
                write: true,
                write_without_response: true,
                method: CharacteristicWriteMethod::Fun(Box::new(move |data, _req| {
                    let p2p_tx = p2p_tx_clone.clone();
                    async move {
                        match process_p2p_write(&data) {
                            Ok(event) => {
                                let _ = p2p_tx.send(event).await;
                                Ok(())
                            }
                            Err(e) => {
                                tracing::error!("Failed to process P2P write: {}", e);
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

        let _app_handle = adapter.serve_gatt_application(app).await?;

        // 创建广播数据
        let mut service_uuids = BTreeSet::new();
        service_uuids.insert(SERVICE_UUID);

        let adv = Advertisement {
            service_uuids: service_uuids.clone(),
            discoverable: Some(true),
            local_name: Some(self.device_name.clone()),
            ..Default::default()
        };

        let adv_handle = adapter.advertise(adv).await?;

        tracing::info!("GATT Server started, advertising as '{}'", self.device_name);

        Ok(GattServerHandle {
            _adv_handle: adv_handle,
            _session: session,
        })
    }
}

/// 处理 P2P 特征写入
fn process_p2p_write(data: &[u8]) -> anyhow::Result<P2pReceiveEvent> {
    let json_str = std::str::from_utf8(data)?;
    let p2p_info: P2pInfo = serde_json::from_str(json_str)?;

    tracing::info!("Received P2P info: {:?}", p2p_info);

    let sender_public_key = p2p_info.key.clone();

    Ok(P2pReceiveEvent {
        p2p_info,
        sender_public_key,
    })
}

/// GATT Server Handle - 保持服务运行
pub struct GattServerHandle {
    _adv_handle: bluer::adv::AdvertisementHandle,
    _session: bluer::Session,
}

impl GattServerHandle {
    /// 等待服务关闭信号
    pub async fn wait_for_shutdown(&self) {
        // 永远等待，直到被 drop
        std::future::pending::<()>().await;
    }
}
