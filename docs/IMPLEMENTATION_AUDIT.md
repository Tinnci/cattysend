# 实现差异对比报告

通过对比 `/home/drie/cattysend` (Rust Linux) 和 `/tmp/CatShare` (Android Kotlin)，发现以下关键差异：

## 1. BLE 广播数据 (Advertisement Data)

| 特性 | CatShare (Android) | Cattysend (Rust) | 状态 |
|------|-------------------|------------------|------|
| **Service UUID** | `00003331-0000-1000-8000-008123456789` | 一致 | ✅ |
| **Service Data (0x01FF)** | 包含 8 字节数据 (其中 6 字节随机) | **缺失** | ❌ 需修复 |
| **Scan Response (0xFFFF)** | 包含自定义厂商数据格式 (BrandID + Name) | **缺失** | ❌ 需修复 |
| **Legacy Mode** | `true` | `true` (默认) | ✅ |

**建议**: 无此数据可能导致 Android 端无法在扫描列表中识别出 Linux 设备。

## 2. JSON 协议字段 (GATT Status)

Android 端 `DeviceInfo` 类定义的字段与 Rust `DeviceStatus` 不一致：

| 字段用途 | CatShare 字段名 (`json`) | Cattysend 字段名 (`serde`) | 状态 |
|---------|-------------------------|--------------------------|------|
| 公钥 | `key` | `publicKey` | ❌ 不兼容 |
| 协议版本 | `catShare` (Int) | `senderVersion` (String) | ❌ 不兼容 |
| MAC 地址 | `mac` | (无，作为 P2PInfo 的一部分?) | ❌ 缺失 |
| 设备状态 | `state` | (无) | ❌ 缺失 |
| 设备名称 | (无，存在于 scan response) | `deviceName` | ⚠️ 差异 |

**建议**: 修改 Rust 端的 struct 字段名以匹配 CatShare。

## 3. 加密实现 (Crypto)

| 特性 | CatShare | Cattysend | 状态 |
|------|----------|-----------|------|
| **算法** | ECDH (P-256) | ECDH (P-256) | ✅ |
| **密钥派生 (KDF)** | 直接使用 ECDH 共享密钥 (Raw Bytes) | 使用 HKDF-SHA256(Shared Secret) | ❌ **严重不兼容** |
| **AES 模式** | AES/CTR/NoPadding | AES-256-CTR | ✅ |
| **IV (初始化向量)** | 字符串 `"0102030405060708"` 的字节 (ASCII) | 8字节 nonce + 8字节 counter | ❌ **不兼容** |

**详细分析**:
- CatShare IV: `30 31 30 32 30 33 30 34 30 35 30 36 30 37 30 38` (Hex of string)
- Cattysend IV: 需要调整为匹配上述固定字节。
- KDF: 必须移除 HKDF，直接使用 ECDH Shared Secret 的前 32 字节（或全部，取决于 Java `generateSecret` 行为，通常 P-256 产生 32 字节 secret）。

## 4. 结论

目前的 Rust 实现遵循了一份理论协议规范，而 CatShare 的实际代码实现与之有较大出入。为实现互通，必须修改 Rust 代码以对其 CatShare 的实际行为。

**修复计划**:
1. 修改 `DeviceStatus` 结构体字段。
2. 移除 Crypto 模块中的 HKDF，直接使用 Shared Secret。
3. 修正 AES IV 为固定的 ASCII 字节数组。
4. 补充 BLE 广播的 Service Data。
