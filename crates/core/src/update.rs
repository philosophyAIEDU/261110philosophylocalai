//! 앱 업데이트 — GitHub 릴리스에서 새 버전을 확인하고 설치 파일을 안전하게 내려받는다.
//!
//! 흐름: (사용자가 버튼을 눌러야만) 릴리스 목록 조회 → 버전 비교 → 릴리스 노트 표시
//!       → 이 컴퓨터용 설치 파일 내려받기 → GitHub가 기록한 SHA-256과 대조 → 설치 파일 실행
//!
//! 보안 원칙
//! - 자동으로 확인하지 않는다. 교수님이 누를 때만 접속한다.
//! - 접속 대상은 NetGuard가 GitHub 호스트로 제한한다.
//! - 파일 이름은 안전한 글자만 허용하고, 크기 상한과 SHA-256 무결성 확인을 통과해야만 저장한다.
//! - SHA-256 값이 없는 자산은 내려받지 않는다 (fail-closed).
use crate::error::{AppError, AppResult};
use crate::security::netguard::Purpose;
use crate::security::NetGuard;
use futures_util::{Stream, StreamExt};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::cmp::Ordering;
use std::path::{Path, PathBuf};
use tokio::io::AsyncWriteExt;
use tokio_util::sync::CancellationToken;
use url::Url;

/// 업데이트를 받아 오는 GitHub 저장소.
pub const REPO_OWNER: &str = "philosophyAIEDU";
pub const REPO_NAME: &str = "261110philosophylocalai";
/// 설치 파일 최대 크기 (비정상적으로 큰 파일 거부)
pub const MAX_INSTALLER_BYTES: u64 = 500 * 1024 * 1024;

pub fn releases_api_url() -> Url {
    Url::parse(&format!("https://api.github.com/repos/{REPO_OWNER}/{REPO_NAME}/releases?per_page=20")).unwrap()
}

pub fn releases_page_url() -> String {
    format!("https://github.com/{REPO_OWNER}/{REPO_NAME}/releases")
}

// ---------------------------------------------------------------------------
// 버전 비교 (Semantic Versioning 2.0 우선순위 규칙)
// ---------------------------------------------------------------------------

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Version {
    pub major: u64,
    pub minor: u64,
    pub patch: u64,
    pub pre: Vec<String>,
}

impl Version {
    pub fn parse(s: &str) -> Option<Self> {
        let s = s.trim().trim_start_matches(['v', 'V']);
        let s = s.split('+').next()?; // 빌드 메타데이터 무시
        let (core, pre) = match s.split_once('-') {
            Some((c, p)) => (c, p.split('.').map(|x| x.to_string()).collect::<Vec<_>>()),
            None => (s, vec![]),
        };
        let mut it = core.split('.');
        let major = it.next()?.parse().ok()?;
        let minor = it.next()?.parse().ok()?;
        let patch = it.next()?.parse().ok()?;
        if it.next().is_some() || pre.iter().any(|p| p.is_empty() || !p.chars().all(|c| c.is_ascii_alphanumeric())) {
            return None;
        }
        Some(Self { major, minor, patch, pre })
    }

    pub fn is_prerelease(&self) -> bool {
        !self.pre.is_empty()
    }
}

impl PartialOrd for Version {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl Ord for Version {
    fn cmp(&self, o: &Self) -> Ordering {
        (self.major, self.minor, self.patch).cmp(&(o.major, o.minor, o.patch)).then_with(|| {
            match (self.pre.is_empty(), o.pre.is_empty()) {
                (true, true) => Ordering::Equal,
                (true, false) => Ordering::Greater, // 정식 > 시험판
                (false, true) => Ordering::Less,
                (false, false) => {
                    for (a, b) in self.pre.iter().zip(o.pre.iter()) {
                        let ord = match (a.parse::<u64>(), b.parse::<u64>()) {
                            (Ok(x), Ok(y)) => x.cmp(&y),
                            (Ok(_), Err(_)) => Ordering::Less,
                            (Err(_), Ok(_)) => Ordering::Greater,
                            (Err(_), Err(_)) => a.cmp(b),
                        };
                        if ord != Ordering::Equal {
                            return ord;
                        }
                    }
                    self.pre.len().cmp(&o.pre.len())
                }
            }
        })
    }
}

// ---------------------------------------------------------------------------
// 릴리스 목록 해석
// ---------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
struct GhAsset {
    name: String,
    size: u64,
    browser_download_url: String,
    #[serde(default)]
    digest: Option<String>,
}

#[derive(Debug, Deserialize)]
struct GhRelease {
    tag_name: String,
    #[serde(default)]
    name: Option<String>,
    #[serde(default)]
    body: Option<String>,
    #[serde(default)]
    draft: bool,
    #[serde(default)]
    prerelease: bool,
    #[serde(default)]
    published_at: Option<String>,
    #[serde(default)]
    html_url: Option<String>,
    #[serde(default)]
    assets: Vec<GhAsset>,
}

#[derive(Debug, Clone, Serialize, PartialEq)]
pub struct InstallerAsset {
    pub name: String,
    pub size: u64,
    pub url: String,
    /// GitHub가 기록한 SHA-256 (16진수 64자)
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct UpdateInfo {
    pub current_version: String,
    pub latest_version: Option<String>,
    pub update_available: bool,
    pub is_prerelease: bool,
    pub title: Option<String>,
    pub notes: Option<String>,
    pub published_at: Option<String>,
    pub release_url: String,
    /// 이 컴퓨터에 맞는 설치 파일 (없으면 None)
    pub asset: Option<InstallerAsset>,
    /// 설치 파일이 없거나 무결성 정보가 없을 때의 안내
    pub asset_note: Option<String>,
}

/// 이 컴퓨터(운영체제·CPU)에 맞는 설치 파일 이름인지.
pub fn asset_matches_platform(name: &str, os: &str, arch: &str) -> bool {
    let n = name.to_ascii_lowercase();
    match (os, arch) {
        ("windows", "x86_64") => n.ends_with("_x64-setup.exe"),
        ("windows", "aarch64") => n.ends_with("_arm64-setup.exe"),
        ("macos", _) => n.ends_with("_universal.dmg") || (arch == "aarch64" && n.ends_with("_aarch64.dmg")) || (arch == "x86_64" && n.ends_with("_x64.dmg")),
        ("linux", "x86_64") => n.ends_with("_amd64.deb"),
        _ => false,
    }
}

/// 파일 이름에 경로·특수문자가 없는지 (내려받은 파일을 앱 폴더 밖에 쓰지 못하게).
pub fn safe_file_name(name: &str) -> bool {
    !name.is_empty()
        && name.len() <= 200
        && !name.starts_with('.')
        && name.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '_' | '-'))
}

fn parse_digest(d: Option<&str>) -> Option<String> {
    let h = d?.strip_prefix("sha256:")?;
    (h.len() == 64 && h.chars().all(|c| c.is_ascii_hexdigit())).then(|| h.to_ascii_lowercase())
}

/// 릴리스 목록 JSON에서 현재보다 새로운 버전을 고른다.
pub fn select_update(json: &str, current: &str, include_prerelease: bool, os: &str, arch: &str) -> AppResult<UpdateInfo> {
    let releases: Vec<GhRelease> = serde_json::from_str(json).map_err(|_| AppError::Network("업데이트 정보를 해석하지 못했습니다".into()))?;
    let cur = Version::parse(current).ok_or_else(|| AppError::Other("현재 버전을 알 수 없습니다".into()))?;
    let best = releases
        .iter()
        .filter(|r| !r.draft)
        .filter_map(|r| Version::parse(&r.tag_name).map(|v| (v, r)))
        .filter(|(v, r)| include_prerelease || (!r.prerelease && !v.is_prerelease()))
        .max_by(|a, b| a.0.cmp(&b.0));

    let mut info = UpdateInfo {
        current_version: current.to_string(),
        latest_version: None,
        update_available: false,
        is_prerelease: false,
        title: None,
        notes: None,
        published_at: None,
        release_url: releases_page_url(),
        asset: None,
        asset_note: None,
    };
    let Some((ver, rel)) = best else { return Ok(info) };
    info.latest_version = Some(rel.tag_name.trim_start_matches('v').to_string());
    info.update_available = ver > cur;
    info.is_prerelease = rel.prerelease || ver.is_prerelease();
    info.title = rel.name.clone();
    // 릴리스 노트는 화면에 글자로만 표시한다 (링크·이미지·HTML은 화면에서 비활성화)
    info.notes = rel.body.as_ref().map(|b| b.chars().take(20_000).collect());
    info.published_at = rel.published_at.clone();
    if let Some(u) = &rel.html_url {
        if u.starts_with(&format!("https://github.com/{REPO_OWNER}/{REPO_NAME}/releases/")) {
            info.release_url = u.clone();
        }
    }
    match rel.assets.iter().find(|a| asset_matches_platform(&a.name, os, arch)) {
        None => info.asset_note = Some("이 컴퓨터용 설치 파일이 이 릴리스에 없습니다.".into()),
        Some(a) if !safe_file_name(&a.name) => info.asset_note = Some("설치 파일 이름이 올바르지 않아 내려받지 않습니다.".into()),
        Some(a) if a.size == 0 || a.size > MAX_INSTALLER_BYTES => info.asset_note = Some("설치 파일 크기가 올바르지 않아 내려받지 않습니다.".into()),
        Some(a) => match parse_digest(a.digest.as_deref()) {
            None => info.asset_note = Some("무결성 확인값(SHA-256)이 없어 안전하게 내려받을 수 없습니다. 릴리스 페이지를 확인해 주세요.".into()),
            Some(sha) => {
                let expected_prefix = format!("https://github.com/{REPO_OWNER}/{REPO_NAME}/releases/download/");
                if a.browser_download_url.starts_with(&expected_prefix) {
                    info.asset = Some(InstallerAsset { name: a.name.clone(), size: a.size, url: a.browser_download_url.clone(), sha256: sha });
                } else {
                    info.asset_note = Some("설치 파일 주소가 이 앱의 공식 저장소가 아니어서 내려받지 않습니다.".into());
                }
            }
        },
    }
    Ok(info)
}

/// GitHub에서 릴리스 목록을 받아 업데이트를 확인한다.
pub async fn check(guard: &NetGuard, current: &str, include_prerelease: bool) -> AppResult<UpdateInfo> {
    check_for_platform(guard, current, include_prerelease, std::env::consts::OS, std::env::consts::ARCH).await
}

/// 운영체제·CPU를 지정해 확인한다 (테스트용으로도 쓰인다).
pub async fn check_for_platform(guard: &NetGuard, current: &str, include_prerelease: bool, os: &str, arch: &str) -> AppResult<UpdateInfo> {
    let url = releases_api_url();
    let resp = guard
        .update_request(Purpose::UpdateCheck, &url, "업데이트 확인")?
        .timeout(std::time::Duration::from_secs(20))
        .send()
        .await
        .map_err(|_| AppError::Network("GitHub에 연결하지 못했습니다. 인터넷 연결을 확인해 주세요".into()))?;
    match resp.status().as_u16() {
        200 => {}
        403 | 429 => return Err(AppError::Network("GitHub 요청 한도를 넘었습니다. 잠시 뒤 다시 확인해 주세요".into())),
        404 => return Err(AppError::Network("업데이트 저장소를 찾을 수 없습니다 (비공개 저장소일 수 있습니다)".into())),
        s => return Err(AppError::Network(format!("업데이트 정보를 받지 못했습니다 (코드 {s})"))),
    }
    let bytes = resp.bytes().await.map_err(|_| AppError::Network("업데이트 정보를 받는 중 연결이 끊겼습니다".into()))?;
    if bytes.len() > 5 * 1024 * 1024 {
        return Err(AppError::Network("업데이트 정보가 비정상적으로 큽니다".into()));
    }
    select_update(&String::from_utf8_lossy(&bytes), current, include_prerelease, os, arch)
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct DownloadProgress {
    pub stage: String,
    pub total_bytes: u64,
    pub completed_bytes: u64,
    pub done: bool,
}

/// 바이트 스트림을 파일에 쓰면서 크기와 SHA-256을 확인한다. 실패하면 파일을 지운다.
pub async fn write_verified<S, E>(
    mut stream: S,
    dest: &Path,
    expected_sha256: &str,
    expected_size: u64,
    mut on_progress: impl FnMut(DownloadProgress),
    cancel: &CancellationToken,
) -> AppResult<()>
where
    S: Stream<Item = Result<Vec<u8>, E>> + Unpin,
{
    let part = dest.with_extension("part");
    let mut file = tokio::fs::File::create(&part).await?;
    let mut hasher = Sha256::new();
    let mut done = 0u64;
    let fail = |part: PathBuf, e: AppError| async move {
        let _ = tokio::fs::remove_file(&part).await;
        Err(e)
    };
    loop {
        let chunk = tokio::select! {
            _ = cancel.cancelled() => { drop(file); return fail(part, AppError::Cancelled).await; }
            c = stream.next() => c,
        };
        match chunk {
            Some(Ok(b)) => {
                done += b.len() as u64;
                if done > expected_size || done > MAX_INSTALLER_BYTES {
                    drop(file);
                    return fail(part, AppError::Blocked("설치 파일 크기가 예상과 달라 중단했습니다".into())).await;
                }
                hasher.update(&b);
                file.write_all(&b).await?;
                on_progress(DownloadProgress { stage: "새 버전을 내려받는 중".into(), total_bytes: expected_size, completed_bytes: done, done: false });
            }
            Some(Err(_)) => {
                drop(file);
                return fail(part, AppError::Network("내려받는 중 연결이 끊겼습니다. 다시 시도해 주세요".into())).await;
            }
            None => break,
        }
    }
    file.flush().await?;
    drop(file);
    if done != expected_size {
        return fail(part, AppError::Network("설치 파일을 끝까지 받지 못했습니다".into())).await;
    }
    on_progress(DownloadProgress { stage: "내려받은 파일이 올바른지 확인하는 중".into(), total_bytes: expected_size, completed_bytes: done, done: false });
    let actual = hex::encode(hasher.finalize());
    if !actual.eq_ignore_ascii_case(expected_sha256) {
        return fail(part, AppError::Blocked("내려받은 파일이 공식 릴리스와 일치하지 않아 삭제했습니다".into())).await;
    }
    tokio::fs::rename(&part, dest).await?;
    on_progress(DownloadProgress { stage: "확인 완료".into(), total_bytes: expected_size, completed_bytes: done, done: true });
    Ok(())
}

/// 설치 파일을 `dir`에 내려받고 무결성을 확인한다. 저장된 파일 경로를 돌려준다.
pub async fn download(
    guard: &NetGuard,
    asset: &InstallerAsset,
    dir: &Path,
    on_progress: impl FnMut(DownloadProgress),
    cancel: &CancellationToken,
) -> AppResult<PathBuf> {
    if !safe_file_name(&asset.name) || asset.size == 0 || asset.size > MAX_INSTALLER_BYTES || asset.sha256.len() != 64 {
        return Err(AppError::Blocked("설치 파일 정보가 올바르지 않습니다".into()));
    }
    let url = Url::parse(&asset.url).map_err(|_| AppError::Blocked("설치 파일 주소가 올바르지 않습니다".into()))?;
    if !asset.url.starts_with(&format!("https://github.com/{REPO_OWNER}/{REPO_NAME}/releases/download/")) {
        return Err(AppError::Blocked("공식 저장소의 설치 파일이 아닙니다".into()));
    }
    tokio::fs::create_dir_all(dir).await?;
    crate::runtime::engine::check_free_space(dir, asset.size * 2)?;
    // 이전에 받은 설치 파일 정리
    if let Ok(mut rd) = tokio::fs::read_dir(dir).await {
        while let Ok(Some(e)) = rd.next_entry().await {
            let _ = tokio::fs::remove_file(e.path()).await;
        }
    }
    let dest = dir.join(&asset.name);
    let resp = guard
        .update_request(Purpose::UpdateDownload, &url, &format!("업데이트 {} 내려받기", asset.name))?
        .send()
        .await
        .map_err(|_| AppError::Network("GitHub에 연결하지 못했습니다".into()))?;
    if !resp.status().is_success() {
        return Err(AppError::Network(format!("설치 파일을 받지 못했습니다 (코드 {})", resp.status().as_u16())));
    }
    let stream = resp.bytes_stream().map(|r| r.map(|b| b.to_vec()));
    write_verified(Box::pin(stream), &dest, &asset.sha256, asset.size, on_progress, cancel).await?;
    Ok(dest)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn v(s: &str) -> Version {
        Version::parse(s).unwrap()
    }

    #[test]
    fn semver_ordering() {
        assert!(v("0.2.0") > v("0.1.0"));
        assert!(v("0.1.0") > v("0.1.0-beta.1"));
        assert!(v("0.1.0-beta.2") > v("0.1.0-beta.1"));
        assert!(v("0.1.0-beta.10") > v("0.1.0-beta.9"));
        assert!(v("0.1.0-rc.1") > v("0.1.0-beta.9"));
        assert!(v("0.1.0-beta") < v("0.1.0-beta.1"));
        assert!(v("1.0.0") > v("0.99.99"));
        assert_eq!(v("v0.1.0-beta.1"), v("0.1.0-beta.1"));
        assert_eq!(v("0.1.0+build5"), v("0.1.0"));
        for bad in ["", "1.0", "1.0.0.0", "a.b.c", "1.0.0-", "1.0.0-be/ta"] {
            assert!(Version::parse(bad).is_none(), "{bad}");
        }
    }

    const SHA: &str = "1da0cab22abd489fa35015148ae077e552453d76595321252e68863f98454354";

    fn release(tag: &str, pre: bool, draft: bool, digest: Option<&str>) -> serde_json::Value {
        serde_json::json!({
            "tag_name": tag, "name": format!("필로소피 로컬 AI {tag}"), "body": "## 새 기능\n- 업데이트", "draft": draft, "prerelease": pre,
            "published_at": "2026-11-20T00:00:00Z",
            "html_url": format!("https://github.com/philosophyAIEDU/261110philosophylocalai/releases/tag/{tag}"),
            "assets": [
                {"name": format!("Philosophy.Local.AI_{}_x64-setup.exe", tag.trim_start_matches('v')), "size": 6806202,
                 "browser_download_url": format!("https://github.com/philosophyAIEDU/261110philosophylocalai/releases/download/{tag}/Philosophy.Local.AI_x64-setup.exe"),
                 "digest": digest},
                {"name": "Philosophy.Local.AI_universal.dmg", "size": 18079737,
                 "browser_download_url": format!("https://github.com/philosophyAIEDU/261110philosophylocalai/releases/download/{tag}/Philosophy.Local.AI_universal.dmg"),
                 "digest": digest}
            ]
        })
    }

    #[test]
    fn selects_newest_with_prerelease_rules() {
        let d = Some(format!("sha256:{SHA}"));
        let json = serde_json::json!([
            release("v0.1.0-beta.1", true, false, d.as_deref()),
            release("v0.3.0-beta.1", true, false, d.as_deref()),
            release("v0.2.0", false, false, d.as_deref()),
            release("v9.0.0", false, true, d.as_deref()),
        ])
        .to_string();
        let i = select_update(&json, "0.1.0-beta.1", true, "windows", "x86_64").unwrap();
        assert_eq!(i.latest_version.as_deref(), Some("0.3.0-beta.1"));
        assert!(i.update_available && i.is_prerelease);
        let a = i.asset.unwrap();
        assert_eq!(a.sha256, SHA);
        assert!(a.name.ends_with("_x64-setup.exe"));
        // 정식판만
        let i = select_update(&json, "0.1.0-beta.1", false, "macos", "aarch64").unwrap();
        assert_eq!(i.latest_version.as_deref(), Some("0.2.0"));
        assert!(i.asset.unwrap().name.ends_with("_universal.dmg"));
        // 이미 최신
        let i = select_update(&json, "0.3.0-beta.1", true, "windows", "x86_64").unwrap();
        assert!(!i.update_available);
        // 초안(draft)은 무시
        assert_ne!(i.latest_version.as_deref(), Some("9.0.0"));
    }

    #[test]
    fn refuses_assets_without_digest_or_from_elsewhere() {
        let json = serde_json::json!([release("v0.2.0", false, false, None)]).to_string();
        let i = select_update(&json, "0.1.0", true, "windows", "x86_64").unwrap();
        assert!(i.update_available && i.asset.is_none());
        assert!(i.asset_note.unwrap().contains("SHA-256"));

        let mut r = release("v0.2.0", false, false, Some(&format!("sha256:{SHA}")));
        r["assets"][0]["browser_download_url"] = "https://evil.example/x_x64-setup.exe".into();
        let i = select_update(&serde_json::json!([r]).to_string(), "0.1.0", true, "windows", "x86_64").unwrap();
        assert!(i.asset.is_none());

        let mut r = release("v0.2.0", false, false, Some(&format!("sha256:{SHA}")));
        r["assets"][0]["name"] = "../../evil_x64-setup.exe".into();
        let i = select_update(&serde_json::json!([r]).to_string(), "0.1.0", true, "windows", "x86_64").unwrap();
        assert!(i.asset.is_none());

        // 이 플랫폼용 파일 없음
        let i = select_update(&serde_json::json!([release("v0.2.0", false, false, Some(&format!("sha256:{SHA}")))]).to_string(), "0.1.0", true, "linux", "aarch64").unwrap();
        assert!(i.asset.is_none() && i.asset_note.is_some());
        assert!(select_update("not json", "0.1.0", true, "windows", "x86_64").is_err());
    }

    #[test]
    fn file_name_and_platform_rules() {
        assert!(safe_file_name("Philosophy.Local.AI_0.2.0_x64-setup.exe"));
        for bad in ["", "../a.exe", "a/b.exe", "a\\b.exe", ".hidden", "a b.exe", "가.exe"] {
            assert!(!safe_file_name(bad), "{bad}");
        }
        assert!(asset_matches_platform("X_0.2.0_x64-setup.exe", "windows", "x86_64"));
        assert!(!asset_matches_platform("X_0.2.0_x64-setup.exe", "macos", "aarch64"));
        assert!(asset_matches_platform("X_0.2.0_universal.dmg", "macos", "x86_64"));
    }

    fn chunks(data: &[u8]) -> futures_util::stream::Iter<std::vec::IntoIter<Result<Vec<u8>, ()>>> {
        futures_util::stream::iter(data.chunks(7).map(|c| Ok(c.to_vec())).collect::<Vec<_>>())
    }

    #[tokio::test]
    async fn verified_write_accepts_only_exact_content() {
        let d = tempfile::tempdir().unwrap();
        let data = b"installer bytes for testing".to_vec();
        let sha = hex::encode(Sha256::digest(&data));
        let dest = d.path().join("setup.exe");
        let c = CancellationToken::new();
        write_verified(chunks(&data), &dest, &sha, data.len() as u64, |_| {}, &c).await.unwrap();
        assert_eq!(std::fs::read(&dest).unwrap(), data);

        // 내용이 다르면 거부하고 파일을 남기지 않음
        let dest2 = d.path().join("tampered.exe");
        let mut bad = data.clone();
        bad[0] ^= 1;
        assert!(matches!(write_verified(chunks(&bad), &dest2, &sha, bad.len() as u64, |_| {}, &c).await, Err(AppError::Blocked(_))));
        assert!(!dest2.exists() && !dest2.with_extension("part").exists());

        // 예상보다 크면 중단
        let dest3 = d.path().join("big.exe");
        assert!(write_verified(chunks(&data), &dest3, &sha, 5, |_| {}, &c).await.is_err());
        assert!(!dest3.with_extension("part").exists());

        // 덜 받으면 실패
        let dest4 = d.path().join("short.exe");
        assert!(write_verified(chunks(&data[..10]), &dest4, &sha, data.len() as u64, |_| {}, &c).await.is_err());

        // 취소
        let c2 = CancellationToken::new();
        c2.cancel();
        let dest5 = d.path().join("cancel.exe");
        assert!(matches!(write_verified(chunks(&data), &dest5, &sha, data.len() as u64, |_| {}, &c2).await, Err(AppError::Cancelled)));
        assert!(!dest5.exists());
    }

    #[tokio::test]
    async fn download_rejects_bad_assets_before_any_network() {
        let g = NetGuard::new();
        g.set_update_consent(true);
        let d = tempfile::tempdir().unwrap();
        let c = CancellationToken::new();
        let ok = InstallerAsset { name: "A_x64-setup.exe".into(), size: 10, url: "https://github.com/philosophyAIEDU/261110philosophylocalai/releases/download/v1/A_x64-setup.exe".into(), sha256: SHA.into() };
        for bad in [
            InstallerAsset { url: "https://evil.example/A_x64-setup.exe".into(), ..ok.clone() },
            InstallerAsset { name: "../A.exe".into(), ..ok.clone() },
            InstallerAsset { sha256: "abc".into(), ..ok.clone() },
            InstallerAsset { size: MAX_INSTALLER_BYTES + 1, ..ok.clone() },
        ] {
            assert!(matches!(download(&g, &bad, d.path(), |_| {}, &c).await, Err(AppError::Blocked(_))));
        }
        assert!(g.audit_log().is_empty(), "검사에서 걸린 요청은 네트워크에 나가지 않아야 함");
    }

    #[tokio::test]
    async fn check_requires_consent() {
        let g = NetGuard::new();
        assert!(matches!(check(&g, "0.1.0", true).await, Err(AppError::ConsentRequired(_))));
    }
}
