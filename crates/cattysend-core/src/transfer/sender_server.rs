//! HTTP/HTTPS 传输服务器
//!
//! 与 CatShare 兼容的文件传输服务。
//!
//! # 功能
//!
//! - HTTPS WebSocket 用于协商和状态同步
//! - HTTPS GET /download 用于 ZIP 文件下载
//!
//! # 协议
//!
//! 使用自定义文本协议 `type:id:name?payload`

use log::{debug, error, info, warn};

use crate::transfer::protocol::WsMessage;
use axum::{
    Router,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use std::io::Write;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::fs::File;
use tokio::io::AsyncReadExt;
use tokio::net::TcpListener;
use tokio::sync::{Mutex, broadcast};
use tokio_tungstenite::tungstenite::Message;

#[derive(Deserialize)]
pub struct DownloadQuery {
    #[serde(rename = "taskId")]
    pub task_id: String,
}

/// 传输任务
#[derive(Debug, Clone)]
pub struct TransferTask {
    pub task_id: String,
    pub files: Vec<FileEntry>,
    pub sender_id: String,
    pub sender_name: String,
}

#[derive(Debug, Clone)]
pub struct FileEntry {
    pub path: PathBuf,
    pub name: String,
    pub size: u64,
    pub mime_type: String,
}

/// 传输状态
#[derive(Debug, Clone)]
pub enum TransferStatus {
    Pending,
    Accepted,
    Rejected(String),
    Transferring { progress: f64 },
    Completed,
    Failed(String),
}

/// 服务器状态
pub struct TransferServerState {
    pub task: TransferTask,
    pub status_tx: broadcast::Sender<TransferStatus>,
}

/// 传输服务器
pub struct TransferServer {
    port: u16,
    state: Arc<Mutex<TransferServerState>>,
}

impl TransferServer {
    pub fn new(task: TransferTask) -> Self {
        let (status_tx, _) = broadcast::channel(16);

        Self {
            port: 0, // 使用随机端口
            state: Arc::new(Mutex::new(TransferServerState { task, status_tx })),
        }
    }

    /// 获取分配的端口
    pub fn port(&self) -> u16 {
        self.port
    }

    /// 订阅传输状态更新
    pub fn subscribe_status(&self) -> broadcast::Receiver<TransferStatus> {
        let state = self.state.blocking_lock();
        state.status_tx.subscribe()
    }

    /// 异步订阅传输状态更新
    pub async fn subscribe_status_async(&self) -> broadcast::Receiver<TransferStatus> {
        let state = self.state.lock().await;
        state.status_tx.subscribe()
    }

    /// 启动服务器（HTTP 版本，用于测试）
    pub async fn start(&mut self) -> anyhow::Result<u16> {
        let state = self.state.clone();

        let app = Router::new()
            .route("/download", get(download_handler))
            .with_state(state);

        let listener = TcpListener::bind("0.0.0.0:0").await?;
        let port = listener.local_addr()?.port();
        self.port = port;

        info!("Transfer server listening on port {}", port);

        tokio::spawn(async move {
            if let Err(e) = axum::serve(listener, app).await {
                error!("Server error: {}", e);
            }
        });

        Ok(port)
    }

    /// 启动 WebSocket + HTTP 服务器
    pub async fn start_with_websocket(&mut self) -> anyhow::Result<u16> {
        let state = self.state.clone();
        let state_for_ws = self.state.clone();

        // HTTP 服务器
        let app = Router::new()
            .route("/download", get(download_handler))
            .with_state(state);

        let http_listener = TcpListener::bind("0.0.0.0:0").await?;
        let port = http_listener.local_addr()?.port();
        self.port = port;

        // 启动 HTTP 服务器
        tokio::spawn(async move {
            if let Err(e) = axum::serve(http_listener, app).await {
                error!("HTTP Server error: {}", e);
            }
        });

        // WebSocket 服务器（在同一端口使用不同路径）
        // 注意：在生产环境中应该合并到一个服务器
        let ws_listener = TcpListener::bind(format!("0.0.0.0:{}", port + 1)).await?;
        let ws_port = ws_listener.local_addr()?.port();

        tokio::spawn(async move {
            while let Ok((stream, _)) = ws_listener.accept().await {
                let state = state_for_ws.clone();
                tokio::spawn(async move {
                    if let Err(e) = handle_websocket_connection(stream, state).await {
                        error!("WebSocket error: {}", e);
                    }
                });
            }
        });

        info!(
            "Transfer server started: HTTP={}, WebSocket={}",
            port, ws_port
        );

        Ok(port)
    }
}

/// 处理 WebSocket 连接
async fn handle_websocket_connection(
    stream: tokio::net::TcpStream,
    state: Arc<Mutex<TransferServerState>>,
) -> anyhow::Result<()> {
    let ws_stream = tokio_tungstenite::accept_async(stream).await?;
    let (mut write, mut read) = ws_stream.split();

    let mut msg_id: u32 = 0;

    // 发送版本协商
    let ver_msg = WsMessage::version_negotiation(msg_id);
    write.send(Message::Text(ver_msg.to_string())).await?;

    // 处理消息
    while let Some(msg) = read.next().await {
        let msg = match msg {
            Ok(Message::Text(text)) => text.to_string(),
            Ok(Message::Close(_)) => break,
            Err(e) => {
                error!("WebSocket read error: {}", e);
                break;
            }
            _ => continue,
        };

        let ws_msg = match WsMessage::parse(&msg) {
            Some(m) => m,
            None => {
                warn!("Invalid WebSocket message: {}", msg);
                continue;
            }
        };

        debug!(
            "WS received: type={}, name={}",
            ws_msg.msg_type, ws_msg.name
        );

        match ws_msg.msg_type.as_str() {
            "ack" => {
                if ws_msg.name == "versionNegotiation" {
                    // 版本协商完成，发送传输请求
                    msg_id += 1;
                    let task = {
                        let s = state.lock().await;
                        s.task.clone()
                    };

                    let total_size: u64 = task.files.iter().map(|f| f.size).sum();
                    let file_name = task
                        .files
                        .first()
                        .map(|f| f.name.clone())
                        .unwrap_or_default();

                    let send_req = WsMessage::action(
                        msg_id,
                        "sendRequest",
                        Some(serde_json::json!({
                            "taskId": task.task_id,
                            "id": task.task_id,
                            "senderId": task.sender_id,
                            "senderName": task.sender_name,
                            "fileName": file_name,
                            "mimeType": task.files.first().map(|f| &f.mime_type).unwrap_or(&"application/octet-stream".to_string()),
                            "fileCount": task.files.len(),
                            "totalSize": total_size
                        })),
                    );
                    write.send(Message::Text(send_req.to_string())).await?;
                }
            }
            "action" => {
                // 发送 ACK
                let ack = WsMessage::ack(ws_msg.id, &ws_msg.name, None);
                write.send(Message::Text(ack.to_string())).await?;

                if ws_msg.name == "status"
                    && let Some(payload) = &ws_msg.payload
                {
                    let status_type = payload.get("type").and_then(|v| v.as_i64()).unwrap_or(0);
                    if status_type == 1 {
                        // 传输完成
                        info!("Transfer completed successfully");
                        let _ = state.lock().await.status_tx.send(TransferStatus::Completed);
                        break;
                    } else if status_type == 3 {
                        // 用户拒绝
                        info!("Transfer rejected by receiver");
                        let reason = payload
                            .get("reason")
                            .and_then(|v| v.as_str())
                            .unwrap_or("rejected");
                        let _ = state
                            .lock()
                            .await
                            .status_tx
                            .send(TransferStatus::Rejected(reason.to_string()));
                        break;
                    }
                }
            }
            _ => {}
        }
    }

    Ok(())
}

/// 文件下载处理器
async fn download_handler(
    Query(query): Query<DownloadQuery>,
    State(state): State<Arc<Mutex<TransferServerState>>>,
) -> impl IntoResponse {
    let task = {
        let s = state.lock().await;
        if s.task.task_id != query.task_id {
            return (StatusCode::NOT_FOUND, "Task not found").into_response();
        }
        s.task.clone()
    };

    info!("Download request for task_id={}", task.task_id);

    // 创建 ZIP 文件
    match create_zip_response(&task.files).await {
        Ok(data) => {
            let headers = [
                ("Content-Type", "application/zip"),
                ("Content-Disposition", "attachment; filename=\"files.zip\""),
            ];
            (headers, data).into_response()
        }
        Err(e) => {
            error!("Failed to create ZIP: {}", e);
            (StatusCode::INTERNAL_SERVER_ERROR, "Failed to create ZIP").into_response()
        }
    }
}

async fn create_zip_response(files: &[FileEntry]) -> anyhow::Result<Vec<u8>> {
    let mut buffer = Vec::new();

    {
        let mut zip = zip::ZipWriter::new(std::io::Cursor::new(&mut buffer));
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);

        for (i, file) in files.iter().enumerate() {
            let entry_name = format!("{}/{}", i, file.name);
            zip.start_file(&entry_name, options)?;

            let mut f = File::open(&file.path).await?;
            let mut contents = Vec::new();
            f.read_to_end(&mut contents).await?;
            zip.write_all(&contents)?;
        }

        zip.finish()?;
    }

    Ok(buffer)
}
