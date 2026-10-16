//! 宿主注入的会话存储：钥匙串、App Group、AndroidKeyStore 这类只有平台能读写的地方，由宿主经 [`SessionStoreBackend`] 提供读、写、删和跨进程锁，编解码、布局、合法性校验和刷新时的加锁顺序都在这里。C 接口一侧（`msime_client_session_store_register`）把宿主的回调包成 [`SessionStoreBackend`]；现有的文件存储（[`FileAccountSessionStorage`]）不变，仍是 Windows 和 Linux 的默认实现。
//!
//! 锁的顺序固定为：进程内按（store，槽）的 `Mutex` → 宿主的 `lock(item)` → `BackendAccountSession` 的会话 `Mutex` → `load`/`save`/`remove`。同一线程在持有某个 store 的任何一把刷新锁时再对同一个 store 加锁，直接返回 [`AccountError::Unavailable`]，不去等一把自己拿着的锁。

use super::file_storage::{
    decode, encode_session, LOCK_TIMEOUT, MAX_KEYSTORE_BYTES, MAX_SESSION_BYTES,
};
use super::*;
use std::cell::RefCell;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::PoisonError;

/// 宿主存储里的一项。数值是 C 接口 `msime_client_session_item` 的值。
#[derive(Clone, Copy, Debug, Eq, PartialEq, Hash)]
pub enum SessionStoreItem {
    /// 登录会话，也是一把刷新锁。
    Account = 1,
    /// 云剪贴板密钥串，UTF-8，不带换行；与登录会话同属账号槽，退出登录时一起删除。
    ClipboardKeystore = 2,
    /// 本机匿名账号的会话，也是一把刷新锁。
    Anonymous = 3,
    /// 本机匿名账号的身份 `{subject, secret}`；写入是「不存在才写」。
    AnonymousIdentity = 4,
}

impl SessionStoreItem {
    /// C 接口里的值。
    pub fn raw(self) -> u32 {
        self as u32
    }

    /// 宿主为这一项读入的最大字节数：会话和身份是 64 KiB，密钥串是 4 KiB，与文件存储的上限相同。
    pub fn capacity(self) -> usize {
        match self {
            Self::ClipboardKeystore => MAX_KEYSTORE_BYTES as usize,
            Self::Account | Self::Anonymous | Self::AnonymousIdentity => MAX_SESSION_BYTES as usize,
        }
    }
}

/// [`SessionStoreBackend::write`] 的结果。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum WriteOutcome {
    Written,
    /// 只对 [`SessionStoreItem::AnonymousIdentity`]：已经有值，没有覆盖。
    AlreadyExists,
}

/// 宿主提供的存储原语。实现方只负责字节和跨进程互斥，不解析内容。
pub trait SessionStoreBackend: Send + Sync + 'static {
    /// 读一项；不存在时为 `None`。返回的字节数超过 `capacity` 时实现方必须报错，不得截断。
    fn read(
        &self,
        item: SessionStoreItem,
        capacity: usize,
    ) -> Result<Option<Vec<u8>>, AccountError>;
    /// 原子替换一项；对 [`SessionStoreItem::AnonymousIdentity`] 是「不存在才写」。
    fn write(&self, item: SessionStoreItem, bytes: &[u8]) -> Result<WriteOutcome, AccountError>;
    /// 删除一项；不存在也算成功。
    fn remove(&self, item: SessionStoreItem) -> Result<(), AccountError>;
    /// 拿跨进程的刷新锁，`item` 只会是 [`SessionStoreItem::Account`] 或 [`SessionStoreItem::Anonymous`]。超时为 [`AccountError::Unavailable`]。只在 [`Self::shared_across_processes`] 为真时调用。
    fn lock(&self, item: SessionStoreItem, timeout: Duration) -> Result<u64, AccountError>;
    /// 释放 [`Self::lock`] 拿到的锁，在同一线程上、用同一个 `item` 和令牌，恰好一次。
    fn unlock(&self, item: SessionStoreItem, token: u64);
    /// 其他进程是否读写同一份存储。为真时每次访问都重新读，刷新时持 [`Self::lock`]。
    fn shared_across_processes(&self) -> bool;
    /// 会话文档的布局。
    fn layout(&self) -> AccountSessionFileLayout;
}

/// 一个 store 的会话存储分两个槽，各是一把刷新锁。
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum HostSessionSlot {
    /// 登录会话（[`SessionStoreItem::Account`]），密钥串也在这个槽里。
    Account,
    /// 匿名会话（[`SessionStoreItem::Anonymous`]）。
    Anonymous,
}

impl HostSessionSlot {
    fn item(self) -> SessionStoreItem {
        match self {
            Self::Account => SessionStoreItem::Account,
            Self::Anonymous => SessionStoreItem::Anonymous,
        }
    }
}

static NEXT_STORE_LOCKS: AtomicU64 = AtomicU64::new(1);

thread_local! {
    /// 本线程正持有刷新锁的 store（[`HostStoreLocks::id`]）。
    static HELD_STORES: RefCell<Vec<u64>> = const { RefCell::new(Vec::new()) };
}

/// 同一个 store 的两个槽共用的进程内锁。宿主的锁回调只需要排斥别的进程；本进程里的并发刷新由这里的 `Mutex` 排斥，所以宿主用 Java `FileChannel.lock` 这类同进程内不互斥的实现也不会出错。
pub struct HostStoreLocks {
    id: u64,
    account: Mutex<()>,
    anonymous: Mutex<()>,
}

impl HostStoreLocks {
    pub fn new() -> Self {
        Self {
            id: NEXT_STORE_LOCKS.fetch_add(1, Ordering::Relaxed),
            account: Mutex::new(()),
            anonymous: Mutex::new(()),
        }
    }

    fn slot(&self, slot: HostSessionSlot) -> &Mutex<()> {
        match slot {
            HostSessionSlot::Account => &self.account,
            HostSessionSlot::Anonymous => &self.anonymous,
        }
    }
}

impl Default for HostStoreLocks {
    fn default() -> Self {
        Self::new()
    }
}

/// 本线程对一个 store 的持锁登记；drop 时撤销。
struct HeldStore(u64);

impl HeldStore {
    fn enter(id: u64) -> Result<Self, AccountError> {
        HELD_STORES.with(|held| {
            let mut held = held.borrow_mut();
            if held.contains(&id) {
                return Err(AccountError::Unavailable);
            }
            held.push(id);
            Ok(Self(id))
        })
    }
}

impl Drop for HeldStore {
    fn drop(&mut self) {
        HELD_STORES.with(|held| held.borrow_mut().retain(|id| *id != self.0));
    }
}

/// 宿主锁的守卫：drop 时在同一线程上解锁，出错或 panic 也一样。
struct HostLockGuard<'a> {
    backend: &'a dyn SessionStoreBackend,
    item: SessionStoreItem,
    token: u64,
}

impl Drop for HostLockGuard<'_> {
    fn drop(&mut self) {
        self.backend.unlock(self.item, self.token);
    }
}

/// 一个 store 里一个槽的会话存储，实现 [`AccountSessionStorage`]。
#[derive(Clone)]
pub struct HostSessionStorage {
    backend: Arc<dyn SessionStoreBackend>,
    slot: HostSessionSlot,
    guard: Arc<HostStoreLocks>,
}

impl HostSessionStorage {
    /// 同一个 store 的两个槽要传同一个 `guard`。
    pub fn new(
        backend: Arc<dyn SessionStoreBackend>,
        slot: HostSessionSlot,
        guard: Arc<HostStoreLocks>,
    ) -> Self {
        Self {
            backend,
            slot,
            guard,
        }
    }
}

impl AccountSessionStorage for HostSessionStorage {
    fn load(&self) -> Result<Option<SavedAccountSession>, AccountError> {
        let item = self.slot.item();
        match self.backend.read(item, item.capacity())? {
            Some(bytes) if bytes.len() <= item.capacity() => decode(&bytes).map(Some),
            Some(_) => Err(AccountError::Storage),
            None => Ok(None),
        }
    }

    fn save(&self, session: &SavedAccountSession) -> Result<(), AccountError> {
        let item = self.slot.item();
        let bytes = encode_session(self.backend.layout(), session)?;
        if bytes.len() > item.capacity() {
            return Err(AccountError::Storage);
        }
        match self.backend.write(item, &bytes)? {
            WriteOutcome::Written => Ok(()),
            WriteOutcome::AlreadyExists => Err(AccountError::Storage),
        }
    }

    /// 账号槽先删会话再删密钥串，两者都试过才报错，与文件存储相同；匿名槽只有会话。
    fn clear(&self) -> Result<(), AccountError> {
        match self.slot {
            HostSessionSlot::Account => {
                let session = self.backend.remove(SessionStoreItem::Account);
                let keystore = self.backend.remove(SessionStoreItem::ClipboardKeystore);
                session.and(keystore)
            }
            HostSessionSlot::Anonymous => self.backend.remove(SessionStoreItem::Anonymous),
        }
    }

    fn load_clipboard_keystore(&self) -> Result<Option<String>, AccountError> {
        if self.slot != HostSessionSlot::Account {
            return Err(AccountError::Storage);
        }
        let item = SessionStoreItem::ClipboardKeystore;
        self.backend
            .read(item, item.capacity())?
            .map(|bytes| {
                if bytes.len() > item.capacity() {
                    return Err(AccountError::Storage);
                }
                let value = String::from_utf8(bytes).map_err(|_| AccountError::Storage)?;
                // 与文件存储相同：别的写入方可能在末尾带一个换行，它不属于密钥串。
                Ok(value.trim_end_matches(['\n', '\r']).to_owned())
            })
            .transpose()
    }

    fn save_clipboard_keystore(&self, value: &str) -> Result<(), AccountError> {
        let item = SessionStoreItem::ClipboardKeystore;
        if self.slot != HostSessionSlot::Account
            || value.is_empty()
            || value.len() > item.capacity()
        {
            return Err(AccountError::Storage);
        }
        match self.backend.write(item, value.as_bytes())? {
            WriteOutcome::Written => Ok(()),
            WriteOutcome::AlreadyExists => Err(AccountError::Storage),
        }
    }

    fn shared_across_processes(&self) -> bool {
        self.backend.shared_across_processes()
    }

    fn with_refresh_lock<T>(
        &self,
        body: impl FnOnce() -> Result<T, AccountError>,
    ) -> Result<T, AccountError> {
        // 先查本线程的登记，再拿进程内锁：本线程已经持有这个槽时，再去拿同一把 `Mutex` 会把自己锁死。
        let held = HeldStore::enter(self.guard.id)?;
        // `()` 没有要维护的不变量，上一个持有者 panic 留下的中毒状态可以直接忽略。
        let process = self
            .guard
            .slot(self.slot)
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        let host = if self.backend.shared_across_processes() {
            let item = self.slot.item();
            let token = self.backend.lock(item, LOCK_TIMEOUT)?;
            Some(HostLockGuard {
                backend: self.backend.as_ref(),
                item,
                token,
            })
        } else {
            None
        };
        let result = body();
        // 与加锁相反的顺序放开：宿主锁、进程内锁、本线程的登记。
        drop(host);
        drop(process);
        drop(held);
        result
    }
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use std::collections::HashMap;

    /// 内存里的宿主存储，记下每次加锁、解锁的线程和令牌。
    #[derive(Default)]
    pub(crate) struct MemoryBackend {
        pub(crate) items: Mutex<HashMap<SessionStoreItem, Vec<u8>>>,
        pub(crate) shared: bool,
        pub(crate) layout: Option<AccountSessionFileLayout>,
        pub(crate) locks: Mutex<Vec<(SessionStoreItem, u64, std::thread::ThreadId)>>,
        pub(crate) unlocks: Mutex<Vec<(SessionStoreItem, u64, std::thread::ThreadId)>>,
        pub(crate) reads: Mutex<usize>,
    }

    impl SessionStoreBackend for MemoryBackend {
        fn read(
            &self,
            item: SessionStoreItem,
            _capacity: usize,
        ) -> Result<Option<Vec<u8>>, AccountError> {
            *self.reads.lock().unwrap() += 1;
            Ok(self.items.lock().unwrap().get(&item).cloned())
        }

        fn write(
            &self,
            item: SessionStoreItem,
            bytes: &[u8],
        ) -> Result<WriteOutcome, AccountError> {
            let mut items = self.items.lock().unwrap();
            if item == SessionStoreItem::AnonymousIdentity && items.contains_key(&item) {
                return Ok(WriteOutcome::AlreadyExists);
            }
            items.insert(item, bytes.to_vec());
            Ok(WriteOutcome::Written)
        }

        fn remove(&self, item: SessionStoreItem) -> Result<(), AccountError> {
            self.items.lock().unwrap().remove(&item);
            Ok(())
        }

        fn lock(&self, item: SessionStoreItem, timeout: Duration) -> Result<u64, AccountError> {
            assert_eq!(timeout, Duration::from_secs(20));
            let mut locks = self.locks.lock().unwrap();
            let token = locks.len() as u64 + 100;
            locks.push((item, token, std::thread::current().id()));
            Ok(token)
        }

        fn unlock(&self, item: SessionStoreItem, token: u64) {
            self.unlocks
                .lock()
                .unwrap()
                .push((item, token, std::thread::current().id()));
        }

        fn shared_across_processes(&self) -> bool {
            self.shared
        }

        fn layout(&self) -> AccountSessionFileLayout {
            self.layout.unwrap_or(AccountSessionFileLayout::Native)
        }
    }

    fn storages(backend: &Arc<MemoryBackend>) -> (HostSessionStorage, HostSessionStorage) {
        let guard = Arc::new(HostStoreLocks::new());
        let dynamic: Arc<dyn SessionStoreBackend> = backend.clone();
        (
            HostSessionStorage::new(dynamic.clone(), HostSessionSlot::Account, guard.clone()),
            HostSessionStorage::new(dynamic, HostSessionSlot::Anonymous, guard),
        )
    }

    fn synthetic_session() -> SavedAccountSession {
        SavedAccountSession {
            tokens: AccountTokens {
                access_token: "a".repeat(64),
                refresh_token: "b".repeat(64),
                token_type: "Bearer".into(),
                expires_in: 900,
                user: AccountUser {
                    id: "synthetic-user".into(),
                    display_name: String::new(),
                    created_at: "2026-01-01T00:00:00Z".into(),
                    email: Some("person@example.test".into()),
                    avatar_url: None,
                },
            },
            expires_at_unix_ms: 1_800_000_000_000,
            session_id: Some(uuid::Uuid::from_u128(7)),
        }
    }

    #[test]
    fn sessions_round_trip_in_the_layout_the_host_chose() {
        for layout in [
            AccountSessionFileLayout::Native,
            AccountSessionFileLayout::Apple,
        ] {
            let backend = Arc::new(MemoryBackend {
                layout: Some(layout),
                ..Default::default()
            });
            let (account, anonymous) = storages(&backend);
            account.save(&synthetic_session()).unwrap();
            let bytes = backend.items.lock().unwrap()[&SessionStoreItem::Account].clone();
            let document: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
            // Apple 布局是 Swift 的 `BackendSavedSession`，保留 sessionID。
            assert_eq!(
                document.get("expiresAt").is_some(),
                layout == AccountSessionFileLayout::Apple
            );
            let loaded = account.load().unwrap().unwrap();
            assert_eq!(loaded.tokens.user, synthetic_session().tokens.user);
            assert_eq!(loaded.expires_at_unix_ms, 1_800_000_000_000);
            assert_eq!(loaded.session_id, Some(uuid::Uuid::from_u128(7)));
            // 两个槽互不相干。
            assert!(anonymous.load().unwrap().is_none());
        }
    }

    #[test]
    fn clearing_the_account_also_removes_the_keystore_but_not_the_anonymous_session() {
        let backend = Arc::new(MemoryBackend::default());
        let (account, anonymous) = storages(&backend);
        account.save(&synthetic_session()).unwrap();
        anonymous.save(&synthetic_session()).unwrap();
        account.save_clipboard_keystore("v1.synthetic").unwrap();
        backend.items.lock().unwrap().insert(
            SessionStoreItem::ClipboardKeystore,
            b"v1.synthetic\n".to_vec(),
        );
        assert_eq!(
            account.load_clipboard_keystore().unwrap().as_deref(),
            Some("v1.synthetic")
        );
        assert_eq!(
            anonymous.load_clipboard_keystore(),
            Err(AccountError::Storage)
        );
        assert_eq!(
            account.save_clipboard_keystore(""),
            Err(AccountError::Storage)
        );

        account.clear().unwrap();
        assert!(account.load().unwrap().is_none());
        assert_eq!(account.load_clipboard_keystore().unwrap(), None);
        assert!(anonymous.load().unwrap().is_some());
    }

    #[test]
    fn undecodable_or_oversized_documents_are_storage_errors() {
        let backend = Arc::new(MemoryBackend::default());
        let (account, _) = storages(&backend);
        backend
            .items
            .lock()
            .unwrap()
            .insert(SessionStoreItem::Account, b"not json".to_vec());
        assert_eq!(account.load().err(), Some(AccountError::Storage));
        // 一份能解码的会话用空格补到上限：正好上限时照常能读，多一个字节时只有长度检查能拦下它。
        let mut padded =
            encode_session(AccountSessionFileLayout::Native, &synthetic_session()).unwrap();
        padded.resize(SessionStoreItem::Account.capacity(), b' ');
        backend
            .items
            .lock()
            .unwrap()
            .insert(SessionStoreItem::Account, padded.clone());
        assert_eq!(
            account.load().unwrap().map(|saved| saved.tokens.user),
            Some(synthetic_session().tokens.user)
        );
        padded.push(b' ');
        backend
            .items
            .lock()
            .unwrap()
            .insert(SessionStoreItem::Account, padded);
        assert_eq!(account.load().err(), Some(AccountError::Storage));
    }

    #[test]
    fn a_shared_store_takes_the_host_lock_of_its_slot_and_releases_it_on_the_same_thread() {
        let backend = Arc::new(MemoryBackend {
            shared: true,
            ..Default::default()
        });
        let (account, anonymous) = storages(&backend);
        assert_eq!(account.with_refresh_lock(|| Ok(1)), Ok(1));
        assert_eq!(
            anonymous.with_refresh_lock(|| Err::<(), _>(AccountError::Conflict)),
            Err(AccountError::Conflict)
        );
        let here = std::thread::current().id();
        assert_eq!(
            *backend.locks.lock().unwrap(),
            [
                (SessionStoreItem::Account, 100, here),
                (SessionStoreItem::Anonymous, 101, here)
            ]
        );
        assert_eq!(
            *backend.unlocks.lock().unwrap(),
            *backend.locks.lock().unwrap()
        );

        // body panic 时也解锁，之后同一个槽照常能拿到锁。
        let panicked = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            account.with_refresh_lock(|| -> Result<(), AccountError> { panic!("synthetic") })
        }));
        assert!(panicked.is_err());
        assert_eq!(backend.unlocks.lock().unwrap().len(), 3);
        assert_eq!(account.with_refresh_lock(|| Ok(2)), Ok(2));
    }

    #[test]
    fn an_unshared_store_never_calls_the_host_lock() {
        let backend = Arc::new(MemoryBackend::default());
        let (account, _) = storages(&backend);
        assert_eq!(account.with_refresh_lock(|| Ok(())), Ok(()));
        assert!(backend.locks.lock().unwrap().is_empty());
    }

    #[test]
    fn nesting_any_lock_of_the_same_store_on_one_thread_is_refused() {
        let backend = Arc::new(MemoryBackend {
            shared: true,
            ..Default::default()
        });
        let (account, anonymous) = storages(&backend);
        let nested = account.with_refresh_lock(|| {
            assert_eq!(
                account.with_refresh_lock(|| Ok(())),
                Err(AccountError::Unavailable)
            );
            assert_eq!(
                anonymous.with_refresh_lock(|| Ok(())),
                Err(AccountError::Unavailable)
            );
            Ok(())
        });
        assert_eq!(nested, Ok(()));
        // 被拒绝的两次没有调宿主的锁。
        assert_eq!(backend.locks.lock().unwrap().len(), 1);
        assert_eq!(backend.unlocks.lock().unwrap().len(), 1);

        // 另一个 store 的锁不受影响。
        let other = Arc::new(MemoryBackend::default());
        let (other_account, _) = storages(&other);
        assert_eq!(
            account.with_refresh_lock(|| other_account.with_refresh_lock(|| Ok(3))),
            Ok(3)
        );
    }

    #[test]
    fn two_threads_refreshing_the_same_slot_take_turns() {
        let backend = Arc::new(MemoryBackend::default());
        let (account, _) = storages(&backend);
        let inside = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        std::thread::scope(|scope| {
            for _ in 0..4 {
                scope.spawn(|| {
                    account
                        .with_refresh_lock(|| {
                            assert_eq!(inside.fetch_add(1, Ordering::SeqCst), 0);
                            std::thread::sleep(Duration::from_millis(10));
                            inside.fetch_sub(1, Ordering::SeqCst);
                            Ok(())
                        })
                        .unwrap();
                });
            }
        });
    }
}
