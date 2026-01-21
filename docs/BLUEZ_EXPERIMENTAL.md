# BlueZ 实验性功能配置

本项目使用了 BlueZ 的实验性 D-Bus 接口来实现 CatShare 兼容的 BLE 广播。

## 为什么需要实验性功能？

CatShare/互传联盟协议需要在 BLE Legacy Advertising 的两个包中分别发送不同的数据：

| 包类型 | 大小 | 内容 |
|--------|------|------|
| 主广播包 (Advertising Data) | 31 bytes | 身份信息 (Brand ID, 5GHz 支持, Sender ID) |
| 扫描响应包 (Scan Response) | 31 bytes | 设备名称 (27 bytes Service Data) |

标准的 bluer/BlueZ API 只支持 `service_data` 字段，会将所有数据放入主广播包。
当数据超过 31 字节时，BlueZ 会自动升级到 Extended Advertising，导致旧设备无法发现。

通过 BlueZ 的实验性 `ScanResponseServiceData` 接口，我们可以精确控制哪些数据放入扫描响应包，
从而保持 Legacy 模式的兼容性。

## 配置步骤

### 1. 启用 BlueZ 实验性功能

编辑 `/etc/bluetooth/main.conf`：

```ini
[General]
# 启用 D-Bus 实验性接口
Experimental = true
```

或使用命令：

```bash
sudo sed -i 's/^#Experimental = true/Experimental = true/' /etc/bluetooth/main.conf
```

### 2. 重启 Bluetooth 服务

```bash
sudo systemctl restart bluetooth
```

### 3. 验证配置

运行 `btmon` 并启动 cattysend，应该能看到：

```
LE Set Extended Advertising Data:
    Data length: 0x1f (31 bytes)
    ...主广播包数据...

LE Set Extended Scan Response Data:
    Data length: 0x1f (31 bytes)
    Service Data: Unknown (0xffff)
      Data[27]: ...设备名数据...
```

## 技术细节

### 使用的 BlueZ 实验性接口

- `ScanResponseServiceData` - Service Data 放入扫描响应包
- `ScanResponseServiceUUIDs` - Service UUID 放入扫描响应包  
- `ScanResponseManufacturerData` - Manufacturer Data 放入扫描响应包
- `ScanResponseData` - 原始数据放入扫描响应包

### bluer fork

由于上游 bluer 不支持这些实验性接口，我们使用了自己的 fork：

- **仓库**: https://github.com/Tinnci/bluer
- **分支**: `feature/scan-response-data`
- **上游 PR**: https://github.com/bluez/bluer/pull/186

### 数据格式

#### 主广播包 - Identity Service Data (6 bytes)

```
Service UUID: 0x01XX (XX = Brand ID, 高位 = 5GHz 标志)
Payload: [sender_id_hi, sender_id_lo, 0, 0, 0, 0]
```

#### 扫描响应包 - Name Service Data (27 bytes)

```
Service UUID: 0xFFFF (标准蓝牙基底)
Payload:
  Bytes 0-7:   协议头 (固定为 0)
  Bytes 8-9:   Sender ID (与主包中相同)
  Bytes 10-25: 设备名 (UTF-8, 最多 16 字符)
  Byte 26:     协议尾 (0, 或 '\t' 表示名称被截断)
```

## 故障排除

### 问题：设备名在接收端为空

**原因**：BlueZ 实验性功能未启用

**解决**：
1. 确认 `/etc/bluetooth/main.conf` 中 `Experimental = true`
2. 重启 bluetooth 服务
3. 使用 `btmon` 验证 Scan Response 包中是否包含 Service Data

### 问题：接收端看到两个设备

**原因**：有多个 cattysend 进程在运行

**解决**：
```bash
# 检查进程
pgrep -a cattysend

# 停止多余的进程
sudo systemctl stop cattysend-daemon
# 或
killall cattysend-daemon
```

### 问题：显示为 "Extended Advertising"

**原因**：数据量超过 31 字节或配置了 secondary_channel

**解决**：确保 `secondary_channel: None` 并检查数据大小

## 参考资料

- [BlueZ LEAdvertisement 文档](https://github.com/bluez/bluez/blob/master/doc/org.bluez.LEAdvertisement.rst)
- [BLE Advertising 数据格式](https://www.bluetooth.com/specifications/assigned-numbers/)
- [CatShare 项目](https://github.com/AceDroidX/CatShare)
