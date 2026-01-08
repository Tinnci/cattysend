mod ble;
mod crypto;
mod transfer;
mod wifi;

use anyhow::Result;
use tracing_subscriber;

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt::init();

    let args: Vec<String> = std::env::args().collect();
    let mode = args.get(1).map(|s| s.as_str()).unwrap_or("help");

    match mode {
        "send" => {
            let file_path = args
                .get(2)
                .ok_or_else(|| anyhow::anyhow!("Usage: cattysend send <file>"))?;
            run_sender(file_path).await?;
        }
        "receive" => {
            run_receiver().await?;
        }
        _ => {
            println!("Cattysend - 互传联盟协议 Linux 实现");
            println!();
            println!("Usage:");
            println!("  cattysend send <file>    发送文件");
            println!("  cattysend receive        接收文件");
        }
    }

    Ok(())
}

async fn run_sender(file_path: &str) -> Result<()> {
    use crate::ble::DeviceStatus;
    use crate::crypto::BleSecurity;
    use crate::transfer::http_server;
    use crate::wifi::p2p_sender::WiFiP2pSender;

    println!("📤 发送模式: {}", file_path);

    // 1. Generate ECDH keypair
    let security = BleSecurity::new()?;
    let public_key = security.get_public_key().to_string();

    // 2. Create device status
    let status = DeviceStatus {
        device_name: "Cattysend-Linux".to_string(),
        os_version: "Linux".to_string(),
        model: "Desktop".to_string(),
        public_key,
        sender_version: "1.0".to_string(),
    };

    println!("📱 设备信息: {:?}", status);

    // 3. Create WiFi P2P group
    let wifi_sender = WiFiP2pSender::new("wlan0");
    let p2p_info = wifi_sender.create_group(33331).await?;
    println!(
        "📡 P2P 热点已创建: SSID={}, 端口={}",
        p2p_info.ssid, p2p_info.port
    );

    // 4. Start BLE advertising (in background)
    let (tx, mut rx) = tokio::sync::mpsc::channel::<Vec<u8>>(1);
    let advertiser = crate::ble::advertiser::BleAdvertiser::new(tx);
    advertiser.set_status(status).await?;

    let ble_handle = tokio::spawn(async move {
        if let Err(e) = advertiser.start().await {
            eprintln!("BLE advertising error: {}", e);
        }
    });

    println!("📻 BLE 广播已启动，等待接收端连接...");

    // 5. Wait for P2P data from receiver (via BLE)
    if let Some(data) = rx.recv().await {
        println!("收到 BLE 数据: {} bytes", data.len());
    }

    // 6. Start HTTP server
    println!("🌐 启动 HTTP 服务器...");
    let file = file_path.to_string();
    http_server::start_http_server(33331, file).await?;

    ble_handle.abort();
    Ok(())
}

async fn run_receiver() -> Result<()> {
    use crate::ble::scanner::BleScanner;
    use std::time::Duration;

    println!("📥 接收模式");

    // 1. Scan for nearby devices
    println!("🔍 扫描附近设备...");
    let scanner = BleScanner::new().await?;
    let devices = scanner.scan(Duration::from_secs(10)).await?;

    if devices.is_empty() {
        println!("未发现可用设备");
        return Ok(());
    }

    println!("发现 {} 个设备:", devices.len());
    for (i, dev) in devices.iter().enumerate() {
        println!("  [{}] {} ({})", i, dev.name, dev.address);
    }

    // TODO: User selection, GATT connection, P2P connection, file download
    println!();
    println!("⚠️  完整流程尚在开发中...");

    Ok(())
}
