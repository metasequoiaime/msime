//! emoji-memory：持久存储（`persist_data`）和往候选里加行（`insert_candidates`）的参考样板。
//!
//! 记住你在某个编码下选过的 emoji，下次打同样的编码时把它插到候选第二位。例如在 emoji 模式里打 `xiao` 选了 😄，以后用拼音打 `xiao` 时 😄 直接出现在「小」后面，不用再进 emoji 模式找。
//!
//! 声明 `candidates` 和 `candidate_selected` 两个钩子，申请 `read_candidates`、`read_commit`、`insert_candidates` 和 `persist_data`：
//! - `candidates`：第一次运行时把存储整块读进来（存储在 `msime_init` 里读不到）。每次记下这一窗的编码（第 0 行候选的编码，去掉 `'` 和空格、转成小写，最长 24 个字母，否则不记）和窗口里来源是 emoji 的行。本地模式是 `none` 时，在存储里找这个编码下次数最多的两个 emoji（次数相同时先记下的在前），窗口里已经有的跳过，用 `candidates_insert` 插在第 1 行（第 0 行之后）。
//! - `candidate_selected`：本地模式是 `none` 或 emoji、选中之后直接上屏（不是暂存进词组）、选中的文字是上一窗里来源为 emoji 的某一行或者插件自己插的行时，这个（编码，emoji）的次数加 1，整块写回存储。别的选择一概不记：插件只记 emoji，不记你打的字。
//!
//! 存储的格式是定长的文本，用户打开 `script-data/emoji-memory.bin` 就能读懂：第一行 `emoji-memory 1`，之后每条 64 字节一行，`次数（5 位数字） 编码（24 字节，空格补齐） emoji（32 字节，空格补齐）`，按第一次记下的先后排列，最多 32 条，满了时替换次数最少的那一条（最少的有几条时替换最早的）。第一行不对、长度不是整条、或者哪一条的末尾不是换行（被别的程序改过）时整块当作空的，下一次记录时覆盖；格式对但内容乱的记录不会出错，只是查不到或者插入前被拒（[`checked`]）。
//!
//! fuel 决定了读和查找的写法：逐字节解析或比较 32 条记录，一次就超过钩子的上限。所以读进来时只看第一行和每条末尾的换行（错位的文件一眼就能认出来），不解析别的；查找先拿编码、emoji 的前 8 字节当一个 64 位整数比（一条不对齐的 `i64.load`），对上了才比整个字段、才读次数。窗口里的 emoji 另有一个 64 位的位图，记录的 emoji 不在位图里就不用逐个比。次数原地改写，不排序（排序要挪动记录，最坏一次挪 2 KiB），找次数最多的两条在查找时顺手比较；写回就是把整块交给 `storage_write`。
//!
//! 存储只在持久实例上的钩子里可用，这两个钩子都是；`before_commit`、`command` 和 `msime_init` 里读写都是 `DENIED`。`persist_data` 不能和 `rewrite_commit` 一起申请，这个插件也不改写上屏。

#![no_std]

use core::ptr::addr_of_mut;
use msime_script_sdk::{
    candidates, commit, event, hook, insert, local_mode, log, outcome, storage, Error,
};

/// 存储的第一行，版本号变了就整块当作空的。
const HEADER: &[u8] = b"emoji-memory 1\n";
/// 每条记录的字节数和各字段的位置。
const RECORD: usize = 64;
const COUNT_AT: usize = 0;
const COUNT_DIGITS: usize = 5;
const CODE_AT: usize = 6;
const CODE_BYTES: usize = 24;
const EMOJI_AT: usize = 31;
const EMOJI_BYTES: usize = 32;
/// 最多记多少条。32 条 2 KiB，远小于 64 KiB 的存储上限；上限定在这里是为了查找的 fuel：所有记录都是同一个编码时，一次 `candidates` 要把每一条都比一遍，满了时一次 `candidate_selected` 要找出次数最少的那条，64 条时后者超过观察类钩子上限的一半。
const MAX_RECORDS: usize = 32;
const STORE_BYTES: usize = HEADER.len() + MAX_RECORDS * RECORD;
/// 次数到这里就不再加。
const MAX_COUNT: u32 = 99_999;

/// 候选窗口最多 20 行（宿主的上限）。
const MAX_ROWS: u32 = 20;
/// 每次 `candidates` 最多插几行（宿主上限是 3）。
const MAX_INSERTS: usize = 2;
/// 一窗里最多记几个 emoji：窗口的 20 行加上插件插的两行。
const MAX_OFFERED: usize = MAX_ROWS as usize + MAX_INSERTS;
/// 插在第几行：第 0 行之后。
const INSERT_AT: u32 = 1;

/// 候选来源（`candidates::SOURCE` 的值），见 crates/engine/src/types.rs 的 `CandidateSource`。
const SOURCE_EMOJI: u64 = 6;

/// 存储读进来没有。
#[derive(Clone, Copy, PartialEq, Eq)]
enum Load {
    NotYet,
    Ready,
    /// 没授予 `persist_data`，或者宿主没交存储：这个实例不记也不插。
    Unavailable,
}

/// 实例的全部状态。实例每次聚焦都重建，所以这里只有一个输入框里的东西；跨输入框的只有存储。
struct State {
    load: Load,
    /// 存储的内容：[`HEADER`] 加 `records` 条记录。
    store: [u8; STORE_BYTES],
    records: usize,
    /// 上一窗的编码，空格补齐到 [`CODE_BYTES`]；`has_key` 为假时没有。
    key: [u8; CODE_BYTES],
    has_key: bool,
    /// 上一窗里来源是 emoji 的行和插件插的行，空格补齐到 [`EMOJI_BYTES`]，和它们的前 8 字节。
    offered: [[u8; EMOJI_BYTES]; MAX_OFFERED],
    offered_heads: [u64; MAX_OFFERED],
    offered_len: usize,
    /// `offered_heads` 的位图：每个前 8 字节按 [`bit`] 占一位。查找时先看这一位，没有就一定不在窗口里，不用逐项比。
    offered_mask: u64,
}

// 初值全是 0，整个状态落在模块的零初始化段里，不占模块的字节（模块大小按 128 倍计进内存总账）。数组在读之前一定先写过。
static mut STATE: State = State {
    load: Load::NotYet,
    store: [0; STORE_BYTES],
    records: 0,
    key: [0; CODE_BYTES],
    has_key: false,
    offered: [[0; EMOJI_BYTES]; MAX_OFFERED],
    offered_heads: [0; MAX_OFFERED],
    offered_len: 0,
    offered_mask: 0,
};

#[no_mangle]
pub extern "C" fn msime_abi() -> i32 {
    msime_script_sdk::ABI_VERSION
}

#[no_mangle]
pub extern "C" fn msime_on_event(kind: i32) -> i32 {
    // SAFETY: 插件单线程运行，宿主一次只调用一个钩子。
    let state = unsafe { &mut *addr_of_mut!(STATE) };
    if kind == hook::CANDIDATES {
        state.on_candidates();
    } else if kind == hook::CANDIDATE_SELECTED {
        state.on_selected();
    }
    0
}

impl State {
    /// 一次 `candidates`：记下编码和 emoji 行，本地模式 `none` 时插入常用的 emoji。
    fn on_candidates(&mut self) {
        self.has_key = false;
        self.offered_len = 0;
        self.offered_mask = 0;
        let count = match candidates::count() {
            Ok(count) => (count as u32).min(MAX_ROWS),
            Err(error) => return log::debug(reason(error)),
        };
        if count == 0 {
            return log::debug("skip: empty");
        }
        self.ensure_loaded();
        if self.load != Load::Ready {
            return log::debug("skip: no storage");
        }
        let mut code = [0u8; 64];
        self.has_key = match candidates::code(0, &mut code) {
            Ok(code) => normalize(code.as_bytes(), &mut self.key),
            Err(_) => false,
        };
        if !self.has_key {
            return log::debug("skip: no code");
        }
        let mut row = 0;
        while row < count {
            if candidates::meta(row, candidates::SOURCE) == Ok(SOURCE_EMOJI) {
                self.remember_row(row);
            }
            row += 1;
        }
        let mut line = Line::new();
        line.push(field(&self.key));
        if event::scalar(event::LOCAL_MODE) != Ok(local_mode::NONE) {
            line.push(b": no insert");
            return line.write();
        }
        self.insert_favourites(&mut line);
        line.write();
    }

    /// 在存储里找这个编码下窗口里还没有的、次数最多的两个 emoji（次数相同时先记下的在前），插在第 1 行。
    fn insert_favourites(&mut self, line: &mut Line) {
        // (次数, 第几条)；次数为 0 的是空位，记录里的次数至少是 1。
        let mut best = [(0u32, 0usize); MAX_INSERTS];
        let key = word(&self.key, 0);
        let mut index = 0;
        while index < self.records {
            if word(self.code_of(index), 0) == key {
                self.consider(index, &mut best);
            }
            index += 1;
        }
        let mut inserted = 0;
        for (count, index) in best {
            if count == 0 {
                break;
            }
            let mut emoji = [0u8; EMOJI_BYTES];
            emoji.copy_from_slice(self.emoji_of(index));
            // 读进来时只核对了结构，emoji 字段的内容在用到时才核对：不是 UTF-8、含空白的跳过；别的不合规（控制字符之类）由宿主拒绝。
            let Some(text) = checked(field(&emoji)) else {
                continue;
            };
            line.push(if inserted == 0 { b": insert " } else { b"," });
            line.push(text.as_bytes());
            if let Err(error) = insert::row(INSERT_AT, text, "常用") {
                line.push(b" ");
                line.push(reason(error).as_bytes());
                return;
            }
            self.offer(emoji);
            inserted += 1;
        }
        if inserted == 0 {
            line.push(b": none");
        }
    }

    /// 一次 `candidate_selected`：选中的是上一窗的 emoji 行或插件插的行时，这个（编码，emoji）的次数加 1 并写回。
    fn on_selected(&mut self) {
        if self.load != Load::Ready || !self.has_key {
            return log::debug("skip: no window");
        }
        if !matches!(
            event::scalar(event::LOCAL_MODE),
            Ok(local_mode::NONE | local_mode::EMOJI)
        ) {
            return log::debug("skip: mode");
        }
        if event::scalar(event::OUTCOME) != Ok(outcome::COMMITTED) {
            return log::debug("skip: not committed");
        }
        let mut buffer = [0u8; EMOJI_BYTES];
        let selected = match commit::bytes(commit::SELECTED, &mut buffer) {
            Ok(selected) => selected,
            Err(Error::Limit) => return log::debug("skip: not an emoji row"),
            Err(error) => return log::debug(reason(error)),
        };
        let emoji = pad(selected);
        if checked(selected).is_none() || !self.offers(&emoji) {
            return log::debug("skip: not an emoji row");
        }
        let count = self.bump(&emoji);
        if let Err(error) = storage::write(&self.store[..HEADER.len() + self.records * RECORD]) {
            return log::debug(reason(error));
        }
        let mut line = Line::new();
        line.push(b"learn ");
        line.push(field(&self.key));
        line.push(b" ");
        line.push(field(&emoji));
        line.push(b" ");
        line.number(count);
        line.write();
    }

    /// 第一次需要时把存储读进来。读到的不是这个格式时当作空的。
    fn ensure_loaded(&mut self) {
        if self.load != Load::NotYet {
            return;
        }
        self.load = Load::Unavailable;
        let length = match storage::len() {
            Ok(length) => length as usize,
            Err(_) => return,
        };
        self.load = Load::Ready;
        self.records = 0;
        let fits = length <= STORE_BYTES
            && length >= HEADER.len()
            && (length - HEADER.len()).is_multiple_of(RECORD);
        let read = fits
            && storage::read(0, &mut self.store[..length]).map(|bytes| bytes.len()) == Ok(length)
            && self.store[..HEADER.len()] == *HEADER;
        if !read {
            self.store[..HEADER.len()].copy_from_slice(HEADER);
            if length > 0 {
                log::debug("reset: unreadable storage");
            }
            return;
        }
        let records = (length - HEADER.len()) / RECORD;
        let mut end = HEADER.len() + RECORD - 1;
        while end < length {
            if self.store[end] != b'\n' {
                self.store[..HEADER.len()].copy_from_slice(HEADER);
                return log::debug("reset: unreadable storage");
            }
            end += RECORD;
        }
        self.records = records;
    }

    /// 第 `index` 条的编码前 8 字节已经和这一窗的对上了：比完整个编码，次数能进前两名、emoji 又不在窗口里时放进 `best`。
    #[inline(never)]
    fn consider(&self, index: usize, best: &mut [(u32, usize); MAX_INSERTS]) {
        if !same(self.code_of(index), &self.key) {
            return;
        }
        let count = read_count(self.record(index));
        // 进不了前两名的不用再看窗口里有没有它。
        if count <= best[MAX_INSERTS - 1].0 || self.offers_record(index) {
            return;
        }
        if count > best[0].0 {
            best[1] = best[0];
            best[0] = (count, index);
        } else {
            best[1] = (count, index);
        }
    }

    #[inline(always)]
    fn record(&self, index: usize) -> &[u8] {
        let start = HEADER.len() + index * RECORD;
        &self.store[start..start + RECORD]
    }

    #[inline(always)]
    fn record_mut(&mut self, index: usize) -> &mut [u8] {
        let start = HEADER.len() + index * RECORD;
        &mut self.store[start..start + RECORD]
    }

    #[inline(always)]
    fn code_of(&self, index: usize) -> &[u8] {
        &self.record(index)[CODE_AT..CODE_AT + CODE_BYTES]
    }

    #[inline(always)]
    fn emoji_of(&self, index: usize) -> &[u8] {
        &self.record(index)[EMOJI_AT..EMOJI_AT + EMOJI_BYTES]
    }

    /// 记下窗口第 `row` 行（来源是 emoji）的文字。太长的不记（反正也选不出来记不下）。
    fn remember_row(&mut self, row: u32) {
        let mut buffer = [0u8; EMOJI_BYTES];
        if let Ok(text) = candidates::bytes(row, &mut buffer) {
            if !text.is_empty() {
                self.offer(pad(text));
            }
        }
    }

    fn offer(&mut self, emoji: [u8; EMOJI_BYTES]) {
        if self.offered_len < MAX_OFFERED {
            let head = word(&emoji, 0);
            self.offered_heads[self.offered_len] = head;
            self.offered_mask |= bit(head);
            self.offered[self.offered_len] = emoji;
            self.offered_len += 1;
        }
    }

    /// `emoji` 在不在上一窗里。先比前 8 字节，对上了才比整段。
    #[inline(never)]
    fn offers(&self, emoji: &[u8; EMOJI_BYTES]) -> bool {
        self.offered_by(word(emoji, 0), emoji)
    }

    /// 第 `index` 条记录的 emoji 在不在上一窗里。
    #[inline(never)]
    fn offers_record(&self, index: usize) -> bool {
        let emoji = self.emoji_of(index);
        self.offered_by(word(emoji, 0), emoji)
    }

    fn offered_by(&self, head: u64, emoji: &[u8]) -> bool {
        if self.offered_mask & bit(head) == 0 {
            return false;
        }
        let mut i = 0;
        while i < self.offered_len {
            if self.offered_heads[i] == head && same(&self.offered[i], emoji) {
                return true;
            }
            i += 1;
        }
        false
    }

    /// 这个（编码，emoji）的次数加 1，返回新的次数。没有这一条时添一条；满了时替换次数最少的那一条（有几条一样少时替换最早的）。
    fn bump(&mut self, emoji: &[u8; EMOJI_BYTES]) -> u32 {
        let index = match self.find(emoji) {
            Some(index) => index,
            None => {
                let index = if self.records < MAX_RECORDS {
                    self.records += 1;
                    self.records - 1
                } else {
                    self.least_used()
                };
                let key = self.key;
                let record = self.record_mut(index);
                record.fill(b' ');
                record[CODE_AT..CODE_AT + CODE_BYTES].copy_from_slice(&key);
                record[EMOJI_AT..EMOJI_AT + EMOJI_BYTES].copy_from_slice(emoji);
                record[RECORD - 1] = b'\n';
                write_count(record, 0);
                index
            }
        };
        let count = (read_count(self.record(index)) + 1).min(MAX_COUNT);
        write_count(self.record_mut(index), count);
        count
    }

    /// 这个编码下的 `emoji` 是第几条。先比编码和 emoji 的前 8 字节，对上了才比整个字段。
    #[inline(never)]
    fn find(&self, emoji: &[u8; EMOJI_BYTES]) -> Option<usize> {
        let key = word(&self.key, 0);
        let head = word(emoji, 0);
        let mut index = 0;
        while index < self.records {
            if word(self.code_of(index), 0) == key
                && word(self.emoji_of(index), 0) == head
                && same(self.code_of(index), &self.key)
                && same(self.emoji_of(index), emoji)
            {
                return Some(index);
            }
            index += 1;
        }
        None
    }

    /// 次数最少的那一条，一样少时取最早的。
    fn least_used(&self) -> usize {
        let mut least = 0;
        let mut fewest = u32::MAX;
        let mut index = 0;
        while index < self.records {
            let count = read_count(self.record(index));
            if count < fewest {
                least = index;
                fewest = count;
            }
            index += 1;
        }
        least
    }
}

/// 候选编码转成存储里的编码：去掉 `'` 和空格，大写转小写，只收字母，1 到 [`CODE_BYTES`] 个，空格补齐。
fn normalize(code: &[u8], out: &mut [u8; CODE_BYTES]) -> bool {
    *out = [b' '; CODE_BYTES];
    let mut length = 0;
    for &byte in code {
        let letter = match byte {
            b'a'..=b'z' => byte,
            b'A'..=b'Z' => byte.to_ascii_lowercase(),
            b'\'' | b' ' => continue,
            _ => return false,
        };
        if length == CODE_BYTES {
            return false;
        }
        out[length] = letter;
        length += 1;
    }
    length > 0
}

/// emoji 文字空格补齐到 [`EMOJI_BYTES`]，调用方保证不超过这么长。
fn pad(text: &[u8]) -> [u8; EMOJI_BYTES] {
    let mut emoji = [b' '; EMOJI_BYTES];
    emoji[..text.len()].copy_from_slice(text);
    emoji
}

/// 能不能当作一个 emoji 记下或插入：非空、不超过 [`EMOJI_BYTES`] 字节、合法 UTF-8、不含空白和控制字符（补齐用的是空格，含空格的文字存进去就读不回原样），也不含零宽连接符 U+200D 和标签字符 U+E0000..U+E007F。后两种是格式字符，插件插入的行不能含格式字符，输入法会拒绝，所以 👨‍👩‍👧 这类组合 emoji 和 🏴󠁧󠁢󠁳󠁣󠁴󠁿 这类旗帜记下来也插不进去，干脆不记。emoji 后面的 VS16（U+FE0F）可以用。
fn checked(text: &[u8]) -> Option<&str> {
    if text.is_empty() || text.len() > EMOJI_BYTES || text.iter().any(|&byte| byte <= b' ') {
        return None;
    }
    // U+200D 是 E2 80 8D，U+E0000..U+E007F 是 F3 A0 80 xx 和 F3 A0 81 xx。
    let format = text.windows(3).any(|three| {
        three == [0xE2, 0x80, 0x8D]
            || (three[0] == 0xF3 && three[1] == 0xA0 && three[2] & 0xFE == 0x80)
    });
    if format {
        return None;
    }
    core::str::from_utf8(text).ok()
}

/// 补齐的字段去掉末尾的空格。
fn field(bytes: &[u8]) -> &[u8] {
    let mut length = bytes.len();
    while length > 0 && bytes[length - 1] == b' ' {
        length -= 1;
    }
    &bytes[..length]
}

/// 记录里的次数。读进来时没有逐条核对数字：不是数字的位按 0 算，下一次加 1 时整段重写成数字。
#[inline(always)]
fn read_count(record: &[u8]) -> u32 {
    let digit = |at: usize| {
        let value = record[COUNT_AT + at].wrapping_sub(b'0');
        u32::from(value) * u32::from(value <= 9)
    };
    digit(0) * 10_000 + digit(1) * 1_000 + digit(2) * 100 + digit(3) * 10 + digit(4)
}

fn write_count(record: &mut [u8], mut count: u32) {
    for digit in record[COUNT_AT..COUNT_AT + COUNT_DIGITS].iter_mut().rev() {
        *digit = b'0' + (count % 10) as u8;
        count /= 10;
    }
}

/// 两个等长（8 的倍数）的字段是否相同，按 8 字节一段比较。
#[inline(never)]
fn same(left: &[u8], right: &[u8]) -> bool {
    let mut i = 0;
    while i + 8 <= left.len() {
        if word(left, i) != word(right, i) {
            return false;
        }
        i += 8;
    }
    true
}

/// 从 `at` 起的 8 字节当作一个 u64：一条不对齐的 `i64.load`。逐字节拼或者先复制到数组里，在解释器里要贵好几倍。
#[inline(always)]
fn word(bytes: &[u8], at: usize) -> u64 {
    assert!(at + 8 <= bytes.len());
    // SAFETY: 上一行核对过这 8 字节都在 `bytes` 里；`read_unaligned` 不要求对齐。
    u64::from_le(unsafe { core::ptr::read_unaligned(bytes.as_ptr().add(at).cast::<u64>()) })
}

/// 前 8 字节在 [`State::offered_mask`] 里占哪一位：乘一个奇数常量后取最高 6 位，把相近的 emoji 编码散开。
#[inline(always)]
fn bit(head: u64) -> u64 {
    1 << (head.wrapping_mul(0x9E37_79B9_7F4A_7C15) >> 58)
}

fn reason(error: Error) -> &'static str {
    match error {
        Error::Denied => "skip: denied",
        Error::Absent => "skip: absent",
        Error::Invalid => "skip: invalid",
        Error::Limit => "skip: limit",
        Error::Other(_) => "skip: error",
    }
}

/// 一行日志：编码、插了哪些 emoji，或者学到了什么。
struct Line {
    bytes: [u8; 128],
    len: usize,
}

impl Line {
    fn new() -> Self {
        Self {
            bytes: [0; 128],
            len: 0,
        }
    }

    fn push(&mut self, text: &[u8]) {
        let end = (self.len + text.len()).min(self.bytes.len());
        self.bytes[self.len..end].copy_from_slice(&text[..end - self.len]);
        self.len = end;
    }

    fn number(&mut self, mut value: u32) {
        let mut digits = [0u8; 5];
        let mut count = 0;
        loop {
            digits[count] = b'0' + (value % 10) as u8;
            count += 1;
            value /= 10;
            if value == 0 || count == digits.len() {
                break;
            }
        }
        while count > 0 {
            count -= 1;
            self.push(&[digits[count]]);
        }
    }

    /// 拼进去的是 ASCII 和完整的 emoji；被截断时可能切在字符中间，那时只写能解码的前一段。
    fn write(&self) {
        let bytes = &self.bytes[..self.len];
        let text = match core::str::from_utf8(bytes) {
            Ok(text) => text,
            Err(error) => core::str::from_utf8(&bytes[..error.valid_up_to()]).unwrap_or(""),
        };
        log::debug(text);
    }
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    core::arch::wasm32::unreachable()
}
