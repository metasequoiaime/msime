//! 双拼组字末尾按 `/` 转超级简拼（#10860）在 runtime 里的一半：宿主高亮。以前单按 `/` 上屏的是宿主高亮的那一行（[`Runtime::finish_with_punctuation`]），而高亮、重排和插件插进来的行都只在 runtime 里，引擎不知道。所以 `/` 转换时在这里记下转换前高亮的座位和候选，退格撤回时把高亮放回那一行，`//` 也先撤回、再从那一行结束组字并接上 `/`，结果与以前单按 `/` 相同。
//!
//! 不经过 runtime 的调用方（C ABI、golden 回放）没有高亮，引擎自己的 `//`（`msime_engine` 的 `escape_super_jianpin_slash`）照旧上屏首选。取舍见 `.agents/notes/implemented/feature/2026-11-05-shuangpin-postfix-super-jianpin.md`。

use super::*;

/// 超级简拼在快照里的模式名（`LocalInputMode::SuperJianpin`）。
const SUPER_JIANPIN: &str = "super_jianpin";

/// `/` 把组字转成超级简拼的那一刻：转换前的组字原文，和宿主当时高亮的座位、候选文字与编码。
pub(crate) struct SlashHighlight {
    origin: String,
    seat: usize,
    text: String,
    code: Option<String>,
}

fn is_slash(action: &Action) -> bool {
    matches!(
        action,
        Action::Punctuation(b'/') | Action::Character { value: b'/', .. }
    )
}

impl<E: InputEngine> Runtime<E> {
    /// 这一键可能把组字转成超级简拼时（`/`，普通组字，引擎正发布 `/`），记下此刻的组字和高亮；转没转成由派发后的 [`Self::settle_jianpin_slash`] 核对。
    pub(super) fn jianpin_slash_from(&self, action: &Action) -> Option<SlashHighlight> {
        if !is_slash(action)
            || self.cached.local_mode != "none"
            || self.cached.editing_text.is_empty()
            || !self.cached.spelling_symbols.contains('/')
        {
            return None;
        }
        Some(SlashHighlight {
            origin: self.cached.editing_text.clone(),
            seat: self.highlighted,
            text: self
                .cached
                .candidates
                .get(self.highlighted)
                .cloned()
                .unwrap_or_default(),
            code: self.cached.candidate_codes.get(self.highlighted).cloned(),
        })
    }

    /// 此刻按 `/` 或退格会撤回 `/` 转出来的超级简拼：转换时记下过高亮，简拼码还是转换前的原文，引擎也仍发布 `/`（光标在末尾，与引擎的 `super_jianpin_slash_escape_ready` 一致；J 模式本身不拿 `/` 拼写）。
    pub(super) fn jianpin_slash_escapes(&self) -> bool {
        self.jianpin_slash.as_ref().is_some_and(|saved| {
            self.cached.local_mode == SUPER_JIANPIN
                && self.cached.editing_text.strip_prefix('J') == Some(saved.origin.as_str())
                && self.cached.spelling_symbols.contains('/')
        })
    }

    /// 派发并刷新之后：`/` 转成了超级简拼就留下转换前的高亮；退格撤回了转换就把高亮放回那一行；已经不在超级简拼里就忘掉。
    pub(super) fn settle_jianpin_slash(&mut self, from: Option<SlashHighlight>, undo: bool) {
        if undo && self.cached.local_mode == "none" {
            if let Some(saved) = self.jianpin_slash.take() {
                if self.cached.editing_text == saved.origin {
                    self.restore_jianpin_slash_highlight(&saved);
                }
            }
            return;
        }
        if let Some(saved) = from {
            if self.cached.local_mode == SUPER_JIANPIN
                && self.cached.editing_text.strip_prefix('J') == Some(saved.origin.as_str())
            {
                self.jianpin_slash = Some(saved);
                return;
            }
        }
        if self.cached.local_mode != SUPER_JIANPIN {
            self.jianpin_slash = None;
        }
    }

    /// `//`：经引擎的退格撤回转换，高亮放回转换前那一行，再照普通标点从它结束组字、接上 `/`，正是以前单按 `/` 的结果。
    pub(super) fn escape_jianpin_slash(&mut self) -> Result<EngineResult, RuntimeError> {
        let saved = self.jianpin_slash.take();
        self.engine.command(Command::Backspace)?;
        self.refresh()?;
        if let Some(saved) = saved {
            self.restore_jianpin_slash_highlight(&saved);
        }
        self.finish_with_punctuation(b'/')
    }

    /// 撤回后的候选与转换前相同时高亮回到原座位；变了（比如云候选没了）就按文字和编码找回那一行，找不到停在原来那一页的开头。
    fn restore_jianpin_slash_highlight(&mut self, saved: &SlashHighlight) {
        if self.cached.candidates.get(saved.seat) == Some(&saved.text) {
            self.highlighted = saved.seat.min(self.visible_len().saturating_sub(1));
        } else {
            self.keep_highlight_on_same_candidate(saved.seat, &saved.text, saved.code.as_deref());
        }
    }
}
