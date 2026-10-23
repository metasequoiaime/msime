//! 双拼句中与末尾（间接）辅助码的输入形状与解析（`contracts/mid_sentence_helpcode.h` 与
//! `shuangpin_query.cpp` 的 Rust 移植）。
//!
//! Windows 侧由 TSF 预判吃键、Server 写串、引擎解析三处共用同一份形状规则；本仓没有 TSF 层，
//! `InputSession::dispatch_character` 承担前两处（决定触发键记成什么字符），方案层在构建请求时按
//! 这里的规则解析。输入串里的标记字符与 Windows 一致：句中段记 `kMarker`（'`'），末尾段记
//! `kEndMarker`（'|'），用户敲的反引号或分号都在写串时换成对应标记，所以解析层不分触发键。
//!
//! 段形状：
//! - 句中段：'`' + 一个字母（大小写都算）+ 可选的第二个大写字母。
//! - 末尾段：'|' + 至多两个字母（大小写都算）；由大写触发时是段首大写加至多一个字母。
//! - 大写段：当前节键数为偶数且至少两个时，大写字母自己开一段，相当于「标记 + 这个字母」；
//!   句中大写段的第二码同样要求大写，末尾大写段任意字母都算第二码。

use super::query::segment_input;
use super::ShuangpinProfile;
use crate::types::{ShuangpinHelpcodeModes, SyllableHelpcode};

/// 句中辅助码段的开头（`FanyImeMidSentenceHelpcode::kMarker`）。
pub const MID_SENTENCE_MARKER: u8 = b'`';
/// 末尾辅助码（间接）一段的开头（`FanyImeMidSentenceHelpcode::kEndMarker`）。不是任何会进组字的
/// 按键字符，与反引号段、`;` 韵母都不会混淆。
pub const END_HELPCODE_MARKER: u8 = b'|';
/// 万象式直接辅助码四码后的终止键（`FanyImeDirectHelpcode::kSlash`）。
pub const DIRECT_HELPCODE_SLASH: u8 = b'/';

/// 哪种段由大写字母开头：句中辅助码的大写触发，或末尾辅助码（间接）的大写触发。两者互斥，都给时
/// 按句中辅助码（`FanyImeMidSentenceHelpcode::UppercaseTriggers`）。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct UppercaseTriggers {
    pub mid: bool,
    pub end: bool,
}

impl UppercaseTriggers {
    pub fn new(mid: bool, end: bool) -> Self {
        Self {
            mid,
            end: end && !mid,
        }
    }
}

/// 由触发方式开关算出大写触发归谁（`ime_session.h` 的 `mid_sentence_uppercase_trigger_active` 与
/// `end_helpcode_uppercase_trigger_active`）：句中大写要求句中总开关和它自己的勾、且万象式没接管；
/// 末尾大写要求辅助码开着、末尾直接关着、自己的勾在、且没被句中大写或万象式占去。
pub fn uppercase_triggers(
    modes: &ShuangpinHelpcodeModes,
    shuangpin_helpcode_enabled: bool,
) -> UppercaseTriggers {
    let mid = modes.mid_sentence && modes.mid_sentence_uppercase && !modes.direct_helpcode;
    let end = shuangpin_helpcode_enabled
        && !modes.direct
        && modes.trigger_uppercase
        && !modes.direct_helpcode
        && !mid;
    UppercaseTriggers::new(mid, end)
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BlockKind {
    /// 句中辅助码：约束它前面那个音节上的字。
    Mid,
    /// 末尾辅助码（间接）：按词的首末字筛整个候选。
    End,
}

fn is_ascii_upper(ch: u8) -> bool {
    ch.is_ascii_uppercase()
}

/// `text[marker]` 是反引号，返回这一段结束的位置（`BlockEnd`）。
fn block_end(text: &[u8], marker: usize) -> usize {
    let mut end = marker + 1;
    if end < text.len() && text[end].is_ascii_alphabetic() {
        end += 1;
        if end < text.len() && is_ascii_upper(text[end]) {
            end += 1;
        }
    }
    end
}

/// `text[first]` 是一段末尾段的第一个字符（'|'，或大写触发时的第一码），`codes` 是这个位置之后
/// 还能收几个辅码（`EndBlockEnd`）。
fn end_block_end(text: &[u8], first: usize, mut codes: usize) -> usize {
    let mut end = first + 1;
    while codes > 0 && end < text.len() && text[end].is_ascii_alphabetic() {
        end += 1;
        codes -= 1;
    }
    end
}

/// 当前这一节已有的键数 `chunk_length` 是偶数且至少两个时，这个位置上的大写字母开一段
/// （`StartsUppercaseBlock`）。
fn starts_uppercase_block(ch: u8, chunk_length: usize) -> bool {
    is_ascii_upper(ch) && chunk_length >= 2 && chunk_length.is_multiple_of(2)
}

/// `text[first]` 是大写段的第一码，返回这一段结束的位置（`UppercaseBlockEnd`）。
fn uppercase_block_end(text: &[u8], first: usize) -> usize {
    let mut end = first + 1;
    if end < text.len() && is_ascii_upper(text[end]) {
        end += 1;
    }
    end
}

/// 扫描 `text`，每碰到一段（反引号段、末尾段或大写段）调用 `on_block(begin, end, codes_begin, kind)`；
/// `codes_begin` 是第一码的位置。返回最后一节（最后一个 `'` 或段之后）的起点（`ScanBlocks`）。
pub fn scan_blocks(
    text: &[u8],
    triggers: UppercaseTriggers,
    mut on_block: impl FnMut(usize, usize, usize, BlockKind),
) -> usize {
    let mut chunk_start = 0;
    let mut index = 0;
    while index < text.len() {
        match text[index] {
            b'\'' => {
                index += 1;
                chunk_start = index;
            }
            MID_SENTENCE_MARKER => {
                let end = block_end(text, index);
                on_block(index, end, index + 1, BlockKind::Mid);
                chunk_start = end;
                index = end;
            }
            END_HELPCODE_MARKER => {
                let end = end_block_end(text, index, 2);
                on_block(index, end, index + 1, BlockKind::End);
                chunk_start = end;
                index = end;
            }
            ch if triggers.mid && starts_uppercase_block(ch, index - chunk_start) => {
                let end = uppercase_block_end(text, index);
                on_block(index, end, index, BlockKind::Mid);
                chunk_start = end;
                index = end;
            }
            ch if triggers.end && starts_uppercase_block(ch, index - chunk_start) => {
                let end = end_block_end(text, index, 1);
                on_block(index, end, index, BlockKind::End);
                chunk_start = end;
                index = end;
            }
            _ => index += 1,
        }
    }
    chunk_start
}

/// `text` 最后一个 `'` 或段之后那一节的起点（`ChunkStart`）。
pub fn chunk_start(text: &[u8], triggers: UppercaseTriggers) -> usize {
    scan_blocks(text, triggers, |_, _, _, _| {})
}

/// `text` 末尾能否接一个反引号：当前这一节是偶数个、至少两个键，即光标前恰好是完整的双拼两键
/// 音节（`AcceptsMarker`）。
pub fn accepts_marker(text: &[u8], triggers: UppercaseTriggers) -> bool {
    let chunk_length = text.len() - chunk_start(text, triggers);
    chunk_length >= 2 && chunk_length.is_multiple_of(2)
}

/// 光标停在 `text[caret]` 时能否插入一个反引号：光标前按 `accepts_marker` 判断；光标后紧跟的
/// 已经是这个音节的段时不收，一个音节上叠两段只会让后一段把前一段盖掉；大写触发开着时，光标后的
/// 大写字母正好开这个音节的段（`AcceptsMarkerAt`）。
pub fn accepts_marker_at(text: &[u8], caret: usize, triggers: UppercaseTriggers) -> bool {
    caret <= text.len()
        && accepts_marker(&text[..caret], triggers)
        && (caret == text.len()
            || (text[caret] != MID_SENTENCE_MARKER
                && !((triggers.mid || triggers.end) && is_ascii_upper(text[caret]))))
}

/// 光标停在 `text[caret]` 时能否插入一段末尾段（'|'）：只在输入串末尾，且前面恰好是完整的双拼
/// 两键音节。末尾段只筛整串的候选，插在句中没有意义（`AcceptsEndMarkerAt`）。
pub fn accepts_end_marker_at(text: &[u8], caret: usize, triggers: UppercaseTriggers) -> bool {
    caret == text.len() && accepts_marker(text, triggers)
}

/// 光标停在 `text[caret]` 时大写字母能否作为第二码：光标前恰好是一段只有第一码的句中段
/// （`AcceptsSecondCodeAt`）。末尾段的辅码大小写都算，不经过这里。句中实现启用前的占位，先留着契约。
#[allow(dead_code)]
pub fn accepts_second_code_at(text: &[u8], caret: usize, triggers: UppercaseTriggers) -> bool {
    if caret > text.len() {
        return false;
    }
    let mut open = false;
    scan_blocks(&text[..caret], triggers, |_, end, codes_begin, kind| {
        open = kind == BlockKind::Mid && end == caret && end - codes_begin == 1;
    });
    open && text[caret - 1].is_ascii_alphabetic()
}

/// 微软/搜狗/紫光双拼的 `;` 是 `ing` 韵母，光标前这一节是奇数键时它是编码键。大写触发开着、或
/// 串里有句中或末尾段时，段不算这一节的键；都没有时保持原来只看最后一个 `'` 之后的规则
/// （`AcceptsSemicolonFinalAt`）。
pub fn accepts_semicolon_final_at(text: &[u8], caret: usize, triggers: UppercaseTriggers) -> bool {
    if caret > text.len() {
        return false;
    }
    let mut scan = triggers.mid || triggers.end;
    for &byte in &text[..caret] {
        if scan {
            break;
        }
        scan = byte == END_HELPCODE_MARKER || byte == MID_SENTENCE_MARKER;
    }
    let start = if scan {
        chunk_start(&text[..caret], triggers)
    } else {
        text[..caret]
            .iter()
            .rposition(|&byte| byte == b'\'')
            .map_or(0, |at| at + 1)
    };
    (caret - start) % 2 == 1
}

fn count_segments(segmentation: &str) -> usize {
    if segmentation.is_empty() {
        0
    } else {
        segmentation.bytes().filter(|&byte| byte == b'\'').count() + 1
    }
}

/// `parse_mid_sentence_helpcodes` 的结果：去段串、段约束、显示装饰与末尾段辅码。
#[derive(Debug, Clone, Default)]
pub struct ParsedHelpcodes {
    /// 每段换成一个 `'` 之后的输入串，保留大小写；位于末尾的末尾段直接去掉。
    pub input: String,
    /// `input[i]` 在原串里的下标，末尾多一项等于原串长度。
    pub source_index: Vec<usize>,
    /// 句中辅助码约束，按去段串的双拼切分计音节序号。
    pub helpcodes: Vec<SyllableHelpcode>,
    /// 每段原样的文本（含还没敲码的光杆标记）及它挂在哪个音节后面，预编辑据此还原显示。
    pub decorations: Vec<(usize, String)>,
    /// 位于输入串末尾的末尾段敲下的辅码（小写，按敲的顺序，至多两个）。不在末尾的末尾段不筛。
    pub end_helpcodes: String,
}

/// 串里有没有任何一段（`has_mid_sentence_helpcode`）。
pub fn has_mid_sentence_helpcode(raw: &[u8], triggers: UppercaseTriggers) -> bool {
    if raw.contains(&MID_SENTENCE_MARKER) || raw.contains(&END_HELPCODE_MARKER) {
        return true;
    }
    (triggers.mid || triggers.end) && {
        let mut found = false;
        scan_blocks(raw, triggers, |_, _, _, _| found = true);
        found
    }
}

/// 解析输入串里的辅助码段（`parse_mid_sentence_helpcodes`）：每段换成 `'`（末尾的末尾段去掉），
/// 句中段产出逐音节约束，末尾段产出辅码。
pub fn parse_mid_sentence_helpcodes(
    raw_with_cases: &str,
    profile: &ShuangpinProfile,
    triggers: UppercaseTriggers,
) -> ParsedHelpcodes {
    let text = raw_with_cases.as_bytes();
    let mut parsed = ParsedHelpcodes {
        input: String::with_capacity(text.len()),
        source_index: Vec::with_capacity(text.len() + 1),
        ..ParsedHelpcodes::default()
    };
    let mut blocks = Vec::new();
    scan_blocks(text, triggers, |begin, end, codes_begin, kind| {
        blocks.push((begin, end, codes_begin, kind));
    });
    let mut next_block = blocks.iter().peekable();
    let mut index = 0;
    while index < text.len() {
        let Some(&&(_begin, end, codes_begin, kind)) = next_block.peek().filter(|b| b.0 == index)
        else {
            parsed.input.push(text[index] as char);
            parsed.source_index.push(index);
            index += 1;
            continue;
        };
        next_block.next();
        let syllables = count_segments(&segment_input(&parsed.input.to_lowercase(), profile));
        if kind == BlockKind::End {
            // 末尾段：在输入串末尾时按词筛（码交给请求），否则只留显示、当分隔符。
            if syllables > 0 {
                parsed
                    .decorations
                    .push((syllables - 1, raw_with_cases[index..end].to_owned()));
            }
            if end == text.len() {
                parsed.end_helpcodes.clear();
                for &code in &text[codes_begin..end] {
                    parsed
                        .end_helpcodes
                        .push((code as char).to_ascii_lowercase());
                }
                index = end;
                continue;
            }
        } else if syllables > 0 {
            let syllable = syllables - 1;
            parsed
                .decorations
                .push((syllable, raw_with_cases[index..end].to_owned()));
            if end > codes_begin {
                let helpcode = SyllableHelpcode {
                    syllable,
                    first: (text[codes_begin] as char).to_ascii_lowercase() as u8,
                    second: if end > codes_begin + 1 {
                        (text[codes_begin + 1] as char).to_ascii_lowercase() as u8
                    } else {
                        0
                    },
                };
                // 同一个音节敲了两段，以后一段为准。
                if let Some(existing) = parsed
                    .helpcodes
                    .iter_mut()
                    .find(|item| item.syllable == syllable)
                {
                    *existing = helpcode;
                } else {
                    parsed.helpcodes.push(helpcode);
                }
            }
        }
        // 段同时是一个确定的音节边界，换成手动分隔符；已经有分隔符就不再叠一个。
        if !parsed.input.is_empty() && !parsed.input.ends_with('\'') {
            parsed.input.push('\'');
            parsed.source_index.push(index);
        }
        index = end;
    }
    parsed.source_index.push(text.len());
    parsed
}

/// `syllable_helpcode_matches`：字的辅助码第一码与约束相同，且约束给了第二码时第二码也相同。
pub fn syllable_helpcode_matches(helpcode: &SyllableHelpcode, code: &[u8]) -> bool {
    !code.is_empty()
        && code[0] == helpcode.first
        && (helpcode.second == 0 || code.get(1) == Some(&helpcode.second))
}

/// 缓存键用的签名：同一串拼音带不同约束，候选不同，不能共用缓存
/// （`syllable_helpcodes_signature`）。
pub fn syllable_helpcodes_signature(helpcodes: &[SyllableHelpcode]) -> String {
    let mut signature = String::new();
    for helpcode in helpcodes {
        signature.push('`');
        signature.push_str(&helpcode.syllable.to_string());
        signature.push(helpcode.first as char);
        if helpcode.second != 0 {
            signature.push(helpcode.second as char);
        }
    }
    signature
}

/// 词（从第一个音节起）的每个字都满足落在它位置上的约束；超出词长的约束不管
/// （`satisfies_syllable_helpcodes`）。
pub fn satisfies_syllable_helpcodes(
    helpcodes: &[SyllableHelpcode],
    word: &str,
    keymap: &crate::helpcode::HelpcodeKeymap,
) -> bool {
    let chars: Vec<char> = word.chars().collect();
    helpcodes.iter().all(|helpcode| {
        chars.get(helpcode.syllable).is_none_or(|ch| {
            let mut buf = [0u8; 4];
            keymap
                .code(ch.encode_utf8(&mut buf))
                .is_some_and(|code| syllable_helpcode_matches(helpcode, code.as_bytes()))
        })
    })
}

/// 把每段辅助码按原样接回切分串里对应音节的后面：`ul'pb'ih'fa` → `ul'pb`x'ih'fa`
/// （`decorate_segmentation`）。切分串必须是由去段串切出来的，音节序号才对得上。
pub fn decorate_segmentation(segmentation: &str, decorations: &[(usize, String)]) -> String {
    if decorations.is_empty() || segmentation.is_empty() {
        return segmentation.to_owned();
    }
    let mut parts: Vec<String> = segmentation.split('\'').map(str::to_owned).collect();
    for &(syllable, ref text) in decorations {
        if let Some(part) = parts.get_mut(syllable) {
            part.push_str(text);
        }
    }
    parts.join("'")
}
