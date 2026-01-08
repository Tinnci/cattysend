use crate::wifi::P2pInfo;

pub struct WiFiP2pSender {
    interface: String,
}

impl WiFiP2pSender {
    pub fn new(interface: &str) -> Self {
        Self {
            interface: interface.to_string(),
        }
    }

    /// Create a WiFi P2P group (hotspot)
    /// This is a simplified implementation - real P2P requires wpa_supplicant access
    pub async fn create_group(&self, port: u16) -> anyhow::Result<P2pInfo> {
        // For a real implementation, we would use wpa_supplicant's D-Bus interface
        // or the wpa_ctrl crate with proper socket access.
        // Here we provide a fallback that creates mock P2P info.

        let ssid = format!("DIRECT-{:04x}", rand::random::<u16>());
        let psk = format!("{:032x}", rand::random::<u128>());

        // Get local MAC address
        let mac_address =
            std::fs::read_to_string(format!("/sys/class/net/{}/address", self.interface))
                .unwrap_or_else(|_| "00:00:00:00:00:00\n".to_string())
                .trim()
                .to_uppercase();

        println!("⚠️  注意: WiFi P2P 热点创建需要 wpa_supplicant 权限");
        println!("   模拟 P2P 信息: SSID={}", ssid);

        Ok(P2pInfo {
            ssid,
            psk,
            mac_address,
            port,
            go_intent: 15,
            band_preference: 1,
        })
    }
}
