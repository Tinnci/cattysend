# Cattysend 代码重复分析与 SSOT 改进建议

通过对比 GUI (`crates/cattysend-gui`) 和 TUI (`crates/cattysend-tui`) 的代码，发现以下违反 Single Source of Truth (SSOT) 原则的重复逻辑：

## 1. 日志数据结构 (High Priority)
**现状**:
- GUI: `crates/cattysend-gui/src/app.rs` 定义了 `LogLevel` (Error, Warn, Info, Debug) 和 `LogEntry`。
- TUI: `crates/cattysend-tui/src/app.rs` 定义了 `LogLevel` (Error...Trace) 和 `LogEntry`，还包含 `from_str` 等辅助方法。

**问题**:
- 如果需要增加日志级别或修改图标，需要同时修改两处。
- 业务逻辑与 UI 框架耦合。

**建议**:
- 将 `LogLevel` 和 `LogEntry` 移动到 `cattysend-core` (建议在 `utils` 或 `logging` 模块)。
- Core 提供通用的 `LogEntry` 结构，GUI/TUI 负责渲染。

## 2. 扫描回调样板代码 (Medium Priority)
**现状**:
- GUI 和 TUI 都手动实现了一个 struct 来包装 `mpsc::Sender` 并实现 `ScanCallback` trait。
- 代码逻辑完全相同：收到 `on_device_found` -> 发送 Event 到 channel。

**建议**:
- 在 `cattysend-core` 中提供一个泛型的 `ChannelScanCallback<F>`，其中 `F` 是一个闭包或转换函数，用于将 `DiscoveredDevice` 转换为目标 Event 类型。

## 3. 传输/接收状态管理 (High Priority + Architecture)
**现状**:
- GUI 在 `app.rs` 中定义了完善的 `ReceiveState` 枚举 (Idle, Advertising, Connecting, Receiving, Completed, Error)。
- TUI 在 `app.rs` 中散落在 `App`结构体的字段中 (`mode`, `progress`, `status_message` 等) 来隐式管理状态。
- 两者都有大量的 boilerplate 代码来启动任务、轮询 `Receiver`/`Sender` 的 rx channel，并将 core event (`SendEvent`/`ReceiveEvent`) 转换为 UI event。

**问题**:
- 状态机逻辑在 GUI 中是显式的，在 TUI 中是隐式的，容易导致 TUI 状态不一致（bug 源头）。
- 重复的 `spawn` + `while let Some(event) = rx.recv()` 循环。

**建议**:
- **Core 状态机**: 将 GUI 的 `ReceiveState` (以及未来的 `SendState`) 泛化并下沉到 `cattysend-core`。
- **Session Controller**: 引入 `TransferSession` 概念，封装 `Sender`/`Receiver` 的生命周期管理。UI 层只需持有 Session 句柄并消费简化的 `Sessionstate` 变更事件。

## 4. API 设计建议
1.  **Config**: 既然 BrandId 已经收敛，`AppSettings` 是否可以直接包含所有验证逻辑？目前看 `config.rs` 做得不错。
2.  **Controller Pattern**: 现在的 Core API (`Sender::new`, `Receiver::new`) 偏底层。建议增加一层 `Controller` 层，暴露更符合用户操作的 API，如 `start_discovery()`, `send_files()`, `cancel_transfer()`，内部处理多线程和回调分发。

---
## 下一步行动计划
1.  **Refactor Logging**: 将 `LogLevel` 和 `LogEntry` 提取到 core。
2.  (Optional) **Refactor Scan**: 提供 `ChannelScanCallback`。
