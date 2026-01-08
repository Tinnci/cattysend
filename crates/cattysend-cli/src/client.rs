//! IPC Client - 与守护进程通信

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::UnixStream;

pub fn socket_path() -> PathBuf {
    std::env::var("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp"))
        .join("cattysend.sock")
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
pub enum IpcRequest {
    #[serde(rename = "status")]
    Status,
    #[serde(rename = "scan")]
    Scan { timeout_secs: u64 },
    #[serde(rename = "send")]
    Send {
        file_path: String,
        device_addr: Option<String>,
    },
    #[serde(rename = "receive")]
    Receive,
    #[serde(rename = "stop")]
    Stop,
}

#[derive(Serialize, Deserialize, Debug)]
#[serde(tag = "type")]
pub enum IpcResponse {
    #[serde(rename = "ok")]
    Ok { message: String },
    #[serde(rename = "error")]
    Error { message: String },
    #[serde(rename = "devices")]
    Devices { devices: Vec<DeviceInfo> },
    #[serde(rename = "status")]
    Status {
        state: String,
        progress: Option<f32>,
    },
}

#[derive(Serialize, Deserialize, Debug, Clone)]
pub struct DeviceInfo {
    pub name: String,
    pub address: String,
    pub rssi: Option<i16>,
}

pub async fn send_request(request: IpcRequest) -> Result<IpcResponse> {
    let path = socket_path();

    let stream = match UnixStream::connect(&path).await {
        Ok(s) => s,
        Err(e) => {
            eprintln!("❌ 无法连接到守护进程: {}", e);
            eprintln!("   请确保 cattysend-daemon 正在运行");
            eprintln!("   运行: cargo xtask dev 或 systemctl start cattysend");
            return Err(e.into());
        }
    };

    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);

    // 发送请求
    let json = serde_json::to_string(&request)?;
    writer.write_all(json.as_bytes()).await?;
    writer.write_all(b"\n").await?;

    // 读取响应
    let mut line = String::new();
    reader.read_line(&mut line).await?;

    let response: IpcResponse = serde_json::from_str(&line)?;

    match &response {
        IpcResponse::Ok { message } => println!("✅ {}", message),
        IpcResponse::Error { message } => eprintln!("❌ {}", message),
        _ => {}
    }

    Ok(response)
}
