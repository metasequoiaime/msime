//! 笔画组合：键入的笔画与通配符，以及 `msime-stroke.db` 对它们给出的单字候选。选中任一候选都结束整个组合，没有分段，也不保留词组进度。

use std::collections::HashSet;

use super::{glyph, is_key, is_stroke, key_of_glyph, MAX_STROKES, WILDCARD};
use crate::error::Result;
use crate::language_dictionary::{LanguageDictionary, LanguageEntry};
use crate::types::{QueryRequest, SchemeKey, SchemeType};

/// 笔画码与键入完全相同（含通配匹配到同样长度）的字最多读这么多。
pub const EXACT_LIMIT: usize = 200;
/// 以键入笔画开头、笔画更多的字最多读这么多。
pub const COMPLETION_LIMIT: usize = 100;

/// 一个候选字和它在 `msime-stroke.db` 里的笔画码。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StrokeCandidate {
    pub text: String,
    pub weight: i64,
    /// 该字完整的笔画码（`entries.key`）。
    pub key: String,
}

#[derive(Debug, Clone, Default)]
pub struct StrokeScheme {
    /// `hspnz` 与 `x`，至多 `MAX_STROKES` 个。键入时 `x` 不会出现在开头；宿主编辑可能把它留在开头。
    input: String,
}

impl StrokeScheme {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn reset(&mut self) {
        self.input.clear();
    }

    /// 笔画键追加一笔；通配符只在已有笔画时追加；Backspace 删最后一笔。达到 `MAX_STROKES` 后不再追加。其他键（别的字母、大写、符号）都不改变组合，是否吞掉由会话决定。
    pub fn handle_key(&mut self, key: SchemeKey) {
        match key {
            SchemeKey::Letter(letter) if is_stroke(letter) => self.push(letter),
            SchemeKey::Letter(WILDCARD) if !self.input.is_empty() => self.push(WILDCARD),
            SchemeKey::Backspace => {
                self.input.pop();
            }
            SchemeKey::Letter(_)
            | SchemeKey::Apostrophe
            | SchemeKey::Semicolon
            | SchemeKey::Minus
            | SchemeKey::Symbol(_)
            | SchemeKey::Requery => {}
        }
    }

    fn push(&mut self, key: u8) {
        if self.input.len() < MAX_STROKES {
            self.input.push(char::from(key));
        }
    }

    /// 用宿主编辑后的文本替换组合：笔画键和通配符原样保留，笔画字形换回对应的键，其他字符丢弃，超出 `MAX_STROKES` 的部分截掉。
    pub fn set_raw_input(&mut self, raw: &str) {
        self.input.clear();
        self.input.reserve(raw.len().min(MAX_STROKES));
        for character in raw.chars() {
            let key = match u8::try_from(character) {
                Ok(byte) if is_key(byte) => Some(byte),
                _ => key_of_glyph(character),
            };
            if let Some(key) = key {
                self.push(key);
            }
        }
    }

    /// 键入的笔画字母（`hspnzx`），Enter 上屏的就是它。
    pub fn input(&self) -> &str {
        &self.input
    }

    pub fn is_empty(&self) -> bool {
        self.input.is_empty()
    }

    /// 组合显示的笔画字形，例如 `一丨＊`。
    pub fn glyphs(&self) -> String {
        self.input.bytes().filter_map(glyph).collect()
    }

    /// 会话刷新用的请求。`raw_input` 是键入的字母，也是宿主编辑的文本；`normalized_segmentation` 是快照 `reading` 带给宿主绘制的笔画字形。
    pub fn build_request(&self) -> QueryRequest {
        QueryRequest {
            scheme: SchemeType::Stroke,
            raw_input: self.input.clone(),
            raw_input_with_cases: self.input.clone(),
            normalized_input: self.input.clone(),
            raw_segmentation: self.input.clone(),
            normalized_segmentation: self.glyphs(),
            segmentation: self.input.clone(),
            valid: !self.input.is_empty(),
            ..QueryRequest::default()
        }
    }

    /// 会话显示并据以判断是否在组合的文本：笔画字形，没有组合时为空。
    pub fn preedit(&self) -> String {
        self.glyphs()
    }

    /// 组合的候选，每个字只列一次：
    /// 1. 有字频、笔画码与键入完全相同的字（通配符匹配任意一笔），重的在前；
    /// 2. 有字频、以键入笔画开头、笔画更多的字，重的在前；
    /// 3. 没有字频的字（权重为 0 的生僻字），同样精确匹配在前、补全在后。
    ///
    /// 笔数恰好打满时，生僻字的精确匹配不再挡在常用字的补全前面。
    pub fn candidates(&self, dictionary: &LanguageDictionary) -> Result<Vec<StrokeCandidate>> {
        let input = self.input.as_str();
        if input.is_empty() {
            return Ok(Vec::new());
        }
        let wildcard = char::from(WILDCARD);
        let (exact, completions) = if input.contains(wildcard) {
            (
                dictionary.lookup_pattern(input, wildcard, false, EXACT_LIMIT)?,
                dictionary.lookup_pattern(input, wildcard, true, COMPLETION_LIMIT)?,
            )
        } else {
            let keyed = |entry: LanguageEntry| (input.to_owned(), entry);
            (
                dictionary
                    .lookup(input, EXACT_LIMIT)?
                    .into_iter()
                    .map(keyed)
                    .collect(),
                dictionary.lookup_completions(input, COMPLETION_LIMIT)?,
            )
        };
        let mut seen = HashSet::with_capacity(exact.len() + completions.len());
        let mut candidates = Vec::with_capacity(exact.len() + completions.len());
        for (key, entry) in exact.into_iter().chain(completions) {
            if seen.insert(entry.text.clone()) {
                candidates.push(StrokeCandidate {
                    text: entry.text,
                    weight: entry.weight,
                    key,
                });
            }
        }
        // 稳定排序：只把没有字频的字移到后面，其余顺序不变。
        candidates.sort_by_key(|candidate| candidate.weight <= 0);
        Ok(candidates)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::language_dictionary::open_read_only;
    use crate::stroke::fixture;

    struct Fixture {
        _dir: tempfile::TempDir,
        dictionary: LanguageDictionary,
    }

    fn fixture() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("msime-stroke.db");
        fixture::build(&path);
        Fixture {
            dictionary: open_read_only(&path).unwrap(),
            _dir: dir,
        }
    }

    fn typed(keys: &str) -> StrokeScheme {
        let mut scheme = StrokeScheme::new();
        for key in keys.bytes() {
            scheme.handle_key(SchemeKey::Letter(key));
        }
        scheme
    }

    fn texts(scheme: &StrokeScheme, dictionary: &LanguageDictionary) -> Vec<String> {
        scheme
            .candidates(dictionary)
            .unwrap()
            .into_iter()
            .map(|candidate| candidate.text)
            .collect()
    }

    #[test]
    fn strokes_append_and_show_as_glyphs() {
        let scheme = typed("hspnz");
        assert_eq!(scheme.input(), "hspnz");
        assert_eq!(scheme.preedit(), "一丨丿丶乛");
        let request = scheme.build_request();
        assert_eq!(request.scheme, SchemeType::Stroke);
        assert_eq!(request.raw_input, "hspnz");
        assert_eq!(request.raw_input_with_cases, "hspnz");
        assert_eq!(request.normalized_segmentation, "一丨丿丶乛");
        assert!(request.valid);
        assert!(!StrokeScheme::new().build_request().valid);
        assert_eq!(StrokeScheme::new().preedit(), "");
    }

    #[test]
    fn other_keys_change_nothing() {
        let mut scheme = typed("h");
        for key in [
            SchemeKey::Letter(b'a'),
            SchemeKey::Letter(b'H'),
            SchemeKey::Letter(b'q'),
            SchemeKey::Apostrophe,
            SchemeKey::Semicolon,
            SchemeKey::Minus,
            SchemeKey::Symbol(b'*'),
            SchemeKey::Requery,
        ] {
            scheme.handle_key(key);
        }
        assert_eq!(scheme.input(), "h");
    }

    #[test]
    fn the_wildcard_needs_a_stroke_before_it() {
        let mut scheme = typed("x");
        assert!(scheme.is_empty());
        scheme.handle_key(SchemeKey::Letter(b'h'));
        scheme.handle_key(SchemeKey::Letter(b'x'));
        scheme.handle_key(SchemeKey::Letter(b'x'));
        assert_eq!(scheme.input(), "hxx");
        assert_eq!(scheme.preedit(), "一＊＊");
    }

    #[test]
    fn backspace_removes_the_last_stroke_and_reset_clears() {
        let mut scheme = typed("hsx");
        scheme.handle_key(SchemeKey::Backspace);
        assert_eq!(scheme.input(), "hs");
        scheme.reset();
        assert!(scheme.is_empty());
        scheme.handle_key(SchemeKey::Backspace);
        assert!(scheme.is_empty());
    }

    #[test]
    fn the_composition_stops_at_the_stroke_limit() {
        let mut scheme = typed(&"h".repeat(MAX_STROKES));
        scheme.handle_key(SchemeKey::Letter(b's'));
        scheme.handle_key(SchemeKey::Letter(b'x'));
        assert_eq!(scheme.input().len(), MAX_STROKES);
        assert!(scheme.input().bytes().all(|key| key == b'h'));
        scheme.set_raw_input(&"s".repeat(MAX_STROKES + 5));
        assert_eq!(scheme.input().len(), MAX_STROKES);
    }

    #[test]
    fn host_edits_keep_keys_and_read_glyphs_back() {
        let mut scheme = StrokeScheme::new();
        scheme.set_raw_input("h丨a丿x1乛 ＊");
        assert_eq!(scheme.input(), "hspxzx");
        assert_eq!(scheme.preedit(), "一丨丿＊乛＊");
        // 宿主编辑留下的开头通配符保留，查询照样能用。
        scheme.set_raw_input("xs");
        assert_eq!(scheme.input(), "xs");
        scheme.set_raw_input("");
        assert!(scheme.is_empty());
    }

    #[test]
    fn exact_codes_come_before_longer_ones_and_texts_are_listed_once() {
        let fixture = fixture();
        let dictionary = &fixture.dictionary;
        // 十 的笔画码正好是 hs，排在更重的长码字前面；土 有两个笔画码，只列一次。
        assert_eq!(texts(&typed("hs"), dictionary), ["十", "土"]);
        assert_eq!(
            texts(&typed("h"), dictionary),
            ["一", "大", "二", "十", "三", "王", "土", "干"]
        );
        let candidates = typed("hs").candidates(dictionary).unwrap();
        assert_eq!(
            candidates[0],
            StrokeCandidate {
                text: "十".to_owned(),
                weight: 4500,
                key: "hs".to_owned(),
            }
        );
        assert_eq!(candidates[1].key, "hsh");
        assert!(texts(&typed("zzzz"), dictionary).is_empty());
        assert!(texts(&StrokeScheme::new(), dictionary).is_empty());
    }

    #[test]
    fn characters_without_a_frequency_follow_every_weighted_one() {
        let fixture = fixture();
        let dictionary = &fixture.dictionary;
        // 乚 的笔画码正好是 sz，但没有字频，排在笔画更多、有字频的 口 后面。
        assert_eq!(texts(&typed("sz"), dictionary), ["口", "乚"]);
        // 通配符同样如此。
        assert_eq!(texts(&typed("sx"), dictionary), ["口", "乚"]);
    }

    #[test]
    fn the_wildcard_matches_any_one_stroke() {
        let fixture = fixture();
        let dictionary = &fixture.dictionary;
        // hx：两笔的 二 十 在前，再是 h 开头、笔画更多的字。
        assert_eq!(
            texts(&typed("hx"), dictionary),
            ["二", "十", "大", "三", "王", "土", "干"]
        );
        assert_eq!(texts(&typed("hxh"), dictionary), ["三", "土"]);
        let mut scheme = StrokeScheme::new();
        scheme.set_raw_input("xn");
        assert_eq!(texts(&scheme, dictionary), ["人"]);
    }
}
