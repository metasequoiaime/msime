//! 普通拼音的字面输入：大写保留原样，特殊符号使整段只提供原文候选。

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

const LITERAL_SYMBOLS: &str = "#$%&";

impl InputSession {
    /// 有组字时特殊符号留在原文中；空大写快捷入口还没有模式内容，也可回到字面组字。
    pub(super) fn takes_literal_symbol(&self, value: u8) -> bool {
        !self.dedicated_english
            && matches!(self.scheme(), SchemeType::Quanpin | SchemeType::Shuangpin)
            && LITERAL_SYMBOLS.as_bytes().contains(&value)
            && if self.local_mode == LocalInputMode::None {
                !self.raw_with_cases().is_empty()
            } else {
                self.local_preedit.len() == 1
                    && self.local_preedit.as_bytes()[0].is_ascii_uppercase()
                    && !self
                        .local_mode
                        .spelling_symbols()
                        .as_bytes()
                        .contains(&value)
            }
    }

    pub(super) fn literal_spelling_symbols(&self) -> impl Iterator<Item = char> + '_ {
        LITERAL_SYMBOLS
            .bytes()
            .filter(|&value| self.takes_literal_symbol(value))
            .map(char::from)
    }

    pub(super) fn insert_literal_symbol(&mut self, value: u8) -> KeyResult {
        let mut text = self.editing_text();
        let caret = self.caret_position();
        text.insert(caret, char::from(value));
        if self.local_mode != LocalInputMode::None {
            self.reset_composition();
        }
        self.replace_editing_text(&text, caret + 1)
    }

    fn has_literal_symbols(&self) -> bool {
        self.raw_with_cases()
            .bytes()
            .any(|value| LITERAL_SYMBOLS.as_bytes().contains(&value))
    }

    /// 含特殊符号时逐字编辑；否则大写逐字、小写仍按原方案的音节导航。
    pub(super) fn literal_raw_boundaries(&self) -> Vec<usize> {
        let raw = self.raw_with_cases();
        if self.has_literal_symbols() {
            return (0..=raw.len()).collect();
        }
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

    pub(super) fn has_literal_input(&self) -> bool {
        if self.dedicated_english
            || self.local_mode != LocalInputMode::None
            || !matches!(self.scheme(), SchemeType::Quanpin | SchemeType::Shuangpin)
        {
            return false;
        }
        let raw = self.raw_with_cases();
        if self.has_literal_symbols() {
            return true;
        }
        // 辅助码段（`` ` ``/`|`/大写码）在请求里已剥掉：字面大写判定看去段后的查询串，
        // 句中段的码（`b;j;X`）不再误当字面大写尾巴，末尾段（`nihcXY`）也不靠豁免。
        let probe = self.engine.request().raw_input_with_cases.as_str();
        let probe = if probe.is_empty() { raw } else { probe };
        let Some(first) = probe.bytes().position(|byte| byte.is_ascii_uppercase()) else {
            return false;
        };
        // 末尾辅助码只能接在没有字面大写的拼音之后，全大写词不能被误判为「拼音加双辅助码」。
        let trailing_helpcode =
            first > 0 && first >= probe.len().saturating_sub(2) && self.has_active_helpcode();
        !trailing_helpcode
            && !(self.is_shuangpin()
                && self.engine.request().enable_shuangpin_helpcode
                && self
                    .helpcode_keymap
                    .as_ref()
                    .is_some_and(|keymap| !keymap.is_empty())
                && self
                    .engine
                    .request()
                    .shuangpin_helpcode_modes
                    .direct_helpcode
                && crate::shuangpin::engine::direct_helpcodes_own_capitals(
                    probe,
                    self.profile,
                    self.engine
                        .request()
                        .shuangpin_helpcode_modes
                        .direct_helpcode_slash,
                    self.engine
                        .request()
                        .shuangpin_helpcode_modes
                        .direct_helpcode_uppercase,
                ))
    }

    /// 含特殊符号时只提供原文；否则只转换第一个小写片段，大写及后续片段保留原文边界。
    pub(super) fn update_literal_candidates(&mut self) -> bool {
        if !self.has_literal_input() {
            return false;
        }
        let raw = self.raw_with_cases().to_owned();
        let end = self.caret_position().min(raw.len());
        let prefix = &raw[..end];
        let start = if self.has_literal_symbols() {
            None
        } else {
            prefix.bytes().position(|byte| byte.is_ascii_lowercase())
        };
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
            && self.mixed_candidates_enabled
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
        (self.has_literal_input()
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
