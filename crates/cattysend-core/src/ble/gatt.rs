use crate::ble::DeviceInfo;
use crate::crypto::SessionCipher;
use crate::wifi::P2pInfo;

/// Handles GATT characteristic read/write operations
pub struct GattHandler;

impl GattHandler {
    /// Parse device info from CHAR_STATUS read
    pub fn parse_device_info(data: &[u8]) -> anyhow::Result<DeviceInfo> {
        let json_str = std::str::from_utf8(data)?;
        let info: DeviceInfo = serde_json::from_str(json_str)?;
        Ok(info)
    }

    /// Decrypt P2P info received via CHAR_P2P
    ///
    /// CatShare 发送的 P2pInfo 格式：
    /// - 如果 `key` 字段存在，则 ssid/psk/mac 是加密的
    /// - 使用发送端的公钥派生会话密钥后解密
    pub fn decrypt_p2p_info(
        encrypted_info: &P2pInfo,
        cipher: &SessionCipher,
    ) -> anyhow::Result<P2pInfo> {
        Ok(P2pInfo {
            id: encrypted_info.id.clone(),
            ssid: cipher.decrypt(&encrypted_info.ssid)?,
            psk: cipher.decrypt(&encrypted_info.psk)?,
            mac: cipher.decrypt(&encrypted_info.mac)?,
            port: encrypted_info.port,
            key: None,
            cat_share: encrypted_info.cat_share,
        })
    }

    /// Encrypt P2P info for CHAR_P2P write
    pub fn encrypt_p2p_info(
        info: &P2pInfo,
        cipher: &SessionCipher,
        sender_id: &str,
        sender_public_key: &str,
    ) -> anyhow::Result<P2pInfo> {
        Ok(P2pInfo::with_encryption(
            sender_id.to_string(),
            cipher.encrypt(&info.ssid)?,
            cipher.encrypt(&info.psk)?,
            cipher.encrypt(&info.mac)?,
            info.port,
            sender_public_key.to_string(),
        ))
    }
}
