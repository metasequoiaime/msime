//! 面向脚本的只读预检；与实际应用共用补丁合并和业务校验。

use super::{PreferencesError, PreferencesSnapshot, PreferencesStore};
use serde_json::Value;

impl PreferencesStore {
    /// 读取一个原子快照并预检 JSON Merge Patch，不创建目录或锁文件，也不修复坏文档。
    /// 应用时仍须传入预检返回的 revision，由锁内事务检测期间发生的修改。
    pub fn preview_patch(&self, patch: &Value) -> Result<PreferencesSnapshot, PreferencesError> {
        let (current, unknown) = self.read_locked_keeping_unknown()?;
        let (preferences, _, _) = super::patch::prepare_patch(&current, &unknown, patch)?;
        Ok(PreferencesSnapshot {
            preferences,
            ..current
        })
    }

    /// 只读取得当前修订号；不恢复损坏文件、不创建存储目录。
    pub fn document_revision(&self) -> Result<u64, PreferencesError> {
        self.read_locked_keeping_unknown()
            .map(|(snapshot, _)| snapshot.revision)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    #[test]
    fn preview_of_a_new_store_does_not_create_it_and_apply_matches_the_preview() {
        let root = tempfile::tempdir().unwrap();
        let directory = root.path().join("new-store");
        let store = PreferencesStore::new(&directory);
        let patch = json!({"candidate_page_size": 7, "sentence_association": {"sentence_collocation_weight": 2.0}});
        let preview = store.preview_patch(&patch).unwrap();
        assert_eq!(preview.revision, 0);
        assert_eq!(preview.preferences.candidate_page_size, 7);
        assert!(!directory.exists());
        let saved = store.apply_patch(preview.revision, &patch).unwrap();
        assert_eq!(saved.preferences, preview.preferences);
        assert!(saved.revision > preview.revision);
    }

    #[test]
    fn invalid_patch_and_existing_unknown_fields_have_the_same_preview_and_apply_rules() {
        let root = tempfile::tempdir().unwrap();
        let store = PreferencesStore::new(root.path());
        let snapshot = store
            .apply_patch(0, &json!({"candidate_page_size": 6}))
            .unwrap();
        let file = root.path().join("preferences.json");
        let mut document: Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
        document["preferences"]["future_synthetic_field"] = json!("synthetic-value");
        std::fs::write(&file, serde_json::to_vec(&document).unwrap()).unwrap();
        let before = std::fs::read(&file).unwrap();
        for patch in [
            json!([]),
            json!({"candidate_page_size": 99}),
            json!({"sentence_association": {"neural_keybord": true}}),
            json!({"future_synthetic_field": "changed"}),
        ] {
            assert!(store.preview_patch(&patch).is_err());
            assert!(store.apply_patch(snapshot.revision, &patch).is_err());
            assert_eq!(std::fs::read(&file).unwrap(), before);
        }
        let patch = json!({"candidate_page_size": 8});
        assert!(store.preview_patch(&patch).is_ok());
        assert_eq!(std::fs::read(&file).unwrap(), before);
        store.apply_patch(snapshot.revision, &patch).unwrap();
        let after: Value = serde_json::from_slice(&std::fs::read(file).unwrap()).unwrap();
        assert_eq!(
            after["preferences"]["future_synthetic_field"],
            "synthetic-value"
        );
    }

    #[test]
    fn revision_changed_after_preview_refuses_apply_without_overwriting() {
        let root = tempfile::tempdir().unwrap();
        let store = PreferencesStore::new(root.path());
        let patch = json!({"candidate_page_size": 7});
        let preview = store.preview_patch(&patch).unwrap();
        store
            .apply_patch(0, &json!({"candidate_page_size": 8}))
            .unwrap();
        assert!(matches!(
            store.apply_patch(preview.revision, &patch),
            Err(PreferencesError::Conflict)
        ));
        assert_eq!(store.load().unwrap().preferences.candidate_page_size, 8);
    }
}
