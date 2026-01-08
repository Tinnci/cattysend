# 互传联盟协议规范 (Linux/Rust 实现)

## 目录
1. [协议层次结构](#协议层次结构)
2. [BLE 层协议](#ble-层协议)
3. [P2P 层协议](#p2p-层协议)
4. [传输层协议](#传输层协议)
5. [消息格式](#消息格式)
6. [状态机](#状态机)

---

## 协议层次结构

```
┌─────────────────────────────────────┐
│      应用层 (Application)           │
│  - 文件传输  - 用户交互             │
├─────────────────────────────────────┤
│    会话层 (Session)                 │
│  - WebSocket 握手  - 任务管理       │
├─────────────────────────────────────┤
│    传输层 (Transport)               │
│  - HTTP/WebSocket  - TCP/IP         │
├─────────────────────────────────────┤
│    链路层 (Link)                    │
│  - BLE GATT  - WiFi Direct          │
├─────────────────────────────────────┤
│    网络层 (Network)                 │
│  - IPv4/IPv6  - 路由                │
├─────────────────────────────────────┤
│   物理层 (Physical)                 │
│  - Bluetooth LE  - 802.11ac         │
└─────────────────────────────────────┘
```

---

## BLE 层协议

### 1.1 UUID 定义

| 用途 | UUID | 说明 |
|------|------|------|
| 广告服务 UUID | `00003331-0000-1000-8000-008123456789` | BLE 广播中的服务标识 |
| 主服务 UUID | `00009955-0000-1000-8000-00805f9b34fb` | 完整的 GATT 服务 |
| 状态特征值 UUID | `00009954-0000-1000-8000-00805f9b34fb` | 设备状态 (读/通知) |
| P2P 特征值 UUID | `00009953-0000-1000-8000-00805f9b34fb` | P2P 连接信息 (读/写) |

### 1.2 BLE 广告数据格式

```
BLE Advertisement Structure:
┌─────────────┬─────────┬──────────────────┬──────────┬──────────┐
│   AD Type   │ Length  │   AD Data        │  Flags   │ Device   │
│   (0x16)    │  (2)    │  (16 bytes)      │  (1)     │ Name (?) │
├─────────────┼─────────┼──────────────────┼──────────┼──────────┤
│ 0xFF        │ 0x1A    │ Service UUID     │ 0x??     │ "OPPO"   │
│ (Mfg Data)  │         │ (00003331-...)   │          │ /Xiaomi  │
└─────────────┴─────────┴──────────────────┴──────────┴──────────┘

详细 AD 数据:
Offset  Len  Type   Value        Description
------  ---  ----   -----        -----------
0       1    0x01   0x06         Flags (LE General Discoverable)
2       2    0x03   UUID (2B)    16-bit Service UUID List
4       16   0xFF   0x33310000...Service UUID (00003331-...)
20      1    0x09   len          Device Name Length
21      N    0x09   "OPPO"       Device Name
...     2    N/A    0x??0x??     Brand ID (OPPO=0x0085, Xiaomi=0x004C)
```

### 1.3 品牌 ID 映射

| 品牌 | ID (16-bit LE) | 说明 |
|------|----------------|------|
| Xiaomi | `0x004C` | 小米 |
| OPPO | `0x0085` | OPPO/OnePlus |
| Vivo | `0x0077` | Vivo |
| Samsung | `0x0075` | 三星 |
| Huawei | `0x0047` | 华为 |

### 1.4 BLE 连接握手流程

```
发送端 (Sender)                    接收端 (Receiver/Advertiser)
     │                                     │
     │  1. 启动扫描                        │
     │  BluetoothAdapter.startDiscovery()  │
     │                                     │
     │◄──────────────────────────────────┤
     │  2. 接收 BLE 广告包                 │
     │     解析 Service UUID               │
     │     提取设备名称 & Brand ID         │
     │                                     │
     │  3. 用户选择设备，发起连接         │
     │  BluetoothGatt.connectGatt()    ──►│
     │                                     │ 4. 接收连接请求
     │                                     │    GattServerCallback
     │                                     │    .onCharacteristicRead()
     │                                     │
     │◄─────────────────────────────────┤
     │  5. 读取 CHAR_STATUS 特征值       │
     │     获取接收端的设备信息           │
     │                                     │
     │  6. 读取接收端的 ECDH 公钥        │
     │     (通过 CHAR_STATUS)            │
     │                                     │
     │  7. 生成本地密钥对并计算共享密钥   │
     │     ECDH(local_private,            │
     │          remote_public)            │
     │                                     │
     │  8. 加密 P2pInfo 对象              │
     │     使用 AES-256-CTR               │
     │                                     │
     │  9. 写入 CHAR_P2P                 │
     │     (加密的 P2pInfo JSON)      ──►│
     │                                     │ 10. 接收加密数据
     │                                     │     解密 P2pInfo
     │                                     │     启动接收服务
     │                                     │     (P2pReceiverService)
     │                                     │
     │  11. 断开 BLE 连接                 │
     │  BluetoothGatt.disconnect()    ──►│
     │                                     │ 12. GATT 连接断开
     ▼                                     ▼
```

### 1.5 CHAR_STATUS 特征值数据格式

```json
{
  "deviceName": "小米 12",
  "osVersion": "Android 12",
  "model": "2212131C",
  "publicKey": "MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAE...",
  "senderVersion": "1.0"
}
```

---

## P2P 层协议

### 2.1 P2pInfo 结构体

```rust
pub struct P2pInfo {
    pub ssid: String,           // WiFi P2P SSID
    pub psk: String,            // WiFi P2P Pre-Shared Key (密码)
    pub mac_address: String,    // 发送端 MAC 地址 (用于验证)
    pub port: u16,              // 数据传输服务器端口
    pub go_intent: u8,          // WiFi P2P Group Owner Intent
    pub band_preference: u8,    // 频段偏好 (0=双频, 1=5GHz, 2=2.4GHz)
}
```

### 2.2 P2P 连接建立流程

```
发送端                           接收端
(Group Owner)                  (Client)
     │                            │
     │  1. 创建 WiFi P2P 组        │
     │  wpa_supplicant:            │
     │  p2p_group_add              │
     │  SSID=DIRECT-xy             │
     │  psk=<随机密码>             │
     │                             │
     │  2. 配置 IP 地址            │
     │  192.168.49.1/24            │
     │                             │
     │  3. 启动 DHCP 服务          │
     │  dnsmasq / udhcpd           │
     │                             │
     │  4. 启动 HTTP 服务器        │
     │  listen on 0.0.0.0:33331    │
     │                             │
     │  5. 发送 P2P 连接信息      │
     │  (via BLE GATT)         ──►│
     │                             │ 6. 接收 P2P 信息
     │                             │    解密
     │                             │    验证 MAC 地址
     │                             │
     │◄──────────────────────────┤ 7. 扫描 WiFi 网络
     │  8. 接收 WiFi 连接请求    │    寻找 DIRECT-xy
     │                             │
     │  9. 允许 P2P 设备加入      │
     │  (D-Bus WPS/自动授权)      │
     │                             │ 10. 连接 WiFi P2P
     │                             │     wpa_supplicant:
     │                             │     p2p_connect
     │                             │     peer_addr mac_addr
     │                             │     pbc/pin
     │                             │
     │                             │ 11. 获取 DHCP IP
     │                             │     (如 192.168.49.2)
     │                             │
     │◄──────────────────────────┤ 12. 验证网络连通性
     │ (PING 192.168.49.1)        │     (PING 发送端)
     │                             │
     ▼                             ▼
```

---

## 传输层协议

### 3.1 WebSocket 协议

#### 3.1.1 握手阶段

```
C: GET /websocket HTTP/1.1
   Host: 192.168.49.1:33331
   Upgrade: websocket
   Connection: Upgrade
   Sec-WebSocket-Key: x3JJHMbDL1EzLkh9GBhXDw==
   Sec-WebSocket-Version: 13

S: HTTP/1.1 101 Switching Protocols
   Upgrade: websocket
   Connection: Upgrade
   Sec-WebSocket-Accept: HSmrc0sMlYUkAGmm5OPpG2HaGWk=
```

#### 3.1.2 WebSocket 消息格式

所有消息通过 WebSocket Text Frame 传输，JSON 编码：

```json
{
  "msgType": "versionNegotiation|sendRequest|confirmReceive|cancel",
  "msgId": "uuid-string",
  "data": {...}
}
```

#### 3.1.3 消息流程

```
接收端                          发送端
(WebSocket Client)              (WebSocket Server)
     │                              │
     │  1. 连接 WebSocket          │
     │◄─────────────────────────────┤
     │  WebSocket connected         │
     │                              │
     │  2. 发送版本协商             │
     │  msgType=versionNegotiation  │
     │  data={version: "1.0"}    ──►│
     │                              │ 3. 验证版本
     │◄──────────────────────────────│
     │  4. 接收版本响应             │
     │  msgType=versionNegotiation  │
     │  data={version: "1.0"}       │
     │                              │
     │                              │ 5. 构建文件列表
     │◄──────────────────────────────│
     │  6. 接收发送请求             │
     │  msgType=sendRequest         │
     │  data={                      │
     │    files: [...],             │
     │    totalSize: 1024000,       │
     │    thumbnail: "base64..."    │
     │  }                           │
     │                              │
     │  7. 用户确认接收             │
     │  (CLI 提示)                  │
     │                              │
     │  8. 发送确认消息         ──►│
     │  msgType=confirmReceive      │
     │  data={accepted: true}       │
     │                              │ 9. 准备文件流
     │                              │    (如果是多个文件
     │                              │     动态打包 ZIP)
     │                              │
     │  10. HTTP GET /download   ──►│
     │  (接下来的数据传输)          │
     │                              │
     ▼                              ▼
```

### 3.2 HTTP 文件传输

#### 3.2.1 下载请求

```http
GET /download?taskId=<uuid>&startByte=0&endByte=-1 HTTP/1.1
Host: 192.168.49.1:33331
Connection: close
Accept: application/octet-stream
```

#### 3.2.2 下载响应

```http
HTTP/1.1 200 OK
Content-Type: application/octet-stream
Content-Length: 1024000
Transfer-Encoding: chunked
Content-Disposition: attachment; filename="files.zip"

[Binary file data stream...]
```

#### 3.2.3 分块传输优化

```
块大小: 64KB (65536 bytes)
超时: 30 秒
重试: 3 次
断点续传: 支持 Range header

C: GET /download?taskId=xxx HTTP/1.1
   Range: bytes=1048576-2097151

S: HTTP/1.1 206 Partial Content
   Content-Range: bytes 1048576-2097151/10485760
   Content-Length: 1048576
   
   [Block data...]
```

---

## 消息格式

### 4.1 versionNegotiation 消息

**方向**: 双向

```json
{
  "msgType": "versionNegotiation",
  "msgId": "550e8400-e29b-41d4-a716-446655440000",
  "data": {
    "version": "1.0"
  }
}
```

### 4.2 sendRequest 消息

**方向**: 发送端 → 接收端

```json
{
  "msgType": "sendRequest",
  "msgId": "550e8400-e29b-41d4-a716-446655440001",
  "data": {
    "files": [
      {
        "name": "document.pdf",
        "size": 1024000,
        "modifiedTime": 1704790800,
        "mimeType": "application/pdf"
      },
      {
        "name": "photo.jpg",
        "size": 2048000,
        "modifiedTime": 1704790900,
        "mimeType": "image/jpeg"
      }
    ],
    "totalSize": 3072000,
    "totalFiles": 2,
    "packageType": "single|multi",
    "thumbnail": "iVBORw0KGgoAAAANSUhEUgAAAAEAAAABCAYAAAAfFcSJAAAADUlEQVR42mNkYPhfDwAChwGA60e6kgAAAABJRU5ErkJggg==",
    "senderDevice": "小米 12"
  }
}
```

### 4.3 confirmReceive 消息

**方向**: 接收端 → 发送端

```json
{
  "msgType": "confirmReceive",
  "msgId": "550e8400-e29b-41d4-a716-446655440002",
  "data": {
    "accepted": true,
    "reason": "user_confirmed|auto_confirmed",
    "downloadDir": "/home/user/Downloads"
  }
}
```

### 4.4 cancel 消息

**方向**: 任意方向

```json
{
  "msgType": "cancel",
  "msgId": "550e8400-e29b-41d4-a716-446655440003",
  "data": {
    "reason": "user_cancel|network_error|timeout",
    "message": "User cancelled transfer"
  }
}
```

### 4.5 progressUpdate 消息

**方向**: 接收端 → 发送端 (可选，用于显示进度)

```json
{
  "msgType": "progressUpdate",
  "msgId": "550e8400-e29b-41d4-a716-446655440004",
  "data": {
    "downloadedBytes": 512000,
    "totalBytes": 1024000,
    "currentFile": "document.pdf",
    "speed": 1024000,
    "remainingSeconds": 10
  }
}
```

---

## 状态机

### 5.1 发送端状态机

```
┌──────────────┐
│  Idle        │ ◄──────────────────┐
└──────┬───────┘                      │
       │ startSend()                  │
       ▼                              │
┌──────────────┐                      │
│ BLE Ready    │ ◄──────────────────┐ │
└──────┬───────┘                    │ │
       │ device.selected()           │ │
       ▼                             │ │
┌──────────────────┐                │ │
│ GATT Connected   │                │ │
└──────┬───────────┘                │ │
       │ create_p2p_group()          │ │
       ▼                             │ │
┌──────────────────┐                │ │
│ P2P Ready        │                │ │
└──────┬───────────┘                │ │
       │ send_p2p_info()             │ │
       ▼                             │ │
┌──────────────────┐                │ │
│ Waiting for      │                │ │
│ Connection       │                │ │
└──────┬───────────┘                │ │
       │ connection_received()       │ │
       ▼                             │ │
┌──────────────────┐                │ │
│ WebSocket        │                │ │
│ Connected        │                │ │
└──────┬───────────┘                │ │
       │ send_request()              │ │
       ▼                             │ │
┌──────────────────┐                │ │
│ Waiting for      │                │ │
│ Confirmation     │                │ │
└──────┬───────────┘                │ │
       │ confirmation_received()     │ │
       ▼                             │ │
┌──────────────────┐                │ │
│ Transferring     │                │ │
└──────┬───────────┘                │ │
       │ transfer_completed()        │ │
       ▼                             │ │
┌──────────────────┐                │ │
│ Completed        │────────────────┘ │
└──────────────────┘ cleanup()        │
                                      │
                   [Error Path]       │
┌──────────────────┐                 │
│ Error            │─────────────────┘
└──────────────────┘ abort()
```

### 5.2 接收端状态机

```
┌──────────────┐
│  Idle        │ ◄──────────────────┐
└──────┬───────┘                      │
       │ startReceive()               │
       ▼                              │
┌──────────────┐                      │
│ BLE Scanning │                      │
└──────┬───────┘                      │
       │ device_found()               │
       ▼                              │
┌──────────────────┐                  │
│ Device Selected  │                  │
└──────┬───────────┘                  │
       │ connect_gatt()               │
       ▼                              │
┌──────────────────┐                  │
│ GATT Connected   │                  │
└──────┬───────────┘                  │
       │ read_p2p_info()              │
       ▼                              │
┌──────────────────┐                  │
│ P2P Connecting   │                  │
└──────┬───────────┘                  │
       │ p2p_connected()              │
       ▼                              │
┌──────────────────┐                  │
│ WiFi Connected   │                  │
└──────┬───────────┘                  │
       │ connect_websocket()          │
       ▼                              │
┌──────────────────┐                  │
│ WebSocket        │                  │
│ Connected        │                  │
└──────┬───────────┘                  │
       │ request_received()           │
       ▼                              │
┌──────────────────┐                  │
│ Waiting for      │                  │
│ User Confirm     │                  │
└──────┬───────────┘                  │
       │ user_confirmed()             │
       ▼                              │
┌──────────────────┐                  │
│ Downloading      │                  │
└──────┬───────────┘                  │
       │ download_completed()         │
       ▼                              │
┌──────────────────┐                  │
│ Completed        │──────────────────┘
└──────────────────┘ cleanup()
                      
                   [Error Path]       
┌──────────────────┐                 
│ Error            │─────────────────┘
└──────────────────┘ abort()
```

---

## 加密算法规范

### 6.1 ECDH 密钥交换

```
曲线: P-256 (secp256r1)
公钥编码: X.509 SubjectPublicKeyInfo (Base64)
私钥编码: PKCS#8 (不传输，本地保存)

交换过程:
1. 生成本地密钥对 (P-256)
   KeyPairGenerator.getInstance("EC").initialize(256)
   
2. 提取对方公钥 (Base64 解码)
   KeyFactory.getInstance("EC").generatePublic()
   
3. ECDH 计算共享密钥
   KeyAgreement.getInstance("ECDH").doPhase()
   
4. KDF 派生会话密钥
   HKDF-SHA256(shared_secret, salt="", info="")
   
5. 截断为 256 bits (32 bytes) AES-256 密钥
```

### 6.2 AES-256-CTR 加密

```
算法: AES-256-CTR (Counter Mode, No Padding)
密钥大小: 256 bits (32 bytes)
IV: "0102030405060708" (64 bits, 固定)
计数器: 初始值 0，大端序 (Big Endian)

加密过程:
1. 将明文转为 UTF-8 字节
2. 初始化 IV 和计数器
3. 生成密钥流: AES-Encrypt(Key, IV || Counter++)
4. 密钥流 XOR 明文 = 密文
5. Base64 编码密文

解密过程:
1. Base64 解码
2. 按上述流程生成密钥流
3. 密钥流 XOR 密文 = 明文
4. 转为字符串
```

### 6.3 MAC 地址验证

```
发送端:
1. 获取 P2P 接口 MAC 地址
   /sys/class/net/<iface>/address
   
2. 序列化 P2pInfo，在 JSON 中包含 MAC
   
3. ECDH 派生密钥后加密整个 JSON

接收端:
1. 解密获得 P2pInfo JSON
2. 提取 MAC 地址
3. 与接收到的蓝牙设备地址对比 (可选)
   确保来自同一设备

验证目的:
- 防止中间人攻击 (MITM)
- 确保设备身份
- 符合互传联盟协议要求
```

---

## 错误处理与重试机制

### 7.1 BLE 连接错误

| 错误码 | 原因 | 恢复策略 |
|-------|------|--------|
| GATT_ERROR | 连接断开 | 重新扫描并重连，最多 3 次 |
| TIMEOUT | 读写超时 | 增加等待时间，重试 |
| PERMISSION_DENIED | 权限不足 | 提示用户授权蓝牙权限 |

### 7.2 WiFi P2P 连接错误

| 错误码 | 原因 | 恢复策略 |
|-------|------|--------|
| NO_NETWORK | 未找到热点 | 重新扫描 WiFi，等待 5 秒 |
| AUTHENTICATION_FAILED | 密码错误 | 从 BLE 重新读取信息 |
| DHCP_TIMEOUT | 获取 IP 失败 | 手动配置 IP，重试 DHCP |

### 7.3 WebSocket 连接错误

| 错误码 | 原因 | 恢复策略 |
|-------|------|--------|
| CONNECTION_REFUSED | 服务器未启动 | 等待 2 秒，重新连接 |
| MESSAGE_TIMEOUT | 消息超时 | 重新发送，最多 3 次 |
| PROTOCOL_ERROR | 协议不匹配 | 通知用户版本不兼容 |

---

## 性能与优化指标

### 8.1 目标指标

| 指标 | 目标值 | 说明 |
|------|-------|------|
| BLE 扫描时间 | < 10 秒 | 发现周围设备 |
| BLE 连接时间 | < 5 秒 | GATT 握手 |
| P2P 连接时间 | < 10 秒 | WiFi 热点连接 + DHCP |
| WebSocket 握手 | < 2 秒 | 连接与版本协商 |
| 文件传输速度 | > 100 MB/s | 5GHz WiFi Direct 理论值 |
| 内存占用 | < 100 MB | 整个应用的内存 |

### 8.2 优化策略

- **BLE**: 使用主动扫描，缩短扫描间隔
- **P2P**: 预加载常用设备，跳过冗长握手
- **HTTP**: 支持分块传输 & 断点续传
- **内存**: 流式读写文件，避免全量加载

---

**文档版本**: 1.0  
**最后更新**: 2026-01-09
