pub mod p2p_receiver;
pub mod p2p_sender;

/// P2pInfo - 必须与 CatShare 的 P2pInfo 字段完全一致
/// CatShare: data class P2pInfo(
///     val id: String?,
///     val ssid: String,
///     val psk: String,
///     val mac: String,
///     val port: Int,
///     val key: String? = null,
///     val catShare: Int? = null,
/// )
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct P2pInfo {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub id: Option<String>,
    pub ssid: String,
    pub psk: String,
    pub mac: String,
    pub port: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cat_share: Option<i32>,
}

impl P2pInfo {
    pub fn new(ssid: String, psk: String, mac: String, port: i32) -> Self {
        Self {
            id: None,
            ssid,
            psk,
            mac,
            port,
            key: None,
            cat_share: Some(1),
        }
    }

    /// Create encrypted P2pInfo with sender's public key
    pub fn with_encryption(
        id: String,
        ssid_encrypted: String,
        psk_encrypted: String,
        mac_encrypted: String,
        port: i32,
        sender_public_key: String,
    ) -> Self {
        Self {
            id: Some(id),
            ssid: ssid_encrypted,
            psk: psk_encrypted,
            mac: mac_encrypted,
            port,
            key: Some(sender_public_key),
            cat_share: Some(1),
        }
    }
}
