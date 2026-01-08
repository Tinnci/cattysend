# Linux 上使用 Rust 实现互传联盟协议

## 目录
1. [概述](#概述)
2. [架构设计](#架构设计)
3. [核心模块实现](#核心模块实现)
4. [技术栈选型](#技术栈选型)
5. [实现路线图](#实现路线图)

---

## 概述

本文档介绍如何在 Linux 上使用 Rust 实现互传联盟（Mutual Transmission Alliance）协议。该协议通过 **BLE 发现** → **WiFi Direct 连接** → **HTTP/WebSocket 传输** 三个阶段实现无缝的跨设备文件传输。

相比 Android 版本（CatShare），Linux 实现的挑战在于：
- BLE 操作需要依赖 D-Bus 或底层蓝牙驱动
- WiFi Direct (P2P) 在 Linux 上的支持有限
- 需要在无系统权限支持的情况下获取网络接口信息

---

## 架构设计

```
┌─────────────────────────────────────────────────────────┐
│           应用层 (Application Layer)                     │
│  - CLI 交互  - 文件管理 - 进度展示 - 日志系统           │
└────────────────┬────────────────────────────────────────┘
                 │
┌────────────────▼────────────────────────────────────────┐
│         协议层 (Protocol Layer)                          │
│  ┌──────────────┬──────────────┬──────────────────┐    │
│  │   BLE 模块   │  WiFi P2P    │  HTTP/WS 模块    │    │
│  │              │  模块        │                  │    │
│  └──────────────┴──────────────┴──────────────────┘    │
└────────────────┬────────────────────────────────────────┘
                 │
┌────────────────▼────────────────────────────────────────┐
│         系统层 (System Layer)                            │
│  ┌──────────────┬──────────────┬──────────────────┐    │
│  │  蓝牙驱动    │  网络接口    │  加密/安全       │    │
│  │  (BlueZ)     │  管理        │  (ECDH, AES)     │    │
│  └──────────────┴──────────────┴──────────────────┘    │
└─────────────────────────────────────────────────────────┘
```

### 核心流程

```
发送端流程：
1. 启动 GATT Server (BLE 广播)
   └─> 监听 CHAR_STATUS 和 CHAR_P2P 特征值写入
2. 创建 WiFi Direct 热点 (wpa_supplicant)
   └─> 获取热点 SSID、密码、MAC 地址
3. 启动 HTTP/WebSocket 服务器 (Tokio + Ktor-like)
   └─> /websocket 路由 (控制信令)
   └─> /download 路由 (数据传输)

接收端流程：
1. 扫描 BLE 设备并过滤
   └─> 检查 Service UUID (00003331-...)
2. 连接 GATT Server 读取设备信息和公钥
   └─> ECDH 密钥交换
3. 读取 P2P 连接信息 (WiFi SSID、密码)
   └─> 解密信息
4. 连接 WiFi Direct 热点
   └─> nmcli 或 wpa_supplicant 连接
5. 启动 WebSocket 客户端并发送文件下载请求
   └─> HTTP GET /download 接收文件流
```

---

## 核心模块实现

### 1. BLE 模块 (`ble/`)

#### 1.1 BLE 发现 (Receiver - 广播端)

**文件**: `src/ble/advertiser.rs`

```rust
use blurz::bluetooth::BluetoothSession;
use blurz::gatt::LocalGattCharacteristic;
use uuid::Uuid;
use tokio::sync::Mutex;
use std::sync::Arc;

/// BLE 服务 UUID 定义
pub struct BleUuids {
    pub adv_service: Uuid,      // 00003331-0000-1000-8000-008123456789
    pub service: Uuid,           // 00009955-0000-1000-8000-00805f9b34fb
    pub char_status: Uuid,       // 00009954-0000-1000-8000-00805f9b34fb
    pub char_p2p: Uuid,          // 00009953-0000-1000-8000-00805f9b34fb
}

pub struct BleAdvertiser {
    session: Arc<BluetoothSession>,
    device: String, // D-Bus object path
    uuids: BleUuids,
}

impl BleAdvertiser {
    /// 启动 BLE 广播
    pub async fn start_advertising(
        &self,
        device_name: &str,
        brand_id: u16,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // 1. 创建 GATT Service
        let service = self.create_gatt_service().await?;
        
        // 2. 创建特征值 (CHAR_STATUS 和 CHAR_P2P)
        self.create_gatt_characteristics(&service).await?;
        
        // 3. 启动 BLE 广告
        self.set_advertising_data(device_name, brand_id).await?;
        
        Ok(())
    }
    
    /// 创建本地 GATT 服务
    async fn create_gatt_service(&self) -> Result<String, Box<dyn std::error::Error>> {
        // 使用 org.bluez 的 D-Bus API
        // CreateService(uuid) -> service_path
        todo!()
    }
    
    /// 创建 GATT 特征值
    async fn create_gatt_characteristics(&self, service: &str) -> Result<(), Box<dyn std::error::Error>> {
        // 创建 CHAR_STATUS 特征值 (读/通知)
        // 创建 CHAR_P2P 特征值 (读/写)
        todo!()
    }
    
    /// 设置 BLE 广告数据
    async fn set_advertising_data(
        &self,
        device_name: &str,
        brand_id: u16,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // 广告数据结构：
        // [Service UUID (16 bytes)] [Device Name] [Brand ID (2 bytes)]
        todo!()
    }
}
```

**依赖**:
```toml
[dependencies]
blurz = "0.6"           # BlueZ D-Bus 客户端
uuid = "1.0"
tokio = { version = "1", features = ["full"] }
dbus = "0.9"
```

---

#### 1.2 BLE 扫描 (Sender - 发现端)

**文件**: `src/ble/scanner.rs`

```rust
use blurz::bluetooth::{BluetoothAdapter, BluetoothSession};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DiscoveredDevice {
    pub address: String,          // 蓝牙 MAC 地址
    pub name: String,             // 设备名称
    pub brand_id: u16,            // 品牌标识
    pub rssi: i16,                // 信号强度
}

pub struct BleScanner {
    session: Arc<BluetoothSession>,
    adapter: BluetoothAdapter,
    target_service_uuid: Uuid,    // 00003331-...
}

impl BleScanner {
    /// 启动 BLE 扫描
    pub async fn scan(
        &self,
        duration_secs: u64,
    ) -> Result<Vec<DiscoveredDevice>, Box<dyn std::error::Error>> {
        // 1. 开始发现设备
        self.adapter.start_discovery()?;
        
        // 2. 监听 D-Bus InterfacesAdded 信号
        // 过滤包含目标 Service UUID 的设备
        let devices = self.listen_for_devices(duration_secs).await?;
        
        // 3. 停止扫描
        self.adapter.stop_discovery()?;
        
        Ok(devices)
    }
    
    /// 监听新设备并提取信息
    async fn listen_for_devices(
        &self,
        timeout_secs: u64,
    ) -> Result<Vec<DiscoveredDevice>, Box<dyn std::error::Error>> {
        // 使用 dbus::blocking::Connection 监听信号
        // 解析广告数据并提取 Brand ID
        todo!()
    }
    
    /// 从广告数据中提取品牌 ID
    fn parse_brand_id(adv_data: &[u8]) -> u16 {
        // 广告数据格式：Service UUID (16) + Name + Brand ID (2)
        if adv_data.len() >= 18 {
            u16::from_le_bytes([adv_data[16], adv_data[17]])
        } else {
            0
        }
    }
}
```

---

#### 1.3 GATT 连接与密钥交换

**文件**: `src/ble/gatt.rs`

```rust
use blurz::gatt::{LocalGattCharacteristic, LocalGattService};
use crate::crypto::BleSecurity;

/// GATT 特征值回调处理
pub struct GattCharacteristicHandler {
    pub char_status: Arc<Mutex<String>>, // 状态信息 JSON
    pub char_p2p: Arc<Mutex<Vec<u8>>>,   // P2P 连接信息 (加密)
}

impl GattCharacteristicHandler {
    /// 处理 CHAR_P2P 写入 (发送端 -> 接收端)
    pub async fn on_char_p2p_write(
        &self,
        data: Vec<u8>,
        security: &BleSecurity,
    ) -> Result<P2pInfo, Box<dyn std::error::Error>> {
        // 1. 解密数据
        let decrypted = security.decrypt(&data)?;
        
        // 2. 解析 P2pInfo JSON
        let p2p_info: P2pInfo = serde_json::from_str(&decrypted)?;
        
        // 3. 验证信息完整性 (MAC 地址、端口号等)
        p2p_info.validate()?;
        
        Ok(p2p_info)
    }
    
    /// 处理 CHAR_P2P 读取 (发送端 -> 接收端)
    pub async fn on_char_p2p_read(
        &self,
        sender_public_key: &str,
        security: &BleSecurity,
        p2p_info: &P2pInfo,
    ) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        // 1. ECDH 密钥交换 (使用发送端的公钥)
        let session_cipher = security.derive_session_key(sender_public_key)?;
        
        // 2. 序列化并加密 P2pInfo
        let json = serde_json::to_string(p2p_info)?;
        let encrypted = session_cipher.encrypt(&json)?;
        
        Ok(encrypted.as_bytes().to_vec())
    }
}
```

---

### 2. WiFi P2P 模块 (`wifi/`)

#### 2.1 WiFi Direct 热点创建 (发送端)

**文件**: `src/wifi/p2p_sender.rs`

```rust
use std::process::Command;
use rand::Rng;

#[derive(Clone, Debug, Serialize)]
pub struct WiFiP2pInfo {
    pub ssid: String,           // WiFi 热点名称 (如 "DIRECT-xy")
    pub password: String,       // 热点密码
    pub mac_address: String,    // 本机 MAC 地址
    pub port: u16,              // 数据传输服务器端口
}

pub struct WiFiP2pSender {
    iface: String,              // WiFi 接口名称 (如 "wlan0")
}

impl WiFiP2pSender {
    /// 创建 WiFi P2P 热点
    pub async fn create_group(
        &self,
        server_port: u16,
    ) -> Result<WiFiP2pInfo, Box<dyn std::error::Error>> {
        // 1. 获取本机 MAC 地址
        let mac_address = self.get_interface_mac(&self.iface)?;
        
        // 2. 生成热点 SSID 和密码
        let ssid = format!("DIRECT-{:02x}", Self::gen_random_bytes());
        let password = Self::gen_random_password();
        
        // 3. 使用 wpa_supplicant 创建 P2P 热点
        self.create_p2p_group(&ssid, &password)?;
        
        // 4. 获取热点的 IP 地址并配置 DHCP
        self.setup_dhcp_server(&self.iface)?;
        
        Ok(WiFiP2pInfo {
            ssid,
            password,
            mac_address,
            port: server_port,
        })
    }
    
    /// 获取网络接口 MAC 地址
    fn get_interface_mac(&self, iface: &str) -> Result<String, Box<dyn std::error::Error>> {
        // 读取 /sys/class/net/{iface}/address
        let path = format!("/sys/class/net/{}/address", iface);
        let mac = std::fs::read_to_string(path)?;
        Ok(mac.trim().to_uppercase())
    }
    
    /// 使用 wpa_supplicant 创建 P2P 组
    fn create_p2p_group(
        &self,
        ssid: &str,
        password: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        // 通过 wpa_supplicant D-Bus 接口创建 P2P 组
        // 或使用 wpa_cli: wpa_cli -p /var/run/wpa_supplicant p2p_group_add
        todo!()
    }
    
    /// 配置 DHCP 服务器
    fn setup_dhcp_server(&self, iface: &str) -> Result<(), Box<dyn std::error::Error>> {
        // 1. 配置 IP 地址 (如 192.168.49.1)
        Command::new("sudo")
            .args(&["ip", "addr", "add", "192.168.49.1/24", "dev", iface])
            .output()?;
        
        // 2. 启动 dnsmasq 或 udhcpd (DHCP 服务)
        Command::new("sudo")
            .args(&["dnsmasq", "--interface", iface, "--dhcp-range", "192.168.49.2,192.168.49.254,12h"])
            .output()?;
        
        Ok(())
    }
    
    /// 生成随机密码
    fn gen_random_password() -> String {
        const CHARSET: &[u8] = b"0123456789abcdefghijklmnopqrstuvwxyzABCDEFGHIJKLMNOPQRSTUVWXYZ";
        let mut rng = rand::thread_rng();
        (0..32)
            .map(|_| {
                let idx = rng.gen_range(0..CHARSET.len());
                CHARSET[idx] as char
            })
            .collect()
    }
}
```

---

#### 2.2 WiFi P2P 连接 (接收端)

**文件**: `src/wifi/p2p_receiver.rs`

```rust
pub struct WiFiP2pReceiver {
    iface: String,
}

impl WiFiP2pReceiver {
    /// 连接到 WiFi P2P 热点
    pub async fn connect(
        &self,
        p2p_info: &WiFiP2pInfo,
    ) -> Result<String, Box<dyn std::error::Error>> {
        // 方案 1: 使用 nmcli (NetworkManager)
        let ip = self.connect_with_nmcli(p2p_info).await?;
        
        // 方案 2: 使用 wpa_supplicant
        // let ip = self.connect_with_wpa_supplicant(p2p_info).await?;
        
        Ok(ip)
    }
    
    /// 使用 nmcli 连接
    async fn connect_with_nmcli(
        &self,
        p2p_info: &WiFiP2pInfo,
    ) -> Result<String, Box<dyn std::error::Error>> {
        use std::process::Command;
        
        // 1. 创建 WiFi 连接配置
        Command::new("nmcli")
            .args(&[
                "device", "wifi", "connect",
                &p2p_info.ssid,
                "password", &p2p_info.password,
                "ifname", &self.iface,
            ])
            .output()?;
        
        // 2. 等待连接完成并获取 IP
        tokio::time::sleep(std::time::Duration::from_secs(3)).await;
        
        let ip = self.get_interface_ip(&self.iface)?;
        Ok(ip)
    }
    
    /// 使用 wpa_supplicant 连接
    async fn connect_with_wpa_supplicant(
        &self,
        p2p_info: &WiFiP2pInfo,
    ) -> Result<String, Box<dyn std::error::Error>> {
        // 通过 wpa_supplicant D-Bus 接口创建网络
        // Interface.AddNetwork(network_config)
        todo!()
    }
    
    /// 获取网络接口 IP 地址
    fn get_interface_ip(&self, iface: &str) -> Result<String, Box<dyn std::error::Error>> {
        let output = std::process::Command::new("ip")
            .args(&["addr", "show", iface])
            .output()?;
        
        let stdout = String::from_utf8(output.stdout)?;
        // 解析 IPv4 地址 (如 192.168.49.2)
        let ip = stdout
            .lines()
            .find_map(|line| {
                let trimmed = line.trim();
                if trimmed.starts_with("inet ") {
                    trimmed.split_whitespace().nth(1).map(|s| s.to_string())
                } else {
                    None
                }
            })
            .ok_or("No IP address found")?;
        
        Ok(ip)
    }
}
```

---

### 3. 加密模块 (`crypto/`)

#### 3.1 ECDH 密钥交换与 AES 加密

**文件**: `src/crypto/ble_security.rs`

```rust
use ring::agreement::{EphemeralPrivateKey, UnixTime, X25519};
use ring::rand::SecureRandom;
use aes::Aes256Cbc;
use block_modes::{BlockMode, Cbc};
use block_modes::block_padding::Pkcs7;
use base64::{engine::general_purpose, Engine as _};

pub struct BleSecurity {
    private_key: Vec<u8>,       // 本地私钥
    public_key: String,         // Base64 编码的公钥
}

impl BleSecurity {
    /// 初始化，生成本地密钥对
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        // 使用 P-256 (secp256r1) 曲线
        let rng = ring::rand::SystemRandom::new();
        
        // 生成密钥对
        let private_key = EphemeralPrivateKey::generate(&X25519, &rng)?;
        let public_key_bytes = private_key.compute_public_key()?;
        
        let public_key = general_purpose::STANDARD.encode(public_key_bytes.as_ref());
        
        Ok(BleSecurity {
            private_key: private_key.as_ref().to_vec(),
            public_key,
        })
    }
    
    /// 获取 Base64 编码的公钥
    pub fn get_public_key(&self) -> &str {
        &self.public_key
    }
    
    /// ECDH 密钥交换，生成会话密钥
    pub fn derive_session_key(
        &self,
        peer_public_key: &str,
    ) -> Result<SessionCipher, Box<dyn std::error::Error>> {
        // 1. 解码对方的公钥
        let peer_public_bytes = general_purpose::STANDARD.decode(peer_public_key)?;
        
        // 2. ECDH 计算共享密钥
        // 注：实际实现需要使用支持 ECDH 的库 (如 elliptic-curve)
        let shared_secret = self.ecdh(&peer_public_bytes)?;
        
        // 3. KDF：将共享密钥派生为 AES-256 密钥
        let cipher_key = self.kdf(&shared_secret, 32)?;
        
        Ok(SessionCipher {
            key: cipher_key,
        })
    }
    
    /// ECDH 密钥协议
    fn ecdh(&self, peer_public_key: &[u8]) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        // 使用 p256 或 elliptic-curve crate
        todo!("Implement ECDH using ring or elliptic-curve")
    }
    
    /// 密钥派生函数 (简单版)
    fn kdf(&self, shared_secret: &[u8], key_len: usize) -> Result<Vec<u8>, Box<dyn std::error::Error>> {
        use hmac::{Hmac, Mac};
        use sha2::Sha256;
        
        // HKDF-SHA256 
        // 这里简化，实际应使用 HKDF-expand
        let mut key = Vec::new();
        let mut counter = 0u32;
        
        while key.len() < key_len {
            let mut mac = Hmac::<Sha256>::new_from_slice(shared_secret)?;
            mac.update(&counter.to_le_bytes());
            let result = mac.finalize();
            key.extend_from_slice(result.as_ref());
            counter += 1;
        }
        
        Ok(key[..key_len].to_vec())
    }
}

/// 会话密码
pub struct SessionCipher {
    key: Vec<u8>,
}

impl SessionCipher {
    /// 解密数据 (Base64 编码的 AES-256-CTR)
    pub fn decrypt(&self, encoded_data: &str) -> Result<String, Box<dyn std::error::Error>> {
        let data = general_purpose::STANDARD.decode(encoded_data)?;
        
        // AES-256-CTR，IV 固定为 "0102030405060708"
        const IV: &[u8] = b"0102030405060708";
        
        // 解密逻辑
        // let plaintext = ...;
        
        todo!()
    }
    
    /// 加密数据
    pub fn encrypt(&self, data: &str) -> Result<String, Box<dyn std::error::Error>> {
        const IV: &[u8] = b"0102030405060708";
        
        // 加密逻辑
        // let ciphertext = ...;
        
        todo!()
    }
}
```

---

### 4. HTTP/WebSocket 数据传输模块 (`transfer/`)

#### 4.1 WebSocket 控制信令 (双方)

**文件**: `src/transfer/websocket_handler.rs`

```rust
use tokio::net::TcpStream;
use tokio_tungstenite::{connect_async, tungstenite::Message};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Debug)]
pub struct WsMessage {
    pub msg_type: String,
    pub msg_id: String,
    pub data: serde_json::Value,
}

#[derive(Serialize, Deserialize)]
pub struct VersionNegotiation {
    pub version: String,
}

#[derive(Serialize, Deserialize)]
pub struct SendRequest {
    pub files: Vec<FileInfo>,
    pub total_size: u64,
    pub thumbnail: Option<String>, // Base64 编码的缩略图
}

#[derive(Serialize, Deserialize, Clone)]
pub struct FileInfo {
    pub name: String,
    pub size: u64,
    pub modified_time: u64,
}

/// 发送端 WebSocket 服务器
pub struct WebSocketServer {
    listener: tokio::net::TcpListener,
    port: u16,
}

impl WebSocketServer {
    pub async fn new(port: u16) -> Result<Self, Box<dyn std::error::Error>> {
        let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
        Ok(WebSocketServer { listener, port })
    }
    
    /// 接收来自接收端的 WebSocket 连接
    pub async fn accept_connection(&self) -> Result<(), Box<dyn std::error::Error>> {
        let (stream, _) = self.listener.accept().await?;
        
        // 升级为 WebSocket
        let ws_stream = tokio_tungstenite::accept_async(stream).await?;
        
        // 处理消息
        self.handle_client(ws_stream).await?;
        
        Ok(())
    }
    
    /// 处理客户端消息
    async fn handle_client(
        &self,
        mut ws: tokio_tungstenite::WebSocketStream<TcpStream>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        use tokio_tungstenite::tungstenite::Message;
        
        while let Some(msg) = tokio_stream::StreamExt::next(&mut ws).await {
            match msg? {
                Message::Text(text) => {
                    let ws_msg: WsMessage = serde_json::from_str(&text)?;
                    
                    match ws_msg.msg_type.as_str() {
                        "versionNegotiation" => {
                            // 响应版本协商
                            let response = WsMessage {
                                msg_type: "versionNegotiation".to_string(),
                                msg_id: ws_msg.msg_id,
                                data: serde_json::json!({ "version": "1.0" }),
                            };
                            ws.send(Message::Text(serde_json::to_string(&response)?)).await?;
                        }
                        "confirmReceive" => {
                            // 接收端确认准备接收
                            println!("接收端已确认，准备开始传输");
                        }
                        _ => {}
                    }
                }
                Message::Close(_) => break,
                _ => {}
            }
        }
        
        Ok(())
    }
}

/// 接收端 WebSocket 客户端
pub struct WebSocketClient {
    url: String,
}

impl WebSocketClient {
    pub fn new(server_ip: &str, server_port: u16) -> Self {
        let url = format!("ws://{}:{}/websocket", server_ip, server_port);
        WebSocketClient { url }
    }
    
    /// 连接并进行握手
    pub async fn handshake(
        &self,
        files: Vec<FileInfo>,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let (ws_stream, _) = connect_async(&self.url).await?;
        
        // 1. 发送版本协商消息
        let version_msg = WsMessage {
            msg_type: "versionNegotiation".to_string(),
            msg_id: uuid::Uuid::new_v4().to_string(),
            data: serde_json::json!({ "version": "1.0" }),
        };
        
        let (mut write, mut read) = tokio::io::split(ws_stream);
        write.send(Message::Text(serde_json::to_string(&version_msg)?)).await?;
        
        // 2. 接收版本响应
        // ...
        
        // 3. 发送发送请求
        let total_size: u64 = files.iter().map(|f| f.size).sum();
        let send_request = WsMessage {
            msg_type: "sendRequest".to_string(),
            msg_id: uuid::Uuid::new_v4().to_string(),
            data: serde_json::to_value(SendRequest {
                files,
                total_size,
                thumbnail: None,
            })?,
        };
        
        write.send(Message::Text(serde_json::to_string(&send_request)?)).await?;
        
        Ok(())
    }
}
```

---

#### 4.2 HTTP 文件传输 (发送端服务器)

**文件**: `src/transfer/http_server.rs`

```rust
use axum::{
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::get,
    Router,
};
use std::sync::Arc;
use tokio::fs::File;
use tokio::io::AsyncReadExt;

#[derive(serde::Deserialize)]
pub struct DownloadQuery {
    pub task_id: String,
}

pub struct HttpServerState {
    pub file_path: String,
    pub chunk_size: usize,
}

pub async fn start_http_server(
    port: u16,
    file_path: String,
) -> Result<(), Box<dyn std::error::Error>> {
    let state = Arc::new(HttpServerState {
        file_path,
        chunk_size: 65536, // 64KB chunks
    });
    
    let app = Router::new()
        .route("/download", get(handle_download))
        .with_state(state);
    
    let listener = tokio::net::TcpListener::bind(format!("0.0.0.0:{}", port)).await?;
    axum::serve(listener, app).await?;
    
    Ok(())
}

async fn handle_download(
    Query(query): Query<DownloadQuery>,
    State(state): State<Arc<HttpServerState>>,
) -> impl IntoResponse {
    // 1. 验证 task_id
    // 2. 打开文件
    match File::open(&state.file_path).await {
        Ok(mut file) => {
            let mut buffer = vec![0u8; state.chunk_size];
            
            // 流式读取文件并发送
            match file.read(&mut buffer).await {
                Ok(n) if n > 0 => {
                    let data = buffer[..n].to_vec();
                    // 返回文件数据流
                    (StatusCode::OK, data).into_response()
                }
                _ => (StatusCode::NOT_FOUND, "File not found").into_response(),
            }
        }
        Err(_) => (StatusCode::NOT_FOUND, "File not found").into_response(),
    }
}
```

---

#### 4.3 HTTP 文件下载 (接收端客户端)

**文件**: `src/transfer/http_client.rs`

```rust
use reqwest::Client;
use tokio::fs::File;
use tokio::io::AsyncWriteExt;

pub struct HttpDownloader {
    client: Client,
    server_ip: String,
    server_port: u16,
}

impl HttpDownloader {
    pub fn new(server_ip: String, server_port: u16) -> Self {
        HttpDownloader {
            client: Client::new(),
            server_ip,
            server_port,
        }
    }
    
    /// 下载文件
    pub async fn download(
        &self,
        task_id: &str,
        output_path: &str,
        progress_callback: impl Fn(u64, u64) + Send + 'static,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let url = format!(
            "http://{}:{}/download?taskId={}",
            self.server_ip, self.server_port, task_id
        );
        
        // 1. 发起 GET 请求
        let response = self.client.get(&url).send().await?;
        let total_size = response.content_length().unwrap_or(0);
        
        // 2. 创建输出文件
        let mut file = File::create(output_path).await?;
        let mut downloaded = 0u64;
        let mut stream = response.bytes_stream();
        
        // 3. 流式接收数据
        use futures::StreamExt;
        while let Some(chunk) = stream.next().await {
            let chunk = chunk?;
            file.write_all(&chunk).await?;
            downloaded += chunk.len() as u64;
            
            // 更新进度
            progress_callback(downloaded, total_size);
        }
        
        Ok(())
    }
}
```

---

### 5. 数据结构定义 (`models/`)

**文件**: `src/models/mod.rs`

```rust
use serde::{Deserialize, Serialize};

/// P2P 连接信息 (加密传输)
#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct P2pInfo {
    pub ssid: String,          // WiFi 热点 SSID
    pub password: String,      // WiFi 密码
    pub mac_address: String,   // 发送端 MAC 地址
    pub port: u16,             // 数据传输服务器端口
    pub go_intent: u8,         // WiFi P2P Group Owner Intent
}

impl P2pInfo {
    /// 验证 P2P 信息的完整性
    pub fn validate(&self) -> Result<(), String> {
        if self.ssid.is_empty() {
            return Err("SSID cannot be empty".to_string());
        }
        if self.password.len() < 8 {
            return Err("Password too short".to_string());
        }
        if self.port == 0 || self.port > 65535 {
            return Err("Invalid port".to_string());
        }
        Ok(())
    }
}

/// 设备信息
#[derive(Serialize, Deserialize)]
pub struct DeviceInfo {
    pub name: String,
    pub os: String,
    pub model: String,
    pub public_key: String,    // ECDH 公钥
}

/// 传输任务
#[derive(Clone)]
pub struct TransferTask {
    pub id: String,
    pub files: Vec<crate::transfer::websocket_handler::FileInfo>,
    pub status: TransferStatus,
}

#[derive(Clone, Debug)]
pub enum TransferStatus {
    Pending,
    InProgress { downloaded: u64 },
    Completed,
    Failed { reason: String },
}
```

---

## 技术栈选型

| 模块 | 技术选择 | 理由 |
|------|--------|------|
| BLE 通信 | `blurz` + `dbus` | 原生 BlueZ D-Bus 接口，最接近系统 |
| 网络通信 | `tokio` + `axum` | 高性能异步 I/O，WebSocket 支持 |
| 加密 | `ring` + `aes` + `elliptic-curve` | 安全、可靠的密码学库 |
| WebSocket | `tokio-tungstenite` | 成熟的 Rust WebSocket 库 |
| HTTP 客户端 | `reqwest` | 易用且功能完整 |
| 序列化 | `serde` + `serde_json` | 标准化的 Rust 序列化框架 |
| CLI 交互 | `clap` + `indicatif` | 现代化的命令行工具库 |
| 日志 | `tracing` + `tracing-subscriber` | 灵活的日志系统 |

---

## 实现路线图

### Phase 1: 基础框架 (Week 1-2)
- [ ] 项目初始化 (`Cargo.toml` 依赖配置)
- [ ] BLE 扫描基础实现 (侦听 BlueZ D-Bus 信号)
- [ ] WiFi 接口信息获取模块
- [ ] ECDH 密钥生成与交换

### Phase 2: BLE 模块完整实现 (Week 3-4)
- [ ] GATT Server 创建与特征值管理
- [ ] BLE 广播数据格式定义与编解码
- [ ] BLE 连接握手流程
- [ ] 密钥交换与数据加密

### Phase 3: WiFi P2P 模块 (Week 5-6)
- [ ] WiFi Direct 热点创建 (wpa_supplicant 集成)
- [ ] DHCP 服务器配置
- [ ] 热点连接模块 (nmcli/wpa_supplicant)
- [ ] 网络连接验证

### Phase 4: 数据传输层 (Week 7-8)
- [ ] WebSocket 服务器与客户端
- [ ] HTTP 文件传输接口
- [ ] 文件流读写与大文件支持
- [ ] 传输进度跟踪

### Phase 5: 应用层与测试 (Week 9-10)
- [ ] CLI 交互界面
- [ ] 日志系统集成
- [ ] 单元测试与集成测试
- [ ] 与 CatShare 协议兼容性测试

---

## 关键实现细节

### 1. BLE 广告数据格式

```
Offset  Length  Description
------  ------  -----------
0       1       AD Flags (0x06 = LE General Discoverable Mode, BR/EDR not supported)
1       1       AD Length (0x02)
2       2       AD Type (0x01) + Value
4       16      Service UUID (00003331-0000-1000-8000-008123456789)
20      32      Device Name (Variable length, max 32 bytes)
52      1       Brand ID High byte
53      1       Brand ID Low byte
```

### 2. ECDH 密钥交换流程

```
发送端:
1. 生成本地密钥对 (EC P-256)
   - private_key_s, public_key_s
2. 将 public_key_s (Base64) 写入 CHAR_P2P
3. 接收来自接收端的数据，其中包含 public_key_r
4. ECDH(private_key_s, public_key_r) -> shared_secret
5. KDF(shared_secret) -> session_key

接收端:
1. 生成本地密钥对
   - private_key_r, public_key_r
2. 读取 CHAR_P2P 中的 public_key_s
3. ECDH(private_key_r, public_key_s) -> shared_secret
4. KDF(shared_secret) -> session_key
```

### 3. WiFi P2P 连接流程

```
发送端:
1. wpa_supplicant 创建 P2P Group (GO)
   p2p_group_add ssid="DIRECT-xy" psk="password"
2. 配置 IP: 192.168.49.1/24
3. 启动 DHCP 服务: dnsmasq

接收端:
1. nmcli device wifi connect "DIRECT-xy" password "password"
   或 wpa_supplicant 创建 P2P Network
2. 等待 DHCP 分配 IP
3. 验证网络连通性 (ping 192.168.49.1)
```

---

## 依赖配置示例

**`Cargo.toml`**

```toml
[package]
name = "cattysend-linux"
version = "0.1.0"
edition = "2021"

[dependencies]
# 异步运行时
tokio = { version = "1", features = ["full"] }
futures = "0.3"

# BLE 通信
blurz = "0.6"
dbus = "0.9"

# 网络通信
axum = "0.7"
tokio-tungstenite = "0.21"
reqwest = { version = "0.11", features = ["stream"] }
hyper = "0.14"

# 加密
ring = "0.17"
aes = "0.8"
block-modes = "0.9"
hmac = "0.12"
sha2 = "0.10"
elliptic-curve = "0.13"
p256 = "0.13"

# 序列化
serde = { version = "1.0", features = ["derive"] }
serde_json = "1.0"
uuid = { version = "1.0", features = ["v4", "serde"] }

# 工具库
clap = { version = "4.0", features = ["derive"] }
indicatif = "0.17"
tracing = "0.1"
tracing-subscriber = "0.3"
anyhow = "1.0"
rand = "0.8"
base64 = "0.21"

[dev-dependencies]
tokio-test = "0.4"
```

---

## 测试与调试

### 1. BlueZ D-Bus 调试

```bash
# 列出所有蓝牙设备
gdbus call --system \
  --dest org.bluez \
  --object-path / \
  --method org.freedesktop.DBus.ObjectManager.GetManagedObjects

# 监听 D-Bus 信号
dbus-monitor --system "interface=org.bluez.Adapter1"
```

### 2. WiFi 信息查看

```bash
# 列出 WiFi 接口
iw dev

# 查看 P2P 设备
iw dev wlan0 link
```

### 3. 日志与追踪

```rust
use tracing::{info, debug, error};

#[tokio::main]
async fn main() {
    tracing_subscriber::fmt::init();
    
    info!("Starting CattyShare");
    // ...
}
```

---

## 参考资源

- **CatShare (Android 实现)**: https://github.com/kmod-midori/CatShare
- **BlueZ 官方文档**: http://www.bluez.org/
- **Rust Bluetooth**: https://github.com/bluez/blurz
- **WiFi P2P 规范**: https://en.wikipedia.org/wiki/Wi-Fi_Direct
- **互传联盟协议**: 各品牌的私有实现，需要逆向工程

---

## 常见问题

### Q: 为什么选择 Rust 而不是 Python?
A: Rust 提供内存安全保证、更好的并发支持和性能。对于 BLE 通信这种低延迟场景尤为重要。

### Q: Linux 上是否可以完全兼容 Android 版本?
A: 大部分可以。主要差异是：
- BLE 操作依赖 BlueZ（开源，兼容性好）
- WiFi P2P 需要依赖 wpa_supplicant（不完全支持所有特性）
- MAC 地址获取可直接读取 `/sys/class/net/`，无权限限制

### Q: 如何处理与其他品牌设备的互通?
A: 维持协议兼容性是关键：
- 使用相同的 UUID 定义
- 遵循相同的加密算法 (ECDH + AES-256-CTR)
- 消息格式使用标准 JSON

---

## 许可证

该项目计划采用 MIT 或 GPL v3 许可证，具体见项目初始化时确定。

---

**文档更新于**: 2026-01-09
**最后修改**: 初始版本
