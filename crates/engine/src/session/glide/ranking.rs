//! 连滑的候选容量与名次合并；不增加组字状态，也不改变明确选词的提交边界。

use super::*;

pub(super) const CANDIDATE_LIMIT: usize = 24;

/// 看末尾的文字而非整句是否含汉字，让「你好 hello」下一笔也能获得英文弱先验。
pub(super) fn english_ending(word: &str) -> Option<bool> {
    word.chars()
        .rev()
        .find(|letter| letter.is_alphabetic())
        .map(|letter| letter.is_ascii())
}

pub(super) fn contains_english(word: &str) -> bool {
    word.bytes().any(|letter| letter.is_ascii_alphabetic())
}

fn push_unique(rows: &mut Vec<WordItem>, row: WordItem) {
    if !rows.iter().any(|other| other.word == row.word) {
        rows.push(row);
    }
}

fn joined(before: &WordItem, after: &WordItem, raw: &str) -> WordItem {
    let space = before
        .word
        .as_bytes()
        .last()
        .is_some_and(u8::is_ascii_alphabetic)
        || after
            .word
            .as_bytes()
            .first()
            .is_some_and(u8::is_ascii_alphabetic);
    let mut row = after.clone();
    row.word = format!(
        "{}{}{}",
        before.word,
        if space { " " } else { "" },
        after.word
    )
    .into();
    row.pinyin = raw.to_owned().into();
    row.canonical_pinyin = raw.to_owned().into();
    row
}

/// 首选前缀保留前五项后续，其余按名次和扩展，兼顾眼前的可选项与早先备选。
pub(super) fn combinations(previous: &[WordItem], next: &[WordItem], raw: &str) -> Vec<WordItem> {
    let mut rows = Vec::with_capacity(CANDIDATE_LIMIT);
    if let Some(before) = previous.first() {
        for after in next.iter().take(5) {
            push_unique(&mut rows, joined(before, after, raw));
        }
    }
    for rank in 0..previous.len().saturating_add(next.len()) {
        for (before_rank, before) in previous.iter().enumerate().take(rank + 1) {
            let Some(after) = next.get(rank - before_rank) else {
                continue;
            };
            push_unique(&mut rows, joined(before, after, raw));
            if rows.len() == CANDIDATE_LIMIT {
                return rows;
            }
        }
    }
    rows
}

/// 完整中文读法保留首选优势，组合候选也有座位，不因词典恰好返回 24 项而全部消失。
pub(super) fn interleave(full: Vec<WordItem>, combinations: Vec<WordItem>) -> Vec<WordItem> {
    let (mut full, mut combinations) = (full.into_iter(), combinations.into_iter());
    let mut rows = Vec::with_capacity(CANDIDATE_LIMIT);
    loop {
        let (a, b) = (full.next(), combinations.next());
        if a.is_none() && b.is_none() {
            return rows;
        }
        for row in a.into_iter().chain(b) {
            push_unique(&mut rows, row);
            if rows.len() == CANDIDATE_LIMIT {
                return rows;
            }
        }
    }
}
