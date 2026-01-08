//! Application state

use std::time::Instant;

#[derive(Debug, Clone)]
pub struct Device {
    pub name: String,
    pub address: String,
    pub rssi: i16,
    pub brand: String,
}

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

pub struct App {
    pub mode: AppMode,
    pub tab: Tab,
    pub devices: Vec<Device>,
    pub selected_device: usize,
    pub progress: f64,
    pub transfer_speed: f64,
    pub logs: Vec<String>,
    pub scan_start: Option<Instant>,
}

impl App {
    pub fn new() -> Self {
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
        }
    }

    pub fn start_scan(&mut self) {
        self.mode = AppMode::Scanning;
        self.scan_start = Some(Instant::now());
        self.devices.clear();
        self.logs.push("开始扫描附近设备...".to_string());
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
        // Simulate scanning
        if self.mode == AppMode::Scanning {
            if let Some(start) = self.scan_start {
                let elapsed = start.elapsed().as_secs_f64();

                // Add simulated devices over time
                if elapsed > 0.5 && self.devices.is_empty() {
                    self.devices.push(Device {
                        name: "小米 12".to_string(),
                        address: "AA:BB:CC:DD:EE:01".to_string(),
                        rssi: -45,
                        brand: "Xiaomi".to_string(),
                    });
                }
                if elapsed > 1.0 && self.devices.len() < 2 {
                    self.devices.push(Device {
                        name: "OPPO Find X5".to_string(),
                        address: "AA:BB:CC:DD:EE:02".to_string(),
                        rssi: -62,
                        brand: "OPPO".to_string(),
                    });
                }
                if elapsed > 1.5 && self.devices.len() < 3 {
                    self.devices.push(Device {
                        name: "Vivo X90".to_string(),
                        address: "AA:BB:CC:DD:EE:03".to_string(),
                        rssi: -78,
                        brand: "Vivo".to_string(),
                    });
                }

                // Stop scanning after 3 seconds
                if elapsed > 3.0 {
                    self.mode = AppMode::Idle;
                    self.logs
                        .push(format!("扫描完成，发现 {} 个设备", self.devices.len()));
                }
            }
        }

        // Simulate transfer progress
        if self.mode == AppMode::Transferring {
            self.progress += 0.02;
            self.transfer_speed = 85.5 + (rand::random::<f64>() * 20.0 - 10.0);
            if self.progress >= 1.0 {
                self.progress = 1.0;
                self.mode = AppMode::Idle;
                self.logs.push("传输完成!".to_string());
            }
        }
    }
}
