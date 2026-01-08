//! Cattysend Core Library
//!
//! 互传联盟协议的核心实现库，与 CatShare (Android) 完全兼容
//!
//! # 模块
//!
//! - **ble**: BLE 扫描、广播、GATT 客户端/服务器
//! - **crypto**: ECDH 密钥交换和 AES-CTR 加密
//! - **wifi**: WiFi P2P 热点创建和连接
//! - **transfer**: HTTP/WebSocket 文件传输
//!
//! # 使用示例
//!
//! ## 发送文件
//!
//! ```ignore
//! use cattysend_core::{BleScanner, BleClient, WiFiP2pSender, TransferServer};
//!
//! // 1. 扫描接收端设备
//! let scanner = BleScanner::new().await?;
//! let devices = scanner.scan(Duration::from_secs(5)).await?;
//!
//! // 2. 创建 WiFi P2P 热点并启动传输服务器
//! let sender = WiFiP2pSender::new("wlan0");
//! let p2p_info = sender.create_group(8443).await?;
//!
//! // 3. 连接到接收端并发送 P2P 信息
//! let ble_client = BleClient::new().await?;
//! ble_client.connect_and_handshake(&device.address, &p2p_info, "sender_id").await?;
//!
//! // 4. 等待接收端连接并传输文件
//! ```
//!
//! ## 接收文件
//!
//! ```ignore
//! use cattysend_core::{GattServer, WiFiP2pReceiver, ReceiverClient};
//!
//! // 1. 启动 GATT Server 等待连接
//! let server = GattServer::new("02:00:00:00:00:00", "MyDevice")?;
//! let handle = server.start().await?;
//!
//! // 2. 等待收到 P2P 信息
//! let p2p_event = p2p_rx.recv().await?;
//!
//! // 3. 连接到发送端热点
//! let receiver = WiFiP2pReceiver::new("wlan0");
//! let ip = receiver.connect(&p2p_event.p2p_info).await?;
//!
//! // 4. 接收文件
//! let client = ReceiverClient::new(&host_ip, p2p_info.port, output_dir);
//! client.start(&callback).await?;
//! ```

pub mod ble;
pub mod crypto;
pub mod transfer;
pub mod wifi;

// BLE re-exports
pub use ble::{
    BleClient, BleScanner, DeviceInfo, DiscoveredDevice, GattServer, GattServerHandle,
    MAIN_SERVICE_UUID, P2P_CHAR_UUID, SERVICE_UUID, STATUS_CHAR_UUID,
};

// Crypto re-exports
pub use crypto::{BleSecurity, SessionCipher};

// WiFi re-exports
pub use wifi::{P2pConfig, P2pInfo, WiFiP2pReceiver, WiFiP2pSender};

// Transfer re-exports
pub use transfer::{
    FileEntry, ReceiverCallback, ReceiverClient, SendRequest, TransferServer, TransferTask,
    WsMessage,
};
