//! 암호화 키 보관과 앱 잠금 암호.
//!
//! 저장 위치
//! - macOS: 키체인, Windows: 자격 증명 관리자 (`keyring` 크레이트)
//! - Linux(개발용·미지원): 앱 자료 폴더의 0600 권한 파일
//!
//! 저장 형식 (한 줄 문자열)
//! - `plain:v1:<키>` — 앱 잠금 없음. 운영체제 계정 보안에만 의존한다.
//! - `pass:v1:<m>:<t>:<p>:<salt>:<nonce+암호문>` — 앱 잠금 암호로 감싼 키.
//!   Argon2id(암호, salt)로 만든 키로 XChaCha20-Poly1305 암호화한다.
//!   암호를 모르면 키 보관소에 접근할 수 있는 사람도 자료를 읽을 수 없다.
//! - (구버전 0.1) 접두사 없는 base64 키 → `plain`으로 취급한다.
//!
//! 암호를 잊으면 자료를 되살릴 수 없다. 이것은 의도된 동작이다.
use super::crypto::Cipher;
use crate::error::{AppError, AppResult};
use argon2::{Algorithm, Argon2, Params, Version};
use base64::{engine::general_purpose::STANDARD as B64, Engine};
use chacha20poly1305::{
    aead::{Aead, AeadCore, KeyInit, OsRng},
    XChaCha20Poly1305, XNonce,
};
use chacha20poly1305::aead::rand_core::RngCore;
use serde::Serialize;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::{Duration, Instant};

#[cfg_attr(not(any(target_os = "macos", target_os = "windows")), allow(dead_code))]
const SERVICE: &str = "kr.philosophy-ai.local-ai";
#[cfg_attr(not(any(target_os = "macos", target_os = "windows")), allow(dead_code))]
const ACCOUNT: &str = "data-encryption-key";
pub const MIN_PASSCODE_CHARS: usize = 6;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyStoreKind {
    /// 운영체제 보안 저장소 (macOS 키체인 / Windows 자격 증명 관리자)
    OsKeychain,
    /// 권한 제한 키 파일 (개발/미지원 플랫폼용)
    KeyFile,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum KeyState {
    /// 키가 아직 없음 (첫 실행 또는 전체 초기화 직후)
    NoKey,
    /// 앱 잠금 없이 보관된 키
    Plain,
    /// 앱 잠금 암호로 감싼 키
    Passcode,
}

/// Argon2id 비용. 기본값은 일반 노트북에서 0.3~1초 정도 걸리도록 정했다.
#[derive(Debug, Clone, Copy)]
pub struct KdfParams {
    pub m_kib: u32,
    pub t: u32,
    pub p: u32,
}

impl Default for KdfParams {
    fn default() -> Self {
        Self { m_kib: 64 * 1024, t: 3, p: 1 }
    }
}

enum Backend {
    #[cfg(any(target_os = "macos", target_os = "windows"))]
    Keychain,
    File(PathBuf),
}

pub struct KeyStore {
    backend: Backend,
    params: KdfParams,
    /// 연속 실패 횟수와 다음 시도 가능 시각 (암호 대입 공격을 늦춘다)
    throttle: Mutex<(u32, Option<Instant>)>,
}

impl KeyStore {
    /// 운영체제에 맞는 기본 보관소.
    pub fn for_platform(data_dir: &Path) -> Self {
        #[cfg(any(target_os = "macos", target_os = "windows"))]
        {
            let _ = data_dir;
            Self::with_backend(Backend::Keychain, KdfParams::default())
        }
        #[cfg(not(any(target_os = "macos", target_os = "windows")))]
        {
            Self::with_backend(Backend::File(data_dir.join("data.key")), KdfParams::default())
        }
    }

    /// 파일 보관소 (테스트·개발용). `params`로 Argon2 비용을 낮출 수 있다.
    pub fn file(path: PathBuf, params: KdfParams) -> Self {
        Self::with_backend(Backend::File(path), params)
    }

    fn with_backend(backend: Backend, params: KdfParams) -> Self {
        Self { backend, params, throttle: Mutex::new((0, None)) }
    }

    pub fn kind(&self) -> KeyStoreKind {
        match self.backend {
            #[cfg(any(target_os = "macos", target_os = "windows"))]
            Backend::Keychain => KeyStoreKind::OsKeychain,
            Backend::File(_) => KeyStoreKind::KeyFile,
        }
    }

    // ---- 저수준 읽기/쓰기 ----
    fn read(&self) -> AppResult<Option<String>> {
        match &self.backend {
            #[cfg(any(target_os = "macos", target_os = "windows"))]
            Backend::Keychain => {
                let e = keyring::Entry::new(SERVICE, ACCOUNT).map_err(|_| AppError::Storage("운영체제 보안 저장소에 접근할 수 없습니다".into()))?;
                match e.get_password() {
                    Ok(s) => Ok(Some(s)),
                    Err(keyring::Error::NoEntry) => Ok(None),
                    Err(_) => Err(AppError::Storage("운영체제 보안 저장소에서 암호화 키를 읽지 못했습니다".into())),
                }
            }
            Backend::File(p) => match std::fs::read_to_string(p) {
                Ok(s) => Ok(Some(s.trim().to_string())),
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
                Err(e) => Err(e.into()),
            },
        }
    }

    fn write(&self, value: &str) -> AppResult<()> {
        match &self.backend {
            #[cfg(any(target_os = "macos", target_os = "windows"))]
            Backend::Keychain => {
                let e = keyring::Entry::new(SERVICE, ACCOUNT).map_err(|_| AppError::Storage("운영체제 보안 저장소에 접근할 수 없습니다".into()))?;
                e.set_password(value).map_err(|_| AppError::Storage("암호화 키를 보안 저장소에 저장하지 못했습니다".into()))
            }
            Backend::File(p) => {
                if let Some(d) = p.parent() {
                    std::fs::create_dir_all(d)?;
                }
                // 임시 파일에 쓴 뒤 교체해, 쓰는 도중 꺼져도 키가 깨지지 않게 한다.
                let tmp = p.with_extension("key.tmp");
                std::fs::write(&tmp, value)?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    std::fs::set_permissions(&tmp, std::fs::Permissions::from_mode(0o600))?;
                }
                std::fs::rename(&tmp, p)?;
                Ok(())
            }
        }
    }

    /// 키를 파기한다. 이후 기존 암호화 자료(백업 사본 포함)는 읽을 수 없다.
    pub fn destroy(&self) -> AppResult<()> {
        match &self.backend {
            #[cfg(any(target_os = "macos", target_os = "windows"))]
            Backend::Keychain => {
                if let Ok(e) = keyring::Entry::new(SERVICE, ACCOUNT) {
                    match e.delete_credential() {
                        Ok(()) | Err(keyring::Error::NoEntry) => {}
                        Err(_) => return Err(AppError::Storage("보안 저장소의 키를 지우지 못했습니다".into())),
                    }
                }
                Ok(())
            }
            Backend::File(p) => {
                if p.exists() {
                    // 파일 내용을 덮어쓴 뒤 지운다 (SSD에서는 완전 삭제가 보장되지 않으나 키 파기가 1차 방어선)
                    let len = std::fs::metadata(p).map(|m| m.len()).unwrap_or(0) as usize;
                    let _ = std::fs::write(p, vec![0u8; len.max(64)]);
                    std::fs::remove_file(p)?;
                }
                Ok(())
            }
        }
    }

    // ---- 상태 ----
    pub fn state(&self) -> AppResult<KeyState> {
        Ok(match self.read()? {
            None => KeyState::NoKey,
            Some(s) if s.starts_with("pass:v1:") => KeyState::Passcode,
            Some(_) => KeyState::Plain,
        })
    }

    /// 새 키를 만들어 잠금 없이 저장한다.
    pub fn create_plain(&self) -> AppResult<Cipher> {
        let c = Cipher::generate();
        self.write(&format!("plain:v1:{}", B64.encode(c.key_bytes())))?;
        Ok(c)
    }

    /// 잠금 없이 보관된 키를 연다.
    pub fn open_plain(&self) -> AppResult<Cipher> {
        let s = self.read()?.ok_or_else(|| AppError::Storage("암호화 키가 없습니다".into()))?;
        if s.starts_with("pass:v1:") {
            return Err(AppError::Locked);
        }
        let b64 = s.strip_prefix("plain:v1:").unwrap_or(&s);
        decode_key(b64).map(Cipher::from_key_bytes)
    }

    /// 앱 잠금 암호로 키를 연다. 연속 실패 시 대기 시간이 늘어난다.
    pub fn open_with_passcode(&self, passcode: &str) -> AppResult<Cipher> {
        {
            let t = self.throttle.lock().unwrap();
            if let Some(until) = t.1 {
                let now = Instant::now();
                if now < until {
                    let secs = (until - now).as_secs() + 1;
                    return Err(AppError::Throttled(format!("{secs}초 뒤에 다시 시도해 주세요")));
                }
            }
        }
        let s = self.read()?.ok_or_else(|| AppError::Storage("암호화 키가 없습니다".into()))?;
        let result = unwrap_with_passcode(&s, passcode);
        let mut t = self.throttle.lock().unwrap();
        match result {
            Ok(c) => {
                *t = (0, None);
                Ok(c)
            }
            Err(e) => {
                t.0 += 1;
                // 3회까지는 바로 재시도, 이후 2,4,8…초 (최대 5분)
                if t.0 >= 3 {
                    let secs = 2u64.saturating_pow(t.0 - 2).min(300);
                    t.1 = Some(Instant::now() + Duration::from_secs(secs));
                }
                Err(e)
            }
        }
    }

    pub fn failed_attempts(&self) -> u32 {
        self.throttle.lock().unwrap().0
    }

    /// 현재 키를 앱 잠금 암호로 감싸 저장한다 (설정·변경 공용).
    pub fn set_passcode(&self, cipher: &Cipher, passcode: &str) -> AppResult<()> {
        validate_passcode(passcode)?;
        let wrapped = wrap_with_passcode(cipher, passcode, self.params)?;
        self.write(&wrapped)?;
        *self.throttle.lock().unwrap() = (0, None);
        Ok(())
    }

    /// 앱 잠금을 끈다 (키를 잠금 없이 다시 저장).
    pub fn remove_passcode(&self, cipher: &Cipher) -> AppResult<()> {
        self.write(&format!("plain:v1:{}", B64.encode(cipher.key_bytes())))
    }
}

pub fn validate_passcode(p: &str) -> AppResult<()> {
    let n = p.chars().count();
    if n < MIN_PASSCODE_CHARS {
        return Err(AppError::Other(format!("잠금 암호는 {MIN_PASSCODE_CHARS}자 이상으로 정해 주세요")));
    }
    if n > 128 {
        return Err(AppError::Other("잠금 암호가 너무 깁니다".into()));
    }
    let first = p.chars().next().unwrap();
    if p.chars().all(|c| c == first) {
        return Err(AppError::Other("같은 글자만 반복한 암호는 쓸 수 없습니다".into()));
    }
    let common = ["123456", "1234567", "12345678", "123456789", "1234567890", "password", "qwerty", "111111", "000000", "abcdef", "abc123", "654321"];
    if common.contains(&p.to_lowercase().as_str()) {
        return Err(AppError::Other("너무 쉬운 암호입니다. 다른 암호를 정해 주세요".into()));
    }
    Ok(())
}

fn decode_key(b64: &str) -> AppResult<[u8; 32]> {
    let raw = B64.decode(b64.trim()).map_err(|_| AppError::Storage("암호화 키가 손상되었습니다".into()))?;
    raw.try_into().map_err(|_| AppError::Storage("암호화 키가 손상되었습니다".into()))
}

fn derive(passcode: &str, salt: &[u8], params: KdfParams) -> AppResult<[u8; 32]> {
    let p = Params::new(params.m_kib, params.t, params.p, Some(32)).map_err(|_| AppError::Storage("잠금 설정이 올바르지 않습니다".into()))?;
    let mut out = [0u8; 32];
    Argon2::new(Algorithm::Argon2id, Version::V0x13, p)
        .hash_password_into(passcode.as_bytes(), salt, &mut out)
        .map_err(|_| AppError::Storage("잠금 암호를 처리하지 못했습니다".into()))?;
    Ok(out)
}

fn wrap_with_passcode(cipher: &Cipher, passcode: &str, params: KdfParams) -> AppResult<String> {
    let mut salt = [0u8; 16];
    OsRng.fill_bytes(&mut salt);
    let kek = derive(passcode, &salt, params)?;
    let aead = XChaCha20Poly1305::new((&kek).into());
    let nonce = XChaCha20Poly1305::generate_nonce(&mut OsRng);
    let ct = aead.encrypt(&nonce, cipher.key_bytes().as_slice()).map_err(|_| AppError::Storage("키를 잠그지 못했습니다".into()))?;
    let mut blob = nonce.to_vec();
    blob.extend_from_slice(&ct);
    Ok(format!("pass:v1:{}:{}:{}:{}:{}", params.m_kib, params.t, params.p, B64.encode(salt), B64.encode(blob)))
}

fn unwrap_with_passcode(stored: &str, passcode: &str) -> AppResult<Cipher> {
    let bad = || AppError::Storage("잠금 정보가 손상되었습니다".into());
    let rest = stored.strip_prefix("pass:v1:").ok_or_else(|| AppError::Other("앱 잠금이 설정되어 있지 않습니다".into()))?;
    let parts: Vec<&str> = rest.split(':').collect();
    if parts.len() != 5 {
        return Err(bad());
    }
    let params = KdfParams {
        m_kib: parts[0].parse().map_err(|_| bad())?,
        t: parts[1].parse().map_err(|_| bad())?,
        p: parts[2].parse().map_err(|_| bad())?,
    };
    // 비정상적으로 큰 값으로 메모리를 고갈시키는 변조 방지
    if params.m_kib > 1024 * 1024 || params.t > 20 || params.p > 16 {
        return Err(bad());
    }
    let salt = B64.decode(parts[3]).map_err(|_| bad())?;
    let blob = B64.decode(parts[4]).map_err(|_| bad())?;
    if blob.len() < 24 + 16 {
        return Err(bad());
    }
    let kek = derive(passcode, &salt, params)?;
    let (nonce, ct) = blob.split_at(24);
    let key = XChaCha20Poly1305::new((&kek).into())
        .decrypt(XNonce::from_slice(nonce), ct)
        .map_err(|_| AppError::WrongPasscode)?;
    let arr: [u8; 32] = key.try_into().map_err(|_| bad())?;
    Ok(Cipher::from_key_bytes(arr))
}

#[cfg(test)]
mod tests {
    use super::*;

    const FAST: KdfParams = KdfParams { m_kib: 256, t: 1, p: 1 };

    fn store() -> (tempfile::TempDir, KeyStore) {
        let d = tempfile::tempdir().unwrap();
        let ks = KeyStore::file(d.path().join("data.key"), FAST);
        (d, ks)
    }

    #[test]
    fn plain_lifecycle_and_legacy_format() {
        let (d, ks) = store();
        assert_eq!(ks.state().unwrap(), KeyState::NoKey);
        let c = ks.create_plain().unwrap();
        assert_eq!(ks.state().unwrap(), KeyState::Plain);
        let enc = c.encrypt("x").unwrap();
        assert_eq!(ks.open_plain().unwrap().decrypt(&enc).unwrap(), "x");
        // 0.1 버전 형식 (접두사 없는 base64)
        std::fs::write(d.path().join("data.key"), B64.encode(c.key_bytes())).unwrap();
        assert_eq!(ks.state().unwrap(), KeyState::Plain);
        assert_eq!(ks.open_plain().unwrap().decrypt(&enc).unwrap(), "x");
    }

    #[test]
    fn passcode_wraps_key_and_rejects_wrong_code() {
        let (d, ks) = store();
        let c = ks.create_plain().unwrap();
        let enc = c.encrypt("학생 상담 기록").unwrap();
        ks.set_passcode(&c, "교수님암호2026").unwrap();
        assert_eq!(ks.state().unwrap(), KeyState::Passcode);
        // 저장된 값에 키 원문이 없어야 한다
        let stored = std::fs::read_to_string(d.path().join("data.key")).unwrap();
        assert!(!stored.contains(&B64.encode(c.key_bytes())));
        // 잠금 상태에서는 암호 없이 열 수 없다
        assert!(matches!(ks.open_plain(), Err(AppError::Locked)));
        assert!(matches!(ks.open_with_passcode("틀린암호입니다"), Err(AppError::WrongPasscode)));
        let opened = ks.open_with_passcode("교수님암호2026").unwrap();
        assert_eq!(opened.decrypt(&enc).unwrap(), "학생 상담 기록");
        // 암호 변경 후에도 같은 키 (기존 자료 그대로 읽힘)
        ks.set_passcode(&opened, "새로운암호!!").unwrap();
        assert!(ks.open_with_passcode("교수님암호2026").is_err());
        assert_eq!(ks.open_with_passcode("새로운암호!!").unwrap().decrypt(&enc).unwrap(), "학생 상담 기록");
        // 잠금 해제(끄기)
        ks.remove_passcode(&opened).unwrap();
        assert_eq!(ks.state().unwrap(), KeyState::Plain);
        assert_eq!(ks.open_plain().unwrap().decrypt(&enc).unwrap(), "학생 상담 기록");
    }

    #[test]
    fn repeated_failures_are_throttled() {
        let (_d, ks) = store();
        let c = ks.create_plain().unwrap();
        ks.set_passcode(&c, "correct-horse").unwrap();
        for _ in 0..3 {
            assert!(matches!(ks.open_with_passcode("wrong-pass"), Err(AppError::WrongPasscode)));
        }
        // 3회 실패 후에는 올바른 암호도 잠시 거부
        assert!(matches!(ks.open_with_passcode("correct-horse"), Err(AppError::Throttled(_))));
        assert_eq!(ks.failed_attempts(), 3);
    }

    #[test]
    fn passcode_policy() {
        assert!(validate_passcode("12345").is_err());
        assert!(validate_passcode("aaaaaaaa").is_err());
        assert!(validate_passcode("123456").is_err());
        assert!(validate_passcode("Password").is_err());
        assert!(validate_passcode("철학과연구실7").is_ok());
    }

    #[test]
    fn tampered_params_are_rejected() {
        let (d, ks) = store();
        let c = ks.create_plain().unwrap();
        ks.set_passcode(&c, "correct-horse").unwrap();
        let p = d.path().join("data.key");
        let s = std::fs::read_to_string(&p).unwrap().replacen(":256:", ":99999999:", 1);
        std::fs::write(&p, s).unwrap();
        assert!(matches!(ks.open_with_passcode("correct-horse"), Err(AppError::Storage(_))));
    }

    #[test]
    fn destroy_removes_key() {
        let (d, ks) = store();
        ks.create_plain().unwrap();
        ks.destroy().unwrap();
        assert_eq!(ks.state().unwrap(), KeyState::NoKey);
        assert!(!d.path().join("data.key").exists());
    }
}
