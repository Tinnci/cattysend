use crate::ble::SERVICE_UUID;
use btleplug::api::{Central, Manager as _, Peripheral, ScanFilter};
use btleplug::platform::Manager;
use std::time::Duration;
use tokio::time;

#[derive(Debug, Clone)]
pub struct DiscoveredDevice {
    pub name: String,
    pub address: String,
    pub brand_id: Option<u16>,
    pub rssi: Option<i16>,
}

pub struct BleScanner {
    manager: Manager,
}

impl BleScanner {
    pub async fn new() -> anyhow::Result<Self> {
        let manager = Manager::new().await?;
        Ok(Self { manager })
    }

    pub async fn scan(&self, timeout: Duration) -> anyhow::Result<Vec<DiscoveredDevice>> {
        let adapters = self.manager.adapters().await?;
        let central = adapters
            .into_iter()
            .next()
            .ok_or_else(|| anyhow::anyhow!("No Bluetooth adapters found"))?;

        central
            .start_scan(ScanFilter {
                services: vec![SERVICE_UUID],
            })
            .await?;

        time::sleep(timeout).await;

        let peripherals = central.peripherals().await?;
        let mut discovered = Vec::new();

        for peripheral in peripherals {
            if let Some(properties) = peripheral.properties().await? {
                // Check if it's our service
                if properties.services.contains(&SERVICE_UUID) {
                    let name = properties
                        .local_name
                        .unwrap_or_else(|| "Unknown".to_string());
                    let address = properties.address.to_string();
                    let rssi = properties.rssi;

                    // Extract brand ID from manufacturer data if possible
                    let brand_id = properties.manufacturer_data.keys().next().cloned();

                    discovered.push(DiscoveredDevice {
                        name,
                        address,
                        brand_id,
                        rssi,
                    });
                }
            }
        }

        central.stop_scan().await?;

        Ok(discovered)
    }
}
