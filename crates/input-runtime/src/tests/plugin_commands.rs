//! 脚本插件的 `/` 指令在 runtime 里的一侧：只在派发类调用、没有暂停、引擎有要问的查询时问提供者；行在同一次派发里写进引擎，代次不变；插件行的上屏不进 AI 上文，失焦时原样上屏；插件的文字只画在插件注释格。
//!
//! 这里的"插件"是一个记录调用、按预设交回行的 [`CommandProvider`]；真正运行脚本、找触发词主人的实现在 host-api 里测。

use super::super::*;
use super::allocations;
use super::{character, generated_mode_runtime};
use std::cell::RefCell;
use std::rc::Rc;

#[derive(Default)]
struct Script {
    /// 每次被问到的（触发词，参数）。
    calls: Vec<(String, String)>,
    /// 交回的行；`None` 表示插件没有运行。
    rows: Option<Vec<(String, String)>>,
}

struct FakeProvider(Rc<RefCell<Script>>);

impl CommandProvider for FakeProvider {
    fn provide(
        &mut self,
        trigger: &str,
        argument: &str,
        out: &mut Vec<(String, String)>,
    ) -> CommandOutcome {
        let mut script = self.0.borrow_mut();
        script.calls.push((trigger.to_owned(), argument.to_owned()));
        match &script.rows {
            Some(rows) => {
                out.extend(rows.iter().cloned());
                CommandOutcome::Ran
            }
            None => CommandOutcome::NotRun,
        }
    }
}

fn rows(rows: &[(&str, &str)]) -> Option<Vec<(String, String)>> {
    Some(
        rows.iter()
            .map(|(text, note)| ((*text).to_owned(), (*note).to_owned()))
            .collect(),
    )
}

fn uuid_commands() -> Vec<PluginCommand> {
    vec![PluginCommand {
        trigger: "uuid".into(),
        title: "UUID".into(),
    }]
}

/// 开着 `/` 指令、挂着插件指令 `uuid` 和假的提供者的 runtime。
fn plugin_runtime(
    directory: &std::path::Path,
    answer: Option<Vec<(String, String)>>,
) -> (Runtime, Rc<RefCell<Script>>) {
    let mut runtime = generated_mode_runtime(directory);
    runtime.set_plugin_commands(&uuid_commands()).unwrap();
    let script = Rc::new(RefCell::new(Script {
        calls: Vec::new(),
        rows: answer,
    }));
    runtime.set_command_provider(Some(Box::new(FakeProvider(Rc::clone(&script)))));
    (runtime, script)
}

fn type_text(runtime: &mut Runtime, text: &str) -> Transition {
    let mut last = None;
    for value in text.bytes() {
        last = Some(character(runtime, value));
    }
    last.expect("some text")
}

fn texts(view: &View) -> Vec<&str> {
    view.candidates
        .iter()
        .map(|candidate| candidate.text.as_str())
        .collect()
}

/// 视图里和身份无关的部分：会话号、代次和候选 ID 两个 runtime 之间本来就不同。
fn shape(view: &View) -> serde_json::Value {
    fn strip(value: &mut serde_json::Value) {
        match value {
            serde_json::Value::Object(map) => {
                for key in ["id", "session", "generation"] {
                    map.remove(key);
                }
                map.values_mut().for_each(strip);
            }
            serde_json::Value::Array(items) => items.iter_mut().for_each(strip),
            _ => {}
        }
    }
    let mut value = serde_json::to_value(view).unwrap();
    strip(&mut value);
    value
}

/// 有插件指令、没有提供者时，`/` 模式的视图和没有插件指令时一模一样：引擎只多出一个不会被问到的查询。
#[test]
fn without_a_provider_the_command_mode_views_are_unchanged() {
    let directory = tempfile::tempdir().unwrap();
    let mut plain = generated_mode_runtime(directory.path());
    let other = tempfile::tempdir().unwrap();
    let mut with_commands = generated_mode_runtime(other.path());
    with_commands.set_plugin_commands(&uuid_commands()).unwrap();
    let keys = || -> Vec<Action> {
        let letter = |value: u8| Action::Character {
            value,
            shift: false,
        };
        let mut keys: Vec<Action> = b"/uuidx".iter().map(|value| letter(*value)).collect();
        keys.extend([
            Action::Command(Command::Backspace),
            Action::NextCandidate,
            Action::Command(Command::MoveLeft),
            Action::Command(Command::Cancel),
            letter(b'/'),
            letter(b'r'),
            letter(b'q'),
            Action::Finish,
        ]);
        keys
    };
    for (index, (left, right)) in keys().into_iter().zip(keys()).enumerate() {
        let left = plain.dispatch(left).unwrap();
        let right = with_commands.dispatch(right).unwrap();
        assert_eq!(left.commit, right.commit, "key {index}");
        assert_eq!(shape(&left.view), shape(&right.view), "key {index}");
    }
}

/// 打完触发词的那一次派发里问插件、写行：代次和没有插件时一样只前进一次，插件行的 ID 当场就能选；引擎注释为空，插件注释是「标题 注释」。
#[test]
fn rows_arrive_in_the_same_dispatch_and_are_selectable() {
    let directory = tempfile::tempdir().unwrap();
    let (mut runtime, script) =
        plugin_runtime(directory.path(), rows(&[("a-b", "小写"), ("A-B", "")]));
    type_text(&mut runtime, "/uui");
    assert!(script.borrow().calls.is_empty());
    let before = runtime.generation();
    let typed = character(&mut runtime, b'd');
    assert_eq!(runtime.generation(), before + 1);
    assert_eq!(script.borrow().calls, [("uuid".to_owned(), String::new())]);
    let view = typed.view;
    assert_eq!(texts(&view), ["a-b", "A-B"]);
    let plugin = msime_engine::CandidateSource::Plugin as u8;
    assert!(view.candidates.iter().all(|row| row.source == plugin));
    assert!(view.candidates.iter().all(|row| row.annotation.is_empty()));
    assert_eq!(
        view.candidates[0].script_annotation.as_deref(),
        Some("UUID 小写")
    );
    assert_eq!(
        view.candidates[1].script_annotation.as_deref(),
        Some("UUID")
    );
    assert_eq!(view.candidates[0].id.generation, runtime.generation());
    let picked = runtime
        .dispatch(Action::Select(view.candidates[1].id))
        .unwrap();
    assert_eq!(picked.commit.as_deref(), Some("A-B"));
    let context = picked.commit_context.expect("a commit context");
    assert!(context.plugin_row);
    assert!(!context.typing_statistics);
    assert_eq!(picked.view.local_mode, "none");
}

/// 方向键、翻页和旁路调用都不再问插件；预编辑串变了再变回来，重新问。
#[test]
fn the_provider_runs_only_in_dispatch_and_once_per_preedit() {
    let directory = tempfile::tempdir().unwrap();
    let (mut runtime, script) = plugin_runtime(directory.path(), rows(&[("甲", "")]));
    type_text(&mut runtime, "/uuid");
    assert_eq!(script.borrow().calls.len(), 1);
    for action in [
        Action::NextCandidate,
        Action::PreviousCandidate,
        Action::NextPage,
        Action::Command(Command::MoveLeft),
        Action::Command(Command::MoveRight),
    ] {
        let view = runtime.dispatch(action).unwrap().view;
        assert_eq!(texts(&view), ["甲"]);
    }
    runtime.all_candidates();
    let generation = runtime.generation();
    let _ = runtime.apply_translations(generation, [("甲".to_owned(), "first".to_owned())]);
    assert_eq!(texts(&runtime.view()), ["甲"]);
    assert_eq!(script.borrow().calls.len(), 1);

    runtime
        .dispatch(Action::Command(Command::Backspace))
        .unwrap();
    assert_eq!(texts(&runtime.view()), ["/uui"]);
    character(&mut runtime, b'd');
    assert_eq!(script.borrow().calls.len(), 2);
    assert_eq!(texts(&runtime.view()), ["甲"]);
}

/// 旁路调用遇到还没问过的预编辑串也不问，只有下一次派发问。
#[test]
fn bypass_refreshes_never_ask_the_provider() {
    let directory = tempfile::tempdir().unwrap();
    let (mut runtime, script) = plugin_runtime(directory.path(), None);
    type_text(&mut runtime, "/uuid");
    // 插件没运行：只有兜底行，查询还在。
    assert_eq!(script.borrow().calls.len(), 1);
    assert_eq!(texts(&runtime.view()), ["/uuid"]);
    script.borrow_mut().rows = rows(&[("甲", "")]);
    runtime.set_command_table(&[]).unwrap();
    runtime.set_plugin_commands(&uuid_commands()).unwrap();
    runtime.all_candidates();
    let generation = runtime.generation();
    let _ = runtime.apply_translations(generation, []);
    assert_eq!(script.borrow().calls.len(), 1);
    assert_eq!(texts(&runtime.view()), ["/uuid"]);
    // 下一次派发再问，这次插件运行了。
    runtime
        .dispatch(Action::Command(Command::MoveLeft))
        .unwrap();
    assert_eq!(script.borrow().calls.len(), 2);
    assert_eq!(texts(&runtime.view()), ["甲"]);
}

/// 插件没运行不写缓存，下一次派发再问；运行了、给了 0 行就缓存成空表，同一份预编辑串不再问。
#[test]
fn not_run_is_asked_again_and_zero_rows_are_not() {
    let directory = tempfile::tempdir().unwrap();
    let (mut runtime, script) = plugin_runtime(directory.path(), None);
    type_text(&mut runtime, "/uuid");
    runtime
        .dispatch(Action::Command(Command::MoveLeft))
        .unwrap();
    assert_eq!(script.borrow().calls.len(), 2);
    script.borrow_mut().rows = rows(&[]);
    runtime
        .dispatch(Action::Command(Command::MoveRight))
        .unwrap();
    assert_eq!(script.borrow().calls.len(), 3);
    assert_eq!(texts(&runtime.view()), ["/uuid"]);
    runtime
        .dispatch(Action::Command(Command::MoveLeft))
        .unwrap();
    assert_eq!(script.borrow().calls.len(), 3);
}

/// 暂停期间不问插件；恢复之后的下一次派发再问。
#[test]
fn a_suspended_runtime_does_not_ask() {
    let directory = tempfile::tempdir().unwrap();
    let (mut runtime, script) = plugin_runtime(directory.path(), rows(&[("甲", "")]));
    runtime.suspend_candidate_transform(true);
    type_text(&mut runtime, "/uuid");
    assert!(script.borrow().calls.is_empty());
    assert_eq!(texts(&runtime.view()), ["/uuid"]);
    runtime.suspend_candidate_transform(false);
    runtime
        .dispatch(Action::Command(Command::MoveLeft))
        .unwrap();
    assert_eq!(script.borrow().calls.len(), 1);
    assert_eq!(texts(&runtime.view()), ["甲"]);
}

/// 去掉提供者之后什么都不问，缓存的行仍由引擎保留到预编辑串变化为止。
#[test]
fn removing_the_provider_stops_the_questions() {
    let directory = tempfile::tempdir().unwrap();
    let (mut runtime, script) = plugin_runtime(directory.path(), rows(&[("甲", "")]));
    runtime.set_command_provider(None);
    type_text(&mut runtime, "/uuid");
    assert!(script.borrow().calls.is_empty());
    assert_eq!(texts(&runtime.view()), ["/uuid"]);
}

/// 插件行的上屏不进 AI 和重排的上文；内置指令的上屏照旧进。
#[test]
fn plugin_commits_stay_out_of_the_ai_context() {
    let directory = tempfile::tempdir().unwrap();
    let (mut runtime, _script) = plugin_runtime(directory.path(), rows(&[("0f8fad5b-d9cb", "")]));
    type_text(&mut runtime, "/rq");
    let date = runtime.dispatch(Action::SelectHighlighted).unwrap();
    let date = date.commit.expect("a date");
    assert!(
        runtime.ai_context.ends_with(&date),
        "{}",
        runtime.ai_context
    );
    let before = runtime.ai_context.clone();
    type_text(&mut runtime, "/uuid");
    let picked = runtime.dispatch(Action::SelectHighlighted).unwrap();
    assert_eq!(picked.commit.as_deref(), Some("0f8fad5b-d9cb"));
    assert_eq!(runtime.ai_context, before);
    assert!(!runtime.ai_context.contains("0f8fad5b"));
}

/// 插件注释只拥有最终结果，不先复制完整标题和注释。
#[test]
fn plugin_annotation_reads_only_allocate_the_final_text() {
    let directory = tempfile::tempdir().unwrap();
    let long = "注".repeat(40);
    let (mut runtime, _script) = plugin_runtime(directory.path(), rows(&[("甲", &long)]));
    type_text(&mut runtime, "/uuid");
    let (annotation, allocations) = allocations::count(|| runtime.script_annotation(0));
    let expected = format!("UUID {}", "注".repeat(27));
    assert_eq!(annotation.as_deref(), Some(expected.as_str()));
    assert_eq!(
        allocations, 1,
        "插件注释读取只应申请最终字符串：{allocations}"
    );
}

/// 插件注释格最多 32 个 UTF-16 码元，在字符边界上截断；含控制字符的注释不显示，只剩标题。
#[test]
fn plugin_annotations_are_cut_to_the_annotation_limit() {
    let directory = tempfile::tempdir().unwrap();
    let long = "注".repeat(40);
    let (mut runtime, _script) = plugin_runtime(
        directory.path(),
        rows(&[
            ("甲", &long),
            ("乙", "a\nb"),
            ("丙", "😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀😀"),
        ]),
    );
    let view = type_text(&mut runtime, "/uuid").view;
    let first = view.candidates[0].script_annotation.clone().unwrap();
    assert_eq!(first.encode_utf16().count(), MAX_ANNOTATION_UTF16);
    assert_eq!(first, format!("UUID {}", "注".repeat(27)));
    assert_eq!(
        view.candidates[1].script_annotation.as_deref(),
        Some("UUID")
    );
    // 一个 emoji 两个码元：放不下的那一半不切开。
    let third = view.candidates[2].script_annotation.clone().unwrap();
    assert_eq!(third, format!("UUID {}", "😀".repeat(13)));
    assert_eq!(third.encode_utf16().count(), 31);
}

/// 宿主交来的表里触发词重复、或者前一条的标题不合规时，行旁画的是引擎留下的那一条的标题：重复的后来者不能把先来者的行标成自己的名字，被引擎丢掉的标题也不显示。
#[test]
fn the_title_beside_a_plugin_row_is_the_one_the_engine_kept() {
    let command = |trigger: &str, title: &str| PluginCommand {
        trigger: trigger.into(),
        title: title.into(),
    };
    let directory = tempfile::tempdir().unwrap();
    let (mut runtime, _script) = plugin_runtime(directory.path(), rows(&[("甲", "")]));
    runtime
        .set_plugin_commands(&[command("uuid", "UUID"), command("uuid", "EVIL")])
        .unwrap();
    let view = type_text(&mut runtime, "/uuid").view;
    assert_eq!(texts(&view), ["甲"]);
    assert_eq!(
        view.candidates[0].script_annotation.as_deref(),
        Some("UUID")
    );

    // 第一条的标题含控制字符，引擎丢掉它、留下第二条。
    runtime.dispatch(Action::Command(Command::Cancel)).unwrap();
    runtime
        .set_plugin_commands(&[command("uuid", "A\tB"), command("uuid", "UUID")])
        .unwrap();
    let view = type_text(&mut runtime, "/uuid").view;
    assert_eq!(texts(&view), ["甲"]);
    assert_eq!(
        view.candidates[0].script_annotation.as_deref(),
        Some("UUID")
    );
}

/// 失焦（命令 20）时高亮的是插件行：原样上屏用户打的 `/uuid`。按键结束和标点照旧上屏高亮行；高亮的不是插件行时，失焦和结束组字一样。
#[test]
fn focus_loss_commits_the_typed_command_instead_of_a_plugin_row() {
    let directory = tempfile::tempdir().unwrap();
    let (mut runtime, _script) = plugin_runtime(directory.path(), rows(&[("甲乙", "")]));
    type_text(&mut runtime, "/uuid");
    let raw = runtime.dispatch(Action::FinishOnFocusLoss).unwrap();
    assert_eq!(raw.commit.as_deref(), Some("/uuid"));
    assert!(!raw.commit_context.expect("a commit context").plugin_row);
    assert_eq!(raw.view.local_mode, "none");

    type_text(&mut runtime, "/uuid");
    let finished = runtime.dispatch(Action::Finish).unwrap();
    assert_eq!(finished.commit.as_deref(), Some("甲乙"));
    assert!(
        finished
            .commit_context
            .expect("a commit context")
            .plugin_row
    );

    type_text(&mut runtime, "/uuid");
    let punctuated = runtime.dispatch(Action::Punctuation(b',')).unwrap();
    assert!(
        punctuated
            .commit
            .as_deref()
            .is_some_and(|commit| commit.starts_with("甲乙") && commit != "甲乙"),
        "{punctuated:?}"
    );

    // `/rq` 的高亮行不是插件行：失焦和结束组字上屏的是同一行。
    type_text(&mut runtime, "/rq");
    let lost = runtime.dispatch(Action::FinishOnFocusLoss).unwrap();
    type_text(&mut runtime, "/rq");
    let done = runtime.dispatch(Action::Finish).unwrap();
    assert!(lost.commit.is_some());
    assert_eq!(lost.commit, done.commit);
    assert!(!lost.commit_context.expect("a commit context").plugin_row);
}

/// 连续两次重新查询同一插件指令时，行容器应复用第一次提供者调用的容量。
#[test]
fn repeated_command_queries_reuse_the_output_rows_buffer() {
    struct BufferProbe {
        provider: FakeProvider,
        reservations: Rc<RefCell<Vec<usize>>>,
    }

    impl CommandProvider for BufferProbe {
        fn provide(
            &mut self,
            trigger: &str,
            argument: &str,
            out: &mut Vec<(String, String)>,
        ) -> CommandOutcome {
            // 只统计回包行容器的容量申请，不把候选字符串或视图的分配算进来。
            let (_, count) = allocations::count(|| out.reserve(1));
            self.reservations.borrow_mut().push(count);
            self.provider.provide(trigger, argument, out)
        }
    }

    let directory = tempfile::tempdir().unwrap();
    let (mut runtime, script) = plugin_runtime(directory.path(), rows(&[("甲", "注释")]));
    let reservations = Rc::new(RefCell::new(Vec::new()));
    runtime.set_command_provider(Some(Box::new(BufferProbe {
        provider: FakeProvider(Rc::clone(&script)),
        reservations: Rc::clone(&reservations),
    })));
    type_text(&mut runtime, "/uuid");
    assert_eq!(script.borrow().calls.len(), 1);
    assert_eq!(*reservations.borrow(), [1]);
    for calls in 2..=3 {
        runtime
            .dispatch(Action::Command(Command::Backspace))
            .unwrap();
        let transition = character(&mut runtime, b'd');
        assert_eq!(script.borrow().calls.len(), calls);
        assert_eq!(texts(&transition.view), ["甲"]);
        assert_eq!(
            transition.view.candidates[0].script_annotation.as_deref(),
            Some("UUID 注释")
        );
        assert_eq!(
            reservations.borrow()[calls - 1],
            0,
            "预热后重新申请了回包行容量"
        );
    }
}
