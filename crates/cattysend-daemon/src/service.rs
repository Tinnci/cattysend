use anyhow::Result;
use async_trait::async_trait;
use cattysend_core::{
    AppSettings, BleScanner, BleSecurityPersistent, BrandId, ReceiveEvent, ReceiveOptions,
    Receiver, SendEvent, SendOptions, Sender, SimpleReceiveCallback, SimpleSendCallback,
};
use cattysend_proto::{
    CattySendService, CattySendServiceServer, DaemonDeviceInfo, DaemonStatus, DiscoveredDevice,
    Empty, ReceiveRequest, ReceiveStatus, ScanRequest, SendRequest, TransferProgress,
    UpdateSettingsRequest,
};
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;
use tokio::net::UnixListener;
use tokio::sync::Mutex;
use tokio_stream::wrappers::{ReceiverStream, UnixListenerStream};
use tonic::transport::Server;
use tonic::{Request, Response, Status};

pub async fn run_service() -> Result<()> {
    tracing::info!("核心服务初始化...");

    // 生成加密密钥对（持久化，在服务生命周期内保持一致）
    let security = Arc::new(BleSecurityPersistent::new()?);

    let socket_path = socket_path();
    // 确保旧 socket 被清理
    let _ = std::fs::remove_file(&socket_path);
    // 确保目录存在
    if let Some(parent) = socket_path.parent() {
        let _ = std::fs::create_dir_all(parent);
    }

    let uds = UnixListener::bind(&socket_path)?;
    let uds_stream = UnixListenerStream::new(uds);

    tracing::info!("gRPC 服务器监听于: {:?}", socket_path);

    let service = CattySendImpl::new(security);

    Server::builder()
        .add_service(CattySendServiceServer::new(service))
        .serve_with_incoming(uds_stream)
        .await?;

    Ok(())
}

fn socket_path() -> PathBuf {
    std::env::var("XDG_RUNTIME_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("/tmp"))
        .join("cattysend.sock")
}

pub struct CattySendImpl {
    devices: Arc<Mutex<Vec<cattysend_core::ble::DiscoveredDevice>>>,
    active_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    scan_task: Arc<Mutex<Option<tokio::task::JoinHandle<()>>>>,
    security: Arc<BleSecurityPersistent>,
    settings: Arc<Mutex<AppSettings>>,
}

impl CattySendImpl {
    pub fn new(security: Arc<BleSecurityPersistent>) -> Self {
        Self {
            devices: Arc::new(Mutex::new(Vec::new())),
            active_task: Arc::new(Mutex::new(None)),
            scan_task: Arc::new(Mutex::new(None)),
            security,
            settings: Arc::new(Mutex::new(AppSettings::load())),
        }
    }

    async fn cancel_active_task(&self) {
        let mut task = self.active_task.lock().await;
        if let Some(handle) = task.take() {
            handle.abort();
            tracing::info!("已中止当前活动任务");
        }
    }

    async fn cancel_scan_task(&self) {
        let mut task = self.scan_task.lock().await;
        if let Some(handle) = task.take() {
            handle.abort();
            tracing::info!("已中止当前扫描任务");
        }
    }
}

#[tonic::async_trait]
impl CattySendService for CattySendImpl {
    async fn get_status(&self, _request: Request<Empty>) -> Result<Response<DaemonStatus>, Status> {
        let state = if self.active_task.lock().await.is_some() {
            "busy"
        } else if self.scan_task.lock().await.is_some() {
            "scanning"
        } else {
            "idle"
        };

        let settings = self.settings.lock().await;
        Ok(Response::new(DaemonStatus {
            state: state.to_string(),
            current_task_id: None,
            device_info: Some(DaemonDeviceInfo {
                name: settings.device_name.clone(),
                brand: settings.brand_id.name().to_string(),
                use_5ghz: settings.supports_5ghz,
            }),
        }))
    }

    type ScanDevicesStream = ReceiverStream<Result<DiscoveredDevice, Status>>;

    async fn scan_devices(
        &self,
        request: Request<ScanRequest>,
    ) -> Result<Response<Self::ScanDevicesStream>, Status> {
        let req = request.into_inner();
        let timeout = if req.timeout_seconds == 0 {
            Duration::from_secs(30)
        } else {
            Duration::from_secs(req.timeout_seconds as u64)
        };

        tracing::info!("开始扫描: timeout={}s", timeout.as_secs());

        // 停止之前的扫描
        self.cancel_scan_task().await;
        self.devices.lock().await.clear();

        let (tx, rx) = tokio::sync::mpsc::channel(32);
        let devices_clone = self.devices.clone();

        let handle = tokio::spawn(async move {
            match BleScanner::new().await {
                Ok(scanner) => {
                    // 同时将发现的设备存入内存缓存，以便后续 SendFile 调用
                    let (internal_tx, mut internal_rx) =
                        tokio::sync::mpsc::channel::<cattysend_core::ble::DiscoveredDevice>(16);
                    let devices_target = devices_clone.clone();

                    tokio::spawn(async move {
                        while let Some(d) = internal_rx.recv().await {
                            let mut list = devices_target.lock().await;
                            if !list.iter().any(|existing| existing.address == d.address) {
                                list.push(d);
                            }
                        }
                    });

                    // 这里我们需要一个组合的回调或者简单的分发
                    // 由于 ChannelScanCallback 是泛型的，我们可以写一个包装
                    struct CompositeCallback {
                        tx_grpc: tokio::sync::mpsc::Sender<Result<DiscoveredDevice, Status>>,
                        tx_internal:
                            tokio::sync::mpsc::Sender<cattysend_core::ble::DiscoveredDevice>,
                    }

                    #[async_trait]
                    impl cattysend_core::ble::ScanCallback for CompositeCallback {
                        async fn on_device_found(&self, d: cattysend_core::ble::DiscoveredDevice) {
                            let proto_dev = DiscoveredDevice {
                                name: d.name.clone(),
                                address: d.address.clone(),
                                rssi: d.rssi.map(|r| r as i32),
                                brand: d.brand.clone(),
                                brand_id: d.brand_id.map(|id| id as i32),
                                supports_5ghz: d.supports_5ghz,
                            };
                            let _ = self.tx_grpc.send(Ok(proto_dev)).await;
                            let _ = self.tx_internal.send(d).await;
                        }
                    }

                    let composite = Arc::new(CompositeCallback {
                        tx_grpc: tx,
                        tx_internal: internal_tx,
                    });

                    let _ = scanner.scan(timeout, Some(composite)).await;
                }
                Err(e) => {
                    let _ = tx
                        .send(Err(Status::internal(format!("扫描器初始化失败: {}", e))))
                        .await;
                }
            }
        });

        *self.scan_task.lock().await = Some(handle);

        Ok(Response::new(ReceiverStream::new(rx)))
    }

    async fn stop_scan(&self, _request: Request<Empty>) -> Result<Response<Empty>, Status> {
        self.cancel_scan_task().await;
        Ok(Response::new(Empty {}))
    }

    type SendFileStream = ReceiverStream<Result<TransferProgress, Status>>;

    async fn send_file(
        &self,
        request: Request<SendRequest>,
    ) -> Result<Response<Self::SendFileStream>, Status> {
        let req = request.into_inner();
        let target_addr = req.target_address.clone();
        let files: Vec<PathBuf> = req.file_paths.iter().map(PathBuf::from).collect();

        // 查找缓存的设备信息
        let device_opt = self
            .devices
            .lock()
            .await
            .iter()
            .find(|d| d.address == target_addr)
            .cloned();

        let device = match device_opt {
            Some(d) => d,
            None => {
                return Err(Status::not_found(
                    "未在近期扫描结果中找到该设备，请先尝试扫描",
                ));
            }
        };

        self.cancel_active_task().await;

        let (tx, rx) = tokio::sync::mpsc::channel(32);
        let _security = self.security.clone();
        let settings = self.settings.lock().await;
        let options = SendOptions {
            sender_name: settings.device_name.clone(),
            use_5ghz: settings.supports_5ghz,
            ..Default::default()
        };
        drop(settings);

        let handle = tokio::spawn(async move {
            let (callback, mut event_rx) = SimpleSendCallback::new();

            // 启动事件转发协程
            let tx_clone = tx.clone();
            tokio::spawn(async move {
                while let Some(event) = event_rx.recv().await {
                    let progress = match event {
                        SendEvent::Status(s) => TransferProgress {
                            status: s,
                            ..Default::default()
                        },
                        SendEvent::Progress { sent, total } => TransferProgress {
                            status: "transferring".to_string(),
                            bytes_transferred: sent,
                            total_bytes: total,
                            ..Default::default()
                        },
                        SendEvent::Complete => TransferProgress {
                            status: "completed".to_string(),
                            ..Default::default()
                        },
                        SendEvent::Error(e) => TransferProgress {
                            status: "error".to_string(),
                            error_message: Some(e),
                            ..Default::default()
                        },
                    };
                    if tx_clone.send(Ok(progress)).await.is_err() {
                        break;
                    }
                }
            });

            match Sender::new(options) {
                Ok(sender) => {
                    if let Err(e) = sender.send_to_device(&device, files, &callback).await {
                        let _ = tx
                            .send(Err(Status::internal(format!("发送失败: {}", e))))
                            .await;
                    }
                }
                Err(e) => {
                    let _ = tx
                        .send(Err(Status::internal(format!("初始化发送器失败: {}", e))))
                        .await;
                }
            }
        });

        *self.active_task.lock().await = Some(handle);

        Ok(Response::new(ReceiverStream::new(rx)))
    }

    type StartReceiveStream = ReceiverStream<Result<ReceiveStatus, Status>>;

    async fn start_receive(
        &self,
        request: Request<ReceiveRequest>,
    ) -> Result<Response<Self::StartReceiveStream>, Status> {
        let req = request.into_inner();
        let settings = self.settings.lock().await;
        let device_name = req
            .device_name
            .unwrap_or_else(|| settings.device_name.clone());

        self.cancel_active_task().await;

        let (tx, rx) = tokio::sync::mpsc::channel(32);
        let options = ReceiveOptions {
            device_name,
            brand_id: settings.brand_id,
            supports_5ghz: settings.supports_5ghz,
            ..Default::default()
        };
        drop(settings);

        let handle = tokio::spawn(async move {
            let (callback, mut event_rx) = SimpleReceiveCallback::new(true);

            let tx_clone = tx.clone();
            tokio::spawn(async move {
                while let Some(event) = event_rx.recv().await {
                    let status = match event {
                        ReceiveEvent::Status(s) => ReceiveStatus {
                            stage: s,
                            ..Default::default()
                        },
                        ReceiveEvent::Progress { received, total } => ReceiveStatus {
                            stage: "receiving".to_string(),
                            progress: Some(TransferProgress {
                                bytes_transferred: received,
                                total_bytes: total,
                                ..Default::default()
                            }),
                            ..Default::default()
                        },
                        ReceiveEvent::Complete(files) => ReceiveStatus {
                            stage: "completed".to_string(),
                            received_files: files
                                .iter()
                                .map(|p| p.to_string_lossy().to_string())
                                .collect(),
                            ..Default::default()
                        },
                        ReceiveEvent::Error(e) => ReceiveStatus {
                            stage: "error".to_string(),
                            progress: Some(TransferProgress {
                                status: "error".to_string(),
                                error_message: Some(e),
                                ..Default::default()
                            }),
                            ..Default::default()
                        },
                        _ => continue,
                    };
                    if tx_clone.send(Ok(status)).await.is_err() {
                        break;
                    }
                }
            });

            match Receiver::new(options) {
                Ok(receiver) => {
                    if let Err(e) = receiver.start(&callback).await {
                        let _ = tx
                            .send(Err(Status::internal(format!("接收失败: {}", e))))
                            .await;
                    }
                }
                Err(e) => {
                    let _ = tx
                        .send(Err(Status::internal(format!("初始化接收器失败: {}", e))))
                        .await;
                }
            }
        });

        *self.active_task.lock().await = Some(handle);

        Ok(Response::new(ReceiverStream::new(rx)))
    }

    async fn stop_task(&self, _request: Request<Empty>) -> Result<Response<Empty>, Status> {
        self.cancel_active_task().await;
        Ok(Response::new(Empty {}))
    }

    async fn update_settings(
        &self,
        request: Request<UpdateSettingsRequest>,
    ) -> Result<Response<DaemonStatus>, Status> {
        let req = request.into_inner();
        let mut settings = self.settings.lock().await;

        if let Some(name) = req.device_name {
            settings.device_name = name;
        }
        if let Some(bid) = req.brand_id {
            let brand = BrandId::from_id(bid as u8);
            if brand != BrandId::Unknown {
                settings.brand_id = brand;
            }
        }
        if let Some(u5) = req.use_5ghz {
            settings.supports_5ghz = u5;
        }

        if req.save_to_disk {
            if let Err(e) = settings.save() {
                return Err(Status::internal(format!("保存配置失败: {}", e)));
            }
        }

        // 释放锁以便 GetStatus 可以运行
        drop(settings);

        // 返回当前状态
        self.get_status(Request::new(Empty {})).await
    }
}
