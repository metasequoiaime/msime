//! rare-chars-last：把含生僻字的候选移到候选窗口末尾，其余候选保持原来的相对顺序，窗口第 0 行不动。
//!
//! 只声明 `candidates`，申请 `read_candidates` 和 `rerank_candidates`。「生僻」的定义和 `plugins/script/shared/common_chars.rs` 相同：码位落在汉字区段里、并且不在常用字集（《通用规范汉字表》8105 字并上它们的繁体形）里的字；一行里只要有一个这样的字，这一行就算生僻。扫描在 `rare.rs`，按 fuel 的计法另写了一遍，判定和共用的 `classify` 逐字相同。
//!
//! 重排的做法：按原来的顺序点名第 0 行和全部不生僻、没有锁定的行，生僻行不点名。宿主把点名的行按次序排在前面，没点名的窗口行保持原来的相对顺序接在后面（仍在第 21 行及以后的候选之前），锁定的座位（固定位置的词、云和 AI 行、五笔四码唯一的 0 号座位）原地不动。生僻行已经全在末尾、点名不会让任何一行移动时不调用 `rerank::order`：几个插件的重排是后面的优先，不必要的点名会盖掉前面插件的排序。
//!
//! 窗口第 0 行永远点名在第一位，即使它是生僻字：首位出现生僻字，多半是用户打了它的精确编码或者学过这个字。窗口第 0 行通常就是引擎下标 0，按首位取词的路径（失焦上屏、原样上屏）取的是它，留在原位，屏幕上的首选和这些路径上屏的字才一致。内置的逐键重排把别的候选提到第 0 行时，引擎下标 0 落在后面某一行；插件看不到引擎的次序，它含生僻字时也会和别的生僻行一样移到末尾（宿主只保证引擎下标 0 不被隐藏，不限制重排）。
//!
//! 工作量：每行先读来源（约 110 fuel），需要判定的行再读文本（100 加每 4 字节 1）并扫描（`rare::scan`）。读入的总字节数不超过 [`SCAN_BYTES`]，扫描的段落数不超过 [`SCAN_STEPS`]；超出预算或超过 [`ROW_BYTES`] 的行算「没读」，按不生僻处理，日志里列在 `unread` 后面。最坏情况的 fuel 见 README。

#![no_std]

#[path = "../../../../docs/script-plugin-template/sdk/msime_script.rs"]
mod msime_script;

mod rare;

use core::ptr::addr_of_mut;
use msime_script::{candidates, event, hook, local_mode, log, rerank, Error};

/// 宿主交给插件的窗口最多 20 行（crates/script-runtime/src/limits.rs）。
const MAX_ROWS: usize = 20;
/// 单行文本最多读这么多字节。更长的行是整句候选，不是单字或词，读不下（`LIMIT`）就按不生僻处理。
const ROW_BYTES: usize = 256;
/// 一次 `candidates` 里读入并扫描的文本总字节数上限。
const SCAN_BYTES: usize = 768;
/// 一次 `candidates` 里扫描的段落数上限（见 `rare::scan`）。一行纯汉字、或者字母后面跟汉字（「T恤」）是一段，汉字和别的字符每交替一次多一段；常见的窗口每行一段，用不到一半。
const SCAN_STEPS: u32 = 80;

static mut TEXT: [u8; ROW_BYTES] = [0; ROW_BYTES];
static mut LINE: [u8; 256] = [0; 256];

/// 一行的判定。
const KEEP: u8 = 0;
const RARE: u8 = 1;
/// 宿主锁定的行（云、AI 和 AI 占位），点名也不会移动，插件不点名它们。
const LOCKED: u8 = 2;
/// 读不下（超长或超出预算），按不生僻处理，日志里单独列出。
const UNREAD: u8 = 3;

/// `candidates::SOURCE` 的取值，见 crates/engine/src/types.rs 的 `CandidateSource`。
mod source {
    pub const USER_DATABASE: u64 = 1;
    pub const CLOUD: u64 = 2;
    pub const AI: u64 = 3;
    pub const ENGLISH: u64 = 4;
    pub const QUICK_PHRASE: u64 = 5;
    pub const EMOJI: u64 = 6;
    pub const KAOMOJI: u64 = 7;
    pub const GENERATED: u64 = 8;
    pub const AI_PENDING: u64 = 12;
}

#[no_mangle]
pub extern "C" fn msime_abi() -> i32 {
    msime_script::ABI_VERSION
}

#[no_mangle]
pub extern "C" fn msime_on_event(kind: i32) -> i32 {
    if kind == hook::CANDIDATES {
        on_candidates();
    }
    0
}

fn on_candidates() {
    let count = match candidates::count() {
        Ok(count) => (count as usize).min(MAX_ROWS),
        Err(error) => return log::debug(reason(error)),
    };
    if count < 2 {
        return log::debug("keep");
    }
    // 临时日文模式的候选是日文，常用字集不覆盖日文新字体（気、込）。
    if event::scalar(event::LOCAL_MODE) == Ok(local_mode::TEMPORARY_JAPANESE) {
        return log::debug("skip: japanese");
    }
    let mut kinds = [KEEP; MAX_ROWS];
    let mut budget = Budget {
        bytes: SCAN_BYTES,
        steps: SCAN_STEPS,
    };
    for (index, kind) in kinds.iter_mut().enumerate().take(count) {
        *kind = match judge(index as u32, &mut budget) {
            Some(kind) => kind,
            // 窗口里有假名：这是日文候选，整窗不动。
            None => return log::debug("skip: kana"),
        };
    }
    let mut order = [0u32; MAX_ROWS];
    let mut named = 0;
    let mut moves = false;
    let mut rare_seen = false;
    for (index, kind) in kinds.iter().enumerate().take(count) {
        match *kind {
            LOCKED => {}
            RARE if index > 0 => rare_seen = true,
            _ => {
                // 一行不生僻的行排在某个生僻行后面，点名才会让它前移。
                moves |= rare_seen;
                order[named] = index as u32;
                named += 1;
            }
        }
    }
    let mut line = Line::new();
    if moves {
        line.push("order");
        for row in &order[..named] {
            line.number(*row);
        }
    } else {
        line.push("keep");
    }
    line.list(" rare", &kinds[..count], RARE);
    line.list(" unread", &kinds[..count], UNREAD);
    log::debug(line.as_str());
    if moves {
        // 宿主拒绝时窗口保持原样，不算失败。
        let _ = rerank::order(&order[..named]);
    }
}

/// 这一次 `candidates` 剩下的工作量。
struct Budget {
    /// 还能读入多少字节文本。
    bytes: usize,
    /// 还能扫描多少段。
    steps: u32,
}

/// 判定第 `index` 行，从 `budget` 里扣掉读入的字节和扫描的段落。含假名时是 `None`。
#[inline(never)]
fn judge(index: u32, budget: &mut Budget) -> Option<u8> {
    match candidates::meta(index, candidates::SOURCE) {
        Ok(source::CLOUD | source::AI | source::AI_PENDING) => return Some(LOCKED),
        // 用户词库、英文、快捷短语、emoji、颜文字、生成的文本：用户自己造的或者不是汉字词，不判定。
        Ok(
            source::USER_DATABASE
            | source::ENGLISH
            | source::QUICK_PHRASE
            | source::EMOJI
            | source::KAOMOJI
            | source::GENERATED,
        ) => return Some(KEEP),
        _ => {}
    }
    read(index, budget)
}

/// 读入并扫描第 `index` 行。和 [`judge`] 分开，不读文本的行（锁定、不判定的来源）不用为这一段付 fuel。
#[inline(never)]
fn read(index: u32, budget: &mut Budget) -> Option<u8> {
    if budget.bytes == 0 || budget.steps == 0 {
        return Some(UNREAD);
    }
    // 只给这一行剩下的预算那么大的缓冲：超出时宿主返回 LIMIT，什么都不复制，读入的总量不会超过预算。
    let room = budget.bytes.min(ROW_BYTES);
    // SAFETY: 插件单线程运行，缓冲只在这一次调用里用。
    let buffer = unsafe { &mut *addr_of_mut!(TEXT) };
    let buffer = &mut buffer[..room];
    let text = match candidates::bytes(index, buffer) {
        Ok(text) => text,
        Err(_) => return Some(UNREAD),
    };
    budget.bytes -= text.len();
    let flags = rare::scan(text, &mut budget.steps);
    if flags & rare::KANA != 0 {
        None
    } else if flags & rare::RARE != 0 {
        Some(RARE)
    } else if flags & rare::INCOMPLETE != 0 {
        Some(UNREAD)
    } else {
        Some(KEEP)
    }
}

fn reason(error: Error) -> &'static str {
    match error {
        Error::Denied => "skip: denied",
        Error::Absent => "skip: absent",
        _ => "skip: unreadable",
    }
}

/// 一行日志，例如 `order 0,2,3 rare 1`。只写 ASCII，写满就不再追加。
struct Line {
    buffer: &'static mut [u8; 256],
    len: usize,
}

impl Line {
    fn new() -> Self {
        // SAFETY: 插件单线程运行，每次事件只建一个 `Line`。
        let buffer = unsafe { &mut *addr_of_mut!(LINE) };
        Line { buffer, len: 0 }
    }

    fn byte(&mut self, byte: u8) {
        if self.len < self.buffer.len() {
            self.buffer[self.len] = byte;
            self.len += 1;
        }
    }

    fn push(&mut self, text: &str) {
        for byte in text.bytes() {
            self.byte(byte);
        }
    }

    /// 追加一个行号：紧跟在词后面时用空格分隔，跟在行号后面时用逗号。
    fn number(&mut self, value: u32) {
        let separator = if self.len > 0 && self.buffer[self.len - 1].is_ascii_digit() {
            b','
        } else {
            b' '
        };
        self.byte(separator);
        if value >= 10 {
            self.byte(b'0' + (value / 10 % 10) as u8);
        }
        self.byte(b'0' + (value % 10) as u8);
    }

    /// 有判定为 `kind` 的行时，追加 `label` 和这些行号。
    fn list(&mut self, label: &str, kinds: &[u8], kind: u8) {
        if !kinds.contains(&kind) {
            return;
        }
        self.push(label);
        for (index, _) in kinds.iter().enumerate().filter(|(_, k)| **k == kind) {
            self.number(index as u32);
        }
    }

    fn as_str(&self) -> &str {
        // SAFETY: 只写入过 ASCII。
        unsafe { core::str::from_utf8_unchecked(&self.buffer[..self.len]) }
    }
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    core::arch::wasm32::unreachable()
}
