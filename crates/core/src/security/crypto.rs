//! 저장 데이터 암호화 (XChaCha20-Poly1305).
//!
//! 키 보관:
//! - macOS: 키체인, Windows: 자격 증명 관리자 (`keyring` 크레이트)
//! - 그 외(개발용 Linux): 앱 데이터 폴더의 0600 권한 키 파일 — 운영체제 보안
//!   저장소보다 약하므로 정식 지원 플랫폼이 아니다.
use crate::error::{AppError, AppResult};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use chacha20poly1305::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    Key, XChaCha20Poly1305, XNonce,
};
use std::path::Path;

const PREFIX: &str = "enc:v1:";

#[derive(Clone)]
pub struct Cipher {
    key: Key,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyStoreKind {
    /// 운영체제 보안 저장소 (macOS 키체인 / Windows 자격 증명 관리자)
    OsKeychain,
    /// 권한 제한 키 파일 (개발/미지원 플랫폼용)
    KeyFile,
    /// 테스트용 메모리 키
    Memory,
}

impl Cipher {
    pub fn from_key_bytes(bytes: [u8; 32]) -> Self {
        Self { key: *Key::from_slice(&bytes) }
    }

    pub fn generate() -> Self {
        Self { key: XChaCha20Poly1305::generate_key(&mut OsRng) }
    }

    /// 운영체제 보안 저장소에서 키를 불러오거나 새로 만든다.
    pub fn load_or_create(data_dir: &Path) -> AppResult<(Self, KeyStoreKind)> {
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            let _ = data_dir;
            let entry = keyring::Entry::new("kr.philosophy-ai.local-ai", "data-encryption-key")
                .map_err(|_| AppError::Storage("운영체제 보안 저장소에 접근할 수 없습니다".into()))?;
            match entry.get_password() {
                Ok(b64) => {
                    let raw = B64.decode(b64.trim()).map_err(|_| AppError::Storage("암호화 키가 손상되었습니다".into()))?;
                    let arr: [u8; 32] = raw.try_into().map_err(|_| AppError::Storage("암호화 키가 손상되었습니다".into()))?;
                    Ok((Self::from_key_bytes(arr), KeyStoreKind::OsKeychain))
                }
                Err(keyring::Error::NoEntry) => {
                    let c = Self::generate();
                    entry
                        .set_password(&B64.encode(c.key.as_slice()))
                        .map_err(|_| AppError::Storage("암호화 키를 보안 저장소에 저장하지 못했습니다".into()))?;
                    Ok((c, KeyStoreKind::OsKeychain))
                }
                Err(_) => Err(AppError::Storage("운영체제 보안 저장소에서 암호화 키를 읽지 못했습니다".into())),
            }
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            let path = data_dir.join("data.key");
            if path.exists() {
                let raw = B64
                    .decode(std::fs::read_to_string(&path)?.trim())
                    .map_err(|_| AppError::Storage("암호화 키가 손상되었습니다".into()))?;
                let arr: [u8; 32] = raw.try_into().map_err(|_| AppError::Storage("암호화 키가 손상되었습니다".into()))?;
                return Ok((Self::from_key_bytes(arr), KeyStoreKind::KeyFile));
            }
            let c = Self::generate();
            std::fs::create_dir_all(data_dir)?;
            std::fs::write(&path, B64.encode(c.key.as_slice()))?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o600))?;
            }
            Ok((c, KeyStoreKind::KeyFile))
        }
    }

    /// 운영체제 보안 저장소의 키를 삭제한다 ("모든 데이터 삭제" 시 사용).
    pub fn destroy_stored_key(data_dir: &Path) {
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            let _ = data_dir;
            if let Ok(e) = keyring::Entry::new("kr.philosophy-ai.local-ai", "data-encryption-key") {
                let _ = e.delete_credential();
            }
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            let _ = std::fs::remove_file(data_dir.join("data.key"));
        }
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
