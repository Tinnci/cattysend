//! 接收端工作流
//!
//! 高层 API 封装完整的接收流程:
//! 1. 启动 GATT Server 等待连接
//! 2. 接收 P2P 信息
//! 3. 连接到发送端 WiFi 热点
//! 4. 通过 HTTP/WebSocket 接收文件

use crate::ble::GattServer;
use crate::crypto::BleSecurityPersistent;
use crate::transfer::{ReceiverCallback, ReceiverClient, SendRequest};
use crate::wifi::WiFiP2pReceiver;
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::mpsc;

/// 接收进度回调
pub trait ReceiveProgressCallback: Send + Sync {
    /// 状态更新
    fn on_status(&self, status: &str);
    /// 收到发送请求，返回是否接受
    fn on_request(&self, request: &ReceiveRequest) -> bool;
    /// 进度更新
    fn on_progress(&self, received: u64, total: u64);
    /// 接收完成
    fn on_complete(&self, files: Vec<PathBuf>);
    /// 接收失败
    fn on_error(&self, error: &str);
}

/// 接收请求信息
#[derive(Debug, Clone)]
pub struct ReceiveRequest {
    pub sender_name: String,
    pub file_name: String,
    pub file_count: u32,
    pub total_size: u64,
}

/// 接收选项
pub struct ReceiveOptions {
    /// 设备名称 (在 BLE 广播中显示)
    pub device_name: String,
    /// WiFi 接口名称
    pub wifi_interface: String,
    /// 文件保存目录
    pub output_dir: PathBuf,
    /// 是否自动接受
    pub auto_accept: bool,
    /// 厂商 ID
    pub brand_id: crate::config::BrandId,
    /// 是否支持 5GHz
    pub supports_5ghz: bool,
}

impl Default for ReceiveOptions {
    fn default() -> Self {
        Self {
            device_name: hostname::get()
                .map(|h| h.to_string_lossy().to_string())
                .unwrap_or_else(|_| "Cattysend".to_string()),
            wifi_interface: "wlan0".to_string(),
            output_dir: dirs::download_dir().unwrap_or_else(|| PathBuf::from(".")),
            auto_accept: false,
            brand_id: crate::config::BrandId::Linux,
            supports_5ghz: true,
        }
    }
}

/// 接收端工作流
pub struct Receiver {
    options: ReceiveOptions,
    security: Arc<BleSecurityPersistent>,
}

impl Receiver {
    pub fn new(options: ReceiveOptions) -> anyhow::Result<Self> {
        let security = Arc::new(BleSecurityPersistent::new()?);
        Ok(Self { options, security })
    }

    /// 开始接收模式
    pub async fn start<C: ReceiveProgressCallback>(
        &self,
        callback: &C,
    ) -> anyhow::Result<Vec<PathBuf>> {
        callback.on_status("启动接收模式...");

        // 获取 MAC 地址
        let mac = self.get_mac_address();

        // 启动 GATT Server
        let mut gatt_server = GattServer::new(
            mac,
            self.options.device_name.clone(),
            self.security.get_public_key().to_string(),
        )?
        .with_security(self.security.clone())
        .with_brand(self.options.brand_id)
        .with_5ghz_support(self.options.supports_5ghz);
        let mut p2p_rx = gatt_server.take_p2p_receiver().unwrap();

        let _handle = gatt_server.start().await?;

        callback.on_status(&format!(
            "正在广播为 '{}'，等待发送端连接...",
            self.options.device_name
        ));

        // 等待 P2P 信息
        let p2p_event = p2p_rx
            .recv()
            .await
            .ok_or_else(|| anyhow::anyhow!("P2P channel closed"))?;

        // P2P 信息已由 GattServer 自动解密（如果提供了公钥）
        let p2p_info = p2p_event.p2p_info;

        if p2p_event.sender_public_key.is_some() {
            callback.on_status("已接收并解密 P2P 信息");
        } else {
            callback.on_status("已接收 P2P 信息");
        }

        callback.on_status(&format!("连接到 WiFi: {}", p2p_info.ssid));

        // 连接到 WiFi P2P 热点（支持双连接）
        let mut wifi_receiver = WiFiP2pReceiver::new(&self.options.wifi_interface);
        let local_ip = wifi_receiver.connect(&p2p_info).await?;

        // 显示连接状态
        if wifi_receiver.is_dual_connected().await {
            callback.on_status(&format!("✅ 已连接（双连接模式），本地 IP: {}", local_ip));
        } else {
            callback.on_status(&format!("✅ 已连接，本地 IP: {}", local_ip));
        }

        // 计算发送端 IP (通常是网关)
        let sender_ip = self.get_gateway_ip(&local_ip);

        callback.on_status(&format!(
            "连接到 WebSocket: wss://{}:{}/websocket",
            sender_ip, p2p_info.port
        ));

        // 创建接收适配器
        let adapter = ReceiverCallbackAdapter {
            callback,
            auto_accept: self.options.auto_accept,
        };

        // 接收文件
        let client = ReceiverClient::new(
            &sender_ip,
            p2p_info.port as u16,
            self.options.output_dir.clone(),
        );

        let files = client.start(&adapter).await?;

        // 断开 WiFi 并清理虚拟接口
        wifi_receiver.disconnect().await?;

        callback.on_complete(files.clone());

        Ok(files)
    }

    /// 获取 MAC 地址
    fn get_mac_address(&self) -> String {
        let path = format!("/sys/class/net/{}/address", self.options.wifi_interface);
        std::fs::read_to_string(&path)
            .map(|s| s.trim().to_uppercase())
            .unwrap_or_else(|_| "02:00:00:00:00:00".to_string())
    }

    /// 从本地 IP 推断网关 IP
    fn get_gateway_ip(&self, local_ip: &str) -> String {
        // 通常网关是 x.x.x.1
        let parts: Vec<&str> = local_ip.split('.').collect();
        if parts.len() == 4 {
            format!("{}.{}.{}.1", parts[0], parts[1], parts[2])
        } else {
            "192.168.49.1".to_string()
        }
    }
}

/// 接收回调适配器
struct ReceiverCallbackAdapter<'a, C: ReceiveProgressCallback> {
    callback: &'a C,
    auto_accept: bool,
}

impl<C: ReceiveProgressCallback> ReceiverCallback for ReceiverCallbackAdapter<'_, C> {
    fn on_send_request(&self, request: &SendRequest) -> bool {
        if self.auto_accept {
            return true;
        }

        let req = ReceiveRequest {
            sender_name: request.sender_name.clone(),
            file_name: request.file_name.clone(),
            file_count: request.file_count,
            total_size: request.total_size,
        };

        self.callback.on_request(&req)
    }

    fn on_progress(&self, received: u64, total: u64) {
        self.callback.on_progress(received, total);
    }

    fn on_complete(&self, files: Vec<PathBuf>) {
        self.callback.on_complete(files);
    }

    fn on_error(&self, error: String) {
        self.callback.on_error(&error);
    }
}

/// 简化的接收回调实现
pub struct SimpleReceiveCallback {
    tx: mpsc::Sender<ReceiveEvent>,
    auto_accept: bool,
}

#[derive(Debug, Clone)]
pub enum ReceiveEvent {
    Status(String),
    Request(ReceiveRequest),
    Progress { received: u64, total: u64 },
    Complete(Vec<PathBuf>),
    Error(String),
}

impl SimpleReceiveCallback {
    pub fn new(auto_accept: bool) -> (Self, mpsc::Receiver<ReceiveEvent>) {
        let (tx, rx) = mpsc::channel(32);
        (Self { tx, auto_accept }, rx)
    }
}

impl ReceiveProgressCallback for SimpleReceiveCallback {
    fn on_status(&self, status: &str) {
        let _ = self.tx.try_send(ReceiveEvent::Status(status.to_string()));
    }

    fn on_request(&self, request: &ReceiveRequest) -> bool {
        let _ = self.tx.try_send(ReceiveEvent::Request(request.clone()));
        self.auto_accept
    }

    fn on_progress(&self, received: u64, total: u64) {
        let _ = self.tx.try_send(ReceiveEvent::Progress { received, total });
    }

    fn on_complete(&self, files: Vec<PathBuf>) {
        let _ = self.tx.try_send(ReceiveEvent::Complete(files));
    }

    fn on_error(&self, error: &str) {
        let _ = self.tx.try_send(ReceiveEvent::Error(error.to_string()));
    }
}
