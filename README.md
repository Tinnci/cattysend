[ [English](#english) | [中文](#chinese) ]

<a name="chinese"></a>

# cattysend

`cattysend` 是一个基于 Rust 开发的高性能 **互传联盟 (Mutual Transfer Alliance, MTA)** 协议实现，专为 Linux 终端环境设计。它利用低功耗蓝牙 (BLE) 和 Wi-Fi Direct (P2P) 技术，实现了 Linux 桌面与移动设备（小米、OPPO、vivo 等）之间的无缝、高速文件发现与传输。

## 渊源与致敬

本项目深受 [CatShare](https://github.com/kmod-midori/CatShare) 的启发，后者是 MTA 协议的先驱性实现。`cattysend` 旨在延续这一技术谱系，通过提供原生的 Linux TUI 体验，针对无头服务器和开发者工作流进行了深度优化。

底层协议是对互传联盟所用标准的逆向工程实现。虽然这不是官方实现，但它严格遵循了跨设备互操作性所需的加密和传输规范。

## 实施状态

项目目前处于活跃的 **开发中 (WIP)** 状态。核心引擎已可运行，完全稳定的二进制版本仍在准备中。

### 功能矩阵

| 模块 | 功能 | 状态 | 备注 |
| :--- | :--- | :--- | :--- |
| **发现** | BLE GATT 广播与服务发现 | ✅ 已完成 | 需 BlueZ 支持 |
| **安全** | ECDH (P-256) 密钥交换 | ✅ 已完成 | 原生实现 |
| **传输** | Wi-Fi Direct (P2P) | ✅ 已完成 | 通过 NetworkManager 管理 |
| **界面** | CLI 前端 | 🚧 Alpha | 基础命令可用 |
| **界面** | TUI 前端 | 🚧 Alpha | 交互逻辑完善中 |

### 重要提示：设备发现与名称显示
为了让您的 Linux 设备在 Android 手机上能够正确显示设备名称（而非空值），**必须**启用 BlueZ 的实验性功能。这是因为互传联盟协议需要精细控制扫描响应包 (Scan Response) 的内容。

请参考 [BlueZ 实验性功能配置指南](docs/BLUEZ_EXPERIMENTAL.md) 进行设置。


## 技术架构与限制说明

### "无 Sudo" 哲学
`cattysend` 的首要设计目标是维护系统完整性。与许多需要 `CAP_NET_ADMIN` 或 `sudo` 权限来操作原始套接字的 Linux 网络工具不同，`cattysend` 将所有网络操作通过 D-Bus 接口委托给 **NetworkManager (NM)** 守护进程处理。

### 连接性权衡 (The Connectivity Trade-off)
当前的 Linux 桌面基础设施对并发 Wi-Fi 操作构成了显著挑战。虽然现代无线硬件通常支持多种并发接口（例如：托管模式 + P2P客户端），但 NetworkManager 的策略引擎往往缺乏从内核解析 `NL80211_ATTR_INTERFACE_COMBINATIONS` 的逻辑。

**当前限制：**
当激活 P2P 连接时，`cattysend` 使用原生的 `nmcli` 后端。由于上游 NM 的实现细节，物理 Wi-Fi 接口可能会暂时挂起其基础设施连接，以优先保障 P2P 组的建立。我们选择了这种“抢占式”行为作为一种更安全、更稳健的替代方案，而非注入未托管的 `wpa_supplicant` 实例或要求不安全的 `sudoers` 配置。

## 源码构建

要构建 `cattysend`，你需要功能完备的 Rust 工具链以及 D-Bus 和 BlueZ 的开发头文件。

### 依赖项
- `libdbus-1-dev` (或同等库)
- `libbluetooth-dev` (BlueZ)
- `NetworkManager` (运行时)

### 构建命令
```bash
cargo build --release
```

生成的二进制文件位于 `target/release/`：
- `cattysend-cli`: 命令行工具
- `cattysend-daemon`: 后台服务
- `cattysend-gui`: 桌面图形界面
- `cattysend-tui`: 终端用户界面（推荐）

## 开发者文档

如果您计划为 `cattysend` 贡献代码，请阅读以下文档：
- [架构说明](docs/ARCHITECTURE.md) - 了解项目模块边界和设计取舍
- [实现审计](docs/IMPLEMENTATION_AUDIT.md) - 了解当前实现状态和待改进项
- [贡献指南](CONTRIBUTING.md)

## 致谢

深切感谢 **CatShare** 的开发者们对 MTA 协议的初步研究。本项目愿作为 Linux 终端社区的一个补充实现，与各位共勉。

## 许可证

本项目基于 MIT 许可证开源。详情请参阅 [LICENSE](LICENSE) 文件。

---

<a name="english"></a>

# cattysend

`cattysend` is a high-performance, Rust-based implementation of the **Mutual Transfer Alliance (MTA)** protocol, specifically designed for Linux terminal environments. It enables seamless, high-speed file discovery and transfer between Linux desktops and mobile devices (Xiaomi, OPPO, vivo, etc.) using Bluetooth Low Energy (BLE) and Wi-Fi Direct (P2P).

## Origins and Lineage

This project is heavily inspired by [CatShare](https://github.com/kmod-midori/CatShare), a pioneering implementation of the MTA protocol. `cattysend` aims to extend this lineage by providing a native Linux TUI experience, optimized for headless servers and developer workflows.

The underlying protocol is a reverse-engineered implementation of the standards used by the Mutual Transfer Alliance. It is not an official implementation, but it adheres to the cryptographic and transport specifications required for cross-device interoperability.

## Implementation Status

The project is currently in an active **Work in Progress (WIP)** state. While the core engine is operational, a fully stable binary release is pending.

### Feature Matrix

| Module | Feature | Status | Notes |
| :--- | :--- | :--- | :--- |
| **Discovery** | BLE GATT Advertisement | ✅ Done | Requires BlueZ |
| **Security** | ECDH (P-256) Key Exchange | ✅ Done | Native implementation |
| **Transport** | Wi-Fi Direct (P2P) | ✅ Done | Managed via NetworkManager |
| **Interface** | CLI Frontend | 🚧 Alpha | Basic commands working |
| **Interface** | TUI Frontend | 🚧 Alpha | Interactive selection pending |

### Important Tip: Device Discovery & Name Display
To ensure your Linux device displays its name correctly on mobile devices (instead of appearing empty), you **must** enable BlueZ experimental features. This is required for precise control over Scan Response packets as per the MTA protocol.

Please refer to the [BlueZ Experimental Features Guide](docs/BLUEZ_EXPERIMENTAL.md) for setup instructions.


## Technical Architecture & Constraints

### The "Sudo-less" Philosophy
A primary design goal of `cattysend` is to maintain system integrity. Unlike many Linux networking tools that require `CAP_NET_ADMIN` or `sudo` for raw socket manipulation, `cattysend` delegates all network operations to the **NetworkManager (NM)** daemon via its D-Bus interface.

### The Connectivity Trade-off
Current Linux desktop infrastructure presents a significant challenge for concurrent Wi-Fi operations. While modern wireless hardware typically supports multiple concurrent interfaces (e.g., Managed + P2P-Client), the NetworkManager policy engine often lacks the logic to parse `NL80211_ATTR_INTERFACE_COMBINATIONS` from the kernel.

**Current limitation:** 
When activating a P2P connection, `cattysend` uses the native `nmcli` backend. Due to upstream NM implementation details, the physical Wi-Fi interface may temporarily suspend its infrastructure connection to prioritize the P2P group. We have chosen this "preemptive" behavior as a safer, more robust alternative to injecting unmanaged `wpa_supplicant` instances or requiring insecure `sudoers` configurations.

## Building from Source

To build `cattysend`, you need a functional Rust toolchain and the development headers for D-Bus and BlueZ.

### Dependencies
- `libdbus-1-dev` (or equivalent)
- `libbluetooth-dev` (BlueZ)
- `NetworkManager` (Runtime)

### Build Command
```bash
cargo build --release
```

The resulting binaries will be located in `target/release/`:
- `cattysend-cli`: Command line utility
- `cattysend-daemon`: Background service
- `cattysend-gui`: Desktop GUI
- `cattysend-tui`: The terminal user interface (recommended)

## Developer Documentation

If you plan to contribute code to `cattysend`, please review the following documentation:
- [Architecture](docs/ARCHITECTURE.md) - Understand module boundaries and design trade-offs
- [Implementation Audit](docs/IMPLEMENTATION_AUDIT.md) - Review current implementation status and known improvement areas
- [Contributing Guide](CONTRIBUTING.md)

## Acknowledgments

Deep gratitude to the developers of **CatShare** for their initial research into the MTA protocol. This project serves as a complementary implementation for the Linux terminal community.

## License

This project is licensed under the MIT License. See [LICENSE](LICENSE) for details.
