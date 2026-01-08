pub mod advertiser;
pub mod gatt;
pub mod scanner;

use uuid::Uuid;

pub const SERVICE_UUID: Uuid = Uuid::from_u128(0x00003331_0000_1000_8000_008123456789);
pub const MAIN_SERVICE_UUID: Uuid = Uuid::from_u128(0x00009955_0000_1000_8000_00805f9b34fb);
pub const STATUS_CHAR_UUID: Uuid = Uuid::from_u128(0x00009954_0000_1000_8000_00805f9b34fb);
pub const P2P_CHAR_UUID: Uuid = Uuid::from_u128(0x00009953_0000_1000_8000_00805f9b34fb);

/// DeviceInfo - 必须与 CatShare 的 DeviceInfo 字段完全一致
/// CatShare: data class DeviceInfo(val state: Int, val key: String?, val mac: String, val catShare: Int? = null)
#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DeviceInfo {
    pub state: i32,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub key: Option<String>,
    pub mac: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub cat_share: Option<i32>,
}

impl DeviceInfo {
    pub fn new(public_key: String, mac: String) -> Self {
        Self {
            state: 0,
            key: Some(public_key),
            mac,
            cat_share: Some(1), // Protocol version
        }
    }
}
