use super::{LocalInputMode, Session};
use crate::types::KeyResult;

impl Session {
    /// 独立快捷键入口只接管空闲会话，不篡改专用英文状态或丢弃已有组字。
    pub fn enter_quick_phrase(&mut self) -> KeyResult {
        if self.nine_key.active()
            || self.input.has_composition()
            || self.input.dedicated_english
            || !self.input.scheme().opens_table_modes()
            || !self.input.local_mode_options.quick_phrase
        {
            return KeyResult::unhandled();
        }
        self.candidates_may_change();
        self.prediction.clear();
        KeyResult::handled().with_diagnostic(
            self.input
                .enter_local_mode(LocalInputMode::QuickPhrase, b'K'),
        )
    }
}
