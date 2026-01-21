# Cattysend 用户态架构说明

## 设计原则

**所有核心功能均在用户态实现，无需root权限或setuid程序。**

---

## 权限级别分层

### 层级1：完全无需权限的操作
- 文件I/O（读取用户自己的文件）
- HTTP/HTTPS服务器（绑定到用户端口 >1024）
- 日志和配置文件读写

### 层级2：需要用户组权限的操作（通过PolicyKit）

#### 蓝牙操作
**使用 `bluer` crate:**
```rust
// 用户态BlueZ D-Bus API
let session = bluer::Session::new().await?;
let adapter = session.default_adapter().await?;
```

**权限要求:**
- 用户需在 `bluetooth` 组
- 或通过PolicyKit规则授权

#### Wi-Fi管理
**使用 NetworkManager D-Bus API:**
```rust
let client = NmClient::new().await?;
client.create_hotspot("DIRECT-abc", "password", "a", "wlan0").await?;
```

**权限模型:**
1. D-Bus调用进入系统总线
2. NetworkManager检查PolicyKit策略
3. 默认策略：允许本地活跃用户管理网络
4. **无需root**，只需PolicyKit授权

**相关PolicyKit规则:**
```xml
<!-- /usr/share/polkit-1/actions/org.freedesktop.NetworkManager.policy -->
<action id="org.freedesktop.NetworkManager.network-control">
  <defaults>
    <allow_any>no</allow_any>
    <allow_inactive>no</allow_inactive>
    <allow_active>yes</allow_active>  <!-- 活跃用户允许 -->
  </defaults>
</action>
```

### 层级3：明确禁止的特权操作 ⛔

**我们不使用以下方式:**
- ❌ `sudo` 或 `pkexec` 调用
- ❌ setuid 二进制文件
- ❌ 直接访问 `/dev/` 设备节点
- ❌ `CAP_NET_ADMIN` capability
- ❌ `wpa_cli` 直接套接字访问

---

## 用户态实现细节

### 1. Wi-Fi P2P / 热点创建

**方案A: NetworkManager D-Bus（主要方案）**
```
用户程序
  ↓ D-Bus调用
NetworkManager守护进程（以root运行）
  ↓ PolicyKit检查
允许？
  ↓ 是 → 执行操作
  ↓ 否 → 拒绝
```

**为什么这是用户态:**
- 应用本身运行在用户空间
- 通过标准IPC（D-Bus）与系统服务通信
- 权限检查由系统策略决定，不是hardcoded

**方案B: wpa_cli（已废弃）**
```
用户程序
  ↓ 执行wpa_cli命令
wpa_supplicant控制套接字
  ↓ 需要root或组权限
⛔ 权限不足，拒绝
```

### 2. 蓝牙BLE操作

**BlueZ D-Bus API（用户态）**
```rust
// 广播
let adv = adapter.advertise(...).await?;

// GATT Server
let app = gatt::Application::new(...);
adapter.register_application(&app).await?;
```

**权限:**
- 默认本地用户可访问BlueZ
- 通过 `/etc/dbus-1/system.d/bluetooth.conf` 配置

### 3. 原始套接字（BLE扫描）

**问题:** `bluer` 的某些功能需要 `CAP_NET_RAW`

**解决方案:**
```rust
// 检测capability
let (_, has_net_raw) = cattysend_core::wifi::check_capabilities();

if !has_net_raw {
    log::warn!("CAP_NET_RAW not available, some features limited");
    // 降级到仅D-Bus扫描（无原始HCI访问）
}
```

**降级路径:**
- 优先使用需要`CAP_NET_RAW`的完整扫描
- 如果不可用，使用纯D-Bus API（功能受限但仍可用）

---

## 安装后的权限配置

### 推荐用户组设置
```bash
# 蓝牙访问
sudo usermod -aG bluetooth $USER

# 网络管理（某些发行版需要）
sudo usermod -aG netdev $USER

# 重新登录以应用组变更
```

### PolicyKit规则（可选，用于更严格的环境）
如果默认策略太宽松，可以创建自定义规则：

```javascript
// /etc/polkit-1/rules.d/50-cattysend.rules
polkit.addRule(function(action, subject) {
    if (action.id == "org.freedesktop.NetworkManager.network-control" &&
        subject.user == "youruser") {
        return polkit.Result.YES;
    }
});
```

---

## 与Java/Android实现的对比

### Android (CatShare)
```kotlin
// Android系统API已经是用户态
val wifiP2pManager = getSystemService(WIFI_P2P_SERVICE)
wifiP2pManager.createGroup(...)  // 系统权限检查
```

**权限声明（AndroidManifest.xml）:**
```xml
<uses-permission android:name="android.permission.ACCESS_WIFI_STATE" />
<uses-permission android:name="android.permission.CHANGE_WIFI_STATE" />
<uses-permission android:name="android.permission.ACCESS_FINE_LOCATION" />
```

### Linux (cattysend)
```rust
// 等效的用户态API
let nm_client = NmClient::new().await?;
nm_client.create_hotspot(...).await?;  // PolicyKit检查
```

**权限要求（运行时）:**
- 用户在 `bluetooth`, `netdev` 组
- PolicyKit允许网络管理

**概念一致性:** 两者都是通过系统服务 + 权限模型实现，都是用户态。

---

## 总结

✅ **Cattysend 完全实现用户态架构**

1. **核心传输**: HTTP服务器，用户端口
2. **蓝牙**: BlueZ D-Bus API + PolicyKit
3. **Wi-Fi**: NetworkManager D-Bus + PolicyKit
4. **无需**: root, sudo, setuid, 特权能力

**唯一例外**: `CAP_NET_RAW` 用于高级BLE扫描，但有降级方案。

这种设计:
- ✅ 符合现代Linux权限最佳实践
- ✅ 便于打包和分发
- ✅ 安全：最小权限原则
- ✅ 与系统策略集成良好
