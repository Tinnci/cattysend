# Cattysend 开发路线图

> 互传联盟协议 Linux 实现

## 项目状态总览

| 阶段 | 状态 | 完成度 |
|------|------|--------|
| Phase 1: 核心协议 | ✅ 完成 | 100% |
| Phase 2: CLI/Daemon 架构 | ✅ 完成 | 100% |
| Phase 3: TUI 交互界面 | ✅ 完成 | 100% |
| Phase 4: 真实 BLE 集成 | 🔄 进行中 | 30% |
| Phase 5: WiFi P2P 实现 | ⏳ 待开始 | 0% |
| Phase 6: GUI 托盘应用 | ⏳ 待开始 | 0% |

---

## Phase 1: 核心协议 ✅

**目标**: 实现与 CatShare (Android) 完全兼容的协议层

### 已完成
- [x] BLE UUID 常量定义
- [x] DeviceInfo 结构体 (state/key/mac/catShare)
- [x] P2pInfo 结构体 (id/ssid/psk/mac/port/key/catShare)
- [x] ECDH (P-256) 密钥交换
- [x] AES-256-CTR 加密/解密
- [x] 正确的 IV: ASCII `"0102030405060708"`
- [x] 直接使用 ECDH 共享密钥 (无 HKDF)

### 测试覆盖
- [x] 加密/解密往返测试
- [x] IV 格式验证测试

---

## Phase 2: CLI/Daemon 架构 ✅

**目标**: 实现守护进程 + CLI 客户端模式

### 已完成
- [x] Cargo Workspace 结构
- [x] `cattysend-core` 核心库
- [x] `cattysend-daemon` systemd 服务
- [x] `cattysend-cli` clap 命令行
- [x] Unix Domain Socket IPC
- [x] `xtask` 自动化工具

### xtask 命令
| 命令 | 功能 |
|------|------|
| `cargo xtask build` | 构建 release 版本 |
| `cargo xtask install` | 安装 systemd 服务 |
| `cargo xtask uninstall` | 卸载服务 |
| `cargo xtask setup-caps` | 设置免 sudo 权限 |
| `cargo xtask dist` | 打包发布 |

---

## Phase 3: TUI 交互界面 ✅

**目标**: 使用 ratatui 提供交互式终端体验

### 已完成
- [x] 设备列表面板
- [x] RSSI 信号强度条显示
- [x] 传输进度条
- [x] 日志面板
- [x] Tab 页切换
- [x] 键盘快捷键

### 快捷键
| 按键 | 功能 |
|------|------|
| `s` | 扫描设备 |
| `r` | 接收模式 |
| `↑/↓` | 选择设备 |
| `Enter` | 连接 |
| `Tab` | 切换标签 |
| `q` | 退出 |

---

## Phase 4: 真实 BLE 集成 🔄

**目标**: 连接真实的 BLE 硬件

### 待完成
- [ ] BLE 扫描集成 (btleplug)
- [ ] GATT 服务发现
- [ ] CHAR_STATUS 读取
- [ ] CHAR_P2P 写入
- [ ] BLE 广播 (bluer GATT Server)
- [ ] 设备过滤 (Service UUID `00003331-...`)

### 技术挑战
- BlueZ 权限配置
- D-Bus 会话管理
- 跨发行版兼容性

---

## Phase 5: WiFi P2P 实现 ⏳

**目标**: 使用 wpa_supplicant 创建/加入 P2P 组

### 待完成
- [ ] wpa_supplicant D-Bus 接口
- [ ] P2P Group Owner 创建
- [ ] DHCP 服务器配置
- [ ] P2P Client 连接
- [ ] IP 地址获取
- [ ] 连接状态监控

### 依赖
- `wpa_supplicant` >= 2.10
- NetworkManager 或手动配置
- CAP_NET_ADMIN 权限

---

## Phase 6: GUI 托盘应用 ⏳

**目标**: 提供类似 AirDrop 的桌面体验

### 方案选择
| 框架 | 优势 | 适用场景 |
|------|------|----------|
| **Slint** | 轻量、原生渲染 | 托盘常驻 |
| **Tauri** | WebView、生态丰富 | 复杂 UI |
| **Iced** | 纯 Rust、Elm 架构 | 性能极致 |

### 待完成
- [ ] 系统托盘图标
- [ ] 桌面通知
- [ ] 拖拽发送
- [ ] 文件接收确认弹窗
- [ ] 设置界面
- [ ] 暗色/亮色主题

---

## 里程碑时间线

```
2026 Q1
├── 1月: Phase 1-3 (核心 + CLI + TUI) ✅
├── 2月: Phase 4 (BLE 集成)
└── 3月: Phase 5 (WiFi P2P)

2026 Q2
├── 4月: Phase 6 (GUI)
├── 5月: 测试 & 文档
└── 6月: v1.0 发布
```

---

## 技术债务 & 改进

### 当前问题
1. **TUI 设备模拟**: 当前使用模拟数据，需连接真实 BLE
2. **P2P 热点**: 当前为模拟实现，需 wpa_supplicant 集成
3. **错误处理**: 部分错误直接 panic，需改为优雅降级

### 代码质量
- [ ] 添加更多单元测试
- [ ] 集成测试 (需要硬件)
- [ ] API 文档 (rustdoc)
- [ ] 示例代码

---

## 贡献指南

### 开发环境
```bash
# 克隆
git clone https://github.com/user/cattysend

# 构建
cargo xtask build

# 开发模式
cargo xtask dev

# 运行 TUI
cargo run -p cattysend-tui
```

### 提交规范
- `feat:` 新功能
- `fix:` 修复
- `refactor:` 重构
- `docs:` 文档
- `test:` 测试

---

**最后更新**: 2026-01-09
