//! 先于登记到达的取消。宿主按自己取的请求 id 取消流式对话（[`crate::account::ChatStreamRequests`]）或 AI 皮肤生成（[`crate::skin::ai::AiSkinRequests`]）时，取消可能落在「宿主已经发起调用、Rust 还没把 id 登记进表」的那一小段里：这时表里找不到这个 id，若什么也不记，这次取消就丢了，请求照常跑完。两张登记表都用这里记下最近这样到达的 id，同一个 id 随后登记时直接按已取消开始。
//!
//! 只记最近 [`MAX_EARLY_CANCELS`] 个，最旧的先丢：宿主的请求 id 每次都新取（UUID 或随机串），在请求结束以后才到的取消也会留在这里，但不会再有同一个 id 的请求来用它，只占一个位置，到上限被挤掉。所以请求 id 取消过一次以后不要再拿来开始新的请求。

use std::collections::VecDeque;

/// 最多记多少个先到的取消。真正处在「调用了、还没登记」那一段里的同时只有几个，其余都是请求结束以后才到的取消。
const MAX_EARLY_CANCELS: usize = 64;

#[derive(Default)]
pub(crate) struct EarlyCancels(VecDeque<String>);

impl EarlyCancels {
    /// 记下一个在表里找不到的取消；已经记过的不重复记。
    pub(crate) fn remember(&mut self, id: &str) {
        if self.0.iter().any(|known| known == id) {
            return;
        }
        if self.0.len() == MAX_EARLY_CANCELS {
            self.0.pop_front();
        }
        self.0.push_back(id.to_owned());
    }

    /// 登记 `id` 时调用：之前记下过它的取消就消掉这条记录并返回 true，这次请求要按已取消开始。
    pub(crate) fn take(&mut self, id: &str) -> bool {
        match self.0.iter().position(|known| known == id) {
            Some(index) => {
                self.0.remove(index);
                true
            }
            None => false,
        }
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
        assert_eq!(early.0.len(), MAX_EARLY_CANCELS - 1);
    }
}
