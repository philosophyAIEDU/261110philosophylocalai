//! 개발용 도구: 실제 앱 수동 시험을 위해 "앱 잠금 + 암호화된 상담 기록"이 있는 자료 폴더를 만든다.
//! 실행: PLA_FIXTURE_DIR=/경로 cargo test -p pla-core --test dev_fixture -- --ignored
use pla_core::security::keystore::{KdfParams, KeyStore};
use pla_core::storage::Store;

#[test]
#[ignore]
fn make_locked_profile() {
    let dir = std::path::PathBuf::from(std::env::var("PLA_FIXTURE_DIR").expect("PLA_FIXTURE_DIR"));
    std::fs::create_dir_all(&dir).unwrap();
    let ks = KeyStore::file(dir.join("data.key"), KdfParams::default());
    let cipher = ks.create_plain().unwrap();
    let store = Store::open(&dir.join("app.db"), cipher.clone()).unwrap();
    store.add_student_note("학생 A", "2026-11-12", "FIXTURE-NOTE 진로 상담: 대학원 진학 고민").unwrap();
    store.set_setting("onboarding_done", "true").unwrap();
    store.set_setting("has_passcode", "true").unwrap();
    store.set_setting("auto_lock_minutes", &std::env::var("PLA_FIXTURE_AUTOLOCK").unwrap_or_else(|_| "10".into())).unwrap();
    ks.set_passcode(&cipher, "Lab-coffee-3pm").unwrap();
}
