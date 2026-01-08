use crate::ble::{DeviceStatus, MAIN_SERVICE_UUID, P2P_CHAR_UUID, SERVICE_UUID, STATUS_CHAR_UUID};
use bluer::{
    adv::Advertisement,
    gatt::local::{Application, Characteristic, CharacteristicRead, Service},
};
use futures_util::FutureExt;
use std::sync::Arc;
use tokio::sync::Mutex;

pub struct BleAdvertiser {
    status_data: Arc<Mutex<String>>,
    #[allow(dead_code)]
    p2p_data_tx: tokio::sync::mpsc::Sender<Vec<u8>>,
}

impl BleAdvertiser {
    pub fn new(p2p_data_tx: tokio::sync::mpsc::Sender<Vec<u8>>) -> Self {
        Self {
            status_data: Arc::new(Mutex::new(String::new())),
            p2p_data_tx,
        }
    }

    pub async fn set_status(&self, status: DeviceStatus) -> anyhow::Result<()> {
        let json = serde_json::to_string(&status)?;
        let mut data = self.status_data.lock().await;
        *data = json;
        Ok(())
    }

    pub async fn start(&self) -> anyhow::Result<()> {
        let session = bluer::Session::new().await?;
        let adapter = session.default_adapter().await?;
        adapter.set_powered(true).await?;

        let status_data = self.status_data.clone();

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
                                let status_data = status_data.clone();
                                async move {
                                    let data = status_data.lock().await;
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

        // bluer 0.17 uses `le_advertisement_type` inside Advertisement directly
        let adv = Advertisement {
            service_uuids: vec![SERVICE_UUID].into_iter().collect(),
            discoverable: Some(true),
            local_name: Some("Cattysend-Linux".to_string()),
            ..Default::default()
        };

        let _adv_handle = adapter.advertise(adv).await?;

        println!("BLE Advertising started...");

        loop {
            tokio::time::sleep(std::time::Duration::from_secs(3600)).await;
        }
    }
}
