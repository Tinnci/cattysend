//! Core Service - BLE/WiFi/Transfer 管理

use anyhow::Result;
use cattysend_core::BleSecurityPersistent;
use cattysend_core::ble::DeviceInfo;

pub async fn run_service() -> Result<()> {
    tracing::info!("核心服务初始化...");

    // 生成加密密钥对（持久化，在服务生命周期内保持一致）
    let security = BleSecurityPersistent::new()?;
    let public_key = security.get_public_key().to_string();

    // 获取 P2P 接口 MAC 地址
    let mac = get_p2p_mac().unwrap_or_else(|| "02:00:00:00:00:00".to_string());

    let info = DeviceInfo::new(public_key, mac);

    tracing::info!("设备信息: {:?}", info);
    tracing::info!("等待 IPC 命令...");

    // 保持服务运行
    loop {
        tokio::time::sleep(std::time::Duration::from_secs(60)).await;
    }
}

fn get_p2p_mac() -> Option<String> {
    // 尝试读取 p2p0 接口的 MAC 地址
    for iface in &["p2p0", "wlan0", "wlp2s0"] {
        let path = format!("/sys/class/net/{}/address", iface);
        if let Ok(mac) = std::fs::read_to_string(&path) {
            return Some(mac.trim().to_uppercase());
        }
    }
    None
}
