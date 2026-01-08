//! Cattysend Crypto Module
//!
//! 实现与 CatShare (Android) 完全兼容的加密逻辑：
//! - ECDH (P-256) 密钥交换
//! - AES-256-CTR 加密（使用固定 IV）
//!
//! 关键兼容性说明：
//! 1. CatShare 使用 Java 的 KeyAgreement.generateSecret("TlsPremasterSecret")
//!    这会返回原始的 ECDH 共享密钥（32 字节），**不使用** HKDF
//! 2. AES IV 是字符串 "0102030405060708" 的 **ASCII 字节**，不是十六进制

use aes::cipher::{KeyIvInit, StreamCipher};
use base64::{Engine as _, engine::general_purpose};
use p256::{PublicKey, ecdh::EphemeralSecret, elliptic_curve::sec1::ToEncodedPoint};
use rand::rngs::OsRng;

type Aes256Ctr = ctr::Ctr128BE<aes::Aes256>;

/// CatShare 使用的固定 IV：字符串 "0102030405060708" 的 ASCII 字节
/// 实际字节: [0x30, 0x31, 0x30, 0x32, 0x30, 0x33, 0x30, 0x34, 0x30, 0x35, 0x30, 0x36, 0x30, 0x37, 0x30, 0x38]
const AES_IV: &[u8; 16] = b"0102030405060708";

pub struct BleSecurity {
    secret: EphemeralSecret,
    public_key_b64: String,
}

pub struct SessionCipher {
    key: [u8; 32],
}

impl BleSecurity {
    /// 生成本地 ECDH 密钥对
    pub fn new() -> anyhow::Result<Self> {
        let secret = EphemeralSecret::random(&mut OsRng);
        let public_key = secret.public_key();

        // CatShare 使用 X.509 SubjectPublicKeyInfo 格式 (Java ECPublicKey.getEncoded())
        // 在 Rust 中，我们使用 SEC1 uncompressed 格式，这与 Java 的编码兼容
        let public_key_bytes = public_key.to_encoded_point(false);
        let public_key_b64 = general_purpose::STANDARD.encode(public_key_bytes.as_bytes());

        Ok(Self {
            secret,
            public_key_b64,
        })
    }

    /// 获取 Base64 编码的公钥（用于 DeviceInfo.key）
    pub fn get_public_key(&self) -> &str {
        &self.public_key_b64
    }

    /// 使用对方公钥派生会话密钥
    ///
    /// 兼容性说明：
    /// - CatShare 使用 `KeyAgreement.generateSecret("TlsPremasterSecret")`
    /// - 这会返回原始 ECDH 共享密钥（32 字节），**不做任何 KDF 处理**
    pub fn derive_session_key(self, peer_pub_key_b64: &str) -> anyhow::Result<SessionCipher> {
        let peer_pub_bytes = general_purpose::STANDARD.decode(peer_pub_key_b64)?;

        // 尝试解析为 SEC1 格式（65 字节 uncompressed）或 X.509 SPKI 格式
        let peer_public = if peer_pub_bytes.len() == 65 && peer_pub_bytes[0] == 0x04 {
            // SEC1 uncompressed format
            PublicKey::from_sec1_bytes(&peer_pub_bytes)
                .map_err(|e| anyhow::anyhow!("Invalid SEC1 public key: {}", e))?
        } else {
            // X.509 SubjectPublicKeyInfo format (Java ECPublicKey.getEncoded())
            use p256::pkcs8::DecodePublicKey;
            PublicKey::from_public_key_der(&peer_pub_bytes)
                .map_err(|e| anyhow::anyhow!("Invalid SPKI public key: {}", e))?
        };

        // ECDH 密钥协商
        let shared_secret = self.secret.diffie_hellman(&peer_public);

        // **关键**: 直接使用原始共享密钥，不做 HKDF 处理
        // CatShare 的 Java 代码: agreement.generateSecret("TlsPremasterSecret")
        // 返回 32 字节的原始 ECDH 共享密钥
        let raw_secret = shared_secret.raw_secret_bytes();

        let mut key = [0u8; 32];
        key.copy_from_slice(raw_secret.as_slice());

        Ok(SessionCipher { key })
    }
}

pub struct SessionCipherRef<'a> {
    key: &'a [u8; 32],
}

impl SessionCipher {
    pub fn new(key: [u8; 32]) -> Self {
        Self { key }
    }

    pub fn as_ref(&self) -> SessionCipherRef<'_> {
        SessionCipherRef { key: &self.key }
    }

    /// 使用 AES-256-CTR 加密数据
    ///
    /// 兼容 CatShare 的加密方式：
    /// - 算法: AES/CTR/NoPadding
    /// - IV: 字符串 "0102030405060708" 的 ASCII 字节
    pub fn encrypt(&self, data: &str) -> anyhow::Result<String> {
        let mut buffer = data.as_bytes().to_vec();

        let mut cipher = Aes256Ctr::new(&self.key.into(), AES_IV.into());
        cipher.apply_keystream(&mut buffer);

        Ok(general_purpose::STANDARD.encode(buffer))
    }

    /// 使用 AES-256-CTR 解密数据
    pub fn decrypt(&self, encoded_data: &str) -> anyhow::Result<String> {
        let mut buffer = general_purpose::STANDARD.decode(encoded_data)?;

        let mut cipher = Aes256Ctr::new(&self.key.into(), AES_IV.into());
        cipher.apply_keystream(&mut buffer);

        String::from_utf8(buffer).map_err(Into::into)
    }
}

impl SessionCipherRef<'_> {
    pub fn encrypt(&self, data: &str) -> anyhow::Result<String> {
        let mut buffer = data.as_bytes().to_vec();
        let mut cipher = Aes256Ctr::new(self.key.into(), AES_IV.into());
        cipher.apply_keystream(&mut buffer);
        Ok(general_purpose::STANDARD.encode(buffer))
    }

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

    #[test]
    fn test_aes_iv_is_ascii() {
        // 验证 IV 是 "0102030405060708" 的 ASCII 字节
        assert_eq!(AES_IV, b"0102030405060708");
        assert_eq!(AES_IV[0], 0x30); // '0'
        assert_eq!(AES_IV[1], 0x31); // '1'
    }

    #[test]
    fn test_encrypt_decrypt_roundtrip() {
        let key = [0u8; 32];
        let cipher = SessionCipher::new(key);

        let plaintext = "Hello, 互传联盟!";
        let encrypted = cipher.encrypt(plaintext).unwrap();
        let decrypted = cipher.decrypt(&encrypted).unwrap();

        assert_eq!(plaintext, decrypted);
    }
}
