//! BLE Client - 用于发送端连接接收端
//!
//! 流程:
//! 1. 连接到目标设备的 GATT Server
//! 2. 读取 CHAR_STATUS 获取 DeviceInfo (包含对方公钥)
//! 3. 派生会话密钥
//! 4. 加密 P2pInfo 并写入 CHAR_P2P

use crate::ble::{DeviceInfo, MAIN_SERVICE_UUID, P2P_CHAR_UUID, STATUS_CHAR_UUID};
use crate::crypto::BleSecurity;
use crate::wifi::P2pInfo;
use btleplug::api::{Central, Characteristic, Manager as _, Peripheral, WriteType};
use btleplug::platform::{Adapter, Manager, Peripheral as PlatformPeripheral};
use std::time::Duration;
use tokio::time;
use uuid::Uuid;

/// BLE 客户端错误
#[derive(Debug, thiserror::Error)]
pub enum BleClientError {
    #[error("No Bluetooth adapters found")]
    NoAdapter,

    #[error("Device not found")]
    DeviceNotFound,

    #[error("Service not found: {0}")]
    ServiceNotFound(Uuid),

    #[error("Characteristic not found: {0}")]
    CharacteristicNotFound(Uuid),

    #[error("Connection failed: {0}")]
    ConnectionFailed(String),

    #[error("IO error: {0}")]
    IoError(#[from] btleplug::Error),

    #[error("Protocol error: {0}")]
    ProtocolError(String),
}

pub struct BleClient {
    adapter: Adapter,
}

impl BleClient {
    pub async fn new() -> Result<Self, BleClientError> {
        let manager = Manager::new().await?;
        let adapters = manager.adapters().await?;
        let adapter = adapters
            .into_iter()
            .next()
            .ok_or(BleClientError::NoAdapter)?;

        Ok(Self { adapter })
    }

    /// 连接到设备并执行 P2P 握手
    ///
    /// 返回接收端的 DeviceInfo
    pub async fn connect_and_handshake(
        &self,
        device_address: &str,
        p2p_info: &P2pInfo,
        sender_id: &str,
    ) -> Result<DeviceInfo, BleClientError> {
        // 查找目标设备
        let peripheral = self.find_device(device_address).await?;

        // 连接
        tracing::info!("Connecting to {}", device_address);
        peripheral.connect().await?;

        // 等待连接稳定
        time::sleep(Duration::from_millis(500)).await;

        // 请求更大的 MTU
        // Note: btleplug 不直接支持 MTU 请求，跳过

        // 发现服务
        tracing::info!("Discovering services...");
        peripheral.discover_services().await?;

        // 查找并读取 STATUS 特征
        let status_char = self.find_characteristic(&peripheral, STATUS_CHAR_UUID)?;
        let status_data = peripheral.read(&status_char).await?;
        let device_info: DeviceInfo = serde_json::from_slice(&status_data)
            .map_err(|e| BleClientError::ProtocolError(format!("Invalid DeviceInfo: {}", e)))?;

        tracing::info!("Remote device info: {:?}", device_info);

        // 如果对方提供了公钥，派生会话密钥并加密 P2P 信息
        let p2p_data = if let Some(peer_key) = &device_info.key {
            let security = BleSecurity::new().map_err(|e| {
                BleClientError::ProtocolError(format!("Failed to init security: {}", e))
            })?;
            let sender_public_key = security.get_public_key().to_string();

            let cipher = security.derive_session_key(peer_key).map_err(|e| {
                BleClientError::ProtocolError(format!("Key exchange failed: {}", e))
            })?;

            // 加密 P2P 信息
            let encrypted_p2p = P2pInfo::with_encryption(
                sender_id.to_string(),
                cipher
                    .encrypt(&p2p_info.ssid)
                    .map_err(|e| BleClientError::ProtocolError(e.to_string()))?,
                cipher
                    .encrypt(&p2p_info.psk)
                    .map_err(|e| BleClientError::ProtocolError(e.to_string()))?,
                cipher
                    .encrypt(&p2p_info.mac)
                    .map_err(|e| BleClientError::ProtocolError(e.to_string()))?,
                p2p_info.port,
                sender_public_key,
            );
            serde_json::to_vec(&encrypted_p2p)
                .map_err(|e| BleClientError::ProtocolError(e.to_string()))?
        } else {
            // 不加密
            serde_json::to_vec(p2p_info)
                .map_err(|e| BleClientError::ProtocolError(e.to_string()))?
        };

        // 写入 P2P 特征
        let p2p_char = self.find_characteristic(&peripheral, P2P_CHAR_UUID)?;
        tracing::info!("Writing P2P info ({} bytes)", p2p_data.len());
        peripheral
            .write(&p2p_char, &p2p_data, WriteType::WithResponse)
            .await?;

        // 断开连接
        peripheral.disconnect().await?;

        Ok(device_info)
    }

    async fn find_device(&self, address: &str) -> Result<PlatformPeripheral, BleClientError> {
        let peripherals = self.adapter.peripherals().await?;

        for peripheral in peripherals {
            if let Some(props) = peripheral.properties().await? {
                if props.address.to_string().to_uppercase() == address.to_uppercase() {
                    return Ok(peripheral);
                }
            }
        }

        Err(BleClientError::DeviceNotFound)
    }

    fn find_characteristic(
        &self,
        peripheral: &PlatformPeripheral,
        uuid: Uuid,
    ) -> Result<Characteristic, BleClientError> {
        for service in peripheral.services() {
            if service.uuid == MAIN_SERVICE_UUID {
                for char in service.characteristics {
                    if char.uuid == uuid {
                        return Ok(char);
                    }
                }
            }
        }
        Err(BleClientError::CharacteristicNotFound(uuid))
    }
}
