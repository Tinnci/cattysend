//! Core Service - BLE/WiFi/Transfer 管理

use anyhow::Result;
use cattysend_core::ble::DeviceStatus;
use cattysend_core::BleSecurity;

pub async fn run_service() -> Result<()> {
    tracing::info!("核心服务初始化...");

    // 生成加密密钥对
    let security = BleSecurity::new()?;
    let public_key = security.get_public_key().to_string();

    let status = DeviceStatus {
        device_name: hostname::get()
            .map(|h| h.to_string_lossy().to_string())
            .unwrap_or_else(|_| "Cattysend-Linux".to_string()),
        os_version: "Linux".to_string(),
        model: "Desktop".to_string(),
        public_key,
        sender_version: env!("CARGO_PKG_VERSION").to_string(),
    };

    tracing::info!("设备信息: {:?}", status);
    tracing::info!("等待 IPC 命令...");

    // 保持服务运行
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
    }
}
