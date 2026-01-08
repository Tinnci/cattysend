pub mod http_server;
pub mod websocket_handler;

use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct WsMessage {
    pub msg_type: String,
    pub msg_id: String,
    pub data: serde_json::Value,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct FileInfo {
    pub name: String,
    pub size: u64,
    pub modified_time: u64,
    pub mime_type: Option<String>,
}

#[derive(Serialize, Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct SendRequest {
    pub files: Vec<FileInfo>,
    pub total_size: u64,
    pub total_files: u32,
    pub package_type: String, // "single" or "multi"
    pub thumbnail: Option<String>,
    pub sender_device: String,
}
