//! 按原始键位置连接已确认的中英日词条。这里只选择完整路径，不查询词库或推断未知词。

use std::ops::Range;

mod dictionaries;
pub use dictionaries::DictionaryComposer;

/// 词条所属的语言；不等同于键盘布局或输入方案。
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
pub enum Language {
    Chinese,
    English,
    Japanese,
}

/// 提供者已确认的词条。范围按原始输入的 UTF-8 字节计数，名次仅来自同读音候选。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Lexeme {
    pub input: Range<usize>,
    pub text: String,
    pub language: Language,
    pub rank: u16,
}

/// 完整路径里相邻同语言词条合成的片段，供后续转换与上屏保留语言边界。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Segment {
    pub input: Range<usize>,
    pub text: String,
    pub language: Language,
}

/// 混合转换及中文歧义证据。中文路径完整时，调用方必须把中文留在混合候选之前。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Conversion {
    pub segments: Vec<Segment>,
    pub chinese_complete: bool,
}

/// 每次解码的原始输入字节上限；超出后调用方保留原候选。
pub const MAX_INPUT_BYTES: usize = 96;
/// 每次解码的词条数上限，包含无效词条，防止无效数据绕过预算。
pub const MAX_LEXEMES: usize = 4096;

/// 选择完整混合路径。词条数、切换数、同读音名次依次更少者优先；完整单语言
/// 路径的词条更少时不拼凑混合候选，同数时保留混合备选。未选语言、无效范围和空文字不参与搜索。
///
/// 返回的片段覆盖全部输入并至少涉及两种语言。没有路径或超出预算时返回 `None`，
/// 不提供部分转换，也不把缺失的尾部当成原文拼接。
pub fn decode(input: &str, lexemes: &[Lexeme], enabled: &[Language]) -> Option<Conversion> {
    if input.is_empty() || input.len() > MAX_INPUT_BYTES || lexemes.len() > MAX_LEXEMES {
        return None;
    }
    let mut outgoing = vec![Vec::new(); input.len() + 1];
    for (index, lexeme) in lexemes.iter().enumerate() {
        if lexeme.input.start < lexeme.input.end
            && lexeme.input.end <= input.len()
            && input.is_char_boundary(lexeme.input.start)
            && input.is_char_boundary(lexeme.input.end)
            && !lexeme.text.is_empty()
            && enabled.contains(&lexeme.language)
        {
            outgoing[lexeme.input.start].push(index);
        }
    }

    // 语言集合有八种，每个集合再保留三种末尾语言；空路径占用第零格。
    let mut states: Vec<[Option<Path>; 24]> = vec![std::array::from_fn(|_| None); input.len() + 1];
    states[0][0] = Some(Path::default());
    for position in 0..input.len() {
        for slot in 0..24 {
            let Some(previous) = states[position][slot].clone() else {
                continue;
            };
            let used = slot / 3;
            let last = slot % 3;
            for &index in &outgoing[position] {
                let lexeme = &lexemes[index];
                let language = lexeme.language as usize;
                let target_slot = (used | (1 << language)) * 3 + language;
                let switches = previous.switches + usize::from(used != 0 && last != language);
                let rank = previous.rank + usize::from(lexeme.rank);
                let target = &mut states[lexeme.input.end][target_slot];
                // 成本已经更差时不复制整条前缀，同成本仍按原有稳定序比较。
                if target.as_ref().is_some_and(|old| {
                    (previous.lexemes.len() + 1, switches, rank)
                        > (old.lexemes.len(), old.switches, old.rank)
                }) {
                    continue;
                }
                let mut path = previous.clone();
                path.lexemes.push(index);
                path.switches = switches;
                path.rank = rank;
                if target
                    .as_ref()
                    .is_none_or(|old| path.precedes(old, lexemes))
                {
                    *target = Some(path);
                }
            }
        }
    }

    let mut best: Option<&Path> = None;
    for (slot, path) in states[input.len()].iter().enumerate() {
        if (slot / 3).count_ones() < 2 {
            continue;
        }
        if let Some(path) = path {
            if best.is_none_or(|old| path.precedes(old, lexemes)) {
                best = Some(path);
            }
        }
    }
    let best = best?;
    // 完整单语言词条能用更少片段回答输入时，不强行把它拆成跨语言碎片。
    if states[input.len()].iter().enumerate().any(|(slot, path)| {
        (slot / 3).count_ones() == 1
            && path
                .as_ref()
                .is_some_and(|single| single.lexemes.len() < best.lexemes.len())
    }) {
        return None;
    }
    let chinese_complete = states[input.len()]
        .iter()
        .enumerate()
        .any(|(slot, path)| slot / 3 == (1 << Language::Chinese as usize) && path.is_some());
    let mut segments: Vec<Segment> = Vec::new();
    for &index in &best.lexemes {
        let lexeme = &lexemes[index];
        if let Some(last) = segments
            .last_mut()
            .filter(|segment| segment.language == lexeme.language)
        {
            last.input.end = lexeme.input.end;
            last.text.push_str(&lexeme.text);
        } else {
            segments.push(Segment {
                input: lexeme.input.clone(),
                text: lexeme.text.clone(),
                language: lexeme.language,
            });
        }
    }
    Some(Conversion {
        segments,
        chinese_complete,
    })
}

#[derive(Clone, Default)]
struct Path {
    lexemes: Vec<usize>,
    switches: usize,
    rank: usize,
}

impl Path {
    fn precedes(&self, other: &Self, lexemes: &[Lexeme]) -> bool {
        let cost = |path: &Self| (path.lexemes.len(), path.switches, path.rank);
        let order = cost(self).cmp(&cost(other));
        if !order.is_eq() {
            return order.is_lt();
        }
        // 词库枚举顺序不能改变同分路径，固定按语言、词面与范围比较。
        let key = |index: &usize| {
            let lexeme = &lexemes[*index];
            (
                lexeme.language,
                lexeme.text.as_str(),
                lexeme.input.start,
                lexeme.input.end,
            )
        };
        self.lexemes
            .iter()
            .map(key)
            .cmp(other.lexemes.iter().map(key))
            .is_lt()
    }
}

#[cfg(test)]
#[path = "multilingual/tests.rs"]
mod tests;

#[cfg(test)]
#[path = "multilingual/reference_tests.rs"]
mod reference_tests;
