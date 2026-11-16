//! 多进程之间的持久登录绑定，不以实例内计数或访问令牌代替登录身份。

use super::*;

#[derive(Clone, Default)]
struct SharedStorage {
    inner: Storage,
    refresh_lock: Arc<Mutex<()>>,
}

impl AccountSessionStorage for SharedStorage {
    fn load(&self) -> Result<Option<SavedAccountSession>, AccountError> {
        self.inner.load()
    }
    fn save(&self, value: &SavedAccountSession) -> Result<(), AccountError> {
        self.inner.save(value)
    }
    fn clear(&self) -> Result<(), AccountError> {
        self.inner.clear()
    }
    fn shared_across_processes(&self) -> bool {
        true
    }
    fn with_refresh_lock<T>(
        &self,
        operation: impl FnOnce() -> Result<T, AccountError>,
    ) -> Result<T, AccountError> {
        let _lock = self.refresh_lock.lock().unwrap();
        operation()
    }
    fn load_clipboard_keystore(&self) -> Result<Option<String>, AccountError> {
        self.inner.load_clipboard_keystore()
    }
    fn save_clipboard_keystore(&self, value: &str) -> Result<(), AccountError> {
        self.inner.save_clipboard_keystore(value)
    }
}

fn process(
    server: &Arc<Mutex<Server>>,
    storage: &SharedStorage,
) -> BackendAccountSession<Api, SharedStorage> {
    BackendAccountSession::new(
        Api {
            server: Arc::clone(server),
        },
        storage.clone(),
    )
}

#[test]
fn relogin_of_the_same_user_rejects_old_list_and_upload_without_network() {
    let server = server();
    let storage = SharedStorage::default();
    let keyboard = process(&server, &storage);
    let settings = process(&server, &storage);
    settings.sign_in("synthetic-challenge", "123456").unwrap();
    let old = keyboard.login_binding().unwrap().unwrap();
    settings.sign_in("synthetic-challenge", "123456").unwrap();
    let current = settings.login_binding().unwrap().unwrap();
    assert_eq!(old.user_id, current.user_id);
    assert_ne!(old.session_id, current.session_id);
    // 合成服务在两次登录中给相同令牌，仍必须按登录编号拒绝。
    for result in [
        keyboard
            .clipboard_for_login("", None, None, Some(&old))
            .map(|_| ()),
        keyboard
            .add_clipboard_for_login("合成旧会话复制", None, None, Some(&old))
            .map(|_| ()),
    ] {
        assert_eq!(clipboard_code(result), "account_cancelled");
    }
    assert!(server.lock().unwrap().log.is_empty());
    assert!(storage.inner.keystore.lock().unwrap().is_none());
    assert!(keyboard
        .clipboard_for_login("", None, None, Some(&current))
        .is_ok());
}

#[test]
fn token_refresh_keeps_the_binding_across_new_processes() {
    let server = server();
    let storage = SharedStorage::default();
    let settings = process(&server, &storage);
    settings.sign_in("synthetic-challenge", "123456").unwrap();
    let login = settings.login_binding().unwrap().unwrap();
    server.lock().unwrap().rejected_token = Some(token(b'a'));
    let keyboard = process(&server, &storage);
    keyboard
        .clipboard_for_login("", None, None, Some(&login))
        .unwrap();
    keyboard
        .add_clipboard_for_login("合成刷新后复制", None, None, Some(&login))
        .unwrap();
    assert_eq!(
        process(&server, &storage).login_binding().unwrap(),
        Some(login)
    );
    let stored = storage.inner.session.lock().unwrap();
    assert_eq!(stored.as_ref().unwrap().tokens.access_token, token(b'c'));
    assert_eq!(server.lock().unwrap().items.len(), 1);
}

#[test]
fn concurrent_old_session_migration_publishes_one_binding_without_network() {
    let server = server();
    let storage = SharedStorage::default();
    let settings = process(&server, &storage);
    assert_eq!(settings.login_binding().unwrap(), None);
    settings.sign_in("synthetic-challenge", "123456").unwrap();
    storage
        .inner
        .session
        .lock()
        .unwrap()
        .as_mut()
        .unwrap()
        .session_id = None;
    let barrier = Arc::new(std::sync::Barrier::new(3));
    let workers: Vec<_> = (0..2)
        .map(|_| {
            let session = process(&server, &storage);
            let barrier = Arc::clone(&barrier);
            std::thread::spawn(move || {
                barrier.wait();
                session.login_binding().unwrap().unwrap()
            })
        })
        .collect();
    barrier.wait();
    let bindings: Vec<_> = workers
        .into_iter()
        .map(|worker| worker.join().unwrap())
        .collect();
    assert_eq!(bindings[0], bindings[1]);
    assert_eq!(
        settings.login_binding().unwrap().as_ref(),
        Some(&bindings[0])
    );
    assert!(server.lock().unwrap().log.is_empty());
}
