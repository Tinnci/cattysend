pub mod p2p_receiver;
pub mod p2p_sender;

#[derive(serde::Serialize, serde::Deserialize, Debug, Clone)]
pub struct P2pInfo {
    pub ssid: String,
    pub psk: String,
    pub mac_address: String,
    pub port: u16,
    pub go_intent: u8,
    pub band_preference: u8,
}
