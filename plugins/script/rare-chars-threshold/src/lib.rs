//! rare-chars-threshold：可调的「隐藏生僻字」，也是插件设置项（`[[script.settings]]`）的参考样板。
//!
//! 生僻的判定、不动哪些行、工作量上限都和 hide-rare-chars 相同（逐字扫描共用 `plugins/script/shared/rare_scan.rs`），多出的是清单里声明的两项设置，用户在「脚本插件（实验）」的插件详情里填写：
//! - `scope`（选项）：`any` 是含生僻字就隐藏，和 hide-rare-chars 相同，是默认值；`single` 只隐藏恰好是一个生僻字的行，「囧」隐藏，「囧rz」「中囧」照常显示。
//! - `keep_rows`（数字，1 到 9，默认 1）：窗口最前面这几行不隐藏。hide-rare-chars 固定是 1（只保留第 0 行）。
//!
//! 设置值在 `msime_init` 里读一次，存进静态变量：读设置不需要能力，`msime_init` 里也能读；输入法在用户改了设置之后的下一次聚焦用新值重建实例，所以插件不必每次 `candidates` 都重读。两项都取默认值时，行为和 hide-rare-chars 逐行相同（夹具的默认一份就是 hide-rare-chars 的全部事件）。
//!
//! 为什么另起一个插件而不是给 hide-rare-chars 加设置：授权绑定在包的内容哈希上，给它加设置会改变清单和模块，已经装了它的用户要重新授权；而且只想要固定行为的用户也不必多看两项设置。

#![no_std]

#[path = "../../shared/common_chars.rs"]
mod common_chars;
#[path = "../../shared/rare_scan.rs"]
mod scan;

use core::ptr::addr_of_mut;
use msime_script_sdk::{candidates, event, filter, hook, local_mode, log, settings, Error};

/// 候选窗口最多 20 行（宿主的上限）。
const MAX_ROWS: usize = 20;
/// 单行最多读这么多字节（约 85 个汉字），更长的行不判定。
const ROW_BYTES: usize = 256;
/// 一次 `candidates` 里所有行一共最多读这么多字节，读满就停。
const SCAN_BYTES: usize = 640;
/// 一次 `candidates` 里最多细看这么多个宽字符（U+3000 及以上，辅助平面的汉字按 3 个算，见 `scan` 的说明），看满就停。
const WIDE_CHARS: u32 = 100;

/// 设置项在清单里的顺序，`setting_value` 按它取值。
const SETTING_SCOPE: u32 = 0;
const SETTING_KEEP_ROWS: u32 = 1;
/// `keep_rows` 的范围，和清单的 `min`、`max` 相同；宿主交来的值已经按清单核对过，这里只是读不出来时的退路。
const DEFAULT_KEEP_ROWS: usize = 1;
const MAX_KEEP_ROWS: usize = 9;

/// 候选来源（`candidates::SOURCE` 的值），见 crates/engine/src/types.rs 的 `CandidateSource`。
const SOURCE_DATABASE: u64 = 0;
const SOURCE_ENGLISH: u64 = 4;
const SOURCE_EMOJI: u64 = 6;
const SOURCE_KAOMOJI: u64 = 7;
const SOURCE_FALLBACK: u64 = 9;
const SOURCE_NEURAL_KEYBOARD: u64 = 11;

static mut TEXT: [u8; ROW_BYTES] = [0; ROW_BYTES];
/// `scope = "single"`：只隐藏单个字的行。
static mut SINGLE_ONLY: bool = false;
/// `keep_rows`：前几行不隐藏。
static mut KEEP_ROWS: usize = DEFAULT_KEEP_ROWS;

#[no_mangle]
pub extern "C" fn msime_abi() -> i32 {
    msime_script_sdk::ABI_VERSION
}

/// 实例化后调用一次：读两项设置。读不出来（宿主版本太旧、值不合规被宿主换回默认值之外的情况）时用默认值，插件照常工作。
#[no_mangle]
pub extern "C" fn msime_init() -> i32 {
    let mut buffer = [0u8; 16];
    let single = matches!(settings::value(SETTING_SCOPE, &mut buffer), Ok("single"));
    let keep = match settings::value(SETTING_KEEP_ROWS, &mut buffer) {
        Ok(text) => parse_keep_rows(text.as_bytes()),
        Err(_) => DEFAULT_KEEP_ROWS,
    };
    // SAFETY: 插件单线程运行，这两个值只在这里写。
    unsafe {
        SINGLE_ONLY = single;
        KEEP_ROWS = keep;
    }
    log::debug(if single {
        settings_line(b"settings: scope=single keep_rows=", keep)
    } else {
        settings_line(b"settings: scope=any keep_rows=", keep)
    });
    0
}

#[no_mangle]
pub extern "C" fn msime_on_event(kind: i32) -> i32 {
    if kind == hook::CANDIDATES {
        on_candidates();
    }
    0
}

/// `keep_rows` 的十进制文本。不是 1 到 9 的一位数时用默认值。
fn parse_keep_rows(text: &[u8]) -> usize {
    match text {
        [digit @ b'1'..=b'9'] => usize::from(digit - b'0').min(MAX_KEEP_ROWS),
        _ => DEFAULT_KEEP_ROWS,
    }
}

/// 初始化日志：`prefix` 后面接一位数。
fn settings_line(prefix: &'static [u8], keep: usize) -> &'static str {
    static mut LINE: [u8; 48] = [0; 48];
    // SAFETY: 单线程，只在初始化时用一次；拼进去的都是 ASCII。
    unsafe {
        let line = &mut *addr_of_mut!(LINE);
        line[..prefix.len()].copy_from_slice(prefix);
        line[prefix.len()] = b'0' + keep as u8;
        core::str::from_utf8_unchecked(&line[..prefix.len() + 1])
    }
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
    // SAFETY: 单线程，初始化之后只读。
    let (keep, single) = unsafe { (KEEP_ROWS, SINGLE_ONLY) };
    // 所有行都在保留的范围里时没有能隐藏的行。
    if count <= keep {
        return log::debug("keep");
    }
    let plan = match scan(count, keep, single) {
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

/// 逐行读文本、分类，收集要隐藏的行。窗口里有日文候选时是 `None`。前 `keep` 行，以及 `single` 时不止一个字的行，都只看有没有假名，不判定生僻。
fn scan(count: usize, keep: usize, single: bool) -> Option<Plan> {
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
                let kind = if row < keep || (single && !one_char(text)) {
                    kana_only(text)
                } else {
                    scanner.row(text)
                };
                match kind {
                    scan::PLAIN => {}
                    scan::RARE => {
                        if judged(row as u32) {
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

/// 不判定生僻的行只看是不是日文。
#[inline(never)]
fn kana_only(text: &[u8]) -> u8 {
    if scan::has_kana(text, 0) {
        scan::KANA
    } else {
        scan::PLAIN
    }
}

/// 这一行是不是恰好一个字。汉字都是三字节或四字节字符，所以只认这两种长度，按首字节判断字符的长度是否正好等于整行。
#[inline(never)]
fn one_char(text: &[u8]) -> bool {
    match text.first() {
        Some(0xE0..=0xEF) => text.len() == 3,
        Some(0xF0..=0xF4) => text.len() == 4,
        _ => false,
    }
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

/// 写一行日志：`keep`、`hide 2,5`，读满预算时后面加 ` stop 7`，宿主拒绝隐藏时加 ` denied` 之类的原因。格式和 hide-rare-chars 相同。
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
