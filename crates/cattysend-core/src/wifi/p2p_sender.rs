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
    pub async fn create_group(&self, port: i32) -> anyhow::Result<P2pInfo> {
        let ssid = format!("DIRECT-{:04x}", rand::random::<u16>());
        let psk = format!("{:032x}", rand::random::<u128>());

        // Get local MAC address
        let mac = std::fs::read_to_string(format!("/sys/class/net/{}/address", self.interface))
            .unwrap_or_else(|_| "02:00:00:00:00:00\n".to_string())
            .trim()
            .to_uppercase();

        tracing::warn!("WiFi P2P 热点创建需要 wpa_supplicant 权限");
        tracing::info!("模拟 P2P 信息: SSID={}", ssid);

        Ok(P2pInfo::new(ssid, psk, mac, port))
    }
}
