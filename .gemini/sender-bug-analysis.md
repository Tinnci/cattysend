# 发送端Bug分析与修复方案

## 当前问题

根据运行日志，发送端存在以下主要问题：

```
[2026-01-21T11:22:31Z WARN] NM D-Bus hotspot failed: Timeout waiting for IP address
[2026-01-21T11:22:31Z WARN] wpa_cli also failed: wpa_cli p2p_group_add failed: Permission denied
```

## 与Java源码的关键差异

### 1. Wi-Fi P2P创建方式

**Java (Android WifiP2pManager):**
- 使用 `WifiP2pConfig.Builder()` 配置P2P组
- 使用 `WifiP2pManager.createGroup()` 创建
- 接口名称由系统动态分配（如 `p2p-wlan0-0`）
- 通过系统广播 `WIFI_P2P_CONNECTION_CHANGED_ACTION` 获取组信息

**Rust (NetworkManager):**
- 创建 hotspot 连接（`connection.type = "802-11-wireless", mode = "ap"`）
- 接口名称通常保持为`wlan0`
- 通过D-Bus等待连接激活并获取IP地址

**问题分析：**
两种方式的IPv4配置机制不同：
- Android P2P：自动配置`192.168.49.x`网段
- NM Hotspot：需要配置`ipv4.method = "shared"`，IP通常是`10.42.0.1`

### 2. IP地址分配超时

**原因：**
`nm_dbus.rs` 中的 `wait_for_ip()` 函数可能：
1. 等待的D-Bus属性路径不正确
2. NetworkManager共享模式的IP分配较慢
3. 超时时间设置不合理（当前15秒）

**Java对比：**
Java代码直接使用WifiP2pGroup信息，不需要等待IP分配完成，因为Android P2P框架会自动处理。

### 3. wpa_cli备用方案失败

**错误信息：**
```
Failed to connect to non-global ctrl_ifname: wlan0  error: Permission denied
```

**原因：**
- wpa_supplicant控制接口需要root或特定组权限
- 应该使用 `-g` 参数连接全局接口：`wpa_cli -g /run/wpa_supplicant/global`
- P2P命令格式可能不正确

**正确的wpa_cli P2P命令：**
```bash
wpa_cli -g /run/wpa_supplicant/global p2p_group_add
wpa_cli -g /run/wpa_supplicant/global p2p_set_ssid "DIRECT-xxxxx"
```

### 4. MAC地址获取

**Java实现：**
```kotlin
val p2pMac = ShizukuUtils.getMacAddress(this@P2pSenderService, "p2p0")
```

使用Shizuku（特权服务）通过 `NetworkInterface.getByName("p2p0")` 获取。

**Rust实现问题：**
- 硬编码接口名 `wlan0` 或 `p2p-dev-wlan0`
- 实际P2P组接口可能是其他名称
- 应该从NetworkManager返回的激活连接中获取实际接口名

### 5. WebSocket协议实现

**Java (Ktor WebSocket):**
```kotlin
webSocket("/websocket") {
    send(Frame.Text(WebSocketMessage(...).toText()))
    incoming.receive() as? Frame.Text
}
```

**Rust (tokio-tungstenite):**
需要确保TLS证书配置正确，与Java的自签名证书兼容。

## 修复方案 Priority List

### 🔥 优先级1：修复IP地址等待逻辑

1. **Option A**: 移除IP地址等待要求
   - NetworkManager创建热点后，接口应该立即可用
   - 不需要等待完整的IPv4配置完成
   - 只需要等待连接状态变为`ACTIVATED`

2. **Option B**: 改进等待逻辑
   - 检查 `Ip4Config` 属性是否正确设置
   - 增加超时时间到30秒
   - 添加更详细的错误日志

**推荐**：先实施Option A，如果接收端连接有问题再考虑Option B。

### 🔥 优先级2：修复wpa_cli备用方案

```rust
// p2p_sender.rs 修改
async fn create_p2p_group_wpa(&self, ssid: &str, psk: &str) -> anyhow::Result<()> {
    // 使用全局接口
    let output = Command::new("wpa_cli")
        .args([
            "-g", "/run/wpa_supplicant/global",
            "p2p_group_add"
        ])
        .output()?;
    
    // ... 获取接口名
    
    // 设置SSID和PSK
    let output = Command::new("wpa_cli")
        .args([
            "-i", &p2p_interface,  // 使用实际P2P接口
            "p2p_set_ssid", &format!("\"{}\"", ssid)
        ])
        .output()?;
}
```

**问题**：即使修复，仍然可能遇到权限问题。建议专注于修复NM方案。

### 优先级3：动态获取P2P接口名和MAC

```rust
async fn create_hotspot_nm(&self, ssid: &str, psk: &str) -> anyhow::Result<String> {
    // ... 现有代码 ...
    
    // 激活连接后获取实际接口
    let device_path = client.get_active_connection_device(&active_conn).await?;
    let interface = client.get_device_interface(&device_path).await?;
    
    info!("Hotspot created on interface: {}", interface);
    
    // 从实际接口读取MAC
    let mac = self.get_mac_for_interface(&interface)?;
    
    Ok(mac)
}
```

### 优先级4：对比WebSocket握手流程

需要逐步调试：
1. 确认HTTP服务器端口正确
2. 确认TLS证书被接收端接受
3. 确认WebSocket消息格式与Java完全一致

---

## 立即行动项

### 1. 测试NM热点IP配置

```bash
# 检查NM热点连接的IPv4配置
nmcli con show cattysend-hotspot-* | grep ipv4
```

预期应该看到 `ipv4.method: shared` 和 `ipv4.addresses: 10.42.0.1/24`

### 2. 修改 `nm_dbus.rs` 的 `wait_for_ip()`

可以考虑：
- 移除这个等待，直接返回默认的`10.42.0.1`
- 或者从连接配置中读取静态IP

### 3. 增加详细日志

在 `p2p_sender.rs` 的关键点添加：
```rust
info!("Connection activated: {:?}", active_conn);
info!("Device path: {:?}", device);
// 打印NM返回的所有接口信息
```

这样可以更清楚知道NM的实际行为。

---

## 测试计划

1. **单独测试NM热点创建**
   ```bash
   # 手动用nmcli创建测试
   nmcli con add type wifi ifname wlan0 con-name test-hotspot \
     ssid "DIRECT-test12" mode ap \
     wifi-sec.key-mgmt wpa-psk wifi-sec.psk "12345678" \
     ipv4.method shared
   nmcli con up test-hotspot
   ```

2. **验证IP分配速度**
   ```bash
   time nmcli con up test-hotspot
   ip addr show wlan0
   ```

3. **测试wpa_cli全局接口**
   ```bash
   sudo wpa_cli -g /run/wpa_supplicant/global status
   sudo wpa_cli -g /run/wpa_supplicant/global p2p_group_add
   ```

---

## 下一步

1. 先修复NM方案的IP等待逻辑（最简单）
2. 测试发送流程是否能继续
3. 如果BLE协商成功但Wi-Fi连接失败，再调查接收端连接问题
4. 最后优化wpa_cli备用方案
