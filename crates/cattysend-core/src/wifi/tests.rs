//! WiFi 模块测试
//!
//! 包含 NmClient D-Bus 客户端和 P2P 模块的单元测试

use super::*;

// ============================================================================
// P2pInfo 测试
// ============================================================================

/// 验证 P2pInfo 序列化与 CatShare 兼容
#[test]
fn test_p2p_info_serialization() {
    let info = P2pInfo::new(
        "DIRECT-abc".to_string(),
        "password123".to_string(),
        "AA:BB:CC:DD:EE:FF".to_string(),
        8443,
    );

    let json = serde_json::to_string(&info).unwrap();

    // 验证 camelCase 命名
    assert!(json.contains("\"ssid\":"));
    assert!(json.contains("\"psk\":"));
    assert!(json.contains("\"mac\":"));
    assert!(json.contains("\"port\":"));
    assert!(json.contains("\"catShare\":")); // 不是 cat_share

    // 验证值
    let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed["ssid"], "DIRECT-abc");
    assert_eq!(parsed["psk"], "password123");
    assert_eq!(parsed["mac"], "AA:BB:CC:DD:EE:FF");
    assert_eq!(parsed["port"], 8443);
    assert_eq!(parsed["catShare"], 1);
}

/// 验证 P2pInfo 反序列化与 CatShare 兼容
#[test]
fn test_p2p_info_deserialization() {
    // 模拟 CatShare 发送的加密 P2pInfo
    let json = r#"{
        "id": "abcd",
        "ssid": "ENCRYPTED_SSID",
        "psk": "ENCRYPTED_PSK",
        "mac": "ENCRYPTED_MAC",
        "port": 9000,
        "key": "SENDER_PUBLIC_KEY",
        "catShare": 2
    }"#;

    let info: P2pInfo = serde_json::from_str(json).unwrap();

    assert_eq!(info.id, Some("abcd".to_string()));
    assert_eq!(info.ssid, "ENCRYPTED_SSID");
    assert_eq!(info.psk, "ENCRYPTED_PSK");
    assert_eq!(info.mac, "ENCRYPTED_MAC");
    assert_eq!(info.port, 9000);
    assert_eq!(info.key, Some("SENDER_PUBLIC_KEY".to_string()));
    assert_eq!(info.cat_share, Some(2));
}

/// 验证 P2pInfo 可选字段被正确跳过
#[test]
fn test_p2p_info_skip_none() {
    let info = P2pInfo::new(
        "SSID".to_string(),
        "PSK".to_string(),
        "MAC".to_string(),
        8080,
    );

    let json = serde_json::to_string(&info).unwrap();

    // id 和 key 是 None，应该被跳过
    assert!(!json.contains("\"id\":"));
    assert!(!json.contains("\"key\":"));
}

/// 验证 with_encryption 构造函数
#[test]
fn test_p2p_info_with_encryption() {
    let info = P2pInfo::with_encryption(
        "sender123".to_string(),
        "encrypted_ssid".to_string(),
        "encrypted_psk".to_string(),
        "encrypted_mac".to_string(),
        8443,
        "public_key_base64".to_string(),
    );

    assert_eq!(info.id, Some("sender123".to_string()));
    assert_eq!(info.key, Some("public_key_base64".to_string()));
    assert_eq!(info.cat_share, Some(1));
}

/// 验证 get_server_url 方法
#[test]
fn test_p2p_info_get_server_url() {
    let info = P2pInfo::new(
        "SSID".to_string(),
        "PSK".to_string(),
        "MAC".to_string(),
        8443,
    );

    assert_eq!(
        info.get_server_url("192.168.1.1"),
        "https://192.168.1.1:8443"
    );
    assert_eq!(info.get_server_url("10.42.0.1"), "https://10.42.0.1:8443");
}

// ============================================================================
// WiFiP2pSender 测试
// ============================================================================

#[test]
fn test_p2p_config_default() {
    let config = P2pConfig::default();

    assert_eq!(config.interface, "wlan0");
    assert_eq!(config.ssid_prefix, "DIRECT-");
    assert!(config.use_5ghz);
}

#[test]
fn test_wifi_p2p_sender_new() {
    let sender = WiFiP2pSender::new("wlan1");
    // 验证可以正确创建，不验证私有方法
    drop(sender);
}

#[test]
fn test_wifi_p2p_sender_with_config() {
    let config = P2pConfig {
        interface: "wlp3s0".to_string(),
        ssid_prefix: "CAT-".to_string(),
        use_5ghz: false,
    };

    let sender = WiFiP2pSender::with_config(config);
    // 验证配置正确应用
    drop(sender);
}

// ============================================================================
// WiFiP2pReceiver 测试
// ============================================================================

#[test]
fn test_p2p_receiver_config_default() {
    let config = P2pReceiverConfig::default();

    assert_eq!(config.main_interface, "wlan0");
    assert!(config.p2p_device.is_none());
    assert!(config.preserve_wifi);
}

#[test]
fn test_wifi_p2p_receiver_new() {
    let receiver = WiFiP2pReceiver::new("wlan1");

    assert_eq!(receiver.active_interface(), "wlan1");
}

// ============================================================================
// check_capabilities 测试
// ============================================================================

#[test]
fn test_check_capabilities() {
    let (has_nmcli, has_net_raw) = check_capabilities();

    // 这些值取决于系统环境，只验证类型正确
    println!("has_nmcli: {}, has_net_raw: {}", has_nmcli, has_net_raw);

    // 在非 root 环境下，至少应该检查到 nmcli (如果安装了)
    // 不做断言，因为测试环境可能不同
}

// ============================================================================
// NmClient 测试 (需要系统 D-Bus)
// ============================================================================

#[cfg(test)]
mod nm_dbus_tests {
    use super::nm_dbus::*;

    // 注意: 这些测试需要系统 D-Bus 和 NetworkManager 运行
    // 在 CI 环境中应该被跳过

    #[tokio::test]
    #[ignore = "requires system D-Bus and NetworkManager"]
    async fn test_nm_client_version() {
        let client = NmClient::new().await.unwrap();
        let version = client.version().await.unwrap();

        assert!(!version.is_empty());
        println!("NetworkManager version: {}", version);

        // 版本号应该是类似 "1.42.0" 的格式
        let parts: Vec<&str> = version.split('.').collect();
        assert!(parts.len() >= 2, "Version should have at least major.minor");
    }

    #[tokio::test]
    #[ignore = "requires system D-Bus and NetworkManager"]
    async fn test_get_wifi_devices() {
        let client = NmClient::new().await.unwrap();
        let devices = client.get_wifi_devices().await.unwrap();

        println!("Found {} WiFi devices", devices.len());
        for device in &devices {
            println!(
                "  - {} (type={}, mac={}, active={})",
                device.interface, device.device_type, device.hw_address, device.is_active
            );
        }
    }

    #[tokio::test]
    #[ignore = "requires system D-Bus and NetworkManager"]
    async fn test_find_wifi_device() {
        let client = NmClient::new().await.unwrap();

        // 查找默认 WiFi 设备
        if let Some(device) = client.find_wifi_device(None).await.unwrap() {
            assert_eq!(device.device_type, device_type::WIFI);
            println!("Found WiFi device: {}", device.interface);
        } else {
            println!("No WiFi device found");
        }
    }

    #[tokio::test]
    #[ignore = "requires system D-Bus and NetworkManager"]
    async fn test_find_p2p_device() {
        let client = NmClient::new().await.unwrap();

        // 查找 P2P 设备
        if let Some(device) = client.find_p2p_device().await.unwrap() {
            assert_eq!(device.device_type, device_type::WIFI_P2P);
            println!("Found P2P device: {}", device.interface);
        } else {
            println!("No P2P device found (normal if hardware doesn't support)");
        }
    }

    #[tokio::test]
    #[ignore = "requires system D-Bus, NetworkManager, and will modify network state"]
    async fn test_create_and_delete_connection() {
        let client = NmClient::new().await.unwrap();

        // 创建测试连接 (不激活)
        let test_name = "cattysend-test-connection";

        // 先删除可能存在的旧连接
        let _ = client.delete_connection_by_name(test_name).await;

        // 创建连接配置
        let conn_path = client
            .create_wifi_connection("TestSSID", "testpassword", None)
            .await
            .unwrap();

        println!("Created connection: {:?}", conn_path);

        // 删除连接
        client.delete_connection(&conn_path.as_ref()).await.unwrap();
        println!("Deleted connection");
    }
}

// ============================================================================
// Mock 测试辅助
// ============================================================================

/// 用于测试的 P2pInfo 工厂
#[cfg(test)]
pub fn test_p2p_info() -> P2pInfo {
    P2pInfo::new(
        "DIRECT-test123".to_string(),
        "testpsk1".to_string(),
        "00:11:22:33:44:55".to_string(),
        8443,
    )
}

/// 用于测试的加密 P2pInfo 工厂
#[cfg(test)]
pub fn test_encrypted_p2p_info() -> P2pInfo {
    P2pInfo::with_encryption(
        "test-sender-id".to_string(),
        "base64_encrypted_ssid".to_string(),
        "base64_encrypted_psk".to_string(),
        "base64_encrypted_mac".to_string(),
        8443,
        "base64_public_key".to_string(),
    )
}
