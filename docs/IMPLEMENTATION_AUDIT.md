# 实现差异对比报告

通过对比 `/home/drie/cattysend` (Rust Linux) 和 `/tmp/CatShare` (Android Kotlin)，以下是当前的兼容性状态：

## 修复状态摘要

| 问题 | 状态 | 说明 |
|------|------|------|
| 公钥格式 (SEC1 vs SPKI) | ✅ 已修复 | 使用 X.509 SPKI DER 格式 |
| AES IV 格式 | ✅ 已修复 | 使用 ASCII `"0102030405060708"` |
| HKDF 移除 | ✅ 已修复 | 直接使用 ECDH 原始共享密钥 |
| DeviceInfo 字段 | ✅ 已修复 | 使用 camelCase 命名 |
| P2pInfo 字段 | ✅ 已修复 | 使用 camelCase 命名 |
| BLE 广播数据 | ✅ 已实现 | 包含 0x01FF 和 0xFFFF |
| 日志系统 | ✅ 已迁移 | 使用 `log` crate 作为门面 |

---

## 1. BLE 广播数据 (Advertisement Data)

| 特性 | CatShare (Android) | Cattysend (Rust) | 状态 |
|------|-------------------|------------------|------|
| **Service UUID** | `00003331-0000-1000-8000-008123456789` | 一致 | ✅ |
| **Service Data (0x01FF)** | 包含 6 字节数据 | 已实现 | ✅ |
| **Scan Response (0xFFFF)** | 包含 27 字节身份数据 | 已实现 | ✅ |
| **Legacy Mode** | `true` | `true` (默认) | ✅ |

## 2. JSON 协议字段 (GATT Status)

| 字段用途 | CatShare 字段名 | Cattysend 字段名 | 状态 |
|---------|----------------|------------------|------|
| 状态 | `state` | `state` | ✅ |
| 公钥 | `key` | `key` | ✅ |
| MAC 地址 | `mac` | `mac` | ✅ |
| 协议版本 | `catShare` (Int) | `catShare` (i32) | ✅ |

## 3. 加密实现 (Crypto)

| 特性 | CatShare | Cattysend | 状态 |
|------|----------|-----------|------|
| **算法** | ECDH (P-256) | ECDH (P-256) | ✅ |
| **公钥格式** | X.509 SPKI DER | X.509 SPKI DER | ✅ |
| **密钥派生** | 直接使用共享密钥 | 直接使用共享密钥 | ✅ |
| **AES 模式** | AES/CTR/NoPadding | AES-256-CTR | ✅ |
| **IV** | ASCII `"0102030405060708"` | ASCII `"0102030405060708"` | ✅ |

## 4. 测试覆盖

当前测试数量: **23 个** (全部通过)

### 测试分布

| 模块 | 测试数量 | 覆盖内容 |
|------|---------|---------|
| `crypto::ble_security` | 8 | ECDH 密钥交换、AES 加解密、公钥格式 |
| `ble` | 5 | UUID 常量、DeviceInfo 序列化/反序列化 |
| `wifi` | 6 | P2pInfo 序列化/反序列化、凭证生成 |
| `transfer::protocol` | 4 | WebSocket 消息解析 |

## 5. 日志系统

已迁移到行业标准架构：

- **库 (cattysend-core)**: 使用 `log` crate 作为门面
- **应用 (TUI/CLI/Daemon)**: 使用 `tracing` + `tracing-subscriber`

这符合 Rust 生态最佳实践：库使用 `log` 保持兼容性，应用选择具体实现。

---

**最后更新**: 2026-01-09
