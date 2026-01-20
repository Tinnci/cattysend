//! Cattysend GUI Application
//!
//! 基于 Dioxus 的跨平台桌面 GUI，实现互传联盟协议的文件传输功能。
//!
//! # 架构
//!
//! ```text
//! ┌─────────────────────────────────────────────────────────────┐
//! │                    Dioxus Desktop App                       │
//! ├─────────────────────────────────────────────────────────────┤
//! │  ┌─────────────┐  ┌─────────────┐  ┌─────────────────────┐  │
//! │  │   Header    │  │   DeviceList│  │   TransferPanel     │  │
//! │  │  (状态栏)    │  │  (设备列表)  │  │   (传输进度)        │  │
//! │  └─────────────┘  └─────────────┘  └─────────────────────┘  │
//! ├─────────────────────────────────────────────────────────────┤
//! │                       Core Logic                            │
//! │              (cattysend-core crate)                         │
//! └─────────────────────────────────────────────────────────────┘
//! ```

mod app;
mod components;
mod state;
mod styles;

fn main() {
    // 初始化日志
    env_logger::Builder::from_env(env_logger::Env::default().default_filter_or("info")).init();

    log::info!("Starting Cattysend GUI...");

    // 启动 Dioxus 桌面应用
    dioxus::launch(app::App);
}
