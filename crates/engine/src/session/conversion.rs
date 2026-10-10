//! 整句改字：全拼、双拼打完一长串后，首选整句里有个别字不对时，用光标键在整句的汉字之间移动，候选换成光标处那段读音的字和词，选中一个把这段钉住替换掉，句子其余部分围绕钉住的段重新转换，确认后整句上屏。钉住与重新转换的语义与注音编辑器相同（`zhuyin::conversion`）。
//!
//! 改字只在首选是一个字对一个音节、读音完整、覆盖整个组字的句子时进入，所以字的位置就是音节的位置。改字期间组字原文不变，字母光标停在末尾；键入字母或撇号、退格、Esc 和字母光标命令都先退出改字（丢掉钉住的段），再照常处理。

use super::composition::normalize_canonical_pinyin_for_word;
use super::input::InputSession;
use super::learning::MAX_LEARNED_SENTENCE_SYLLABLES;
use crate::diagnostics;
use crate::lattice::decode::PinnedSpan;
use crate::pinyin::segment::split_segments;
use crate::text::count_han_chars;
use crate::types::{CandidateSource, Command, KeyResult, LocalInputMode, SchemeType, WordItem};

/// 每个长度的跨度最多列出的候选条数。
const ROWS_PER_SPAN: usize = 64;
/// 候选里最长的词，与词网格的 `max_phrase_syllables` 一致。
const MAX_SPAN_SYLLABLES: usize = 7;

/// 改字的状态。`segments` 首尾相接覆盖全部音节，钉住的段原样出现在其中。
#[derive(Debug, Clone)]
pub(super) struct ConversionEdit {
    syllables: Vec<String>,
    segments: Vec<PinnedSpan>,
    pins: Vec<PinnedSpan>,
    /// 光标在第几个字之前；等于字数时在句末，没有候选。
    focus: usize,
    rows: Vec<WordItem>,
    /// 进入改字时的那一行。没有钉住任何段就上屏时原样上屏它；否则上屏的整句沿用它的 `pinyin` 和方案，组字推进因此消耗整个组字。
    origin: WordItem,
    /// `origin` 在拼音候选列表里的位置。
    origin_index: usize,
}

impl ConversionEdit {
    /// 首选能改字时的初始状态：读音是一个字对一个完整音节，至少两个字。整句候选按它的词切段，其余候选整个算一段。
    fn from_row(row: &WordItem) -> Option<Self> {
        let canonical = normalize_canonical_pinyin_for_word(&row.canonical_pinyin, &row.word);
        if canonical.is_empty() {
            return None;
        }
        let syllables = split_segments(&canonical);
        let count = syllables.len();
        if count < 2 || row.word.chars().count() != count {
            return None;
        }
        let words_cover = !row.sentence_words.is_empty()
            && row
                .sentence_words
                .iter()
                .map(|word| word.chars().count())
                .sum::<usize>()
                == count;
        let words = if words_cover {
            row.sentence_words.clone()
        } else {
            vec![row.word.clone()]
        };
        let mut segments = Vec::with_capacity(words.len());
        let mut start = 0;
        for word in words {
            let end = start + word.chars().count();
            segments.push(PinnedSpan {
                start,
                end,
                key: syllables[start..end].join("'"),
                word,
            });
            start = end;
        }
        Some(Self {
            focus: count - 1,
            syllables,
            segments,
            pins: Vec::new(),
            rows: Vec::new(),
            origin: row.clone(),
            origin_index: 0,
        })
    }

    pub(super) fn text(&self) -> String {
        self.segments
            .iter()
            .map(|segment| segment.word.as_str())
            .collect()
    }

    pub(super) fn rows(&self) -> &[WordItem] {
        &self.rows
    }

    /// 光标所在的字，和光标处那一段的结尾（都按字计）。光标在句末时两者相等。
    pub(super) fn focus_span(&self) -> (usize, usize) {
        (self.focus, self.natural_end())
    }

    /// 光标所在那一段的结尾：光标在词中间时是这个词的结尾，在句末时就是句末。
    fn natural_end(&self) -> usize {
        self.segments
            .iter()
            .find(|segment| segment.start <= self.focus && self.focus < segment.end)
            .map_or(self.focus, |segment| segment.end)
    }

    /// 当前转换在 `start..end` 这几个字上的文字。
    fn text_between(&self, start: usize, end: usize) -> String {
        let mut text = String::new();
        for segment in &self.segments {
            for (offset, character) in segment.word.chars().enumerate() {
                let at = segment.start + offset;
                if (start..end).contains(&at) {
                    text.push(character);
                }
            }
        }
        text
    }
}

/// 重新转换解不出路径时的退路：没碰到钉住段的段原样保留，碰到的拆成单字，再放进钉住的段。
fn overlay_pins(segments: &[PinnedSpan], pins: &[PinnedSpan]) -> Vec<PinnedSpan> {
    let mut result = Vec::with_capacity(segments.len() + pins.len());
    for segment in segments {
        let touched = pins
            .iter()
            .any(|pin| segment.start < pin.end && pin.start < segment.end);
        if !touched {
            result.push(segment.clone());
            continue;
        }
        let keys: Vec<&str> = segment.key.split('\'').collect();
        for (offset, character) in segment.word.chars().enumerate() {
            let at = segment.start + offset;
            if pins.iter().any(|pin| pin.start <= at && at < pin.end) {
                continue;
            }
            result.push(PinnedSpan {
                start: at,
                end: at + 1,
                word: character.to_string(),
                key: keys.get(offset).copied().unwrap_or_default().to_owned(),
            });
        }
    }
    result.extend(pins.iter().cloned());
    result.sort_by_key(|span| span.start);
    result
}

impl InputSession {
    /// 改字只用于全拼和双拼的普通组字。
    fn conversion_applies(&self) -> bool {
        matches!(self.scheme(), SchemeType::Quanpin | SchemeType::Shuangpin)
            && self.local_mode == LocalInputMode::None
            && !self.dedicated_english
            && self.temporary_original_scheme.is_none()
    }

    /// `ConversionLeft` / `ConversionRight`。不在改字里时左移从首选尝试进入改字；进不了（右移、首选不是可改的整句）时为 `None`，调用方按字母光标处理。
    pub(super) fn move_conversion(&mut self, command: Command) -> Option<KeyResult> {
        self.move_conversion_from(command, 0)
    }

    /// 同 `move_conversion`，进入改字时从第 `row` 个候选开始：宿主高亮的那一行可能因为 runtime 的重排不在首位，改的应当是用户看着的那一句。
    pub(super) fn move_conversion_from(
        &mut self,
        command: Command,
        row_index: usize,
    ) -> Option<KeyResult> {
        if let Some(edit) = self.conversion.as_mut() {
            let count = edit.syllables.len();
            edit.focus = if command == Command::ConversionLeft {
                edit.focus.saturating_sub(1)
            } else {
                (edit.focus + 1).min(count)
            };
            self.refresh_conversion_rows();
            return Some(KeyResult::handled());
        }
        if command != Command::ConversionLeft || !self.conversion_applies() {
            return None;
        }
        let row = self.candidates().get(row_index)?;
        if !self.selection_completes_composition(&row.pinyin, &row.word, row.scheme) {
            return None;
        }
        let mut edit = ConversionEdit::from_row(row)?;
        // 没钉住任何段就上屏时，上屏的是这一行。
        edit.origin_index = row_index;
        self.caret = None;
        self.conversion = Some(edit);
        self.refresh_conversion_rows();
        Some(KeyResult::handled())
    }

    /// 改字期间的命令；`None` 表示已退出改字，命令照常处理。
    pub(super) fn handle_conversion_command(&mut self, command: Command) -> Option<KeyResult> {
        match command {
            Command::ConversionLeft | Command::ConversionRight => self.move_conversion(command),
            Command::CommitCandidate | Command::CommitRaw => Some(self.commit_conversion()),
            // 回到拼音，组字保留；再按一次 Esc 才丢掉组字。
            Command::Backspace | Command::Cancel => {
                self.leave_conversion();
                Some(KeyResult::handled())
            }
            Command::MoveLeft
            | Command::MoveRight
            | Command::MoveHome
            | Command::MoveEnd
            | Command::DeleteForward => {
                self.leave_conversion();
                None
            }
            Command::CycleKanaVariant | Command::CommitReading | Command::ConvertHanja => {
                Some(KeyResult::unhandled())
            }
        }
    }

    /// 丢掉改字，回到原来的拼音组字和候选。
    pub(super) fn leave_conversion(&mut self) {
        self.conversion = None;
    }

    /// 选中光标处的第 `index` 个候选：把这段钉住、重新转换，光标移到这段后面。光标在句末（没有候选）时上屏整句。
    pub(super) fn select_conversion_row(&mut self, index: usize) -> KeyResult {
        let Some(mut edit) = self.conversion.take() else {
            return KeyResult::unhandled();
        };
        if edit.rows.is_empty() {
            self.conversion = Some(edit);
            return self.commit_conversion();
        }
        let Some(row) = edit.rows.get(index) else {
            self.conversion = Some(edit);
            return KeyResult::unhandled();
        };
        let start = edit.focus;
        let end = start + row.word.chars().count();
        let pin = PinnedSpan {
            start,
            end,
            word: row.word.clone(),
            key: row.canonical_pinyin.clone(),
        };
        edit.pins
            .retain(|other| other.end <= start || other.start >= end);
        edit.pins.push(pin);
        edit.pins.sort_by_key(|pin| pin.start);
        let converted = self
            .canonical_phrase_engine()
            .convert_pinned(&edit.syllables, &edit.pins);
        edit.segments = if converted.is_empty() {
            overlay_pins(&edit.segments, &edit.pins)
        } else {
            converted
        };
        edit.focus = end;
        self.conversion = Some(edit);
        self.refresh_conversion_rows();
        KeyResult::handled()
    }

    /// 上屏改好的整句。什么都没钉住时与选中首选完全相同；否则把整句当作一条整句候选上屏：存成个人词条（不超过整句学习的上限时），按句中的词记入个人上下文。
    pub(super) fn commit_conversion(&mut self) -> KeyResult {
        let Some(edit) = self.conversion.take() else {
            return KeyResult::unhandled();
        };
        if edit.pins.is_empty() {
            return self.commit(edit.origin_index);
        }
        let text = edit.text();
        let mut item = edit.origin;
        item.canonical_pinyin = edit.syllables.join("'");
        item.sentence_words = edit
            .segments
            .into_iter()
            .map(|segment| segment.word)
            .collect();
        item.word = text.clone();
        item.weight = 0;
        item.source = CandidateSource::Generated;
        item.sentence_association = true;
        item.fixed_position = 0;
        item.fuzzy = false;
        item.corrected_from.clear();
        let diagnostic = self.learn_conversion_sentence(&item);
        self.caret = None;
        self.commit_selection(Some(item), Some(text), diagnostic)
    }

    /// 把改好的整句存成个人词条，规则与选中整句候选（`learn_sentence_candidate`）相同。
    fn learn_conversion_sentence(&mut self, item: &WordItem) -> Option<String> {
        if !self.learning_enabled || !self.can_learn_sentence_candidate(item) {
            return None;
        }
        let canonical = normalize_canonical_pinyin_for_word(&item.canonical_pinyin, &item.word);
        if canonical.is_empty() || split_segments(&canonical).len() > MAX_LEARNED_SENTENCE_SYLLABLES
        {
            return None;
        }
        self.store_user_phrase_from_canonical_pinyin(&canonical, &item.word)
            .err()
            .map(|_| diagnostics::SENTENCE_NOT_PERSISTED.to_owned())
    }

    /// 光标处的候选：先是光标所在那一段（光标在词中间时从光标到词尾），当前转换的文字排第一；再是更短的段，最后是更长的段，同一长度按权重。
    fn refresh_conversion_rows(&mut self) {
        let Some(mut edit) = self.conversion.take() else {
            return;
        };
        edit.rows.clear();
        let count = edit.syllables.len();
        let focus = edit.focus;
        if focus < count {
            let natural = edit.natural_end() - focus;
            let longest = (count - focus).min(MAX_SPAN_SYLLABLES).max(natural);
            let mut lengths = Vec::with_capacity(longest);
            lengths.push(natural);
            lengths.extend((1..natural).rev());
            lengths.extend(natural + 1..=longest);
            let scheme = self.scheme();
            let push = |rows: &mut Vec<WordItem>, word: String, key: &str, weight: i64| {
                if rows.iter().any(|row| row.word == word) {
                    return;
                }
                let mut item = WordItem::new(key, word, weight, CandidateSource::Database, key);
                item.scheme = scheme;
                rows.push(item);
            };
            let current = edit.text_between(focus, focus + natural);
            let current_key = edit.syllables[focus..focus + natural].join("'");
            push(&mut edit.rows, current, &current_key, 0);
            let engine = self.canonical_phrase_engine();
            for length in lengths {
                let span = &edit.syllables[focus..focus + length];
                let key = span.join("'");
                for row in engine.conversion_rows(span, ROWS_PER_SPAN) {
                    if row.value.chars().count() == length && count_han_chars(&row.value) == length
                    {
                        push(&mut edit.rows, row.value, &key, row.weight);
                    }
                }
            }
        }
        self.conversion = Some(edit);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn span(start: usize, end: usize, word: &str, key: &str) -> PinnedSpan {
        PinnedSpan {
            start,
            end,
            word: word.to_owned(),
            key: key.to_owned(),
        }
    }

    #[test]
    fn a_sentence_row_is_cut_at_its_words() {
        let mut row = WordItem::new(
            "woqubeijing",
            "我去背景",
            0,
            CandidateSource::Generated,
            "wo'qu'bei'jing",
        );
        row.sentence_words = vec!["我".into(), "去".into(), "背景".into()];
        let edit = ConversionEdit::from_row(&row).expect("a convertible sentence");
        assert_eq!(edit.text(), "我去背景");
        assert_eq!(edit.segments[2], span(2, 4, "背景", "bei'jing"));
        assert_eq!(edit.focus_span(), (3, 4));
        assert_eq!(edit.text_between(1, 3), "去背");

        // 词典里的整词没有切分，整个算一段；单字和读音不完整的不能改字。
        let word = WordItem::new("nihao", "你好", 9, CandidateSource::Database, "ni'hao");
        let edit = ConversionEdit::from_row(&word).expect("a whole word");
        assert_eq!(edit.segments, [span(0, 2, "你好", "ni'hao")]);
        assert_eq!(edit.focus_span(), (1, 2));
        let single = WordItem::new("ni", "你", 9, CandidateSource::Database, "ni");
        assert!(ConversionEdit::from_row(&single).is_none());
        let abbreviated = WordItem::new("nh", "你好", 9, CandidateSource::Database, "n'h");
        assert!(ConversionEdit::from_row(&abbreviated).is_none());
    }

    #[test]
    fn the_fallback_splits_only_what_a_pin_touches() {
        let segments = [span(0, 1, "我", "wo"), span(1, 3, "背景", "bei'jing")];
        let result = overlay_pins(&segments, &[span(2, 3, "京", "jing")]);
        assert_eq!(
            result,
            [
                span(0, 1, "我", "wo"),
                span(1, 2, "背", "bei"),
                span(2, 3, "京", "jing")
            ]
        );
    }
}
