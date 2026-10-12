//! hide-rare-chars：把候选窗口里含生僻字的候选隐藏起来。打 `kao` 时「丂」这类字不再出现在列表里。
//!
//! 只声明 `candidates`，申请 `read_candidates` 和 `filter_candidates`。「生僻」的定义和数据在 `plugins/script/shared/common_chars.rs`：一行里只要有一个字落在汉字区段、又不在常用字集（《通用规范汉字表》8105 字并上它们的繁体形）里，这一行就算生僻。
//!
//! 规则：
//! - 窗口第 0 行永远不点名。宿主另外保证引擎下标 0 不被隐藏；两者不一定是同一行，插件看不出来，也不需要看出来。
//! - 只判定引擎词库给的行（来源 0 词库、4 英文、9 兜底、11 整句模型）。用户词、快捷短语、emoji、颜文字、生成的文本、云和 AI 行（宿主锁定）都不点名，以后新增的来源也不点名。
//! - 窗口里有含假名的行（emoji、颜文字行除外），或者本地模式是临时日文，整窗不动：常用字集不收日文新字体（気、込），日文候选会被误判。
//! - 工作量有上限：所有行一共最多读 [`SCAN_BYTES`] 字节文本，第 1 行起最多细看 [`WIDE_CHARS`] 个宽字符，哪个先满就停在那一行，这一行和后面的行不判定、不隐藏；单行超过 [`ROW_BYTES`] 字节的也不判定。最坏情况的 fuel 见 README。
//!
//! fuel 的计法决定了写法：wasmi 在函数入口和每轮循环开头按函数体、循环体词法上的全部指令扣 fuel，不管走哪个分支。逐字节的扫描在 `scan` 里，见那里的说明；这里的逐行循环只读文本和分类，第 0 行只看有没有假名，查来源（每次约 110 fuel）只在要隐藏或含假名的行上才做。

#![no_std]

#[path = "../../../../docs/script-plugin-template/sdk/msime_script.rs"]
mod msime_script;

#[path = "../../shared/common_chars.rs"]
mod common_chars;
mod scan;

use core::ptr::addr_of_mut;
use msime_script::{candidates, event, filter, hook, local_mode, log, Error};

/// 候选窗口最多 20 行（宿主的上限）。
const MAX_ROWS: usize = 20;
/// 单行最多读这么多字节（约 85 个汉字），更长的行不判定。
const ROW_BYTES: usize = 256;
/// 一次 `candidates` 里所有行一共最多读这么多字节，读满就停。
const SCAN_BYTES: usize = 640;
/// 一次 `candidates` 里第 1 行起最多细看这么多个宽字符（U+3000 及以上，辅助平面的汉字按 3 个算，见 `scan` 的说明），看满就停。
const WIDE_CHARS: u32 = 100;

/// 候选来源（`candidates::SOURCE` 的值），见 crates/engine/src/types.rs 的 `CandidateSource`。
const SOURCE_DATABASE: u64 = 0;
const SOURCE_ENGLISH: u64 = 4;
const SOURCE_EMOJI: u64 = 6;
const SOURCE_KAOMOJI: u64 = 7;
const SOURCE_FALLBACK: u64 = 9;
const SOURCE_NEURAL_KEYBOARD: u64 = 11;

static mut TEXT: [u8; ROW_BYTES] = [0; ROW_BYTES];

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

/// 一次 `candidates` 的判定结果。
struct Plan {
    /// 要隐藏的行号，升序。
    rows: [u32; MAX_ROWS],
    len: usize,
    /// 工作量（[`SCAN_BYTES`]、[`WIDE_CHARS`]）用完时停在哪一行，这一行和后面的都没判定。
    stop: Option<u32>,
}

fn on_candidates() {
    if event::scalar(event::LOCAL_MODE) == Ok(local_mode::TEMPORARY_JAPANESE) {
        return log::debug("skip: japanese");
    }
    let count = match candidates::count() {
        Ok(count) => (count as usize).min(MAX_ROWS),
        Err(error) => return log::debug(reason(error)),
    };
    // 只有第 0 行时没有能隐藏的行。
    if count < 2 {
        return log::debug("keep");
    }
    let plan = match scan(count) {
        Some(plan) => plan,
        None => return log::debug("skip: kana"),
    };
    let result = if plan.len == 0 {
        Ok(())
    } else {
        filter::hide(&plan.rows[..plan.len])
    };
    summary(&plan, result);
}

/// 逐行读文本、分类，收集要隐藏的行。窗口里有日文候选时是 `None`。
fn scan(count: usize) -> Option<Plan> {
    let mut plan = Plan {
        rows: [0; MAX_ROWS],
        len: 0,
        stop: None,
    };
    // SAFETY: 插件单线程运行，缓冲只在这一次调用里用。
    let buffer = unsafe { &mut *addr_of_mut!(TEXT) };
    let mut budget = SCAN_BYTES;
    let mut scanner = scan::Scanner { budget: WIDE_CHARS };
    let mut row = 0;
    while row < count {
        let cap = budget.min(ROW_BYTES);
        match candidates::bytes(row as u32, &mut buffer[..cap]) {
            Ok(text) => {
                budget -= text.len();
                // 第 0 行不会被点名，只看它是不是日文。
                let kind = if row == 0 {
                    if scan::has_kana(text, 0) {
                        scan::KANA
                    } else {
                        scan::PLAIN
                    }
                } else {
                    scanner.row(text)
                };
                match kind {
                    scan::PLAIN => {}
                    scan::RARE => {
                        if row > 0 && judged(row as u32) {
                            plan.rows[plan.len] = row as u32;
                            plan.len += 1;
                        }
                    }
                    scan::KANA => {
                        if !decorative(row as u32) {
                            return None;
                        }
                    }
                    // 工作量用完：这一行和后面的都不判定。
                    _ => {
                        plan.stop = Some(row as u32);
                        break;
                    }
                }
            }
            // 预算不够读这一行：这一行和后面的都不判定。
            Err(Error::Limit) if cap < ROW_BYTES => {
                plan.stop = Some(row as u32);
                break;
            }
            // 单行太长，或者读不到：这一行按不生僻处理。
            Err(_) => {}
        }
        row += 1;
    }
    Some(plan)
}

/// 这一行的来源是不是引擎词库。只有它们才会被隐藏；读不到来源时不隐藏。
#[inline(never)]
fn judged(row: u32) -> bool {
    matches!(
        candidates::meta(row, candidates::SOURCE),
        Ok(SOURCE_DATABASE | SOURCE_ENGLISH | SOURCE_FALLBACK | SOURCE_NEURAL_KEYBOARD)
    )
}

/// 这一行是不是 emoji、颜文字。颜文字常带片假名（ツ、ノ），不能因此把整窗当成日文。
#[inline(never)]
fn decorative(row: u32) -> bool {
    matches!(
        candidates::meta(row, candidates::SOURCE),
        Ok(SOURCE_EMOJI | SOURCE_KAOMOJI)
    )
}

/// 读候选失败时的日志。
fn reason(error: Error) -> &'static str {
    match error {
        Error::Denied => "skip: denied",
        Error::Absent => "skip: absent",
        _ => "skip: unreadable",
    }
}

/// 宿主拒绝隐藏时日志里的原因。
fn refusal(error: Error) -> &'static [u8] {
    match error {
        Error::Denied => b" denied",
        Error::Absent => b" absent",
        Error::Invalid => b" invalid",
        Error::Limit => b" limit",
        Error::Other(_) => b" error",
    }
}

/// 写一行日志：`keep`、`hide 2,5`，读满预算时后面加 ` stop 7`，宿主拒绝隐藏时加 ` denied` 之类的原因。
#[inline(never)]
fn summary(plan: &Plan, result: Result<(), Error>) {
    // 最长是 "hide " 加 19 个行号和逗号（47 字节）、" stop 19" 和 " invalid"，不到 96 字节。
    let mut line = Line {
        bytes: [0; 96],
        len: 0,
    };
    if plan.len == 0 {
        line.push(b"keep");
    } else {
        line.push(b"hide ");
        for (i, row) in plan.rows[..plan.len].iter().enumerate() {
            if i > 0 {
                line.push(b",");
            }
            line.number(*row);
        }
    }
    if let Some(stop) = plan.stop {
        line.push(b" stop ");
        line.number(stop);
    }
    if let Err(error) = result {
        line.push(refusal(error));
    }
    // SAFETY: 只拼进了 ASCII。
    log::debug(unsafe { core::str::from_utf8_unchecked(&line.bytes[..line.len]) });
}

struct Line {
    bytes: [u8; 96],
    len: usize,
}

impl Line {
    fn push(&mut self, text: &[u8]) {
        self.bytes[self.len..self.len + text.len()].copy_from_slice(text);
        self.len += text.len();
    }

    /// 行号不超过 19。
    fn number(&mut self, value: u32) {
        if value >= 10 {
            self.push(&[b'0' + (value / 10) as u8]);
        }
        self.push(&[b'0' + (value % 10) as u8]);
    }
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    core::arch::wasm32::unreachable()
}
