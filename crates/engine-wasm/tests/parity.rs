//! WebHost 和桌面宿主的 `msime_input_runtime::Runtime` 必须给出逐行相同的首页（D4）。
//!
//! 需要真实的词库和句子模型，只在设置了 `MSIME_EVAL_RESOURCES` 时运行，否则打印 skipped 后通过。用 release 跑：四个评测集共两万多例。
//!
//! Runtime 一侧照 convert_eval 配置（`prepare_options`、关掉学习和所有本地模式、`sentence_alternatives` 打开、挂上 `sentence-model.safetensors`），只有每页候选数不同：convert_eval 默认取共享偏好的出厂值 6，这里用网页的默认值 9（见 `PAGE_SIZE`）。每例先 `Cancel` + `clear_context` + `seed_context`。WebHost 一侧读同一份暂存出来的工作副本，每例 `reset` + `seed_context_for_eval`。convert_eval 还会在资源里有桌面版的定稿模型时挂上它并在每例末尾 `rerank_settled`；网页不发布那个模型，这里两边都不用它。

#[path = "support/eval.rs"]
mod eval;

use std::path::Path;
use std::sync::Arc;

use msime_engine::host::{prepare_options, Command, Session};
use msime_engine_wasm::host::{Scheme, WebHost};
use msime_input_runtime::{Action, Reranker, Runtime, SentenceModel};

/// 失败时最多打印多少例。
const SHOWN_MISMATCHES: usize = 20;

/// 两边共用的每页候选数。产品宿主把同一个数既交给 runtime 分页，也交给引擎作首屏门槛（host-api 的 `options.first_page_rows = page_size`，`WebHost::with_paths` 同样如此）：全长组的精确同音词不足这么多行时，补全词并进首页。只给 runtime 传分页、引擎留在 `prepare_options` 的默认 6 行，有 6 到 8 个精确同音词的输入两边首页就不同；`Runtime::new` 现在会把分页交给引擎，下面仍在选项里写同一个数，建会话时就对齐。
const PAGE_SIZE: u8 = 9;

fn runtime(resources: &Path, state: &Path, model: &[u8]) -> (Runtime, String) {
    let text = |path: &Path| path.to_str().expect("UTF-8 path").to_owned();
    let mut options = prepare_options(
        &text(resources),
        &text(&state.join("user")),
        &text(&state.join("cache")),
        "web-engine-parity",
    )
    .expect("prepared options");
    options.scheme = 0;
    options.learning = false;
    options.frequency_mode = "disabled".into();
    options.autocorrect_transposition = false;
    options.autocorrect_neighbor = false;
    options.fuzzy_pinyin_rules = 0;
    options.helpcode = false;
    options.show_helpcode = false;
    options.mixed_english = false;
    options.mixed_emoji = false;
    options.mixed_kaomoji = false;
    options.mixed_symbols = false;
    options.local_unicode = false;
    options.local_date_time = false;
    options.local_quick_phrase = false;
    options.local_emoji = false;
    options.local_kaomoji = false;
    options.local_super_jianpin = false;
    options.local_temporary_english = false;
    options.local_temporary_japanese = false;
    // 网页的 `session_options` 把本地模式全部关掉（行内日期时间要读本地时间，在 wasm 上会 panic），所以行内日期时间、日期菜单和其余本地模式这里也要关：只关 `local_date_time` 时，`riqi`、`shijian`、`jintian` 这类输入在 runtime 一侧第 2 格起是当天的日期时间候选，网页没有，而且差异随运行日期变化。
    options.local_date_time_inline = false;
    options.local_date_time_menu = false;
    options.local_expression = false;
    options.local_command = false;
    options.local_mention = false;
    options.local_chaizi = false;
    options.local_symbol = false;
    options.local_shift_letter_entry = false;
    options.sentence_alternatives = true;
    options.first_page_rows = usize::from(PAGE_SIZE);
    let dictionaries = options.dictionaries.clone();
    let mut runtime =
        Runtime::new(Session::new(&options).expect("session"), PAGE_SIZE).expect("runtime");
    runtime.set_reranker(Some(Reranker::new(Arc::new(
        SentenceModel::load(model).expect("sentence model"),
    ))));
    runtime.focus(true).expect("focus");
    (runtime, dictionaries)
}

#[test]
fn webhost_first_page_matches_runtime() {
    let Some(resources) = eval::resources() else {
        println!("skipped: MSIME_EVAL_RESOURCES is not set");
        return;
    };
    let model = std::fs::read(resources.join("sentence-model.safetensors")).expect("model");
    let state = tempfile::tempdir().expect("state directory");
    let (mut runtime, dictionaries) = runtime(&resources, state.path(), &model);
    let user = tempfile::tempdir().expect("user directory");
    let cache = tempfile::tempdir().expect("cache directory");
    let mut host = WebHost::new_with_paths(
        Scheme::Quanpin,
        usize::from(PAGE_SIZE),
        Some(&model),
        Path::new(&dictionaries),
        user.path(),
        cache.path(),
    )
    .expect("web host");
    host.disable_slow_frame_breaker_for_eval();

    let mut cases = 0usize;
    let mut mismatches = Vec::new();
    for set in eval::SETS {
        for case in eval::load(&eval::eval_set(set)) {
            cases += 1;
            runtime
                .dispatch(Action::Command(Command::Cancel))
                .expect("cancel");
            runtime.clear_context();
            runtime.seed_context(&case.context);
            for byte in case.input.bytes() {
                runtime
                    .dispatch(Action::Character {
                        value: byte,
                        shift: false,
                    })
                    .expect("key");
            }
            let expected: Vec<(String, String)> = runtime
                .view()
                .candidates
                .into_iter()
                .map(|candidate| (candidate.text, candidate.code))
                .collect();

            host.reset();
            host.seed_context_for_eval(&case.context);
            let frame = host.keys(&eval::typed(&case.input));
            assert!(frame.model_on, "{set} {}: the model went off", case.id);
            let actual: Vec<(String, String)> = frame
                .page
                .into_iter()
                .map(|row| (row.text, row.code))
                .collect();
            if actual != expected {
                mismatches.push(format!(
                    "{set} {} {:?}\n  runtime: {expected:?}\n  webhost: {actual:?}",
                    case.id, case.input
                ));
            }
        }
    }
    println!("parity: {cases} cases, {} mismatches", mismatches.len());
    assert!(
        mismatches.is_empty(),
        "{} of {cases} cases differ:\n{}",
        mismatches.len(),
        mismatches
            .iter()
            .take(SHOWN_MISMATCHES)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n")
    );
}
