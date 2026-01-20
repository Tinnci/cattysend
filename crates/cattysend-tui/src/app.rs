//! Application state

use cattysend_core::{
    AppSettings, BleScanner, DiscoveredDevice, ReceiveEvent, ReceiveOptions, Receiver,
    ScanCallback, SendOptions, Sender, SimpleReceiveCallback, SimpleSendCallback,
};
use std::sync::Arc;
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

/// 日志级别过滤
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum LogLevel {
    Error = 0,
    Warn = 1,
    Info = 2,
    Debug = 3,
    Trace = 4,
}

impl LogLevel {
    pub fn from_str(s: &str) -> Self {
        match s.to_uppercase().as_str() {
            "ERROR" => LogLevel::Error,
            "WARN" => LogLevel::Warn,
            "INFO" => LogLevel::Info,
            "DEBUG" => LogLevel::Debug,
            "TRACE" => LogLevel::Trace,
            _ => LogLevel::Info,
        }
    }

    pub fn name(&self) -> &'static str {
        match self {
            LogLevel::Error => "ERROR",
            LogLevel::Warn => "WARN",
            LogLevel::Info => "INFO",
            LogLevel::Debug => "DEBUG",
            LogLevel::Trace => "TRACE",
        }
    }

    pub fn icon(&self) -> &'static str {
        match self {
            LogLevel::Error => "❌",
            LogLevel::Warn => "⚠️",
            LogLevel::Info => "ℹ️",
            LogLevel::Debug => "🔍",
            LogLevel::Trace => "📝",
        }
    }
}

/// 带级别的日志条目
#[derive(Debug, Clone)]
pub struct LogEntry {
    pub level: LogLevel,
    pub message: String,
}

pub struct App {
    pub mode: AppMode,
    pub tab: Tab,
    pub devices: Vec<DiscoveredDevice>,
    pub selected_device: usize,
    pub progress: f64,
    pub transfer_speed: f64,
    pub file_to_send: Option<String>,

    /// 原始日志列表（所有级别）
    raw_logs: Vec<LogEntry>,
    /// 当前显示的日志级别过滤器
    pub log_filter: LogLevel,

    pub scan_start: Option<Instant>,

    // 异步任务通信
    pub event_rx: mpsc::Receiver<AppEvent>,
    pub event_tx: mpsc::Sender<AppEvent>,

    // 任务句柄
    pub active_task: Option<tokio::task::JoinHandle<()>>,

    // 权限状态
    pub has_nmcli: bool,
    pub has_net_raw: bool,
    pub show_perm_warning: bool,

    // 应用设置
    pub settings: AppSettings,
}

impl App {
    pub fn new() -> Self {
        let (event_tx, event_rx) = mpsc::channel(100);
        let (has_nmcli, has_net_raw) = cattysend_core::wifi::check_capabilities();

        let settings = AppSettings::load();

        let mut app = Self {
            mode: AppMode::Idle,
            tab: Tab::Devices,
            devices: vec![],
            selected_device: 0,
            progress: 0.0,
            transfer_speed: 0.0,
            file_to_send: None,
            raw_logs: vec![],
            log_filter: LogLevel::Info,
            scan_start: None,
            event_rx,
            event_tx,
            active_task: None,
            has_nmcli,
            has_net_raw,
            show_perm_warning: !has_nmcli || !has_net_raw,
            settings,
        };

        // 添加初始消息
        app.add_log(LogLevel::Info, "Cattysend TUI 启动".to_string());
        app.add_log(
            LogLevel::Info,
            format!(
                "配置已加载: 设备名='{}', 厂商='{}', 5GHz={}",
                app.settings.device_name,
                app.settings.brand_id.name(),
                app.settings.supports_5ghz
            ),
        );

        if app.show_perm_warning {
            if !app.has_nmcli {
                app.add_log(
                    LogLevel::Warn,
                    "⚠️ 系统缺少 nmcli，双连接功能将不可用。".to_string(),
                );
            }
            if !app.has_net_raw {
                app.add_log(
                    LogLevel::Warn,
                    "⚠️ 缺少 CAP_NET_RAW 权限，蓝牙扫描可能受限。".to_string(),
                );
            }
        } else {
            app.add_log(
                LogLevel::Info,
                "✅ NetworkManager 已就绪，双连接支持已激活。".to_string(),
            );
        }

        app.add_log(
            LogLevel::Info,
            "[s]扫描 [r]接收 [d]日志级别 [c]清空日志 [q]退出".to_string(),
        );

        app
    }

    pub fn dismiss_warning(&mut self) {
        self.show_perm_warning = false;
    }

    pub fn set_file_to_send(&mut self, path: String) {
        self.file_to_send = Some(path);
        self.add_log(
            LogLevel::Info,
            format!("待发送文件已设置: {}", self.file_to_send.as_ref().unwrap()),
        );
    }

    pub fn run_sender(&mut self, device_addr: String, file_path: String) {
        let tx = self.event_tx.clone();

        self.add_log(
            LogLevel::Info,
            format!("正在连接设备 {} (发送 {})...", device_addr, file_path),
        );
        self.mode = AppMode::Sending;

        // 取消现有任务（如果有）
        if let Some(handle) = self.active_task.take() {
            handle.abort();
        }

        // 查找选中的 DiscoveredDevice
        let device = self
            .devices
            .iter()
            .find(|d| d.address == device_addr)
            .cloned();

        if let Some(device) = device {
            let task = tokio::spawn(async move {
                let options = SendOptions {
                    wifi_interface: "wlan0".to_string(),
                    use_5ghz: true,
                    sender_name: "Cattysend-TUI".to_string(),
                };

                // 1. 创建回调和接收通道
                let (callback, mut rx_internal) = SimpleSendCallback::new();

                // 2. 启动一个子任务来转发回调事件到主 App 通道
                let tx_clone = tx.clone();
                tokio::spawn(async move {
                    while let Some(event) = rx_internal.recv().await {
                        let tx = tx_clone.clone();
                        match event {
                            cattysend_core::SendEvent::Status(s) => {
                                let _ = tx.send(AppEvent::StatusUpdate(s)).await;
                            }
                            cattysend_core::SendEvent::Progress { sent, total, .. } => {
                                let _ = tx.send(AppEvent::ProgressUpdate { sent, total }).await;
                            }
                            cattysend_core::SendEvent::Complete => {
                                let _ = tx.send(AppEvent::TransferComplete).await;
                            }
                            cattysend_core::SendEvent::Error(e) => {
                                let _ = tx.send(AppEvent::Error(e)).await;
                            }
                        }
                    }
                });

                // 3. 执行发送
                match Sender::new(options) {
                    Ok(sender) => {
                        if let Err(e) = sender
                            .send_to_device(
                                &device,
                                vec![std::path::PathBuf::from(file_path)],
                                &callback,
                            )
                            .await
                        {
                            let _ = tx
                                .send(AppEvent::Error(format!("发送过程错误: {}", e)))
                                .await;
                        }
                    }
                    Err(e) => {
                        let _ = tx
                            .send(AppEvent::Error(format!("无法初始化发送器: {}", e)))
                            .await;
                    }
                }
            });
            self.active_task = Some(task);
        } else {
            self.add_log(LogLevel::Error, "未找到目标设备信息".to_string());
            self.mode = AppMode::Idle;
        }
    }

    /// 添加日志条目
    pub fn add_log(&mut self, level: LogLevel, message: String) {
        self.raw_logs.push(LogEntry { level, message });
        // 保持最多 500 条日志
        if self.raw_logs.len() > 500 {
            self.raw_logs.remove(0);
        }
    }

    /// 获取过滤后的日志（用于显示）
    pub fn filtered_logs(&self) -> Vec<String> {
        self.raw_logs
            .iter()
            .filter(|e| e.level <= self.log_filter)
            .map(|e| format!("{} {}", e.level.icon(), e.message))
            .collect()
    }

    /// 切换日志级别（循环: Info -> Debug -> Trace -> Info）
    pub fn toggle_log_level(&mut self) {
        self.log_filter = match self.log_filter {
            LogLevel::Error => LogLevel::Warn,
            LogLevel::Warn => LogLevel::Info,
            LogLevel::Info => LogLevel::Debug,
            LogLevel::Debug => LogLevel::Trace,
            LogLevel::Trace => LogLevel::Info,
        };
        self.add_log(
            LogLevel::Info,
            format!("日志级别切换为: {}", self.log_filter.name()),
        );
    }

    /// 清空日志
    pub fn clear_logs(&mut self) {
        self.raw_logs.clear();
        self.add_log(LogLevel::Info, "日志已清空".to_string());
    }

    pub fn start_scan(&mut self) {
        if self.mode == AppMode::Scanning {
            return;
        }

        self.mode = AppMode::Scanning;
        self.scan_start = Some(Instant::now());
        self.devices.clear();
        self.selected_device = 0;
        self.add_log(LogLevel::Info, "开始扫描附近设备...".to_string());

        let tx = self.event_tx.clone();

        // 扫面回调实现
        struct TuiScanCallback {
            tx: mpsc::Sender<AppEvent>,
        }

        #[async_trait::async_trait]
        impl ScanCallback for TuiScanCallback {
            async fn on_device_found(&self, device: DiscoveredDevice) {
                let _ = self.tx.send(AppEvent::DeviceFound(device)).await;
            }
        }

        let callback: Arc<dyn ScanCallback> = Arc::new(TuiScanCallback { tx: tx.clone() });

        // 启动扫描任务
        tokio::spawn(async move {
            match BleScanner::new().await {
                Ok(scanner) => match scanner.scan(Duration::from_secs(10), Some(callback)).await {
                    Ok(_) => {
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
                    self.add_log(
                        LogLevel::Info,
                        format!("扫描完成，发现 {} 个设备", self.devices.len()),
                    );
                }
            }
            AppEvent::StatusUpdate(msg) => {
                self.add_log(LogLevel::Info, msg);
            }
            AppEvent::ProgressUpdate { sent, total } => {
                self.progress = sent as f64 / total as f64;
                self.mode = AppMode::Transferring;
            }
            AppEvent::TransferComplete => {
                self.mode = AppMode::Idle;
                self.progress = 1.0;
                self.add_log(LogLevel::Info, "传输任务已完成".to_string());
            }
            AppEvent::Error(msg) => {
                self.mode = AppMode::Idle;
                self.add_log(LogLevel::Error, msg);
            }
            AppEvent::LogMessage { level, message } => {
                let log_level = LogLevel::from_str(&level);
                self.add_log(log_level, message);
            }
        }
    }

    pub fn toggle_receive_mode(&mut self) {
        if self.mode == AppMode::Receiving {
            if let Some(handle) = self.active_task.take() {
                handle.abort();
            }
            self.mode = AppMode::Idle;
            self.add_log(LogLevel::Info, "停止接收模式".to_string());
            return;
        }

        self.mode = AppMode::Receiving;
        self.add_log(LogLevel::Info, "进入接收模式，正在广播...".to_string());

        let tx = self.event_tx.clone();
        let options = ReceiveOptions::default();

        let handle = tokio::spawn(async move {
            match Receiver::new(options) {
                Ok(receiver) => {
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
            self.add_log(
                LogLevel::Info,
                format!("选中设备: {} ({})", device.name, device.address),
            );
            // 提示用户如何发送文件
            self.add_log(
                LogLevel::Info,
                "💡 使用方法: cargo run -p cattysend-tui <文件路径>".to_string(),
            );
            self.add_log(
                LogLevel::Info,
                "   然后按 Enter 发送到选中的设备".to_string(),
            );
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
