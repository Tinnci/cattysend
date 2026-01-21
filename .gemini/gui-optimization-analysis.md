# GUI 发送接收结构分析与优化方案

## 📊 当前架构分析

### 1. **发送流程** (app.rs:217-291)

#### 当前实现：
```rust
let on_send = move |_| {
    // 1. 检查设备和文件
    // 2. 直接在 UI 事件回调中 spawn 任务
    // 3. 嵌套 spawn - 外层发送，内层事件转发
    // 4. 缺少任务管理（无法取消）
    // 5. sender_id 和 supports_5ghz 使用占位值
}
```

#### 🔴 **问题识别**：

| 问题 | 位置 | 影响 |
|------|------|------|
| **缺少发送任务管理** | 217-291 | 无法取消正在进行的发送 |
| **sender_id 占位值** | 281 | `sender_id: String::new()` - 应从 settings 获取 |
| **supports_5ghz 占位值** | 282 | `supports_5ghz: false` - 应从扫描结果获取 |
| **硬编码 WiFi 接口** | 232 | `wifi_interface: "wlan0"` - 应自动检测 |
| **嵌套 spawn 结构** | 230, 241 | 代码复杂度高，难以维护 |
| **没有状态检查** | 218 | 可能在传输中重复点击发送 |

---

### 2. **接收流程** (app.rs:294-400)

#### 当前实现：
```rust
let mut on_mode_change = move |new_mode: AppMode| {
    if new_mode == AppMode::Receiving {
        // ✅ 有状态检查（防止重复启动）
        // ✅ 有任务管理（active_receive_task）
        // ✅ 有详细日志
        // ⚠️ 使用 ReceiveOptions::default()
    }
}
```

#### 🟡 **可改进点**：

| 项目 | 当前 | 建议 |
|------|------|------|
| **ReceiveOptions** | 从 settings 逐字段构建 | ✅ 已正确实现 |
| **错误处理** | 简单日志 | 可添加重试机制 |
| **任务取消** | Task drop | ✅ 已正确实现 |

---

### 3. **扫描流程** (app.rs:163-201)

#### 当前实现：
```rust
let on_refresh_devices = move |_| {
    // 1. 清空设备列表
    // 2. spawn 扫描任务
    // 3. 嵌套 spawn 事件转发
    // 4. 缺少任务管理
}
```

#### 🔴 **问题识别**：

| 问题 | 位置 | 影响 |
|------|------|------|
| **缺少扫描任务管理** | 163-201 | 无法取消正在进行的扫描 |
| **嵌套 spawn** | 168, 180 | 代码结构复杂 |
| **没有状态检查** | 163 | 可能在扫描中重复点击 |

---

## 🎯 优化方案

### **阶段 1：添加任务管理** ⭐⭐⭐

#### 目标：
- 为发送和扫描添加任务管理（类似接收）
- 防止重复操作
- 支持任务取消

#### 实现：
```rust
// 添加 Signal
let mut active_send_task = use_signal(|| Option::<Task>::None);
let mut active_scan_task = use_signal(|| Option::<Task>::None);

// 发送前检查
if status.read().is_busy() {
    log("正在传输中，请等待完成");
    return;
}

// 保存任务引用
let handle = spawn(async move { /* ... */ });
active_send_task.set(Some(handle));
```

---

### **阶段 2：修复占位数据** ⭐⭐⭐

#### 2.1 sender_id 生成

**当前**：
```rust
sender_id: String::new(),  // ❌ 空字符串
```

**优化**：
```rust
// 方案 A：从 AppSettings 获取（需要添加字段）
sender_id: current_settings.sender_id.clone(),

// 方案 B：动态生成（推荐）
sender_id: format!("{:04x}", rand::random::<u16>()),

// 方案 C：使用设备名哈希
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};
let mut hasher = DefaultHasher::new();
current_settings.device_name.hash(&mut hasher);
sender_id: format!("{:04x}", (hasher.finish() & 0xFFFF) as u16),
```

#### 2.2 supports_5ghz 检测

**当前**：
```rust
supports_5ghz: false,  // ❌ 硬编码
```

**优化**：
```rust
// 从扫描结果获取（BLE 广播中包含此信息）
supports_5ghz: dev.supports_5ghz,

// 或从 settings 获取作为 fallback
supports_5ghz: dev.supports_5ghz.unwrap_or(current_settings.supports_5ghz),
```

**注意**：需要检查 `DiscoveredDeviceInfo` 是否有此字段！

#### 2.3 WiFi 接口自动检测

**当前**：
```rust
wifi_interface: "wlan0".to_string(),  // ❌ 硬编码
```

**优化**：
```rust
// 使用 cattysend-core 的检测功能
wifi_interface: cattysend_core::wifi::detect_wifi_interface()
    .unwrap_or_else(|| "wlan0".to_string()),
```

---

### **阶段 3：简化嵌套结构** ⭐⭐

#### 当前问题：
```rust
spawn(async move {          // 外层：主任务
    spawn(async move {      // 内层：事件转发
        while let Some(event) = rx.recv().await { ... }
    });
    // 主逻辑
});
```

#### 优化方案：
```rust
// 方案 A：使用单层 tokio::select
spawn(async move {
    let mut rx = ...;
    loop {
        tokio::select! {
            Some(event) = rx.recv() => { /* 处理事件 */ }
            result = sender.send(...) => { /* 处理结果 */ }
        }
    }
});

// 方案 B：保持双层但更清晰（推荐）
// - 内层专注事件转发（不变）
// - 外层添加清晰注释和错误处理
```

---

### **阶段 4：增强错误处理** ⭐

#### 当前：
```rust
if let Ok(sender) = Sender::new(options) {
    let _ = sender.send_to_device(...).await;  // ❌ 忽略错误
}
```

#### 优化：
```rust
match Sender::new(options) {
    Ok(sender) => {
        match sender.send_to_device(...).await {
            Ok(_) => {
                tx.send(GuiEvent::Log(Info, "发送完成"));
            }
            Err(e) => {
                tx.send(GuiEvent::Error(format!("发送失败: {}", e)));
            }
        }
    }
    Err(e) => {
        tx.send(GuiEvent::Error(format!("初始化发送器失败: {}", e)));
    }
}
```

---

## 📋 优先级清单

### 🔥 **高优先级（立即修复）**

- [ ] **P1**: 添加 `active_send_task` 任务管理
- [ ] **P1**: 修复 `sender_id` 占位值（生成随机 ID）
- [ ] **P1**: 修复 `supports_5ghz` 占位值（从扫描结果获取）
- [ ] **P1**: 添加发送状态检查（防止重复点击）

### ⚠️ **中优先级（重要改进）**

- [ ] **P2**: 添加 `active_scan_task` 扫描任务管理
- [ ] **P2**: WiFi 接口自动检测
- [ ] **P2**: 增强所有错误处理
- [ ] **P2**: 添加任务取消按钮

### 💡 **低优先级（可选优化）**

- [ ] **P3**: 简化嵌套 spawn 结构
- [ ] **P3**: 添加传输速度计算
- [ ] **P3**: 添加传输历史记录

---

## 🔍 需要检查的数据结构

### DiscoveredDeviceInfo (state.rs)
```rust
pub struct DiscoveredDeviceInfo {
    pub name: String,
    pub address: String,
    pub rssi: i16,
    pub brand: Option<String>,
    // ⚠️ 检查是否有 supports_5ghz 字段？
}
```

### AppSettings (cattysend-core)
```rust
// ⚠️ 检查是否有 sender_id 字段？
// ⚠️ 检查是否有自动检测 WiFi 接口的方法？
```

---

## 🚀 实施建议

### 第一批（本次实施）：
1. ✅ 添加 `active_send_task` Signal
2. ✅ 生成动态 sender_id
3. ✅ 修复 supports_5ghz（从设备信息获取）
4. ✅ 添加发送前状态检查

### 第二批（后续优化）：
1. WiFi 接口自动检测
2. 扫描任务管理
3. 增强错误处理

### 第三批（锦上添花）：
1. 代码结构重构
2. 添加更多 UI 反馈
3. 性能优化
