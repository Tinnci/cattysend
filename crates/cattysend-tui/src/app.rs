//! Application state

use cattysend_core::{BleScanner, DiscoveredDevice};
use std::time::{Duration, Instant};
use tokio::sync::mpsc;

#[derive(Debug, Clone, Copy, PartialEq)]
pub enum AppMode {
    Idle,
    Scanning,
    Receiving,
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
    Error(String),
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
                Ok(scanner) => match scanner.scan(Duration::from_secs(5)).await {
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
                // 去重
                if !self.devices.iter().any(|d| d.address == device.address) {
                    self.devices.push(device);
                }
            }
            AppEvent::ScanFinished => {
                self.mode = AppMode::Idle;
                self.logs
                    .push(format!("扫描完成，发现 {} 个设备", self.devices.len()));
            }
            AppEvent::Error(msg) => {
                self.mode = AppMode::Idle;
                self.logs.push(format!("错误: {}", msg));
            }
        }
    }

    pub fn toggle_receive_mode(&mut self) {
        match self.mode {
            AppMode::Receiving => {
                self.mode = AppMode::Idle;
                self.logs.push("停止接收模式".to_string());
            }
            _ => {
                self.mode = AppMode::Receiving;
                self.logs.push("进入接收模式，等待连接...".to_string());
                // TODO: 启动接收任务
            }
        }
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
                .push(format!("连接到: {} ({})", device.name, device.address));
            self.mode = AppMode::Sending;
            // TODO: 启动发送任务
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
        // 处理异步事件
        while let Ok(event) = self.event_rx.try_recv() {
            self.handle_event(event);
        }

        // Transfer simulation removal (will replace with real progress)
        if self.mode == AppMode::Transferring {
            self.progress += 0.02;
            if self.progress >= 1.0 {
                self.progress = 1.0;
                self.mode = AppMode::Idle;
                self.logs.push("传输完成!".to_string());
            }
        }
    }
}
