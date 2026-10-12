//! unicode-codepoint：给候选注上 Unicode 码位。单个字符的候选注 `U+4E2D 基本区` 这样的码位，落在汉字相关区段或私用区时加中文区段名；emoji 序列和异体字序列（2 到 4 个码位）逐个注出码位，例如 `U+2764 U+FE0F`；别的多字候选不注。
//!
//! 只声明 `candidates`，申请 `read_candidates` 和 `annotate_candidates`。注释另放一格显示，不改引擎的注释；不改顺序，也不隐藏。
//!
//! 工作量是固定的：窗口最多 20 行，每行最多读 16 字节（4 个码位最多 16 字节，更长的读不进来，宿主返回 `LIMIT`，直接跳过），只有文本是 AI 占位字符的行才多查一次来源。fuel 的计法决定了写法：wasmi 在函数入口按函数体（循环以外）词法上的全部指令一次扣完，每轮循环按循环体的全部指令扣，`local.get` 也算一条。所以不注释的行在 [`row`] 里尽早返回；单个码位和序列各走一个小函数；注释直接写进日志缓冲，交给宿主的就是日志里的那一段，不另拼一份再复制。最坏情况的 fuel 见 README。

#![no_std]

#[path = "../../../../docs/script-plugin-template/sdk/msime_script.rs"]
mod msime_script;

use core::ptr::addr_of_mut;
use msime_script::{annotate, candidates, hook, log, Error};

/// 窗口最多 20 行（宿主的上限），多给的行号不看。
const MAX_ROWS: u64 = 20;
/// 最多注几个码位。
const MAX_SCALARS: usize = 4;
/// 一行文本最多读这么多字节：4 个码位每个最多 4 字节。读不下的行一定多于 4 个码位。
const MAX_TEXT: usize = 4 * MAX_SCALARS;
/// 注释的上限：32 个 UTF-16 码元。
const MAX_UNITS: usize = 32;
/// 一条注释最多多少字节。单个码位最长是 `U+10FFFF 补充私用区B`，27 字节；序列只有 ASCII，最长 4 个 `U+XXXXXX` 加 3 个空格，35 字节（实际放得进 32 个码元的最多 32 字节，见 [`sequence`]）。
const MAX_NOTE: usize = 35;
/// `candidates::SOURCE` 的 AI 占位行，不注释。
const AI_PENDING: u64 = 12;
const ZWJ: u32 = 0x200D;

/// 日志行：`ann` 加每个注释过的行 `; 12=<注释>`。20 行写满是 3 + 20 × (2 + 2 + 1 + 35) = 803 字节，再留 8 字节给 [`hex`] 一次切出的那一段。
const LOG_CAP: usize = 3 + MAX_ROWS as usize * (2 + 2 + 1 + MAX_NOTE) + 8;
static mut LOG: [u8; LOG_CAP] = [0; LOG_CAP];

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
    // 没有授权、没有窗口，或者本地模式不给读文本（网址、命令等）时什么都不做。
    let rows = match candidates::count() {
        Ok(rows) => rows.min(MAX_ROWS) as u32,
        Err(error) => return log::debug(reason(error)),
    };
    // SAFETY: 插件单线程运行，日志缓冲只在这一次调用里用。
    let mut line = Line {
        buffer: unsafe { &mut *addr_of_mut!(LOG) },
        len: 3,
    };
    line.buffer[..3].copy_from_slice(b"ann");
    for index in 0..rows {
        if let Err(error) = row(index, &mut line) {
            return log::debug(reason(error));
        }
    }
    if line.len == 3 {
        return log::debug("none");
    }
    // SAFETY: 日志缓冲里只写了 ASCII 和整段的区段名（合法 UTF-8）。
    log::debug(unsafe { core::str::from_utf8_unchecked(&line.buffer[..line.len]) });
}

fn reason(error: Error) -> &'static str {
    match error {
        Error::Denied => "skip: denied",
        Error::Absent => "skip: absent",
        _ => "skip: unreadable",
    }
}

/// 处理窗口第 `index` 行：该注释就注释，并记进日志。只有没有授权（`Denied`）或窗口不在了（`Absent`）时返回错误，整次放弃。不注释的行在这里就返回，只付读文本和解码的 fuel。
#[inline(never)]
fn row(index: u32, line: &mut Line) -> Result<(), Error> {
    // 文本读进前 16 字节；后面的字节留作 0，解码最后一个字符时可以放心往后多读 3 个字节。
    let mut buffer = [0u8; TEXT_BUFFER];
    let len = match candidates::bytes(index, &mut buffer[..MAX_TEXT]) {
        Ok(text) => text.len(),
        // 超过 16 字节，一定多于 4 个码位，不注释。
        Err(Error::Limit) => return Ok(()),
        Err(error) => return Err(error),
    };
    let mut scalars = [0u32; MAX_SCALARS];
    let count = decode(&buffer, len, &mut scalars);
    if count == 1 {
        // AI 占位行的文本是「…」（等回答）或「—」（没有回答）。只有文本是这两个字符时才多查一次来源，查不到时照常注释。
        if is_placeholder(scalars[0])
            && candidates::meta(index, candidates::SOURCE) == Ok(AI_PENDING)
        {
            return Ok(());
        }
        return single(index, scalars[0], line);
    }
    if count == 0 || !is_sequence(&scalars[..count]) {
        return Ok(());
    }
    sequence(index, &scalars[..count], line)
}

/// 读文本的缓冲：[`MAX_TEXT`] 字节的文本，后面再留 4 个字节的 0。
const TEXT_BUFFER: usize = MAX_TEXT + 4;

/// 把 `buffer[..len]`（宿主给的合法 UTF-8）解码成码位写进 `out`，返回个数。空串或多于 [`MAX_SCALARS`] 个码位时返回 0。
///
/// 不分支：每个字符都取首字节和后面 3 个字节拼成 21 位，再按字节数右移，属于下一个字符的低位被移掉。按首字节高 4 位查字节数和有效位。循环体因此很短，每个字符的 fuel 和字节数无关。
#[inline(never)]
fn decode(buffer: &[u8; TEXT_BUFFER], len: usize, out: &mut [u32; MAX_SCALARS]) -> usize {
    let mut count = 0;
    let mut i = 0;
    while i < len {
        if count == MAX_SCALARS {
            return 0;
        }
        // `i < len <= 16`，`& 15` 只是让编译器看出后面四次读都不越界。
        let at = i & 15;
        let lead = usize::from(buffer[at] >> 4);
        let bits = u32::from(buffer[at] & LEAD_MASK[lead]) << 18
            | u32::from(buffer[at + 1] & 0x3F) << 12
            | u32::from(buffer[at + 2] & 0x3F) << 6
            | u32::from(buffer[at + 3] & 0x3F);
        out[count & 3] = bits >> LEAD_SHIFT[lead];
        count += 1;
        i += usize::from(LEAD_LEN[lead]);
    }
    count
}

/// 按 UTF-8 首字节的高 4 位：字符的字节数、首字节的有效位、拼好的 21 位要右移多少。8..B 是续字节，不会出现在首字节的位置。
static LEAD_LEN: [u8; 16] = [1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 1, 2, 2, 3, 4];
static LEAD_MASK: [u8; 16] = [
    0x7F, 0x7F, 0x7F, 0x7F, 0x7F, 0x7F, 0x7F, 0x7F, 0x7F, 0x7F, 0x7F, 0x7F, 0x1F, 0x1F, 0x0F, 0x07,
];
static LEAD_SHIFT: [u32; 16] = [18, 18, 18, 18, 18, 18, 18, 18, 18, 18, 18, 18, 12, 12, 6, 0];

/// AI 占位行用的两个字符：U+2026「…」和 U+2014「—」。
fn is_placeholder(scalar: u32) -> bool {
    scalar == 0x2026 || scalar == 0x2014
}

/// 2 到 4 个码位的候选算不算一个「序列」：除第一个以外，每个码位都是附着在前面的成分（变体选择符、零宽连接符、肤色修饰符、键帽符号、标签字符），或者紧跟在零宽连接符后面；两个地区指示符组成的旗帜也算。`中国`、`ab`、`a中` 这样的普通多字候选不算，不注释。
#[inline(never)]
fn is_sequence(scalars: &[u32]) -> bool {
    if scalars.len() == 2 && is_regional(scalars[0]) && is_regional(scalars[1]) {
        return true;
    }
    let mut i = 1;
    while i < scalars.len() {
        if !is_component(scalars[i]) && scalars[i - 1] != ZWJ {
            return false;
        }
        i += 1;
    }
    true
}

/// 附着在前一个字符上的成分：变体选择符（含异体字选择符 U+E0100..U+E01EF）、零宽连接符、键帽符号 U+20E3、肤色修饰符、标签字符。
fn is_component(scalar: u32) -> bool {
    matches!(
        scalar,
        0xFE00..=0xFE0F | ZWJ | 0x20E3 | 0x1F3FB..=0x1F3FF | 0xE0020..=0xE007F | 0xE0100..=0xE01EF
    )
}

/// 地区指示符 U+1F1E6..U+1F1FF，两个组成一面旗帜。
fn is_regional(scalar: u32) -> bool {
    matches!(scalar, 0x1F1E6..=0x1F1FF)
}

/// 单个码位：`U+XXXX`，落在 [`BLOCKS`] 的区段里再加空格和区段名。最长 15 个 UTF-16 码元，不用核对上限。
#[inline(never)]
fn single(index: u32, scalar: u32, line: &mut Line) -> Result<(), Error> {
    let start = line.begin(index);
    let mut at = hex(line.buffer, start, scalar);
    if let Some(block) = block_of(scalar) {
        let name = block.name.as_bytes();
        line.buffer[at] = b' ';
        line.buffer[at + 1..at + 1 + name.len()].copy_from_slice(name);
        at += 1 + name.len();
    }
    line.finish(index, start, at)
}

/// 序列：各码位的 `U+XXXX` 用空格隔开。只有 ASCII，字节数就是 UTF-16 码元数。六位的码位（U+100000 以上）只能出现在开头或零宽连接符后面，其余位置最多七个字符，按 [`is_sequence`] 的规则最长正好 32 个码元；这里仍然核对一次，超出就不注释。
#[inline(never)]
fn sequence(index: u32, scalars: &[u32], line: &mut Line) -> Result<(), Error> {
    let start = line.begin(index);
    let mut at = start;
    for (i, scalar) in scalars.iter().enumerate() {
        if i > 0 {
            line.buffer[at] = b' ';
            at += 1;
        }
        at = hex(line.buffer, at, *scalar);
    }
    if at - start > MAX_UNITS {
        return Ok(());
    }
    line.finish(index, start, at)
}

/// 在 `buffer[at..]` 写 `U+` 加大写十六进制，至少 4 位：U+0061、U+4E2D、U+20000、U+10FFFF。返回写完的位置。
#[inline(never)]
fn hex(buffer: &mut [u8; LOG_CAP], at: usize, scalar: u32) -> usize {
    // 先切出固定 8 字节的一段，后面按常量下标写，不再逐字节查边界。
    let out: &mut [u8; 8] = (&mut buffer[at..at + 8]).try_into().unwrap();
    let digits = match scalar {
        0..=0xFFFF => 4,
        0x10000..=0xFFFFF => 5,
        _ => 6,
    };
    out[0] = b'U';
    out[1] = b'+';
    // 从最低位往前写。
    let mut k = digits + 1;
    let mut rest = scalar;
    loop {
        out[k & 7] = HEX[(rest & 0xF) as usize];
        if k == 2 {
            return at + 2 + digits;
        }
        rest >>= 4;
        k -= 1;
    }
}

static HEX: [u8; 16] = *b"0123456789ABCDEF";

/// 日志行，也是拼注释的地方：每条注释写在日志里它自己那一条的后半段，交给宿主的就是这一段。宿主收下之后才把它算进 `len`，没收下的就被下一条覆盖。
struct Line {
    buffer: &'static mut [u8; LOG_CAP],
    len: usize,
}

impl Line {
    /// 在 `len` 之后写这一条的开头（分隔、行号、等号），返回注释开始的位置。行号不超过 19，最多两位。
    #[inline(never)]
    fn begin(&mut self, index: u32) -> usize {
        let mut at = self.len;
        if at > 3 {
            self.buffer[at] = b';';
            at += 1;
        }
        self.buffer[at] = b' ';
        at += 1;
        if index >= 10 {
            self.buffer[at] = b'0' + (index / 10 % 10) as u8;
            at += 1;
        }
        self.buffer[at] = b'0' + (index % 10) as u8;
        self.buffer[at + 1] = b'=';
        at + 2
    }

    /// 把 `buffer[start..end]` 作为第 `index` 行的注释交给宿主，收下了就留在日志里。
    #[inline(never)]
    fn finish(&mut self, index: u32, start: usize, end: usize) -> Result<(), Error> {
        // SAFETY: 注释只由 ASCII 和整段的区段名（合法 UTF-8）拼成。
        let note = unsafe { core::str::from_utf8_unchecked(&self.buffer[start..end]) };
        match annotate::set(index, note) {
            Ok(()) => self.len = end,
            Err(Error::Denied) => return Err(Error::Denied),
            // 不应该发生：注释只含 ASCII 和汉字，长度先核对过。真发生了就在日志里记一笔，别的行照常注释。
            Err(_) => {
                self.buffer[start..start + 8].copy_from_slice(b"rejected");
                self.len = start + 8;
            }
        }
        Ok(())
    }
}

/// 带中文名的区段。
struct Block {
    first: u32,
    last: u32,
    name: &'static str,
}

/// 汉字相关的区段和私用区，范围取自 Unicode 17.0 的 Blocks.txt（逐条核对过起止），按起点排序。名字指整个区段，包括其中尚未分配的码位；其余区段只注码位，不注名字。名字都很短，单个码位的注释最长是 `U+10FFFF 补充私用区B`。
const BLOCK_TABLE: [Block; 20] = [
    Block {
        first: 0x2E80,
        last: 0x2EFF,
        name: "部首补充",
    },
    Block {
        first: 0x2F00,
        last: 0x2FDF,
        name: "康熙部首",
    },
    Block {
        first: 0x2FF0,
        last: 0x2FFF,
        name: "结构描述符",
    },
    Block {
        first: 0x31C0,
        last: 0x31EF,
        name: "笔画",
    },
    Block {
        first: 0x3400,
        last: 0x4DBF,
        name: "扩展A",
    },
    Block {
        first: 0x4E00,
        last: 0x9FFF,
        name: "基本区",
    },
    Block {
        first: 0xE000,
        last: 0xF8FF,
        name: "私用区",
    },
    Block {
        first: 0xF900,
        last: 0xFAFF,
        name: "兼容汉字",
    },
    Block {
        first: 0x20000,
        last: 0x2A6DF,
        name: "扩展B",
    },
    Block {
        first: 0x2A700,
        last: 0x2B73F,
        name: "扩展C",
    },
    Block {
        first: 0x2B740,
        last: 0x2B81F,
        name: "扩展D",
    },
    Block {
        first: 0x2B820,
        last: 0x2CEAF,
        name: "扩展E",
    },
    Block {
        first: 0x2CEB0,
        last: 0x2EBEF,
        name: "扩展F",
    },
    Block {
        first: 0x2EBF0,
        last: 0x2EE5F,
        name: "扩展I",
    },
    Block {
        first: 0x2F800,
        last: 0x2FA1F,
        name: "兼容汉字补充",
    },
    Block {
        first: 0x30000,
        last: 0x3134F,
        name: "扩展G",
    },
    Block {
        first: 0x31350,
        last: 0x323AF,
        name: "扩展H",
    },
    Block {
        first: 0x323B0,
        last: 0x3347F,
        name: "扩展J",
    },
    Block {
        first: 0xF0000,
        last: 0xFFFFF,
        name: "补充私用区A",
    },
    Block {
        first: 0x100000,
        last: 0x10FFFF,
        name: "补充私用区B",
    },
];

static BLOCKS: [Block; 20] = BLOCK_TABLE;

/// 基本区在 [`BLOCKS`] 里的下标：最常见，先单独看。
const BASIC: usize = 5;
/// [`block_of`] 按平面切表：`..PLANE_2` 是基本平面，`PLANE_2..PLANE_15` 是第二、第三平面，`PLANE_15..` 是两个补充私用区。
const PLANE_2: usize = 8;
const PLANE_15: usize = 18;

// 编译时核对表的顺序和切分点，改表时漏改下标就编不过。
const _: () = {
    let mut i = 0;
    while i < BLOCK_TABLE.len() {
        let block = &BLOCK_TABLE[i];
        assert!(block.first <= block.last);
        assert!(i == 0 || BLOCK_TABLE[i - 1].last < block.first);
        let plane = block.first >> 16;
        assert!(plane == block.last >> 16);
        if i < PLANE_2 {
            assert!(plane == 0);
        } else if i < PLANE_15 {
            assert!(plane == 2 || plane == 3);
        } else {
            assert!(plane == 15 || plane == 16);
        }
        i += 1;
    }
    assert!(BLOCK_TABLE[0].first == 0x2E80);
    assert!(BLOCK_TABLE[BASIC].first == 0x4E00 && BLOCK_TABLE[BASIC].last == 0x9FFF);
};

/// `scalar` 落在哪个带名字的区段里。基本区先单独看；其余按平面缩小范围，ASCII、标点、假名、第一平面（emoji 等）不用查表。
#[inline(never)]
fn block_of(scalar: u32) -> Option<&'static Block> {
    if (0x4E00..=0x9FFF).contains(&scalar) {
        return Some(&BLOCKS[BASIC]);
    }
    let candidates = match scalar >> 16 {
        0 if scalar >= BLOCKS[0].first => &BLOCKS[..PLANE_2],
        2 | 3 => &BLOCKS[PLANE_2..PLANE_15],
        15 | 16 => &BLOCKS[PLANE_15..],
        _ => return None,
    };
    for block in candidates {
        if scalar < block.first {
            return None;
        }
        if scalar <= block.last {
            return Some(block);
        }
    }
    None
}

#[panic_handler]
fn panic(_: &core::panic::PanicInfo) -> ! {
    core::arch::wasm32::unreachable()
}
