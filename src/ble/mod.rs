pub mod advertiser;
pub mod gatt;
pub mod scanner;

use uuid::Uuid;

pub const SERVICE_UUID: Uuid = Uuid::from_u128(0x00003331_0000_1000_8000_008123456789);
pub const MAIN_SERVICE_UUID: Uuid = Uuid::from_u128(0x00009955_0000_1000_8000_00805f9b34fb);
pub const STATUS_CHAR_UUID: Uuid = Uuid::from_u128(0x00009954_0000_1000_8000_00805f9b34fb);
pub const P2P_CHAR_UUID: Uuid = Uuid::from_u128(0x00009953_0000_1000_8000_00805f9b34fb);

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
#[serde(rename_all = "camelCase")]
pub struct DeviceStatus {
    pub device_name: String,
    pub os_version: String,
    pub model: String,
    pub public_key: String,
    pub sender_version: String,
}
