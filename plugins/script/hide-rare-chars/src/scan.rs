//! 按字节扫描一行候选，判断它含不含生僻字、假名。判定规则和数据都来自共用的 `common_chars`（`is_han`、`is_common`、`is_kana`），这里只负责用尽量少的 fuel 找到需要判定的字，并给工作量设上限。
//!
//! 为什么不直接用 `common_chars::classify`：wasmi 在函数入口按函数体里（循环以外）词法上的全部指令扣 fuel，不管走哪个分支；`classify` 对每个非 ASCII 字符调用一个把解码、假名、汉字区段和查表全部内联在一起的函数，实测每个常用汉字约 290 fuel、每个 ASCII 字节约 36 fuel。这里的做法：
//! - 只有首字节 ≥ `E3` 的字符（U+3000 及以上的三字节字符和四字节字符，下面叫「宽字符」）才可能是汉字或假名。逐字节的循环只找这样的首字节，循环体只有一次比较，每个字节约 17 fuel。
//! - 找到之后三字节、四字节字符分别交给很小的函数解码、判定，再整个字符跳过。一个常用汉字约 250 fuel，一个辅助平面的常用字要在两百来项的列表上二分查找，约 560 fuel。
//! - 一行确定是生僻之后不再查表，只看剩下的部分有没有假名。
//! - 每个宽字符扣一个单位的 [`Scanner::budget`]，辅助平面的汉字另扣 [`SIP_EXTRA`]。不够扣时这一行和后面的行都不再判定（[`STOP`]）。

use crate::common_chars;

/// 没有生僻字，也没有假名。
pub const PLAIN: u8 = 0;
/// 至少一个生僻字，没有假名。
pub const RARE: u8 = 1;
/// 含假名。
pub const KANA: u8 = 2;
/// 工作量用完，这一行没判定完。
pub const STOP: u8 = 3;

/// 辅助平面的汉字比基本区的贵一倍多，除了每个宽字符的一个单位，再扣这么多。
pub const SIP_EXTRA: u32 = 2;

pub struct Scanner {
    /// 还能细看几个宽字符（单位见模块说明）。
    pub budget: u32,
}

impl Scanner {
    /// 扫描一行，返回 [`PLAIN`]、[`RARE`]、[`KANA`] 或 [`STOP`]。`text` 是宿主给的合法 UTF-8。
    #[inline(never)]
    pub fn row(&mut self, text: &[u8]) -> u8 {
        let mut i = 0;
        loop {
            i = next_wide(text, i);
            if i >= text.len() {
                return PLAIN;
            }
            if self.budget == 0 {
                return STOP;
            }
            self.budget -= 1;
            // 首字节 E3..EF 是三字节字符，F0..F4 是四字节字符。
            let (kind, len) = if text[i] < 0xF0 {
                (bmp(text, i), 3)
            } else {
                (self.sip(text, i), 4)
            };
            if kind == RARE {
                return if has_kana(text, i + len) { KANA } else { RARE };
            }
            if kind != PLAIN {
                return kind;
            }
            i += len;
        }
    }

    /// 四字节字符：只有 U+20000..U+3FFFF 是汉字区段，查表另扣 [`SIP_EXTRA`]。
    #[inline(never)]
    fn sip(&mut self, text: &[u8], i: usize) -> u8 {
        let cp = decode4(text, i);
        if !common_chars::is_han(cp) {
            return PLAIN;
        }
        if self.budget < SIP_EXTRA {
            return STOP;
        }
        self.budget -= SIP_EXTRA;
        rare(cp)
    }
}

/// U+3000..U+FFFF 的字符：假名，或者要查表的汉字，其余（标点、谚文、全角字母等）都不算。
#[inline(never)]
fn bmp(text: &[u8], i: usize) -> u8 {
    let cp = decode3(text, i);
    if common_chars::is_han(cp) {
        return rare(cp);
    }
    if common_chars::is_kana(cp) {
        KANA
    } else {
        PLAIN
    }
}

/// 已知是汉字区段的字：在不在常用字集里。
#[inline(always)]
fn rare(cp: u32) -> u8 {
    if common_chars::is_common(cp) {
        PLAIN
    } else {
        RARE
    }
}

/// 从 `i` 起第一个 ≥ `E3` 的字节；没有时是 `text.len()`。ASCII、两字节字符、U+0800..U+2FFF 的三字节字符和所有续字节都 < `E3`，都在这里跳过。逐字节执行的循环单独成函数，循环体只有一次比较。
#[inline(never)]
fn next_wide(text: &[u8], mut i: usize) -> usize {
    while i < text.len() && text[i] < 0xE3 {
        i += 1;
    }
    i
}

/// 从 `i` 起还有没有假名。假名都是 `E3` 开头的三字节字符，循环只找 `E3`，不扣工作量：调用方读进来的字节数本身有上限。
#[inline(never)]
pub fn has_kana(text: &[u8], mut i: usize) -> bool {
    loop {
        while i < text.len() && text[i] != 0xE3 {
            i += 1;
        }
        if i >= text.len() {
            return false;
        }
        if common_chars::is_kana(decode3(text, i)) {
            return true;
        }
        i += 3;
    }
}

fn byte(text: &[u8], i: usize) -> u32 {
    u32::from(text.get(i).copied().unwrap_or(0))
}

/// 解码 `text[i]` 开头的三字节字符。越界的续字节按 0 处理，不会 panic。
fn decode3(text: &[u8], i: usize) -> u32 {
    (byte(text, i) & 0x0F) << 12 | (byte(text, i + 1) & 0x3F) << 6 | (byte(text, i + 2) & 0x3F)
}

/// 解码 `text[i]` 开头的四字节字符。
fn decode4(text: &[u8], i: usize) -> u32 {
    (byte(text, i) & 0x07) << 18
        | (byte(text, i + 1) & 0x3F) << 12
        | (byte(text, i + 2) & 0x3F) << 6
        | (byte(text, i + 3) & 0x3F)
}
