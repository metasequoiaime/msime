//! 句子模型把某一行提到首位之后，标点和空格上屏的必须是玩家看到的那个首位，而不是引擎自己的第 0 个候选：选择要经过 `engine_order` 座位映射。
//!
//! 需要真实的句子模型，只在设置了 `MSIME_EVAL_RESOURCES` 时运行，否则打印 skipped 后通过。

#[path = "support/eval.rs"]
mod eval;

use std::path::Path;

use msime_engine_wasm::host::{Key, Out, Scheme, WebHost};

fn host(
    dictionaries: &Path,
    model: Option<&[u8]>,
) -> (WebHost, tempfile::TempDir, tempfile::TempDir) {
    let user = tempfile::tempdir().expect("user directory");
    let cache = tempfile::tempdir().expect("cache directory");
    let mut host = WebHost::new_with_paths(
        Scheme::Quanpin,
        9,
        model,
        dictionaries,
        user.path(),
        cache.path(),
    )
    .expect("web host");
    host.disable_slow_frame_breaker_for_eval();
    (host, user, cache)
}

#[test]
fn a_reranked_first_seat_is_what_punctuation_commits() {
    let Some(resources) = eval::resources() else {
        println!("skipped: MSIME_EVAL_RESOURCES is not set");
        return;
    };
    let model = std::fs::read(resources.join("sentence-model.safetensors")).expect("model");
    let state = tempfile::tempdir().expect("state directory");
    let dictionaries = eval::stage(&resources, state.path());
    let (mut reranked, _user, _cache) = host(&dictionaries, Some(&model));
    let (mut plain, _plain_user, _plain_cache) = host(&dictionaries, None);

    // 第一个被模型改了首位的句子。
    let case = eval::load(&eval::eval_set("sentences-v1.tsv"))
        .into_iter()
        .find(|case| {
            let keys = eval::typed(&case.input);
            plain.reset();
            plain.seed_context_for_eval(&case.context);
            let engine_first = plain.keys(&keys).page.first().map(|row| row.text.clone());
            reranked.reset();
            reranked.seed_context_for_eval(&case.context);
            let model_first = reranked
                .keys(&keys)
                .page
                .first()
                .map(|row| row.text.clone());
            engine_first.is_some() && model_first != engine_first
        })
        .expect("the model reorders at least one sentences-v1 case");

    for (finish, mark) in [(Key::Punct(b'.'), Some("。")), (Key::Space, None)] {
        reranked.reset();
        reranked.seed_context_for_eval(&case.context);
        let shown = reranked.keys(&eval::typed(&case.input));
        assert!(shown.model_on);
        let first = shown.page[0].text.clone();
        let mut expected = vec![Out::Commit {
            text: first,
            seat: 0,
        }];
        if let Some(mark) = mark {
            expected.push(Out::Commit {
                text: mark.to_owned(),
                seat: -1,
            });
        }
        let frame = reranked.keys(&[finish]);
        assert_eq!(
            frame.out, expected,
            "{} {:?} {finish:?}",
            case.id, case.input
        );
    }
}
