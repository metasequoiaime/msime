//! 真实注册回调与目录包装的学习密钥行为，保持旧宿主和匿名账号边界。

use super::*;
use crate::ffi::session_store::tests::{leaked_store, registered, unregister};

fn host(entry: &StoreEntry, slot: HostSessionSlot) -> SessionStorage {
    SessionStorage::Host(HostSessionStorage::new(
        entry.backend.clone(),
        slot,
        Arc::new(HostStoreLocks::new()),
    ))
}

#[test]
fn legacy_registration_never_calls_the_new_item_and_still_clears_the_account() {
    let store = leaked_store();
    let id = registered(store, SESSION_STORE_SHARED, 0);
    let entry = lookup(id).unwrap();
    let account = host(&entry, HostSessionSlot::Account);
    let learning = SessionStoreItem::LearningKeystore;
    assert!(!entry.backend.supports_learning_keystore());
    let loads = store.loads.load(Ordering::SeqCst);
    assert_eq!(account.load_learning_keystore(), Err(AccountError::Storage));
    assert_eq!(
        account.save_learning_keystore("synthetic"),
        Err(AccountError::Storage)
    );
    // 即使绕过上层直接用回调包装，也不能把未知项发送给旧宿主。
    assert_eq!(
        entry.backend.read(learning, learning.capacity()),
        Err(AccountError::Storage)
    );
    assert_eq!(
        entry.backend.write(learning, b"synthetic"),
        Err(AccountError::Storage)
    );
    assert_eq!(entry.backend.remove(learning), Err(AccountError::Storage));
    assert_eq!(store.loads.load(Ordering::SeqCst), loads);
    assert!(store.saves.lock().unwrap().is_empty());
    assert!(store.removes.lock().unwrap().is_empty());
    account.clear().unwrap();
    assert_eq!(*store.removes.lock().unwrap(), [1, 2]);
    drop(account);
    drop(entry);
    unregister(id);
    assert_eq!(store.releases.load(Ordering::SeqCst), 1);
}

#[test]
fn learning_flag_is_independent_of_shared_and_layout_and_preserves_existing_items() {
    for shared in [0, SESSION_STORE_SHARED] {
        for layout in [0, 1] {
            let store = leaked_store();
            let id = registered(store, shared | SESSION_STORE_LEARNING_KEYSTORE, layout);
            let entry = lookup(id).unwrap();
            assert!(entry.backend.supports_learning_keystore());
            let account = host(&entry, HostSessionSlot::Account);
            account
                .with_refresh_lock(|| {
                    account.save_clipboard_keystore("synthetic-clipboard")?;
                    account.save_learning_keystore("synthetic-learning-long\r\n")?;
                    assert_eq!(
                        account.load_learning_keystore()?.as_deref(),
                        Some("synthetic-learning-long")
                    );
                    account.save_learning_keystore("short")
                })
                .unwrap();
            assert_eq!(
                account.load_learning_keystore().unwrap().as_deref(),
                Some("short")
            );
            assert_eq!(
                account.load_clipboard_keystore().unwrap().as_deref(),
                Some("synthetic-clipboard")
            );
            assert_eq!(*store.saves.lock().unwrap(), [2, 5, 5]);
            assert_eq!(store.locks.lock().unwrap().len(), usize::from(shared != 0));
            for (item, _, _) in store.locks.lock().unwrap().iter() {
                assert_eq!(*item, 1);
            }
            assert_eq!(*store.locks.lock().unwrap(), *store.unlocks.lock().unwrap());
            // 原有匿名身份仍是只创建，不受学习项可替换语义影响。
            assert_eq!(
                entry
                    .backend
                    .write(SessionStoreItem::AnonymousIdentity, b"first"),
                Ok(WriteOutcome::Written)
            );
            assert_eq!(
                entry
                    .backend
                    .write(SessionStoreItem::AnonymousIdentity, b"second"),
                Ok(WriteOutcome::AlreadyExists)
            );
            account.clear().unwrap();
            assert_eq!(*store.removes.lock().unwrap(), [1, 2, 5]);
            assert_eq!(store.items.lock().unwrap().get(&4).unwrap(), b"first");
            drop(account);
            drop(entry);
            unregister(id);
            assert_eq!(store.releases.load(Ordering::SeqCst), 1);
        }
    }
}

#[test]
fn registered_anonymous_slot_cannot_access_or_clear_the_learning_key() {
    let store = leaked_store();
    let id = registered(store, SESSION_STORE_LEARNING_KEYSTORE, 0);
    let entry = lookup(id).unwrap();
    let account = host(&entry, HostSessionSlot::Account);
    let anonymous = host(&entry, HostSessionSlot::Anonymous);
    account
        .save_learning_keystore("synthetic-learning")
        .unwrap();
    assert_eq!(
        anonymous.load_learning_keystore(),
        Err(AccountError::Storage)
    );
    assert_eq!(
        anonymous.save_learning_keystore("other"),
        Err(AccountError::Storage)
    );
    anonymous.clear().unwrap();
    assert_eq!(*store.removes.lock().unwrap(), [3]);
    assert_eq!(
        account.load_learning_keystore().unwrap().as_deref(),
        Some("synthetic-learning")
    );
    drop((account, anonymous, entry));
    unregister(id);
}

#[test]
fn registered_learning_slot_rejects_oversized_and_invalid_host_responses() {
    let store = leaked_store();
    let id = registered(store, SESSION_STORE_LEARNING_KEYSTORE, 0);
    let entry = lookup(id).unwrap();
    let account = host(&entry, HostSessionSlot::Account);
    let capacity = SessionStoreItem::LearningKeystore.capacity();
    account
        .save_learning_keystore(&"x".repeat(capacity))
        .unwrap();
    assert_eq!(
        account.load_learning_keystore().unwrap().unwrap().len(),
        capacity
    );
    *store.oversized.lock().unwrap() = Some(5);
    assert_eq!(account.load_learning_keystore(), Err(AccountError::Storage));
    *store.oversized.lock().unwrap() = None;
    store.items.lock().unwrap().insert(5, vec![0xff]);
    assert_eq!(account.load_learning_keystore(), Err(AccountError::Storage));
    *store.save_result.lock().unwrap() = Some(1);
    assert_eq!(
        account.save_learning_keystore("synthetic"),
        Err(AccountError::Storage)
    );
    drop((account, entry));
    unregister(id);
}

#[test]
fn directory_wrapper_forwards_learning_and_anonymous_wrapper_leaves_it_alone() {
    let root = tempfile::tempdir().unwrap();
    let account = SessionStorage::File(FileAccountSessionStorage::new(
        root.path(),
        AccountSessionFileLayout::Native,
    ));
    let anonymous = SessionStorage::AnonymousFile(LockedAnonymousSessionStorage::new(root.path()));
    assert_eq!(account.load_learning_keystore().unwrap(), None);
    account
        .with_refresh_lock(|| {
            account.save_clipboard_keystore("synthetic-clipboard")?;
            account.save_learning_keystore("synthetic-learning")
        })
        .unwrap();
    assert_eq!(
        account.load_learning_keystore().unwrap().as_deref(),
        Some("synthetic-learning")
    );
    assert_eq!(
        anonymous.load_learning_keystore(),
        Err(AccountError::Storage)
    );
    assert_eq!(
        anonymous.save_learning_keystore("other"),
        Err(AccountError::Storage)
    );
    anonymous.clear().unwrap();
    assert_eq!(
        account.load_learning_keystore().unwrap().as_deref(),
        Some("synthetic-learning")
    );
    account.clear().unwrap();
    assert_eq!(account.load_learning_keystore().unwrap(), None);
    assert_eq!(account.load_clipboard_keystore().unwrap(), None);
}
