//! The MSJPDT1 lemma dictionary (schemes-lang.md §5.7-§5.8, data-formats.md §8): tokens sorted by reading, a connection matrix and a string blob, all little-endian. 每个路径在进程里只读一次：`JapaneseDictionary::shared` 把读好的模型强引用留在进程级缓存里，会话结束也不放手，因为 Android 每进一个输入框都重建会话，原先只留 `Weak` 时每个新输入框的第一个假名都要在按键线程上重新校验 66 MB 并重建前缀索引。缓存每次取用都对一遍文件身份（长度、修改时间，unix 上加设备号和 inode），同一路径被 rename 换成新文件时读新的，文件已换或已删的条目在这次取用时丢掉（不取用就不检查，没有后台去盯文件）；读失败不缓存；宿主的清缓存动作（`Session::reset_cache`，iOS 键盘扩展在内存告警时调用）经 `JapaneseDictionary::release_shared` 把强引用整个放掉。被放掉或因满了被挤出去的模型若还有会话在用，缓存只留它的 `Weak`，再要同一路径时交回这一份，不在它还活着时另读一份。
//!
//! The file is mapped read-only, as japanese_sentence_decoder.cpp:101-125 did, so its 66 MB are clean, file-backed pages the system can evict under memory pressure (the iOS keyboard extension's limit) rather than dirty heap read on the first Japanese query. The mapping rests on the resource contract: `msime-japanese.dat` ships read-only in the resource bundle and a replacement arrives by rename, never by an in-place write, so a mapped inode keeps its bytes for as long as the dictionary lives (`replacing_a_model_file_never_alters_a_loaded_dictionary`). Access is by offset with unaligned little-endian loads, as the C++ `memcpy` did. A host without a file system (the browser) hands the bytes over instead (`JapaneseDictionary::preload`), and they live on the heap for as long as the dictionary does.

use std::collections::{BinaryHeap, HashMap, HashSet};
use std::ops::Deref;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex, MutexGuard, Weak};
use std::time::SystemTime;

use memmap2::Mmap;

#[cfg(test)]
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct JapaneseLemma {
    pub reading: String,
    pub surface: String,
    pub left_id: u16,
    pub right_id: u16,
    pub word_cost: i32,
    pub token_id: u32,
}

/// 文本借用词库已验证的字节；生命周期仅取决于词库，不依赖查询字符串。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) struct JapaneseLemmaRef<'a> {
    pub reading: &'a str,
    pub surface: &'a str,
    pub left_id: u16,
    pub right_id: u16,
    pub word_cost: i32,
    pub token_id: u32,
}

#[cfg(test)]
impl JapaneseLemmaRef<'_> {
    fn into_owned(self) -> JapaneseLemma {
        JapaneseLemma {
            reading: self.reading.to_owned(),
            surface: self.surface.to_owned(),
            left_id: self.left_id,
            right_id: self.right_id,
            word_cost: self.word_cost,
            token_id: self.token_id,
        }
    }
}

const MAGIC: &[u8; 8] = b"MSJPDT1\0";
const HEADER_SIZE: usize = 56;
const TOKEN_SIZE: usize = 20;
const MAX_TOKEN_COUNT: u32 = 2_000_000;
const MAX_STRING_SIZE: u64 = 1 << 32;
const MAX_CONNECTION_COUNT: u64 = 20_000_000;
/// Out-of-range ids cost this much, so a corrupt id can never look like a cheap transition.
const INVALID_CONNECTION_COST: i32 = 10_000;
/// Pending romaji such as `k` expands into several one-kana prefix queries, by far the widest ranges; the best this many of each first-kana group are kept at load.
const SHORT_PREFIX_CANDIDATE_COUNT: usize = 64;

/// One 20-byte packed token record (`<IHIHHHi`).
#[derive(Debug, Clone, Copy)]
struct Token {
    reading_offset: u32,
    reading_length: u16,
    surface_offset: u32,
    surface_length: u16,
    left_id: u16,
    right_id: u16,
    word_cost: i32,
}

/// The model's bytes: the read-only file mapping on platforms with a file system, or bytes the host handed over (the browser, which has none; see [`JapaneseDictionary::preload`]).
enum ModelBytes {
    Mapped(Mmap),
    Owned(Box<[u8]>),
}

impl Deref for ModelBytes {
    type Target = [u8];

    fn deref(&self) -> &[u8] {
        match self {
            ModelBytes::Mapped(map) => map,
            ModelBytes::Owned(bytes) => bytes,
        }
    }
}

/// Dictionaries the host handed over as bytes, by the path a provider will ask for. They stay until [`JapaneseDictionary::unload`]: a host without a file system has no other copy to load again.
static PRELOADED: LazyLock<Mutex<HashMap<PathBuf, Arc<JapaneseDictionary>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// 进程级缓存最多留几个路径的模型。实际只有一两个路径（资源目录里的那份、按需下载的资源包那份），上限防的是桌面上资源按代次换目录、旧代次文件还留在盘上时条目越攒越多。测试二进制里几百个用例各在自己的临时目录写模型并行跑，上限放宽，免得互相挤掉对方正在断言的条目。
const SHARED_MODEL_CAPACITY: usize = if cfg!(test) { 256 } else { 2 };

/// 判断同一路径上的模型文件是否换过：长度和修改时间，unix 上加设备号和 inode，Windows 上加创建时间。rename 进来的新文件 inode 一定不同，长度和修改时间也几乎一定不同。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct FileIdentity {
    len: u64,
    modified: Option<SystemTime>,
    #[cfg(unix)]
    device: u64,
    #[cfg(unix)]
    inode: u64,
    #[cfg(windows)]
    created: u64,
}

impl FileIdentity {
    fn of(metadata: &std::fs::Metadata) -> Self {
        #[cfg(unix)]
        use std::os::unix::fs::MetadataExt;
        #[cfg(windows)]
        use std::os::windows::fs::MetadataExt;
        Self {
            len: metadata.len(),
            modified: metadata.modified().ok(),
            #[cfg(unix)]
            device: metadata.dev(),
            #[cfg(unix)]
            inode: metadata.ino(),
            #[cfg(windows)]
            created: metadata.creation_time(),
        }
    }

    /// 路径上此刻那个普通文件的身份；不存在、不是普通文件（含符号链接）时为 `None`，与 `load` 的拒绝条件一致。
    fn at(path: &Path) -> Option<Self> {
        let metadata = std::fs::symlink_metadata(path).ok()?;
        metadata.file_type().is_file().then(|| Self::of(&metadata))
    }
}

struct SharedModel {
    path: PathBuf,
    identity: FileIdentity,
    model: Arc<JapaneseDictionary>,
}

/// 被 `release_shared` 放掉或因满了被挤出去的模型：缓存不再钉住它，但只要还有会话在用，再要同一路径时就交回这一份。
struct ReleasedModel {
    path: PathBuf,
    identity: FileIdentity,
    model: Weak<JapaneseDictionary>,
}

impl ReleasedModel {
    fn of(entry: &SharedModel) -> Self {
        Self {
            path: entry.path.clone(),
            identity: entry.identity,
            model: Arc::downgrade(&entry.model),
        }
    }
}

#[derive(Default)]
struct SharedModels {
    /// 强引用，最近放进来的排在最后。
    entries: Vec<SharedModel>,
    /// 弱引用：缓存放手了、可能还有会话在用的模型。没人用了或文件换了就在下一次取用时丢掉，所以条数不超过还活着的模型数。
    released: Vec<ReleasedModel>,
    /// `release_shared` 每放一次加一。读文件时不持锁，读到一半被放掉的结果按这个数判断，不再写回强引用。
    generation: u64,
}

impl SharedModels {
    /// 丢掉文件已经不在或被换掉的条目，以及已经没人用的弱引用。被丢掉的模型随最后一个持有它的会话释放，旧 inode 占的磁盘空间也跟着还回去。
    fn prune(&mut self) {
        self.entries
            .retain(|entry| FileIdentity::at(&entry.path) == Some(entry.identity));
        self.released.retain(|entry| {
            entry.model.strong_count() > 0 && FileIdentity::at(&entry.path) == Some(entry.identity)
        });
    }

    /// 强引用里 `path` 的模型。
    fn strong(&self, path: &Path) -> Option<Arc<JapaneseDictionary>> {
        self.entries
            .iter()
            .find(|entry| entry.path == path)
            .map(|entry| Arc::clone(&entry.model))
    }

    /// 先 `prune`，再找 `path` 的模型：强引用里有就用；没有但弱引用里那份还有会话在用，就把它重新放回强引用并交出去，不另读一份。
    fn current(&mut self, path: &Path, capacity: usize) -> Option<Arc<JapaneseDictionary>> {
        self.prune();
        if let Some(model) = self.strong(path) {
            return Some(model);
        }
        let position = self.released.iter().position(|entry| entry.path == path)?;
        let released = self.released.swap_remove(position);
        let model = released.model.upgrade()?;
        self.insert(
            SharedModel {
                path: released.path,
                identity: released.identity,
                model: Arc::clone(&model),
            },
            capacity,
        );
        Some(model)
    }

    /// 放进 `path` 的模型，替换它原先的条目；满了先把最早放进来的挤成弱引用。
    fn insert(&mut self, entry: SharedModel, capacity: usize) {
        self.entries.retain(|existing| existing.path != entry.path);
        self.released.retain(|existing| existing.path != entry.path);
        while !self.entries.is_empty() && self.entries.len() >= capacity {
            let evicted = self.entries.remove(0);
            self.released.push(ReleasedModel::of(&evicted));
        }
        self.entries.push(entry);
    }

    /// 强引用全部降为弱引用，代次加一。
    fn release(&mut self) {
        let entries = std::mem::take(&mut self.entries);
        self.released.extend(entries.iter().map(ReleasedModel::of));
        self.released.retain(|entry| entry.model.strong_count() > 0);
        self.generation = self.generation.wrapping_add(1);
    }
}

/// `JapaneseDictionary::shared` 读进来的模型，强引用，会话都结束了也留着。
static SHARED: LazyLock<Mutex<SharedModels>> =
    LazyLock::new(|| Mutex::new(SharedModels::default()));
/// 同一时刻只读一个模型文件：后台预热和第一个查询撞上时，后到的等先到的读完直接拿结果，不会把 66 MB 校验两遍。`SHARED` 本身不在读文件期间持有，`release_shared` 不会被一次读文件卡住。
static LOADING: Mutex<()> = Mutex::new(());
/// 正在后台预热的路径，重复切到日文时不再起第二个线程。
static WARMING: LazyLock<Mutex<HashSet<PathBuf>>> = LazyLock::new(|| Mutex::new(HashSet::new()));

fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
}

/// 动到进程级缓存、又要断言缓存内容的测试共用这把锁，免得 `release_shared` 在别的用例两次取用之间把缓存清掉。
#[cfg(test)]
pub(crate) static SHARED_CACHE_TEST_LOCK: Mutex<()> = Mutex::new(());

/// 每个路径真正读过几次文件，测试据此断言「只读一遍」。
#[cfg(test)]
static LOAD_COUNTS: LazyLock<Mutex<HashMap<PathBuf, usize>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

/// 测试让下一次读某个路径停在读文件之前：读的线程先和测试线程碰一次头，再等测试线程放行，测试借这段时间制造并发。
#[cfg(test)]
static LOAD_PAUSES: LazyLock<Mutex<HashMap<PathBuf, Arc<std::sync::Barrier>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

pub struct JapaneseDictionary {
    bytes: ModelBytes,
    token_offset: usize,
    token_count: usize,
    connection_offset: usize,
    connection_size: usize,
    string_offset: usize,
    /// First code point of a reading -> the best token ids of that group, sorted by (cost, id).
    short_prefix_index: HashMap<String, Vec<u32>>,
}

fn u16_at(bytes: &[u8], offset: usize) -> u16 {
    u16::from_le_bytes([bytes[offset], bytes[offset + 1]])
}

fn u32_at(bytes: &[u8], offset: usize) -> u32 {
    u32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("four bytes"))
}

fn i32_at(bytes: &[u8], offset: usize) -> i32 {
    i32::from_le_bytes(bytes[offset..offset + 4].try_into().expect("four bytes"))
}

fn u64_at(bytes: &[u8], offset: usize) -> u64 {
    u64::from_le_bytes(bytes[offset..offset + 8].try_into().expect("eight bytes"))
}

fn collect_query_ids<I>(ids: I, capacity: usize) -> Vec<u32>
where
    I: IntoIterator<Item = u32>,
{
    let mut collected = Vec::with_capacity(capacity);
    collected.extend(ids);
    collected
}

/// Keeps the `limit` cheapest ids by (cost, id), cheapest first: the C++ bounded max-heap, whose result is the same set in the same order because (cost, id) is a total order.
fn best_ids(mut ids: Vec<u32>, limit: usize, cost: impl Fn(u32) -> i32) -> Vec<u32> {
    let key = |id: &u32| (cost(*id), *id);
    if limit > 0 && ids.len() > limit {
        ids.select_nth_unstable_by_key(limit - 1, key);
    }
    ids.truncate(limit);
    ids.sort_unstable_by_key(key);
    ids
}

/// 扫描大读音范围时只保留成本最低的 ID；堆顶保存当前最差项，容量受结果限额约束。
/// 空范围在申请排名存储前返回。
fn best_ids_from_iter<I>(ids: I, limit: usize, cost: impl Fn(u32) -> i32) -> Vec<u32>
where
    I: IntoIterator<Item = u32>,
{
    if limit == 0 {
        return Vec::new();
    }
    let mut ids = ids.into_iter();
    let Some(first) = ids.next() else {
        return Vec::new();
    };
    let mut best: BinaryHeap<(i32, u32)> = BinaryHeap::with_capacity(limit);
    for id in std::iter::once(first).chain(ids) {
        let key = (cost(id), id);
        if best.len() < limit {
            best.push(key);
        } else if key < *best.peek().expect("non-empty bounded heap") {
            best.pop();
            best.push(key);
        }
    }
    // 成本已随 ID 保存，按已有键排序，不再次读取模型成本。
    let mut keys = best.into_vec();
    keys.sort_unstable();
    keys.into_iter().map(|(_, id)| id).collect()
}

impl JapaneseDictionary {
    /// `None` when the file is missing or fails any header, bounds or ordering check. 产品代码一律经 [`JapaneseDictionary::shared`] 取模型，这个不进缓存的直读只留给测试。
    #[cfg(test)]
    pub fn load(path: &Path) -> Option<JapaneseDictionary> {
        Self::load_with_identity(path).map(|(dictionary, _)| dictionary)
    }

    /// 同 [`JapaneseDictionary::load`]，另外给出映射的那个文件的身份：取自打开的文件句柄，所以就是被映射的 inode，不会和随后 rename 进来的文件混淆。
    #[allow(unsafe_code)]
    fn load_with_identity(path: &Path) -> Option<(JapaneseDictionary, FileIdentity)> {
        #[cfg(test)]
        {
            let pause = lock(&LOAD_PAUSES).remove(path);
            if let Some(pause) = pause {
                pause.wait();
                pause.wait();
            }
            *lock(&LOAD_COUNTS).entry(path.to_path_buf()).or_default() += 1;
        }
        if !std::fs::symlink_metadata(path).ok()?.file_type().is_file() {
            return None;
        }
        let file = crate::paths::open_file_no_follow(path).ok()?;
        let metadata = file.metadata().ok()?;
        // A directory or a file too short for the header is refused before anything is mapped.
        if !metadata.is_file() || metadata.len() < HEADER_SIZE as u64 {
            return None;
        }
        // SAFETY: a mapping is only sound while nothing changes the file underneath it. The model ships read-only with the resources and is replaced by rename, never written in place (module doc), so the mapped inode keeps its bytes for as long as the map lives.
        let bytes = unsafe { Mmap::map(&file) }.ok()?;
        let dictionary = Self::parse(ModelBytes::Mapped(bytes))?;
        Some((dictionary, FileIdentity::of(&metadata)))
    }

    /// The model from bytes already in memory, with the same checks as [`JapaneseDictionary::load`].
    pub fn from_bytes(bytes: Box<[u8]>) -> Option<JapaneseDictionary> {
        Self::parse(ModelBytes::Owned(bytes))
    }

    /// Makes `bytes` the dictionary [`JapaneseDictionary::shared`] answers for `path`, replacing any earlier one there; false, with nothing changed, when the bytes are not a valid model. For hosts without a file system (the browser), which download the model and hand it over before a Japanese session starts.
    pub fn preload(path: &Path, bytes: Box<[u8]>) -> bool {
        let Some(dictionary) = Self::from_bytes(bytes) else {
            return false;
        };
        PRELOADED
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .insert(path.to_path_buf(), Arc::new(dictionary));
        true
    }

    /// Forgets the dictionary preloaded for `path`; sessions that already hold it keep it until they end.
    pub fn unload(path: &Path) {
        PRELOADED
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
            .remove(path);
    }

    fn parse(bytes: ModelBytes) -> Option<JapaneseDictionary> {
        if bytes.len() < HEADER_SIZE || &bytes[..8] != MAGIC {
            return None;
        }
        let version = u32_at(&bytes, 8);
        let token_count = u32_at(&bytes, 12);
        let connection_size = u32_at(&bytes, 16);
        let token_offset = u64_at(&bytes, 24);
        let connection_offset = u64_at(&bytes, 32);
        let string_offset = u64_at(&bytes, 40);
        let string_size = u64_at(&bytes, 48);
        if version != 1
            || connection_size == 0
            || token_count > MAX_TOKEN_COUNT
            || string_size > MAX_STRING_SIZE
        {
            return None;
        }
        let size = bytes.len() as u64;
        let contains = |offset: u64, length: u64| {
            offset >= HEADER_SIZE as u64 && offset <= size && length <= size - offset
        };
        let connection_count = u64::from(connection_size) * u64::from(connection_size);
        if connection_count > MAX_CONNECTION_COUNT
            || !contains(token_offset, u64::from(token_count) * TOKEN_SIZE as u64)
            || !contains(connection_offset, connection_count * 2)
            || !contains(string_offset, string_size)
        {
            return None;
        }
        let mut dictionary = JapaneseDictionary {
            bytes,
            token_offset: token_offset as usize,
            token_count: token_count as usize,
            connection_offset: connection_offset as usize,
            connection_size: connection_size as usize,
            string_offset: string_offset as usize,
            short_prefix_index: HashMap::new(),
        };
        // Lemma text is handed out as `&str`; the builder only writes UTF-8, so a blob that is not is corrupt.
        let strings = dictionary
            .bytes
            .get(dictionary.string_offset..dictionary.string_offset + string_size as usize)?;
        let strings = std::str::from_utf8(strings).ok()?;
        for index in 0..dictionary.token_count {
            let token = dictionary.token_at(index);
            let reading_end = u64::from(token.reading_offset) + u64::from(token.reading_length);
            let surface_end = u64::from(token.surface_offset) + u64::from(token.surface_length);
            if reading_end > string_size
                || surface_end > string_size
                || usize::from(token.left_id) >= dictionary.connection_size
                || usize::from(token.right_id) >= dictionary.connection_size
            {
                return None;
            }
            for (start, end) in [
                (token.reading_offset as usize, reading_end as usize),
                (token.surface_offset as usize, surface_end as usize),
            ] {
                if !strings.is_char_boundary(start) || !strings.is_char_boundary(end) {
                    return None;
                }
            }
            // The file order is the search index; unsorted input would make every lookup silently wrong.
            if index > 0
                && dictionary.reading(&token) < dictionary.reading(&dictionary.token_at(index - 1))
            {
                return None;
            }
        }
        if dictionary.token_count == 0 {
            return None;
        }
        dictionary.short_prefix_index = dictionary.build_short_prefix_index()?;
        Some(dictionary)
    }

    /// Readings are sorted, so each first-code-point group is contiguous and one linear pass indexes them all. `None` for an empty reading.
    fn build_short_prefix_index(&self) -> Option<HashMap<String, Vec<u32>>> {
        let mut index = HashMap::new();
        let mut group_start = 0;
        while group_start < self.token_count {
            let first = self.reading(&self.token_at(group_start)).chars().next()?;
            let mut group_end = group_start;
            while group_end < self.token_count
                && self.reading(&self.token_at(group_end)).starts_with(first)
            {
                group_end += 1;
            }
            let ids = collect_query_ids(
                group_start as u32..group_end as u32,
                group_end - group_start,
            );
            let best = best_ids(ids, SHORT_PREFIX_CANDIDATE_COUNT, |id| self.cost_of(id));
            index.entry(first.to_string()).or_insert(best);
            group_start = group_end;
        }
        Some(index)
    }

    /// 每个路径在进程里共用一份模型，读好后留在进程级缓存里，最后一个会话结束也不放手（模块文档）。路径上的文件被换过（身份对不上）时读新的；文件不在或校验不过时答 `None`，不缓存这个失败，文件下次出现就能读到。为 `path` 预载过的模型优先于文件。
    pub fn shared(path: &Path) -> Option<Arc<JapaneseDictionary>> {
        if let Some(preloaded) = lock(&PRELOADED).get(path) {
            return Some(Arc::clone(preloaded));
        }
        if let Some(model) = lock(&SHARED).current(path, SHARED_MODEL_CAPACITY) {
            return Some(model);
        }
        let _loading = lock(&LOADING);
        // 等锁期间别的线程（预热或另一个会话）可能已经把它读好了。
        if let Some(model) = lock(&SHARED).current(path, SHARED_MODEL_CAPACITY) {
            return Some(model);
        }
        let generation = lock(&SHARED).generation;
        let (dictionary, identity) = Self::load_with_identity(path)?;
        let model = Arc::new(dictionary);
        let entry = SharedModel {
            path: path.to_path_buf(),
            identity,
            model: Arc::clone(&model),
        };
        let mut shared = lock(&SHARED);
        if shared.generation == generation {
            shared.insert(entry, SHARED_MODEL_CAPACITY);
        } else {
            // 读的时候缓存被清过：不钉住它，但这期间来要的会话仍拿同一份。
            let released = ReleasedModel::of(&entry);
            shared.released.push(released);
        }
        Some(model)
    }

    /// 只取已经就绪的共享模型，不获取加载锁，也不读取和校验模型内容。
    pub(crate) fn cached(path: &Path) -> Option<Arc<JapaneseDictionary>> {
        if let Some(preloaded) = lock(&PRELOADED).get(path) {
            return Some(Arc::clone(preloaded));
        }
        lock(&SHARED).current(path, SHARED_MODEL_CAPACITY)
    }

    /// 放掉进程级缓存对全部模型的强引用。正在用它的会话照常用到结束，这期间再要同一路径拿到的仍是这一份（不另读一份，内存告警时也不会因此多出一份）；没人用了它就随之释放，之后第一个查日文的会话重新读文件。宿主的清缓存动作经 `Session::reset_cache` 调到这里，iOS 键盘扩展在内存告警时靠它交出前缀索引和映射。预载的模型不受影响，它们只能由 [`JapaneseDictionary::unload`] 放掉。
    pub fn release_shared() {
        lock(&SHARED).release();
    }

    /// 在后台线程把 `path` 的模型读进进程级缓存，让切到日文之后的第一个假名不必在按键线程上等它。已经在缓存里、已经预载或正在预热时什么也不做。起不了线程（网页引擎没有线程）时也什么都不做，第一个查询照旧自己读。
    pub fn warm_up_in_background(path: &Path) {
        if lock(&PRELOADED).contains_key(path) {
            return;
        }
        if lock(&SHARED).current(path, SHARED_MODEL_CAPACITY).is_some() {
            return;
        }
        if !lock(&WARMING).insert(path.to_path_buf()) {
            return;
        }
        /// 预热线程无论怎样结束（包括 panic）都把路径从 `WARMING` 里拿掉。
        struct Warming(PathBuf);
        impl Drop for Warming {
            fn drop(&mut self) {
                lock(&WARMING).remove(&self.0);
            }
        }
        let warming = Warming(path.to_path_buf());
        // 起线程失败时闭包连同 `warming` 一起被丢掉，路径随之从 `WARMING` 里拿掉，第一个查询照旧自己读。
        let _ = std::thread::Builder::new()
            .name("msime-japanese-model".to_owned())
            .spawn(move || {
                let _ = Self::shared(&warming.0);
                drop(warming);
            });
    }

    /// `path` 的模型此刻是否被进程级缓存强引用着；只看不动，不会把弱引用里的那份放回去。
    #[cfg(test)]
    pub(crate) fn is_shared(path: &Path) -> bool {
        let mut shared = lock(&SHARED);
        shared.prune();
        shared.strong(path).is_some()
    }

    /// `path` 到目前为止真正读过几次文件。
    #[cfg(test)]
    pub(crate) fn load_count(path: &Path) -> usize {
        lock(&LOAD_COUNTS).get(path).copied().unwrap_or(0)
    }

    /// 让下一次读 `path` 停在读文件之前；返回的屏障先 `wait` 一次等读的线程到场，做完要并发的事再 `wait` 一次放它继续。
    #[cfg(test)]
    pub(crate) fn pause_next_load(path: &Path) -> Arc<std::sync::Barrier> {
        let pause = Arc::new(std::sync::Barrier::new(2));
        lock(&LOAD_PAUSES).insert(path.to_path_buf(), Arc::clone(&pause));
        pause
    }

    /// 等 `path` 的后台预热结束，最多等五秒。
    #[cfg(test)]
    pub(crate) fn wait_until_warmed(path: &Path) {
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(5);
        while lock(&WARMING).contains(path) {
            assert!(
                std::time::Instant::now() < deadline,
                "warm-up did not finish"
            );
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
    }

    /// 读音等于 `reading` 的词条，按 `(cost, id)` 取最便宜的 `limit` 条。
    #[cfg(test)]
    pub fn exact_lemmas(&self, reading: &str, limit: usize) -> Vec<JapaneseLemma> {
        self.exact_lemmas_with(reading, limit, |id| self.lemma(id))
    }

    #[cfg(test)]
    pub(crate) fn exact_lemma_views(
        &self,
        reading: &str,
        limit: usize,
    ) -> Vec<JapaneseLemmaRef<'_>> {
        self.exact_lemmas_with(reading, limit, |id| self.lemma_ref(id))
    }

    /// 按原成本顺序同步访问精确词条，视图只借用词库，不收集结果向量。
    pub(crate) fn for_each_exact_lemma_view<'a>(
        &'a self,
        reading: &str,
        limit: usize,
        mut visit: impl FnMut(JapaneseLemmaRef<'a>),
    ) {
        // 复用原筛选和排序，映射为零大小的 `()` 不申请结果元素存储。
        self.exact_lemmas_with(reading, limit, |id| visit(self.lemma_ref(id)));
    }

    /// 与 [`Self::for_each_exact_lemma_view`] 同样按 `(cost, id)` 访问最便宜的 `limit` 条，排名缓冲由调用方跨查询复用。列整组同音词时限额远大于读音组，按限额预留的排名堆每次都要申请；这里只放读音组本身，热查询不申请。
    pub(crate) fn for_each_exact_lemma_view_reusing<'a>(
        &'a self,
        reading: &str,
        limit: usize,
        ranking: &mut Vec<(i32, u32)>,
        mut visit: impl FnMut(JapaneseLemmaRef<'a>),
    ) {
        ranking.clear();
        if reading.is_empty() || limit == 0 {
            return;
        }
        let start = self.lower_bound(reading);
        ranking.extend(
            (start..self.token_count)
                .map(|index| (index, self.token_at(index)))
                .take_while(|(_, token)| self.reading(token) == reading)
                .map(|(index, token)| (token.word_cost, index as u32)),
        );
        if ranking.len() > limit {
            ranking.select_nth_unstable(limit - 1);
            ranking.truncate(limit);
        }
        ranking.sort_unstable();
        for &(_, id) in ranking.iter() {
            visit(self.lemma_ref(id));
        }
    }

    fn exact_lemmas_with<T>(
        &self,
        reading: &str,
        limit: usize,
        build: impl FnMut(u32) -> T,
    ) -> Vec<T> {
        if reading.is_empty() || limit == 0 {
            return Vec::new();
        }
        let start = self.lower_bound(reading);
        let ids = (start..self.token_count)
            .take_while(|&index| self.reading(&self.token_at(index)) == reading)
            .map(|index| index as u32);
        best_ids_from_iter(ids, limit, |id| self.cost_of(id))
            .into_iter()
            .map(build)
            .collect()
    }

    /// 读音以 `prefix` 开头的最便宜词条；单码点前缀优先使用预计算索引。
    #[cfg(test)]
    pub fn prefix_lemmas(&self, prefix: &str, limit: usize) -> Vec<JapaneseLemma> {
        self.prefix_lemmas_with(prefix, limit, |id| self.lemma(id))
    }

    #[cfg(test)]
    pub(crate) fn prefix_lemma_views(
        &self,
        prefix: &str,
        limit: usize,
    ) -> Vec<JapaneseLemmaRef<'_>> {
        self.prefix_lemmas_with(prefix, limit, |id| self.lemma_ref(id))
    }

    /// 按成本顺序访问前缀词条，避免为只消费一次的结果建立视图向量。
    pub(crate) fn for_each_prefix_lemma_view<'a>(
        &'a self,
        prefix: &str,
        limit: usize,
        mut visit: impl FnMut(JapaneseLemmaRef<'a>),
    ) {
        // 复用同一套筛选与排序；闭包返回的 `()` 是零大小类型，`Vec<()>` 不申请结果存储。
        self.prefix_lemmas_with(prefix, limit, |id| visit(self.lemma_ref(id)));
    }

    fn prefix_lemmas_with<T>(
        &self,
        prefix: &str,
        limit: usize,
        mut build: impl FnMut(u32) -> T,
    ) -> Vec<T> {
        if prefix.is_empty() || limit == 0 {
            return Vec::new();
        }
        if let Some(cached) = self.short_prefix_index.get(prefix) {
            if limit <= SHORT_PREFIX_CANDIDATE_COUNT {
                return cached.iter().take(limit).map(|&id| build(id)).collect();
            }
        }
        let start = self.lower_bound(prefix);
        let ids = (start..self.token_count)
            .take_while(|&index| self.reading(&self.token_at(index)).starts_with(prefix))
            .map(|index| index as u32);
        best_ids_from_iter(ids, limit, |id| self.cost_of(id))
            .into_iter()
            .map(build)
            .collect()
    }

    /// Token ids whose reading starts with `prefix` followed by one of `next_kana`.
    /// Each suffix is a contiguous sorted range, so querying those ranges avoids
    /// scanning unrelated readings in the whole `prefix` group. Overlapping
    /// suffixes are deduplicated before ranking.
    #[cfg(test)]
    fn continuing_candidate_ids(&self, prefix: &str, next_kana: &[&str]) -> Vec<u32> {
        let mut matches = Vec::new();
        for kana in next_kana {
            if kana.is_empty() {
                for index in self.lower_bound(prefix)..self.token_count {
                    let reading = self.reading(&self.token_at(index));
                    let Some(remaining) = reading.strip_prefix(prefix) else {
                        break;
                    };
                    if !remaining.is_empty() {
                        matches.push(index as u32);
                    }
                }
                continue;
            }
            let mut query = String::with_capacity(prefix.len() + kana.len());
            query.push_str(prefix);
            query.push_str(kana);
            let start = self.lower_bound(&query);
            for index in start..self.token_count {
                if !self.reading(&self.token_at(index)).starts_with(&query) {
                    break;
                }
                matches.push(index as u32);
            }
        }
        matches.sort_unstable();
        matches.dedup();
        matches
    }

    /// 读音严格长于 `prefix`，剩余部分以 `next_kana` 中任一项开头。
    ///
    /// 原参考实现在等于前缀的读音处结束扫描，导致前缀本身是词时无结果；这里跳过等长读音继续扫描。
    #[cfg(test)]
    pub fn prefix_lemmas_continuing(
        &self,
        prefix: &str,
        next_kana: &[&str],
        limit: usize,
    ) -> Vec<JapaneseLemma> {
        self.prefix_lemmas_continuing_with(prefix, next_kana, limit, |id| self.lemma(id))
    }

    #[cfg(test)]
    pub(crate) fn continuing_lemma_views(
        &self,
        prefix: &str,
        next_kana: &[&str],
        limit: usize,
    ) -> Vec<JapaneseLemmaRef<'_>> {
        self.prefix_lemmas_continuing_with(prefix, next_kana, limit, |id| self.lemma_ref(id))
    }

    /// 按原排名顺序消费继续补全词条，不为单次消费建立视图结果向量。
    pub(crate) fn for_each_continuing_lemma_view<'a>(
        &'a self,
        prefix: &str,
        next_kana: &[&str],
        limit: usize,
        mut visit: impl FnMut(JapaneseLemmaRef<'a>),
    ) {
        // 与前缀访问共用零大小结果模式，拼接键、排名与重复 ID 仍由原查询处理。
        self.prefix_lemmas_continuing_with(prefix, next_kana, limit, |id| {
            visit(self.lemma_ref(id))
        });
    }

    fn prefix_lemmas_continuing_with<T>(
        &self,
        prefix: &str,
        next_kana: &[&str],
        limit: usize,
        build: impl FnMut(u32) -> T,
    ) -> Vec<T> {
        if prefix.is_empty() || next_kana.is_empty() || limit == 0 {
            return Vec::new();
        }
        let mut best: BinaryHeap<(i32, u32)> = BinaryHeap::new();
        let mut consider = |id: u32| {
            let key = (self.cost_of(id), id);
            if best.len() < limit {
                if best.is_empty() {
                    best.reserve_exact(limit);
                }
                best.push(key);
            } else if key < *best.peek().expect("non-empty bounded heap") {
                best.pop();
                best.push(key);
            }
        };
        let mut query = String::new();
        for kana in next_kana {
            if kana.is_empty() {
                for index in self.lower_bound(prefix)..self.token_count {
                    let reading = self.reading(&self.token_at(index));
                    let Some(remaining) = reading.strip_prefix(prefix) else {
                        break;
                    };
                    if !remaining.is_empty() {
                        consider(index as u32);
                    }
                }
                continue;
            }
            if query.is_empty() {
                // 只在首个非空后缀预留最长键容量，公共前缀在整个查询中保留。
                let suffix_capacity = if next_kana.len() == 1 {
                    kana.len()
                } else {
                    next_kana
                        .iter()
                        .map(|suffix| suffix.len())
                        .max()
                        .unwrap_or(0)
                };
                query.reserve_exact(prefix.len() + suffix_capacity);
                query.push_str(prefix);
            }
            query.truncate(prefix.len());
            query.push_str(kana);
            let start = self.lower_bound(&query);
            for index in start..self.token_count {
                if !self.reading(&self.token_at(index)).starts_with(&query) {
                    break;
                }
                consider(index as u32);
            }
        }
        // 已保存的键同时决定筛选和输出顺序；保留重叠后缀产生的重复 ID。
        let mut keys = best.into_vec();
        keys.sort_unstable();
        keys.into_iter().map(|(_, id)| id).map(build).collect()
    }

    /// 10000 for an out-of-range id.
    pub fn connection_cost(&self, right_id: u16, left_id: u16) -> i32 {
        let (right, left) = (usize::from(right_id), usize::from(left_id));
        if right >= self.connection_size || left >= self.connection_size {
            return INVALID_CONNECTION_COST;
        }
        let index = right * self.connection_size + left;
        i32::from(u16_at(&self.bytes, self.connection_offset + index * 2) as i16)
    }

    /// 按文件里的读音顺序浏览，先排除纯标点和符号再分页，只为当前页分配字符串；同读音的不同词性保留为独立模型行。
    pub(crate) fn browse(
        &self,
        query: &str,
        offset: usize,
        limit: usize,
    ) -> Vec<crate::dictionary::DictRow> {
        let reading: String = query
            .chars()
            .map(|ch| {
                if ('ァ'..='ヶ').contains(&ch) {
                    char::from_u32(ch as u32 - 0x60).expect("平片假名对应码点")
                } else {
                    ch
                }
            })
            .collect();
        (0..self.token_count)
            .map(|id| self.lemma_ref(id as u32))
            .filter(|lemma| {
                lemma.surface.chars().any(|ch| {
                    // 长音与迭代记号具有字母属性，单独出现时仍不是可浏览的词条。
                    ch.is_alphanumeric()
                        && !matches!(
                            ch,
                            '々' | '〻' | '\u{3031}'
                                ..='\u{3035}' | 'ゝ' | 'ゞ' | 'ヽ' | 'ヾ' | 'ー' | 'ｰ'
                        )
                })
            })
            .filter(|lemma| {
                query.is_empty()
                    || lemma.reading.starts_with(&reading)
                    || lemma.surface.contains(query)
            })
            .skip(offset)
            .take(limit)
            .map(|lemma| crate::dictionary::DictRow {
                key: lemma.reading.to_owned(),
                value: lemma.surface.to_owned(),
                weight: i64::from(lemma.word_cost),
            })
            .collect()
    }

    fn token_at(&self, index: usize) -> Token {
        let offset = self.token_offset + index * TOKEN_SIZE;
        let bytes = &self.bytes;
        Token {
            reading_offset: u32_at(bytes, offset),
            reading_length: u16_at(bytes, offset + 4),
            surface_offset: u32_at(bytes, offset + 6),
            surface_length: u16_at(bytes, offset + 10),
            left_id: u16_at(bytes, offset + 12),
            right_id: u16_at(bytes, offset + 14),
            word_cost: i32_at(bytes, offset + 16),
        }
    }

    /// Load checked that every token range lies inside the blob on character boundaries of valid UTF-8, so the conversion cannot fail.
    fn text(&self, offset: u32, length: u16) -> &str {
        let start = self.string_offset + offset as usize;
        std::str::from_utf8(&self.bytes[start..start + usize::from(length)])
            .expect("token strings are validated at load")
    }

    fn reading(&self, token: &Token) -> &str {
        self.text(token.reading_offset, token.reading_length)
    }

    fn cost_of(&self, id: u32) -> i32 {
        self.token_at(id as usize).word_cost
    }

    /// First token whose reading is not less than `reading`, bytewise as the C++ `string_view` compare.
    fn lower_bound(&self, reading: &str) -> usize {
        let (mut first, mut last) = (0, self.token_count);
        while first < last {
            let middle = first + (last - first) / 2;
            if self.reading(&self.token_at(middle)) < reading {
                first = middle + 1;
            } else {
                last = middle;
            }
        }
        first
    }

    #[cfg(test)]
    fn lemma(&self, id: u32) -> JapaneseLemma {
        self.lemma_ref(id).into_owned()
    }

    fn lemma_ref(&self, id: u32) -> JapaneseLemmaRef<'_> {
        let token = self.token_at(id as usize);
        JapaneseLemmaRef {
            reading: self.reading(&token),
            surface: self.text(token.surface_offset, token.surface_length),
            left_id: token.left_id,
            right_id: token.right_id,
            word_cost: token.word_cost,
            token_id: id,
        }
    }
}

/// Builds MSJPDT1 files for the tests of this module and its callers, the way `build_sentence_model.py` lays them out.
#[cfg(test)]
pub(crate) mod test_model {
    /// `(reading, surface, left_id, right_id, cost)`; the caller keeps readings in byte order, as the loader requires.
    pub type Entry<'a> = (&'a str, &'a str, u16, u16, i32);

    pub fn bytes(entries: &[Entry<'_>], connection_size: u32, connection: &[i16]) -> Vec<u8> {
        let mut strings = Vec::<u8>::new();
        let mut interned = std::collections::HashMap::<String, u32>::new();
        let mut intern = |text: &str, strings: &mut Vec<u8>| -> (u32, u16) {
            let offset = *interned.entry(text.to_owned()).or_insert_with(|| {
                let offset = strings.len() as u32;
                strings.extend_from_slice(text.as_bytes());
                offset
            });
            (offset, text.len() as u16)
        };
        let mut tokens = Vec::new();
        for &(reading, surface, left, right, cost) in entries {
            let reading = intern(reading, &mut strings);
            let surface = intern(surface, &mut strings);
            tokens.extend_from_slice(&reading.0.to_le_bytes());
            tokens.extend_from_slice(&reading.1.to_le_bytes());
            tokens.extend_from_slice(&surface.0.to_le_bytes());
            tokens.extend_from_slice(&surface.1.to_le_bytes());
            tokens.extend_from_slice(&left.to_le_bytes());
            tokens.extend_from_slice(&right.to_le_bytes());
            tokens.extend_from_slice(&cost.to_le_bytes());
        }
        let token_offset = 56u64;
        let connection_offset = token_offset + tokens.len() as u64;
        let string_offset = connection_offset + connection.len() as u64 * 2;
        let mut file = Vec::new();
        file.extend_from_slice(b"MSJPDT1\0");
        file.extend_from_slice(&1u32.to_le_bytes());
        file.extend_from_slice(&(entries.len() as u32).to_le_bytes());
        file.extend_from_slice(&connection_size.to_le_bytes());
        file.extend_from_slice(&0u32.to_le_bytes());
        for value in [
            token_offset,
            connection_offset,
            string_offset,
            strings.len() as u64,
        ] {
            file.extend_from_slice(&value.to_le_bytes());
        }
        file.extend_from_slice(&tokens);
        for cost in connection {
            file.extend_from_slice(&cost.to_le_bytes());
        }
        file.extend_from_slice(&strings);
        file
    }

    /// `test_engine_smoke.cpp:159-198`: two lemmas and a one-id matrix, enough to drive the provider's prefix-lemma branch.
    pub fn smoke() -> Vec<u8> {
        bytes(
            &[("かんじ", "漢字", 0, 0, 1000), ("しし", "四肢", 0, 0, 1200)],
            1,
            &[0],
        )
    }

    /// `test_runtime_isolation.cpp` `make_japanese_model`: one かな lemma with the given surface.
    pub fn single(surface: &str) -> Vec<u8> {
        bytes(&[("かな", surface, 0, 0, 100)], 1, &[0])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The same read-only mapping type `load` produces, over anonymous memory, so the checks run on in-memory bytes.
    fn mapped(bytes: &[u8]) -> Mmap {
        let mut map = memmap2::MmapMut::map_anon(bytes.len()).expect("anonymous map");
        map.copy_from_slice(bytes);
        map.make_read_only().expect("read-only map")
    }

    fn parse(bytes: impl AsRef<[u8]>) -> Option<JapaneseDictionary> {
        JapaneseDictionary::parse(ModelBytes::Mapped(mapped(bytes.as_ref())))
    }

    fn parsed(bytes: Vec<u8>) -> JapaneseDictionary {
        parse(bytes).expect("valid model")
    }

    fn surfaces(lemmas: &[JapaneseLemma]) -> Vec<&str> {
        lemmas.iter().map(|lemma| lemma.surface.as_str()).collect()
    }

    #[test]
    fn browse_filters_reading_and_surface_before_paging() {
        let dictionary = parsed(test_model::bytes(
            &[
                ("かな", "仮名", 0, 0, 400),
                ("かな", "仮名", 0, 0, 500),
                ("にほん", "日本", 0, 0, -10),
                ("にほんご", "日本語", 0, 0, 600),
                ("わいるど", "語%_", 0, 0, 700),
            ],
            1,
            &[0],
        ));
        assert_eq!(
            dictionary
                .browse("", 1, 2)
                .iter()
                .map(|row| row.value.as_str())
                .collect::<Vec<_>>(),
            ["仮名", "日本"]
        );
        for query in ["日本", "にほん", "ニホン"] {
            let rows = dictionary.browse(query, 1, 10);
            assert_eq!(rows.len(), 1);
            assert_eq!(rows[0].value, "日本語");
        }
        assert_eq!(dictionary.browse("日本", 0, 1)[0].weight, -10);
        assert_eq!(dictionary.browse("%_", 0, 10)[0].value, "語%_");
        assert!(dictionary.browse("不存在", 0, 10).is_empty());
        assert!(dictionary.browse("", 5, 10).is_empty());
    }

    #[test]
    fn browse_omits_symbol_only_surfaces_before_offset_without_changing_conversion() {
        let symbols = [
            " ", "!", "$", "&", "-", ".", "。", "・", "々", "ゝ", "ー", "ｰ", "🙂",
        ];
        let mut entries: Vec<_> = symbols
            .iter()
            .map(|symbol| (*symbol, *symbol, 0, 0, 10))
            .collect();
        entries.extend([
            ("えーびー", "A/B", 0, 0, 20),
            ("かな", "仮名", 0, 0, 30),
            ("かな", "仮名", 0, 0, 40),
            ("きごう", "※", 0, 0, 50),
            ("こーひー", "コーヒー", 0, 0, 60),
            ("ぜろ", "007", 0, 0, 70),
            ("ときどき", "時々", 0, 0, 80),
            ("にほん", "日本", 0, 0, 90),
        ]);
        entries.sort_by_key(|entry| entry.0);
        let dictionary = parsed(test_model::bytes(&entries, 1, &[0]));
        let rows = dictionary.browse("", 0, 100);
        assert_eq!(
            rows.iter()
                .map(|row| row.value.as_str())
                .collect::<Vec<_>>(),
            ["A/B", "仮名", "仮名", "コーヒー", "007", "時々", "日本"]
        );
        assert_eq!(
            dictionary
                .browse("", 3, 2)
                .iter()
                .map(|row| row.value.as_str())
                .collect::<Vec<_>>(),
            ["コーヒー", "007"]
        );
        assert!(dictionary.browse("", 7, 2).is_empty());
        for query in ["!", "きごう", "※", "🙂"] {
            assert!(dictionary.browse(query, 0, 10).is_empty(), "{query}");
        }
        for query in ["こー", "コー", "コーヒー", "ー"] {
            assert_eq!(dictionary.browse(query, 0, 10)[0].value, "コーヒー");
        }
        assert_eq!(dictionary.browse("/", 0, 10)[0].value, "A/B");
        assert_eq!(dictionary.exact_lemmas("!", 1)[0].surface, "!");
        assert_eq!(dictionary.exact_lemmas("きごう", 1)[0].surface, "※");
    }

    fn assert_borrowed_lemmas(
        dictionary: &JapaneseDictionary,
        views: &[JapaneseLemmaRef<'_>],
        owned: &[JapaneseLemma],
        view_allocations: usize,
        owned_allocations: usize,
    ) {
        assert_eq!(views.len(), owned.len());
        let start = dictionary.bytes.as_ptr() as usize;
        let end = start + dictionary.bytes.len();
        for (view, owned) in views.iter().zip(owned) {
            assert_eq!(view.into_owned(), *owned);
            for text in [view.reading, view.surface] {
                let pointer = text.as_ptr() as usize;
                assert!(pointer >= start && pointer + text.len() <= end);
            }
        }
        assert_eq!(owned_allocations, view_allocations + 2 * views.len());
    }

    #[test]
    fn borrowed_lemma_queries_preserve_cost_order_without_text_allocations() {
        let surfaces: Vec<_> = (0..70).map(|index| format!("語{index:02}")).collect();
        let entries: Vec<_> = surfaces
            .iter()
            .enumerate()
            .map(|(index, surface)| ("かな", surface.as_str(), 0, 0, (index % 7) as i32))
            .collect();
        let dictionary = parsed(test_model::bytes(&entries, 1, &[0]));
        let mut expected_ids: Vec<u32> = (0..70).collect();
        expected_ids.sort_unstable_by_key(|id| (id % 7, *id));
        for limit in [0, 1, 2, 24, 64, 65, 70, 100] {
            let (views, view_allocations) = crate::ime::personal_rerank::allocations::count(|| {
                dictionary.exact_lemma_views("かな", limit)
            });
            let (owned, owned_allocations) =
                crate::ime::personal_rerank::allocations::count(|| {
                    dictionary.exact_lemmas("かな", limit)
                });
            assert_borrowed_lemmas(
                &dictionary,
                &views,
                &owned,
                view_allocations,
                owned_allocations,
            );
            assert_eq!(
                views.iter().map(|view| view.token_id).collect::<Vec<_>>(),
                expected_ids[..limit.min(70)]
            );
            assert_eq!(view_allocations, if limit == 0 { 0 } else { 2 });
            for prefix in ["か", "かな"] {
                let (views, view_allocations) =
                    crate::ime::personal_rerank::allocations::count(|| {
                        dictionary.prefix_lemma_views(prefix, limit)
                    });
                let (owned, owned_allocations) =
                    crate::ime::personal_rerank::allocations::count(|| {
                        dictionary.prefix_lemmas(prefix, limit)
                    });
                assert_borrowed_lemmas(
                    &dictionary,
                    &views,
                    &owned,
                    view_allocations,
                    owned_allocations,
                );
                assert_eq!(
                    views.iter().map(|view| view.token_id).collect::<Vec<_>>(),
                    expected_ids[..limit.min(70)]
                );
                assert_eq!(
                    view_allocations,
                    if limit == 0 {
                        0
                    } else if prefix == "か" && limit <= 64 {
                        1
                    } else {
                        2
                    }
                );
            }
            for next in ["な", ""] {
                let (views, view_allocations) =
                    crate::ime::personal_rerank::allocations::count(|| {
                        dictionary.continuing_lemma_views("か", &[next], limit)
                    });
                let (owned, owned_allocations) =
                    crate::ime::personal_rerank::allocations::count(|| {
                        dictionary.prefix_lemmas_continuing("か", &[next], limit)
                    });
                assert_borrowed_lemmas(
                    &dictionary,
                    &views,
                    &owned,
                    view_allocations,
                    owned_allocations,
                );
                assert_eq!(
                    views.iter().map(|view| view.token_id).collect::<Vec<_>>(),
                    expected_ids[..limit.min(70)]
                );
                assert_eq!(
                    view_allocations,
                    if limit == 0 {
                        0
                    } else {
                        2 + usize::from(!next.is_empty())
                    }
                );
            }
        }
    }

    #[test]
    fn prefix_lemma_views_can_be_consumed_without_result_vector() {
        let dictionary = parsed(test_model::bytes(
            &[
                ("かな", "仮名", 0, 0, 500),
                ("かなこ", "加奈子", 0, 0, 600),
                ("かなで", "奏で", 0, 0, 700),
            ],
            1,
            &[0],
        ));
        let visits = std::cell::Cell::new(0);
        let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
            dictionary.for_each_prefix_lemma_view("かな", 16, |_| {
                visits.set(visits.get() + 1);
            });
        });
        assert_eq!(visits.get(), 3);
        assert_eq!(allocations, 1);
    }

    #[test]
    fn streamed_prefixes_keep_ties_limits_missing_queries_and_dictionary_borrows() {
        let surfaces: Vec<_> = (0..70).map(|index| format!("語{index:02}")).collect();
        let entries: Vec<_> = surfaces
            .iter()
            .enumerate()
            .map(|(index, surface)| ("かな", surface.as_str(), 0, 0, index as i32 % 7 - 3))
            .collect();
        let dictionary = parsed(test_model::bytes(&entries, 1, &[0]));
        let mut expected: Vec<u32> = (0..70).collect();
        expected.sort_unstable_by_key(|id| (id % 7, *id));
        for prefix in ["", "く", "か", "かな"] {
            for limit in [0, 1, 2, 24, 64, 65, 70, 100] {
                let mut views = [None; 70];
                let mut used = 0;
                let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
                    dictionary.for_each_prefix_lemma_view(prefix, limit, |view| {
                        views[used] = Some(view);
                        used += 1;
                    });
                });
                let missing = prefix.is_empty() || prefix == "く" || limit == 0;
                let ids = if missing {
                    &[][..]
                } else {
                    &expected[..limit.min(70)]
                };
                assert_eq!(used, ids.len());
                for (view, &id) in views[..used].iter().zip(ids) {
                    assert_eq!(view.unwrap(), dictionary.lemma_ref(id));
                }
                assert_eq!(
                    allocations,
                    usize::from(!missing && (prefix != "か" || limit > 64))
                );
            }
        }
        let mut saved = None;
        {
            // 查询文本释放后，保存的视图仍只依赖词库。
            let query = String::from("かな");
            dictionary.for_each_prefix_lemma_view(&query, 1, |view| saved = Some(view));
        }
        assert_eq!(saved.unwrap(), dictionary.lemma_ref(expected[0]));
    }

    #[test]
    fn borrowed_queries_keep_empty_missing_and_overlapping_suffix_behavior() {
        let dictionary = parsed(test_model::bytes(
            &[
                ("か", "蚊", 0, 0, 1),
                ("かな", "仮名", 0, 0, 10),
                ("かない", "家内", 0, 0, 20),
                ("かん", "漢", 0, 0, 30),
            ],
            1,
            &[0],
        ));
        for query in ["", "く"] {
            assert!(dictionary.exact_lemma_views(query, 8).is_empty());
            assert!(dictionary.prefix_lemma_views(query, 8).is_empty());
            assert!(dictionary
                .continuing_lemma_views(query, &["な"], 8)
                .is_empty());
        }
        assert!(dictionary.continuing_lemma_views("か", &[], 8).is_empty());
        let next = ["な", "ない", "ん"];
        let views = dictionary.continuing_lemma_views("か", &next, 8);
        let owned = dictionary.prefix_lemmas_continuing("か", &next, 8);
        assert_eq!(
            views.iter().map(|view| view.token_id).collect::<Vec<_>>(),
            [1, 2, 2, 3]
        );
        assert_eq!(
            views
                .iter()
                .map(|view| view.into_owned())
                .collect::<Vec<_>>(),
            owned
        );
        let from_temporary_query = {
            let query = String::from("かな");
            dictionary.exact_lemma_views(&query, 8)
        };
        assert_eq!(from_temporary_query[0].surface, "仮名");
        assert_eq!(
            dictionary
                .continuing_lemma_views("か", &[""], 8)
                .iter()
                .map(|view| view.token_id)
                .collect::<Vec<_>>(),
            [1, 2, 3]
        );
    }

    #[test]
    fn empty_ranked_lookups_do_not_allocate_heap_storage() {
        let dictionary = parsed(test_model::bytes(&[("かな", "仮名", 0, 0, 500)], 1, &[0]));
        for query in ["あ", "か", "かに", "漢", "😀", "ん"] {
            let (exact, allocations) = crate::ime::personal_rerank::allocations::count(|| {
                dictionary.exact_lemma_views(query, 24)
            });
            assert!(exact.is_empty());
            eprintln!("日文 exact 未命中排名分配：{allocations}");
            assert_eq!(allocations, 0, "未命中不应分配排名堆");
        }
    }

    #[test]
    fn empty_prefix_lookups_do_not_allocate_heap_storage() {
        let dictionary = parsed(test_model::bytes(&[("かな", "仮名", 0, 0, 500)], 1, &[0]));
        for prefix in ["あ", "かに", "漢", "😀", "ん"] {
            let (views, allocations) = crate::ime::personal_rerank::allocations::count(|| {
                dictionary.prefix_lemma_views(prefix, 24)
            });
            assert!(views.is_empty());
            eprintln!("日文 prefix 未命中排名分配：{allocations}");
            assert_eq!(allocations, 0, "未命中不应分配排名堆");
        }
    }

    #[test]
    fn empty_continuing_lookups_do_not_allocate_heap_storage() {
        let dictionary = parsed(test_model::bytes(&[("かな", "仮名", 0, 0, 500)], 1, &[0]));
        for (prefix, next) in [("か", "に"), ("あ", "な"), ("かな", "")] {
            let (views, allocations) = crate::ime::personal_rerank::allocations::count(|| {
                dictionary.continuing_lemma_views(prefix, &[next], 24)
            });
            assert!(views.is_empty());
            eprintln!("日文 continuing 未命中分配：{allocations}");
            assert_eq!(
                allocations,
                usize::from(!next.is_empty()),
                "只应保留非空后缀的查询字符串分配"
            );
        }
    }

    #[test]
    fn ranked_lookup_skips_zero_limit_and_empty_costs() {
        let ids = std::iter::from_fn(|| panic!("零限额不应读取候选"));
        let (result, allocations) = crate::ime::personal_rerank::allocations::count(|| {
            best_ids_from_iter(ids, 0, |_| panic!("零限额不应读取成本"))
        });
        assert!(result.is_empty());
        assert_eq!(allocations, 0);
        let (result, allocations) = crate::ime::personal_rerank::allocations::count(|| {
            best_ids_from_iter([], 24, |_| panic!("空候选不应读取成本"))
        });
        assert!(result.is_empty());
        assert_eq!(allocations, 0);
    }

    #[test]
    fn ranked_lookup_preserves_first_item_ties_and_replacement() {
        let costs = [-10, 30, -10, -20, 50, 0];
        let expected = [3, 0, 2, 5, 1, 4];
        for limit in [1, 2, 3, 6, 24] {
            let visited = std::cell::RefCell::new(Vec::new());
            let ids = [0, 4, 1, 2, 5, 3]
                .into_iter()
                .inspect(|id| visited.borrow_mut().push(*id));
            let result = best_ids_from_iter(ids, limit, |id| costs[id as usize]);
            assert_eq!(result, expected[..limit.min(expected.len())]);
            assert_eq!(*visited.borrow(), [0, 4, 1, 2, 5, 3]);
        }
        assert_eq!(best_ids_from_iter([0], 24, |_| -10), [0]);
    }

    #[test]
    fn continuing_lookup_reserves_after_an_initial_suffix_miss() {
        let dictionary = parsed(test_model::bytes(
            &[
                ("かな", "仮名", 0, 0, 500),
                ("かに", "蟹", 0, 0, -10),
                ("かに", "下荷", 0, 0, -10),
            ],
            1,
            &[0],
        ));
        for limit in [1, 2, 24] {
            let (views, allocations) = crate::ime::personal_rerank::allocations::count(|| {
                dictionary.continuing_lemma_views("か", &["ん", "に", "な"], limit)
            });
            assert_eq!(
                views.iter().map(|view| view.token_id).collect::<Vec<_>>(),
                [1, 2, 0][..limit.min(3)]
            );
            eprintln!("日文三个后缀 continuing 命中分配：{allocations}");
            assert_eq!(allocations, 3, "一份查询键、排名堆及结果向量");
        }
    }

    #[test]
    fn scanned_query_ids_reserve_the_lookup_limit() {
        let ids = collect_query_ids([1_u32, 2, 3], 8);
        assert_eq!(ids, [1, 2, 3]);
        assert!(ids.capacity() >= 8);
    }

    // test_runtime_isolation.cpp:105-139.
    #[test]
    fn replacing_a_model_file_never_alters_a_loaded_dictionary() {
        let root = tempfile::tempdir().expect("temporary directory");
        let path = root.path().join("mapped-japanese.dat");
        std::fs::write(&path, test_model::single("甲")).expect("write model");
        let original = JapaneseDictionary::load(&path).expect("original loads");
        assert_eq!(original.exact_lemmas("かな", 8)[0].surface, "甲");
        let original_views = original.exact_lemma_views("かな", 8);
        let original_pointer = original_views[0].surface.as_ptr();

        let replacement = root.path().join("replacement-japanese.dat");
        std::fs::write(&replacement, test_model::single("乙")).expect("write replacement");
        std::fs::remove_file(&path).expect("remove");
        std::fs::rename(&replacement, &path).expect("rename");
        let updated = JapaneseDictionary::load(&path).expect("updated loads");
        assert_eq!(updated.exact_lemmas("かな", 8)[0].surface, "乙");
        assert_eq!(original.exact_lemmas("かな", 8)[0].surface, "甲");
        assert_eq!(original_views[0].surface, "甲");
        assert_eq!(original_views[0].surface.as_ptr(), original_pointer);
    }

    #[test]
    fn truncated_or_corrupt_models_are_refused() {
        let valid = test_model::single("甲");
        for length in [0, 55, 77, valid.len() - 1] {
            assert!(parse(&valid[..length]).is_none(), "{length}");
        }
        let mut invalid_offset = valid.clone();
        invalid_offset[24..32].fill(0xff);
        assert!(parse(invalid_offset).is_none());
        let mut invalid_reading = valid.clone();
        invalid_reading[60] = 0xff;
        invalid_reading[61] = 0xff;
        assert!(parse(invalid_reading).is_none());

        let root = tempfile::tempdir().expect("temporary directory");
        assert!(JapaneseDictionary::load(&root.path().join("absent.dat")).is_none());
        assert!(JapaneseDictionary::load(root.path()).is_none());
        // Files shorter than the header, including an empty one, are refused before mapping.
        for length in [0, HEADER_SIZE - 1] {
            let short = root.path().join(format!("short-{length}.dat"));
            std::fs::write(&short, &valid[..length]).expect("write short model");
            assert!(JapaneseDictionary::load(&short).is_none(), "{length}");
        }
    }

    #[cfg(unix)]
    #[test]
    fn rejects_a_symlinked_valid_model() {
        use std::os::unix::fs::symlink;

        let root = tempfile::tempdir().unwrap();
        let external = root.path().join("external.dat");
        std::fs::write(&external, test_model::single("甲")).unwrap();
        let linked = root.path().join("linked.dat");
        symlink(&external, &linked).unwrap();

        assert!(JapaneseDictionary::load(&external).is_some());
        assert!(JapaneseDictionary::load(&linked).is_none());
    }

    /// `load` maps the file (the `bytes` field is a read-only `Mmap`, not an owned buffer) and every lookup, the matrix search included, reads through that mapping; unlinking the file does not disturb a loaded dictionary.
    #[test]
    fn a_mapped_model_answers_lookups_and_sentence_search() {
        let root = tempfile::tempdir().expect("temporary directory");
        let path = root.path().join("msime-japanese.dat");
        let file = test_model::bytes(
            &[("かな", "仮名", 0, 0, 900), ("し", "詩", 0, 0, 400)],
            1,
            &[10],
        );
        std::fs::write(&path, &file).expect("write model");
        let dictionary = JapaneseDictionary::load(&path).expect("model loads");
        assert!(matches!(dictionary.bytes, ModelBytes::Mapped(_)));
        assert_eq!(dictionary.bytes.len(), file.len());
        assert_eq!(&dictionary.bytes[..], &file[..]);
        std::fs::remove_file(&path).expect("remove");

        assert_eq!(surfaces(&dictionary.exact_lemmas("かな", 8)), vec!["仮名"]);
        assert_eq!(surfaces(&dictionary.prefix_lemmas("か", 8)), vec!["仮名"]);
        assert_eq!(dictionary.connection_cost(0, 0), 10);
        let sentence = super::super::matrix::search_converted(
            &dictionary,
            &super::super::romaji::convert_romaji("kanasi"),
            4,
        );
        // Sentence start, 仮名, 詩 and sentence end are each joined by the one 10-cost transition: 900 + 400 + 3 * 10.
        assert_eq!(sentence[0].text, "仮名詩");
        assert_eq!(sentence[0].cost, 1_330);
    }

    #[test]
    fn header_and_ordering_checks() {
        let mut bad_magic = test_model::single("甲");
        bad_magic[0] = b'X';
        assert!(parse(bad_magic).is_none());
        let mut bad_version = test_model::single("甲");
        bad_version[8] = 2;
        assert!(parse(bad_version).is_none());

        let unsorted = test_model::bytes(&[("し", "市", 0, 0, 1), ("か", "蚊", 0, 0, 1)], 1, &[0]);
        assert!(parse(unsorted).is_none());
        let empty_reading = test_model::bytes(&[("", "空", 0, 0, 1)], 1, &[0]);
        assert!(parse(empty_reading).is_none());
        let bad_id = test_model::bytes(&[("か", "蚊", 1, 0, 1)], 1, &[0]);
        assert!(parse(bad_id).is_none());
        let no_tokens = test_model::bytes(&[], 1, &[0]);
        assert!(parse(no_tokens).is_none());
        // A string range that ends inside a character.
        let mut split = test_model::single("甲");
        split[60] = 5;
        assert!(parse(split).is_none());
    }

    #[test]
    fn lemma_lookups_order_by_cost_then_id() {
        let dictionary = parsed(test_model::bytes(
            &[
                ("か", "蚊", 0, 1, 300),
                ("か", "化", 1, 0, 100),
                ("か", "可", 0, 0, 100),
                ("かな", "仮名", 0, 0, 50),
                ("かんじ", "漢字", 1, 1, 10),
                ("き", "木", 0, 0, 1),
            ],
            2,
            &[5, -7, 300, 40],
        ));
        let exact = dictionary.exact_lemmas("か", 8);
        assert_eq!(surfaces(&exact), vec!["化", "可", "蚊"]);
        assert_eq!(exact[0].token_id, 1);
        assert_eq!(
            (exact[0].left_id, exact[0].right_id, exact[0].word_cost),
            (1, 0, 100)
        );
        assert_eq!(exact[0].reading, "か");
        assert_eq!(
            surfaces(&dictionary.exact_lemmas("か", 2)),
            vec!["化", "可"]
        );
        assert!(dictionary.exact_lemmas("く", 8).is_empty());
        assert!(dictionary.exact_lemmas("か", 0).is_empty());

        // One code point comes from the index, longer prefixes from a scan, with the same order.
        assert_eq!(
            surfaces(&dictionary.prefix_lemmas("か", 3)),
            vec!["漢字", "仮名", "化"]
        );
        assert_eq!(
            surfaces(&dictionary.prefix_lemmas("か", 100)),
            vec!["漢字", "仮名", "化", "可", "蚊"]
        );
        assert_eq!(surfaces(&dictionary.prefix_lemmas("かん", 8)), vec!["漢字"]);
        assert!(dictionary.prefix_lemmas("", 8).is_empty());

        assert_eq!(
            surfaces(&dictionary.prefix_lemmas_continuing("か", &["な", "ん"], 8)),
            vec!["漢字", "仮名"]
        );
        assert_eq!(
            surfaces(&dictionary.prefix_lemmas_continuing("か", &["ん"], 8)),
            vec!["漢字"]
        );
        assert!(dictionary
            .prefix_lemmas_continuing("かな", &["な"], 8)
            .is_empty());
        assert!(dictionary.prefix_lemmas_continuing("か", &[], 8).is_empty());

        assert_eq!(dictionary.connection_cost(0, 0), 5);
        assert_eq!(dictionary.connection_cost(0, 1), -7);
        assert_eq!(dictionary.connection_cost(1, 0), 300);
        assert_eq!(dictionary.connection_cost(1, 1), 40);
        assert_eq!(dictionary.connection_cost(2, 0), 10_000);
        assert_eq!(dictionary.connection_cost(0, 2), 10_000);
    }

    #[test]
    fn continuing_candidate_ranges_deduplicate_overlapping_kana() {
        let dictionary = parsed(test_model::bytes(
            &[
                ("かな", "仮名", 0, 0, 10),
                ("かない", "家内", 0, 0, 20),
                ("かに", "蟹", 0, 0, 40),
                ("かん", "漢", 0, 0, 30),
            ],
            1,
            &[0],
        ));

        let ids = dictionary.continuing_candidate_ids("か", &["な", "ない", "ん"]);
        assert_eq!(ids, vec![0, 1, 3]);
    }

    #[test]
    fn short_prefix_index_keeps_the_best_sixty_four() {
        let surfaces_owned: Vec<String> = (0..70).map(|index| format!("語{index:02}")).collect();
        let entries: Vec<test_model::Entry<'_>> = surfaces_owned
            .iter()
            .enumerate()
            .map(|(index, surface)| ("か", surface.as_str(), 0, 0, 1_000 - index as i32))
            .collect();
        let dictionary = parsed(test_model::bytes(&entries, 1, &[0]));
        let cached = dictionary.prefix_lemmas("か", 64);
        assert_eq!(cached.len(), 64);
        assert_eq!(cached[0].surface, "語69");
        assert_eq!(cached[63].surface, "語06");
        // Past the cached count the full range is scanned.
        assert_eq!(dictionary.prefix_lemmas("か", 65).len(), 65);
    }

    fn shared_cache_test_lock() -> MutexGuard<'static, ()> {
        lock(&SHARED_CACHE_TEST_LOCK)
    }

    #[test]
    fn shared_returns_one_dictionary_per_path() {
        let _serial = shared_cache_test_lock();
        let root = tempfile::tempdir().expect("temporary directory");
        let path = root.path().join("msime-japanese.dat");
        std::fs::write(&path, test_model::single("甲")).expect("write model");
        let first = JapaneseDictionary::shared(&path).expect("loads");
        let second = JapaneseDictionary::shared(&path).expect("shared");
        assert!(Arc::ptr_eq(&first, &second));
        assert!(JapaneseDictionary::shared(&root.path().join("absent.dat")).is_none());
    }

    /// Android 每进一个输入框都重建会话：上一个会话放掉模型后，下一个会话拿到的仍是同一份，不再重新校验整个文件。
    #[test]
    fn shared_keeps_the_dictionary_after_every_session_drops_it() {
        let _serial = shared_cache_test_lock();
        let root = tempfile::tempdir().expect("temporary directory");
        let path = root.path().join("msime-japanese.dat");
        std::fs::write(&path, test_model::single("甲")).expect("write model");
        let first = JapaneseDictionary::shared(&path).expect("loads");
        let address = Arc::as_ptr(&first);
        drop(first);
        let again = JapaneseDictionary::shared(&path).expect("still cached");
        assert_eq!(Arc::as_ptr(&again), address);
        assert!(JapaneseDictionary::is_shared(&path));
    }

    /// 资源包和 Android 的引导资源都在固定路径上，更新时 rename 换成新文件：缓存认出文件换了，读新的；还在用旧模型的会话不受影响。
    #[test]
    fn shared_reads_a_model_renamed_into_the_same_path() {
        let _serial = shared_cache_test_lock();
        let root = tempfile::tempdir().expect("temporary directory");
        let path = root.path().join("msime-japanese.dat");
        std::fs::write(&path, test_model::single("甲")).expect("write model");
        let original = JapaneseDictionary::shared(&path).expect("original loads");
        assert_eq!(surfaces(&original.exact_lemmas("かな", 1)), ["甲"]);

        let replacement = root.path().join("replacement-japanese.dat");
        std::fs::write(&replacement, test_model::single("乙")).expect("write replacement");
        std::fs::rename(&replacement, &path).expect("rename over");
        let updated = JapaneseDictionary::shared(&path).expect("updated loads");
        assert!(!Arc::ptr_eq(&original, &updated));
        assert_eq!(surfaces(&updated.exact_lemmas("かな", 1)), ["乙"]);
        assert_eq!(surfaces(&original.exact_lemmas("かな", 1)), ["甲"]);
        assert!(Arc::ptr_eq(
            &updated,
            &JapaneseDictionary::shared(&path).expect("cached again")
        ));

        // 文件删掉后条目一并丢掉，不再钉住已删文件的映射。
        std::fs::remove_file(&path).expect("remove");
        assert!(JapaneseDictionary::shared(&path).is_none());
        assert!(!JapaneseDictionary::is_shared(&path));
    }

    /// 读失败不缓存：文件先不在、或先是坏的，之后放上合法模型，下一次就读得到。
    #[test]
    fn shared_does_not_cache_a_failed_load() {
        let _serial = shared_cache_test_lock();
        let root = tempfile::tempdir().expect("temporary directory");
        let path = root.path().join("msime-japanese.dat");
        assert!(JapaneseDictionary::shared(&path).is_none());
        std::fs::write(&path, b"not a model").expect("write corrupt model");
        assert!(JapaneseDictionary::shared(&path).is_none());
        assert!(!JapaneseDictionary::is_shared(&path));

        let replacement = root.path().join("replacement-japanese.dat");
        std::fs::write(&replacement, test_model::single("甲")).expect("write model");
        std::fs::rename(&replacement, &path).expect("rename over");
        let loaded = JapaneseDictionary::shared(&path).expect("loads once valid");
        assert_eq!(surfaces(&loaded.exact_lemmas("かな", 1)), ["甲"]);
    }

    /// 清缓存放掉进程级缓存里的模型：持有它的会话照常用，之后再要就重新读一份。
    #[test]
    fn release_shared_drops_the_cached_dictionary() {
        let _serial = shared_cache_test_lock();
        let root = tempfile::tempdir().expect("temporary directory");
        let path = root.path().join("msime-japanese.dat");
        std::fs::write(&path, test_model::single("甲")).expect("write model");
        let held = JapaneseDictionary::shared(&path).expect("loads");
        let released = Arc::downgrade(&JapaneseDictionary::shared(&path).expect("cached"));
        JapaneseDictionary::release_shared();
        assert!(!JapaneseDictionary::is_shared(&path));
        assert_eq!(surfaces(&held.exact_lemmas("かな", 1)), ["甲"]);
        assert!(released.upgrade().is_some(), "a session still holds it");
        drop(held);
        assert!(released.upgrade().is_none(), "nothing else keeps it alive");
        let reloaded = JapaneseDictionary::shared(&path).expect("loads again");
        assert_eq!(surfaces(&reloaded.exact_lemmas("かな", 1)), ["甲"]);
        assert_eq!(JapaneseDictionary::load_count(&path), 2);
    }

    /// 清缓存时还有会话拿着模型（iOS 内存告警时当前会话就是这样）：下一个要它的会话拿回同一份，不在内存紧张时另读一份。
    #[test]
    fn release_shared_hands_back_a_dictionary_a_session_still_holds() {
        let _serial = shared_cache_test_lock();
        let root = tempfile::tempdir().expect("temporary directory");
        let path = root.path().join("msime-japanese.dat");
        std::fs::write(&path, test_model::single("甲")).expect("write model");
        let held = JapaneseDictionary::shared(&path).expect("loads");
        JapaneseDictionary::release_shared();
        assert!(!JapaneseDictionary::is_shared(&path));
        let again = JapaneseDictionary::shared(&path).expect("handed back");
        assert!(Arc::ptr_eq(&held, &again));
        assert_eq!(JapaneseDictionary::load_count(&path), 1);
        // 交回来的那份重新被缓存强引用着，会话都放手后仍在。
        let address = Arc::as_ptr(&held);
        drop((held, again));
        assert!(JapaneseDictionary::is_shared(&path));
        let cached = JapaneseDictionary::shared(&path).expect("cached");
        assert_eq!(Arc::as_ptr(&cached), address);
        assert_eq!(JapaneseDictionary::load_count(&path), 1);
    }

    /// 后台预热正读着文件时第一个查询来了：查询等预热读完拿同一份，文件只读一遍。
    #[test]
    fn a_query_racing_the_warm_up_reads_the_model_once() {
        let _serial = shared_cache_test_lock();
        let root = tempfile::tempdir().expect("temporary directory");
        let path = root.path().join("msime-japanese.dat");
        std::fs::write(&path, test_model::single("甲")).expect("write model");
        let pause = JapaneseDictionary::pause_next_load(&path);
        JapaneseDictionary::warm_up_in_background(&path);
        pause.wait();
        let query = std::thread::spawn({
            let path = path.clone();
            move || JapaneseDictionary::shared(&path)
        });
        // 让查询线程走到 `LOADING` 上等着。它若还没走到，就在预热读完后直接拿缓存，下面的断言同样成立，只是这一次没测到等锁那条路。
        std::thread::sleep(std::time::Duration::from_millis(50));
        pause.wait();
        let queried = query.join().expect("query thread").expect("loads");
        JapaneseDictionary::wait_until_warmed(&path);
        let cached = JapaneseDictionary::shared(&path).expect("cached");
        assert!(Arc::ptr_eq(&queried, &cached));
        assert_eq!(JapaneseDictionary::load_count(&path), 1);
    }

    /// 读文件期间缓存被清：读完的模型不写回强引用，但拿着它的会话还在时，再要的会话拿到同一份。
    #[test]
    fn a_load_that_finishes_after_release_shared_is_not_pinned() {
        let _serial = shared_cache_test_lock();
        let root = tempfile::tempdir().expect("temporary directory");
        let path = root.path().join("msime-japanese.dat");
        std::fs::write(&path, test_model::single("甲")).expect("write model");
        let pause = JapaneseDictionary::pause_next_load(&path);
        let loader = std::thread::spawn({
            let path = path.clone();
            move || JapaneseDictionary::shared(&path)
        });
        pause.wait();
        JapaneseDictionary::release_shared();
        pause.wait();
        let loaded = loader.join().expect("loader thread").expect("loads");
        assert!(!JapaneseDictionary::is_shared(&path));
        let again = JapaneseDictionary::shared(&path).expect("handed back");
        assert!(Arc::ptr_eq(&loaded, &again));
        assert_eq!(JapaneseDictionary::load_count(&path), 1);
    }

    /// 缓存满了先挤掉最早读进来的路径；同一路径重新放进来替换原条目，不占第二个位置。
    #[test]
    fn shared_models_evict_the_oldest_path_when_full() {
        let root = tempfile::tempdir().expect("temporary directory");
        let mut models = SharedModels::default();
        let entry = |name: &str| {
            let path = root.path().join(name);
            std::fs::write(&path, test_model::single(name)).expect("write model");
            let (dictionary, identity) =
                JapaneseDictionary::load_with_identity(&path).expect("loads");
            SharedModel {
                path,
                identity,
                model: Arc::new(dictionary),
            }
        };
        let (first, second, third) = (entry("一"), entry("二"), entry("三"));
        let (first_path, second_path, third_path) =
            (first.path.clone(), second.path.clone(), third.path.clone());
        models.insert(first, 2);
        models.insert(second, 2);
        models.insert(entry("二"), 2);
        assert_eq!(models.entries.len(), 2);
        models.insert(third, 2);
        assert!(models.current(&first_path, 2).is_none());
        assert!(models.current(&second_path, 2).is_some());
        assert!(models.current(&third_path, 2).is_some());

        // 挤出去时还有人拿着的，再要时交回同一份，并重新占一个位置。
        let fourth = entry("四");
        let (fourth_path, held) = (fourth.path.clone(), Arc::clone(&fourth.model));
        models.insert(fourth, 2);
        models.insert(entry("五"), 2);
        models.insert(entry("六"), 2);
        assert!(models.strong(&fourth_path).is_none());
        let again = models.current(&fourth_path, 2).expect("handed back");
        assert!(Arc::ptr_eq(&held, &again));
        assert!(models.strong(&fourth_path).is_some());
        assert_eq!(models.entries.len(), 2);
    }

    /// 预热在后台线程读好模型；之后的查询直接拿缓存里的那份。
    #[test]
    fn warm_up_loads_the_dictionary_in_the_background() {
        let _serial = shared_cache_test_lock();
        let root = tempfile::tempdir().expect("temporary directory");
        let path = root.path().join("msime-japanese.dat");
        std::fs::write(&path, test_model::single("甲")).expect("write model");
        JapaneseDictionary::warm_up_in_background(&path);
        JapaneseDictionary::wait_until_warmed(&path);
        assert!(JapaneseDictionary::is_shared(&path));

        // 文件不在时预热什么也不留下，也不会一直占着「正在预热」。
        let absent = root.path().join("absent.dat");
        JapaneseDictionary::warm_up_in_background(&absent);
        JapaneseDictionary::wait_until_warmed(&absent);
        assert!(!JapaneseDictionary::is_shared(&absent));
    }

    #[test]
    fn a_preloaded_dictionary_answers_for_its_path_until_unloaded() {
        // No file exists at this path: a host without a file system hands the bytes over instead.
        let path = Path::new("/preloaded-test/msime-japanese.dat");
        assert!(JapaneseDictionary::shared(path).is_none());
        assert!(!JapaneseDictionary::preload(
            path,
            b"not a model".to_vec().into_boxed_slice()
        ));
        assert!(JapaneseDictionary::shared(path).is_none());
        assert!(JapaneseDictionary::preload(
            path,
            test_model::single("甲").into_boxed_slice()
        ));
        let first = JapaneseDictionary::shared(path).expect("preloaded");
        assert_eq!(surfaces(&first.exact_lemmas("かな", 1)), ["甲"]);
        let original_views = first.exact_lemma_views("かな", 1);
        let original_pointer = original_views[0].surface.as_ptr();
        assert!(Arc::ptr_eq(
            &first,
            &JapaneseDictionary::shared(path).expect("again")
        ));
        // A later preload replaces it; a session still holding the first keeps it.
        assert!(JapaneseDictionary::preload(
            path,
            test_model::single("乙").into_boxed_slice()
        ));
        assert_eq!(
            surfaces(
                &JapaneseDictionary::shared(path)
                    .unwrap()
                    .exact_lemmas("かな", 1)
            ),
            ["乙"]
        );
        assert_eq!(surfaces(&first.exact_lemmas("かな", 1)), ["甲"]);
        JapaneseDictionary::unload(path);
        assert!(JapaneseDictionary::shared(path).is_none());
        assert_eq!(original_views[0].surface, "甲");
        assert_eq!(original_views[0].surface.as_ptr(), original_pointer);
    }

    /// The shipped `dict-v2.0.1` model (data-formats.md §8): header values and a few lookups.
    #[test]
    fn real_model_loads() {
        let Some(resources) = std::env::var_os("MSIME_EVAL_RESOURCES") else {
            eprintln!(
                "skipped: MSIME_EVAL_RESOURCES is not set to the dict-v2.0.1 resource directory"
            );
            return;
        };
        let path = Path::new(&resources).join(crate::assets::JAPANESE_MODEL);
        let dictionary = JapaneseDictionary::load(&path).expect("the shipped model loads");
        assert_eq!(dictionary.token_count, 1_284_987);
        assert_eq!(dictionary.connection_size, 2_672);
        assert!(surfaces(&dictionary.exact_lemmas("かんじ", 16)).contains(&"漢字"));
        assert!(!dictionary.prefix_lemmas("か", 24).is_empty());
    }
}

#[cfg(test)]
mod continuing_tests;

#[cfg(test)]
#[path = "decoder/exact_stream_tests.rs"]
mod exact_stream_tests;

#[cfg(test)]
#[path = "decoder/ranking_reference.rs"]
mod ranking_reference;

#[cfg(test)]
#[path = "decoder/ranking_tests.rs"]
mod ranking_tests;

#[cfg(test)]
#[path = "decoder/ranking_heap_sort.rs"]
mod ranking_heap_sort;
