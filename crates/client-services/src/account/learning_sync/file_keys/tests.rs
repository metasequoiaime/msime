//! learning 密钥文件与剪贴板隔离，沿用账号私有文件的边界。

use super::*;

fn storage(directory: &Path) -> FileAccountSessionStorage {
    FileAccountSessionStorage::new(directory, AccountSessionFileLayout::Native)
}

#[test]
fn independent_keys_are_replaced_whole_and_both_cleared_on_logout() {
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join("account");
    let store = storage(&directory);
    assert_eq!(store.load_learning_keystore().unwrap(), None);
    assert!(!directory.exists());
    store
        .save_clipboard_keystore("synthetic-clipboard")
        .unwrap();
    store
        .save_learning_keystore("synthetic-learning-long\r\n")
        .unwrap();
    assert_eq!(
        store.load_learning_keystore().unwrap().as_deref(),
        Some("synthetic-learning-long")
    );
    store.save_learning_keystore("short").unwrap();
    assert_eq!(
        store.load_learning_keystore().unwrap().as_deref(),
        Some("short")
    );
    assert_eq!(
        store.load_clipboard_keystore().unwrap().as_deref(),
        Some("synthetic-clipboard")
    );
    store.clear().unwrap();
    assert_eq!(store.load_learning_keystore().unwrap(), None);
    assert_eq!(store.load_clipboard_keystore().unwrap(), None);
}

#[test]
fn invalid_and_oversized_values_cannot_replace_a_key() {
    let root = tempfile::tempdir().unwrap();
    let store = storage(root.path());
    store.save_learning_keystore("synthetic-existing").unwrap();
    for value in [String::new(), "x".repeat(MAX_KEYSTORE_BYTES as usize + 1)] {
        assert_eq!(
            store.save_learning_keystore(&value),
            Err(AccountError::Storage)
        );
        assert_eq!(
            store.load_learning_keystore().unwrap().as_deref(),
            Some("synthetic-existing")
        );
    }
    let path = root.path().join(LEARNING_KEYSTORE_FILE);
    std::fs::write(&path, [0xff]).unwrap();
    assert_eq!(store.load_learning_keystore(), Err(AccountError::Storage));
    std::fs::write(&path, vec![0; MAX_KEYSTORE_BYTES as usize + 1]).unwrap();
    assert_eq!(store.load_learning_keystore(), Err(AccountError::Storage));
}

#[cfg(unix)]
#[test]
fn key_permissions_are_private_and_symlinks_are_not_followed() {
    use std::os::unix::fs::{symlink, PermissionsExt};
    let root = tempfile::tempdir().unwrap();
    let directory = root.path().join("account");
    let store = storage(&directory);
    store.save_learning_keystore("synthetic-private").unwrap();
    let path = directory.join(LEARNING_KEYSTORE_FILE);
    assert_eq!(
        std::fs::metadata(&path).unwrap().permissions().mode() & 0o777,
        0o600
    );
    std::fs::set_permissions(&path, std::fs::Permissions::from_mode(0o644)).unwrap();
    assert_eq!(store.load_learning_keystore(), Err(AccountError::Storage));
    std::fs::remove_file(&path).unwrap();
    let target = root.path().join("synthetic-target");
    std::fs::write(&target, "synthetic-unchanged").unwrap();
    symlink(&target, &path).unwrap();
    assert_eq!(store.load_learning_keystore(), Err(AccountError::Storage));
    // 原子改名替换链接本身，不会写入链接目标。
    store.save_learning_keystore("replacement").unwrap();
    assert_eq!(
        store.load_learning_keystore().unwrap().as_deref(),
        Some("replacement")
    );
    store.clear().unwrap();
    assert_eq!(
        std::fs::read_to_string(target).unwrap(),
        "synthetic-unchanged"
    );
}
