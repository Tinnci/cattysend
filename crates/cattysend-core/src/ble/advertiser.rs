//! BLE 广播器
//!
//! 提供低级别的 BLE 广播和 GATT 服务功能。
//! 通常应使用更高层的 `GattServer` 代替。

use log::info;

use crate::ble::{DeviceInfo, MAIN_SERVICE_UUID, P2P_CHAR_UUID, SERVICE_UUID, STATUS_CHAR_UUID};
use bluer::{
    adv::Advertisement,
    gatt::local::{Application, Characteristic, CharacteristicRead, Service},
};
use futures_util::FutureExt;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct BleAdvertiser {
    device_info: Arc<Mutex<String>>,
    /// Reserved for future P2P write functionality
    _p2p_data_tx: tokio::sync::mpsc::Sender<Vec<u8>>,
}

impl BleAdvertiser {
    pub fn new(p2p_data_tx: tokio::sync::mpsc::Sender<Vec<u8>>) -> Self {
        Self {
            device_info: Arc::new(Mutex::new(String::new())),
            _p2p_data_tx: p2p_data_tx,
        }
    }

    pub async fn set_device_info(&self, info: DeviceInfo) -> anyhow::Result<()> {
        let json = serde_json::to_string(&info)?;
        let mut data = self.device_info.lock().await;
        *data = json;
        Ok(())
    }

    pub async fn start(&self) -> anyhow::Result<()> {
        let session = bluer::Session::new().await?;
        let adapter = session.default_adapter().await?;
        adapter.set_powered(true).await?;

        let device_info = self.device_info.clone();

        let app = Application {
            services: vec![Service {
                uuid: MAIN_SERVICE_UUID,
                primary: true,
                characteristics: vec![
                    Characteristic {
                        uuid: STATUS_CHAR_UUID,
                        read: Some(CharacteristicRead {
                            read: true,
                            fun: Box::new(move |_| {
                                let device_info = device_info.clone();
                                async move {
                                    let data = device_info.lock().await;
                                    Ok(data.as_bytes().to_vec())
                                }
                                .boxed()
                            }),
                            ..Default::default()
                        }),
                        ..Default::default()
                    },
                    Characteristic {
                        uuid: P2P_CHAR_UUID,
                        ..Default::default()
                    },
                ],
                ..Default::default()
            }],
            ..Default::default()
        };

        let _app_handle = adapter.serve_gatt_application(app).await?;

        let adv = Advertisement {
            service_uuids: vec![SERVICE_UUID].into_iter().collect(),
            discoverable: Some(true),
            local_name: Some("Cattysend".to_string()),
            ..Default::default()
        };

        let _adv_handle = adapter.advertise(adv).await?;

        info!("BLE advertiser started");

        loop {
            tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
        }
    }
}
