//! 저장 데이터 암호화 (XChaCha20-Poly1305).
//!
//! 키 보관과 앱 잠금 암호는 `security::keystore`가 담당한다.
use crate::error::{AppError, AppResult};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use chacha20poly1305::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Key, XChaCha20Poly1305, XNonce,
};

const PREFIX: &str = "enc:v1:";

#[derive(Clone)]
pub struct Cipher {
    key: Key,
}

impl Cipher {
    pub fn from_key_bytes(bytes: [u8; 32]) -> Self {
        Self { key: *Key::from_slice(&bytes) }
    }

    pub fn generate() -> Self {
        Self { key: XChaCha20Poly1305::generate_key(&mut OsRng) }
    }

    /// 키 원본 (키 보관소에 감싸서 저장할 때만 사용).
    pub(crate) fn key_bytes(&self) -> [u8; 32] {
        let mut out = [0u8; 32];
        out.copy_from_slice(self.key.as_slice());
        out
    }

    pub fn encrypt(&self, plaintext: &str) -> AppResult<String> {
        let cipher = XChaCha20Poly1305::new(&self.key);
        let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
        let ct = cipher
            .encrypt(&nonce, plaintext.as_bytes())
            .map_err(|_| AppError::Storage("암호화에 실패했습니다".into()))?;
        let mut blob = nonce.to_vec();
        blob.extend_from_slice(&ct);
        Ok(format!("{PREFIX}{}", B64.encode(blob)))
    }

    pub fn decrypt(&self, stored: &str) -> AppResult<String> {
        let err = || AppError::Storage("저장된 데이터를 복호화하지 못했습니다".into());
        let b64 = stored.strip_prefix(PREFIX).ok_or_else(err)?;
        let blob = B64.decode(b64).map_err(|_| err())?;
        if blob.len() < 24 {
            return Err(err());
        }
        let (nonce, ct) = blob.split_at(24);
        let cipher = XChaCha20Poly1305::new(&self.key);
        let pt = cipher.decrypt(XNonce::from_slice(nonce), ct).map_err(|_| err())?;
        String::from_utf8(pt).map_err(|_| err())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn roundtrip_and_tamper_detection() {
        let c = Cipher::generate();
        let enc = c.encrypt("학생 상담 기록: 김철수").unwrap();
        assert!(enc.starts_with(PREFIX));
        assert!(!enc.contains("김철수"));
        assert_eq!(c.decrypt(&enc).unwrap(), "학생 상담 기록: 김철수");
        // 다른 키로는 복호화 불가
        assert!(Cipher::generate().decrypt(&enc).is_err());
        // 변조 감지
        let mut t = enc.clone();
        t.pop();
        t.push(if enc.ends_with('A') { 'B' } else { 'A' });
        assert!(c.decrypt(&t).is_err());
    }

    #[test]
    fn nonce_is_random() {
        let c = Cipher::generate();
        assert_ne!(c.encrypt("x").unwrap(), c.encrypt("x").unwrap());
    }
}
