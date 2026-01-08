//! 文件传输模块
//!
//! 包含:
//! - WebSocket 协议实现 (CatShare 兼容)
//! - HTTP/HTTPS 服务器 (发送端)
//! - HTTP/HTTPS 客户端 (接收端)

pub mod http_server;
pub mod protocol;
pub mod receiver_client;
pub mod sender_server;
pub mod websocket_handler;

pub use protocol::{SendRequest, WsMessage};
pub use receiver_client::{ReceiverCallback, ReceiverClient};
pub use sender_server::{FileEntry, TransferServer, TransferTask};

use serde::{Deserialize, Serialize};

/// 文件信息（用于传输协商）
#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FileInfo {
    pub name: String,
    pub size: u64,
    pub modified_time: u64,
    pub mime_type: Option<String>,
}
