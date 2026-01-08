use crate::ble::DeviceStatus;
use crate::crypto::SessionCipher;
use crate::wifi::P2pInfo;

/// Handles GATT characteristic read/write operations
pub struct GattHandler;

impl GattHandler {
    /// Parse device status from CHAR_STATUS read
    pub fn parse_device_status(data: &[u8]) -> anyhow::Result<DeviceStatus> {
        let json_str = std::str::from_utf8(data)?;
        let status: DeviceStatus = serde_json::from_str(json_str)?;
        Ok(status)
    }

    /// Decrypt P2P info from CHAR_P2P read
    pub fn decrypt_p2p_info(
        encrypted_data: &str,
        cipher: &SessionCipher,
    ) -> anyhow::Result<P2pInfo> {
        let decrypted = cipher.decrypt(encrypted_data)?;
        let info: P2pInfo = serde_json::from_str(&decrypted)?;
        Ok(info)
    }

    /// Encrypt P2P info for CHAR_P2P write
    pub fn encrypt_p2p_info(info: &P2pInfo, cipher: &SessionCipher) -> anyhow::Result<String> {
        let json = serde_json::to_string(info)?;
        cipher.encrypt(&json)
    }
}
