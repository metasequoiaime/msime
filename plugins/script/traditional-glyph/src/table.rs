//! 简繁对照表 `data/s2t.bin` 的读取：单字条目、值池和 2 到 4 字的词组表。
//!
//! 数据由 `plugins/script/tools/gen_s2t_table.py` 从仓库里 vendored 的 OpenCC 生成，格式见那里的 `encode()`，编译时用 `include_bytes!` 放进模块的只读数据。头在编译时检查，魔数、版本或长度不对时编译失败。
//!
//! 查一个字的条目是常数时间：码位按 64 个一块（基本平面和第二、第三平面），目录给出块号，块的位图加上置位计数（`popcnt` 是一条 wasm 指令）给出条目下标。
//!
//! 热路径上的读取不做边界检查（[`rd16`]、[`rd32`]、[`rd64`]）：wasmi 按指令计 fuel，每次检查连同分支要多付好几条。各段的起点和长度来自编译时检查过的头，段内的下标（块号、条目下标、值池偏移）由生成器保证在范围内，`--check` 保证提交的文件就是生成器的输出。

static DATA: &[u8] = include_bytes!("../data/s2t.bin");

const fn u16_at(offset: usize) -> u16 {
    u16::from_le_bytes([DATA[offset], DATA[offset + 1]])
}

const fn u32_at(offset: usize) -> u32 {
    u32::from_le_bytes([
        DATA[offset],
        DATA[offset + 1],
        DATA[offset + 2],
        DATA[offset + 3],
    ])
}

const HEADER_BYTES: usize = 40;
const BLOCKS: usize = u32_at(8) as usize;
const ENTRIES: usize = u32_at(12) as usize;
const DIRECTORY_SLOTS: usize = u32_at(16) as usize;
const POOL_BYTES: usize = u32_at(20) as usize;
const N2: usize = u32_at(24) as usize;
const N3: usize = u32_at(28) as usize;
const N4: usize = u32_at(32) as usize;

const DIRECTORY: usize = HEADER_BYTES;
const MASKS: usize = DIRECTORY + DIRECTORY_SLOTS * 2;
const BASES: usize = MASKS + BLOCKS * 8;
const TABLE: usize = BASES + BLOCKS * 2;
const POOL: usize = TABLE + ENTRIES * 4;
const KEYS2: usize = POOL + POOL_BYTES;
const KEYS3: usize = KEYS2 + N2 * 4;
const KEYS4: usize = KEYS3 + N3 * 8;
const SELECT2: usize = KEYS4 + N4 * 8;
const SELECT3: usize = SELECT2 + N2;
const SELECT4: usize = SELECT3 + N3;
/// 词组的过滤位图（和生成器的 BLOOM_* 一致）：2 字 2^15 位，3、4 字各 2^14 位。
const BLOOM_MULTIPLIER: u64 = 0x9E37_79B9_7F4A_7C15;
const BLOOM2: usize = SELECT4 + N4;
const BLOOM3: usize = BLOOM2 + (1 << 15) / 8;
const BLOOM4: usize = BLOOM3 + (1 << 14) / 8;
const END: usize = BLOOM4 + (1 << 14) / 8;

const _: () = {
    assert!(DATA[0] == b'M' && DATA[1] == b'S' && DATA[2] == b'T' && DATA[3] == b'G');
    assert!(u16_at(4) == 1);
    // 基本平面 1024 块，第二、第三平面 2048 块。
    assert!(DIRECTORY_SLOTS == 1024 + 2048);
    assert!(DATA.len() == END);
    // 值池后面紧跟着词组表（至少一条 4 字节的键），所以从任何一个值的起点读 4 个字节都不会越过 DATA 的末尾，复制值时可以一律复制 4 个字节。
    assert!(N2 > 0 && N3 > 0 && N4 > 0);
};

/// 读 DATA 里 `offset` 处的小端整数，不检查边界；调用方保证 `offset` 落在对应的段里（见本模块开头）。
fn rd16(offset: usize) -> u16 {
    debug_assert!(offset + 2 <= DATA.len());
    // SAFETY: 见上。wasm 是小端，允许不对齐的读取。
    unsafe { DATA.as_ptr().add(offset).cast::<u16>().read_unaligned() }
}

fn rd32(offset: usize) -> u32 {
    debug_assert!(offset + 4 <= DATA.len());
    // SAFETY: 同 rd16。
    unsafe { DATA.as_ptr().add(offset).cast::<u32>().read_unaligned() }
}

fn rd64(offset: usize) -> u64 {
    debug_assert!(offset + 8 <= DATA.len());
    // SAFETY: 同 rd16。
    unsafe { DATA.as_ptr().add(offset).cast::<u64>().read_unaligned() }
}

// 条目（u32）的位布局，和生成器开头的常量一一对应。
const POOL_MASK: u32 = (1 << 14) - 1;
const JOINED_SHIFT: u32 = 14;
const JOINED_MASK: u32 = 31;
const COUNT_SHIFT: u32 = 19;
const COUNT_MASK: u32 = 7;
/// 繁体专用字：在单字表的值里、本身不是键、也不在 GB2312 里（們、這、發）。
pub const TRAD: u32 = 1 << 22;
/// 简体专用字：单字表的键，值里不含它自己，也不是任何键的值（发、这、们）。
pub const SIMP: u32 = 1 << 23;
/// 第 24、25、26 位：是某条 2、3、4 字词组的首字。
pub const START_SHIFT: u32 = 24;
const SELF_SHIFT: u32 = 27;
const HAS_SELF: u32 = 1 << 29;
/// 这个字在某条词组里不取第一个值（取别的值，或者保留原字）。词组表的每一条都至少含一个这样的字（生成器核对过）。
pub const IN_PHRASE: u32 = 1 << 30;

/// 条目（或几个条目的按位或）里有没有单字表的键。
pub fn has_key(entry: u32) -> bool {
    count(entry) != 0
}

/// 值的个数；不是单字表的键时是 0。
pub fn count(entry: u32) -> u32 {
    entry >> COUNT_SHIFT & COUNT_MASK
}

/// 取第 `index` 个值会不会改变这个字：值就是它自己时不算改变。
pub fn changes(entry: u32, index: u32) -> bool {
    entry & HAS_SELF == 0 || entry >> SELF_SHIFT & 3 != index
}

/// 全部值用「/」连起来的 UTF-8，例如「發/髮」。
pub fn joined(entry: u32) -> &'static [u8] {
    let start = POOL + (entry & POOL_MASK) as usize;
    &DATA[start..start + (entry >> JOINED_SHIFT & JOINED_MASK) as usize]
}

/// 第一个值的前 4 个字节（值本身是 3 或 4 个字节，多出的部分由调用方丢掉）。`entry` 必须是单字表的键。
#[inline(always)]
pub fn first_value(entry: u32) -> u32 {
    rd32(POOL + (entry & POOL_MASK) as usize)
}

/// 第 `index` 个值的前 4 个字节（值本身是 3 或 4 个字节，多出的部分由调用方丢掉）和它的字节数。`entry` 必须是单字表的键，`index` 小于值的个数。
pub fn value(entry: u32, index: u32) -> (u32, usize) {
    let mut start = POOL + (entry & POOL_MASK) as usize;
    if index != 0 {
        start = skip_values(start, index);
    }
    let bytes = rd32(start);
    // 值都是三字节（首字节 E0..EF）或四字节（F0..F4）的字。
    (bytes, 3 + usize::from(bytes & 0xFF >= 0xF0))
}

/// 从值池的 `start` 起跳过 `index` 个值和它们后面的「/」。
#[inline(never)]
fn skip_values(mut start: usize, index: u32) -> usize {
    for _ in 0..index {
        start += 4 + usize::from(DATA[start] >= 0xF0);
    }
    start
}

/// 第二、第三平面的字的条目；不在表里是 0。调用方保证 `cp` 在 U+20000..U+3FFFF。
#[inline(never)]
pub fn sip_entry(cp: u32) -> u32 {
    block_entry(1024 + ((cp as usize - 0x20000) >> 6 & 2047), cp)
}

/// 基本平面的字的条目；不在表里是 0。
#[inline(always)]
pub fn bmp_entry(cp: u32) -> u32 {
    block_entry(cp as usize >> 6 & 1023, cp)
}

/// 第 `slot` 块里 `cp` 的条目：目录、块的位图和置位计数，没有循环。
#[inline(always)]
fn block_entry(slot: usize, cp: u32) -> u32 {
    let block = rd16(DIRECTORY + slot * 2) as usize;
    if block == 0 {
        return 0;
    }
    let mask = rd64(MASKS + (block - 1) * 8);
    let bit = cp & 63;
    if mask >> bit & 1 == 0 {
        return 0;
    }
    let rank =
        rd16(BASES + (block - 1) * 2) as usize + (mask & ((1 << bit) - 1)).count_ones() as usize;
    rd32(TABLE + rank * 4)
}

/// `length` 字的词组表里可能有 `key`：过滤位图里它的那一位是 1。为 0 时一定没有；为 1 时约有一成是误报，要再用 [`phrase`] 查。
#[inline(always)]
pub fn maybe(length: usize, key: u64) -> bool {
    let (bloom, bits) = match length {
        2 => (BLOOM2, 15),
        3 => (BLOOM3, 14),
        _ => (BLOOM4, 14),
    };
    let bit = (key.wrapping_mul(BLOOM_MULTIPLIER) >> (64 - bits)) as usize;
    DATA[bloom + (bit >> 3)] >> (bit & 7) & 1 == 1
}

/// 在 `length`（2 到 4）字的词组表里找 `key`（每字一个 UTF-16 码元，首字在高位）。找到时返回选择字节加 256：第 j 字取单字表第 `(sel >> 2j) & 3` 个值；找不到是 0。调用方先用 [`maybe`] 过滤，只有少数窗口走到这里。
#[inline(never)]
pub fn phrase(length: usize, key: u64) -> u32 {
    let (keys, count, select) = match length {
        2 => (KEYS2, N2, SELECT2),
        3 => (KEYS3, N3, SELECT3),
        _ => (KEYS4, N4, SELECT4),
    };
    let found = if length == 2 {
        search32(keys, count, key as u32)
    } else {
        search64(keys, count, key)
    };
    if found == usize::MAX {
        0
    } else {
        256 | u32::from(DATA[select + found])
    }
}

// 两个二分查找都是「先缩到一个位置，最后再比一次」的写法：循环体只有一次读取、一次比较和一次选择，wasmi 每轮按循环体的全部指令扣 fuel，这样比每轮比两次便宜。找不到时返回 usize::MAX。各表至少一条（编译时检查）。

#[inline(never)]
fn search32(keys: usize, count: usize, key: u32) -> usize {
    let (mut base, mut size) = (0, count);
    while size > 1 {
        let half = size / 2;
        if rd32(keys + (base + half) * 4) <= key {
            base += half;
        }
        size -= half;
    }
    if rd32(keys + base * 4) == key {
        base
    } else {
        usize::MAX
    }
}

#[inline(never)]
fn search64(keys: usize, count: usize, key: u64) -> usize {
    let (mut base, mut size) = (0, count);
    while size > 1 {
        let half = size / 2;
        if rd64(keys + (base + half) * 8) <= key {
            base += half;
        }
        size -= half;
    }
    if rd64(keys + base * 8) == key {
        base
    } else {
        usize::MAX
    }
}
