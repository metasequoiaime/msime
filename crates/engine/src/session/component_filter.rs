//! 全拼的部件辅助段（#11804）：拼音完整、光标在末尾时按反引号，后面跟着打部件的全拼，筛出末字由这些部件组成的候选，例如 `cheng` 后打 `` `chong `` 只留 蛏。
//!
//! 反引号和部件段记进全拼输入串（预编辑是 `` cheng`chong ``，退格逐字删回去），但请求的 `raw_input` 及其派生字段只有反引号前的拼音（`QuanpinScheme::build_request_from_raw_into`），所以查询、切分、选词推进、学词、纠错抑制键和云候选都看不到部件段；看原串的只有预编辑、编辑光标和这里。筛选规则在 `local::chaizi::component_filter`，引擎候选在 `QuanpinEngine::query` 里筛，混进来的快捷短语、表情等在混排之后再筛一次。
//!
//! 由 `SessionOptions::component_filter_backtick` 打开，默认关。取舍见 `.agents/notes/implemented/feature/2026-11-27-component-pinyin-helpcode.md`。

use super::input::InputSession;
use crate::local::chaizi::component_filter;
use crate::pinyin::segment::is_complete_pinyin_input;
use crate::quanpin::scheme::COMPONENT_FILTER_MARKER;
use crate::types::{LocalInputMode, SchemeType, WordItem};

impl InputSession {
    /// 现在按反引号会不会打开部件辅助段。`spelling_symbols`、`handle_punctuation` 和 `dispatch_character` 共用这一个判断，发布的键就是接受的键。
    pub(super) fn component_filter_backtick_ready(&self) -> bool {
        if !self.engine.component_filter_backtick()
            || self.scheme() != SchemeType::Quanpin
            || self.engine.wubi_pinyin_lookup_active()
            || self.dedicated_english
            || self.local_mode != LocalInputMode::None
            || !self.has_composition()
            || self.conversion.is_some()
            || self.caret_position() != self.editing_text_len()
            || self.helpcode_entry.is_some()
            // 反引号已经是辅助码前导键时归它。
            || self.helpcode_trigger_key == Some(COMPONENT_FILTER_MARKER)
            || self.caret_edit.active
            || !self.caret_edit.held.is_empty()
            || self.glide_candidates.is_some()
        {
            return false;
        }
        // 已经有反引号时这里是带反引号的原串，被下面的字节检查排除；大写字母（辅助码或字面大写）同样排除。
        let raw = self.raw_with_cases();
        raw.bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte == b'\'')
            && is_complete_pinyin_input(raw)
    }

    /// 部件辅助段打开着（反引号已经在输入串里，后面可以还没有字母）。
    pub(super) fn component_filter_open(&self) -> bool {
        let request = self.engine.request();
        self.scheme() == SchemeType::Quanpin
            && self.local_mode == LocalInputMode::None
            && !self.dedicated_english
            && request
                .raw_input_with_helpcode_marks
                .as_bytes()
                .contains(&COMPONENT_FILTER_MARKER)
    }

    /// 只留末字对得上部件辅助段的行；没有在筛时原样返回。混排之后调用，管住引擎以外混进来的行。
    pub(super) fn apply_component_filter(&self, rows: Vec<WordItem>) -> Vec<WordItem> {
        let parts = &self.engine.request().component_filter_parts;
        if parts.is_empty() || !self.component_filter_open() {
            return rows;
        }
        component_filter(rows, parts)
    }
}
