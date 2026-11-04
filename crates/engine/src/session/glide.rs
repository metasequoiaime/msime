//! 组字会话里的滑行输入：解码出的字母像打出来的一样，一次编辑写进组字。

mod composition;
mod contractions;
mod ranking;

use super::input::InputSession;
use crate::pinyin::glide::{
    decode_glide, rank_by_dictionary, GlideKeyboard, GlideMatcher, GlidePoint,
};
use crate::types::{CandidateSource, KeyResult, LocalInputMode, SchemeType, WordItem};

/// 几何交给词典排序的候选串数；共线的键（`zhong` 与 `zong`）要求第二、第三名也留下。
const GLIDE_HYPOTHESES: usize = 16;

/// 词典行的 `pinyin` 可以只是匹配前缀；整笔必须覆盖完整读音，不能替用户补出未滑过的字母。
fn complete_chinese_glide_candidate(row: &WordItem, spelling: &str) -> bool {
    !row.word.is_ascii()
        && [&row.pinyin, &row.canonical_pinyin].iter().all(|reading| {
            reading
                .bytes()
                .filter(|&letter| letter != b'\'')
                .eq(spelling.bytes().filter(|&letter| letter != b'\''))
        })
}

impl InputSession {
    /// 每笔同时查询拼音和英文；完整候选独立保留，选词不改变中英文模式。
    pub(super) fn bilingual_glide(
        &mut self,
        keyboard: &GlideKeyboard,
        points: &[GlidePoint],
    ) -> KeyResult {
        if self.dedicated_english
            || self.local_mode != LocalInputMode::None
            || self.scheme() != SchemeType::Quanpin
            || self.has_active_helpcode()
        {
            return KeyResult::unhandled();
        }
        let Some(matcher) = GlideMatcher::new(keyboard, points) else {
            return KeyResult::unhandled();
        };
        let mut chinese = Vec::new();
        for hypothesis in decode_glide(keyboard, points, GLIDE_HYPOTHESES) {
            let rows = self
                .engine
                .query_raw_candidates(&hypothesis.letters, &hypothesis.letters);
            for mut row in rows
                .into_iter()
                .filter(|row| complete_chinese_glide_candidate(row, &hypothesis.letters))
                .take(3)
            {
                // 英文真词与任意拼音拼出的生僻组合竞争时，完整词典词应有有限优势。
                let cost = hypothesis.cost + if row.source.is_dictionary() { 0.0 } else { 2.0 };
                row.source = CandidateSource::Generated;
                chinese.push((hypothesis.letters.clone(), row, cost));
            }
        }
        let mut english = Vec::new();
        for letter in matcher.starting_letters() {
            let prefix = char::from(letter).to_string();
            // 每个起点最多查询 4096 个高频词，不把整份英文库常驻键盘内存。
            for mut row in self
                .queries
                .english_dictionary()
                .query_prefix(&prefix, 4096)
            {
                if let Some(cost) = matcher.score(&row.pinyin) {
                    row.source = CandidateSource::Generated;
                    english.push((row.pinyin.to_string(), row, cost));
                }
            }
        }
        contractions::append(
            &matcher,
            |base| {
                self.queries
                    .english_dictionary()
                    .query_exact(base)
                    .map(|row| row.weight)
            },
            &mut english,
        );
        fn normalize(rows: &mut [(String, WordItem, f32)]) {
            // 两份词库的权重尺度不同：先各自归一化，频度最多抵两点几何代价。
            let maximum = rows
                .iter()
                .map(|(_, row, _)| row.weight.max(1))
                .max()
                .unwrap_or(1) as f64;
            for (_, row, cost) in rows {
                *cost += ((maximum / row.weight.max(1) as f64).ln() * 0.25).min(2.0) as f32;
            }
        }
        normalize(&mut chinese);
        normalize(&mut english);
        let preference = self
            .candidates()
            .first()
            .filter(|_| self.has_composition())
            .and_then(|row| ranking::english_ending(&row.word))
            .or(self.glide_prefers_english);
        if let Some(prefer_english) = preference {
            let preferred = if prefer_english {
                &mut english
            } else {
                &mut chinese
            };
            for (_, _, cost) in preferred {
                *cost -= 0.5;
            }
        }
        chinese.extend(english);
        chinese.sort_by(|a, b| a.2.total_cmp(&b.2).then_with(|| a.1.word.cmp(&b.1.word)));
        let Some((spelling, _, best)) = chinese.first() else {
            return KeyResult::unhandled();
        };
        let spelling = spelling.clone();
        let ceiling = *best + 10.0;
        let mut rows = Vec::new();
        for (_, mut row, cost) in chinese {
            if cost <= ceiling && !rows.iter().any(|other: &WordItem| other.word == row.word) {
                row.corrected_from = spelling.clone().into();
                rows.push(row);
                if rows.len() == ranking::CANDIDATE_LIMIT {
                    break;
                }
            }
        }
        if self.has_composition() {
            return self.continue_bilingual_glide(&spelling, &rows);
        }
        // 候选语言只影响排序，不能替用户决定何时上屏。
        self.leave_conversion();
        let result = self.replace_editing_text(&spelling, spelling.len());
        self.glide_candidates = Some((spelling, rows));
        self.glide_selection_pending = true;
        result
    }

    pub(super) fn glide(&mut self, keyboard: &GlideKeyboard, points: &[GlidePoint]) -> KeyResult {
        self.exit_date_time_menu();
        if self.dedicated_english
            || self.local_mode != LocalInputMode::None
            || self.scheme() != SchemeType::Quanpin
        {
            return KeyResult::unhandled();
        }
        // 一笔滑行写进组字，与键入字母一样先退出整句改字。
        self.leave_conversion();
        let hypotheses = decode_glide(keyboard, points, GLIDE_HYPOTHESES);
        let engine = &self.engine;
        let ranked = rank_by_dictionary(hypotheses, |keys| engine.quanpin_best_weights(keys));
        let Some(best) = ranked.into_iter().next() else {
            return KeyResult::unhandled();
        };
        self.insert_glide_letters(&best.letters)
    }

    /// 把 `letters`（小写、非空）写到光标处。新组字的第一个字母按打字的方式输入，所以它的开始和打字时一样；滑进已有组字时，凡是挨着其他字母的一侧都加 `'` 隔开，两次滑行不会被读成一个音节（先 `xi` 再 `an` 仍是 `xi'an`）。
    fn insert_glide_letters(&mut self, letters: &str) -> KeyResult {
        let Some((&first, rest)) = letters.as_bytes().split_first() else {
            return KeyResult::unhandled();
        };
        if !self.has_composition() {
            let typed = self.handle_character(first, false);
            if !typed.handled || rest.is_empty() {
                return typed;
            }
            let text = {
                let current = self.raw_with_cases();
                let mut text = String::with_capacity(current.len() + rest.len());
                text.push_str(current);
                text.push_str(&letters[1..]);
                text
            };
            let caret = text.len();
            return self.replace_editing_text(&text, caret);
        }
        let caret = self.caret_position();
        let (text, new_caret) = {
            let current = self.raw_with_cases();
            let left_separator = caret > 0 && !current[..caret].ends_with('\'');
            let right_separator = caret < current.len() && !current[caret..].starts_with('\'');
            let inserted_len =
                letters.len() + usize::from(left_separator) + usize::from(right_separator);
            let mut text = String::with_capacity(current.len() + inserted_len);
            text.push_str(&current[..caret]);
            if left_separator {
                text.push('\'');
            }
            text.push_str(letters);
            if right_separator {
                text.push('\'');
            }
            text.push_str(&current[caret..]);
            (text, caret + inserted_len)
        };
        self.replace_editing_text(&text, new_caret)
    }
}
