//! 组字会话里的滑行输入：解码出的字母像打出来的一样，一次编辑写进组字。

use super::input::InputSession;
use crate::pinyin::glide::{decode_glide, rank_by_dictionary, GlideKeyboard, GlidePoint};
use crate::types::{KeyResult, LocalInputMode, SchemeType};

/// 几何交给词典排序的候选串数；共线的键（`zhong` 与 `zong`）要求第二、第三名也留下。
const GLIDE_HYPOTHESES: usize = 16;

impl InputSession {
    pub(super) fn glide(&mut self, keyboard: &GlideKeyboard, points: &[GlidePoint]) -> KeyResult {
        if self.dedicated_english
            || self.local_mode != LocalInputMode::None
            || self.scheme() != SchemeType::Quanpin
        {
            return KeyResult::unhandled();
        }
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
