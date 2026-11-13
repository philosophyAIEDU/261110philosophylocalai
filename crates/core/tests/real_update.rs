//! 실제 GitHub 릴리스로 하는 업데이트 확인·내려받기 테스트 (인터넷 필요, 기본으로 건너뜀).
//! 실행: PLA_REAL_UPDATE=1 cargo test -p pla-core --test real_update -- --nocapture
use pla_core::security::NetGuard;
use pla_core::update;
use tokio_util::sync::CancellationToken;

#[tokio::test]
async fn real_github_check_and_verified_download() {
    if std::env::var("PLA_REAL_UPDATE").is_err() {
        eprintln!("PLA_REAL_UPDATE 미설정 — 건너뜁니다");
        return;
    }
    let g = NetGuard::new();
    g.set_update_consent(true);
    // 아주 오래된 버전으로 가정 → 공개된 최신 릴리스를 찾아야 한다
    let info = update::check_for_platform(&g, "0.0.1", true, "windows", "x86_64").await.expect("업데이트 확인");
    eprintln!("latest={:?} prerelease={} asset={:?} note={:?}", info.latest_version, info.is_prerelease, info.asset.as_ref().map(|a| (&a.name, a.size)), info.asset_note);
    assert!(info.update_available);
    let asset = info.asset.expect("Windows 설치 파일과 SHA-256이 있어야 함");
    let dir = tempfile::tempdir().unwrap();
    let mut last = 0;
    let path = update::download(&g, &asset, dir.path(), |p| last = p.completed_bytes, &CancellationToken::new()).await.expect("내려받기·무결성 확인");
    assert_eq!(std::fs::metadata(&path).unwrap().len(), asset.size);
    assert_eq!(last, asset.size);
    eprintln!("verified {} ({} bytes)", path.display(), asset.size);
    // 감사 기록: 업데이트 확인·내려받기만 있고, 대상은 GitHub
    for a in g.audit_log() {
        eprintln!("audit: {:?} {} {}", a.purpose, a.host, a.detail);
        assert!(a.host.ends_with("github.com"));
    }
    // 이미 최신이면 업데이트 없음
    let same = update::check_for_platform(&g, info.latest_version.as_deref().unwrap(), true, "windows", "x86_64").await.unwrap();
    assert!(!same.update_available);
}
