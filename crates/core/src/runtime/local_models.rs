//! AI 실행 엔진이 아직 없거나 멈춰 있어도, 앱 모델 저장소에 남아 있는 완전한 모델을 찾는다.
//! (앱을 다시 설치한 뒤 같은 AI를 또 내려받지 않게 하기 위함)
//!
//! Ollama 저장소 구조 (0.40.x 기준, 엔진 소스 `manifest/paths.go`·`manifest.go` 확인)
//! - 새 구조: `manifests-v2/ollama.com/library/<모델>/<태그>`
//!   → 매니페스트 내용 자체가 `blobs/sha256-<해시>`에 저장되고, 이 경로는 그 파일을 가리키는
//!     심볼릭 링크(macOS·Linux)이거나 복사본(Windows)이다. 새로 내려받으면 옛 경로는 지워진다.
//! - 옛 구조: `manifests/registry.ollama.ai/library/<모델>/<태그>` (일반 파일)
//! - 매니페스트는 단일 매니페스트이거나, 여러 매니페스트를 가리키는 "매니페스트 목록"일 수 있다.
//!
//! 모든 조각(blob)이 기록된 크기 그대로 있을 때만 "있음"으로 본다. 경로는 카탈로그 이름으로만
//! 만들고, 링크가 저장소 밖을 가리키면 무시한다.
use super::InstalledModel;
use crate::models::Catalog;
use serde_json::Value;
use std::path::{Path, PathBuf};

const MEDIA_TYPE_MANIFEST_LIST: &str = "application/vnd.ollama.manifest.list.v2+json";
/// 매니페스트 파일 최대 크기 (비정상 파일 방지)
const MAX_MANIFEST_BYTES: u64 = 1024 * 1024;

fn valid_digest(d: &str) -> Option<&str> {
    let hex = d.strip_prefix("sha256:").or_else(|| d.strip_prefix("sha256-"))?;
    (hex.len() == 64 && hex.bytes().all(|b| b.is_ascii_hexdigit())).then_some(hex)
}

fn blob_path(root: &Path, digest: &str) -> Option<PathBuf> {
    Some(root.join("blobs").join(format!("sha256-{}", valid_digest(digest)?.to_ascii_lowercase())))
}

/// 저장소 안의 파일만 읽는다 (링크가 저장소 밖을 가리키면 거부).
fn read_inside(root_canon: &Path, path: &Path) -> Option<Vec<u8>> {
    let real = std::fs::canonicalize(path).ok()?;
    if !real.starts_with(root_canon) {
        return None;
    }
    if std::fs::metadata(&real).ok()?.len() > MAX_MANIFEST_BYTES {
        return None;
    }
    std::fs::read(real).ok()
}

/// 단일 매니페스트의 모든 조각이 온전히 있으면 전체 크기를 돌려준다.
fn complete_manifest_size(root: &Path, root_canon: &Path, m: &Value) -> Option<u64> {
    let layers = m["layers"].as_array()?;
    if layers.is_empty() {
        return None;
    }
    let config = &m["config"];
    let mut total = 0u64;
    for part in std::iter::once(config).chain(layers.iter()) {
        let blob = blob_path(root, part["digest"].as_str()?)?;
        let expected = part["size"].as_u64()?;
        if expected == 0 || std::fs::metadata(&blob).ok()?.len() != expected {
            return None;
        }
        total = total.checked_add(expected)?;
    }
    // 원격(클라우드) 모델 표시가 있으면 로컬 모델이 아니다.
    let cfg: Value = serde_json::from_slice(&read_inside(root_canon, &blob_path(root, config["digest"].as_str()?)?)?).ok()?;
    if ["remote_host", "remote_model"].iter().any(|k| cfg[*k].as_str().is_some_and(|s| !s.is_empty())) {
        return None;
    }
    Some(total)
}

/// 매니페스트(또는 매니페스트 목록)가 실행 가능한 완전한 모델을 가리키면 크기를 돌려준다.
fn complete_size(root: &Path, root_canon: &Path, m: &Value) -> Option<u64> {
    if m["mediaType"].as_str() == Some(MEDIA_TYPE_MANIFEST_LIST) {
        // 목록 안의 매니페스트 중 하나라도 온전하면 쓸 수 있다.
        return m["manifests"].as_array()?.iter().find_map(|entry| {
            let blob = blob_path(root, entry["digest"].as_str()?)?;
            let child: Value = serde_json::from_slice(&read_inside(root_canon, &blob)?).ok()?;
            complete_manifest_size(root, root_canon, &child)
        });
    }
    complete_manifest_size(root, root_canon, m)
}

pub fn discover(root: &Path, catalog: &Catalog) -> Vec<InstalledModel> {
    let Ok(root_canon) = std::fs::canonicalize(root) else { return Vec::new() };
    catalog
        .allowed_ids()
        .iter()
        .filter_map(|name| {
            let (model, tag) = name.split_once(':')?;
            // 카탈로그 이름만 사용하며 저장소 밖의 경로는 허용하지 않는다.
            if [model, tag].iter().any(|s| s.is_empty() || s.contains('/') || s.contains('\\') || s.contains("..")) {
                return None;
            }
            let candidates = [
                root.join("manifests-v2").join("ollama.com").join("library").join(model).join(tag),
                root.join("manifests").join("registry.ollama.ai").join("library").join(model).join(tag),
            ];
            let size_bytes = candidates.iter().find_map(|p| {
                let m: Value = serde_json::from_slice(&read_inside(&root_canon, p)?).ok()?;
                complete_size(root, &root_canon, &m)
            })?;
            Some(InstalledModel { name: name.clone(), size_bytes, modified_at: String::new(), parameter_size: None, quantization: None, is_remote: false })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use sha2::{Digest, Sha256};

    fn put_blob(root: &Path, data: &[u8]) -> (String, u64) {
        let hex = hex::encode(Sha256::digest(data));
        std::fs::create_dir_all(root.join("blobs")).unwrap();
        std::fs::write(root.join("blobs").join(format!("sha256-{hex}")), data).unwrap();
        (format!("sha256:{hex}"), data.len() as u64)
    }

    fn manifest(root: &Path, config: &[u8], layer: &[u8]) -> Value {
        let (cd, cs) = put_blob(root, config);
        let (ld, ls) = put_blob(root, layer);
        serde_json::json!({"schemaVersion": 2, "mediaType": "application/vnd.docker.distribution.manifest.v2+json",
            "config": {"digest": cd, "size": cs}, "layers": [{"digest": ld, "size": ls}]})
    }

    /// Ollama 0.40.x가 내려받을 때와 같은 방식: 매니페스트를 blob으로 저장하고 v2 경로에서 링크(또는 복사).
    fn link_v2(root: &Path, model: &str, tag: &str, data: &[u8], copy: bool) {
        let (d, _) = put_blob(root, data);
        let dir = root.join("manifests-v2/ollama.com/library").join(model);
        std::fs::create_dir_all(&dir).unwrap();
        let blob = root.join("blobs").join(format!("sha256-{}", valid_digest(&d).unwrap()));
        #[cfg(unix)]
        if !copy {
            std::os::unix::fs::symlink(Path::new("../../../../blobs").join(blob.file_name().unwrap()), dir.join(tag)).unwrap();
            return;
        }
        let _ = copy;
        std::fs::copy(blob, dir.join(tag)).unwrap();
    }

    #[test]
    fn finds_models_in_v2_layout_written_by_current_engine() {
        for copy in [false, true] {
            let dir = tempfile::tempdir().unwrap();
            let root = dir.path();
            let m = manifest(root, br#"{"model_format":"gguf"}"#, b"weights");
            link_v2(root, "gemma3", "4b", m.to_string().as_bytes(), copy);
            let found = discover(root, &Catalog::builtin());
            assert_eq!(found.len(), 1, "copy={copy}");
            assert_eq!(found[0].name, "gemma3:4b");
            assert_eq!(found[0].size_bytes, br#"{"model_format":"gguf"}"#.len() as u64 + 7);
        }
    }

    #[test]
    fn finds_models_behind_manifest_list() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let child = manifest(root, b"{}", b"weights");
        let (cd, cs) = put_blob(root, child.to_string().as_bytes());
        let list = serde_json::json!({"schemaVersion": 2, "mediaType": MEDIA_TYPE_MANIFEST_LIST,
            "manifests": [{"mediaType": "application/vnd.docker.distribution.manifest.v2+json", "digest": cd, "size": cs}]});
        link_v2(root, "qwen3", "8b", list.to_string().as_bytes(), false);
        assert_eq!(discover(root, &Catalog::builtin())[0].name, "qwen3:8b");
    }

    #[test]
    fn finds_legacy_layout_but_rejects_partial_remote_and_outside_files() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let legacy = root.join("manifests/registry.ollama.ai/library/gemma3");
        std::fs::create_dir_all(&legacy).unwrap();
        let mut m = manifest(root, b"{}", b"model");
        std::fs::write(legacy.join("4b"), m.to_string()).unwrap();
        assert_eq!(discover(root, &Catalog::builtin())[0].name, "gemma3:4b");
        // 덜 받은 조각
        let layer = root.join("blobs").join(format!("sha256-{}", valid_digest(m["layers"][0]["digest"].as_str().unwrap()).unwrap()));
        std::fs::write(&layer, b"part").unwrap();
        assert!(discover(root, &Catalog::builtin()).is_empty());
        std::fs::write(&layer, b"model").unwrap();
        // 원격 모델
        let remote = br#"{"remote_host":"https://example.com"}"#;
        let (rd, rs) = put_blob(root, remote);
        m["config"] = serde_json::json!({"digest": rd, "size": rs});
        std::fs::write(legacy.join("4b"), m.to_string()).unwrap();
        assert!(discover(root, &Catalog::builtin()).is_empty());
        // 경로 탈출 시도
        m["config"]["digest"] = "sha256:../../outside".into();
        std::fs::write(legacy.join("4b"), m.to_string()).unwrap();
        assert!(discover(root, &Catalog::builtin()).is_empty());
    }

    #[cfg(unix)]
    #[test]
    fn ignores_links_pointing_outside_the_store() {
        let outside = tempfile::tempdir().unwrap();
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let m = manifest(root, b"{}", b"w");
        std::fs::write(outside.path().join("evil"), m.to_string()).unwrap();
        let v2 = root.join("manifests-v2/ollama.com/library/gemma3");
        std::fs::create_dir_all(&v2).unwrap();
        std::os::unix::fs::symlink(outside.path().join("evil"), v2.join("4b")).unwrap();
        assert!(discover(root, &Catalog::builtin()).is_empty());
    }

    #[test]
    fn missing_store_is_empty() {
        assert!(discover(Path::new("/nonexistent/pla-models"), &Catalog::builtin()).is_empty());
    }
}
