//! Application state

use cattysend_core::{
    BleScanner, DiscoveredDevice, ReceiveEvent, ReceiveOptions, Receiver, SimpleReceiveCallback,
};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

/// Application operation mode
#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AppMode {
    Idle,
    Scanning,
    Receiving,
    #[allow(dead_code)] // Planned for future file sending feature
    Sending,
    Transferring,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum Tab {
    Devices,
    Transfer,
    Log,
}

/// 发送给 App 的异步事件
#[derive(Debug)]
pub enum AppEvent {
    DeviceFound(DiscoveredDevice),
    ScanFinished,
    StatusUpdate(String),
    ProgressUpdate {
        sent: u64,
        total: u64,
    },
    TransferComplete,
    Error(String),
    /// 日志消息（显示在日志面板）
    LogMessage {
        level: String,
        message: String,
    },
}

pub struct App {
    pub mode: AppMode,
    pub tab: Tab,
    pub devices: Vec<DiscoveredDevice>,
    pub selected_device: usize,
    pub progress: f64,
    pub transfer_speed: f64,
    pub logs: Vec<String>,
    pub scan_start: Option<Instant>,

    // 异步任务通信
    pub event_rx: mpsc::Receiver<AppEvent>,
    pub event_tx: mpsc::Sender<AppEvent>, // 用于克隆给 worker

    // 任务句柄
    pub active_task: Option<tokio::task::JoinHandle<()>>,
}

impl App {
    pub fn new() -> Self {
        let (event_tx, event_rx) = mpsc::channel(100);

        Self {
            mode: AppMode::Idle,
            tab: Tab::Devices,
            devices: vec![],
            selected_device: 0,
            progress: 0.0,
            transfer_speed: 0.0,
            logs: vec![
                "Cattysend TUI 启动".to_string(),
                "按 's' 扫描设备, 'r' 接收模式, 'q' 退出".to_string(),
            ],
            scan_start: None,
            event_rx,
            event_tx,
            active_task: None,
        }
    }

    pub fn start_scan(&mut self) {
        if self.mode == AppMode::Scanning {
            return;
        }

        self.mode = AppMode::Scanning;
        self.scan_start = Some(Instant::now());
        self.devices.clear();
        self.selected_device = 0;
        self.logs.push("开始扫描附近设备...".to_string());

        let tx = self.event_tx.clone();

        // 启动扫描任务
        tokio::spawn(async move {
            match BleScanner::new().await {
                Ok(scanner) => match scanner.scan(Duration::from_secs(10)).await {
                    Ok(devices) => {
                        for device in devices {
                            let _ = tx.send(AppEvent::DeviceFound(device)).await;
                        }
                        let _ = tx.send(AppEvent::ScanFinished).await;
                    }
                    Err(e) => {
                        let _ = tx.send(AppEvent::Error(format!("扫描失败: {}", e))).await;
                    }
                },
                Err(e) => {
                    let _ = tx
                        .send(AppEvent::Error(format!("无法初始化扫描器: {}", e)))
                        .await;
                }
            }
        });
    }

    pub fn handle_event(&mut self, event: AppEvent) {
        match event {
            AppEvent::DeviceFound(device) => {
                if !self.devices.iter().any(|d| d.address == device.address) {
                    self.devices.push(device);
                }
            }
            AppEvent::ScanFinished => {
                if self.mode == AppMode::Scanning {
                    self.mode = AppMode::Idle;
                    self.logs
                        .push(format!("扫描完成，发现 {} 个设备", self.devices.len()));
                }
            }
            AppEvent::StatusUpdate(msg) => {
                self.logs.push(msg);
            }
            AppEvent::ProgressUpdate { sent, total } => {
                self.progress = sent as f64 / total as f64;
                self.mode = AppMode::Transferring;
            }
            AppEvent::TransferComplete => {
                self.mode = AppMode::Idle;
                self.progress = 1.0;
                self.logs.push("传输任务已完成".to_string());
            }
            AppEvent::Error(msg) => {
                self.mode = AppMode::Idle;
                self.logs.push(format!("❌ {}", msg));
            }
            AppEvent::LogMessage { level, message } => {
                // 格式化日志消息并添加到日志列表
                let icon = match level.as_str() {
                    "ERROR" => "❌",
                    "WARN" => "⚠️",
                    "INFO" => "ℹ️",
                    "DEBUG" => "🔍",
                    "TRACE" => "📝",
                    _ => "•",
                };
                self.logs.push(format!("{} {}", icon, message));
                // 保持日志列表不超过 100 条
                if self.logs.len() > 100 {
                    self.logs.remove(0);
                }
            }
        }
    }

    pub fn toggle_receive_mode(&mut self) {
        if self.mode == AppMode::Receiving {
            if let Some(handle) = self.active_task.take() {
                handle.abort();
            }
            self.mode = AppMode::Idle;
            self.logs.push("停止接收模式".to_string());
            return;
        }

        self.mode = AppMode::Receiving;
        self.logs.push("进入接收模式，正在广播...".to_string());

        let tx = self.event_tx.clone();
        let options = ReceiveOptions::default();

        let handle = tokio::spawn(async move {
            match Receiver::new(options) {
                Ok(mut receiver) => {
                    let (callback, mut rx) = SimpleReceiveCallback::new(true); // auto_accept = true

                    // 转发回调事件到 App
                    let tx_clone = tx.clone();
                    tokio::spawn(async move {
                        while let Some(event) = rx.recv().await {
                            match event {
                                ReceiveEvent::Status(s) => {
                                    let _ = tx_clone.send(AppEvent::StatusUpdate(s)).await;
                                }
                                ReceiveEvent::Progress { received, total } => {
                                    let _ = tx_clone
                                        .send(AppEvent::ProgressUpdate {
                                            sent: received,
                                            total,
                                        })
                                        .await;
                                }
                                ReceiveEvent::Complete(_) => {
                                    let _ = tx_clone.send(AppEvent::TransferComplete).await;
                                }
                                ReceiveEvent::Error(e) => {
                                    let _ = tx_clone.send(AppEvent::Error(e)).await;
                                }
                                _ => {}
                            }
                        }
                    });

                    if let Err(e) = receiver.start(&callback).await {
                        let _ = tx
                            .send(AppEvent::Error(format!("接收流程出错: {}", e)))
                            .await;
                    }
                }
                Err(e) => {
                    let _ = tx
                        .send(AppEvent::Error(format!("无法初始化接收器: {}", e)))
                        .await;
                }
            }
        });

        self.active_task = Some(handle);
    }

    pub fn next_device(&mut self) {
        if !self.devices.is_empty() {
            self.selected_device = (self.selected_device + 1) % self.devices.len();
        }
    }

    pub fn previous_device(&mut self) {
        if !self.devices.is_empty() {
            self.selected_device = self
                .selected_device
                .checked_sub(1)
                .unwrap_or(self.devices.len() - 1);
        }
    }

    pub fn select_device(&mut self) {
        if let Some(device) = self.devices.get(self.selected_device) {
            self.logs
                .push(format!("选中设备: {} ({})", device.name, device.address));
            // TODO: 这里应弹出文件选择，目前先占位
            self.logs.push("发送功能尚在完善中".to_string());
        }
    }

    pub fn next_tab(&mut self) {
        self.tab = match self.tab {
            Tab::Devices => Tab::Transfer,
            Tab::Transfer => Tab::Log,
            Tab::Log => Tab::Devices,
        };
    }

    pub fn tick(&mut self) {
        while let Ok(event) = self.event_rx.try_recv() {
            self.handle_event(event);
        }
    }
}
