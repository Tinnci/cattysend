//! IPC Server - Unix Domain Socket 通信

use anyhow::Result;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use tokio::io::{AsyncBufReadExt, AsyncWriteExt, BufReader};
use tokio::net::{UnixListener, UnixStream};

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

pub async fn run_ipc_server() -> Result<()> {
    let path = socket_path();

    // 删除旧的 socket 文件
    let _ = std::fs::remove_file(&path);

    let listener = UnixListener::bind(&path)?;
    tracing::info!("IPC 服务器已启动: {:?}", path);

    loop {
        match listener.accept().await {
            Ok((stream, _)) => {
                tokio::spawn(handle_client(stream));
            }
            Err(e) => {
                tracing::warn!("接受连接失败: {}", e);
            }
        }
    }
}

async fn handle_client(stream: UnixStream) -> Result<()> {
    let (reader, mut writer) = stream.into_split();
    let mut reader = BufReader::new(reader);
    let mut line = String::new();

    while reader.read_line(&mut line).await? > 0 {
        let request: IpcRequest = match serde_json::from_str(&line) {
            Ok(req) => req,
            Err(e) => {
                let resp = IpcResponse::Error {
                    message: format!("Invalid request: {}", e),
                };
                writer
                    .write_all(serde_json::to_string(&resp)?.as_bytes())
                    .await?;
                writer.write_all(b"\n").await?;
                line.clear();
                continue;
            }
        };

        tracing::debug!("收到请求: {:?}", request);

        let response = match request {
            IpcRequest::Status => IpcResponse::Status {
                state: "idle".to_string(),
                progress: None,
            },
            IpcRequest::Scan { timeout_secs } => {
                tracing::info!("开始扫描设备 ({}s)...", timeout_secs);
                // TODO: 调用 cattysend_core::ble::scanner
                IpcResponse::Devices { devices: vec![] }
            }
            IpcRequest::Send {
                file_path,
                device_addr,
            } => {
                tracing::info!("发送文件: {} -> {:?}", file_path, device_addr);
                IpcResponse::Ok {
                    message: "发送任务已启动".to_string(),
                }
            }
            IpcRequest::Receive => {
                tracing::info!("进入接收模式");
                IpcResponse::Ok {
                    message: "接收模式已启动".to_string(),
                }
            }
            IpcRequest::Stop => {
                tracing::info!("停止当前任务");
                IpcResponse::Ok {
                    message: "已停止".to_string(),
                }
            }
        };

        writer
            .write_all(serde_json::to_string(&response)?.as_bytes())
            .await?;
        writer.write_all(b"\n").await?;
        line.clear();
    }

    Ok(())
}
