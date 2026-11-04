//! 停顿重解即使保留文字，也必须把新的候选身份交回宿主。

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
fn unchanged_deferred_candidates_publish_selectable_fresh_ids() {
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
    let view = &value["view"];
    assert!(view.is_object(), "候选换代后缺少新身份：{result}");
    assert!(view["generation"].as_u64().unwrap() > before["generation"].as_u64().unwrap());
    assert_eq!(
        view["candidates"][0]["text"],
        before["candidates"][0]["text"]
    );
    let selected = read(msime_client_select(
        handle,
        view["generation"].as_u64().unwrap(),
        0,
    ));
    assert_eq!(selected["value"]["commit"], view["candidates"][0]["text"]);
    assert_eq!(read(msime_client_destroy(handle))["ok"], true);
}
