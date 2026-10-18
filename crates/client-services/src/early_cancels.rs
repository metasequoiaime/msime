//! 先于登记到达的取消。宿主按自己取的请求 id 取消流式对话（[`crate::account::ChatStreamRequests`]）或 AI 皮肤生成（[`crate::skin::ai::AiSkinRequests`]）时，取消可能落在「宿主已经发起调用、Rust 还没把 id 登记进表」的那一小段里：这时表里找不到这个 id，若什么也不记，这次取消就丢了，请求照常跑完。两张登记表都用这里记下最近这样到达的 id，同一个 id 随后登记时直接按已取消开始。
//!
//! 表里找不到的取消还有另一种：请求已经结束以后才到的（用户在结束的同一刻点了取消，或者宿主关页面时把记着的 id 全取消一遍）。宿主的请求 id 每次都新取（UUID 或随机串），不会再有同一个 id 的请求来用它，记下它只会占一个位置。所以这里也记最近结束的 [`MAX_FINISHED`] 个 id，取消的是其中一个时不记；否则先到的取消和这些迟到的挤在同一个 [`MAX_EARLY_CANCELS`] 的上限里，几十个迟到的就能把一个真正先到的挤掉，那次请求就照常跑完了。结束得更早、已经不在这份记录里的 id 再被取消时仍会当成先到的记下，只占一个位置，到上限被挤掉。请求 id 不要重复用：结束过的 id 再拿来开始新的请求时，这次请求登记之前到的取消会被当成迟到的、不起作用。

use std::collections::VecDeque;

/// 最多记多少个先到的取消。真正处在「调用了、还没登记」那一段里的同时只有几个。
const MAX_EARLY_CANCELS: usize = 64;
/// 最多记多少个最近结束的 id，结束以后才到的取消按这份记录认出来。
const MAX_FINISHED: usize = 256;

#[derive(Default)]
pub(crate) struct EarlyCancels {
    early: VecDeque<String>,
    finished: VecDeque<String>,
}

impl EarlyCancels {
    /// 记下一个在表里找不到的取消；已经记过的不重复记，最近结束的请求的 id 不记。
    pub(crate) fn remember(&mut self, id: &str) {
        if self.finished.iter().any(|known| known == id)
            || self.early.iter().any(|known| known == id)
        {
            return;
        }
        if self.early.len() == MAX_EARLY_CANCELS {
            self.early.pop_front();
        }
        self.early.push_back(id.to_owned());
    }

    /// 登记 `id` 时调用：之前记下过它的取消就消掉这条记录并返回 true，这次请求要按已取消开始。
    pub(crate) fn take(&mut self, id: &str) -> bool {
        match self.early.iter().position(|known| known == id) {
            Some(index) => {
                self.early.remove(index);
                true
            }
            None => false,
        }
    }

    /// 注销 `id` 时调用：记下它已经结束，之后才到的取消不当成先到的记下。
    pub(crate) fn finish(&mut self, id: &str) {
        if let Some(index) = self.finished.iter().position(|known| known == id) {
            self.finished.remove(index);
        } else if self.finished.len() == MAX_FINISHED {
            self.finished.pop_front();
        }
        self.finished.push_back(id.to_owned());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn remembered_cancels_are_taken_once_and_bounded() {
        let mut early = EarlyCancels::default();
        assert!(!early.take("synthetic-missing"));
        early.remember("synthetic-early");
        early.remember("synthetic-early");
        assert!(early.take("synthetic-early"));
        // 每条记录只用一次，重复记的也只有一条。
        assert!(!early.take("synthetic-early"));
        for index in 0..=MAX_EARLY_CANCELS {
            early.remember(&format!("synthetic-{index}"));
        }
        // 超过上限时最旧的被挤掉，最新的还在。
        assert!(!early.take("synthetic-0"));
        assert!(early.take(&format!("synthetic-{MAX_EARLY_CANCELS}")));
        assert_eq!(early.early.len(), MAX_EARLY_CANCELS - 1);
    }

    #[test]
    fn late_cancels_do_not_push_out_an_early_one() {
        let mut early = EarlyCancels::default();
        early.remember("synthetic-early");
        for index in 0..MAX_EARLY_CANCELS * 2 {
            let id = format!("synthetic-late-{index}");
            assert!(!early.take(&id));
            early.finish(&id);
            early.remember(&id);
        }
        assert_eq!(early.early.len(), 1);
        assert!(early.take("synthetic-early"));
        // 结束的记录也有上限，最旧的先丢。
        for index in 0..=MAX_FINISHED {
            early.finish(&format!("synthetic-finished-{index}"));
        }
        assert_eq!(early.finished.len(), MAX_FINISHED);
        early.remember("synthetic-finished-0");
        assert!(early.take("synthetic-finished-0"));
        early.remember(&format!("synthetic-finished-{MAX_FINISHED}"));
        assert!(!early.take(&format!("synthetic-finished-{MAX_FINISHED}")));
    }
}
