//! 普通拼音里的大写字面片段：只查询小写片段，候选携带实际覆盖的原文字节。

use super::editing::quanpin_raw_boundaries;
use super::input::InputSession;
use crate::ime::queries::{spells_whole_syllables, MIXED_ENGLISH_LIMIT};
use crate::quanpin::scheme::QuanpinScheme;
use crate::shuangpin::query::raw_length_for_effective_prefix;
use crate::text::count_han_chars;
use crate::types::{
    CandidateSource, KeyResult, LocalInputMode, QueryRequest, SchemeType, WordItem,
};
use crate::user_dictionary::positions::apply_fixed_positions_with_state;

impl InputSession {
    /// 大写逐字编辑，小写继续按原方案的音节导航；显示原文不能抹掉小写片段的单位边界。
    pub(super) fn literal_raw_boundaries(&self) -> Vec<usize> {
        let raw = self.raw_with_cases();
        let mut boundaries = vec![0];
        let mut start = 0;
        while start < raw.len() {
            let end = if raw.as_bytes()[start].is_ascii_uppercase() {
                start + 1
            } else {
                raw[start..]
                    .bytes()
                    .position(|byte| byte.is_ascii_uppercase())
                    .map_or(raw.len(), |offset| start + offset)
            };
            let span = &raw[start..end];
            if raw.as_bytes()[start].is_ascii_uppercase() {
                boundaries.push(end);
            } else {
                let units = if self.is_shuangpin() {
                    crate::shuangpin::query::segment_raw_boundaries(span, self.profile)
                } else {
                    let mut request = QueryRequest::default();
                    QuanpinScheme::build_request_from_raw_into(span, span, &mut request);
                    quanpin_raw_boundaries(span, &request.raw_segmentation)
                };
                boundaries.extend(units.into_iter().map(|offset| start + offset));
                boundaries.push(end);
            }
            start = end;
        }
        boundaries.sort_unstable();
        boundaries.dedup();
        boundaries
    }

    pub(super) fn has_literal_capitals(&self) -> bool {
        if self.dedicated_english
            || self.local_mode != LocalInputMode::None
            || !matches!(self.scheme(), SchemeType::Quanpin | SchemeType::Shuangpin)
        {
            return false;
        }
        let raw = self.raw_with_cases();
        let Some(first) = raw.bytes().position(|byte| byte.is_ascii_uppercase()) else {
            return false;
        };
        // 末尾辅助码只能接在没有字面大写的拼音之后，全大写词不能被误判为「拼音加双辅助码」。
        let trailing_helpcode =
            first > 0 && first >= raw.len().saturating_sub(2) && self.has_active_helpcode();
        !trailing_helpcode
            && !(self.is_shuangpin()
                && self.engine.request().enable_shuangpin_helpcode
                && self
                    .helpcode_keymap
                    .as_ref()
                    .is_some_and(|keymap| !keymap.is_empty())
                && crate::shuangpin::engine::direct_helpcodes_own_capitals(raw, self.profile))
    }

    /// 大写片段不送进中文查询。一次候选覆盖开头的字面片段和第一个小写片段，后续片段留待选词后继续。
    pub(super) fn update_literal_candidates(&mut self) -> bool {
        if !self.has_literal_capitals() {
            return false;
        }
        let raw = self.raw_with_cases().to_owned();
        let end = self.caret_position().min(raw.len());
        let prefix = &raw[..end];
        let start = prefix.bytes().position(|byte| byte.is_ascii_lowercase());
        let mut rows = Vec::new();
        let mut literal_first = true;
        if let Some(start) = start {
            let lower_end = prefix[start..]
                .bytes()
                .position(|byte| byte.is_ascii_uppercase())
                .map_or(prefix.len(), |offset| start + offset);
            let lower = &prefix[start..lower_end];
            literal_first =
                start > 0 && !spells_whole_syllables(lower, self.scheme(), self.profile);
            for item in self.engine.query_raw_candidates(lower, lower) {
                // 双拼字典也可能返回全拼读音；只有实际匹配原文的编码才能直接作为字节数。
                let effective = if self.is_shuangpin() {
                    let code_length = item.pinyin.bytes().filter(|&byte| byte != b'\'').count();
                    if code_length > 0
                        && lower
                            .bytes()
                            .filter(|&byte| byte != b'\'')
                            .take(code_length)
                            .eq(item.pinyin.bytes().filter(|&byte| byte != b'\''))
                    {
                        code_length
                    } else {
                        count_han_chars(&item.word).saturating_mul(2)
                    }
                } else if item.corrected_from.is_empty() {
                    item.pinyin.bytes().filter(|&byte| byte != b'\'').count()
                } else {
                    item.corrected_from.len()
                };
                if effective == 0 {
                    continue;
                }
                let consumed = raw_length_for_effective_prefix(lower, effective);
                let mut consumed_end = start + consumed;
                // 转完这个小写片段后，紧接的大写片段可以一起保留，下一段小写仍独立查询。
                if lower[consumed..].bytes().all(|byte| byte == b'\'') {
                    consumed_end = lower_end;
                    consumed_end += prefix[lower_end..]
                        .bytes()
                        .take_while(|byte| byte.is_ascii_uppercase() || *byte == b'\'')
                        .count();
                }
                let word = format!(
                    "{}{}{}",
                    prefix[..start].replace('\'', ""),
                    item.word,
                    prefix[start + consumed..consumed_end].replace('\'', "")
                );
                if !rows.iter().any(|row: &WordItem| row.word == word) {
                    rows.push(WordItem::new(
                        &prefix[..consumed_end],
                        word,
                        item.weight,
                        CandidateSource::Generated,
                        "",
                    ));
                }
            }
        }
        let literal = WordItem::new(prefix, prefix, 0, CandidateSource::Fallback, "");
        if !prefix.is_empty() && !rows.iter().any(|row| row.word == prefix) {
            if literal_first {
                rows.insert(0, literal);
            } else {
                rows.push(literal);
            }
        }
        // 英文行保留词库身份，大小写只在快照和上屏副本中恢复。内部光标时不提供会整段结束组字的英文行。
        if end == raw.len()
            && self.english_options.mixed_candidates
            && raw.len() >= self.english_options.minimum_prefix
            && raw.bytes().all(|byte| byte.is_ascii_alphabetic())
        {
            let key = raw.to_ascii_lowercase();
            let mut english = self
                .queries
                .english_dictionary()
                .query_prefix(&key, MIXED_ENGLISH_LIMIT);
            // 完全匹配的英文优先，补全仍可选择；未知词的原文始终有自己的候选。
            if let Some(index) = english.iter().position(|row| row.pinyin == key) {
                let exact = english.remove(index);
                let mut display = exact.word.clone();
                self.apply_english_word_case(exact.source, &mut display);
                if display == raw {
                    rows.retain(|row| row.source != CandidateSource::Fallback || row.word != raw);
                }
                rows.insert(0, exact);
            }
            rows.extend(english);
            let journal = self.journal_path();
            let context = self.position_context(true, false);
            apply_fixed_positions_with_state(
                journal,
                context.as_ref(),
                &mut rows,
                false,
                None,
                true,
                journal.try_exists().unwrap_or(false),
            );
        }
        self.prefix_active = end < raw.len();
        self.personal_reranked = false;
        self.ranking_candidates = None;
        self.mixed_candidates = rows;
        true
    }

    pub(super) fn literal_candidate_consumed(&self, item: &WordItem) -> Option<usize> {
        (self.has_literal_capitals()
            && item.source.is_generated_or_fallback()
            && item.canonical_pinyin.is_empty()
            && !item.pinyin.is_empty()
            && self.raw_with_cases().starts_with(&item.pinyin))
        .then_some(item.pinyin.len())
    }

    /// 字面混合行不具有词库读音；按候选记录的原文范围推进，不调词频或把拼接结果学成汉语词。
    pub(super) fn commit_literal_candidate(&mut self, index: usize) -> Option<KeyResult> {
        let item = self.candidates().get(index)?;
        let consumed = self.literal_candidate_consumed(item)?;
        let text = item.word.clone();
        let rest = self.raw_with_cases()[consumed..]
            .trim_start_matches('\'')
            .to_owned();
        self.reset_composition();
        self.chain.reset();
        if !rest.is_empty() {
            self.engine
                .replace_active_raw_input(&rest.to_ascii_lowercase(), &rest);
            self.update_mixed_candidates();
        }
        Some(KeyResult::committed(text))
    }
}
