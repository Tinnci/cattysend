//! 集成测试 - 加密与协议兼容性
//!
//! 验证 cattysend-core 与 CatShare (Android) 的互操作性。

use cattysend_core::ble::DeviceInfo;
use cattysend_core::crypto::BleSecurity;
use cattysend_core::wifi::P2pInfo;

/// 测试完整的 ECDH 密钥交换流程
///
/// 模拟发送端和接收端的密钥协商过程
#[test]
fn test_full_ecdh_handshake() {
    // 1. 接收端生成密钥对并广播公钥
    let receiver = BleSecurity::new().unwrap();
    let receiver_pub_key = receiver.get_public_key().to_string();

    // 2. 发送端生成密钥对
    let sender = BleSecurity::new().unwrap();
    let sender_pub_key = sender.get_public_key().to_string();

    // 3. 发送端使用接收端公钥派生会话密钥并加密 P2pInfo
    let sender_cipher = sender.derive_session_key(&receiver_pub_key).unwrap();

    let original_ssid = "DIRECT-test1234";
    let original_psk = "password123";
    let original_mac = "AA:BB:CC:DD:EE:FF";

    let encrypted_ssid = sender_cipher.encrypt(original_ssid).unwrap();
    let encrypted_psk = sender_cipher.encrypt(original_psk).unwrap();
    let encrypted_mac = sender_cipher.encrypt(original_mac).unwrap();

    // 4. 接收端使用发送端公钥派生会话密钥并解密
    let receiver_cipher = receiver.derive_session_key(&sender_pub_key).unwrap();

    let decrypted_ssid = receiver_cipher.decrypt(&encrypted_ssid).unwrap();
    let decrypted_psk = receiver_cipher.decrypt(&encrypted_psk).unwrap();
    let decrypted_mac = receiver_cipher.decrypt(&encrypted_mac).unwrap();

    // 5. 验证解密结果
    assert_eq!(decrypted_ssid, original_ssid);
    assert_eq!(decrypted_psk, original_psk);
    assert_eq!(decrypted_mac, original_mac);
}

/// 测试 DeviceInfo 与 CatShare JSON 格式的完全兼容性
#[test]
fn test_device_info_catshare_compatibility() {
    // 模拟 CatShare 发送的 DeviceInfo JSON
    let catshare_json = r#"{
        "state": 0,
        "key": "MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAE...",
        "mac": "AA:BB:CC:DD:EE:FF",
        "catShare": 123
    }"#;

    // 解析
    let device_info: DeviceInfo = serde_json::from_str(catshare_json).unwrap();

    assert_eq!(device_info.state, 0);
    assert_eq!(
        device_info.key,
        Some("MFkwEwYHKoZIzj0CAQYIKoZIzj0DAQcDQgAE...".to_string())
    );
    assert_eq!(device_info.mac, "AA:BB:CC:DD:EE:FF");
    assert_eq!(device_info.cat_share, Some(123));

    // 序列化回去
    let serialized = serde_json::to_string(&device_info).unwrap();

    // 验证 camelCase 命名
    assert!(serialized.contains("\"catShare\":123"));
    assert!(!serialized.contains("\"cat_share\""));
}

/// 测试 P2pInfo 与 CatShare JSON 格式的完全兼容性
#[test]
fn test_p2p_info_catshare_compatibility() {
    // 模拟 CatShare 发送的加密 P2pInfo JSON
    let catshare_json = r#"{
        "id": "a1b2",
        "ssid": "ENCRYPTED_BASE64_SSID",
        "psk": "ENCRYPTED_BASE64_PSK",
        "mac": "ENCRYPTED_BASE64_MAC",
        "port": 8443,
        "key": "SENDER_PUBLIC_KEY_BASE64",
        "catShare": 1
    }"#;

    // 解析
    let p2p_info: P2pInfo = serde_json::from_str(catshare_json).unwrap();

    assert_eq!(p2p_info.id, Some("a1b2".to_string()));
    assert_eq!(p2p_info.ssid, "ENCRYPTED_BASE64_SSID");
    assert_eq!(p2p_info.psk, "ENCRYPTED_BASE64_PSK");
    assert_eq!(p2p_info.mac, "ENCRYPTED_BASE64_MAC");
    assert_eq!(p2p_info.port, 8443);
    assert_eq!(p2p_info.key, Some("SENDER_PUBLIC_KEY_BASE64".to_string()));
    assert_eq!(p2p_info.cat_share, Some(1));

    // 序列化回去
    let serialized = serde_json::to_string(&p2p_info).unwrap();

    // 验证 camelCase 命名
    assert!(serialized.contains("\"catShare\":1"));
    assert!(!serialized.contains("\"cat_share\""));
}

/// 测试完整的 P2pInfo 加密/解密流程
#[test]
fn test_p2p_info_encryption_roundtrip() {
    // 创建密钥对
    let receiver = BleSecurity::new().unwrap();
    let sender = BleSecurity::new().unwrap();

    let receiver_pub = receiver.get_public_key().to_string();
    let sender_pub = sender.get_public_key().to_string();

    // 原始 P2P 信息
    let original = P2pInfo::new(
        "DIRECT-abcd1234".to_string(),
        "secretpass".to_string(),
        "11:22:33:44:55:66".to_string(),
        9443,
    );

    // 发送端加密
    let sender_cipher = sender.derive_session_key(&receiver_pub).unwrap();
    let encrypted = P2pInfo::with_encryption(
        "sender123".to_string(),
        sender_cipher.encrypt(&original.ssid).unwrap(),
        sender_cipher.encrypt(&original.psk).unwrap(),
        sender_cipher.encrypt(&original.mac).unwrap(),
        original.port,
        sender_pub.clone(),
    );

    // 序列化（模拟 BLE 传输）
    let json = serde_json::to_string(&encrypted).unwrap();

    // 解析
    let received: P2pInfo = serde_json::from_str(&json).unwrap();

    // 接收端解密
    let receiver_cipher = receiver.derive_session_key(&sender_pub).unwrap();

    let decrypted_ssid = receiver_cipher.decrypt(&received.ssid).unwrap();
    let decrypted_psk = receiver_cipher.decrypt(&received.psk).unwrap();
    let decrypted_mac = receiver_cipher.decrypt(&received.mac).unwrap();

    // 验证
    assert_eq!(decrypted_ssid, original.ssid);
    assert_eq!(decrypted_psk, original.psk);
    assert_eq!(decrypted_mac, original.mac);
    assert_eq!(received.port, original.port);
}

/// 测试公钥格式兼容性 (SPKI)
#[test]
fn test_public_key_spki_format() {
    let security = BleSecurity::new().unwrap();
    let pub_key_b64 = security.get_public_key();

    // Base64 解码
    use base64::Engine;
    let pub_key_bytes = base64::engine::general_purpose::STANDARD
        .decode(pub_key_b64)
        .unwrap();

    // SPKI 格式验证
    // P-256 SPKI 公钥应该：
    // 1. 以 0x30 (SEQUENCE) 开头
    // 2. 长度约 91 字节
    assert_eq!(
        pub_key_bytes[0], 0x30,
        "Public key should be SPKI format (0x30 = SEQUENCE)"
    );

    assert!(
        pub_key_bytes.len() >= 88 && pub_key_bytes.len() <= 92,
        "SPKI P-256 public key should be ~91 bytes, got {}",
        pub_key_bytes.len()
    );
}
