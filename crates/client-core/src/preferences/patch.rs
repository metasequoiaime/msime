//! 偏好文档的局部事务；复用完整保存的校验、迁移和原子写入。

use super::{
    deserialize_keeping_unknown, document_for_disk, newer_list_values, Preferences,
    PreferencesError, PreferencesSnapshot, PreferencesStore, UnknownPreferenceField,
};

impl PreferencesStore {
    /// 在偏好锁内合并局部字段。`null` 删除键，数组整体替换；未知键只能沿用盘上已有的值。
    pub fn apply_patch(
        &self,
        expected_revision: u64,
        patch: &serde_json::Value,
    ) -> Result<PreferencesSnapshot, PreferencesError> {
        if !patch.is_object() {
            return Err(PreferencesError::InvalidPatch);
        }
        let _lock = self.lock()?;
        let (current, unknown) = self.read_locked_keeping_unknown()?;
        if current.revision != expected_revision {
            return Err(PreferencesError::Conflict);
        }
        let (next, retained_unknown, removed_unknown) = prepare_patch(&current, &unknown, patch)?;
        self.save_locked(
            current,
            &retained_unknown,
            expected_revision,
            next,
            false,
            removed_unknown,
        )
    }
}

/// 预检和锁内应用共用的规则；这里只构造结果，不接触磁盘。
pub(super) fn prepare_patch(
    current: &PreferencesSnapshot,
    unknown: &[UnknownPreferenceField],
    patch: &serde_json::Value,
) -> Result<(Preferences, Vec<UnknownPreferenceField>, bool), PreferencesError> {
    if !patch.is_object() {
        return Err(PreferencesError::InvalidPatch);
    }
    let mut document = document_for_disk(current, unknown)?;
    let preferences = document
        .get_mut("preferences")
        .ok_or(PreferencesError::InvalidPatch)?;
    let (_, original_unknown) = deserialize_keeping_unknown::<Preferences>(preferences, &[])?;
    merge_preferences_patch(preferences, patch);
    let (next, patched_unknown) = deserialize_keeping_unknown::<Preferences>(preferences, &[])?;
    if patched_unknown != original_unknown {
        return Err(PreferencesError::InvalidPatch);
    }
    if newer_list_values(preferences)
        .iter()
        .any(|field| patch_replaces_path(patch, &field.path))
    {
        return Err(PreferencesError::InvalidPatch);
    }
    next.validate()?;
    let retained_unknown: Vec<UnknownPreferenceField> = unknown
        .iter()
        .filter(|field| !field.elements || !patch_replaces_path(patch, &field.path))
        .cloned()
        .collect();
    let removed_unknown = retained_unknown.len() != unknown.len();
    Ok((next, retained_unknown, removed_unknown))
}

/// JSON Merge Patch 的对象递归合并规则。`null` 删除成员，其他值替换成员。
fn merge_preferences_patch(target: &mut serde_json::Value, patch: &serde_json::Value) {
    let serde_json::Value::Object(changes) = patch else {
        *target = patch.clone();
        return;
    };
    if !target.is_object() {
        *target = serde_json::json!({});
    }
    let fields = target.as_object_mut().expect("target is an object");
    for (key, value) in changes {
        if value.is_null() {
            fields.remove(key);
        } else {
            merge_preferences_patch(
                fields.entry(key.clone()).or_insert(serde_json::Value::Null),
                value,
            );
        }
    }
}

/// 数组补丁替换整个列表，不能把旧版本不认识的列表元素再接回去。
fn patch_replaces_path(patch: &serde_json::Value, path: &[String]) -> bool {
    let mut node = patch;
    for segment in path {
        let serde_json::Value::Object(fields) = node else {
            return true;
        };
        let Some(next) = fields.get(segment) else {
            return false;
        };
        node = next;
    }
    !node.is_object()
}
