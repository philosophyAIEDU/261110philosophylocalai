//! 엔진 재설치 전에도 앱 모델 저장소에 남아 있는 완전한 모델을 찾는다.
use super::InstalledModel;
use crate::models::Catalog;
use std::path::Path;

pub fn discover(root: &Path, catalog: &Catalog) -> Vec<InstalledModel> {
    catalog.allowed_ids().iter().filter_map(|name| {
        let (model, tag) = name.split_once(':')?;
        // 카탈로그 이름만 사용하며 저장소 밖의 경로는 허용하지 않는다.
        if [model, tag].iter().any(|s| s.is_empty() || s.contains('/') || s.contains('\\') || *s == "..") {
            return None;
        }
        let path = root.join("manifests/registry.ollama.ai/library").join(model).join(tag);
        let manifest: serde_json::Value = serde_json::from_slice(&std::fs::read(path).ok()?).ok()?;
        let config = &manifest["config"];
        let layers = manifest["layers"].as_array()?;
        if layers.is_empty() { return None; }
        let mut size_bytes = 0u64;
        for part in std::iter::once(config).chain(layers.iter()) {
            let digest = part["digest"].as_str()?.strip_prefix("sha256:")?;
            if digest.len() != 64 || !digest.bytes().all(|b| b.is_ascii_hexdigit()) { return None; }
            let blob = root.join("blobs").join(format!("sha256-{digest}"));
            let expected = part["size"].as_u64()?;
            if expected == 0 || std::fs::metadata(&blob).ok()?.len() != expected { return None; }
            size_bytes = size_bytes.checked_add(expected)?;
            if std::ptr::eq(part, config) {
                let value: serde_json::Value = serde_json::from_slice(&std::fs::read(blob).ok()?).ok()?;
                if ["remote_host", "remote_model"].iter().any(|k| value[*k].as_str().is_some_and(|s| !s.is_empty())) { return None; }
            }
        }
        Some(InstalledModel { name: name.clone(), size_bytes, modified_at: String::new(), parameter_size: None, quantization: None, is_remote: false })
    }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_retained_models_without_engine_but_rejects_partial_and_remote_files() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let manifests = root.join("manifests/registry.ollama.ai/library/gemma3");
        std::fs::create_dir_all(&manifests).unwrap();
        std::fs::create_dir(root.join("blobs")).unwrap();
        let config = root.join(format!("blobs/sha256-{}", "a".repeat(64)));
        let layer = root.join(format!("blobs/sha256-{}", "b".repeat(64)));
        std::fs::write(&config, b"{}").unwrap();
        std::fs::write(&layer, b"model").unwrap();
        let mut manifest = serde_json::json!({"config": {"digest": format!("sha256:{}", "a".repeat(64)), "size": 2}, "layers": [{"digest": format!("sha256:{}", "b".repeat(64)), "size": 5}]});
        let save = |m: &serde_json::Value| std::fs::write(manifests.join("4b"), m.to_string()).unwrap();
        save(&manifest);
        assert_eq!(discover(root, &Catalog::builtin())[0].name, "gemma3:4b");
        std::fs::write(&layer, b"part").unwrap();
        assert!(discover(root, &Catalog::builtin()).is_empty());
        std::fs::write(&layer, b"model").unwrap();
        let remote = br#"{"remote_host":"https://example.com"}"#;
        std::fs::write(&config, remote).unwrap();
        manifest["config"]["size"] = remote.len().into();
        save(&manifest);
        assert!(discover(root, &Catalog::builtin()).is_empty());
        manifest["config"]["digest"] = "sha256:../../outside".into();
        save(&manifest);
        assert!(discover(root, &Catalog::builtin()).is_empty());
    }
}
