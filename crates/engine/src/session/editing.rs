//! Caret editing, the editing text, segment boundaries and caret-prefix decoding (core-session.md §5.9, overlays.md §7.6).

use super::input::InputSession;
use crate::local::url;
use crate::local::GENERATED_MODE_INPUT_LIMIT;
use crate::shuangpin::query::{
    detect_active_double_helpcode_length, segment_raw_boundaries,
    trim_trailing_letters_preserve_delimiters,
};
use crate::stroke;
use crate::types::{Command, KeyResult, LocalInputMode, SchemeType, ShuangpinProfileKind};

pub(super) fn temporary_japanese_preedit(raw: &str) -> String {
    let mut preedit = String::with_capacity(1 + raw.len());
    preedit.push('R');
    preedit.push_str(raw);
    preedit
}

impl InputSession {
    /// 专用英文的预编辑；临时日文是 `"R"` 加带大小写的原文；本地模式的预编辑；越南文和藏文是显示出来的文字；粤拼是按音节加空格的字母；其余是带大小写的原文，笔画里就是键入的 `hspnzx` 字母，与 reading 画出的笔画字形一一对应。
    pub(super) fn editing_text(&self) -> String {
        if self.dedicated_english {
            return self.dedicated_english_preedit.clone();
        }
        match self.local_mode {
            LocalInputMode::TemporaryJapanese => {
                temporary_japanese_preedit(&self.engine.request().raw_input_with_cases)
            }
            // A Vietnamese word is edited as the text it shows, not as its keystrokes.
            LocalInputMode::None if self.is_vietnamese() => self.engine.preedit().to_owned(),
            // 藏文音节串同样按显示出来的藏文编辑，而不是按威利按键。
            LocalInputMode::None if self.is_tibetan() => self.engine.preedit().to_owned(),
            // Jyutping is edited as the syllables it shows (`nei hou`); an edit drops the spaces again, because the scheme keeps only letters and `'`.
            LocalInputMode::None if self.is_cantonese() => {
                self.engine.request().normalized_segmentation.clone()
            }
            LocalInputMode::None => self.raw_with_cases().to_owned(),
            _ => self.local_preedit.clone(),
        }
    }

    pub(super) fn editing_text_len(&self) -> usize {
        if self.dedicated_english {
            return self.dedicated_english_preedit.len();
        }
        match self.local_mode {
            LocalInputMode::TemporaryJapanese => {
                1 + self.engine.request().raw_input_with_cases.len()
            }
            LocalInputMode::None if self.is_vietnamese() || self.is_tibetan() => {
                self.engine.preedit().len()
            }
            LocalInputMode::None if self.is_cantonese() => {
                self.engine.request().normalized_segmentation.len()
            }
            LocalInputMode::None => self.raw_with_cases().len(),
            _ => self.local_preedit.len(),
        }
    }

    pub(super) fn caret_position(&self) -> usize {
        let length = self.editing_text_len();
        self.caret.unwrap_or(length).min(length)
    }

    /// input_session_editing.cpp:123-159.
    pub(super) fn edit_at_caret(&mut self, command: Command) -> KeyResult {
        let text_len = self.editing_text_len();
        let mut caret = self.caret.unwrap_or(text_len).min(text_len);
        // 本地模式的前缀字母是模式标记，不是可编辑的内容；网址模式没有前缀字母，整段都能编辑。
        let begin = usize::from(!matches!(
            self.local_mode,
            LocalInputMode::None | LocalInputMode::Url
        ));
        match command {
            Command::MoveLeft => caret = caret.saturating_sub(1).max(begin),
            Command::MoveRight => caret = (caret + 1).min(text_len),
            Command::MoveHome => caret = begin,
            Command::MoveEnd => caret = text_len,
            Command::Backspace => {
                if caret <= begin {
                    return KeyResult::handled();
                }
                let mut text = self.editing_text();
                caret -= 1;
                let removed = text.remove(caret);
                return self.delete_editing_character(text, caret, removed);
            }
            Command::DeleteForward => {
                if caret == text_len {
                    return KeyResult::handled();
                }
                let mut text = self.editing_text();
                let removed = text.remove(caret);
                return self.delete_editing_character(text, caret, removed);
            }
            _ => return KeyResult::unhandled(),
        }
        self.caret = Some(caret);
        // Moving the caret changes which prefix is decoded.
        self.update_mixed_candidates();
        KeyResult::handled()
    }

    /// 从编辑文字里删掉 `removed` 后剩下 `text`。网址模式删空就退出；删掉的正是进入网址模式的那个键（`url_reverts`）时退回组字，否则与其他模式一样替换编辑文字。
    fn delete_editing_character(&mut self, text: String, caret: usize, removed: char) -> KeyResult {
        if self.local_mode == LocalInputMode::Url {
            // 网址删空后没有前缀字母可留，退出模式，否则会停在空的网址模式里吞掉后续按键。
            if text.is_empty() {
                self.reset_composition();
                return KeyResult::handled();
            }
            if self.url_reverts(&text, removed) {
                self.restore_composition_from_url(text);
                return KeyResult::handled();
            }
        }
        self.replace_editing_text(&text, caret)
    }

    /// input_session_editing.cpp:161-210.
    pub(super) fn insert_at_caret(&mut self, value: u8) -> KeyResult {
        let mut text = self.editing_text();
        let caret = self.caret_position();
        let lower = value.is_ascii_lowercase();
        let upper = value.is_ascii_uppercase();
        let mut accepted = lower || upper;
        if !self.dedicated_english {
            match self.local_mode {
                LocalInputMode::Unicode => {
                    accepted = value.is_ascii_hexdigit()
                        || (value == b'+' && caret == 1 && !text.contains('+'));
                }
                LocalInputMode::QuickPhrase => accepted = lower,
                LocalInputMode::DateTime => accepted = false,
                // 笔画只接受笔画键；通配符不能插在最前面；已满 `MAX_STROKES` 笔时不再插入（否则截断会丢掉末尾那一笔）。组合中的其他字母被吞掉，与在末尾键入时一样。
                LocalInputMode::None if self.stroke_rules_apply() => {
                    if !stroke::is_key(value)
                        || (value == stroke::WILDCARD && caret == 0)
                        || text.len() >= stroke::MAX_STROKES
                    {
                        return if value.is_ascii_alphabetic() {
                            KeyResult::handled()
                        } else {
                            KeyResult::unhandled()
                        };
                    }
                    accepted = true;
                }
                LocalInputMode::None => {
                    let scheme = self.scheme();
                    accepted = lower
                        || (upper
                            && ((scheme == SchemeType::Quanpin && self.quanpin_helpcode_enabled)
                                || (scheme == SchemeType::Shuangpin
                                    && self.shuangpin_helpcode_enabled)));
                    if value == b';'
                        && scheme == SchemeType::Shuangpin
                        && self.profile == ShuangpinProfileKind::Microsoft
                    {
                        // The Microsoft `ing` key is a final: it may only complete an odd-length chunk.
                        let start = text[..caret].rfind('\'').map_or(0, |at| at + 1);
                        accepted = (caret - start) % 2 == 1;
                    }
                    if value == b'\'' && scheme.accepts_apostrophe() {
                        accepted = caret > 0;
                    }
                }
                LocalInputMode::Emoji
                | LocalInputMode::Kaomoji
                | LocalInputMode::TemporaryJapanese => accepted = accepted || value == b'\'',
                LocalInputMode::SuperJianpin | LocalInputMode::TemporaryEnglish => {}
                LocalInputMode::Expression => {
                    accepted = text.len() < GENERATED_MODE_INPUT_LIMIT
                        && self
                            .local_mode
                            .spelling_symbols()
                            .as_bytes()
                            .contains(&value);
                }
                LocalInputMode::Command | LocalInputMode::Mention => {
                    accepted = text.len() < GENERATED_MODE_INPUT_LIMIT && lower;
                }
                // 与 `handle_local_character` 同一组规则。
                LocalInputMode::Url => {
                    if !url::accepts(value) {
                        return KeyResult::unhandled();
                    }
                    // 已到长度上限时吞掉按键，与行末键入一致；只有网址不收的键才交还 runtime。
                    if text.len() >= url::INPUT_LIMIT {
                        return KeyResult::handled();
                    }
                    accepted = true;
                }
            }
        }
        if !accepted {
            return KeyResult::unhandled();
        }
        let bytes = text.as_bytes();
        // 网址里的撇号是字面字符，可以连着出现。
        if value == b'\''
            && self.local_mode != LocalInputMode::Url
            && ((caret > 0 && bytes[caret - 1] == b'\'') || bytes.get(caret) == Some(&b'\''))
        {
            return KeyResult::handled();
        }
        text.insert(caret, value as char);
        self.replace_editing_text(&text, caret + 1)
    }

    /// input_session_editing.cpp:212-245.
    pub(super) fn replace_editing_text(&mut self, text: &str, caret: usize) -> KeyResult {
        let mut diagnostic = None;
        if self.dedicated_english {
            self.dedicated_english_preedit = text.to_owned();
            self.update_dedicated_english_candidates();
        } else if self.local_mode != LocalInputMode::None
            && self.local_mode != LocalInputMode::TemporaryJapanese
        {
            self.local_preedit = text.to_owned();
            diagnostic = self.update_local_candidates();
        } else {
            let payload = if self.local_mode == LocalInputMode::TemporaryJapanese {
                &text[1..]
            } else {
                text
            };
            self.pending_sequence = Some(payload.to_ascii_lowercase());
            self.pending_sequence_with_cases = Some(payload.to_owned());
            self.apply_pending_sequence();
            if self.local_mode == LocalInputMode::TemporaryJapanese {
                self.refresh_temporary_japanese();
            }
        }
        self.caret = Some(caret);
        // The edit may have moved the caret into or out of a complete prefix.
        self.update_mixed_candidates();
        self.online_requests.invalidate();
        self.discard_abandoned_phrase_progress();
        KeyResult::handled().with_diagnostic(diagnostic)
    }

    /// input_session_editing.cpp:80-121.
    pub(super) fn segment_raw_boundaries(&self) -> Vec<usize> {
        // Local modes and dedicated English spell words, not syllables: the host edits them one character at a time.
        if self.dedicated_english || self.local_mode != LocalInputMode::None {
            return Vec::new();
        }
        let raw_with_cases = self.raw_with_cases();
        if raw_with_cases.is_empty() {
            return Vec::new();
        }
        match self.engine.current_scheme_type() {
            SchemeType::Shuangpin => {
                let profile = self.shuangpin_profile();
                let raw = &self.engine.request().raw_input;
                let helpcode_length =
                    detect_active_double_helpcode_length(raw, raw_with_cases, profile);
                let base = if helpcode_length > 0 {
                    trim_trailing_letters_preserve_delimiters(raw_with_cases, helpcode_length)
                } else {
                    raw_with_cases.to_owned()
                };
                let mut boundaries = segment_raw_boundaries(&base, profile);
                if helpcode_length > 0 && !boundaries.is_empty() {
                    // The active double helpcode is one editable unit of its own, the same boundary raw_segmentation draws before it.
                    boundaries.push(base.len());
                    boundaries.push(raw_with_cases.len());
                    boundaries.sort_unstable();
                    boundaries.dedup();
                }
                boundaries
            }
            SchemeType::Quanpin => {
                quanpin_raw_boundaries(raw_with_cases, &self.pinyin_segmentation_with_cases())
            }
            // The Cantonese editing text is spaced, so raw offsets would not land on its syllables; the host edits it one character at a time. A stroke is one character, so Stroke has no units either.
            SchemeType::Wubi
            | SchemeType::JapaneseRomaji
            | SchemeType::Korean
            | SchemeType::Cantonese
            | SchemeType::Zhuyin
            | SchemeType::Vietnamese
            | SchemeType::Tibetan
            | SchemeType::Stroke => Vec::new(),
        }
    }

    /// Clamped to the editing text; recomputes the prefix candidates. Korean has no caret inside its open syllable and Zhuyin none inside its conversion, so the caret stays at the end.
    pub(super) fn set_caret(&mut self, caret: Option<usize>) {
        if self.engine.current_scheme_type().locks_caret()
            && !self.dedicated_english
            && self.local_mode == LocalInputMode::None
        {
            self.caret = None;
            return;
        }
        let length = self.editing_text_len();
        self.caret = caret.map(|position| position.min(length));
        // Only the scheme composition decodes by caret; local and English lists do not depend on it.
        if !self.dedicated_english && self.local_mode == LocalInputMode::None {
            self.update_mixed_candidates();
        }
    }

    /// The last segment boundary at or before the caret, or the raw length.
    pub(super) fn prefix_end(&self) -> usize {
        let raw_length = self.raw_with_cases().len();
        if self.caret.is_none() {
            return raw_length;
        }
        let boundaries = self.segment_raw_boundaries();
        let caret = self.caret_position();
        boundaries
            .iter()
            .rev()
            .find(|boundary| **boundary <= caret)
            .copied()
            .unwrap_or(raw_length)
    }

    pub(super) fn pending_suffix(&self) -> String {
        let raw = self.raw_with_cases();
        let end = self.prefix_end();
        raw.get(end..).unwrap_or_default().to_owned()
    }

    /// Decode only the complete units before the caret; cached by prefix.
    pub(super) fn refresh_prefix_candidates(&mut self) {
        self.prefix_active = false;
        if self.caret.is_none() || self.dedicated_english || self.local_mode != LocalInputMode::None
        {
            self.prefix_candidates.clear();
            self.prefix_query_input.clear();
            return;
        }
        let end = self.prefix_end();
        let raw_with_cases = self.raw_with_cases();
        // A caret inside the first unit has no whole unit before it: the whole-input list stays rather than an empty prefix query.
        if end == 0 || end >= raw_with_cases.len() {
            self.prefix_candidates.clear();
            self.prefix_query_input.clear();
            return;
        }
        let prefix_with_cases = &raw_with_cases[..end];
        let lowercase = prefix_with_cases
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte == b'\'');
        // Caret moves inside one unit keep the same prefix; only a new prefix costs a query.
        if lowercase {
            if self.prefix_query_input != prefix_with_cases {
                let prefix = prefix_with_cases.to_owned();
                self.prefix_candidates = self.engine.query_raw_candidates(&prefix, &prefix);
                self.prefix_query_input = prefix;
            }
        } else {
            let prefix = prefix_with_cases.to_ascii_lowercase();
            if self.prefix_query_input != prefix {
                let raw_with_cases = prefix_with_cases.to_owned();
                self.prefix_candidates = self.engine.query_raw_candidates(&prefix, &raw_with_cases);
                self.prefix_query_input = prefix;
            }
        }
        self.prefix_active = true;
    }

    /// The typed raw input with its case, as the scheme holds it.
    pub(super) fn raw_with_cases(&self) -> &str {
        let request = self.engine.request();
        if request.raw_input_with_cases.is_empty() {
            &request.raw_input
        } else {
            &request.raw_input_with_cases
        }
    }
}

/// Unit boundaries of a quanpin spelling in raw coordinates (input_session_editing.cpp:35-77). The visible preedit is always rebuilt from the raw letters, the autocorrect cut and the alias layer only moving or adding separators, so a separator in the display marks where the next raw unit starts. A display that cannot explain the raw letters yields no boundaries, and the host falls back to editing single characters rather than deleting an arbitrary span.
pub(super) fn quanpin_raw_boundaries(raw: &str, display: &str) -> Vec<usize> {
    let raw_bytes = raw.as_bytes();
    let raw_letter_count = raw_bytes.iter().filter(|byte| **byte != b'\'').count();
    if raw_letter_count == 0 {
        return Vec::new();
    }
    let mut boundaries = Vec::with_capacity(raw_letter_count + 1);
    boundaries.push(0);
    let mut raw_offset = 0;
    let mut letters_seen = 0;
    for byte in display.bytes() {
        if byte != b'\'' {
            letters_seen += 1;
            while raw_offset < raw_bytes.len() && raw_bytes[raw_offset] == b'\'' {
                raw_offset += 1;
            }
            raw_offset = raw_offset.saturating_add(1);
            continue;
        }
        // A trailing separator has no next letter and starts no unit.
        let mut next = raw_offset;
        while next < raw_bytes.len() && raw_bytes[next] == b'\'' {
            next += 1;
        }
        if next < raw_bytes.len() {
            boundaries.push(next);
        }
    }
    if letters_seen != raw_letter_count {
        return Vec::new();
    }
    boundaries.push(raw.len());
    boundaries.dedup();
    boundaries
}

#[cfg(test)]
mod tests {
    use super::quanpin_raw_boundaries;

    /// Why caret 1 keeps the whole-input list on `nihaoma` but decodes 虐 on `nhaoma` (golden qp_caret_editing, MoveRight vs DeleteForward): the prefix floors to a segmentation boundary, and a lone initial is its own unit, so only the second spelling has a boundary at 1.
    #[test]
    fn a_lone_initial_is_a_complete_unit_but_a_split_syllable_is_not() {
        assert_eq!(quanpin_raw_boundaries("nihaoma", "ni'hao'ma"), [0, 2, 5, 7]);
        assert_eq!(quanpin_raw_boundaries("nhaoma", "n'hao'ma"), [0, 1, 4, 6]);
    }

    #[test]
    fn quanpin_boundaries_do_not_allocate_temporary_raw_offsets() {
        let (boundaries, allocations) = crate::ime::personal_rerank::allocations::count(|| {
            quanpin_raw_boundaries("nihaoma", "ni'hao'ma")
        });

        assert_eq!(boundaries, [0, 2, 5, 7]);
        assert_eq!(allocations, 1, "boundary allocations: {allocations}");
    }
}
