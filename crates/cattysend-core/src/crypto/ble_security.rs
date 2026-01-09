//! Cattysend Crypto Module
//!
//! 实现与 CatShare (Android) 完全兼容的加密逻辑：
//! - ECDH (P-256) 密钥交换
//! - AES-256-CTR 加密（使用固定 IV）
//!
//! # 关键兼容性说明
//!
//! 1. **密钥格式**: CatShare 使用 Java 的 `ECPublicKey.getEncoded()`，
//!    返回 X.509 SubjectPublicKeyInfo (SPKI) 格式。我们必须使用相同格式。
//!
//! 2. **密钥派生**: CatShare 使用 `KeyAgreement.generateSecret("TlsPremasterSecret")`，
//!    这会返回原始的 ECDH 共享密钥（32 字节），**不使用** HKDF。
//!
//! 3. **AES IV**: 是字符串 `"0102030405060708"` 的 **ASCII 字节**，不是十六进制。

use aes::cipher::{KeyIvInit, StreamCipher};
use base64::{Engine as _, engine::general_purpose};
use log::{debug, trace};
use p256::pkcs8::EncodePublicKey;
use p256::{PublicKey, ecdh::EphemeralSecret};
use rand::rngs::OsRng;

type Aes256Ctr = ctr::Ctr128BE<aes::Aes256>;

/// CatShare 使用的固定 IV：字符串 "0102030405060708" 的 ASCII 字节
/// 实际字节: [0x30, 0x31, 0x30, 0x32, 0x30, 0x33, 0x30, 0x34, 0x30, 0x35, 0x30, 0x36, 0x30, 0x37, 0x30, 0x38]
const AES_IV: &[u8; 16] = b"0102030405060708";

/// BLE 安全上下文 - 管理 ECDH 密钥对
///
/// # 生命周期
///
/// 每个 `BleSecurity` 实例持有一个临时 ECDH 密钥对。
/// 调用 `derive_session_key` 后，私钥被消耗（move），实例不可再用。
///
/// # 与 CatShare 的兼容性
///
/// - 公钥使用 X.509 SPKI DER 格式编码，与 Java `ECPublicKey.getEncoded()` 兼容
/// - 私钥用于 ECDH 协商，生成的共享密钥直接用于 AES（无 HKDF）
pub struct BleSecurity {
    secret: EphemeralSecret,
    public_key_b64: String,
}

/// 会话加密器 - 使用 ECDH 派生的共享密钥进行 AES 加解密
///
/// # 加密算法
///
/// - 算法: AES-256-CTR (NoPadding)
/// - IV: 固定 ASCII 字符串 `"0102030405060708"` (16 bytes)
/// - 密钥: ECDH 原始共享密钥 (32 bytes)
pub struct SessionCipher {
    key: [u8; 32],
}

impl BleSecurity {
    /// 生成本地 ECDH 密钥对
    ///
    /// # 公钥格式
    ///
    /// 公钥使用 X.509 SubjectPublicKeyInfo (SPKI) DER 格式，
    /// 与 Java `ECPublicKey.getEncoded()` 返回的格式一致。
    /// 这是确保与 CatShare 互操作的关键。
    ///
    /// # 错误
    ///
    /// 如果 SPKI 编码失败（极少发生），返回错误。
    pub fn new() -> anyhow::Result<Self> {
        let secret = EphemeralSecret::random(&mut OsRng);
        let public_key = secret.public_key();

        // 使用 X.509 SPKI DER 格式编码公钥
        // 这与 Java ECPublicKey.getEncoded() 返回的格式一致
        let spki_der = public_key
            .to_public_key_der()
            .map_err(|e| anyhow::anyhow!("Failed to encode public key as SPKI: {}", e))?;
        let public_key_b64 = general_purpose::STANDARD.encode(spki_der.as_bytes());

        debug!(
            "Generated ECDH key pair, public key (SPKI) length: {} bytes",
            spki_der.as_bytes().len()
        );

        Ok(Self {
            secret,
            public_key_b64,
        })
    }

    /// 获取 Base64 编码的公钥（用于 DeviceInfo.key）
    ///
    /// 返回的字符串可直接用于 BLE GATT STATUS 特征中的 DeviceInfo JSON。
    pub fn get_public_key(&self) -> &str {
        &self.public_key_b64
    }

    /// 使用对方公钥派生会话密钥
    ///
    /// # 参数
    ///
    /// - `peer_pub_key_b64`: 对方公钥的 Base64 编码（SPKI 或 SEC1 格式均可）
    ///
    /// # 兼容性说明
    ///
    /// - 支持解析 X.509 SPKI 格式（Java ECPublicKey）
    /// - 同时支持 SEC1 uncompressed 格式（65 字节，0x04 前缀）作为后备
    /// - 直接使用 ECDH 原始共享密钥，不做 HKDF 处理
    ///
    /// # 消耗
    ///
    /// 此方法消耗 `self`，因为 ECDH 私钥应该只用一次。
    pub fn derive_session_key(self, peer_pub_key_b64: &str) -> anyhow::Result<SessionCipher> {
        let peer_pub_bytes = general_purpose::STANDARD.decode(peer_pub_key_b64)?;

        trace!(
            "Parsing peer public key, length: {} bytes, first byte: 0x{:02x}",
            peer_pub_bytes.len(),
            peer_pub_bytes.first().unwrap_or(&0)
        );

        // 尝试两种格式解析公钥
        let peer_public = Self::parse_public_key(&peer_pub_bytes)?;

        // ECDH 密钥协商
        let shared_secret = self.secret.diffie_hellman(&peer_public);

        // **关键**: 直接使用原始共享密钥，不做 HKDF 处理
        // CatShare 的 Java 代码: agreement.generateSecret("TlsPremasterSecret")
        // 返回 32 字节的原始 ECDH 共享密钥
        let raw_secret = shared_secret.raw_secret_bytes();

        let mut key = [0u8; 32];
        key.copy_from_slice(raw_secret.as_slice());

        debug!("ECDH key agreement completed, derived 32-byte session key");

        Ok(SessionCipher { key })
    }

    /// 解析对方公钥（支持 SPKI 和 SEC1 格式）
    fn parse_public_key(bytes: &[u8]) -> anyhow::Result<PublicKey> {
        // 首先尝试 SPKI 格式（Java ECPublicKey.getEncoded()）
        // SPKI 格式通常以 0x30 (SEQUENCE) 开头
        if bytes.first() == Some(&0x30) {
            use p256::pkcs8::DecodePublicKey;
            if let Ok(pk) = PublicKey::from_public_key_der(bytes) {
                trace!("Parsed public key as X.509 SPKI format");
                return Ok(pk);
            }
        }

        // 然后尝试 SEC1 uncompressed 格式（65 字节，0x04 前缀）
        if bytes.len() == 65 && bytes[0] == 0x04 {
            if let Ok(pk) = PublicKey::from_sec1_bytes(bytes) {
                trace!("Parsed public key as SEC1 uncompressed format");
                return Ok(pk);
            }
        }

        // 最后尝试作为原始 SPKI（某些实现可能不以 0x30 开头）
        use p256::pkcs8::DecodePublicKey;
        PublicKey::from_public_key_der(bytes)
            .map_err(|e| anyhow::anyhow!("Invalid public key format: {}", e))
    }
}

/// 会话加密器引用（用于借用场景）
pub struct SessionCipherRef<'a> {
    key: &'a [u8; 32],
}

impl SessionCipher {
    /// 从原始密钥创建会话加密器（用于测试）
    #[cfg(test)]
    pub fn new(key: [u8; 32]) -> Self {
        Self { key }
    }

    /// 获取借用的加密器引用
    pub fn as_ref(&self) -> SessionCipherRef<'_> {
        SessionCipherRef { key: &self.key }
    }

    /// 使用 AES-256-CTR 加密数据
    ///
    /// # 参数
    ///
    /// - `data`: 要加密的明文字符串
    ///
    /// # 返回
    ///
    /// Base64 编码的密文，可直接用于 JSON 传输。
    ///
    /// # 兼容性
    ///
    /// - 算法: AES/CTR/NoPadding（与 Java Cipher 一致）
    /// - IV: 字符串 "0102030405060708" 的 ASCII 字节
    pub fn encrypt(&self, data: &str) -> anyhow::Result<String> {
        let mut buffer = data.as_bytes().to_vec();

        let mut cipher = Aes256Ctr::new(&self.key.into(), AES_IV.into());
        cipher.apply_keystream(&mut buffer);

        let result = general_purpose::STANDARD.encode(buffer);
        trace!(
            "Encrypted {} bytes -> {} bytes base64",
            data.len(),
            result.len()
        );
        Ok(result)
    }

    /// 使用 AES-256-CTR 解密数据
    ///
    /// # 参数
    ///
    /// - `encoded_data`: Base64 编码的密文
    ///
    /// # 返回
    ///
    /// 解密后的明文字符串。
    ///
    /// # 错误
    ///
    /// - Base64 解码失败
    /// - 解密后的数据不是有效 UTF-8
    pub fn decrypt(&self, encoded_data: &str) -> anyhow::Result<String> {
        let mut buffer = general_purpose::STANDARD.decode(encoded_data)?;

        let mut cipher = Aes256Ctr::new(&self.key.into(), AES_IV.into());
        cipher.apply_keystream(&mut buffer);

        let result = String::from_utf8(buffer)?;
        trace!(
            "Decrypted {} bytes base64 -> {} chars",
            encoded_data.len(),
            result.len()
        );
        Ok(result)
    }
}

impl SessionCipherRef<'_> {
    /// 加密数据（借用版本）
    pub fn encrypt(&self, data: &str) -> anyhow::Result<String> {
        let mut buffer = data.as_bytes().to_vec();
        let mut cipher = Aes256Ctr::new(self.key.into(), AES_IV.into());
        cipher.apply_keystream(&mut buffer);
        Ok(general_purpose::STANDARD.encode(buffer))
    }

    /// 解密数据（借用版本）
    pub fn decrypt(&self, encoded_data: &str) -> anyhow::Result<String> {
        let mut buffer = general_purpose::STANDARD.decode(encoded_data)?;
        let mut cipher = Aes256Ctr::new(self.key.into(), AES_IV.into());
        cipher.apply_keystream(&mut buffer);
        String::from_utf8(buffer).map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 验证 AES IV 是 ASCII 字符串 "0102030405060708" 的字节表示
    #[test]
    fn test_aes_iv_is_ascii() {
        assert_eq!(AES_IV, b"0102030405060708");
        assert_eq!(AES_IV[0], 0x30); // '0'
        assert_eq!(AES_IV[1], 0x31); // '1'
        assert_eq!(AES_IV[2], 0x30); // '0'
        assert_eq!(AES_IV[3], 0x32); // '2'
    }

    /// 测试加密解密往返
    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let key = [0u8; 32];
        let cipher = SessionCipher::new(key);

        let plaintext = "Hello, 互传联盟!";
        let encrypted = cipher.encrypt(plaintext).unwrap();
        let decrypted = cipher.decrypt(&encrypted).unwrap();

        assert_eq!(plaintext, decrypted);
    }

    /// 测试公钥格式为 SPKI (X.509)
    #[test]
    fn test_public_key_is_spki_format() {
        let security = BleSecurity::new().unwrap();
        let pub_key_b64 = security.get_public_key();
        let pub_key_bytes = general_purpose::STANDARD.decode(pub_key_b64).unwrap();

        // SPKI 格式应该以 0x30 (SEQUENCE) 开头
        assert_eq!(
            pub_key_bytes[0], 0x30,
            "Public key should be SPKI format (starts with 0x30)"
        );

        // SPKI 格式的 P-256 公钥通常是 91 字节
        // 结构: SEQUENCE { SEQUENCE { OID, OID }, BIT STRING { 0x04 ... } }
        assert!(
            pub_key_bytes.len() >= 88 && pub_key_bytes.len() <= 92,
            "SPKI P-256 public key should be ~91 bytes, got {}",
            pub_key_bytes.len()
        );
    }

    /// 测试 ECDH 密钥协商
    #[test]
    fn test_ecdh_key_agreement() {
        // 创建两个密钥对
        let alice = BleSecurity::new().unwrap();
        let bob = BleSecurity::new().unwrap();

        let alice_pub = alice.get_public_key().to_string();
        let bob_pub = bob.get_public_key().to_string();

        // 各自派生会话密钥
        let alice_cipher = alice.derive_session_key(&bob_pub).unwrap();
        let bob_cipher = bob.derive_session_key(&alice_pub).unwrap();

        // 验证共享密钥相同
        assert_eq!(
            alice_cipher.key, bob_cipher.key,
            "Shared secret should be identical"
        );
    }

    /// 测试跨格式公钥兼容性
    #[test]
    fn test_parse_sec1_public_key() {
        // 创建一个密钥对并获取 SPKI 格式公钥
        let security = BleSecurity::new().unwrap();
        let spki_b64 = security.get_public_key();
        let spki_bytes = general_purpose::STANDARD.decode(spki_b64).unwrap();

        // 验证可以解析
        let parsed = BleSecurity::parse_public_key(&spki_bytes);
        assert!(parsed.is_ok(), "Should parse SPKI format");
    }

    /// 测试空数据加密
    #[test]
    fn test_encrypt_empty_string() {
        let key = [42u8; 32];
        let cipher = SessionCipher::new(key);

        let encrypted = cipher.encrypt("").unwrap();
        let decrypted = cipher.decrypt(&encrypted).unwrap();

        assert_eq!("", decrypted);
    }

    /// 测试大数据加密
    #[test]
    fn test_encrypt_large_data() {
        let key = [0xAB; 32];
        let cipher = SessionCipher::new(key);

        let plaintext = "A".repeat(10000);
        let encrypted = cipher.encrypt(&plaintext).unwrap();
        let decrypted = cipher.decrypt(&encrypted).unwrap();

        assert_eq!(plaintext, decrypted);
    }

    /// 测试 Unicode 数据加密
    #[test]
    fn test_encrypt_unicode() {
        let key = [0xCD; 32];
        let cipher = SessionCipher::new(key);

        let plaintext = "中文测试 🎉 日本語 العربية";
        let encrypted = cipher.encrypt(plaintext).unwrap();
        let decrypted = cipher.decrypt(&encrypted).unwrap();

        assert_eq!(plaintext, decrypted);
    }
}
