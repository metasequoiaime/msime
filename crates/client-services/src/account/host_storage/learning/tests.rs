//! 宿主能力、密钥隔离、输入边界与退出清理的行为测试。

use super::*;
use crate::account::host_storage::tests::MemoryBackend;

#[derive(Default)]
struct Backend {
    inner: MemoryBackend,
    learning: bool,
    removed: Mutex<Vec<SessionStoreItem>>,
    remove_error: Option<SessionStoreItem>,
    already_exists: bool,
}

impl SessionStoreBackend for Backend {
    fn supports_learning_keystore(&self) -> bool {
        self.learning
    }

    fn read(
        &self,
        item: SessionStoreItem,
        capacity: usize,
    ) -> Result<Option<Vec<u8>>, AccountError> {
        assert!(self.learning || item != SessionStoreItem::LearningKeystore);
        self.inner.read(item, capacity)
    }

    fn write(&self, item: SessionStoreItem, bytes: &[u8]) -> Result<WriteOutcome, AccountError> {
        assert!(self.learning || item != SessionStoreItem::LearningKeystore);
        if self.already_exists && item == SessionStoreItem::LearningKeystore {
            return Ok(WriteOutcome::AlreadyExists);
        }
        self.inner.write(item, bytes)
    }

    fn remove(&self, item: SessionStoreItem) -> Result<(), AccountError> {
        assert!(self.learning || item != SessionStoreItem::LearningKeystore);
        self.removed.lock().unwrap().push(item);
        if self.remove_error == Some(item) {
            return Err(AccountError::Storage);
        }
        self.inner.remove(item)
    }

    fn lock(&self, item: SessionStoreItem, timeout: Duration) -> Result<u64, AccountError> {
        self.inner.lock(item, timeout)
    }

    fn unlock(&self, item: SessionStoreItem, token: u64) {
        self.inner.unlock(item, token);
    }

    fn shared_across_processes(&self) -> bool {
        self.inner.shared
    }

    fn layout(&self) -> AccountSessionFileLayout {
        self.inner.layout()
    }
}

fn stores(backend: &Arc<Backend>) -> (HostSessionStorage, HostSessionStorage) {
    let locks = Arc::new(HostStoreLocks::new());
    (
        HostSessionStorage::new(backend.clone(), HostSessionSlot::Account, locks.clone()),
        HostSessionStorage::new(backend.clone(), HostSessionSlot::Anonymous, locks),
    )
}

#[test]
fn legacy_hosts_reject_learning_without_callbacks_and_keep_logout_working() {
    let backend = Arc::new(Backend::default());
    // 不实现新能力方法的现有存储也保持相同的拒绝行为。
    assert!(!MemoryBackend::default().supports_learning_keystore());
    let (account, _) = stores(&backend);
    account
        .save_clipboard_keystore("synthetic-clipboard")
        .unwrap();
    let reads = *backend.inner.reads.lock().unwrap();
    assert_eq!(account.load_learning_keystore(), Err(AccountError::Storage));
    assert_eq!(
        account.save_learning_keystore("synthetic"),
        Err(AccountError::Storage)
    );
    assert_eq!(*backend.inner.reads.lock().unwrap(), reads);
    assert_eq!(
        account.load_clipboard_keystore().unwrap().as_deref(),
        Some("synthetic-clipboard")
    );
    account.clear().unwrap();
    assert_eq!(
        *backend.removed.lock().unwrap(),
        [
            SessionStoreItem::Account,
            SessionStoreItem::ClipboardKeystore
        ]
    );
}

#[test]
fn independent_learning_key_replaces_whole_and_uses_the_account_refresh_lock() {
    let backend = Arc::new(Backend {
        learning: true,
        inner: MemoryBackend {
            shared: true,
            ..Default::default()
        },
        ..Default::default()
    });
    let (account, _) = stores(&backend);
    account
        .save_clipboard_keystore("synthetic-clipboard")
        .unwrap();
    account
        .with_refresh_lock(|| {
            assert_eq!(account.load_learning_keystore()?, None);
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
    let here = std::thread::current().id();
    assert_eq!(
        *backend.inner.locks.lock().unwrap(),
        [(SessionStoreItem::Account, 100, here)]
    );
    assert_eq!(
        *backend.inner.unlocks.lock().unwrap(),
        *backend.inner.locks.lock().unwrap()
    );
}

#[test]
fn anonymous_storage_cannot_read_write_or_delete_the_learning_key() {
    let backend = Arc::new(Backend {
        learning: true,
        ..Default::default()
    });
    let (account, anonymous) = stores(&backend);
    account
        .save_learning_keystore("synthetic-learning")
        .unwrap();
    let reads = *backend.inner.reads.lock().unwrap();
    assert_eq!(
        anonymous.load_learning_keystore(),
        Err(AccountError::Storage)
    );
    assert_eq!(
        anonymous.save_learning_keystore("synthetic-other"),
        Err(AccountError::Storage)
    );
    assert_eq!(*backend.inner.reads.lock().unwrap(), reads);
    anonymous.clear().unwrap();
    assert_eq!(
        *backend.removed.lock().unwrap(),
        [SessionStoreItem::Anonymous]
    );
    assert_eq!(
        account.load_learning_keystore().unwrap().as_deref(),
        Some("synthetic-learning")
    );
}

#[test]
fn malformed_and_oversized_keys_fail_without_overwriting_existing_data() {
    let backend = Arc::new(Backend {
        learning: true,
        ..Default::default()
    });
    let (account, _) = stores(&backend);
    let capacity = SessionStoreItem::LearningKeystore.capacity();
    assert_eq!(capacity, 4096);
    let boundary = "界".repeat(capacity / 3) + "a";
    assert_eq!(boundary.len(), capacity);
    account.save_learning_keystore(&boundary).unwrap();
    assert_eq!(
        account.load_learning_keystore().unwrap(),
        Some(boundary.clone())
    );
    for invalid in [String::new(), boundary.clone() + "b"] {
        assert_eq!(
            account.save_learning_keystore(&invalid),
            Err(AccountError::Storage)
        );
        assert_eq!(
            account.load_learning_keystore().unwrap(),
            Some(boundary.clone())
        );
    }
    for invalid in [vec![0xff], vec![b'x'; capacity + 1]] {
        backend
            .inner
            .items
            .lock()
            .unwrap()
            .insert(SessionStoreItem::LearningKeystore, invalid);
        assert_eq!(account.load_learning_keystore(), Err(AccountError::Storage));
    }
}

#[test]
fn logout_attempts_all_supported_keys_even_when_one_removal_fails() {
    for failed in [
        None,
        Some(SessionStoreItem::Account),
        Some(SessionStoreItem::ClipboardKeystore),
        Some(SessionStoreItem::LearningKeystore),
    ] {
        let backend = Arc::new(Backend {
            learning: true,
            remove_error: failed,
            ..Default::default()
        });
        let (account, _) = stores(&backend);
        for item in [
            SessionStoreItem::Account,
            SessionStoreItem::ClipboardKeystore,
            SessionStoreItem::LearningKeystore,
            SessionStoreItem::Anonymous,
            SessionStoreItem::AnonymousIdentity,
        ] {
            backend
                .inner
                .items
                .lock()
                .unwrap()
                .insert(item, b"synthetic".to_vec());
        }
        assert_eq!(
            account.clear(),
            failed.map_or(Ok(()), |_| Err(AccountError::Storage))
        );
        assert_eq!(
            *backend.removed.lock().unwrap(),
            [
                SessionStoreItem::Account,
                SessionStoreItem::ClipboardKeystore,
                SessionStoreItem::LearningKeystore
            ]
        );
        let remaining = backend.inner.items.lock().unwrap();
        for item in [
            SessionStoreItem::Account,
            SessionStoreItem::ClipboardKeystore,
            SessionStoreItem::LearningKeystore,
        ] {
            assert_eq!(remaining.contains_key(&item), Some(item) == failed);
        }
        assert!(remaining.contains_key(&SessionStoreItem::Anonymous));
        assert!(remaining.contains_key(&SessionStoreItem::AnonymousIdentity));
    }
}

#[test]
fn learning_key_is_replaceable_not_create_only() {
    let backend = Arc::new(Backend {
        learning: true,
        already_exists: true,
        ..Default::default()
    });
    let (account, _) = stores(&backend);
    assert_eq!(
        account.save_learning_keystore("synthetic"),
        Err(AccountError::Storage)
    );
    assert_eq!(account.load_learning_keystore().unwrap(), None);
}
