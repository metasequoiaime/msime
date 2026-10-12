//! traditional-glyph：在候选旁注出繁体字形。单字列出全部繁体写法（发 → 發/髮，台 → 臺/檯/颱/台），多字候选给出按 OpenCC 词表转换后的结果（头发 → 頭髮，而不是逐字的 頭發）。
//!
//! 只声明 `candidates`，申请 `read_candidates` 和 `annotate_candidates`。字形以 OpenCC 的 `s2t`（OpenCC 标准繁体）为准，数据和输入法自己的繁体输出来自同一份 OpenCC 词表；单字和 2 到 4 字的词组和它一致，5 字以上的词组、两个词组拼接处的个别情况和兼容汉字不同（只收了短词组、不做规范化，见 README 的「已知的局限」）。对照表见 [`table`]。
//!
//! 每次投递的做法：
//!
//! 1. 临时日文模式下整窗不注。
//! 2. 逐行读文本（最多 20 行），逐字查表，同时写出逐字取第一个值的结果。文本超过 128 字节、超过 32 个字、含格式字符（宿主不接受这样的注释）、转换后超过 32 个 UTF-16 码元的行不注；含 ASCII 控制字符的行由宿主拒绝，结果一样。任意一行含假名就整窗不注（日文候选里的汉字不该被注成繁体），颜文字行里的假名除外。
//! 3. 单字候选改为列出全部值，用「/」连起来。多字候选里有在词组里会变的字（发、了、只这类值不止一个的字，以及霉、恒这类在个别词组里保留原字的字）时，在 2 到 4 字的词组表上做正向最长匹配，命中的词组就地换成词组给的值。结果和原文不同才注。
//! 4. 含繁体专用字（們、這）的行比含简体专用字（们、这）的行多时，判定这是注音、粤语这类本来就输出繁体的方案，整窗不注，免得给 台、只、了 这类简繁同形字注出噪音。
//!
//! 英文、emoji、颜文字和 AI 占位行（来源 4、6、7、12）不注。这几类候选正常不含要转换的汉字，所以来源只对真要加注释的行和含假名的行才读（后者用来区分颜文字和日文候选），省下大部分行的一次宿主调用。云和 AI 行照常注释。
//!
//! fuel：wasmi 在函数入口按函数体里（循环以外）词法上的全部指令扣 fuel，每轮循环按循环体的全部指令扣，不管走哪个分支（`local.get` 也算一条）。所以逐字的循环体只做取字、保存和前进，查表和写出交给很小的不内联函数，少见的情况（两字节、四字节字符，假名，词组）各自拆开，热路径上的数组访问不做边界检查。工作量有上限：每一步动手之前先按实测的单价从 [`WORK_BUDGET`] 里扣（见 [`ROW_COST`] 等常量），不够扣时这一行和后面的行都不注，所以一次投递的 fuel 不会超过预算加上一点固定开销。最坏情况的 fuel 见 README。

#![no_std]

#[path = "../../../../docs/script-plugin-template/sdk/msime_script.rs"]
mod msime_script;
mod table;

use core::ptr::addr_of_mut;
use msime_script::{annotate, candidates, event, hook, local_mode, log, Error};

/// 窗口最多几行，和宿主的 `MAX_TRANSFORM_ROWS` 相同。
const MAX_ROWS: usize = 20;
/// 注释的上限：32 个 UTF-16 码元（宿主的规则；码元不超过 32 时字节一定不超过 96）。一行也最多看这么多个字：转换不改变字数。
const MAX_UNITS: usize = 32;
/// 读一行文本的上限。输出的每个字至少一个码元、输入的每个字至多 4 字节，所以超过 128 字节的行转换后一定放不进注释。
const MAX_TEXT: usize = 128;
/// 文本和注释的缓冲都多留 4 个字节：读写一个字时一律读写 4 个字节，再按实际长度前进。
const TEXT_BUFFER: usize = MAX_TEXT + 4;
const OUT_BUFFER: usize = MAX_UNITS * 4 + 4;
/// 一次投递的工作量预算，单位约等于 fuel。各项单价按 rustc 1.97.1、wasmi 2.0 实测取整、往高里取，所以实际花费不超过预算；`candidates` 单次上限是 125 000，同一个按键里的几次投递共用一份，这里留在 40 000 以内。正常的窗口（20 行、每行几个字）用不完。
const WORK_BUDGET: u32 = 34_000;
/// 每一行的固定开销：读文本（超过 128 字节、宿主返回 LIMIT 的行也算）、解码的入口、行循环和最后的注释循环。
const ROW_COST: u32 = 360;
/// 每个字：解码、查表、写出逐字转换的结果。下标是压缩值的第 20..23 位，即 `慢路径 | 字节数 << 1`，单价取同一下标里最贵的那条路径：ASCII（2）约 101；U+3400..U+FDFF 的三字节字符（6）约 235；其余三字节字符（7）走 [`outside`] 的快速区间约 294（全角标点），落进 [`kana_mark`] 约 317（注音符号、「・」），落进 [`forbidden_mark`] 约 427（“”‘’…— 这类通用标点、泰文等）；两字节字符（5）快速区间约 405（é），否则约 538（©、阿拉伯字母）；四字节字符（9）查表约 413、快速区间约 405，否则约 538。
const CHAR_COSTS: [u32; 16] = [0, 0, 110, 0, 0, 560, 245, 440, 0, 560, 0, 0, 0, 0, 0, 0];
/// 有注释的行：读来源、交给宿主、写进日志。
const ANNOTATION_COST: u32 = 450;
/// 有在词组里会变的字的多字候选：准备词组匹配（每个字一份）。
const PHRASE_CHAR_COST: u32 = 130;
/// 在一个位置上找词组（不含查表）、每查一次词组表。
const PHRASE_STEP_COST: u32 = 220;
const LOOKUP_COST: u32 = 450;
/// 日志一行：`ann` 加每行的 ` 行号:注释`，最后可能有 ` (budget)`。
const LOG_BYTES: usize = 3 + MAX_ROWS * (4 + MAX_UNITS * 3) + 16;

/// 颜文字的来源编号（engine 的 `CandidateSource::Kaomoji`），连同英文、emoji 和 AI 占位一起不注。
const ENGLISH: u64 = 4;
const EMOJI: u64 = 6;
const KAOMOJI: u64 = 7;
const AI_PENDING: u64 = 12;

// 一个字压成一个 u64，各位的含义：
/// 第 0..15 位：码位的低 16 位（词组的键只用基本平面的字）。
const CP_MASK: u64 = 0xFFFF;
/// 第 16、17 位：写出的字节数减 1。
const OUT_SHIFT: u32 = 16;
/// 第 18 位：写出的第一个值和原字不同。
const CHANGED: u64 = 1 << 18;
/// 第 19 位：写出的是四字节字符（占两个 UTF-16 码元）。
const OUT_WIDE: u64 = 1 << 19;
/// 第 20 位：这个字走的是 [`plain3`] 或 [`other`]（扣预算时单价更高，见 [`CHAR_COSTS`]）。
const SLOW: u64 = 1 << 20;
/// 第 21..23 位：原字的字节数。
const SIZE_SHIFT: u32 = 21;
/// 第 24..31 位：在注释里的字节起点（由 [`scan`] 填）。
const AT_SHIFT: u32 = 24;
/// 第 32..62 位：条目（见 [`table`]）；第 63 位：这个字要整行或整窗拒绝，这时其余各位都不用。
const ENTRY_SHIFT: u32 = 32;
const REJECT: u64 = 1 << 63;
/// 拒绝时的第 32 位：1 是假名（整窗不注），0 是注释不允许的字符（这一行不注）。
const KANA_BIT: u64 = 1 << ENTRY_SHIFT;

/// 一行里的字和逐字转换的结果。
struct Chars {
    packed: [u64; MAX_UNITS],
    count: usize,
    /// 各字压缩值的按位或。
    all: u64,
    /// 注释已经写了多少字节。
    at: usize,
}

struct State {
    text: [u8; TEXT_BUFFER],
    chars: Chars,
    /// 每行的注释；长度为 0 表示不注。
    out: [[u8; OUT_BUFFER]; MAX_ROWS],
    out_len: [u8; MAX_ROWS],
    log: [u8; LOG_BYTES],
    /// 剩下的工作量预算，见 [`WORK_BUDGET`]。
    work_left: u32,
    /// 含繁体专用字、简体专用字的行数。
    traditional: u32,
    simplified: u32,
}

static mut STATE: State = State {
    text: [0; TEXT_BUFFER],
    chars: Chars {
        packed: [0; MAX_UNITS],
        count: 0,
        all: 0,
        at: 0,
    },
    out: [[0; OUT_BUFFER]; MAX_ROWS],
    out_len: [0; MAX_ROWS],
    log: [0; LOG_BYTES],
    work_left: 0,
    traditional: 0,
    simplified: 0,
};

#[no_mangle]
pub extern "C" fn msime_abi() -> i32 {
    msime_script::ABI_VERSION
}

#[no_mangle]
pub extern "C" fn msime_on_event(kind: i32) -> i32 {
    if kind == hook::CANDIDATES {
        // SAFETY: 插件单线程运行，STATE 只在这一次调用里用，每次投递开头重置，不跨事件保留内容。
        let state = unsafe { &mut *addr_of_mut!(STATE) };
        on_candidates(state);
    }
    0
}

/// 一行的结果（[`process_row`] 的返回值）。
const NEXT: u8 = 0;
/// 含假名：整窗不注。
const KANA: u8 = 1;
/// 预算用完：这一行和后面的行都不注。
const SPENT: u8 = 2;
/// 读不到候选：整窗不注。分别对应宿主的 `DENIED`、`ABSENT` 和其他错误。
const DENIED: u8 = 3;
const ABSENT: u8 = 4;
const UNREADABLE: u8 = 5;

fn on_candidates(state: &mut State) {
    if event::scalar(event::LOCAL_MODE) == Ok(local_mode::TEMPORARY_JAPANESE) {
        return log::debug("skip: japanese");
    }
    let rows = match candidates::count() {
        Ok(rows) => (rows as usize).min(MAX_ROWS),
        Err(error) => return log::debug(reason(failure(error))),
    };
    state.work_left = WORK_BUDGET;
    state.traditional = 0;
    state.simplified = 0;
    state.out_len = [0; MAX_ROWS];
    let mut spent = false;
    for row in 0..rows {
        match process_row(state, row) {
            NEXT => {}
            SPENT => {
                spent = true;
                break;
            }
            other => return log::debug(reason(other)),
        }
    }
    if state.traditional > state.simplified {
        return log::debug("skip: traditional");
    }
    annotate_rows(state, rows, spent);
}

/// 宿主返回的错误对应的结果。`LIMIT`（这一行超过 128 字节）只跳过这一行。
fn failure(error: Error) -> u8 {
    match error {
        Error::Limit => NEXT,
        Error::Denied => DENIED,
        Error::Absent => ABSENT,
        _ => UNREADABLE,
    }
}

fn reason(result: u8) -> &'static str {
    match result {
        KANA => "skip: kana",
        DENIED => "skip: denied",
        ABSENT => "skip: absent",
        _ => "skip: unreadable",
    }
}

/// 从剩下的预算里扣 `cost`；不够扣时什么都不改，返回 `false`。
fn charge(work_left: &mut u32, cost: u32) -> bool {
    if *work_left < cost {
        return false;
    }
    *work_left -= cost;
    true
}

/// 读第 `row` 行，逐字查表并写出逐字转换的结果；有单字表的键时交给 [`finish_row`]。
#[inline(never)]
fn process_row(state: &mut State, row: usize) -> u8 {
    // 先扣再读：超过 128 字节的行（宿主返回 LIMIT）也要一次宿主调用，同样算一行的开销。
    if !charge(&mut state.work_left, ROW_COST) {
        return SPENT;
    }
    let len = match candidates::bytes(row as u32, &mut state.text[..MAX_TEXT]) {
        Ok(text) => text.len(),
        Err(error) => return failure(error),
    };
    match scan(
        &state.text,
        len,
        &mut state.chars,
        &mut state.out[row],
        &mut state.work_left,
    ) {
        SCANNED => {}
        REJECTED_KANA => return kana_row(row),
        OUT_OF_WORK => return SPENT,
        _ => return NEXT,
    }
    let marks = (state.chars.all >> ENTRY_SHIFT) as u32;
    state.traditional += u32::from(marks & table::TRAD != 0);
    state.simplified += u32::from(marks & table::SIMP != 0);
    if table::has_key(marks) {
        finish_row(state, row)
    } else {
        NEXT
    }
}

/// 有单字表的键的行：算出注释（单字列出全部值，多字候选找词组），看来源，记下注释的长度。
#[inline(never)]
fn finish_row(state: &mut State, row: usize) -> u8 {
    if !charge(&mut state.work_left, ANNOTATION_COST) {
        return SPENT;
    }
    let chars = &mut state.chars;
    let len = if chars.count == 1 {
        single(chars, &mut state.out[row])
    } else {
        let changed = if (chars.all >> ENTRY_SHIFT) as u32 & table::IN_PHRASE != 0 {
            match phrases(chars, &mut state.out[row], &mut state.work_left) {
                Some(changed) => changed,
                None => return SPENT,
            }
        } else {
            chars.all & CHANGED != 0
        };
        if changed && fits(chars) {
            chars.at
        } else {
            0
        }
    };
    if len == 0 {
        return NEXT;
    }
    // 英文、emoji、颜文字和 AI 占位行不注。这几类候选正常不会含有要转换的汉字，所以只对真要加注释的行读来源。
    match candidates::meta(row as u32, candidates::SOURCE) {
        Ok(ENGLISH | EMOJI | KAOMOJI | AI_PENDING) => {}
        Ok(_) => state.out_len[row] = len as u8,
        Err(error) => return failure(error),
    }
    NEXT
}

/// 含假名的行：颜文字（常用片假名）和英文、emoji、AI 占位行不算日文候选，其余的让整窗不注。
#[inline(never)]
fn kana_row(row: usize) -> u8 {
    match candidates::meta(row as u32, candidates::SOURCE) {
        Ok(ENGLISH | EMOJI | KAOMOJI | AI_PENDING) => NEXT,
        Ok(_) => KANA,
        Err(error) => failure(error),
    }
}

/// 单字：注释是全部值用「/」连起来，返回字节数。最长的是「臺/檯/颱/台」，7 个码元。
#[inline(never)]
fn single(chars: &Chars, out: &mut [u8; OUT_BUFFER]) -> usize {
    let joined = table::joined((chars.packed[0] >> ENTRY_SHIFT) as u32);
    out[..joined.len()].copy_from_slice(joined);
    joined.len()
}

/// 写出的结果不超过 32 个 UTF-16 码元。只有写出了四字节字符（表里的值或原样写出的字）才需要逐字数。
fn fits(chars: &Chars) -> bool {
    chars.all & OUT_WIDE == 0 || count_units(chars) <= MAX_UNITS
}

#[inline(never)]
fn count_units(chars: &Chars) -> usize {
    let wide = chars.packed[..chars.count]
        .iter()
        .filter(|&&packed| packed & OUT_WIDE != 0)
        .count();
    chars.count + wide
}

/// 把算好的注释交给宿主，并写一行日志：`ann 0:發/髮 3:頭髮`，没有注释时是 `ann none`；预算用完时末尾加 ` (budget)`。
#[inline(never)]
fn annotate_rows(state: &mut State, rows: usize, spent: bool) {
    let mut log_len = put(&mut state.log, 0, b"ann");
    for row in 0..rows {
        if state.out_len[row] != 0 {
            log_len = annotate_row(state, row, log_len);
        }
    }
    if log_len == 3 {
        log_len = put(&mut state.log, log_len, b" none");
    }
    if spent {
        log_len = put(&mut state.log, log_len, b" (budget)");
    }
    // SAFETY: 日志由 ASCII 和注释的 UTF-8 拼成（见 annotate_row）。
    log::debug(unsafe { core::str::from_utf8_unchecked(&state.log[..log_len]) });
}

/// 给第 `row` 行加注释，成功时把「 行号:注释」接在日志后面，返回日志的新长度。
#[inline(never)]
fn annotate_row(state: &mut State, row: usize, log_len: usize) -> usize {
    let out = &state.out[row][..state.out_len[row] as usize];
    // SAFETY: 注释由宿主给的合法 UTF-8 文本里整字复制的片段、值池里的整字和 ASCII 的「/」拼成。
    let text = unsafe { core::str::from_utf8_unchecked(out) };
    // 宿主拒绝时这一行不注，不算失败。长度和格式字符都先检查过，只有含 ASCII 控制字符的行会走到这里（见 scan）。
    if annotate::set(row as u32, text).is_err() {
        return log_len;
    }
    // 一位数的行号从第二个字节起写，前面仍是空格。
    let mut number = [b' ', b'0' + (row / 10) as u8, b'0' + (row % 10) as u8, b':'];
    let skip = usize::from(row < 10);
    number[skip] = b' ';
    let log_len = put(&mut state.log, log_len, &number[skip..]);
    put(&mut state.log, log_len, out)
}

/// 把 `bytes` 接在日志的第 `at` 个字节之后，返回新的长度。
fn put(log: &mut [u8; LOG_BYTES], at: usize, bytes: &[u8]) -> usize {
    log[at..at + bytes.len()].copy_from_slice(bytes);
    at + bytes.len()
}

/// [`scan`] 的结果。
const SCANNED: u8 = 0;
/// 太长，或者含注释不允许的字符：这一行不注。
const REJECTED_ROW: u8 = 1;
const REJECTED_KANA: u8 = 2;
/// 预算用完。
const OUT_OF_WORK: u8 = 3;

/// 把 `text[..len]`（宿主给的合法 UTF-8）逐字解码、查表，存进 `chars`，同时把逐字取第一个值的结果写进 `out`。
///
/// 每个字按 [`CHAR_COSTS`] 从预算里扣，不够扣时停下，返回 [`OUT_OF_WORK`]。
///
/// 逐字执行的循环，循环体只有 ASCII 的处理、扣预算、保存和前进；多字节字符交给不内联的 [`wide`]。wasmi 每轮按循环体的全部指令扣 fuel，内联进来的分支每个字都要付。ASCII 控制字符（快捷短语里可能有换行）也不在这里拦：宿主会拒绝含控制字符的注释（返回 `INVALID`），结果同样是这一行不注，不算失败。
#[inline(never)]
fn scan(
    text: &[u8; TEXT_BUFFER],
    len: usize,
    chars: &mut Chars,
    out: &mut [u8; OUT_BUFFER],
    work_left: &mut u32,
) -> u8 {
    let len = len.min(MAX_TEXT);
    let mut work = *work_left;
    let mut all = 0;
    let mut count = 0;
    let mut at = 0;
    let mut i = 0;
    let mut result = SCANNED;
    while i < len {
        if count == MAX_UNITS {
            result = REJECTED_ROW;
            break;
        }
        let byte = text[i];
        let packed = if byte >= 0x80 {
            wide(text, i, out, at)
        } else {
            out[at & (MAX_UNITS * 4 - 1)] = byte;
            u64::from(byte) | 1 << SIZE_SHIFT
        };
        if packed & REJECT != 0 {
            result = rejected(packed);
            break;
        }
        let cost = CHAR_COSTS[(packed >> 20 & 15) as usize];
        if work < cost {
            result = OUT_OF_WORK;
            break;
        }
        work -= cost;
        chars.packed[count] = packed | (at as u64) << AT_SHIFT;
        all |= packed;
        count += 1;
        at += (packed >> OUT_SHIFT & 3) as usize + 1;
        i += (packed >> SIZE_SHIFT & 7) as usize;
    }
    *work_left = work;
    chars.count = count;
    chars.all = all;
    chars.at = at;
    result
}

#[inline(never)]
fn rejected(packed: u64) -> u8 {
    if packed & KANA_BIT != 0 {
        REJECTED_KANA
    } else {
        REJECTED_ROW
    }
}

/// `text[i]` 开头的多字节字符（宿主给的合法 UTF-8，不校验）：查表，把它逐字转换的结果写到 `out[at..]`，返回压缩值。
///
/// 只就地处理最常见的情况：U+3400..U+FDFF 的三字节字符，汉字都在这里，查表没有循环（[`table::bmp_entry`]）。其余的三字节字符（标点、假名、符号）交给 [`plain3`]，两字节、四字节字符交给 [`other`]。
#[inline(never)]
fn wide(text: &[u8; TEXT_BUFFER], i: usize, out: &mut [u8; OUT_BUFFER], at: usize) -> u64 {
    // SAFETY: 调用方保证 i < MAX_TEXT，缓冲多留了 4 个字节，读 4 个字节不越界。多读的字节只在字符本身更短时出现，会被掩掉。
    let bytes = unsafe { text.as_ptr().add(i).cast::<u32>().read_unaligned() };
    let cp = (bytes & 0x0F) << 12 | (bytes >> 8 & 0x3F) << 6 | bytes >> 16 & 0x3F;
    if bytes & 0xF0 != 0xE0 {
        return other(bytes, out, at);
    }
    if !(0x3400..0xFE00).contains(&cp) {
        return plain3(cp, bytes, out, at);
    }
    let entry = table::bmp_entry(cp);
    write_first(u64::from(cp) | 3 << SIZE_SHIFT, bytes, entry, out, at)
}

/// U+3400..U+FDFF 以外的三字节字符：不在单字表里（生成器保证），是假名或注释不允许的字符时返回拒绝的压缩值，否则原样写出。返回值同 [`wide`]。
#[inline(never)]
fn plain3(cp: u32, bytes: u32, out: &mut [u8; OUT_BUFFER], at: usize) -> u64 {
    let mark = outside(cp);
    if mark != 0 {
        return mark;
    }
    // SAFETY: 同 write_first。
    unsafe {
        out.as_mut_ptr()
            .add(at & (MAX_UNITS * 4 - 1))
            .cast::<u32>()
            .write_unaligned(bytes)
    };
    u64::from(cp) | 2 << OUT_SHIFT | SLOW | 3 << SIZE_SHIFT
}

/// 两字节、四字节字符，返回值同 [`wide`]。第二、第三平面的字查表，其余的看是不是注释不允许的字符，不是就原样写出。
#[inline(never)]
fn other(bytes: u32, out: &mut [u8; OUT_BUFFER], at: usize) -> u64 {
    let (b0, b1, b2, b3) = (
        bytes & 0xFF,
        bytes >> 8 & 0x3F,
        bytes >> 16 & 0x3F,
        bytes >> 24 & 0x3F,
    );
    let (cp, size) = if b0 < 0xE0 {
        ((b0 & 0x1F) << 6 | b1, 2)
    } else {
        ((b0 & 0x07) << 18 | b1 << 12 | b2 << 6 | b3, 4)
    };
    let entry = if (0x20000..0x40000).contains(&cp) {
        table::sip_entry(cp)
    } else {
        let mark = outside(cp);
        if mark != 0 {
            return mark;
        }
        0
    };
    write_first(
        u64::from(cp) & CP_MASK | SLOW | size << SIZE_SHIFT,
        bytes,
        entry,
        out,
        at,
    )
}

/// 把一个字逐字转换的结果（单字表的键取第一个值，其余原样）写到 `out[at..]`，返回压缩值：`base`（码位和原字字节数）加上写出的长度、是否改变和条目。
#[inline(always)]
fn write_first(base: u64, bytes: u32, entry: u32, out: &mut [u8; OUT_BUFFER], at: usize) -> u64 {
    let (written, flags) = if table::has_key(entry) {
        let value = table::first_value(entry);
        let wide = value & 0xF8 == 0xF0;
        let flags = ((2 + u64::from(wide)) << OUT_SHIFT)
            | (u64::from(wide) * OUT_WIDE)
            | (u64::from(table::changes(entry, 0)) * CHANGED);
        (value, flags)
    } else {
        // 原样写出的四字节字符（emoji、不在表里的辅助平面字）在 UTF-16 里也占两个码元，同样要让 fits 逐字数。
        let size = base >> SIZE_SHIFT & 7;
        (
            bytes,
            ((size - 1) << OUT_SHIFT) | (u64::from(size == 4) * OUT_WIDE),
        )
    };
    // SAFETY: 调用方保证 at 不超过 (MAX_UNITS - 1) * 4，缓冲多留了 4 个字节。多写的字节会被下一个字覆盖，或者落在有效长度之外。
    unsafe {
        out.as_mut_ptr()
            .add(at & (MAX_UNITS * 4 - 1))
            .cast::<u32>()
            .write_unaligned(written)
    };
    base | flags | u64::from(entry) << ENTRY_SHIFT
}

/// 查表范围之外的字：假名和注释不允许的字符返回拒绝的压缩值（见 [`REJECT`]、[`KANA_BIT`]），其余（标点、字母、符号）是 0。先用几个区间放过最常见的情况，两张列表各在自己的函数里，只有落在它们范围里的字才付它们的 fuel。
#[inline(never)]
fn outside(cp: u32) -> u64 {
    if (0x3040..0x3200).contains(&cp) {
        return kana_mark(cp);
    }
    // 这几段里没有格式字符：拉丁字母到西里尔字母、通用标点之后到全角形式、全角形式、辅助平面的符号和 emoji。
    if (0xAE..0x600).contains(&cp)
        || (0x2070..0xFEFF).contains(&cp)
        || (0xFF00..0xFFF9).contains(&cp)
        || (0x1D17B..0xE0000).contains(&cp)
    {
        return 0;
    }
    forbidden_mark(cp)
}

#[inline(never)]
fn kana_mark(cp: u32) -> u64 {
    if is_kana(cp) {
        REJECT | KANA_BIT
    } else {
        0
    }
}

#[inline(never)]
fn forbidden_mark(cp: u32) -> u64 {
    if is_forbidden(cp) {
        REJECT
    } else {
        0
    }
}

/// 平假名和片假名（含片假名音标扩展）。不含 U+30FB「・」，中文的外文人名有时也用它。
fn is_kana(cp: u32) -> bool {
    matches!(cp, 0x3040..=0x30FA | 0x30FC..=0x30FF | 0x31F0..=0x31FF)
}

/// 宿主不接受的注释字符里的非 ASCII 部分：C1 控制字符、格式字符（Cf，按 Unicode 17）和 U+2028、U+2029。候选里含这些字符时整行不注，不去改写它们。
fn is_forbidden(cp: u32) -> bool {
    matches!(
        cp,
        0x80..=0x9F
            | 0xAD
            | 0x600..=0x605
            | 0x61C
            | 0x6DD
            | 0x70F
            | 0x890..=0x891
            | 0x8E2
            | 0x180E
            | 0x200B..=0x200F
            | 0x2028..=0x202E
            | 0x2060..=0x2064
            | 0x2066..=0x206F
            | 0xFEFF
            | 0xFFF9..=0xFFFB
            | 0x110BD
            | 0x110CD
            | 0x13430..=0x1343F
            | 0x1BCA0..=0x1BCA3
            | 0x1D173..=0x1D17A
            | 0xE0001
            | 0xE0020..=0xE007F
    )
}

/// 多字候选里有在词组里会变的字（[`table::IN_PHRASE`]）：在 2 到 4 字的词组表上做正向最长匹配，命中的词组就地换成词组给的值（同一个字的各个值和它自己一样长，生成器保证）。返回结果和原文是否不同；预算用完时是 `None`。
#[inline(never)]
fn phrases(chars: &mut Chars, out: &mut [u8; OUT_BUFFER], work_left: &mut u32) -> Option<bool> {
    let count = chars.count;
    if !charge(work_left, count as u32 * PHRASE_CHAR_COST) {
        return None;
    }
    let mut matcher = Matcher {
        in_phrase: 0,
        wide: 0,
        changed: 0,
        work_left: *work_left,
    };
    for (k, &packed) in chars.packed[..count].iter().enumerate() {
        matcher.in_phrase |= u32::from((packed >> ENTRY_SHIFT) as u32 & table::IN_PHRASE != 0) << k;
        matcher.wide |= u32::from(packed >> SIZE_SHIFT & 7 == 4) << k;
        matcher.changed |= u32::from(packed & CHANGED != 0) << k;
    }
    let mut i = 0;
    while i < count {
        i = matcher.step(chars, out, i);
    }
    *work_left = matcher.work_left;
    if i == SPENT_POSITION {
        return None;
    }
    Some(matcher.changed != 0)
}

/// [`Matcher::step`] 预算用完时返回的位置，大于任何一行的字数，让 [`phrases`] 的循环停下。
const SPENT_POSITION: usize = usize::MAX;

/// 词组匹配的状态：各字的位图和剩下的预算。
struct Matcher {
    /// 第 k 位：第 k 个字在词组里会变（[`table::IN_PHRASE`]）。
    in_phrase: u32,
    /// 第 k 位：第 k 个字是四字节字符（词组表不收）。
    wide: u32,
    /// 第 k 位：第 k 个字写出的值和原字不同。
    changed: u32,
    work_left: u32,
}

impl Matcher {
    /// 正向最长匹配的一步：从第 `i` 个字起找词组，命中就换掉，返回下一步的位置。
    ///
    /// 词组表的每一条都含在词组里会变的字，所以从一个位置起的 4 个字里没有这样的字时，这里不可能有词组，直接跳到下一个这样的字之前 3 个字的位置；后面没有这样的字就结束。首字不带任何长度的词组标记的位置也直接跳过。被跳过的字保持逐字转换的结果，和正向最长匹配一致。
    #[inline(never)]
    fn step(&mut self, chars: &Chars, out: &mut [u8; OUT_BUFFER], i: usize) -> usize {
        let rest = self.in_phrase >> (i & (MAX_UNITS - 1));
        if rest == 0 {
            return chars.count;
        }
        let i = i.max((i + rest.trailing_zeros() as usize).saturating_sub(3)) & (MAX_UNITS - 1);
        let starts = (chars.packed[i] >> (ENTRY_SHIFT + table::START_SHIFT)) as u32 & 7;
        if starts == 0 {
            return i + 1;
        }
        match self.longest(chars, i, starts) {
            0 => i + 1,
            SPENT_MATCH => SPENT_POSITION,
            matched => {
                let length = matched >> 16;
                self.changed = replace(chars, out, i, length, matched & 0xFF, self.changed);
                i + length as usize
            }
        }
    }

    /// 从第 `i` 个字起最长的词组：返回 `长度 << 16 | 256 | 选择字节`，没有时是 0，预算用完时是 [`SPENT_MATCH`]。
    ///
    /// 能查的长度先用位运算筛出来：首字带着这个长度的标记（`starts` 的第 L-2 位），窗口不超出这一行，窗口里有在词组里会变的字（`in_phrase` 从第 i 位起的最低置位落在窗口里），窗口里没有四字节字符。剩下的长度再看过滤位图，三个长度一起算、不用循环；大部分不在表里的窗口到此为止，不做二分查找。
    #[inline(never)]
    fn longest(&mut self, chars: &Chars, i: usize, starts: u32) -> u32 {
        if !charge(&mut self.work_left, PHRASE_STEP_COST) {
            return SPENT_MATCH;
        }
        let i = i & (MAX_UNITS - 1);
        let low = ((self.in_phrase >> i).trailing_zeros() + 1).max(2);
        let high = ((chars.count - i) as u32)
            .min((self.wide >> i).trailing_zeros())
            .min(4);
        if high < low {
            return 0;
        }
        // 从这里起 4 个字的键；更短的词组把它右移，超出窗口的字随之移出。
        let packed = &chars.packed;
        let key = (packed[i] & CP_MASK) << 48
            | (packed[(i + 1) & (MAX_UNITS - 1)] & CP_MASK) << 32
            | (packed[(i + 2) & (MAX_UNITS - 1)] & CP_MASK) << 16
            | packed[(i + 3) & (MAX_UNITS - 1)] & CP_MASK;
        // 第 L-2 位表示长度 L。
        let lengths = starts
            & ((2 << (high - 2)) - 1)
            & !((1 << (low - 2)) - 1)
            & (u32::from(table::maybe(2, key >> 32))
                | u32::from(table::maybe(3, key >> 16)) << 1
                | u32::from(table::maybe(4, key)) << 2);
        if lengths == 0 {
            return 0;
        }
        self.lookup(key, lengths)
    }

    /// 按长度从长到短查词组表，返回值同 [`Matcher::longest`]。
    #[inline(never)]
    fn lookup(&mut self, key: u64, mut lengths: u32) -> u32 {
        while lengths != 0 {
            let bit = 31 - lengths.leading_zeros();
            lengths &= !(1 << bit);
            if !charge(&mut self.work_left, LOOKUP_COST) {
                return SPENT_MATCH;
            }
            let length = bit as usize + 2;
            let found = table::phrase(length, key >> (16 * (4 - length)));
            if found != 0 {
                return (length as u32) << 16 | found;
            }
        }
        0
    }
}

/// [`Matcher::longest`] 预算用完时的返回值，和任何命中的结果都不同（命中时第 8 位是 1、长度不超过 4）。
const SPENT_MATCH: u32 = u32::MAX;

/// 把第 `i` 个字起 `length` 个字换成词组给的值：第 j 个字取第 `(select >> 2j) & 3` 个值，这个数等于值的个数时保留原字。返回更新后的「和原字不同」位图。
#[inline(never)]
fn replace(
    chars: &Chars,
    out: &mut [u8; OUT_BUFFER],
    i: usize,
    length: u32,
    mut select: u32,
    mut changed: u32,
) -> u32 {
    for k in i..i + length as usize {
        let packed = chars.packed[k & (MAX_UNITS - 1)];
        let entry = (packed >> ENTRY_SHIFT) as u32;
        // 不是键的字在词组里只会取它自己（选择为 0），保持原样。
        if table::has_key(entry) {
            let index = select & 3;
            let keep = index >= table::count(entry);
            let value = if keep {
                // 词组里的字都在基本平面，单字表的键不小于 U+3400，所以原字是三字节 UTF-8。
                utf8_3(packed as u32 & 0xFFFF)
            } else {
                table::value(entry, index).0
            };
            let at = (packed >> AT_SHIFT & 0xFF) as usize & (MAX_UNITS * 4 - 1);
            let len = (packed >> OUT_SHIFT & 3) as usize + 1;
            // SAFETY: at 是这个字在注释里的起点，不超过 (MAX_UNITS - 1) * 4，缓冲多留了 4 个字节。同一个字的各个值和它自己一样长；值只有三个字节时，第四个字节属于下一个字，写之前存下、写完放回。
            unsafe {
                let target = out.as_mut_ptr().add(at);
                let following = target.add(3).read();
                target.cast::<u32>().write_unaligned(value);
                if len == 3 {
                    target.add(3).write(following);
                }
            }
            let bit = 1 << (k & (MAX_UNITS - 1));
            changed = if !keep && table::changes(entry, index) {
                changed | bit
            } else {
                changed & !bit
            };
        }
        select >>= 2;
    }
    changed
}

/// 基本平面 U+0800 以上的码位编成三字节 UTF-8，放在 u32 的低三个字节（小端，和读进来的文本一样）。
fn utf8_3(cp: u32) -> u32 {
    (0xE0 | cp >> 12) | (0x80 | cp >> 6 & 0x3F) << 8 | (0x80 | cp & 0x3F) << 16
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    core::arch::wasm32::unreachable()
}
