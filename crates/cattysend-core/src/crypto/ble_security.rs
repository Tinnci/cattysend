use aes::cipher::{KeyIvInit, StreamCipher};
use base64::{Engine as _, engine::general_purpose};
use hkdf::Hkdf;
use p256::{
    EncodedPoint, PublicKey,
    ecdh::EphemeralSecret,
    elliptic_curve::sec1::{FromEncodedPoint, ToEncodedPoint},
};
use rand::rngs::OsRng;
use sha2::Sha256;

type Aes256Ctr = ctr::Ctr128BE<aes::Aes256>;

pub struct BleSecurity {
    secret: EphemeralSecret,
    public_key_b64: String,
}

pub struct SessionCipher {
    key: [u8; 32],
}

impl BleSecurity {
    /// Initialize and generate local EC key pair
    pub fn new() -> anyhow::Result<Self> {
        let secret = EphemeralSecret::random(&mut OsRng);
        let public_key = secret.public_key();

        let public_key_bytes = public_key.to_encoded_point(false);
        let public_key_b64 = general_purpose::STANDARD.encode(public_key_bytes.as_bytes());

        Ok(Self {
            secret,
            public_key_b64,
        })
    }

    pub fn get_public_key(&self) -> &str {
        &self.public_key_b64
    }

    /// Derive session key using peer's public key
    pub fn derive_session_key(self, peer_pub_key_b64: &str) -> anyhow::Result<SessionCipher> {
        let peer_pub_bytes = general_purpose::STANDARD.decode(peer_pub_key_b64)?;
        let encoded_point = EncodedPoint::from_bytes(&peer_pub_bytes)
            .map_err(|e| anyhow::anyhow!("Invalid peer public key encoding: {}", e))?;

        let peer_public = Option::<PublicKey>::from(PublicKey::from_encoded_point(&encoded_point))
            .ok_or_else(|| anyhow::anyhow!("Invalid peer public key"))?;

        let shared_secret = self.secret.diffie_hellman(&peer_public);
        let shared_secret_bytes = shared_secret.raw_secret_bytes();

        // HKDF-SHA256(shared_secret, salt="", info="")
        let hk = Hkdf::<Sha256>::new(None, shared_secret_bytes.as_slice());
        let mut session_key = [0u8; 32];
        hk.expand(&[], &mut session_key)
            .map_err(|_| anyhow::anyhow!("HKDF expansion failed"))?;

        Ok(SessionCipher { key: session_key })
    }
}

impl SessionCipher {
    pub fn new(key: [u8; 32]) -> Self {
        Self { key }
    }

    /// Encrypt data using AES-256-CTR
    pub fn encrypt(&self, data: &str) -> anyhow::Result<String> {
        let mut buffer = data.as_bytes().to_vec();
        let mut iv = [0u8; 16];
        // Fixed IV: "0102030405060708" (8 bytes) + Counter (8 bytes)
        iv[..8].copy_from_slice(b"01234567");
        // Counter is already zeros in iv[8..16]

        let mut cipher = Aes256Ctr::new(&self.key.into(), &iv.into());
        cipher.apply_keystream(&mut buffer);

        Ok(general_purpose::STANDARD.encode(buffer))
    }

    /// Decrypt data using AES-256-CTR
    pub fn decrypt(&self, encoded_data: &str) -> anyhow::Result<String> {
        let mut buffer = general_purpose::STANDARD.decode(encoded_data)?;
        let mut iv = [0u8; 16];
        iv[..8].copy_from_slice(b"01234567");

        let mut cipher = Aes256Ctr::new(&self.key.into(), &iv.into());
        cipher.apply_keystream(&mut buffer);

        String::from_utf8(buffer).map_err(Into::into)
    }
}
