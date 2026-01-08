//! Cattysend CLI
//!
//! 命令行客户端，通过 Unix Socket 与守护进程通信

mod client;

use anyhow::Result;
use clap::{Parser, Subcommand};

#[derive(Parser)]
#[command(name = "cattysend", version, about = "互传联盟 - Linux 文件传输工具")]
struct Cli {
    #[command(subcommand)]
    command: Commands,
}

#[derive(Subcommand)]
enum Commands {
    /// 发送文件
    Send {
        /// 要发送的文件路径
        file: String,
        /// 目标设备地址 (可选，不指定则交互式选择)
        #[arg(short, long)]
        device: Option<String>,
    },
    /// 接收文件
    Receive {
        /// 保存目录 (默认: ~/Downloads)
        #[arg(short, long)]
        output: Option<String>,
    },
    /// 扫描附近设备
    Scan {
        /// 扫描超时时间 (秒)
        #[arg(short, long, default_value = "10")]
        timeout: u64,
    },
    /// 查看当前状态
    Status,
    /// 停止当前传输
    Stop,
}

#[tokio::main]
async fn main() -> Result<()> {
    let cli = Cli::parse();

    match cli.command {
        Commands::Send { file, device } => {
            println!("📤 发送文件: {}", file);
            if let Some(dev) = &device {
                println!("   目标设备: {}", dev);
            }
            client::send_request(client::IpcRequest::Send {
                file_path: file,
                device_addr: device,
            })
            .await?;
        }
        Commands::Receive { output } => {
            let dir = output.unwrap_or_else(|| {
                dirs::download_dir()
                    .map(|p| p.to_string_lossy().to_string())
                    .unwrap_or_else(|| ".".to_string())
            });
            println!("📥 接收模式 (保存到: {})", dir);
            client::send_request(client::IpcRequest::Receive).await?;
        }
        Commands::Scan { timeout } => {
            println!("🔍 扫描设备 ({}s)...", timeout);
            let resp = client::send_request(client::IpcRequest::Scan {
                timeout_secs: timeout,
            })
            .await?;
            if let client::IpcResponse::Devices { devices } = resp {
                if devices.is_empty() {
                    println!("   未发现设备");
                } else {
                    for (i, dev) in devices.iter().enumerate() {
                        println!("   [{}] {} ({})", i, dev.name, dev.address);
                    }
                }
            }
        }
        Commands::Status => {
            let resp = client::send_request(client::IpcRequest::Status).await?;
            if let client::IpcResponse::Status { state, progress } = resp {
                println!("状态: {}", state);
                if let Some(p) = progress {
                    println!("进度: {:.1}%", p * 100.0);
                }
            }
        }
        Commands::Stop => {
            println!("⏹️  停止传输");
            client::send_request(client::IpcRequest::Stop).await?;
        }
    }

    Ok(())
}
