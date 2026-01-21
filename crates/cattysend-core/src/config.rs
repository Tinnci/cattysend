//! 应用配置和持久化
//!
//! 提供设备名称、厂商 ID 等设置的存储和读取。

use log::debug;
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// 厂商 ID 枚举
///
/// 与 CatShare 兼容的厂商 ID 列表
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum BrandId {
    #[default]
    Unknown = 0,
    Oppo = 10,
    Realme = 11,
    Vivo = 20,
    Xiaomi = 30,
    OnePlus = 41,
    Meizu = 50,
    Samsung = 70,
    Lenovo = 100,
    // 自定义 ID 用于 Linux 设备
    Linux = 200,
}

impl BrandId {
    /// 获取厂商名称
    pub fn name(&self) -> &'static str {
        match self {
            BrandId::Unknown => "Unknown",
            BrandId::Oppo => "OPPO",
            BrandId::Realme => "realme",
            BrandId::Vivo => "vivo",
            BrandId::Xiaomi => "Xiaomi",
            BrandId::OnePlus => "OnePlus",
            BrandId::Meizu => "Meizu",
            BrandId::Samsung => "Samsung",
            BrandId::Lenovo => "Lenovo",
            BrandId::Linux => "Linux",
        }
    }

    /// 从 ID 值创建
    pub fn from_id(id: u8) -> Self {
        match id {
            10 => BrandId::Oppo,
            11 => BrandId::Realme,
            20..=29 => BrandId::Vivo,
            30..=39 => BrandId::Xiaomi,
            41..=45 => BrandId::OnePlus,
            50..=59 => BrandId::Meizu,
            70..=75 => BrandId::Samsung,
            100..=109 => BrandId::Lenovo,
            200 => BrandId::Linux,
            _ => BrandId::Unknown,
        }
    }

    /// 获取 ID 值
    pub fn id(&self) -> u8 {
        *self as u8
    }
}

/// 应用设置
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AppSettings {
    /// 设备名称（在扫描时显示）
    pub device_name: String,
    /// 厂商 ID
    pub brand_id: BrandId,
    /// 是否支持 5GHz WiFi
    pub supports_5ghz: bool,
    /// WiFi 接口名称
    pub wifi_interface: String,
    /// 下载目录
    pub download_dir: PathBuf,
    /// 是否自动接受传输
    pub auto_accept: bool,
    /// 详细日志模式
    pub verbose: bool,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            device_name: get_default_device_name(),
            brand_id: BrandId::Xiaomi,
            supports_5ghz: true,
            wifi_interface: "wlan0".to_string(),
            download_dir: dirs::download_dir().unwrap_or_else(|| PathBuf::from(".")),
            auto_accept: false,
            verbose: false,
        }
    }
}

impl AppSettings {
    /// 获取配置文件路径
    fn config_path() -> PathBuf {
        let config_dir = dirs::config_dir()
            .unwrap_or_else(|| PathBuf::from("."))
            .join("cattysend");
        config_dir.join("settings.toml")
    }

    /// 加载设置（如果文件不存在则使用默认值）
    pub fn load() -> Self {
        let path = Self::config_path();
        if path.exists() {
            match fs::read_to_string(&path) {
                Ok(content) => match toml::from_str(&content) {
                    Ok(settings) => {
                        debug!("Loaded settings from {:?}", path);
                        return settings;
                    }
                    Err(e) => {
                        log::warn!("Failed to parse settings: {}, using defaults", e);
                    }
                },
                Err(e) => {
                    log::warn!("Failed to read settings file: {}, using defaults", e);
                }
            }
        }
        Self::default()
    }

    /// 保存设置
    pub fn save(&self) -> anyhow::Result<()> {
        let path = Self::config_path();
        if let Some(parent) = path.parent() {
            fs::create_dir_all(parent)?;
        }
        let content = toml::to_string_pretty(self)?;
        fs::write(&path, content)?;
        debug!("Saved settings to {:?}", path);
        Ok(())
    }

    /// 获取用于广播的能力 UUID
    ///
    /// 格式: 0000XXYY-0000-1000-8000-00805f9b34fb
    /// - XX = 5GHz 标志 (0x01 = 支持, 0x00 = 不支持)
    /// - YY = 厂商 ID
    pub fn capability_uuid(&self) -> uuid::Uuid {
        let flag_5ghz: u8 = if self.supports_5ghz { 0x01 } else { 0x00 };
        let brand = self.brand_id.id();
        // 构造 UUID: 0000XXYY-0000-1000-8000-00805f9b34fb
        let high = (flag_5ghz as u16) << 8 | (brand as u16);
        uuid::Uuid::from_u128(((high as u128) << 96) | (0x0000_1000_8000_0080_5f9b_34fb_u128))
    }
}

/// 获取默认设备名称（主机名）
fn get_default_device_name() -> String {
    hostname::get()
        .map(|h| h.to_string_lossy().to_string())
        .unwrap_or_else(|_| "Cattysend".to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_brand_id() {
        assert_eq!(BrandId::Xiaomi.id(), 30);
        assert_eq!(BrandId::from_id(30).name(), "Xiaomi");
    }

    #[test]
    fn test_capability_uuid() {
        let settings = AppSettings {
            supports_5ghz: true,
            brand_id: BrandId::Xiaomi, // 30 = 0x1E
            ..Default::default()
        };

        let uuid = settings.capability_uuid();
        let uuid_str = uuid.to_string();
        // 应该是 0000011e-0000-1000-8000-00805f9b34fb
        assert!(uuid_str.starts_with("0000011e"), "UUID: {}", uuid_str);
        assert!(uuid_str.ends_with("00805f9b34fb"), "UUID: {}", uuid_str);
    }

    #[test]
    fn test_default_settings() {
        let settings = AppSettings::default();
        // 默认为 Xiaomi 以确保兼容性
        assert_eq!(settings.brand_id, BrandId::Xiaomi);
        assert!(settings.supports_5ghz);
    }
}
