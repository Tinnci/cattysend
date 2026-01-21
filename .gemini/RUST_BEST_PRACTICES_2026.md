# Rust 2026 最佳实践指南

本文档记录了 `cattysend` 项目采用的 Rust 2026 代码质量标准和最佳实践。

## 1. 使用 `reason` 参数文档化抑制理由

从 Rust 1.81 开始，`#[allow]`、`#[expect]`、`#[warn]` 等 lint 属性均支持 `reason` 参数。

### 做法

```rust
#[expect(dead_code, reason = "此接口预留给未来插件系统调用")]
fn reserved_api() {
    // ...
}
```

### 价值

它强制开发者在禁用检查时解释"为什么"，这对团队协作和长期维护至关重要，避免后人不敢删除过期的 lint 抑制。

## 2. 结合 Clippy 强制执行最佳实践

Clippy 提供了一些专门用于约束 lint 使用习惯的规则。本项目在 `clippy.toml` 中启用了这些规则：

### 配置的规则

- **`clippy::allow_attributes`**: 强制将所有 `#[allow]` 升级为 `#[expect]`，确保没有失效的抑制规则。
- **`clippy::allow_attributes_without_reason`**: 强制所有抑制属性必须附带 `reason` 解释。

### 示例

❌ **不推荐**:
```rust
#[allow(dead_code)]
fn unused_function() {}
```

✅ **推荐**:
```rust
#[expect(dead_code, reason = "将在 v2.0 中用于新的传输协议")]
fn future_transport_api() {}
```

## 3. 严格的错误处理实践

在 2026 年的生产环境下，代码的"健壮性"已成为核心考量。

### 禁用隐式恐慌

在 CI 或项目配置中禁止使用 `.unwrap()`。推荐使用 `.expect("详细的错误上下文")` 明确说明预期，或者通过 `?` 传播错误。

❌ **不推荐**:
```rust
let config = load_config().unwrap();
```

✅ **推荐**:
```rust
let config = load_config()
    .expect("配置文件必须存在且格式正确，请检查 config.toml");
```

或者更好的做法：
```rust
let config = load_config()
    .context("加载配置文件失败")?;
```

### 使用专用错误库

- **库代码**: 推荐使用 `thiserror` 定义精确错误类型
- **应用层代码**: 使用 `anyhow` 简化处理

#### 库代码示例（使用 thiserror）

```rust
use thiserror::Error;

#[derive(Error, Debug)]
pub enum WifiError {
    #[error("无法连接到 NetworkManager D-Bus 服务")]
    DBusConnectionFailed(#[from] zbus::Error),
    
    #[error("Wi-Fi 设备 {0} 不支持 P2P 模式")]
    P2PNotSupported(String),
    
    #[error("激活连接超时（{timeout}秒）")]
    ActivationTimeout { timeout: u64 },
}
```

#### 应用层代码示例（使用 anyhow）

```rust
use anyhow::{Context, Result};

fn main() -> Result<()> {
    let wifi = WifiManager::new()
        .context("初始化 Wi-Fi 管理器失败")?;
    
    wifi.start_p2p()
        .context("启动 Wi-Fi Direct 失败")?;
    
    Ok(())
}
```

## 4. 强制执行 `must_use`

### 做法

为函数或结构体添加 `#[must_use]` 属性。

```rust
#[must_use = "这个 Future 必须被 await，否则不会执行任何操作"]
pub async fn send_file(path: &Path) -> Result<()> {
    // ...
}

#[must_use = "Builder 必须调用 .build() 才能生效"]
pub struct FileTransferBuilder {
    // ...
}
```

### 价值

如果调用者忽略了返回值（例如一个可能失败的操作或一个需要手动释放的资源），编译器会发出警告。这在 2026 年被视为编写"防错接口"的标准做法。

## 5. 持续集成中的 `deny(warnings)`

### 做法

在 CI 脚本中使用以下命令：

```bash
cargo clippy -- -D warnings
```

或在项目的 `lib.rs`/`main.rs` 中全局启用：

```rust
#![deny(warnings)]
#![warn(clippy::all, clippy::pedantic)]
```

### 价值

确保任何新出现的 lint 警告（包括 `#[expect]` 因失效转成的警告）都会导致构建失败，从而维持代码库的长期清洁。

## 总结

通过这些实践，我们将 Rust 的静态检查从"报错工具"转变为"设计辅助工具"，不仅能通过编译器发现错误，还能通过代码属性传达设计意图。

### 快速检查清单

- [ ] 所有 lint 抑制都使用 `#[expect]` 而非 `#[allow]`
- [ ] 所有 lint 抑制都包含 `reason` 参数
- [ ] 错误处理使用 `thiserror`（库）或 `anyhow`（应用）
- [ ] 避免使用 `.unwrap()`，使用 `.expect()` 或 `?`
- [ ] 关键 API 标记了 `#[must_use]`
- [ ] CI 管道包含 `cargo clippy -- -D warnings`
