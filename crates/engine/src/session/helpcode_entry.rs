//! 显式辅助码前导键只转换后续字母；筛选和编码消费继续由既有辅助码算法负责。

use super::input::InputSession;
use super::options::HelpcodeTrigger;
use crate::types::{Command, KeyResult, LocalInputMode, SchemeType};

impl InputSession {
    pub(super) fn helpcode_trigger_available(&self) -> bool {
        let Some(key) = self.helpcode_trigger_key else {
            return false;
        };
        if !self.helpcode_enabled()
            || (self.scheme() == SchemeType::Shuangpin
                && self.engine.request().shuangpin_helpcode_modes.manual_tab)
            || self.dedicated_english
            || self.local_mode != LocalInputMode::None
            || self.conversion.is_some()
            || self.helpcode_entry.is_some()
            || self.caret_position() != self.editing_text_len()
        {
            return false;
        }
        let raw = self.raw_with_cases();
        if raw.is_empty()
            || !raw
                .bytes()
                .all(|value| value.is_ascii_lowercase() || matches!(value, b'\'' | b';'))
        {
            return false;
        }
        // 微软的半音节仍先接收分号韵母；完整音节后的分号才是前导键。
        !(key == b';'
            && self.scheme() == SchemeType::Shuangpin
            && self.profile.uses_semicolon_key()
            && raw.rsplit('\'').next().unwrap_or_default().len() % 2 == 1)
    }

    pub(super) fn helpcode_character(&mut self, value: u8) -> Option<KeyResult> {
        if let Some(begin) = self.helpcode_entry {
            if self.helpcode_enabled()
                && self.caret_position() == self.editing_text_len()
                && self.raw_with_cases().len().saturating_sub(begin) < 2
                && value.is_ascii_alphabetic()
            {
                if self.raw_with_cases().len() == begin + 1 {
                    self.set_first_shuangpin_helpcode_case(begin, false);
                }
                return Some(self.dispatch_character(value.to_ascii_uppercase(), false));
            }
            self.helpcode_entry = None;
        }
        if self.helpcode_trigger_key == Some(value) && self.helpcode_trigger_available() {
            self.helpcode_entry = Some(self.raw_with_cases().len());
            return Some(KeyResult::handled());
        }
        None
    }

    pub(super) fn helpcode_command(&mut self, command: Command) -> Option<KeyResult> {
        let begin = self.helpcode_entry?;
        if command == Command::Backspace {
            if self.raw_with_cases().len() == begin {
                self.helpcode_entry = None;
                return Some(KeyResult::handled());
            }
            if self.raw_with_cases().len() == begin + 2 {
                self.set_first_shuangpin_helpcode_case(begin, true);
            }
        } else {
            self.helpcode_entry = None;
        }
        None
    }

    /// 前导键还在等辅助码时它在原文里的位置（#10731）。两位码齐了就不再显示：下一个字母开始新的拼音，前导状态随之结束，分号若留到那一键才消失，看起来像被那个字母吃掉了。
    pub(super) fn helpcode_trigger_display(&self) -> Option<HelpcodeTrigger> {
        let begin = self.helpcode_entry?;
        let key = self.helpcode_trigger_key?;
        let codes = self.raw_with_cases().len().checked_sub(begin)?;
        (codes < 2).then_some(HelpcodeTrigger {
            key: char::from(key),
            position: begin,
        })
    }

    /// 把前导键插进显示用的 `preedit`，紧挨在末尾已输入的辅助码前面。各种显示里辅助码都在 `preedit` 末尾（`ni'F`、`niF`、双拼拼音分词的 `ni'f`），插进去是 `ni';F`、`ni;F`、`ni';f`。只插不删：引擎为隔开辅助码画的 `'` 留着，宿主（macOS 的置顶按 `preedit` 记码）因此能按 `editing_text` 里码的位数把前导键原样摘掉，拿回不含它的拼写。末尾对不上辅助码时不插，宁可不显示也不画错位置。
    pub(super) fn preedit_with_helpcode_trigger(
        &self,
        preedit: String,
        trigger: HelpcodeTrigger,
    ) -> String {
        let Some(codes) = self.raw_with_cases().get(trigger.position..) else {
            return preedit;
        };
        let Some(split) = preedit.len().checked_sub(codes.len()) else {
            return preedit;
        };
        if !preedit.is_char_boundary(split) || !preedit[split..].eq_ignore_ascii_case(codes) {
            return preedit;
        }
        let mut shown = preedit;
        shown.insert(split, trigger.key);
        shown
    }

    fn set_first_shuangpin_helpcode_case(&mut self, begin: usize, uppercase: bool) {
        if self.scheme() != SchemeType::Shuangpin {
            return;
        }
        // 既有双拼协议把首位大写的双码倒序；显式前导输入按键入顺序编码。
        // 退回单码时恢复大写，保持单码优先末字的既有语义。
        let mut cased = self.raw_with_cases().as_bytes().to_vec();
        cased[begin] = if uppercase {
            cased[begin].to_ascii_uppercase()
        } else {
            cased[begin].to_ascii_lowercase()
        };
        let cased = String::from_utf8(cased).expect("拼音与辅助码只包含 ASCII");
        self.engine
            .replace_active_raw_input(&cased.to_ascii_lowercase(), &cased);
    }
}
