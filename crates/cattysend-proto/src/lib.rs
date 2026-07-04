pub mod cattysend {
    tonic::include_proto!("cattysend");
}

// Re-exports for easier access
pub use cattysend::catty_send_service_client::CattySendServiceClient;
pub use cattysend::catty_send_service_server::{CattySendService, CattySendServiceServer};
pub use cattysend::{
    DaemonDeviceInfo, DaemonStatus, DiscoveredDevice, Empty, ReceiveRequest, ReceiveStatus,
    ScanRequest, SendRequest, TransferDirection, TransferProgress, UpdateSettingsRequest,
};
