use crate::wifi::P2pInfo;
use std::process::Command;

pub struct WiFiP2pReceiver {
    interface: String,
}

impl WiFiP2pReceiver {
    pub fn new(interface: &str) -> Self {
        Self {
            interface: interface.to_string(),
        }
    }

    pub async fn connect(&self, info: &P2pInfo) -> anyhow::Result<String> {
        let output = Command::new("nmcli")
            .args(&[
                "device",
                "wifi",
                "connect",
                &info.ssid,
                "password",
                &info.psk,
                "ifname",
                &self.interface,
            ])
            .output()?;

        if !output.status.success() {
            let err = String::from_utf8_lossy(&output.stderr);
            return Err(anyhow::anyhow!("nmcli failed: {}", err));
        }

        tokio::time::sleep(std::time::Duration::from_secs(2)).await;

        let ip = self.get_interface_ip()?;
        Ok(ip)
    }

    fn get_interface_ip(&self) -> anyhow::Result<String> {
        let output = Command::new("ip")
            .args(&["-o", "addr", "show", &self.interface])
            .output()?;

        let stdout = String::from_utf8(output.stdout)?;
        for line in stdout.lines() {
            if line.contains("inet ") {
                let parts: Vec<&str> = line.split_whitespace().collect();
                if let Some(pos) = parts.iter().position(|&s| s == "inet") {
                    if let Some(ip_range) = parts.get(pos + 1) {
                        if let Some(ip) = ip_range.split('/').next() {
                            return Ok(ip.to_string());
                        }
                    }
                }
            }
        }
        Err(anyhow::anyhow!("Could not find IP address"))
    }
}
