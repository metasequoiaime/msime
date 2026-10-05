//! 移动端 AI 皮肤生成任务的登记与取消。

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};

#[derive(Default)]
pub(crate) struct AiSkinRequests(Mutex<HashMap<String, Arc<AtomicBool>>>);

impl AiSkinRequests {
    pub(crate) fn begin(&self, id: &str) -> Result<Arc<AtomicBool>, &'static str> {
        let mut requests = self.0.lock().map_err(|_| "ai_skin_unavailable")?;
        // 拒绝重复 id 时保留原任务的取消标记。
        if requests.contains_key(id) {
            return Err("ai_skin_busy");
        }
        let cancelled = Arc::new(AtomicBool::new(false));
        requests.insert(id.to_owned(), Arc::clone(&cancelled));
        Ok(cancelled)
    }

    pub(crate) fn cancel(&self, id: &str) -> Result<(), &'static str> {
        let requests = self.0.lock().map_err(|_| "ai_skin_unavailable")?;
        if let Some(cancelled) = requests.get(id) {
            cancelled.store(true, Ordering::Release);
        }
        Ok(())
    }

    pub(crate) fn finish(&self, id: &str) {
        if let Ok(mut requests) = self.0.lock() {
            requests.remove(id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_request_keeps_original_generation_cancellable() {
        let requests = AiSkinRequests::default();
        let original = requests.begin("synthetic-request").unwrap();
        assert_eq!(
            requests.begin("synthetic-request").unwrap_err(),
            "ai_skin_busy"
        );
        requests.cancel("synthetic-request").unwrap();
        assert!(original.load(Ordering::Acquire));
    }

    #[test]
    fn cancelling_one_request_does_not_cancel_another() {
        let requests = AiSkinRequests::default();
        let first = requests.begin("synthetic-first").unwrap();
        let second = requests.begin("synthetic-second").unwrap();
        requests.cancel("synthetic-missing").unwrap();
        requests.cancel("synthetic-first").unwrap();
        assert!(first.load(Ordering::Acquire));
        assert!(!second.load(Ordering::Acquire));
    }

    #[test]
    fn finished_request_id_can_be_reused_with_a_new_cancel_flag() {
        let requests = AiSkinRequests::default();
        let first = requests.begin("synthetic-request").unwrap();
        requests.finish("synthetic-request");
        let second = requests.begin("synthetic-request").unwrap();
        requests.cancel("synthetic-request").unwrap();
        assert!(!first.load(Ordering::Acquire));
        assert!(second.load(Ordering::Acquire));
    }
}
