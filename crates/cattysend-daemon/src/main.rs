//! Cattysend Daemon
//!
//! 后台守护进程，负责：
//! - BLE 广播/扫描
//! - WiFi P2P 热点管理
//! - HTTP/WebSocket 服务
//! - 通过 Unix Socket 与 CLI 通信

mod ipc;
mod service;

use anyhow::Result;
use tracing_subscriber::EnvFilter;

#[tokio::main]
async fn main() -> Result<()> {
    // 初始化日志
    tracing_subscriber::fmt()
        .with_env_filter(EnvFilter::from_default_env().add_directive("cattysend=debug".parse()?))
        .init();

    tracing::info!("Cattysend Daemon 启动中...");

    // 启动 IPC 服务器
    let ipc_handle = tokio::spawn(ipc::run_ipc_server());

    // 启动核心服务
    let service_handle = tokio::spawn(service::run_service());

    // 等待任一任务完成
    tokio::select! {
        res = ipc_handle => {
            tracing::error!("IPC 服务器退出: {:?}", res);
        }
        res = service_handle => {
            tracing::error!("核心服务退出: {:?}", res);
        }
    }

    Ok(())
}
