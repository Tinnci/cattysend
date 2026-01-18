//! HTTP/HTTPS 接收客户端
//!
//! 与 CatShare 兼容的文件接收客户端。
//!
//! # 功能
//!
//! - 连接发送端的 HTTPS WebSocket
//! - 协商版本和处理发送请求
//! - 下载 ZIP 文件并解压
//!
//! # 安全性
//!
//! - 使用 HTTPS 传输（跳过证书验证，因为发送端使用自签名证书）
//! - WebSocket 协议用于状态同步

use log::{debug, error, info, warn};

use crate::transfer::protocol::{SendRequest, WsMessage};
use futures_util::{SinkExt, StreamExt};
use std::io::Read;
use std::path::PathBuf;
use tokio::fs::{File, create_dir_all};
use tokio::io::AsyncWriteExt;
use tokio_tungstenite::tungstenite::Message;

/// 接收事件回调
pub trait ReceiverCallback: Send + Sync {
    /// 收到发送请求，返回是否接受
    fn on_send_request(&self, request: &SendRequest) -> bool;

    /// 进度更新
    fn on_progress(&self, received: u64, total: u64);

    /// 接收完成
    fn on_complete(&self, files: Vec<PathBuf>);

    /// 接收失败
    fn on_error(&self, error: String);
}

/// 文件接收客户端
pub struct ReceiverClient {
    host: String,
    port: u16,
    output_dir: PathBuf,
}

impl ReceiverClient {
    pub fn new(host: &str, port: u16, output_dir: PathBuf) -> Self {
        Self {
            host: host.to_string(),
            port,
            output_dir,
        }
    }

    /// 开始接收
    pub async fn start<C: ReceiverCallback>(&self, callback: &C) -> anyhow::Result<Vec<PathBuf>> {
        // 创建输出目录
        create_dir_all(&self.output_dir).await?;

        // 连接 WebSocket (不验证证书)
        let ws_url = format!("wss://{}:{}/websocket", self.host, self.port);
        info!("Connecting to WebSocket: {}", ws_url);

        // 使用不验证证书的 TLS 配置
        let connector = native_tls::TlsConnector::builder()
            .danger_accept_invalid_certs(true)
            .build()?;
        let connector = tokio_native_tls::TlsConnector::from(connector);

        // 建立 TCP 连接
        let tcp_stream =
            tokio::net::TcpStream::connect(format!("{}:{}", self.host, self.port)).await?;

        // TLS 握手
        let tls_stream = connector.connect(&self.host, tcp_stream).await?;

        // WebSocket 握手
        let (ws_stream, _) = tokio_tungstenite::client_async(&ws_url, tls_stream).await?;

        let (mut write, mut read) = ws_stream.split();

        let mut msg_id: u32 = 0;
        let mut task_id: Option<String> = None;
        let mut total_size: u64 = 0;

        // 消息循环
        while let Some(msg) = read.next().await {
            let msg = match msg {
                Ok(Message::Text(text)) => text.to_string(),
                Ok(Message::Close(_)) => break,
                Err(e) => {
                    callback.on_error(format!("WebSocket error: {}", e));
                    return Err(e.into());
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

            match ws_msg.name.as_str() {
                "versionNegotiation" => {
                    // 版本协商
                    let ack = WsMessage::ack(
                        ws_msg.id,
                        "versionNegotiation",
                        Some(serde_json::json!({
                            "version": 1,
                            "threadLimit": 5
                        })),
                    );
                    write.send(Message::Text(ack.to_string())).await?;
                }

                "sendRequest" => {
                    if let Some(payload) = ws_msg.payload {
                        debug!("sendRequest payload: {}", payload);
                        let request: SendRequest = match serde_json::from_value(payload.clone()) {
                            Ok(req) => req,
                            Err(e) => {
                                error!("Failed to parse sendRequest: {}. Payload: {}", e, payload);
                                return Err(anyhow::anyhow!("Protocol error: {}", e));
                            }
                        };
                        total_size = request.total_size;

                        // 获取任务 ID
                        let req_task_id = request.get_task_id();

                        // 询问用户是否接受
                        if callback.on_send_request(&request) {
                            task_id = Some(req_task_id.clone());

                            // 发送 ACK
                            let ack = WsMessage::ack(ws_msg.id, "sendRequest", None);
                            write.send(Message::Text(ack.to_string())).await?;

                            // 开始下载
                            break;
                        } else {
                            // 拒绝
                            msg_id += 1;
                            let status = WsMessage::status(msg_id, &req_task_id, 3, "user refuse");
                            write.send(Message::Text(status.to_string())).await?;
                            return Err(anyhow::anyhow!("User rejected transfer"));
                        }
                    }
                }

                _ => {
                    // 发送 ACK
                    let ack = WsMessage::ack(ws_msg.id, &ws_msg.name, None);
                    write.send(Message::Text(ack.to_string())).await?;
                }
            }
        }

        // 下载文件
        let task_id = task_id.ok_or_else(|| anyhow::anyhow!("No task ID received"))?;
        let download_url = format!(
            "https://{}:{}/download?taskId={}",
            self.host, self.port, task_id
        );

        info!("Downloading file from: {}", download_url);

        // 使用不验证证书的 HTTP 客户端
        let client = reqwest::Client::builder()
            .danger_accept_invalid_certs(true)
            .build()?;

        let response = client.get(&download_url).send().await?;
        let zip_bytes = response.bytes().await?;

        // 解压 ZIP
        let files = self.extract_zip(&zip_bytes, callback, total_size).await?;

        // 发送完成状态
        msg_id += 1;
        let status = WsMessage::status(msg_id, &task_id, 1, "ok");
        write.send(Message::Text(status.to_string())).await?;

        callback.on_complete(files.clone());

        Ok(files)
    }

    async fn extract_zip<C: ReceiverCallback>(
        &self,
        data: &[u8],
        callback: &C,
        total_size: u64,
    ) -> anyhow::Result<Vec<PathBuf>> {
        let cursor = std::io::Cursor::new(data);
        let mut archive = zip::ZipArchive::new(cursor)?;

        let mut received: u64 = 0;
        let mut files = Vec::new();

        for i in 0..archive.len() {
            // 读取并写入 (先读到内存，释放 zip 文件句柄避免跨 await)
            let (filename, buffer, is_dir) = {
                let mut file = archive.by_index(i)?;
                let is_dir = file.is_dir();
                let name = file.name().to_string();
                let filename = name.split('/').next_back().unwrap_or(&name).to_string();
                let mut buffer = Vec::new();
                if !is_dir {
                    file.read_to_end(&mut buffer)?;
                }
                (filename, buffer, is_dir)
            };

            if is_dir {
                continue;
            }

            let output_path = self.output_dir.join(filename);
            let mut output_file = File::create(&output_path).await?;
            output_file.write_all(&buffer).await?;

            received += buffer.len() as u64;
            callback.on_progress(received, total_size);

            files.push(output_path);
        }

        Ok(files)
    }
}
