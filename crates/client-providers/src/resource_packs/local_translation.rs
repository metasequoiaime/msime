//! 编译期固定的翻译权重，复用资源包的下载、离线导入、取消和摘要校验。

use crate::resources::ResourceSet;
use std::sync::OnceLock;

/// 冷加载权重前核验固定摘要；每个读取块都检查取消，不加载清单之外的路径。
pub fn verified_model(
    state_root: &std::path::Path,
    cancelled: &dyn Fn() -> bool,
) -> Result<std::path::PathBuf, super::LocalModelError> {
    if cancelled() {
        return Err(super::LocalModelError::Cancelled);
    }
    let pack = super::ResourcePack::LocalTranslation;
    let name = &pack.set().artifacts[0].name;
    let path =
        super::installed_file(state_root, pack, name).ok_or(super::LocalModelError::InvalidRoot)?;
    let directory = path.parent().ok_or(super::LocalModelError::InvalidRoot)?;
    super::verify_artifacts_with_cancel(directory, pack.set(), cancelled)?;
    Ok(path)
}

pub(super) fn set() -> &'static ResourceSet {
    static SET: OnceLock<ResourceSet> = OnceLock::new();
    SET.get_or_init(|| {
        let set: ResourceSet = serde_json::from_str(include_str!(
            "../../../../resources/local-translation-model.lock.json"
        ))
        .expect("local translation model lock is valid");
        set.validate()
            .expect("local translation model artifacts are valid");
        set
    })
}

#[cfg(test)]
mod tests {
    use super::super::*;

    #[test]
    fn verification_stops_during_hashing() {
        let root = tempfile::tempdir().unwrap();
        let bytes = vec![7u8; 32_768];
        fs::write(root.path().join("synthetic.gguf"), &bytes).unwrap();
        let set = ResourceSet {
            source_commit: "a".repeat(40),
            artifacts: vec![crate::resources::Artifact {
                name: "synthetic.gguf".into(),
                url: "https://example.invalid/synthetic.gguf".into(),
                size: bytes.len() as u64,
                sha256: hex::encode(Sha256::digest(&bytes)),
            }],
        };
        let calls = std::cell::Cell::new(0);
        assert!(matches!(
            verify_artifacts_with_cancel(root.path(), &set, &|| {
                calls.set(calls.get() + 1);
                calls.get() > 2
            }),
            Err(LocalModelError::Cancelled)
        ));
        assert_eq!(calls.get(), 3);
        verify_artifacts(root.path(), &set).unwrap();
    }

    #[test]
    fn translation_pack_is_missing_until_verified_import() {
        let root = tempfile::tempdir().unwrap();
        let pack = ResourcePack::LocalTranslation;
        assert_eq!(ResourcePack::from_id("local-translation"), Some(pack));
        assert_eq!(pack.size(), 1_133_080_448);
        assert!(pack.schemes().is_empty());
        assert!(verify_installed(root.path(), pack).is_err());
        let file = root.path().join("synthetic.gguf");
        fs::write(&file, b"GGUFsynthetic untrusted weights").unwrap();
        assert!(import(
            root.path(),
            pack,
            &[file],
            &mut |_| {},
            &AtomicBool::new(false)
        )
        .is_err());
        assert!(verify_installed(root.path(), pack).is_err());
        assert!(installed_file(root.path(), pack, &pack.set().artifacts[0].name).is_none());
    }
}
