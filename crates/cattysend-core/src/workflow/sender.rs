//! 发送端工作流
//!
//! 高层 API 封装完整的发送流程:
//! 1. 创建 WiFi P2P 热点
//! 2. 启动 HTTP 传输服务器
//! 3. 通过 BLE 连接接收端并发送 P2P 信息
//! 4. 等待接收端连接和下载文件

use crate::ble::{BleClient, DiscoveredDevice};
use crate::transfer::{FileEntry, TransferServer, TransferTask};
use crate::wifi::{P2pConfig, WiFiP2pSender};
use std::path::PathBuf;
use tokio::sync::mpsc;

/// 发送进度回调
pub trait SendProgressCallback: Send + Sync {
    /// 状态更新
    fn on_status(&self, status: &str);
    /// 进度更新
    fn on_progress(&self, sent: u64, total: u64);
    /// 发送完成
    fn on_complete(&self);
    /// 发送失败
    fn on_error(&self, error: &str);
}

/// 发送选项
pub struct SendOptions {
    /// WiFi 接口名称
    pub wifi_interface: String,
    /// 是否使用 5GHz
    pub use_5ghz: bool,
    /// 发送者名称
    pub sender_name: String,
}

impl Default for SendOptions {
    fn default() -> Self {
        Self {
            wifi_interface: "wlan0".to_string(),
            use_5ghz: true,
            sender_name: hostname::get()
                .map(|h| h.to_string_lossy().to_string())
                .unwrap_or_else(|_| "Cattysend".to_string()),
        }
    }
}

/// 发送端工作流
pub struct Sender {
    options: SendOptions,
    wifi_sender: WiFiP2pSender,
}

impl Sender {
    pub fn new(options: SendOptions) -> Self {
        let wifi_sender = WiFiP2pSender::with_config(P2pConfig {
            interface: options.wifi_interface.clone(),
            use_5ghz: options.use_5ghz,
            ..Default::default()
        });

        Self {
            options,
            wifi_sender,
        }
    }

    /// 发送文件到指定设备
    pub async fn send_to_device<C: SendProgressCallback>(
        &self,
        device: &DiscoveredDevice,
        files: Vec<PathBuf>,
        callback: &C,
    ) -> anyhow::Result<()> {
        callback.on_status("准备发送...");

        // 准备文件信息
        let mut file_entries = Vec::new();
        let mut _total_size: u64 = 0;

        for path in &files {
            let metadata = tokio::fs::metadata(path).await?;
            let name = path
                .file_name()
                .map(|n| n.to_string_lossy().to_string())
                .unwrap_or_else(|| "unknown".to_string());
            let size = metadata.len();
            _total_size += size;

            // 猜测 MIME 类型
            let mime_type = mime_guess::from_path(path)
                .first()
                .map(|m| m.to_string())
                .unwrap_or_else(|| "application/octet-stream".to_string());

            file_entries.push(FileEntry {
                path: path.clone(),
                name,
                size,
                mime_type,
            });
        }

        callback.on_status("创建 WiFi 热点...");

        // 创建传输任务
        let task_id = uuid::Uuid::new_v4().to_string();
        let sender_id = format!("{:04x}", rand::random::<u16>());

        let task = TransferTask {
            task_id: task_id.clone(),
            files: file_entries,
            sender_id: sender_id.clone(),
            sender_name: self.options.sender_name.clone(),
        };

        // 启动传输服务器
        let mut server = TransferServer::new(task);
        let port = server.start().await?;

        callback.on_status(&format!("服务器启动于端口 {}", port));

        // 创建 WiFi P2P 热点
        let p2p_info = self.wifi_sender.create_group(port as i32).await?;

        callback.on_status(&format!("热点已创建: {}", p2p_info.ssid));

        // 连接到接收端 BLE 设备
        callback.on_status("连接到接收端...");

        let ble_client = BleClient::new().await?;
        let _device_info = ble_client
            .connect_and_handshake(&device.address, &p2p_info, &sender_id)
            .await?;

        callback.on_status("等待接收端连接...");

        // 等待传输完成
        // TODO: 监听传输状态并更新进度

        // 简单等待一段时间让传输完成
        tokio::time::sleep(std::time::Duration::from_secs(30)).await;

        // 清理
        self.wifi_sender.stop_group().await?;

        callback.on_complete();

        Ok(())
    }
}

/// 简化的发送回调实现
pub struct SimpleSendCallback {
    tx: mpsc::Sender<SendEvent>,
}

#[derive(Debug, Clone)]
pub enum SendEvent {
    Status(String),
    Progress { sent: u64, total: u64 },
    Complete,
    Error(String),
}

impl SimpleSendCallback {
    pub fn new() -> (Self, mpsc::Receiver<SendEvent>) {
        let (tx, rx) = mpsc::channel(32);
        (Self { tx }, rx)
    }
}

impl SendProgressCallback for SimpleSendCallback {
    fn on_status(&self, status: &str) {
        let _ = self.tx.try_send(SendEvent::Status(status.to_string()));
    }

    fn on_progress(&self, sent: u64, total: u64) {
        let _ = self.tx.try_send(SendEvent::Progress { sent, total });
    }

    fn on_complete(&self) {
        let _ = self.tx.try_send(SendEvent::Complete);
    }

    fn on_error(&self, error: &str) {
        let _ = self.tx.try_send(SendEvent::Error(error.to_string()));
    }
}
