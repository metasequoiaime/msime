//! 一到两个音节的触点纠错：查完整词条，按落点罚分后的词频排序。

use super::super::typo_edges::{priced_shuangpin_variants, LearnedTypos, SHUANGPIN_TYPO_TYPES};
use super::super::utils::cvt_single_sp_to_pinyin;
use super::ShuangpinDictionary;
use crate::quanpin::typo_edges::{TYPO_KEY_BUDGET, TYPO_ROWS_PER_KEY};
use crate::types::{CandidateSource, QueryRequest, WordItem};

fn score(weight: i64) -> f64 {
    (weight.max(1) as f64).ln()
}

fn coverage(row: &WordItem) -> usize {
    row.pinyin.bytes().filter(|&key| key != b'\'').count()
}

impl ShuangpinDictionary {
    /// 只在最终的普通请求上合并，辅助码和模糊音的中间查询不产生短词纠错。
    pub(in crate::shuangpin) fn merge_short_touch_words(
        &mut self,
        candidates: &mut Vec<WordItem>,
        request: &QueryRequest,
    ) {
        let raw = request.raw_input.as_str();
        if !request.valid
            || !matches!(raw.len(), 2 | 4)
            || !raw
                .bytes()
                .all(|key| key.is_ascii_lowercase() || key == b';')
            || request.raw_input_with_cases != raw
            || !request.end_helpcodes.is_empty()
            || !request.syllable_helpcodes.is_empty()
            || request.key_touches.len() != raw.len()
            || request.key_touches.iter().any(Option::is_none)
            || candidates.is_empty()
        {
            return;
        }
        let Some(geometry) = request.key_geometry.as_deref() else {
            return;
        };
        let types = self.autocorrect_types & SHUANGPIN_TYPO_TYPES;
        if types == 0 {
            return;
        }
        let codes: Vec<&str> = (0..raw.len())
            .step_by(2)
            .map(|at| &raw[at..at + 2])
            .collect();
        let literal: Vec<String> = codes
            .iter()
            .map(|code| cvt_single_sp_to_pinyin(code, self.profile))
            .collect();
        let tuning = request.touch_tuning;
        let variants = priced_shuangpin_variants(
            self.profile,
            &codes,
            &request.key_touches,
            geometry,
            &tuning,
            LearnedTypos::new(&self.typo_profile, self.profile.kind),
            types,
        );
        let mut planned: Vec<(String, f64)> = Vec::new();
        for (position, alternatives) in variants.iter().enumerate() {
            for variant in alternatives
                .iter()
                .filter(|v| v.penalty <= tuning.show_penalty)
            {
                if literal
                    .iter()
                    .enumerate()
                    .any(|(at, value)| at != position && value.is_empty())
                {
                    continue;
                }
                let mut syllables = literal.clone();
                syllables[position] = variant.syllable.to_owned();
                let key = syllables.join("'");
                if let Some((_, penalty)) = planned.iter_mut().find(|(old, _)| *old == key) {
                    *penalty = penalty.min(variant.penalty);
                } else {
                    planned.push((key, variant.penalty));
                }
            }
        }
        planned.sort_by(|left, right| {
            left.1
                .total_cmp(&right.1)
                .then_with(|| left.0.cmp(&right.0))
        });
        planned.truncate(TYPO_KEY_BUDGET);
        let misses: Vec<String> = planned
            .iter()
            .filter(|(key, _)| self.typo_span_cache.get_ref(key).is_none())
            .map(|(key, _)| key.clone())
            .collect();
        if !misses.is_empty() {
            let mut rows = self
                .database
                .query_exact_keys_per_key(&misses, TYPO_ROWS_PER_KEY);
            for key in misses {
                let found = rows.remove(&key).unwrap_or_default();
                self.typo_span_cache.insert(key, found);
            }
        }
        let literal_key = literal.join("'");
        let literal_best = candidates
            .iter()
            .filter(|row| {
                row.source.is_dictionary()
                    && row.canonical_pinyin == literal_key
                    && row.corrected_from.is_empty()
            })
            .map(|row| score(row.weight))
            .reduce(f64::max);
        let mut corrected = Vec::new();
        for (key, penalty) in planned {
            let Some(rows) = self.typo_span_cache.get_ref(&key) else {
                continue;
            };
            for row in rows {
                let rank = score(row.weight) - penalty;
                if literal_best.is_some_and(|best| rank < best - tuning.show_margin)
                    || candidates.iter().enumerate().any(|(index, old)| {
                        old.word == row.value && (index == 0 || coverage(old) >= raw.len())
                    })
                {
                    continue;
                }
                let mut word = WordItem::new(
                    raw,
                    row.value.clone(),
                    row.weight,
                    CandidateSource::Database,
                    row.key.clone(),
                );
                word.corrected_from = raw.to_owned().into();
                word.touch_supported = true;
                corrected.push((word, rank));
            }
        }
        corrected.sort_by(|left, right| {
            right
                .1
                .total_cmp(&left.1)
                .then_with(|| left.0.canonical_pinyin.cmp(&right.0.canonical_pinyin))
                .then_with(|| left.0.word.cmp(&right.0.word))
        });
        let mut count = 0;
        for (row, rank) in corrected {
            if candidates.iter().enumerate().any(|(index, old)| {
                old.word == row.word && (index == 0 || coverage(old) >= raw.len())
            }) {
                continue;
            }
            candidates.retain(|old| old.word != row.word);
            // 字面首选保持；其余完整词比较罚分后的词频，前缀留在完整纠错词之后。
            let at = candidates
                .iter()
                .enumerate()
                .skip(1)
                .find(|(_, old)| {
                    // `corrected` 已按排名从高到低插入，下面词库行的 `score < rank` 随排名下降只会更严，后插入的纠错行不可能排到先插入的前面。
                    if !old.corrected_from.is_empty() {
                        return false;
                    }
                    coverage(old) < raw.len()
                        || (old.source.is_dictionary() && score(old.weight) < rank)
                })
                .map_or(candidates.len(), |(at, _)| at);
            candidates.insert(at, row);
            count += 1;
            if count == TYPO_ROWS_PER_KEY {
                break;
            }
        }
    }
}
