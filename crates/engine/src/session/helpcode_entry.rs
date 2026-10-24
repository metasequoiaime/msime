//! 显式辅助码前导键只转换后续字母；筛选和编码消费继续由既有辅助码算法负责。

use super::input::InputSession;
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
