//! Cattysend Daemon
//!
//! 后台守护进程，负责：
//! - BLE 广播/扫描
//! - WiFi P2P 热点管理
//! - HTTP/WebSocket 服务
//! - 通过 Unix Socket 与 CLI 通信

mod service;

use anyhow::Result;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    // 桥接 log crate（cattysend-core 使用）到 tracing
    let _ = tracing_log::LogTracer::init();

    // 初始化日志
    let _ = tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| EnvFilter::new("info,cattysend_core=debug")),
        )
        .try_init();

    tracing::info!("Cattysend Daemon starting...");

    // 启动 gRPC 服务
    if let Err(e) = service::run_service().await {
        tracing::error!("服务运行错误: {:?}", e);
    }

    Ok(())
}
