//! Cattysend Core Library
//!
//! 互传联盟协议的核心实现库，包含：
//! - BLE 设备发现与 GATT 通信
//! - ECDH 密钥交换与 AES 加密
//! - WiFi P2P 热点管理
//! - HTTP/WebSocket 文件传输

pub mod ble;
pub mod crypto;
pub mod transfer;
pub mod wifi;

pub use ble::{DeviceStatus, MAIN_SERVICE_UUID, SERVICE_UUID};
pub use crypto::{BleSecurity, SessionCipher};
pub use wifi::P2pInfo;
