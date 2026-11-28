//! 停顿重解与候选身份：候选变了才换代、交回新视图；什么都没变时代次不动，宿主手上画着的身份照旧选得中（#11819）。

use super::*;

struct UnchangedScorer;

impl msime_input_runtime::CandidateScorer for UnchangedScorer {
    fn context_length(&self) -> usize {
        64
    }

    fn log_probabilities(&mut self, _: &str, candidates: &[&str]) -> Vec<f32> {
        vec![-5.0; candidates.len()]
    }
}

#[test]
fn unchanged_deferred_candidates_keep_the_drawn_ids() {
    let directory = tempfile::tempdir().unwrap();
    let handle = test_host_with_pinyin_fixture(directory.path(), chinese_preferences());
    read(msime_client_focus(handle, true));
    let before = type_letters(handle, b"nihao");
    SESSIONS.with(|sessions| {
        let mut sessions = sessions.borrow_mut();
        let session = sessions.get_mut(&handle).unwrap();
        session.keyboard_reranker_loading = None;
        session
            .runtime
            .set_deferred_scorer(Some(Box::new(UnchangedScorer)));
    });
    let result = read(msime_client_start_settled_rerank(handle));
    assert_eq!(result["ok"], true, "{result}");
    let value = &result["value"];
    assert_eq!(value["status"], "done");
    assert_eq!(value["moved"], false, "{result}");
    // 挂着延后的模型：宿主据此在打字的空隙里就起跑。
    assert_eq!(value["deferred"], true, "{result}");
    // 候选没变：不换代，也就没有新视图要宿主收。
    assert!(value.get("view").is_none(), "候选没变却换了代：{result}");
    let selected = read(msime_client_select(
        handle,
        before["generation"].as_u64().unwrap(),
        0,
    ));
    assert_eq!(selected["ok"], true, "{selected}");
    assert_eq!(selected["value"]["commit"], before["candidates"][0]["text"]);
    assert_eq!(read(msime_client_destroy(handle))["ok"], true);
}

/// 记下自己被调了几次的打分器：看得出模型是在打字时跑的，还是在上屏前补做的重解里跑的。
struct CountingScorer(std::sync::Arc<std::sync::atomic::AtomicUsize>);

impl msime_input_runtime::CandidateScorer for CountingScorer {
    fn context_length(&self) -> usize {
        64
    }

    fn log_probabilities(&mut self, _: &str, candidates: &[&str]) -> Vec<f32> {
        self.0.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        vec![-5.0; candidates.len()]
    }
}

fn defer(handle: u64, scorer: Box<dyn msime_input_runtime::CandidateScorer>) {
    SESSIONS.with(|sessions| {
        let mut sessions = sessions.borrow_mut();
        let session = sessions.get_mut(&handle).unwrap();
        session.keyboard_reranker_loading = None;
        session.runtime.set_deferred_scorer(Some(scorer));
    });
}

/// 打完拼音马上按空格时停顿重解还没跑：宿主先调 `msime_client_finish_pause_rerank`，它把当前组字的停顿重解做完（打字时模型一次都没跑）；候选变了交回新代次的视图，没变就不换代、不带视图，宿主照画着的高亮项上屏，上屏的都是用户看到的那一行。已经解完时再调立即返回、不再跑模型、不再换代，没挂延后的模型时什么都不做。
#[test]
fn finishing_the_pause_rerank_hands_back_the_view_to_commit_from() {
    let directory = tempfile::tempdir().unwrap();
    let handle = test_host_with_pinyin_fixture(directory.path(), chinese_preferences());
    read(msime_client_focus(handle, true));
    // 没挂延后的模型：什么都不做。
    type_letters(handle, b"ni");
    let none = read(msime_client_finish_pause_rerank(handle));
    assert_eq!(none["ok"], true, "{none}");
    assert_eq!(none["value"]["status"], "none", "{none}");
    assert_eq!(none["value"]["deferred"], false, "{none}");

    let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    defer(handle, Box::new(CountingScorer(calls.clone())));
    let before = type_letters(handle, b"hao");
    let count = || calls.load(std::sync::atomic::Ordering::SeqCst);
    assert_eq!(count(), 0, "延后的模型不在字母键上跑");

    let finished = read(msime_client_finish_pause_rerank(handle));
    assert_eq!(finished["ok"], true, "{finished}");
    let value = &finished["value"];
    assert_eq!(value["status"], "done", "{finished}");
    assert_eq!(value["deferred"], true, "{finished}");
    // 这份小词库里首选是词库的确定结果，重排在调模型之前就返回，候选与不挂模型时相同：不换代、不带视图，宿主画着的身份照旧有效。带着模型重解、模型参与排序和换代由 input-runtime 的 `settle_deferred_scorer` 测试覆盖。
    assert_eq!(value["moved"], false, "{finished}");
    assert!(value.get("view").is_none(), "{finished}");

    // 已经解完（模型还挂在按键的位置上）：立即返回，不再跑模型、不再换代。
    let ran = count();
    let again = read(msime_client_finish_pause_rerank(handle));
    assert_eq!(again["value"]["status"], "done", "{again}");
    assert_eq!(again["value"]["moved"], false, "{again}");
    assert!(again["value"].get("view").is_none(), "{again}");
    assert_eq!(count(), ran);

    let selected = read(msime_client_select(
        handle,
        before["generation"].as_u64().unwrap(),
        0,
    ));
    assert_eq!(
        selected["value"]["commit"], before["candidates"][0]["text"],
        "{selected}"
    );
    assert_eq!(read(msime_client_destroy(handle))["ok"], true);
}
