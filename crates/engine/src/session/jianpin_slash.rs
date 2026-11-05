//! 双拼的后置超级简拼（#10860）：组字末尾按 `/`，把已经打的这段原文整段改成超级简拼查询，之后与「Shift+J 再打这几个字母」完全一样（`local_mode = SuperJianpin`，预编辑是 `J` 加原文），查询、排序、学习都走 J 模式现有的路径。
//!
//! 只在双拼、中文标点下，组字是 3 个以上的纯小写字母时触发：两个字母在双拼里正好是一个整音节，单字加 `/`（和/或、元/斤）不能被抢走。`/` 已经被别的功能占用时让位：万象式辅助码的 `/` 结束键、`/` 作辅助码前导键、`/` 出顿号。
//!
//! 转出来之后、简拼码还没改过时，退格和第二个 `/` 都撤回这次转换：退格回到按 `/` 之前的组字，`//` 一次得到今天单按 `/` 的结果（上屏首选再出 `/`）。`//` 在引擎里一次做完并且算已处理，不交还 runtime 再走一遍标点，否则缓存的 `spelling_symbols` 过期时会再次触发或重复上屏。引擎不知道宿主高亮着哪一行，这里的 `//` 上屏首选，只给没有高亮的直接调用方（C ABI、golden、单测）；经过 runtime 的宿主由 runtime 记下转换前的高亮，`//` 先用退格撤回、再从那一行结束组字（`msime_input_runtime` 的 `jianpin_slash`），退格撤回时高亮也回到那一行。
//!
//! 取舍和被放弃的方案见 `.agents/notes/implemented/feature/2026-11-05-shuangpin-postfix-super-jianpin.md`。

use super::input::InputSession;
use crate::types::{KeyResult, LocalInputMode, SchemeType};

/// 后置触发要求的最少字母数。
const MIN_LETTERS: usize = 3;

impl InputSession {
    /// 现在按 `/` 会不会把组字转成超级简拼。`spelling_symbols` 和 `dispatch_character` 共用这一个判断，发布的键就是接受的键。只做内存里的判断，不查库。
    pub(super) fn super_jianpin_slash_ready(&self) -> bool {
        let options = self.local_mode_options;
        if self.scheme() != SchemeType::Shuangpin
            || !options.super_jianpin
            || !options.super_jianpin_slash
            || self.dedicated_english
            || self.local_mode != LocalInputMode::None
            || !self.has_composition()
            || self.conversion.is_some()
            || self
                .caret
                .is_some_and(|caret| caret < self.editing_text_len())
            || self.helpcode_entry.is_some()
            || !self.phrase_progress.word.is_empty()
            || !self.caret_edit.held.is_empty()
            || self.glide_candidates.is_some()
        {
            return false;
        }
        // 有辅助码标记时这里是带标记的原文，所以大写辅码、反引号或 `|` 标记、`'`、微软双拼的 `;` 和九键的数字都在这一步排除。
        let raw = self.raw_with_cases();
        if raw.len() < MIN_LETTERS || !raw.bytes().all(|byte| byte.is_ascii_lowercase()) {
            return false;
        }
        // 与空组字时 `/` 进指令模式同一道门（`local_mode_for_symbol`）：英文标点下 `/` 是字面字符。
        if !self.chinese_punctuation_enabled || self.punctuation_lock == 2 {
            return false;
        }
        // `/` 已经被别的功能占用时让位：万象式辅助码的结束键，辅助码前导键（此刻用不上也让），`/` 出顿号。
        self.shuangpin_helpcode_marker(b'/').is_none()
            && self.helpcode_trigger_key != Some(b'/')
            && !self.punctuation.marks().enumeration_comma_slash
    }

    /// 现在按 `/` 或退格会不会撤回 `/` 转出来的超级简拼：简拼码还是转换时那段原文，光标在末尾。
    pub(super) fn super_jianpin_slash_escape_ready(&self) -> bool {
        self.local_mode == LocalInputMode::SuperJianpin
            && self
                .jianpin_slash_origin
                .as_deref()
                .is_some_and(|origin| self.local_preedit.get(1..) == Some(origin))
            && self
                .caret
                .is_none_or(|caret| caret >= self.editing_text_len())
    }

    /// 本地模式里用户打的原文：`/` 转出来的超级简拼去掉 `J`，那个 `J` 不是用户打的；其余模式就是预编辑。回车上屏和没有匹配时的兜底行都用它。
    pub(super) fn local_raw_text(&self) -> &str {
        if self.local_mode == LocalInputMode::SuperJianpin && self.jianpin_slash_origin.is_some() {
            self.local_preedit.get(1..).unwrap_or_default()
        } else {
            &self.local_preedit
        }
    }

    /// 丢弃组字，以原文进入超级简拼。
    pub(super) fn enter_super_jianpin_slash(&mut self) -> KeyResult {
        let origin = self.raw_with_cases().to_owned();
        self.reset_composition();
        self.local_mode = LocalInputMode::SuperJianpin;
        self.local_preedit = format!("J{origin}");
        self.local_candidates.clear();
        self.plugin_rows = None;
        self.jianpin_slash_chain = self.chain.clone();
        self.chain.reset();
        self.jianpin_slash_origin = Some(origin);
        self.invalidate_online_requests();
        KeyResult::handled().with_diagnostic(self.update_local_candidates())
    }

    /// 退回按 `/` 之前的组字。原组字、提交链（上一个上屏的词）和候选恢复原样，触点数据不恢复。
    pub(super) fn revert_super_jianpin_slash(&mut self) {
        let origin = self.jianpin_slash_origin.take().unwrap_or_default();
        let chain = std::mem::take(&mut self.jianpin_slash_chain);
        self.reset_composition();
        self.chain = chain;
        self.engine.replace_active_raw_input(&origin, &origin);
        self.invalidate_online_requests();
        self.update_mixed_candidates();
    }

    /// `//`：撤回转换，照常上屏首选（学习这次选词）再接上 `/`。没有宿主高亮时这正是今天单按 `/` 的结果；有高亮的宿主经 runtime 走另一条路，见文件头。
    pub(super) fn escape_super_jianpin_slash(&mut self) -> KeyResult {
        self.revert_super_jianpin_slash();
        let mut result = self.finish_composition(0);
        self.chain.reset();
        result.handled = true;
        result.commit.get_or_insert_with(String::new).push('/');
        result
    }
}
