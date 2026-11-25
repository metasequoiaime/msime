//! 全拼九键精确模式下，按音节把候选的完整读音对齐到实际输入的数字。

use super::{
    is_initials_row, rank_key, retain_unique_words, KeyGrid, NineKeySession, QuanpinDictionary,
    WordItem,
};
use crate::pinyin::syllables::is_intact;
use std::cmp::Reverse;

pub(super) struct Alignment {
    pub(super) covered: usize,
    completion: bool,
}

fn is_single_initial(syllable: &str) -> bool {
    matches!(syllable.as_bytes(), [first] if !matches!(first, b'a' | b'e' | b'o'))
}

pub(super) fn align(
    reading: &str,
    digits: &str,
    locked: &[String],
    splits: &[usize],
    mut piece: impl FnMut(&str),
) -> Option<Alignment> {
    let mut covered = 0;
    let mut completion = false;
    let mut syllables = reading.split('\'').enumerate().peekable();
    while let Some((index, syllable)) = syllables.next() {
        let remaining = &digits[covered..];
        if remaining.is_empty() || syllable.is_empty() {
            return None;
        }
        let typed = if let Some(spelling) = locked.get(index) {
            if syllable == spelling
                || (is_single_initial(spelling) && syllable.starts_with(spelling))
            {
                spelling.as_str()
            } else {
                return None;
            }
        } else {
            let typed = syllable.get(..syllable.len().min(remaining.len()))?;
            if typed != syllable {
                if syllables.peek().is_some() {
                    return None;
                }
                // 短词的完整尾音节按字面解释；长句仍可以继续补全尾音节。
                if index < 3 && is_intact(typed) {
                    return None;
                }
                completion = !is_single_initial(typed);
            }
            typed
        };
        if KeyGrid::NineKey.matching_code_prefix_len(typed, remaining) != Some(typed.len()) {
            return None;
        }
        let end = covered + typed.len();
        if splits.iter().any(|&split| split > covered && split < end)
            || (completion && splits.contains(&end))
        {
            return None;
        }
        covered = end;
        piece(typed);
    }
    Some(Alignment {
        covered,
        completion,
    })
}

pub(super) fn append_long_tail_completions(
    dictionary: &QuanpinDictionary,
    key: &str,
    rows: &mut Vec<WordItem>,
) {
    if key.bytes().filter(|&byte| byte == b'\'').count() < 3
        || !rows
            .iter()
            .any(|row| row.source.is_dictionary() && row.canonical_pinyin == key)
    {
        return;
    }
    let segments = key.split('\'').map(str::to_owned).collect::<Vec<_>>();
    rows.extend(dictionary.grid_tail_completions(&segments, key));
}

impl NineKeySession {
    pub(super) fn precise(&self) -> bool {
        self.precise_nine_key && self.grid == KeyGrid::NineKey
    }

    pub(super) fn precise_completion(&self, item: &WordItem) -> bool {
        if is_initials_row(item) {
            return false;
        }
        align(
            &item.canonical_pinyin,
            &item.pinyin,
            &self.locked,
            &self.splits,
            |_| {},
        )
        .is_none_or(|reading| reading.completion)
    }

    pub(super) fn order_precise_candidates(
        &self,
        candidates: &mut Vec<WordItem>,
        initials_lead: bool,
    ) {
        candidates.sort_by_cached_key(|item| {
            let key = rank_key(item, false, initials_lead, KeyGrid::NineKey);
            (key.0, self.precise_completion(item), key)
        });
        retain_unique_words(candidates);
    }

    pub(super) fn restore_precise_groups(&self, candidates: &mut [WordItem]) {
        candidates
            .sort_by_cached_key(|item| (Reverse(item.pinyin.len()), self.precise_completion(item)));
    }

    pub(super) fn write_precise_reading(&self, item: &WordItem, reading: &mut String) -> bool {
        let result = align(
            &item.canonical_pinyin,
            &item.pinyin,
            &self.locked,
            &self.splits,
            |piece| {
                if !reading.is_empty() {
                    reading.push('\'');
                }
                reading.push_str(piece);
            },
        );
        if result.is_none() {
            reading.clear();
        }
        result.is_some()
    }
}
