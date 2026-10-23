//! `ShuangpinEngine` (schemes-lang.md §1.6): double and single helpcode queries, the normal series query, and the fuzzy tail borrowed from quanpin.

use std::borrow::Cow;
use std::collections::HashSet;

use super::dictionary::ShuangpinDictionary;
use super::query::{
    detect_active_double_helpcode_length, fuzzy_initial_spellings, is_complete_input,
    raw_length_for_effective_prefix, remove_manual_delimiters, segment_input,
    to_quanpin_segmentation, trim_trailing_letters_preserve_delimiters,
};
use super::utils::{cvt_single_sp_to_pinyin, get_full_help_codes, is_all_complete_pinyin};
use super::ShuangpinProfile;
use crate::helpcode::{
    filter_candidates_with_double_helpcodes, reorder_candidates_with_single_helpcode,
    HelpcodeKeymap,
};
use crate::paths::RuntimePaths;
use crate::quanpin::QuanpinDictionary;
use crate::types::{
    autocorrect_type, CandidateSource, FuzzyPinyinOptions, QueryRequest, SchemeType, WordItem,
};

// 双拼模糊候选的短合并直接扫描已有词，避免临时哈希表和重复索引分配。
const SMALL_FUZZY_DEDUP: usize = 64;

/// The base the dictionary queries and the codes that filter or reorder it.
struct HelpcodeQuery {
    base_pure: String,
    base_segmentation: String,
    help_codes: String,
}

#[derive(Clone, Debug)]
struct DirectSpelling {
    code: [u8; 2],
    first: Option<char>,
    second: Option<char>,
    end: usize,
}

fn direct_helpcode_paths(typed: &str, profile: &ShuangpinProfile) -> Vec<Vec<DirectSpelling>> {
    fn visit(
        typed: &str,
        position: usize,
        profile: &ShuangpinProfile,
        path: &mut Vec<DirectSpelling>,
        paths: &mut Vec<Vec<DirectSpelling>>,
        remaining: &mut usize,
    ) {
        if paths.len() >= 64 || *remaining == 0 {
            return;
        }
        *remaining -= 1;
        if position == typed.len() {
            if path.iter().any(|spelling| spelling.first.is_some()) {
                paths.push(path.clone());
            }
            return;
        }
        if position + 2 > typed.len() {
            return;
        }
        let code = [
            typed.as_bytes()[position].to_ascii_lowercase(),
            typed.as_bytes()[position + 1].to_ascii_lowercase(),
        ];
        let code_text = std::str::from_utf8(&code).expect("直接辅助码编码只含 ASCII");
        if cvt_single_sp_to_pinyin(code_text, profile).is_empty() {
            return;
        }
        path.push(DirectSpelling {
            code,
            first: None,
            second: None,
            end: position + 2,
        });
        visit(typed, position + 2, profile, path, paths, remaining);
        path.pop();

        if position + 3 <= typed.len() {
            let first = typed.as_bytes()[position + 2] as char;
            if first.is_ascii_lowercase() {
                path.push(DirectSpelling {
                    code,
                    first: Some(first.to_ascii_lowercase()),
                    second: None,
                    end: position + 3,
                });
                visit(typed, position + 3, profile, path, paths, remaining);
                path.pop();
            }
        }
        if position + 4 <= typed.len() {
            let first = typed.as_bytes()[position + 2] as char;
            let second = typed.as_bytes()[position + 3] as char;
            if first.is_ascii_lowercase() && second.is_ascii_uppercase() {
                path.push(DirectSpelling {
                    code,
                    first: Some(first.to_ascii_lowercase()),
                    second: Some(second.to_ascii_lowercase()),
                    end: position + 4,
                });
                visit(typed, position + 4, profile, path, paths, remaining);
                path.pop();
            }
        }
    }

    let mut paths = Vec::new();
    if typed.is_ascii() {
        // 同时限制搜索节点，避免大量无解分支绕过结果数量上限。
        visit(typed, 0, profile, &mut Vec::new(), &mut paths, &mut 512);
    }
    paths
}

/// 音节内双辅助码的第二码可以大写，但音节本身的大写仍属于字面英文。
pub(crate) fn direct_helpcodes_own_capitals(typed: &str, profile: &ShuangpinProfile) -> bool {
    direct_helpcode_paths(typed, profile).iter().any(|path| {
        let mut start = 0;
        path.iter().all(|spelling| {
            let lowercase_code = typed.as_bytes()[start..start + 2]
                .iter()
                .all(u8::is_ascii_lowercase);
            start = spelling.end;
            lowercase_code
        })
    })
}

fn direct_helpcode_matches(word: &str, path: &[DirectSpelling], keymap: &HelpcodeKeymap) -> bool {
    let mut characters = word.chars();
    let mut next_index = 0;
    for (index, spelling) in path.iter().enumerate() {
        let Some(first) = spelling.first else {
            continue;
        };
        let Some(character) = characters.nth(index - next_index) else {
            return false;
        };
        next_index = index + 1;
        let mut encoded = [0; 4];
        let character = character.encode_utf8(&mut encoded);
        let Some(code) = keymap.code(character) else {
            return false;
        };
        if code.as_bytes().first().copied() != Some(first as u8) {
            return false;
        }
        if let Some(second) = spelling.second {
            if code.as_bytes().get(1).copied() != Some(second as u8) {
                return false;
            }
        }
    }
    true
}

/// engine.cpp:59-74.
fn full_helpcode_query(
    raw: &str,
    raw_with_cases: &str,
    profile: &ShuangpinProfile,
) -> Option<HelpcodeQuery> {
    if detect_active_double_helpcode_length(raw, raw_with_cases, profile) != 2 {
        return None;
    }
    let base_raw = trim_trailing_letters_preserve_delimiters(raw, 2);
    Some(HelpcodeQuery {
        base_pure: remove_manual_delimiters(&base_raw),
        base_segmentation: segment_input(&base_raw, profile),
        help_codes: get_full_help_codes(&remove_manual_delimiters(raw_with_cases)),
    })
}

/// engine.cpp:76-109.
fn single_helpcode_query(
    raw: &str,
    pure_with_cases: &str,
    profile: &ShuangpinProfile,
) -> Option<HelpcodeQuery> {
    let length = pure_with_cases.len();
    if length <= 1 || length.is_multiple_of(2) {
        return None;
    }
    // A `'` right before the last letter makes that letter a user-delimited pinyin segment, not a helpcode; the composition layer reads `ui'u` the same way.
    let raw_prefix = raw_length_for_effective_prefix(raw, length - 1);
    if raw.as_bytes().get(raw_prefix) == Some(&b'\'') {
        return None;
    }
    let base_raw = trim_trailing_letters_preserve_delimiters(raw, 1);
    let base_pure = remove_manual_delimiters(&base_raw);
    let base_segmentation = segment_input(&base_raw, profile);
    if raw.contains('\'') && !base_segmentation.split('\'').all(|part| part.len() == 2) {
        return None;
    }
    if !is_all_complete_pinyin(&base_pure, &base_segmentation) {
        return None;
    }
    Some(HelpcodeQuery {
        base_pure,
        base_segmentation,
        // The case is kept: uppercase means "prefer the last character".
        help_codes: pure_with_cases[length - 1..].to_string(),
    })
}

/// The active helpcode reading of a request, double first; `None` reads the input as plain pinyin (`cls` is `c'ls`, engine.cpp:162).
fn active_helpcode_query(
    request: &QueryRequest,
    profile: &ShuangpinProfile,
) -> Option<HelpcodeQuery> {
    if !request.enable_shuangpin_helpcode || !request.shuangpin_helpcode_modes.direct {
        return None;
    }
    let raw = &request.raw_input;
    let raw_with_cases = if request.raw_input_with_cases.is_empty() {
        raw
    } else {
        &request.raw_input_with_cases
    };
    full_helpcode_query(raw, raw_with_cases, profile).or_else(|| {
        let pure_with_cases = if raw_with_cases.contains('\'') {
            Cow::Owned(remove_manual_delimiters(raw_with_cases))
        } else {
            Cow::Borrowed(raw_with_cases.as_str())
        };
        single_helpcode_query(raw, &pure_with_cases, profile)
    })
}

pub struct ShuangpinEngine {
    profile: &'static ShuangpinProfile,
    dictionary: ShuangpinDictionary,
    paths: RuntimePaths,
    /// Opened on the first fuzzy query only.
    fuzzy_dictionary: Option<QuanpinDictionary>,
}

impl ShuangpinEngine {
    pub fn new(profile: &'static ShuangpinProfile, paths: &RuntimePaths) -> Self {
        Self {
            profile,
            dictionary: ShuangpinDictionary::new(profile, paths),
            paths: paths.clone(),
            fuzzy_dictionary: None,
        }
    }

    /// :122-167.
    pub fn query(
        &mut self,
        request: &QueryRequest,
        keymap: Option<&HelpcodeKeymap>,
    ) -> Vec<WordItem> {
        let mut candidates = self.query_standard(request, keymap);
        if !request.valid {
            return candidates;
        }
        let alternatives =
            fuzzy_initial_spellings(&request.raw_input, self.profile, request.fuzzy_pinyin, 32);
        if alternatives.is_empty() {
            return candidates;
        }
        let mut single_helpcode = None;
        for spelling in alternatives {
            let mut alternate = request.clone();
            // 替代的声母保持原位置的大小写，辅助码与手动分隔符仍交给原查询识别。
            alternate.raw_input_with_cases = spelling
                .bytes()
                .zip(request.raw_input_with_cases.bytes())
                .map(|(new, old)| {
                    char::from(if old.is_ascii_uppercase() {
                        new.to_ascii_uppercase()
                    } else {
                        new
                    })
                })
                .collect();
            alternate.raw_input = spelling;
            // 原编码非法时识别不到末尾辅助码，采用合法替代读法识别出的同一末码。
            if single_helpcode.is_none() {
                single_helpcode = active_helpcode_query(&alternate, self.profile)
                    .map(|query| query.help_codes)
                    .filter(|code| code.len() == 1);
            }
            let mut rows = self.query_standard(&alternate, keymap);
            rows.retain_mut(|row| {
                let mut original = request.raw_input.bytes().filter(|&byte| byte != b'\'');
                let code: Option<String> = row
                    .pinyin
                    .bytes()
                    .map(|byte| {
                        if byte == b'\'' {
                            Some('\'')
                        } else {
                            original.next().map(char::from)
                        }
                    })
                    .collect();
                let Some(code) = code else {
                    return false;
                };
                row.pinyin = code;
                row.fuzzy = true;
                true
            });
            append_fuzzy_rows(&mut candidates, rows);
        }
        sort_by_coverage(&mut candidates);
        // 不同替代路径的词库行共用权重，随后恢复辅助码匹配优先级。
        sort_dictionary_weights(&mut candidates);
        if let (Some(helpcode), Some(keymap)) = (single_helpcode, keymap) {
            candidates = reorder_candidates_with_single_helpcode(candidates, &helpcode, keymap);
            sort_by_coverage(&mut candidates);
        }
        keep_typo_sentence_off_first_seat(&mut candidates);
        candidates
    }

    fn query_standard(
        &mut self,
        request: &QueryRequest,
        keymap: Option<&HelpcodeKeymap>,
    ) -> Vec<WordItem> {
        if !request.valid {
            return Vec::new();
        }
        self.dictionary
            .set_sentence_alternatives(request.sentence_alternatives);
        self.dictionary
            .set_sentence_association(request.sentence_association.clone());
        self.dictionary
            .set_rescoring_context(&request.rescoring_context);
        self.dictionary
            .set_autocorrect_types(request_autocorrect_types(request));

        let raw = &request.raw_input;
        if raw.bytes().all(|byte| byte == b'\'') {
            return Vec::new();
        }
        // Without a loaded table nothing matches a helpcode, which is what the reference's null keymap did.
        let empty_keymap = HelpcodeKeymap::default();
        let keymap = keymap.unwrap_or(&empty_keymap);

        // 移动端在直接辅助码后会继续输入：`uibjm` 是 `ui` 加首码 `b` 再加 `jm`，
        // `tartantab` 则在三个音节上各带一位辅助码。普通的末尾辅助码路径只会看到最后一个奇数字母，
        // 所以把这类编码解析成逐音节约束；与完整小写双拼有歧义时仍保留普通读法。
        let typed = if request.raw_input_with_cases.is_empty() {
            raw.as_str()
        } else {
            request.raw_input_with_cases.as_str()
        };
        let mut ambiguous_direct = None;
        if request.enable_shuangpin_helpcode && !keymap.is_empty() {
            let mut direct_best: Option<(usize, Vec<WordItem>)> = None;
            for path in direct_helpcode_paths(typed, self.profile) {
                if typed.len() < 4 && path.len() == 1 {
                    continue;
                }
                let mut pure = String::with_capacity(path.len() * 2);
                for spelling in &path {
                    pure.push(spelling.code[0] as char);
                    pure.push(spelling.code[1] as char);
                }
                let segmentation = segment_input(&pure, self.profile);
                let mut candidates = self.dictionary.generate_series(&pure, &segmentation, &pure);
                candidates
                    .retain(|candidate| direct_helpcode_matches(&candidate.word, &path, keymap));
                if path.len() >= 2
                    && !candidates.iter().any(|candidate| {
                        candidate.canonical_pinyin.split('\'').count() == path.len()
                    })
                {
                    let sentences = self.dictionary.generate_constrained_sentences(
                        &segmentation,
                        raw,
                        |start, word| {
                            let end = start + word.chars().count();
                            path.get(start..end)
                                .is_some_and(|span| direct_helpcode_matches(word, span, keymap))
                        },
                    );
                    candidates.splice(0..0, sentences);
                }
                // 选词按原始输入消耗编码，不能留下解析时剥掉的辅助码。
                for candidate in &mut candidates {
                    let syllables = candidate.canonical_pinyin.split('\'').count();
                    if let Some(spelling) = path.get(syllables - 1) {
                        candidate.pinyin = raw[..spelling.end].to_string();
                    }
                }
                let constrained = path
                    .iter()
                    .filter(|spelling| spelling.first.is_some())
                    .count();
                if candidates.is_empty() {
                    continue;
                }
                let score = constrained * 2 + candidates[0].word.chars().count().min(path.len());
                if direct_best.as_ref().is_none_or(|(best, _)| score > *best) {
                    direct_best = Some((score, candidates));
                }
            }
            if let Some((_, candidates)) = direct_best {
                if typed.bytes().all(|byte| byte.is_ascii_lowercase())
                    && is_complete_input(raw, self.profile)
                {
                    ambiguous_direct = Some(candidates);
                } else {
                    return candidates;
                }
            }
        }

        let mut candidates = if let Some(helpcode) = active_helpcode_query(request, self.profile) {
            let exact = self.dictionary.generate_with_helpcodes(
                &helpcode.base_pure,
                &helpcode.base_segmentation,
                raw,
                &helpcode.help_codes,
                keymap,
            );
            self.append_fuzzy(
                exact,
                &helpcode.base_segmentation,
                request.fuzzy_pinyin,
                &helpcode.help_codes,
                keymap,
            )
        } else {
            let segmentation = segment_input(raw, self.profile);
            let exact =
                self.dictionary
                    .generate_series(&remove_manual_delimiters(raw), &segmentation, raw);
            self.append_fuzzy(exact, &segmentation, request.fuzzy_pinyin, "", keymap)
        };
        if let Some(direct) = ambiguous_direct {
            // 辅助码可以与普通两键音节重叠；完整读法（含模糊音和整句）先于歧义约束。
            let complete = |candidate: &WordItem| {
                candidate.corrected_from.is_empty()
                    && candidate
                        .pinyin
                        .bytes()
                        .filter(|&byte| byte != b'\'')
                        .count()
                        == raw.len()
                    && candidate.canonical_pinyin.split('\'').count() == raw.len() / 2
            };
            let whole_count = candidates
                .iter()
                .filter(|candidate| complete(candidate))
                .count();
            if whole_count == 0 {
                return direct;
            }
            candidates.sort_by_key(|candidate| !complete(candidate));
            let prefixes = candidates.split_off(whole_count);
            append_fuzzy_rows(&mut candidates, direct);
            append_fuzzy_rows(&mut candidates, prefixes);
        }
        keep_typo_sentence_off_first_seat(&mut candidates);
        candidates
    }

    /// 模糊候选沿用覆盖的双拼按键，按词面去重合并；同覆盖词库行共用调频权重，辅助码匹配仍优先。
    fn append_fuzzy(
        &mut self,
        mut exact: Vec<WordItem>,
        raw_segmentation: &str,
        options: FuzzyPinyinOptions,
        help_codes: &str,
        keymap: &HelpcodeKeymap,
    ) -> Vec<WordItem> {
        if options.rules == 0 {
            return exact;
        }
        let paths = &self.paths;
        let fuzzy_dictionary = self
            .fuzzy_dictionary
            .get_or_insert_with(|| QuanpinDictionary::new(paths));
        let mut fuzzy = fuzzy_dictionary.fuzzy_candidates(
            &to_quanpin_segmentation(raw_segmentation, self.profile),
            options,
        );
        relabel_fuzzy_rows(&mut fuzzy, raw_segmentation);
        let fuzzy = match help_codes.len() {
            2 => filter_candidates_with_double_helpcodes(fuzzy, help_codes, keymap),
            1 => reorder_candidates_with_single_helpcode(fuzzy, help_codes, keymap),
            _ => fuzzy,
        };
        if fuzzy.is_empty() {
            sort_by_coverage(&mut exact);
            return exact;
        }
        append_fuzzy_rows(&mut exact, fuzzy);
        sort_by_coverage(&mut exact);
        sort_dictionary_weights(&mut exact);
        if help_codes.len() == 1 {
            exact = reorder_candidates_with_single_helpcode(exact, help_codes, keymap);
            sort_by_coverage(&mut exact);
        }
        exact
    }

    /// Only when the first segment is one letter (:169-185).
    pub fn expand_initial_candidates(
        &mut self,
        request: &QueryRequest,
        candidates: &mut Vec<WordItem>,
    ) -> bool {
        if request.scheme != SchemeType::Shuangpin {
            return false;
        }
        let segmentation = segment_input(&request.raw_input, self.profile);
        let initial = segmentation.split('\'').next().unwrap_or_default();
        if initial.len() != 1 {
            return false;
        }
        self.dictionary
            .expand_initial_candidates(initial, candidates, &request.raw_input)
    }

    /// Routes online rows to the cache the request's own query read: the double-helpcode entry for its codes, the single-helpcode entries, or the series entry (pinyin_candidate_provider.cpp:186-210). The provider re-derived the mode with looser rules than the query and, for a batch, dropped the double codes, so a batch could land in an entry no query reads; this uses the query's decision (overlays.md §5.1).
    pub fn insert_online_words(
        &mut self,
        request: &QueryRequest,
        words: &[String],
        source: CandidateSource,
    ) -> bool {
        if !request.valid {
            return false;
        }
        // With a reranker loaded the context is part of the cache key, so the rows must go under the context this request was queried with.
        self.dictionary
            .set_rescoring_context(&request.rescoring_context);
        match active_helpcode_query(request, self.profile) {
            Some(helpcode) => {
                let double = if helpcode.help_codes.len() == 2 {
                    helpcode.help_codes.as_str()
                } else {
                    ""
                };
                self.dictionary.insert_word_to_active_helpcode_cache(
                    &request.raw_input,
                    words,
                    source,
                    double,
                )
            }
            None => self
                .dictionary
                .insert_word_to_series_cache(&request.raw_input, words, source),
        }
    }

    pub fn clear_online_candidates(&mut self, source: CandidateSource) {
        self.dictionary.clear_online_candidates(source);
    }

    pub fn find_candidate(&self, key: &str, value: &str) -> Option<WordItem> {
        self.dictionary.find_candidate(key, value)
    }

    pub fn reset_cache(&mut self) {
        self.dictionary.reset_cache();
        if let Some(fuzzy_dictionary) = &mut self.fuzzy_dictionary {
            fuzzy_dictionary.reset_cache();
        }
    }
}

/// 会话只有一个「拼音纠错」开关，按当前方案作用：请求里那两个字段名带 quanpin 是历史原因，双拼也读它们。双拼不加漏键和多键，见 `typo_edges::SHUANGPIN_TYPO_TYPES`。
fn request_autocorrect_types(request: &QueryRequest) -> u32 {
    let transposition = if request.enable_quanpin_autocorrect_transposition {
        autocorrect_type::TRANSPOSITION
    } else {
        0
    };
    let neighbor = if request.enable_quanpin_autocorrect_neighbor {
        autocorrect_type::NEIGHBOR
    } else {
        0
    };
    transposition | neighbor
}

/// 展示用音节分隔符不增加实际按键覆盖。
fn coverage(item: &WordItem) -> usize {
    item.pinyin.bytes().filter(|&byte| byte != b'\'').count()
}

/// 按覆盖的按键长度稳定排序。纠错整句覆盖全部按键，可能因此被排到第一位，由 `keep_typo_sentence_off_first_seat` 在 `query` 返回前挪开。
fn sort_by_coverage(rows: &mut [WordItem]) {
    rows.sort_by_key(|item| std::cmp::Reverse(coverage(item)));
    let Some(first) = rows.first() else {
        return;
    };
    if !first.sentence_association {
        return;
    }
    let full = coverage(first);
    let bucket = rows
        .iter()
        .take_while(|item| coverage(item) == full)
        .count();
    if rows[..bucket]
        .iter()
        .any(|item| !item.fuzzy && item.source.is_dictionary())
    {
        return;
    }
    // 没有字面整词时，模糊整词仍优先于替补整句；顺序不再依靠分隔符的虚假长度。
    rows[..bucket].sort_by_key(|item| !(item.fuzzy && item.source.is_dictionary()));
}

/// 同一覆盖内的词库行共用调频权重；整句分数不参与比较，并留在原来的位置。
fn sort_dictionary_weights(rows: &mut [WordItem]) {
    let mut start = 0;
    while start < rows.len() {
        let count = coverage(&rows[start]);
        let end = start
            + rows[start..]
                .iter()
                .take_while(|row| coverage(row) == count)
                .count();
        let bucket = &mut rows[start..end];
        let mut previous = i64::MAX;
        let unsorted = bucket
            .iter()
            .filter(|row| row.source.is_dictionary())
            .any(|row| {
                let inverted = row.weight > previous;
                previous = row.weight;
                inverted
            });
        if unsorted {
            let (positions, mut dictionary): (Vec<_>, Vec<_>) = bucket
                .iter_mut()
                .enumerate()
                .filter(|(_, row)| row.source.is_dictionary())
                .map(|(index, row)| (index, std::mem::take(row)))
                .unzip();
            dictionary.sort_by_key(|row| std::cmp::Reverse(row.weight));
            for (index, row) in positions.into_iter().zip(dictionary) {
                bucket[index] = row;
            }
        }
        start = end;
    }
}

/// 纠错整句不抢首选（见 `dictionary::typo_sentence_seat`）。词典放好座位之后还有几道重排会把它挪到第一位：模糊音按覆盖长度排序（整句块为空、全码又没有词条时它是唯一最长的一行），单辅助码把匹配的行提到最前，双辅助码滤掉排在它前面的行。所以不在每道重排里各自守，而在 `query` 返回前统一处理：首位是纠错行时，把第一条非纠错行提到首位，其余行相对顺序不变。整张表只剩纠错行（例如双辅助码只留下了它）时没有可让的位置，和 `typo_sentence_seat` 在表空时不给纠错行一样，整张表清空，免得空格直接上屏一条改正。
fn keep_typo_sentence_off_first_seat(rows: &mut Vec<WordItem>) {
    if rows.first().is_none_or(|row| row.corrected_from.is_empty()) {
        return;
    }
    match rows.iter().position(|row| row.corrected_from.is_empty()) {
        Some(literal) => rows[..=literal].rotate_right(1),
        None => rows.clear(),
    }
}

/// 只需段数时直接统计分隔符，避免为每个模糊候选复制音节字符串。
fn segment_count(segmentation: &str) -> usize {
    if segmentation.is_empty() {
        0
    } else {
        segmentation.bytes().filter(|&byte| byte == b'\'').count() + 1
    }
}

// 直接借用输入前缀，保留空段；候选编码容量足够时不创建临时字符串。
fn relabel_fuzzy_rows(rows: &mut [WordItem], raw_segmentation: &str) {
    for item in rows {
        if let Some(prefix) = segment_prefix(raw_segmentation, segment_count(&item.pinyin)) {
            item.pinyin.clear();
            item.pinyin.push_str(prefix);
        }
    }
}

fn segment_prefix(segmentation: &str, count: usize) -> Option<&str> {
    if count == 0 {
        return Some("");
    }
    if segmentation.is_empty() {
        return None;
    }
    let mut segments = segmentation.split('\'');
    let mut end = segments.next()?.len();
    for _ in 1..count {
        end += 1 + segments.next()?.len();
    }
    Some(&segmentation[..end])
}

fn append_fuzzy_rows(exact: &mut Vec<WordItem>, fuzzy: Vec<WordItem>) {
    if exact.len().saturating_add(fuzzy.len()) <= SMALL_FUZZY_DEDUP {
        exact.reserve(fuzzy.len());
        for item in fuzzy {
            if exact.iter().any(|existing| existing.word == item.word) {
                continue;
            }
            exact.push(item);
        }
        return;
    }
    // 借用词面计算重复项，释放集合后再移动整行，避免移动时仍持有借用。
    let mut seen: HashSet<&str> = exact.iter().map(|item| item.word.as_str()).collect();
    let duplicates = fuzzy
        .iter()
        .enumerate()
        .filter_map(|(index, item)| (!seen.insert(item.word.as_str())).then_some(index))
        .collect::<Vec<_>>();
    drop(seen);
    let unique_count = fuzzy.len() - duplicates.len();
    exact.reserve(unique_count);
    let mut duplicates = duplicates.into_iter().peekable();
    exact.extend(fuzzy.into_iter().enumerate().filter_map(|(index, item)| {
        if duplicates.peek() == Some(&index) {
            duplicates.next();
            None
        } else {
            Some(item)
        }
    }));
}

#[cfg(test)]
mod tests {
    #[test]
    fn direct_helpcode_search_handles_invalid_and_long_input() {
        let profile =
            crate::shuangpin::profile::profile(crate::types::ShuangpinProfileKind::Ziranma)
                .unwrap();
        assert!(super::direct_helpcode_paths("uib字", profile).is_empty());
        assert!(super::direct_helpcode_paths("ui'bjm", profile).is_empty());
        let ambiguous = "ta".repeat(80);
        assert!(super::direct_helpcode_paths(&ambiguous, profile).len() <= 64);
    }

    #[test]
    fn direct_helpcode_matching_reuses_character_storage() {
        use std::collections::HashMap;

        let keymap = crate::helpcode::HelpcodeKeymap::from_codes(HashMap::from([
            ("你".to_owned(), "a".to_owned()),
            ("好".to_owned(), "b".to_owned()),
        ]));
        let path = [super::DirectSpelling {
            code: *b"ni",
            first: Some('a'),
            second: None,
            end: 3,
        }];

        let (matched, allocations) = crate::ime::personal_rerank::allocations::count(|| {
            super::direct_helpcode_matches("你好", &path, &keymap)
        });

        assert!(matched);
        assert_eq!(allocations, 0);
    }

    #[test]
    fn direct_helpcode_paths_do_not_allocate_code_strings() {
        let profile =
            crate::shuangpin::profile::profile(crate::types::ShuangpinProfileKind::Ziranma)
                .unwrap();

        let (paths, allocations) = crate::ime::personal_rerank::allocations::count(|| {
            super::direct_helpcode_paths("tartantab", profile)
        });

        assert!(!paths.is_empty());
        assert!(
            allocations <= 13,
            "路径生成不应为每个编码单元申请独立字符串：{allocations}"
        );
    }

    use super::{
        append_fuzzy_rows, keep_typo_sentence_off_first_seat, relabel_fuzzy_rows, segment_count,
        sort_by_coverage, sort_dictionary_weights,
    };
    use crate::types::{CandidateSource, WordItem};

    fn row(word: &str) -> WordItem {
        WordItem::new("ni", word, 1, CandidateSource::Database, "ni")
    }

    #[test]
    fn fuzzy_dictionary_weights_keep_sentence_scores_and_prefix_slots_separate() {
        let literal = WordItem::new("nihc", "你好", 100, CandidateSource::Database, "ni'hao");
        let mut fuzzy = WordItem::new("ni'hc", "利好", 500, CandidateSource::Database, "li'hao");
        fuzzy.fuzzy = true;
        let sentence = WordItem::new("nihc", "拟好", 10000, CandidateSource::Generated, "ni'hao");
        let prefix = WordItem::new("ni", "你", 100000, CandidateSource::Database, "ni");
        let mut rows = vec![
            literal.clone(),
            sentence.clone(),
            fuzzy.clone(),
            prefix.clone(),
        ];
        sort_by_coverage(&mut rows);
        sort_dictionary_weights(&mut rows);
        assert_eq!(rows, [fuzzy, sentence, literal, prefix]);
    }

    #[test]
    fn short_ordinary_inputs_do_not_allocate_helpcode_query_state() {
        let profile =
            crate::shuangpin::profile::profile(crate::types::ShuangpinProfileKind::Xiaohe).unwrap();
        for raw in ["n", "ni", "nihc", "nihcab"] {
            for cased in [raw, ""] {
                let request = crate::types::QueryRequest {
                    raw_input: raw.into(),
                    raw_input_with_cases: cased.into(),
                    enable_shuangpin_helpcode: true,
                    ..Default::default()
                };
                let (query, allocations) = crate::ime::personal_rerank::allocations::count(|| {
                    super::active_helpcode_query(&request, profile)
                });
                assert!(query.is_none());
                assert_eq!(allocations, 0, "raw={raw:?}, cased={cased:?}");
            }
        }
    }

    #[test]
    fn borrowed_helpcode_queries_match_the_owned_input_reference() {
        use crate::types::{QueryRequest, ShuangpinProfileKind};
        let fields = |query: Option<super::HelpcodeQuery>| {
            query.map(|query| (query.base_pure, query.base_segmentation, query.help_codes))
        };
        for kind in [
            ShuangpinProfileKind::Xiaohe,
            ShuangpinProfileKind::Ziranma,
            ShuangpinProfileKind::Shoudao,
            ShuangpinProfileKind::Microsoft,
            ShuangpinProfileKind::SmartAbc,
        ] {
            let profile = crate::shuangpin::profile::profile(kind).unwrap();
            for typed in [
                "", "n", "ni", "niu", "niU", "ui'u", "niu'", "'niu", "ni''u", "ni'hcU", "nihcAB",
                "nihcAb", "nihcaB", "ni'hcAB'", "ni'hc'AB", "j;gkA", "''",
            ] {
                for keep_cases in [false, true] {
                    for enabled in [false, true] {
                        let request = QueryRequest {
                            raw_input: typed.to_ascii_lowercase(),
                            raw_input_with_cases: if keep_cases {
                                typed.into()
                            } else {
                                String::new()
                            },
                            enable_shuangpin_helpcode: enabled,
                            ..Default::default()
                        };
                        let expected = if !enabled {
                            None
                        } else {
                            let raw = &request.raw_input;
                            let cased = if request.raw_input_with_cases.is_empty() {
                                raw
                            } else {
                                &request.raw_input_with_cases
                            };
                            super::full_helpcode_query(raw, cased, profile).or_else(|| {
                                super::single_helpcode_query(
                                    raw,
                                    &super::remove_manual_delimiters(cased),
                                    profile,
                                )
                            })
                        };
                        assert_eq!(fields(super::active_helpcode_query(&request, profile)), fields(expected),
                            "kind={kind:?}, typed={typed:?}, keep_cases={keep_cases}, enabled={enabled}");
                    }
                }
            }
        }
    }

    #[test]
    fn fuzzy_query_reuses_the_decoded_row_code_capacity() {
        let root = tempfile::tempdir().unwrap();
        let paths = crate::paths::RuntimePaths {
            resources: root.path().into(),
            user_data: root.path().into(),
            cache: root.path().into(),
            dictionaries: root.path().into(),
        };
        rusqlite::Connection::open(paths.dictionary(crate::assets::MAIN_DICTIONARY))
            .unwrap()
            .execute_batch("CREATE TABLE tbl_1_z(key TEXT, jp TEXT, value TEXT, weight INTEGER); INSERT INTO tbl_1_z VALUES('zi','z','模糊',100);")
            .unwrap();
        let mut engine = super::ShuangpinEngine::new(
            crate::shuangpin::profile::profile(crate::types::ShuangpinProfileKind::Xiaohe).unwrap(),
            &paths,
        );
        let rows = engine.append_fuzzy(
            Vec::new(),
            "vi",
            crate::types::FuzzyPinyinOptions {
                rules: crate::types::fuzzy_rule::Z_ZH,
            },
            "",
            &crate::helpcode::HelpcodeKeymap::default(),
        );

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].pinyin, "vi");
        assert_eq!(rows[0].canonical_pinyin, "zi");
        assert!(rows[0].pinyin.capacity() >= "zhi".len());
    }

    #[test]
    fn fuzzy_relabeling_reuses_capacity_without_allocating() {
        let mut rows = vec![row("首"), row("次"), row("空")];
        rows[0].pinyin = "zhi'hao'jie".into();
        rows[1].pinyin = "zhi'hao".into();
        rows[2].pinyin.clear();
        let pointers = rows
            .iter()
            .map(|row| row.pinyin.as_ptr())
            .collect::<Vec<_>>();
        let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
            relabel_fuzzy_rows(&mut rows, "zi'hc'jx");
        });
        assert_eq!(allocations, 0);
        assert_eq!(
            rows.iter()
                .map(|row| row.pinyin.as_str())
                .collect::<Vec<_>>(),
            ["zi'hc'jx", "zi'hc", ""]
        );
        assert_eq!(
            rows.iter()
                .map(|row| row.pinyin.as_ptr())
                .collect::<Vec<_>>(),
            pointers
        );
        let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
            relabel_fuzzy_rows(&mut [], "zi'hc'jx");
        });
        assert_eq!(allocations, 0);
    }

    #[test]
    fn fuzzy_relabeling_matches_owned_segment_reference() {
        use crate::pinyin::segment::{join_segments, split_segments};
        let long = std::iter::repeat_n("长", 130).collect::<Vec<_>>().join("'");
        for raw in [
            "",
            "zi",
            "'",
            "''",
            "'zi",
            "zi'",
            "zi''hc'",
            "你'好'🙂",
            &long,
        ] {
            let typed = split_segments(raw);
            let mut rows = (0..=typed.len() + 1)
                .map(|count| {
                    let mut item = WordItem::new(
                        std::iter::repeat_n("a", count)
                            .collect::<Vec<_>>()
                            .join("'"),
                        format!("合成{count}"),
                        count as i64,
                        CandidateSource::Generated,
                        "yuan'ci",
                    );
                    item.fuzzy = true;
                    item.fixed_position = 2;
                    item.corrected_from = "synthetic".into();
                    item.sentence_association = true;
                    item.sentence_words = vec!["合成".into(), count.to_string()];
                    item
                })
                .collect::<Vec<_>>();
            let mut expected = rows.clone();
            for item in &mut expected {
                let count = split_segments(&item.pinyin).len();
                if count <= typed.len() {
                    item.pinyin = join_segments(&typed[..count]);
                }
            }
            relabel_fuzzy_rows(&mut rows, raw);
            assert_eq!(rows, expected, "raw={raw:?}");
        }
    }

    #[test]
    fn segment_count_does_not_allocate_for_fuzzy_row_relabeling() {
        let (count, allocations) =
            crate::ime::personal_rerank::allocations::count(|| segment_count("ni'hao'jie"));

        assert_eq!(count, 3);
        assert_eq!(allocations, 0);
        assert_eq!(segment_count(""), 0);
        assert_eq!(segment_count("ni"), 1);
        for input in ["'", "ni''hao'", "'ni", "你'好", "a'a'a'a'a"] {
            assert_eq!(
                segment_count(input),
                crate::pinyin::segment::split_segments(input).len()
            );
        }
    }

    #[test]
    fn short_fuzzy_rows_are_appended_without_temporary_heap_state() {
        let mut exact = Vec::with_capacity(8);
        exact.push(row("你"));
        let fuzzy = vec![row("你"), row("好"), row("好"), row("吗")];
        let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
            append_fuzzy_rows(&mut exact, fuzzy);
        });

        assert_eq!(allocations, 0);
        assert_eq!(
            exact
                .iter()
                .map(|item| item.word.as_str())
                .collect::<Vec<_>>(),
            ["你", "好", "吗"]
        );
    }

    #[test]
    fn large_fuzzy_rows_keep_first_occurrence_order() {
        let mut exact = Vec::with_capacity(70);
        exact.push(row("已有"));
        let fuzzy = (0..65)
            .map(|index| row(if index == 0 { "已有" } else { "模糊" }))
            .collect();

        append_fuzzy_rows(&mut exact, fuzzy);

        assert_eq!(
            exact
                .iter()
                .map(|item| item.word.as_str())
                .collect::<Vec<_>>(),
            ["已有", "模糊"]
        );
    }

    /// 整句块为空、全码没有词条时，纠错整句是唯一覆盖全部按键的一行，按长度排序后也不能坐第一位。
    #[test]
    fn sorting_by_coverage_keeps_the_typo_sentence_off_the_first_seat() {
        let sort = |rows: &mut Vec<WordItem>| {
            sort_by_coverage(rows);
            keep_typo_sentence_off_first_seat(rows);
        };
        let prefix =
            |word: &str| WordItem::new("mwgf", word, 1, CandidateSource::Database, "mei'gen");
        let mut typo = WordItem::new(
            "mwgfxi",
            "没关系",
            1,
            CandidateSource::Generated,
            "mei'guan'xi",
        );
        typo.sentence_association = true;
        typo.corrected_from = "mwgfxi".to_owned();
        let words = |rows: &[WordItem]| {
            rows.iter()
                .map(|item| item.word.clone())
                .collect::<Vec<_>>()
        };

        let mut rows = vec![prefix("没跟"), typo.clone(), prefix("没")];
        sort(&mut rows);
        assert_eq!(words(&rows), ["没跟", "没关系", "没"]);

        // 有同样覆盖全部按键的字面行时，稳定排序本来就让它留在纠错行前面。
        let literal = WordItem::new(
            "mwgfxi",
            "没跟系",
            1,
            CandidateSource::Generated,
            "mei'gen'xi",
        );
        let mut rows = vec![literal, typo.clone(), prefix("没跟")];
        sort(&mut rows);
        assert_eq!(words(&rows), ["没跟系", "没关系", "没跟"]);

        // 只有它一行时没有可让的位置，不给纠错行。
        let mut rows = vec![typo.clone()];
        sort(&mut rows);
        assert!(rows.is_empty());

        // 前面有不止一条纠错行时，第一条非纠错行提到首位，其余行相对顺序不变。
        let mut other_typo = typo.clone();
        other_typo.word = "美关系".to_owned();
        let mut rows = vec![typo, other_typo, prefix("没跟"), prefix("没")];
        keep_typo_sentence_off_first_seat(&mut rows);
        assert_eq!(words(&rows), ["没跟", "没关系", "美关系", "没"]);
    }
}
