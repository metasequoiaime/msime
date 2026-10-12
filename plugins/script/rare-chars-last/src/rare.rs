//! 按 wasmi 的 fuel 计法写的生僻字扫描：一行候选里有没有生僻字、有没有假名。
//!
//! 判定和 `plugins/script/shared/common_chars.rs` 的 `classify` 完全相同（汉字区段里、不在常用字集里的字算生僻；平假名、片假名算假名），数据也是同一个 `plugins/script/data/common-chars.bin`。不直接用 `classify` 是因为 fuel：wasmi 在函数入口按函数体（循环以外）词法上的全部指令扣 fuel，每轮循环按循环体的全部指令扣，不管走哪个分支。`classify` 每个多字节字符都要进一次通用的解码函数，再经 `is_common` 的区段判断和二分查找，实测每个 ASCII 字节约 35、每个汉字约 200、每个辅助平面汉字约 500 fuel，20 行每行 30 个辅助平面字会耗尽 `candidates` 的 fuel。
//!
//! 这里在编译时把数据展开成两张直接查的表，扫描时每个字只查一次表：
//!
//! - [`BMP`]：U+3000..U+9FFF 每个码位 2 位，值是 [`RARE`] 或 [`KANA`]，7 KiB。三字节字符的首字节 `E3..E9` 正好是这一段。
//! - 辅助平面（U+20000..U+3FFFF）每 256 个码位一组：[`SIP_SLOT`] 记这一组有没有常用字、有的话位图在 [`SIP_BITS`] 的第几项。常用字集里辅助平面只有 203 字，分在 61 组里，两张表合计约 2.5 KiB。
//!
//! 实测（rustc 1.97.1、wasmi 2.0）连续的 ASCII 每字节约 15、连续的汉字每字约 64、连续的辅助平面字每字约 81 fuel，每换一种字符（一段）另加约 200。数据文件的格式见 `plugins/script/tools/gen_common_chars.py` 的 `encode()`，头部在编译时检查，格式变了编译失败。

static DATA: &[u8] = include_bytes!("../../data/common-chars.bin");

const HEADER_BYTES: usize = 16;
const URO_START: u32 = 0x4E00;
const URO_END: u32 = 0x9FFF;
const EXT_A_START: u32 = 0x3400;
const EXT_A_END: u32 = 0x4DBF;
const URO_OFFSET: usize = HEADER_BYTES;
const EXT_A_OFFSET: usize = URO_OFFSET + ((URO_END - URO_START + 1) as usize).div_ceil(8);
const OTHERS_OFFSET: usize = EXT_A_OFFSET + ((EXT_A_END - EXT_A_START + 1) as usize).div_ceil(8);

const fn u32_at(offset: usize) -> u32 {
    u32::from_le_bytes([
        DATA[offset],
        DATA[offset + 1],
        DATA[offset + 2],
        DATA[offset + 3],
    ])
}

/// 列表里的常用字个数（基本区和扩展 A 以外的，目前都在辅助平面）。
const OTHERS: usize = u32_at(12) as usize;

/// 辅助平面汉字的范围，同 `common_chars::is_han`。
const SIP_START: u32 = 0x20000;
const SIP_END: u32 = 0x3FFFF;
const SIP_GROUPS: usize = ((SIP_END - SIP_START + 1) >> 8) as usize;

// 数据文件的头在编译时检查：魔数、版本和长度不对时编译失败。列表里的字必须升序、都在辅助平面：兼容汉字区（U+F900..U+FAFF）因此整段算生僻，[`wide`] 不查表。
const _: () = {
    assert!(DATA[0] == b'M' && DATA[1] == b'S' && DATA[2] == b'C' && DATA[3] == b'C');
    assert!(DATA[4] == 1 && DATA[5] == 0);
    assert!(DATA.len() == OTHERS_OFFSET + OTHERS * 4);
    let mut k = 0;
    while k < OTHERS {
        let cp = u32_at(OTHERS_OFFSET + k * 4);
        assert!(cp >= SIP_START && cp <= SIP_END);
        assert!(k == 0 || u32_at(OTHERS_OFFSET + (k - 1) * 4) < cp);
        k += 1;
    }
};

/// 扫描结果的标志。
pub const RARE: u32 = 1;
pub const KANA: u32 = 2;
/// 段落数用完，没有扫完这一行。
pub const INCOMPLETE: u32 = 4;

const fn data_bit(offset: usize, index: u32) -> bool {
    DATA[offset + (index >> 3) as usize] >> (index & 7) & 1 == 1
}

/// U+3000..U+9FFF 一个码位的标志，同 `common_chars` 的 `is_kana` 和 `is_rare`。
const fn bmp_flags(cp: u32) -> u8 {
    if matches!(cp, 0x3040..=0x30FA | 0x30FC..=0x30FF | 0x31F0..=0x31FF) {
        KANA as u8
    } else if cp >= URO_START && cp <= URO_END {
        if data_bit(URO_OFFSET, cp - URO_START) {
            0
        } else {
            RARE as u8
        }
    } else if cp >= EXT_A_START && cp <= EXT_A_END {
        if data_bit(EXT_A_OFFSET, cp - EXT_A_START) {
            0
        } else {
            RARE as u8
        }
    } else {
        0
    }
}

const BMP_START: u32 = 0x3000;
const BMP_CODES: usize = 0xA000 - BMP_START as usize;

static BMP: [u8; BMP_CODES / 4] = {
    let mut table = [0u8; BMP_CODES / 4];
    let mut index = 0;
    while index < BMP_CODES {
        table[index >> 2] |= bmp_flags(BMP_START + index as u32) << ((index & 3) * 2);
        index += 1;
    }
    table
};

/// 有常用字的组数。
const SIP_USED: usize = {
    let mut used = 0;
    let mut k = 0;
    while k < OTHERS {
        let group = (u32_at(OTHERS_OFFSET + k * 4) - SIP_START) >> 8;
        if k == 0 || (u32_at(OTHERS_OFFSET + (k - 1) * 4) - SIP_START) >> 8 != group {
            used += 1;
        }
        k += 1;
    }
    used
};

// 组号存成 u8，0 表示这一组没有常用字。
const _: () = assert!(SIP_USED < 256);

/// 每组在 [`SIP_BITS`] 里的位置加 1；0 是没有常用字。
static SIP_SLOT: [u8; SIP_GROUPS] = {
    let mut slots = [0u8; SIP_GROUPS];
    let mut used = 0;
    let mut k = 0;
    while k < OTHERS {
        let group = ((u32_at(OTHERS_OFFSET + k * 4) - SIP_START) >> 8) as usize;
        if slots[group] == 0 {
            used += 1;
            slots[group] = used as u8;
        }
        k += 1;
    }
    slots
};

/// 有常用字的组的位图，256 个码位 32 字节。
static SIP_BITS: [[u8; 32]; SIP_USED] = {
    let mut bits = [[0u8; 32]; SIP_USED];
    let mut k = 0;
    while k < OTHERS {
        let offset = u32_at(OTHERS_OFFSET + k * 4) - SIP_START;
        let slot = SIP_SLOT[(offset >> 8) as usize] as usize - 1;
        bits[slot][((offset & 0xFF) >> 3) as usize] |= 1 << (offset & 7);
        k += 1;
    }
    bits
};

/// 扫描一行文本（宿主给的合法 UTF-8），返回 [`RARE`]、[`KANA`] 标志。
///
/// 文本按字节种类分段，每种段落各有一个很小的不内联循环，循环体里只有这种段落要做的事：
///
/// - [`skip`]：首字节小于 `E3` 的字节逐个跳过。汉字和假名的 UTF-8 首字节都不小于 `E3`（U+3000 起），而 ASCII、续字节（`80..BF`）和两字节字符的首字节都小于它。
/// - [`run3`]：首字节 `E3..E9` 的三字节字符，正好是 U+3000..U+9FFF，查 [`BMP`]。
/// - [`run4`]：第二、第三平面的四字节字符，查 [`SIP_SLOT`] 和 [`SIP_BITS`]。
/// - 其余字符（U+A000 以上的三字节字符、其他平面）一个一个交给 [`other`]。
///
/// 段落内每个汉字约 64 fuel，但每进入一次 [`run3`]、[`run4`] 或 [`other`] 要另加约 200：汉字和字母逐个交替的「中a中a」每对约 260，每字节六十多。所以 `steps` 限制这样的进入次数（一行纯汉字是一步，「T恤」也是一步，「中a中a」两步），用完时返回带 [`INCOMPLETE`] 的标志，调用方把这一行当作没读完。
#[inline(never)]
pub fn scan(text: &[u8], steps: &mut u32) -> u32 {
    let n = text.len();
    let mut flags = 0;
    let mut i = 0;
    loop {
        i = skip(text, i);
        if i >= n {
            return flags;
        }
        if *steps == 0 {
            return flags | INCOMPLETE;
        }
        *steps -= 1;
        let mut packed = match text[i] {
            0xE3..=0xE9 => run3(text, i),
            0xF0 => run4(text, i),
            _ => pack(i, 0),
        };
        // 段落函数没有前进：这个字符不归它们管，交给 `other`，它总是前进。
        if packed as u32 as usize == i {
            packed = other(text, i);
        }
        i = packed as u32 as usize;
        flags |= (packed >> 32) as u32;
    }
}

/// 打包段落函数的结果：低 32 位是段落之后的位置，高 32 位是标志。
fn pack(next: usize, flags: u32) -> u64 {
    u64::from(flags) << 32 | next as u64
}

/// 从 `i` 起第一个不小于 `E3` 的字节；没有时是 `text.len()`。逐字节执行的循环单独成函数，循环体只有一次比较：写在 [`scan`] 里时，LLVM 会把别的分支也排进这个循环，每个 ASCII 字节都要为它们付 fuel。
#[inline(never)]
fn skip(text: &[u8], mut i: usize) -> usize {
    while i < text.len() && text[i] < 0xE3 {
        i += 1;
    }
    i
}

/// 从 `i` 起连续的、首字节 `E3..E9` 的三字节字符（U+3000..U+9FFF）。
#[inline(never)]
fn run3(text: &[u8], mut i: usize) -> u64 {
    let mut flags = 0;
    while i + 2 < text.len() {
        let lead = text[i].wrapping_sub(0xE3);
        if lead >= 7 {
            break;
        }
        let index = usize::from(lead) << 12
            | usize::from(text[i + 1] & 0x3F) << 6
            | usize::from(text[i + 2] & 0x3F);
        flags |= u32::from(BMP[index >> 2] >> ((index & 3) << 1) & 3);
        i += 3;
    }
    pack(i, flags)
}

/// 从 `i` 起连续的第二、第三平面字符（U+20000..U+3FFFF，`F0 A0..BF xx xx`）。第一个字符就不是（第一平面的 emoji 等）时不前进。
#[inline(never)]
fn run4(text: &[u8], mut i: usize) -> u64 {
    let mut flags = 0;
    while i + 3 < text.len() && text[i] == 0xF0 && text[i + 1] >= 0xA0 {
        let offset = u32::from(text[i + 1] & 0x1F) << 12
            | u32::from(text[i + 2] & 0x3F) << 6
            | u32::from(text[i + 3] & 0x3F);
        let slot = SIP_SLOT[(offset >> 8) as usize];
        let common = slot != 0
            && SIP_BITS[usize::from(slot) - 1][((offset & 0xFF) >> 3) as usize] >> (offset & 7) & 1
                == 1;
        if !common {
            flags = RARE;
        }
        i += 4;
    }
    pack(i, flags)
}

/// `text[i]` 开头的一个其余字符：U+A000 以上的三字节字符，或第一平面、第四平面以上的四字节字符（以及文本末尾不完整的字符，宿主给的是合法 UTF-8，不会出现）。只有兼容汉字 U+F900..U+FAFF（`EF A4 80..EF AB BF`）算生僻：常用字集里没有它们（见上面的编译时检查）。
#[inline(never)]
fn other(text: &[u8], i: usize) -> u64 {
    let lead = text[i];
    if lead >= 0xF0 {
        return pack(i + 4, 0);
    }
    let second = text.get(i + 1).copied().unwrap_or(0);
    let compat = lead == 0xEF && (0xA4..=0xAB).contains(&second);
    pack(i + 3, if compat { RARE } else { 0 })
}
