//! 进程级的部分：模块缓存和后台编译线程、内存总账、熔断与健康文件、panic 总开关。
//!
//! - **模块缓存**按内容哈希存放编译好的 `ScriptModule`（内部是 `Arc`，可以跨线程共享）。会话线程只登记请求、查结果；读包、核对哈希、校验和 Eager 编译都在后台编译线程上做，不在按键和 focus 路径上。还没编译好的插件这一次跳过，不算失败。
//! - **熔断**按 (插件根目录, id, 哈希) 计，不按会话：IBus 改一个设置就重建会话，按会话熔断会让坏插件在每个新会话里重来一次。初始化失败、ABI 不符或模块不合规立即停用；否则最近 50 次投递里失败 3 次、或最近 20 次里有 3 次超过 4 ms，就停用。停用持续到授权文件的 `revision` 比以前看到过的都大（`observe_revision`）、写入方在上界回绕到时间戳，或进程重启。
//! - **健康文件** `<user_data>/script-health.json` 只记被停用的插件和停用的类别（`invalid`、`init`、`failed`、`slow`），在失焦和 destroy 时交给后台线程写出，只有停用或恢复之后才重写；进程重启也是恢复，本进程第一次读授权时文件在就重写一次（`adopt_health`）。不记失败次数，也不按原因细分：哪种 trap、失败几次都由脚本自己选，按次数或原因写出去，同一用户下的进程每次失焦后对比文件，就是一条能反复使用、按位传文本的通道；只记停用，熔断之后要等新的授权代次或进程重启才能再触发，剩下的是设计接受的一次性约 1 bit。具体的原因码只在停用时进一次诊断。
//! - **panic 总开关**：投递里任何一处 panic 都关掉本进程的全部脚本，直到进程重启；那次调用照常返回上屏结果。
//!
//! import 从不回调宿主，所以这里的锁都不会在脚本代码运行时被重入；锁的顺序固定为先 `REGISTRY` 后单个 `Health`，从不反过来。

use msime_client_core::file_lock;
use msime_client_core::plugins::script_pack;
use msime_script_runtime::{limits, Failure, ScriptModule};
use serde_json::json;
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::mpsc::{self, Sender};
use std::sync::{Arc, LazyLock, Mutex, MutexGuard, OnceLock, PoisonError};
use std::time::{Duration, Instant};

/// 失败率熔断的窗口：最近这么多次投递。
const FAILURE_WINDOW: u32 = 50;
/// 窗口里失败这么多次就停用。
const FAILURE_LIMIT: u32 = 3;
/// 耗时熔断的窗口：最近这么多次投递。fuel 才是硬限制，墙钟只做事后熔断。
const SLOW_WINDOW: u32 = 20;
/// 窗口里有这么多次超时就停用。
const SLOW_LIMIT: u32 = 3;
/// 一次投递超过这么久算慢。远低于 Windows TIP 等 Server 回复的 50 ms。
#[cfg_attr(test, allow(dead_code))]
const SLOW_AFTER: Duration = Duration::from_millis(4);
/// 健康文件名，放在 `user_data` 下；授权页按同一个名字读它。
pub(super) const HEALTH_FILE: &str = msime_client_core::plugins::script_grants::HEALTH_FILE;

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    // 持锁时不跑脚本代码，也不调用会 panic 的东西；真被毒化了，里面的计数仍然可用。
    mutex.lock().unwrap_or_else(PoisonError::into_inner)
}

// ---- 开关 ----

/// 投递中 panic 过：本进程的全部脚本关掉，直到重启。
static PANICKED: AtomicBool = AtomicBool::new(false);
/// panic 只报告一次。
static PANIC_REPORTED: AtomicBool = AtomicBool::new(false);

#[cfg(test)]
thread_local! {
    /// 测试线程自己的进程开关：设了就不看全局开关，测试之间互不影响。
    pub(crate) static TEST_SWITCH: std::cell::Cell<Option<bool>> = const { std::cell::Cell::new(None) };
    /// 测试线程自己的 panic 总开关。
    pub(crate) static TEST_PANICKED: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
    /// 测试里墙钟熔断关掉；设为 true 时每次投递都算慢。
    pub(crate) static TEST_SLOW: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

fn panicked() -> bool {
    #[cfg(test)]
    {
        TEST_PANICKED.with(std::cell::Cell::get)
    }
    #[cfg(not(test))]
    {
        PANICKED.load(Ordering::Acquire)
    }
}

/// 进程开关开着，并且没有因为 panic 关掉。
pub(super) fn running() -> bool {
    #[cfg(test)]
    let switched = TEST_SWITCH
        .with(std::cell::Cell::get)
        .unwrap_or_else(super::switched_on);
    #[cfg(not(test))]
    let switched = super::switched_on();
    switched && !panicked()
}

/// 投递里出了 panic：关掉本进程的全部脚本并报告一次。
pub(super) fn panic_caught() {
    #[cfg(test)]
    TEST_PANICKED.with(|panicked| panicked.set(true));
    PANICKED.store(true, Ordering::Release);
    if !PANIC_REPORTED.swap(true, Ordering::AcqRel) {
        crate::diagnostics::report(
            "script runtime panicked",
            "every script plugin is off until the input method restarts",
        );
    }
}

/// 墙钟：这次投递是否算慢。测试里不看真实时间。
pub(super) fn slow(started: Instant) -> bool {
    #[cfg(test)]
    {
        let _ = started;
        TEST_SLOW.with(std::cell::Cell::get)
    }
    #[cfg(not(test))]
    {
        started.elapsed() > SLOW_AFTER
    }
}

// ---- 模块缓存与后台编译线程 ----

enum Slot {
    /// 已登记，后台线程还没编译完。
    Pending,
    Ready(ScriptModule),
    /// 包读不出，或者已安装的包和授权的哈希对不上（授权之后包被换过）。不是插件的失败，包的戳变了再试。
    Retry,
    /// 模块不符合 ABI v1。内容按哈希寻址，同样的字节永远不合规，不再重试。
    Invalid(&'static str),
}

struct Cached {
    /// 哪个插件根目录登记的，淘汰时按它分开。
    root: PathBuf,
    slot: Slot,
    /// 计进内存总账的模块开销：翻译结果的上界、固定开销和它的 Engine 留着的一份值栈（`limits::module_memory_bytes`）。模块有自己的 Engine，条目淘汰、最后一个克隆释放时这些内存真的还回去。
    bytecode: usize,
}

static CACHE: LazyLock<Mutex<HashMap<String, Cached>>> = LazyLock::new(Default::default);

/// 会话查缓存的结果。
pub(super) enum Lookup {
    Ready(ScriptModule),
    /// 还在编译，或者要等包变了再试：这一次跳过，不算失败。
    Waiting,
    /// 模块不合规，原因是封闭的错误码。
    Invalid(&'static str),
}

pub(super) fn lookup(hash: &str) -> Lookup {
    match lock(&CACHE).get(hash).map(|cached| &cached.slot) {
        Some(Slot::Ready(module)) => Lookup::Ready(module.clone()),
        Some(Slot::Invalid(reason)) => Lookup::Invalid(reason),
        Some(Slot::Pending | Slot::Retry) | None => Lookup::Waiting,
    }
}

/// 登记一个要编译的模块。已经编译好、正在编译或确定不合规的不重复登记；`Retry` 的重新登记。
pub(super) fn request(root: &Path, id: &str, hash: &str) {
    {
        let mut cache = lock(&CACHE);
        match cache.get_mut(hash) {
            Some(Cached {
                slot: slot @ Slot::Retry,
                ..
            }) => *slot = Slot::Pending,
            Some(_) => return,
            None => {
                cache.insert(
                    hash.to_owned(),
                    Cached {
                        root: root.to_owned(),
                        slot: Slot::Pending,
                        bytecode: 0,
                    },
                );
            }
        }
    }
    let sent = send(Job::Compile {
        root: root.to_owned(),
        id: id.to_owned(),
        hash: hash.to_owned(),
    });
    if !sent {
        // 编译线程起不来：留作 Retry，下次聚焦时再登记。
        if let Some(cached) = lock(&CACHE).get_mut(hash) {
            cached.slot = Slot::Retry;
        }
    }
}

/// 淘汰 `root` 登记的、授权里已经不再启用的模块，免得反复更新的包让旧版本一直占着总账。还在编译的不动：编译完发现条目已经不在就丢掉结果。会话手上已有的克隆照样能用到它下一次重读授权。
pub(super) fn retain(root: &Path, hashes: &[&str]) {
    lock(&CACHE).retain(|hash, cached| {
        let live_clone = matches!(&cached.slot, Slot::Ready(module) if module.is_shared());
        let keep = cached.root != root
            || matches!(cached.slot, Slot::Pending)
            || hashes.contains(&hash.as_str())
            || live_clone;
        if !keep {
            BYTECODE_BYTES.fetch_sub(cached.bytecode, Ordering::AcqRel);
        }
        keep
    });
}

/// 测试直接放进一个编译好的模块，不经过包和后台线程。
#[cfg(test)]
pub(crate) fn insert_ready(root: &Path, hash: &str, module: ScriptModule) {
    lock(&CACHE).insert(
        hash.to_owned(),
        Cached {
            root: root.to_owned(),
            slot: Slot::Ready(module),
            bytecode: 0,
        },
    );
}

enum Job {
    Compile {
        root: PathBuf,
        id: String,
        hash: String,
    },
    WriteHealth {
        path: PathBuf,
        bytes: Vec<u8>,
    },
}

/// 交给后台线程。线程在第一次需要时才起：从不运行脚本的进程（设置应用、MCP、Windows TIP）不起任何线程。
fn send(job: Job) -> bool {
    static WORKER: OnceLock<Option<Sender<Job>>> = OnceLock::new();
    let worker = WORKER.get_or_init(|| {
        let (sender, receiver) = mpsc::channel::<Job>();
        let spawned = std::thread::Builder::new()
            .name("msime-script-compile".into())
            .spawn(move || {
                for job in receiver {
                    let compiling = match &job {
                        Job::Compile { hash, .. } => Some(hash.clone()),
                        Job::WriteHealth { .. } => None,
                    };
                    // 编译器或文件读写里的 panic 不能带走这个线程，否则之后的模块永远停在 Pending。按设计 §4.5，捕获到 panic 就关掉本进程的全部脚本；那个模块记为不合规，不再停在 Pending，也不再重试。
                    let ran = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| run(job)));
                    if ran.is_err() {
                        if let Some(hash) = compiling {
                            settle(&hash, Slot::Invalid("compile_panicked"), 0);
                        }
                        panic_caught();
                    }
                }
            });
        match spawned {
            Ok(_) => Some(sender),
            Err(error) => {
                crate::diagnostics::report("script compile thread not started", &error.to_string());
                None
            }
        }
    });
    worker
        .as_ref()
        .is_some_and(|sender| sender.send(job).is_ok())
}

fn run(job: Job) {
    match job {
        Job::Compile { root, id, hash } => compile(&root, &id, &hash),
        Job::WriteHealth { path, bytes } => {
            if let Err(error) = file_lock::replace_private_file(&path, &bytes) {
                crate::diagnostics::report("script health not written", &error.to_string());
            }
        }
    }
}

/// 读出已安装的包、核对哈希、按 ABI v1 校验并 Eager 编译，结果放回缓存。
fn compile(root: &Path, id: &str, hash: &str) {
    #[cfg(test)]
    if id == TEST_COMPILE_PANIC_ID {
        panic!("injected compile panic");
    }
    let (slot, bytecode) = match script_pack::load(root, id) {
        Err(_) => (Slot::Retry, 0),
        Ok(loaded) if loaded.pack.content_hash != hash => (Slot::Retry, 0),
        Ok(loaded) => {
            match msime_script_runtime::compile(&loaded.wasm, loaded.pack.declaration()) {
                Ok(module) => (
                    Slot::Ready(module),
                    limits::module_memory_bytes(loaded.wasm.len()),
                ),
                Err(error) => (Slot::Invalid(error.reason()), 0),
            }
        }
    };
    settle(hash, slot, bytecode);
}

/// 测试里编译这个 id 时在后台线程上 panic。
#[cfg(test)]
pub(crate) const TEST_COMPILE_PANIC_ID: &str = "panicking-compile";

/// 把编译结果放回缓存。
fn settle(hash: &str, slot: Slot, bytecode: usize) {
    let mut cache = lock(&CACHE);
    // 编译期间条目被淘汰了：结果没人要。
    if let Some(cached) = cache.get_mut(hash) {
        if matches!(cached.slot, Slot::Pending) {
            cached.slot = slot;
            cached.bytecode = bytecode;
            BYTECODE_BYTES.fetch_add(bytecode, Ordering::AcqRel);
        }
    }
}

// ---- 内存总账 ----

/// 缓存里各模块开销的合计（`limits::module_memory_bytes`）。
static BYTECODE_BYTES: AtomicUsize = AtomicUsize::new(0);
/// 活着的实例一次预留的线性内存合计。
static LIVE_BYTES: AtomicUsize = AtomicUsize::new(0);

/// 一个实例占着的线性内存，丢弃时还回总账。
pub(super) struct Reservation(usize);

impl Drop for Reservation {
    fn drop(&mut self) {
        LIVE_BYTES.fetch_sub(self.0, Ordering::AcqRel);
    }
}

/// 给一个新实例记账：线性内存预留和全部模块开销（翻译结果、固定开销、各自的值栈）合计不超过 `limits::PROCESS_MEMORY_BYTES`。授权时已经按单个会话做过准入检查，这里是多个会话同时有实例时的兜底；放不下时这一次不实例化，不算失败。
pub(super) fn reserve(bytes: usize) -> Option<Reservation> {
    LIVE_BYTES
        .fetch_update(Ordering::AcqRel, Ordering::Acquire, |live| {
            let total = live
                .saturating_add(bytes)
                .saturating_add(BYTECODE_BYTES.load(Ordering::Acquire));
            (total <= limits::PROCESS_MEMORY_BYTES).then_some(live + bytes)
        })
        .ok()
        .map(|_| Reservation(bytes))
}

// ---- 熔断与健康文件 ----

/// 停用的类别。健康文件里只有它；具体原因码只进诊断。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Trip {
    /// 模块不符合 ABI v1，或者编译时出了 panic。
    Invalid,
    /// 实例化或 `msime_init` 失败，包括 ABI 版本不符。
    Init,
    /// 投递时失败太多次。
    Failed,
    /// 投递太慢的次数太多。
    Slow,
}

impl Trip {
    fn as_str(self) -> &'static str {
        match self {
            Self::Invalid => "invalid",
            Self::Init => "init",
            Self::Failed => "failed",
            Self::Slow => "slow",
        }
    }
}

/// 一个 (根目录, id, 哈希) 的健康状况，进程里只有一份，各会话共用。
pub(super) struct Health {
    root: PathBuf,
    id: String,
    hash: String,
    /// 停用的快速判断，按键路径上不用拿锁。
    disabled: AtomicBool,
    state: Mutex<HealthState>,
}

#[derive(Default)]
struct HealthState {
    /// 最近 `FAILURE_WINDOW` 次投递，位为 1 的是失败，最新的在最低位。
    recent: u64,
    /// 最近 `SLOW_WINDOW` 次投递，位为 1 的是慢。
    slow: u64,
    /// 停用的类别和具体原因码。
    disabled: Option<(Trip, &'static str)>,
}

fn window(bits: u64, width: u32) -> u64 {
    bits & ((1u64 << width) - 1)
}

impl Health {
    pub(super) fn disabled(&self) -> bool {
        self.disabled.load(Ordering::Acquire)
    }

    /// 记一次投递。`failure` 为 `None` 是成功。窗口里的失败或慢的次数到了上限就停用。没停用时什么都不写出。
    pub(super) fn record(&self, failure: Option<Failure>, slow: bool) {
        let tripped = {
            let mut state = lock(&self.state);
            if state.disabled.is_some() {
                return;
            }
            state.recent = window(
                state.recent << 1 | u64::from(failure.is_some()),
                FAILURE_WINDOW,
            );
            state.slow = window(state.slow << 1 | u64::from(slow), SLOW_WINDOW);
            let tripped = match failure {
                Some(failure) if state.recent.count_ones() >= FAILURE_LIMIT => {
                    Some((Trip::Failed, failure.as_str()))
                }
                _ if state.slow.count_ones() >= SLOW_LIMIT => Some((Trip::Slow, "slow")),
                _ => None,
            };
            if tripped.is_some() {
                state.disabled = tripped;
                self.disabled.store(true, Ordering::Release);
            }
            tripped
        };
        if let Some((_, reason)) = tripped {
            mark_dirty(&self.root);
            self.report(reason);
        }
    }

    /// 立即停用：模块不合规、初始化失败、ABI 不符。
    pub(super) fn disable(&self, trip: Trip, reason: &'static str) {
        {
            let mut state = lock(&self.state);
            if state.disabled.is_some() {
                return;
            }
            state.disabled = Some((trip, reason));
            self.disabled.store(true, Ordering::Release);
        }
        mark_dirty(&self.root);
        self.report(reason);
    }

    /// 诊断行只有插件 id 和封闭的原因码，不带任何文本；每次停用只报一次。
    fn report(&self, reason: &'static str) {
        crate::diagnostics::report("script plugin disabled", &format!("{} {reason}", self.id));
    }

    fn reset(&self) {
        *lock(&self.state) = HealthState::default();
        self.disabled.store(false, Ordering::Release);
    }
}

#[derive(Default)]
struct Registry {
    /// 每个插件根目录看到过的最大的授权 `(revision_epoch, revision)`。
    revisions: HashMap<PathBuf, (u64, u64)>,
    /// 本进程读过授权的插件根目录：第一次读时接管健康文件（`adopt_health`）。
    adopted: HashSet<PathBuf>,
    entries: HashMap<(PathBuf, String, String), Arc<Health>>,
    /// 有没写出去的变化的根目录。
    dirty: HashSet<PathBuf>,
}

static REGISTRY: LazyLock<Mutex<Registry>> = LazyLock::new(Default::default);

fn mark_dirty(root: &Path) {
    lock(&REGISTRY).dirty.insert(root.to_owned());
}

/// 这个插件在这个哈希下的健康状况，进程里只有一份。
pub(super) fn health(root: &Path, id: &str, hash: &str) -> Arc<Health> {
    lock(&REGISTRY)
        .entries
        .entry((root.to_owned(), id.to_owned(), hash.to_owned()))
        .or_insert_with(|| {
            Arc::new(Health {
                root: root.to_owned(),
                id: id.to_owned(),
                hash: hash.to_owned(),
                disabled: AtomicBool::new(false),
                state: Mutex::new(HealthState::default()),
            })
        })
        .clone()
}

/// 测试读一个插件的 (最近 50 次投递里的失败次数, 停用的具体原因码)。
#[cfg(test)]
pub(crate) fn health_snapshot(root: &Path, id: &str, hash: &str) -> (u32, Option<&'static str>) {
    let health = health(root, id, hash);
    let state = lock(&health.state);
    (
        state.recent.count_ones(),
        state.disabled.map(|(_, reason)| reason),
    )
}

/// 会话重读了授权：`(revision_epoch, revision)` 比以前看到过的都大（用户在设置页改过授权、删过或重新导入过包），这个根目录下被熔断的插件全部恢复。第一次看到时只记下来；不比以前大的（放回一份旧文件、删掉文件）什么都不做，来回换文件不能让熔断反复复位。同一用户下的程序直接写一份更大代次的文件仍然能让它复位：授权文件不签名，这和它能伪造授权是同一个残余风险。
pub(super) fn observe_revision(root: &Path, revision_epoch: u64, revision: u64) {
    let mut registry = lock(&REGISTRY);
    let previous = registry.revisions.get(root).copied();
    let next = (revision_epoch, revision);
    if previous.is_some_and(|previous| next <= previous) {
        return;
    }
    registry.revisions.insert(root.to_owned(), next);
    if previous.is_none() {
        return;
    }
    for ((entry_root, _, _), health) in &registry.entries {
        if entry_root == root {
            health.reset();
        }
    }
    registry.dirty.insert(root.to_owned());
}

/// 本进程第一次为这个根目录读授权时调用：健康文件里已有的停用记录是上一个输入法进程留下的，停用只持续到进程重启，本进程里那些插件已经恢复了。文件在时标记为待写，下一次失焦按本进程的状态重写它，设置页不再显示早已解除的停用；文件不在时不写，没有变化就不写的约定不变。只在开关打开的进程里调用（会话重读授权的前提），所以不运行脚本的进程（Windows TIP、设置应用）从不改写别人的记录。
pub(super) fn adopt_health(root: &Path, file: Option<&Path>) {
    if lock(&REGISTRY).adopted.contains(root) {
        return;
    }
    // 不在持锁时碰文件系统。
    let stale = file.is_some_and(Path::exists);
    let mut registry = lock(&REGISTRY);
    if registry.adopted.insert(root.to_owned()) && stale {
        registry.dirty.insert(root.to_owned());
    }
}

/// 测试用的屏障：后台线程按顺序处理，`file` 写出来时，之前交过去的写入都已经完成。
#[cfg(test)]
pub(crate) fn health_barrier(file: &Path) {
    send(Job::WriteHealth {
        path: file.to_owned(),
        bytes: b"{}".to_vec(),
    });
}

/// 这个根目录下有插件停用或恢复过时，交给后台线程写进 `file`：每个停用的插件只有 id、哈希和停用的类别。
pub(super) fn flush_health(root: &Path, file: &Path) {
    let plugins = {
        let mut registry = lock(&REGISTRY);
        if !registry.dirty.remove(root) {
            return;
        }
        let mut plugins: Vec<_> = registry
            .entries
            .values()
            .filter(|health| health.root == root)
            .filter_map(|health| {
                let (trip, _) = lock(&health.state).disabled?;
                Some(json!({
                    "id": health.id,
                    "hash": health.hash,
                    "disabled": trip.as_str(),
                }))
            })
            .collect();
        plugins.sort_by(|a, b| {
            (a["id"].as_str(), a["hash"].as_str()).cmp(&(b["id"].as_str(), b["hash"].as_str()))
        });
        plugins
    };
    let document = json!({ "format_version": 1, "plugins": plugins });
    if let Ok(bytes) = serde_json::to_vec_pretty(&document) {
        send(Job::WriteHealth {
            path: file.to_owned(),
            bytes,
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use msime_script_runtime::{compile_text, Declaration, Hook, HookSet};
    use std::sync::atomic::{AtomicU64, Ordering};

    static NEXT_ID: AtomicU64 = AtomicU64::new(0);

    #[test]
    fn retaining_a_cache_entry_with_live_clones_keeps_its_memory_charge() {
        let id = NEXT_ID.fetch_add(1, Ordering::Relaxed);
        let root = PathBuf::from(format!("/script-cache-test-{id}"));
        let hash = format!("cache-test-{id}");
        let module = compile_text(
            r#"(module
                (memory (export "memory") 1)
                (func (export "msime_abi") (result i32) i32.const 1)
                (func (export "msime_on_event") (param i32) (result i32) i32.const 0))"#,
            Declaration {
                capabilities: Default::default(),
                hooks: HookSet::EMPTY.with(Hook::Commit),
                memory_pages: 1,
            },
        )
        .expect("test module compiles");
        let charged = 123usize;
        let before = BYTECODE_BYTES.fetch_add(charged, Ordering::AcqRel);
        lock(&CACHE).insert(
            hash.clone(),
            Cached {
                root: root.clone(),
                slot: Slot::Ready(module),
                bytecode: charged,
            },
        );

        let live_clone = match lookup(&hash) {
            Lookup::Ready(module) => module,
            _ => panic!("cache entry was not ready"),
        };
        retain(&root, &[]);
        assert_eq!(BYTECODE_BYTES.load(Ordering::Acquire), before + charged);
        assert!(matches!(lookup(&hash), Lookup::Ready(_)));

        drop(live_clone);
        retain(&root, &[]);
        assert_eq!(BYTECODE_BYTES.load(Ordering::Acquire), before);
        assert!(matches!(lookup(&hash), Lookup::Waiting));
    }
}
