//! 生僻字插件（rare-chars-last、hide-rare-chars）共用的常用字集：查找、汉字区段判定和逐字节扫描一行候选（`classify`，参照实现）。
//!
//! 插件用 `#[path = "../../shared/common_chars.rs"] mod common_chars;` 引入。数据是 `plugins/script/data/common-chars.bin`，由 `plugins/script/tools/gen_common_chars.py` 生成（格式见那里的 `encode()`），编译时用 `include_bytes!` 放进模块的只读数据，约 4 KiB。只用 `core`。
//!
//! 「生僻」的定义：码位落在 [`is_han`] 的汉字区段里，并且不在常用字集里。常用字集是《通用规范汉字表》8105 字并上它们的繁体形，见生成器的说明。非汉字（字母、数字、标点、假名、emoji）永远不算生僻。

#![allow(dead_code)]

static DATA: &[u8] = include_bytes!("../data/common-chars.bin");

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

/// 列表里的常用字个数（辅助平面汉字等）。
const OTHERS: usize = u32_at(12) as usize;

// 数据文件的头在编译时检查：魔数、版本和长度不对时编译失败，不会带着坏数据运行。
const _: () = {
    assert!(DATA[0] == b'M' && DATA[1] == b'S' && DATA[2] == b'C' && DATA[3] == b'C');
    assert!(DATA[4] == 1 && DATA[5] == 0);
    assert!(DATA.len() == OTHERS_OFFSET + OTHERS * 4);
};

/// 汉字区段：扩展 A、基本区、兼容汉字和第二、第三平面（扩展 B 以后）。部首、笔画、注音和带圈字不算。
pub fn is_han(cp: u32) -> bool {
    matches!(cp, 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF | 0x20000..=0x3FFFF)
}

/// 平假名和片假名（含片假名音标扩展）。不含 U+30FB「・」，中文的外文人名有时也用它。
pub fn is_kana(cp: u32) -> bool {
    matches!(cp, 0x3040..=0x30FA | 0x30FC..=0x30FF | 0x31F0..=0x31FF)
}

fn bit(offset: usize, index: u32) -> bool {
    DATA[offset + (index >> 3) as usize] >> (index & 7) & 1 == 1
}

/// `cp` 在不在常用字集里。
pub fn is_common(cp: u32) -> bool {
    if (URO_START..=URO_END).contains(&cp) {
        return bit(URO_OFFSET, cp - URO_START);
    }
    if (EXT_A_START..=EXT_A_END).contains(&cp) {
        return bit(EXT_A_OFFSET, cp - EXT_A_START);
    }
    // 升序列表上的二分查找，两百来项，最多 8 次比较。
    let (mut low, mut high) = (0, OTHERS);
    while low < high {
        let middle = (low + high) / 2;
        let value = u32_at(OTHERS_OFFSET + middle * 4);
        if value == cp {
            return true;
        }
        if value < cp {
            low = middle + 1;
        } else {
            high = middle;
        }
    }
    false
}

/// 汉字区段里、不在常用字集里的字。
pub fn is_rare(cp: u32) -> bool {
    is_han(cp) && !is_common(cp)
}

/// 一行候选文本的判定结果。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Row {
    /// 没有生僻字，也没有假名。
    Plain,
    /// 至少一个生僻字，没有假名。
    Rare,
    /// 含假名：日文候选，常用字集不覆盖日文新字体（気、込），插件应整窗跳过。
    Kana,
}

const RARE: u32 = 1;
const KANA: u32 = 2;

/// 逐字节扫描一行候选（宿主给的合法 UTF-8，可以是 `candidates::bytes` 读出的未校验字节）。
///
/// ASCII 字节在循环里直接跳过；多字节字符交给不内联的 [`scalar_flags`]，让循环体保持很小：wasmi 在每轮循环里按循环体词法上的全部指令扣 fuel，慢路径内联进来会让每个 ASCII 字节都付它的钱。
///
/// 这是判定规则的参照实现，不适合在候选窗口里逐行调用：实测每个常用汉字约 200–290 fuel、每个 ASCII 字节约 35，扫满 20 行会超过 `candidates` 的单次上限。hide-rare-chars（`src/scan.rs`）和 rare-chars-last（`src/rare.rs`）各有按 fuel 写的扫描，判定和这里逐字相同。
pub fn classify(text: &[u8]) -> Row {
    let mut flags = 0;
    let mut i = 0;
    while i < text.len() {
        if text[i] < 0x80 {
            i += 1;
            continue;
        }
        let packed = scalar_flags(text, i);
        i += (packed & 7) as usize;
        flags |= packed >> 8;
    }
    if flags & KANA != 0 {
        Row::Kana
    } else if flags & RARE != 0 {
        Row::Rare
    } else {
        Row::Plain
    }
}

/// `text[i]` 开头的多字节字符：低 3 位是字节数，第 8 位起是 `RARE`、`KANA` 标志。
#[inline(never)]
fn scalar_flags(text: &[u8], i: usize) -> u32 {
    let (cp, len) = decode(text, i);
    let flags = if is_kana(cp) {
        KANA
    } else if is_rare(cp) {
        RARE
    } else {
        0
    };
    len as u32 | flags << 8
}

/// 解码 `text[i]` 开头的一个 UTF-8 字符，返回码位和字节数。不校验：输入来自宿主，总是合法的 UTF-8；越界的续字节按 0 处理，不会 panic。
pub fn decode(text: &[u8], i: usize) -> (u32, usize) {
    let at = |k: usize| u32::from(text.get(i + k).copied().unwrap_or(0) & 0x3F);
    let b0 = u32::from(text[i]);
    if b0 < 0x80 {
        (b0, 1)
    } else if b0 < 0xE0 {
        ((b0 & 0x1F) << 6 | at(1), 2)
    } else if b0 < 0xF0 {
        ((b0 & 0x0F) << 12 | at(1) << 6 | at(2), 3)
    } else {
        ((b0 & 0x07) << 18 | at(1) << 12 | at(2) << 6 | at(3), 4)
    }
}
