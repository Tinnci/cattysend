//! Cattysend Core Library
//!
//! 互传联盟协议的核心实现库，与 CatShare (Android) 完全兼容

pub mod ble;
pub mod crypto;
pub mod transfer;
pub mod wifi;

pub use ble::{DeviceInfo, MAIN_SERVICE_UUID, P2P_CHAR_UUID, SERVICE_UUID, STATUS_CHAR_UUID};
pub use crypto::{BleSecurity, SessionCipher};
pub use wifi::P2pInfo;
