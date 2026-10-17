//! 宿主注入的会话存储：`msime_client_session_store_register` 收下宿主的 load、save、remove、lock、unlock、release 回调，建好这个 store 的登录会话、匿名会话、社区服务和 AI 皮肤服务，之后 `msime_client_account`、`msime_client_account_chat_stream`、`msime_client_community`、`msime_client_account_cloud_clipboard`、`msime_client_account_access_token`、`msime_client_cloud_dictionary` 和 AI 皮肤的两个入口带 `session_store` 就经它们走，不带时仍按 `directory` 读写会话文件，行为不变。回调的契约见头文件 `msime_client_session_store` 的注释；编解码、校验和加锁顺序在 client-services 的 `account::host_storage`。
//!
//! 注册表里每个 store 一个条目，注册时一次建好全部对象，请求路径只查不插：`unregister` 之后不会有调用把会话重新插回去。条目最后一个引用释放时调用宿主的 `release`，那时本模块不持有任何锁。
//!
//! 属于 C ABI 的一部分，这些薄封装保证什么见父模块。

use super::account::Session;
use super::community::Services;
use crate::*;
use msime_client_services::account::{
    valid_client_agent, AccountError, AccountSessionFileLayout, AccountSessionStorage,
    BackendAccountClient, BackendAccountSession, FileAccountSessionStorage, HostSessionSlot,
    HostSessionStorage, HostStoreLocks, LockedAnonymousSessionStorage, SavedAccountSession,
    SessionStoreBackend, SessionStoreItem, WriteOutcome,
};
use std::ffi::c_void;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::PoisonError;
use std::time::Duration;

/// 头文件的 `MSIME_CLIENT_SESSION_STORE_VERSION`。
const SESSION_STORE_VERSION: u32 = 1;
/// 头文件的 `MSIME_CLIENT_SESSION_STORE_SHARED`。
const SESSION_STORE_SHARED: u32 = 1;
/// `client_agent` 最多读这么多字节，再往后还没有 NUL 就不合法。
const MAX_CLIENT_AGENT_BYTES: usize = 256;
/// id 留在 2^53 以内，JS 和 ArkTS 宿主当普通数字用不会丢精度。
const MAX_STORE_ID: u64 = (1 << 53) - 1;
const INVALID_SESSION_STORE: &str = "invalid_session_store";

type LoadCallback = unsafe extern "C" fn(*mut c_void, u32, *mut u8, usize) -> isize;
type SaveCallback = unsafe extern "C" fn(*mut c_void, u32, *const u8, usize) -> i32;
type RemoveCallback = unsafe extern "C" fn(*mut c_void, u32) -> i32;
type LockCallback = unsafe extern "C" fn(*mut c_void, u32, u32, *mut u64) -> i32;
type UnlockCallback = unsafe extern "C" fn(*mut c_void, u32, u64);
type ReleaseCallback = unsafe extern "C" fn(*mut c_void);

/// 头文件的 `msime_client_session_store`（版本 1）。函数指针都是 `Option`，宿主传 NULL 时读出来是 `None`，不是未定义行为。
#[repr(C)]
#[derive(Clone, Copy)]
pub struct SessionStoreV1 {
    pub version: u32,
    pub layout: u32,
    pub flags: u32,
    pub client_agent: *const c_char,
    pub context: *mut c_void,
    pub load: Option<LoadCallback>,
    pub save: Option<SaveCallback>,
    pub remove: Option<RemoveCallback>,
    pub lock: Option<LockCallback>,
    pub unlock: Option<UnlockCallback>,
    pub release: Option<ReleaseCallback>,
}

/// 注册成功的宿主回调。只有注册的最后一步才构造它，所以 `Drop` 里的 `release` 只会对注册成功的 context 调用，且恰好一次。
pub(super) struct CallbackBackend {
    store: SessionStoreV1,
    layout: AccountSessionFileLayout,
    shared: bool,
}

// SAFETY: 头文件要求回调可以在任意线程上被并发调用，`context` 在 `release` 之前一直有效；结构体里的其余指针（`client_agent`）注册后已清空，不再读。
unsafe impl Send for CallbackBackend {}
// SAFETY: 同上；这里只把函数指针和 `context` 原样交回宿主，不经共享引用改动任何东西。
unsafe impl Sync for CallbackBackend {}

impl SessionStoreBackend for CallbackBackend {
    fn read(
        &self,
        item: SessionStoreItem,
        capacity: usize,
    ) -> Result<Option<Vec<u8>>, AccountError> {
        let load = self.store.load.ok_or(AccountError::Storage)?;
        let mut buffer = vec![0_u8; capacity];
        // SAFETY: 缓冲区在本次调用期间有效、可写 `capacity` 字节；回调遵守头文件的契约。
        let length = unsafe {
            load(
                self.store.context,
                item.raw(),
                buffer.as_mut_ptr(),
                capacity,
            )
        };
        match usize::try_from(length) {
            Ok(0) => Ok(None),
            Ok(length) if length <= capacity => {
                buffer.truncate(length);
                Ok(Some(buffer))
            }
            // 负数是宿主报告失败；超过 capacity 说明宿主写越界或报错了长度，都不读。
            _ => Err(AccountError::Storage),
        }
    }

    fn write(&self, item: SessionStoreItem, bytes: &[u8]) -> Result<WriteOutcome, AccountError> {
        let save = self.store.save.ok_or(AccountError::Storage)?;
        // SAFETY: `bytes` 在本次调用期间有效；回调不保留它。
        match unsafe { save(self.store.context, item.raw(), bytes.as_ptr(), bytes.len()) } {
            0 => Ok(WriteOutcome::Written),
            1 if item == SessionStoreItem::AnonymousIdentity => Ok(WriteOutcome::AlreadyExists),
            _ => Err(AccountError::Storage),
        }
    }

    fn remove(&self, item: SessionStoreItem) -> Result<(), AccountError> {
        let remove = self.store.remove.ok_or(AccountError::Storage)?;
        // SAFETY: 回调遵守头文件的契约。
        match unsafe { remove(self.store.context, item.raw()) } {
            0 => Ok(()),
            _ => Err(AccountError::Storage),
        }
    }

    fn lock(&self, item: SessionStoreItem, timeout: Duration) -> Result<u64, AccountError> {
        let lock = self.store.lock.ok_or(AccountError::Storage)?;
        let timeout = u32::try_from(timeout.as_millis()).unwrap_or(u32::MAX);
        let mut token = 0_u64;
        // SAFETY: `token` 在本次调用期间有效、可写；回调遵守头文件的契约。
        match unsafe { lock(self.store.context, item.raw(), timeout, &mut token) } {
            0 => Ok(token),
            // 超时与文件存储拿不到 account-refresh.lock 时相同。
            1 => Err(AccountError::Unavailable),
            _ => Err(AccountError::Storage),
        }
    }

    fn unlock(&self, item: SessionStoreItem, token: u64) {
        if let Some(unlock) = self.store.unlock {
            // SAFETY: 与成功的 `lock` 配对，同一线程、同一个 item 和令牌。
            unsafe { unlock(self.store.context, item.raw(), token) }
        }
    }

    fn shared_across_processes(&self) -> bool {
        self.shared
    }

    fn layout(&self) -> AccountSessionFileLayout {
        self.layout
    }
}

impl Drop for CallbackBackend {
    fn drop(&mut self) {
        if let Some(release) = self.store.release {
            // SAFETY: 最后一个引用在这里结束，之后不再调用任何回调。
            unsafe { release(self.store.context) }
        }
    }
}

/// 会话对象用的存储：按 `directory` 的会话文件（登录会话 `account-session.json`、匿名会话 `anonymous-session.json`），或宿主注入的存储。
pub(super) enum SessionStorage {
    File(FileAccountSessionStorage),
    AnonymousFile(LockedAnonymousSessionStorage),
    Host(HostSessionStorage),
}

impl AccountSessionStorage for SessionStorage {
    fn load(&self) -> Result<Option<SavedAccountSession>, AccountError> {
        match self {
            Self::File(storage) => storage.load(),
            Self::AnonymousFile(storage) => storage.load(),
            Self::Host(storage) => storage.load(),
        }
    }

    fn save(&self, session: &SavedAccountSession) -> Result<(), AccountError> {
        match self {
            Self::File(storage) => storage.save(session),
            Self::AnonymousFile(storage) => storage.save(session),
            Self::Host(storage) => storage.save(session),
        }
    }

    fn clear(&self) -> Result<(), AccountError> {
        match self {
            Self::File(storage) => storage.clear(),
            Self::AnonymousFile(storage) => storage.clear(),
            Self::Host(storage) => storage.clear(),
        }
    }

    fn load_clipboard_keystore(&self) -> Result<Option<String>, AccountError> {
        match self {
            Self::File(storage) => storage.load_clipboard_keystore(),
            Self::AnonymousFile(storage) => storage.load_clipboard_keystore(),
            Self::Host(storage) => storage.load_clipboard_keystore(),
        }
    }

    fn save_clipboard_keystore(&self, value: &str) -> Result<(), AccountError> {
        match self {
            Self::File(storage) => storage.save_clipboard_keystore(value),
            Self::AnonymousFile(storage) => storage.save_clipboard_keystore(value),
            Self::Host(storage) => storage.save_clipboard_keystore(value),
        }
    }

    fn shared_across_processes(&self) -> bool {
        match self {
            Self::File(storage) => storage.shared_across_processes(),
            Self::AnonymousFile(storage) => storage.shared_across_processes(),
            Self::Host(storage) => storage.shared_across_processes(),
        }
    }

    fn with_refresh_lock<T>(
        &self,
        body: impl FnOnce() -> Result<T, AccountError>,
    ) -> Result<T, AccountError> {
        match self {
            Self::File(storage) => storage.with_refresh_lock(body),
            Self::AnonymousFile(storage) => storage.with_refresh_lock(body),
            Self::Host(storage) => storage.with_refresh_lock(body),
        }
    }
}

/// 一个注册的 store：注册时一次建好，请求只读。账号、匿名、社区和 AI 皮肤共用一个 HTTP 客户端。
pub(super) struct StoreEntry {
    pub(super) backend: Arc<CallbackBackend>,
    pub(super) account: Arc<Session>,
    pub(super) anonymous: Arc<Session>,
    pub(super) community: Arc<Services>,
    pub(super) ai_skin: Arc<super::ai_skin::AiSkinService>,
}

static STORES: OnceLock<Mutex<HashMap<u64, Arc<StoreEntry>>>> = OnceLock::new();
static NEXT_STORE_ID: AtomicU64 = AtomicU64::new(1);

fn stores() -> std::sync::MutexGuard<'static, HashMap<u64, Arc<StoreEntry>>> {
    // 表里只有 `Arc`，上一个持有者 panic 也不会留下半改的条目。
    STORES
        .get_or_init(|| Mutex::new(HashMap::new()))
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}

/// 注册过的 store；只在取引用时持表锁，调用宿主时不持。
pub(super) fn lookup(id: u64) -> Option<Arc<StoreEntry>> {
    stores().get(&id).cloned()
}

/// 一次请求用哪份会话：按目录，或注册的 store。
pub(super) enum Source<D> {
    Directory(D),
    Store(Arc<StoreEntry>),
}

/// `directory` 和 `session_store` 恰好给一个，`legacy_directory` 只能跟 `directory` 一起给，`session_store` 必须是注册过的；否则为 `None`，调用方报自己的「请求不合法」码。
pub(super) fn session_source<D>(
    directory: Option<D>,
    legacy_given: bool,
    session_store: Option<u64>,
) -> Option<Source<D>> {
    match (directory, session_store) {
        (Some(directory), None) => Some(Source::Directory(directory)),
        (None, Some(id)) if !legacy_given => lookup(id).map(Source::Store),
        _ => None,
    }
}

/// 读宿主给的 User-Agent：NULL 时为 `None`；超过 256 字节还没有 NUL、不是 UTF-8 或形状不对时是错误。
unsafe fn client_agent(pointer: *const c_char) -> Result<Option<String>, ()> {
    if pointer.is_null() {
        return Ok(None);
    }
    let mut bytes = Vec::new();
    for index in 0..=MAX_CLIENT_AGENT_BYTES {
        // SAFETY: 头文件要求非 NULL 的 client_agent 是以 NUL 结尾的字符串；读到 NUL 或第 257 个字节就停。
        let byte = unsafe { *pointer.add(index) } as u8;
        if byte == 0 {
            let agent = String::from_utf8(bytes).map_err(|_| ())?;
            return valid_client_agent(&agent).then_some(Some(agent)).ok_or(());
        }
        bytes.push(byte);
    }
    Err(())
}

fn register(store: SessionStoreV1) -> Result<Value, String> {
    let layout = match store.layout {
        0 => AccountSessionFileLayout::Native,
        1 => AccountSessionFileLayout::Apple,
        _ => return Err(INVALID_SESSION_STORE.into()),
    };
    let shared = store.flags & SESSION_STORE_SHARED != 0;
    if store.version != SESSION_STORE_VERSION
        || store.load.is_none()
        || store.save.is_none()
        || store.remove.is_none()
        || store.flags & !SESSION_STORE_SHARED != 0
        || (shared && (store.lock.is_none() || store.unlock.is_none()))
    {
        return Err(INVALID_SESSION_STORE.into());
    }
    // SAFETY: 由 register 的调用方契约保证。
    let agent = unsafe { client_agent(store.client_agent) }.map_err(|()| INVALID_SESSION_STORE)?;
    let client = match agent.as_deref() {
        Some(agent) => BackendAccountClient::with_user_agent(agent),
        None => BackendAccountClient::new(),
    }
    .map_err(|_| AccountError::Unavailable.code())?;
    let id = NEXT_STORE_ID.fetch_add(1, Ordering::Relaxed);
    if id > MAX_STORE_ID {
        return Err(AccountError::Unavailable.code().into());
    }
    // 从这里起不会再失败：CallbackBackend 一旦构造，drop 时就会调用 release。
    let backend = Arc::new(CallbackBackend {
        store: SessionStoreV1 {
            client_agent: std::ptr::null(),
            ..store
        },
        layout,
        shared,
    });
    let locks = Arc::new(HostStoreLocks::new());
    let storage = |slot| {
        let dynamic: Arc<dyn SessionStoreBackend> = backend.clone();
        SessionStorage::Host(HostSessionStorage::new(dynamic, slot, Arc::clone(&locks)))
    };
    let account = Arc::new(BackendAccountSession::new(
        client.clone(),
        storage(HostSessionSlot::Account),
    ));
    let anonymous = Arc::new(BackendAccountSession::new(
        client.clone(),
        storage(HostSessionSlot::Anonymous),
    ));
    let ai_skin = Arc::new(super::ai_skin::AiSkinService::with_anonymous(
        client.clone(),
        Arc::clone(&account),
        Arc::clone(&anonymous),
    ));
    let community = Arc::new(Services::new(client, Arc::clone(&account)));
    stores().insert(
        id,
        Arc::new(StoreEntry {
            backend,
            account,
            anonymous,
            community,
            ai_skin,
        }),
    );
    Ok(json!({ "session_store": id }))
}

/// 注册宿主的会话存储。请求、回调契约和返回值见头文件。
/// # Safety
/// `store` 指向一个可读的 `msime_client_session_store`；NULL 被拒绝。注册成功后结构体已被复制，`context` 要一直有效到 `release` 被调用。
#[no_mangle]
pub unsafe extern "C" fn msime_client_session_store_register(
    store: *const SessionStoreV1,
) -> *mut c_char {
    response(|| {
        if store.is_null() {
            return Err(INVALID_SESSION_STORE.into());
        }
        // SAFETY: 由调用方契约保证可读；字段都是 Copy。
        register(unsafe { std::ptr::read(store) })
    })
}

/// 注销一个 store。Value 是这个 id 是否曾经注册。进行中的调用照常结束，最后一个结束时调用 `release`。
#[no_mangle]
pub extern "C" fn msime_client_session_store_unregister(session_store: u64) -> *mut c_char {
    response(|| {
        // 先出表、放开表锁，再取消进行中的 Google 登录；`entry` 最后在这里 drop，那时不持有任何锁。
        let entry = stores().remove(&session_store);
        Ok(Value::Bool(match entry {
            Some(entry) => {
                entry.account.cancel_google_sign_in();
                drop(entry);
                true
            }
            None => false,
        }))
    })
}

#[cfg(test)]
pub(crate) mod tests {
    use super::*;
    use msime_client_services::account::{AccountTokens, AccountUser};
    use std::ffi::CStr;
    use std::sync::atomic::AtomicUsize;
    use std::time::{SystemTime, UNIX_EPOCH};

    /// 测试用的宿主存储：内存里的一张表，记下每次回调。
    #[derive(Default)]
    pub(crate) struct TestStore {
        pub(crate) items: Mutex<HashMap<u32, Vec<u8>>>,
        pub(crate) loads: AtomicUsize,
        pub(crate) saves: Mutex<Vec<u32>>,
        pub(crate) locks: Mutex<Vec<(u32, u64, std::thread::ThreadId)>>,
        pub(crate) unlocks: Mutex<Vec<(u32, u64, std::thread::ThreadId)>>,
        pub(crate) releases: AtomicUsize,
        /// 这一项的 load 把存着的字节用空格补满 capacity，再返回比 capacity 大一的长度：缓冲区里是一份能解码的会话，只有长度检查能拦下它。
        pub(crate) oversized: Mutex<Option<u32>>,
        /// 不为 None 时 save 不写入、直接返回这个值。
        pub(crate) save_result: Mutex<Option<i32>>,
        /// 不为 None 时 lock 不加锁、直接返回这个值。
        pub(crate) lock_result: Mutex<Option<i32>>,
        /// lock 回调开头调用一次，用来模拟违反契约、在回调里重入 `msime_client_*` 的宿主。
        pub(crate) on_lock: Mutex<Option<Box<dyn FnOnce() + Send>>>,
    }

    fn context(context: *mut c_void) -> &'static TestStore {
        // SAFETY: 测试把 `&'static TestStore` 作 context 传入。
        unsafe { &*(context as *const TestStore) }
    }

    unsafe extern "C" fn load(
        store: *mut c_void,
        item: u32,
        buffer: *mut u8,
        capacity: usize,
    ) -> isize {
        let store = context(store);
        store.loads.fetch_add(1, Ordering::SeqCst);
        if *store.oversized.lock().unwrap() == Some(item) {
            let mut padded = store.items.lock().unwrap().get(&item).cloned().unwrap();
            padded.resize(capacity, b' ');
            // SAFETY: Rust 给的缓冲区可写 capacity 字节，`padded` 正好这么长。
            unsafe { std::ptr::copy_nonoverlapping(padded.as_ptr(), buffer, capacity) };
            return capacity as isize + 1;
        }
        match store.items.lock().unwrap().get(&item) {
            Some(bytes) if bytes.len() <= capacity => {
                // SAFETY: Rust 给的缓冲区可写 capacity 字节。
                unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), buffer, bytes.len()) };
                bytes.len() as isize
            }
            Some(_) => -1,
            None => 0,
        }
    }

    unsafe extern "C" fn save(
        store: *mut c_void,
        item: u32,
        bytes: *const u8,
        length: usize,
    ) -> i32 {
        let store = context(store);
        store.saves.lock().unwrap().push(item);
        if let Some(code) = *store.save_result.lock().unwrap() {
            return code;
        }
        let mut items = store.items.lock().unwrap();
        if item == 4 && items.contains_key(&item) {
            return 1;
        }
        // SAFETY: Rust 给的字节在本次调用期间有效。
        items.insert(
            item,
            unsafe { std::slice::from_raw_parts(bytes, length) }.to_vec(),
        );
        0
    }

    unsafe extern "C" fn remove(store: *mut c_void, item: u32) -> i32 {
        context(store).items.lock().unwrap().remove(&item);
        0
    }

    unsafe extern "C" fn lock(
        store: *mut c_void,
        item: u32,
        timeout_ms: u32,
        token: *mut u64,
    ) -> i32 {
        assert_eq!(timeout_ms, 20_000);
        let store = context(store);
        let reenter = store.on_lock.lock().unwrap().take();
        if let Some(reenter) = reenter {
            reenter();
        }
        if let Some(code) = *store.lock_result.lock().unwrap() {
            return code;
        }
        let mut locks = store.locks.lock().unwrap();
        let value = 1_000 + locks.len() as u64;
        locks.push((item, value, std::thread::current().id()));
        // SAFETY: Rust 给的令牌指针在本次调用期间有效。
        unsafe { *token = value };
        0
    }

    unsafe extern "C" fn unlock(store: *mut c_void, item: u32, token: u64) {
        context(store)
            .unlocks
            .lock()
            .unwrap()
            .push((item, token, std::thread::current().id()));
    }

    unsafe extern "C" fn release(store: *mut c_void) {
        context(store).releases.fetch_add(1, Ordering::SeqCst);
    }

    pub(crate) fn leaked_store() -> &'static TestStore {
        Box::leak(Box::default())
    }

    pub(crate) fn descriptor(store: &'static TestStore, flags: u32, layout: u32) -> SessionStoreV1 {
        SessionStoreV1 {
            version: 1,
            layout,
            flags,
            client_agent: std::ptr::null(),
            context: store as *const TestStore as *mut c_void,
            load: Some(load),
            save: Some(save),
            remove: Some(remove),
            lock: Some(lock),
            unlock: Some(unlock),
            release: Some(release),
        }
    }

    fn read(raw: *mut c_char) -> Value {
        // SAFETY: 导出函数返回的字符串，读完释放。
        unsafe {
            let text = CStr::from_ptr(raw).to_str().unwrap().to_owned();
            msime_client_string_free(raw);
            serde_json::from_str(&text).unwrap()
        }
    }

    pub(crate) fn register_store(descriptor: &SessionStoreV1) -> Value {
        // SAFETY: 描述符在调用期间有效，context 是泄漏的测试表。
        read(unsafe { msime_client_session_store_register(descriptor) })
    }

    pub(crate) fn registered(store: &'static TestStore, flags: u32, layout: u32) -> u64 {
        register_store(&descriptor(store, flags, layout))["value"]["session_store"]
            .as_u64()
            .unwrap()
    }

    pub(crate) fn unregister(id: u64) -> Value {
        read(msime_client_session_store_unregister(id))
    }

    /// 一份未过期的合成会话，按 Native 布局编码。
    pub(crate) fn session_bytes(user: &str, access: char, session_id: bool) -> Vec<u8> {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_millis() as u64;
        serde_json::to_vec(&SavedAccountSession {
            tokens: AccountTokens {
                access_token: access.to_string().repeat(64),
                refresh_token: "d".repeat(64),
                token_type: "Bearer".into(),
                expires_in: 3600,
                user: AccountUser {
                    id: user.into(),
                    display_name: "合成用户".into(),
                    created_at: "2026-01-01T00:00:00Z".into(),
                    email: None,
                    avatar_url: None,
                },
            },
            expires_at_unix_ms: now + 3_600_000,
            session_id: session_id.then(|| msime_client_core::uuid::Uuid::from_u128(9)),
        })
        .unwrap()
    }

    #[cfg(target_pointer_width = "64")]
    #[test]
    fn the_descriptor_has_the_layout_the_header_declares() {
        use std::mem::{offset_of, size_of};
        assert_eq!(size_of::<SessionStoreV1>(), 80);
        assert_eq!(offset_of!(SessionStoreV1, version), 0);
        assert_eq!(offset_of!(SessionStoreV1, layout), 4);
        assert_eq!(offset_of!(SessionStoreV1, flags), 8);
        assert_eq!(offset_of!(SessionStoreV1, client_agent), 16);
        assert_eq!(offset_of!(SessionStoreV1, context), 24);
        assert_eq!(offset_of!(SessionStoreV1, load), 32);
        assert_eq!(offset_of!(SessionStoreV1, save), 40);
        assert_eq!(offset_of!(SessionStoreV1, remove), 48);
        assert_eq!(offset_of!(SessionStoreV1, lock), 56);
        assert_eq!(offset_of!(SessionStoreV1, unlock), 64);
        assert_eq!(offset_of!(SessionStoreV1, release), 72);
    }

    #[test]
    fn invalid_descriptors_are_refused_without_taking_the_context() {
        let store = leaked_store();
        let invalid = json!({ "ok": false, "error": "invalid_session_store" });
        // SAFETY: NULL 被拒绝，不解引用。
        assert_eq!(
            read(unsafe { msime_client_session_store_register(std::ptr::null()) }),
            invalid
        );
        let long_agent =
            std::ffi::CString::new(format!("msime-ios/1 ({})", "x".repeat(300))).unwrap();
        let changes: [fn(&mut SessionStoreV1); 10] = [
            |d| d.version = 2,
            |d| d.layout = 2,
            |d| d.flags = 2,
            |d| d.load = None,
            |d| d.save = None,
            |d| d.remove = None,
            |d| d.lock = None,
            |d| d.unlock = None,
            |d| d.client_agent = c"MSIME/Android".as_ptr(),
            |d| d.client_agent = c"msime-ios/1.0 (\xe4\xb8\xad)".as_ptr(),
        ];
        let mut cases = Vec::new();
        for change in changes {
            let mut case = descriptor(store, SESSION_STORE_SHARED, 0);
            change(&mut case);
            cases.push(case);
        }
        let mut too_long = descriptor(store, SESSION_STORE_SHARED, 0);
        too_long.client_agent = long_agent.as_ptr();
        cases.push(too_long);
        for (index, case) in cases.iter().enumerate() {
            assert_eq!(register_store(case), invalid, "case {index}");
        }
        assert_eq!(store.releases.load(Ordering::SeqCst), 0);

        // 不跨进程共享时可以不给锁回调；带合法的 User-Agent 也能注册。
        let mut unshared = descriptor(store, 0, 1);
        unshared.lock = None;
        unshared.unlock = None;
        unshared.client_agent = c"msime-ios/1.4.0 (iPhone; iOS 18; edition=full)".as_ptr();
        let id = register_store(&unshared)["value"]["session_store"]
            .as_u64()
            .unwrap();
        assert!(id > 0 && id <= MAX_STORE_ID);
        assert_eq!(unregister(id), json!({ "ok": true, "value": true }));
        assert_eq!(store.releases.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn release_waits_for_the_last_call_in_flight_and_runs_once() {
        let store = leaked_store();
        let id = registered(store, SESSION_STORE_SHARED, 0);
        let in_flight = lookup(id).unwrap();
        assert_eq!(unregister(id), json!({ "ok": true, "value": true }));
        // 注销后新请求查不到它，进行中的那一个还拿着引用。
        assert!(lookup(id).is_none());
        assert_eq!(store.releases.load(Ordering::SeqCst), 0);
        assert!(in_flight.account.status().unwrap().is_none());
        drop(in_flight);
        assert_eq!(store.releases.load(Ordering::SeqCst), 1);
        assert_eq!(unregister(id), json!({ "ok": true, "value": false }));
        assert_eq!(store.releases.load(Ordering::SeqCst), 1);
    }

    #[test]
    fn a_load_longer_than_the_buffer_is_a_storage_error() {
        let store = leaked_store();
        let id = registered(store, 0, 0);
        let entry = lookup(id).unwrap();
        // 对照：用空格补满整个缓冲区的会话照常能读，下面的失败只来自长度。
        let mut padded = session_bytes("synthetic-user", 'a', true);
        padded.resize(SessionStoreItem::Account.capacity(), b' ');
        store.items.lock().unwrap().insert(1, padded);
        assert_eq!(
            entry.backend.read(
                SessionStoreItem::Account,
                SessionStoreItem::Account.capacity()
            ),
            Ok(Some(store.items.lock().unwrap()[&1].clone()))
        );
        *store.oversized.lock().unwrap() = Some(1);
        assert_eq!(
            entry.backend.read(
                SessionStoreItem::Account,
                SessionStoreItem::Account.capacity()
            ),
            Err(AccountError::Storage)
        );
        assert_eq!(entry.account.status(), Err(AccountError::Storage));
        drop(entry);
        unregister(id);
    }

    #[test]
    fn the_raw_return_codes_of_save_remove_and_lock_map_to_account_errors() {
        let store = leaked_store();
        let id = registered(store, SESSION_STORE_SHARED, 0);
        let entry = lookup(id).unwrap();
        let backend = &entry.backend;
        let timeout = Duration::from_secs(20);

        // save：0 是写入；1 只对匿名身份是「已存在」，之后重读到的是另一方写的那份；其余是失败。
        assert_eq!(
            backend.write(SessionStoreItem::AnonymousIdentity, b"ours"),
            Ok(WriteOutcome::Written)
        );
        store.items.lock().unwrap().insert(4, b"theirs".to_vec());
        assert_eq!(
            backend.write(SessionStoreItem::AnonymousIdentity, b"ours"),
            Ok(WriteOutcome::AlreadyExists)
        );
        assert_eq!(
            backend.read(SessionStoreItem::AnonymousIdentity, 65_536),
            Ok(Some(b"theirs".to_vec()))
        );
        for (code, item) in [
            (1, SessionStoreItem::Account),
            (1, SessionStoreItem::ClipboardKeystore),
            (1, SessionStoreItem::Anonymous),
            (2, SessionStoreItem::AnonymousIdentity),
            (-1, SessionStoreItem::Account),
        ] {
            *store.save_result.lock().unwrap() = Some(code);
            assert_eq!(
                backend.write(item, b"x"),
                Err(AccountError::Storage),
                "save {code} {item:?}"
            );
        }
        *store.save_result.lock().unwrap() = None;

        // lock：0 是持有，1 是超时（与拿不到 account-refresh.lock 相同），其余是失败。
        assert_eq!(backend.lock(SessionStoreItem::Account, timeout), Ok(1_000));
        for (code, expected) in [
            (1, AccountError::Unavailable),
            (2, AccountError::Storage),
            (-1, AccountError::Storage),
        ] {
            *store.lock_result.lock().unwrap() = Some(code);
            assert_eq!(
                backend.lock(SessionStoreItem::Anonymous, timeout),
                Err(expected),
                "lock {code}"
            );
        }
        drop(entry);
        unregister(id);
    }

    #[test]
    fn exactly_one_of_directory_and_session_store_selects_the_session() {
        let store = leaked_store();
        let id = registered(store, 0, 0);
        assert!(matches!(
            session_source(Some("d"), false, None),
            Some(Source::Directory("d"))
        ));
        assert!(matches!(
            session_source::<&str>(None, false, Some(id)),
            Some(Source::Store(_))
        ));
        assert!(session_source(Some("d"), false, Some(id)).is_none());
        assert!(session_source::<&str>(None, false, None).is_none());
        assert!(session_source::<&str>(None, true, Some(id)).is_none());
        assert!(session_source::<&str>(None, false, Some(u64::MAX)).is_none());
        unregister(id);
        assert!(session_source::<&str>(None, false, Some(id)).is_none());
    }
}
