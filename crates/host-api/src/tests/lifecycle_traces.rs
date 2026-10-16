//! 生命周期核心经 C ABI 的黄金轨迹与不变量：宿主照契约调用，记录下来的事件序列就是这里写的样子。

use super::*;
use crate::lifecycle::{
    Call, CommitOrigin, Cue, InputClass, LifecycleEvent, COMMIT_OBSERVE_ONLY, CUES, FEEDBACK,
    HOST_HOLDS_PIECES, SUPPORTED_FLAGS, TRAIT_KNOWN, TRAIT_SECURE,
};
use msime_client_services::plugins::EffectStyle;
use msime_input_runtime::{SelectionHow, SelectionOutcome};
use std::cell::RefCell;
use std::rc::Rc;

type Trace = Rc<RefCell<Vec<LifecycleEvent>>>;

/// 让会话选择加入并挂上记录事件的测试消费者。
fn record(handle: u64, flags: u32) -> Trace {
    assert_eq!(
        msime_client_set_lifecycle(handle, flags),
        flags & SUPPORTED_FLAGS
    );
    let trace = Trace::default();
    let sink = trace.clone();
    SESSIONS.with(|sessions| {
        let mut sessions = sessions.borrow_mut();
        let state = sessions
            .get_mut(&handle)
            .unwrap()
            .lifecycle
            .as_deref_mut()
            .unwrap();
        state.probe = Some(Box::new(move |event| sink.borrow_mut().push(*event)));
    });
    trace
}

/// 把事件写成短名字，黄金轨迹按名字比较。
fn name(event: &LifecycleEvent) -> String {
    match event {
        LifecycleEvent::FocusIn => "focus_in".into(),
        LifecycleEvent::FocusOut => "focus_out".into(),
        LifecycleEvent::TraitsChanged { .. } => "traits".into(),
        LifecycleEvent::Activated => "activated".into(),
        LifecycleEvent::Deactivated => "deactivated".into(),
        LifecycleEvent::KeyDown(_) => "key".into(),
        LifecycleEvent::Input { class, .. } => match class {
            InputClass::Cancel => "input{Cancel}".into(),
            InputClass::HostCommit => "input{HostCommit}".into(),
            _ => "input".into(),
        },
        LifecycleEvent::SelectionRetracted(retraction) => format!("retracted{{{retraction:?}}}"),
        LifecycleEvent::CandidateSelected(selection) => format!("selected{{{:?}}}", selection.how),
        LifecycleEvent::BeforeCommit { .. } => "before_commit".into(),
        LifecycleEvent::Commit {
            origin, continues, ..
        } => {
            if *continues {
                format!("commit{{{origin:?},continues}}")
            } else {
                format!("commit{{{origin:?}}}")
            }
        }
        LifecycleEvent::CompositionEnd(reason) => format!("end{{{reason:?}}}"),
        LifecycleEvent::CompositionStart => "start".into(),
        LifecycleEvent::CompositionUpdate => "update".into(),
        LifecycleEvent::CandidatesShown { .. } => "shown".into(),
        LifecycleEvent::CandidatesHidden(reason) => format!("hidden{{{reason:?}}}"),
        LifecycleEvent::SessionEnd => "session_end".into(),
    }
}

fn names(trace: &Trace) -> Vec<String> {
    trace
        .borrow_mut()
        .drain(..)
        .map(|event| name(&event))
        .collect()
}

/// 宿主为一个字母键做的事：先 `msime_client_key`，再派发。
fn letter(handle: u64, letter: u8) -> Value {
    msime_client_key(handle, 0);
    read(msime_client_character(handle, letter, false))
}

fn pinyin_host(root: &std::path::Path) -> u64 {
    let handle = test_host_with_pinyin_fixture(root, chinese_preferences());
    assert_eq!(read(msime_client_focus(handle, true))["ok"], true);
    handle
}

fn korean_host(root: &std::path::Path) -> u64 {
    let handle = test_host_preferences(
        root,
        Preferences {
            scheme: InputScheme::Korean,
            ..chinese_preferences()
        },
    );
    assert_eq!(read(msime_client_focus(handle, true))["ok"], true);
    handle
}

/// 一段轨迹里组字开始和结束的次数。
fn starts_and_ends(trace: &[String]) -> (usize, usize) {
    (
        trace.iter().filter(|name| *name == "start").count(),
        trace.iter().filter(|name| name.starts_with("end{")).count(),
    )
}

/// 黄金轨迹专用的全拼词库：除了 ni'hao，`n`（`ni`）和 `h`（`hao`）也有字，所以打下第一键就既开始组字又出候选。共用的 `test_host_with_pinyin_fixture` 只收了 ni'hao，`n` 和 `ni` 没有候选，走不出设计 §2.2 的轨迹。
fn golden_pinyin_host(root: &std::path::Path) -> u64 {
    golden_pinyin_host_with(root, chinese_preferences())
}

/// 同 [`golden_pinyin_host`]，用给定的偏好。
fn golden_pinyin_host_with(root: &std::path::Path, preferences: Preferences) -> u64 {
    golden_pinyin_host_with_phrase_preedit(root, preferences, false)
}

fn golden_pinyin_host_with_phrase_preedit(
    root: &std::path::Path,
    preferences: Preferences,
    phrase_preedit: bool,
) -> u64 {
    let path = |name| {
        let path = root.join(name);
        std::fs::create_dir_all(&path).unwrap();
        path
    };
    let resources = path("resources");
    let dictionaries = path("dictionaries");
    let fixture = "CREATE TABLE tbl_1_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);
                   INSERT INTO tbl_1_n VALUES('ni','n','你',100);
                   CREATE TABLE tbl_1_h(key TEXT,jp TEXT,value TEXT,weight INTEGER);
                   INSERT INTO tbl_1_h VALUES('hao','h','好',100);
                   CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);
                   INSERT INTO tbl_2_n VALUES('ni''hao','nh','你好',1000),('ni''hao','nh','拟好',80);
                   CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);
                   CREATE TABLE quick_parases(key TEXT,value TEXT,weight INTEGER);
                   CREATE INDEX idx_quick_parases_key_weight ON quick_parases(key,weight DESC);";
    for directory in [&resources, &dictionaries] {
        rusqlite::Connection::open(directory.join("msime-pinyin.db"))
            .unwrap()
            .execute_batch(fixture)
            .unwrap();
    }
    let options = json!({ "api_version": 1, "resources": resources, "user_data": path("user"), "cache": path("cache"), "dictionaries": dictionaries, "preferences": preferences, "phrase_preedit": phrase_preedit }).to_string();
    let created = read(unsafe { msime_client_create(options.as_ptr(), options.len()) });
    assert_eq!(created["ok"], true, "{created}");
    let handle = created["value"]["session"].as_u64().unwrap();
    assert_eq!(read(msime_client_focus(handle, true))["ok"], true);
    handle
}

#[test]
fn pinyin_typed_and_picked_follows_the_golden_trace() {
    let dir = tempfile::tempdir().unwrap();
    let handle = golden_pinyin_host(dir.path());
    let trace = record(handle, 0);
    for key in b"nihao" {
        let typed = letter(handle, *key);
        assert!(
            !typed["value"]["view"]["candidates"]
                .as_array()
                .unwrap()
                .is_empty(),
            "{typed}"
        );
    }
    msime_client_key(handle, 1);
    let picked = read(msime_client_command(handle, 1));
    assert_eq!(picked["value"]["commit"], "你好");
    // 设计 §2.2：`key, input, start, update, shown, (key, input, update, shown)×4, key, input, selected, before_commit, commit, end{Commit}, hidden`。
    let mut expected = vec!["key", "input", "start", "update", "shown"];
    for _ in 0..4 {
        expected.extend(["key", "input", "update", "shown"]);
    }
    expected.extend([
        "key",
        "input",
        "selected{Highlighted}",
        "before_commit",
        "commit{Selection}",
        "end{Commit}",
        "hidden{Commit}",
    ]);
    assert_eq!(names(&trace), expected);
    read(msime_client_destroy(handle));
}

#[test]
fn both_punctuation_routes_pick_then_commit_with_the_mark_as_tail() {
    let dir = tempfile::tempdir().unwrap();
    let handle = pinyin_host(dir.path());
    let trace = record(handle, 0);
    for (route, mark) in [(0, b','), (1, b'.')] {
        for key in b"nihao" {
            letter(handle, *key);
        }
        trace.borrow_mut().clear();
        msime_client_key(handle, 0);
        let finished = read(if route == 0 {
            msime_client_punctuation(handle, mark)
        } else {
            msime_client_punctuation_ascii(handle, mark)
        });
        assert!(finished["value"]["commit"]
            .as_str()
            .unwrap()
            .starts_with("本地"));
        let events = trace.borrow().clone();
        assert_eq!(
            events.iter().map(name).collect::<Vec<_>>(),
            [
                "key",
                "input",
                "selected{PunctuationFinish}",
                "before_commit",
                "commit{Selection}",
                "end{Commit}",
                "hidden{Commit}",
            ],
            "route {route}"
        );
        let split = events.iter().find_map(|event| match event {
            LifecycleEvent::BeforeCommit {
                body_chars,
                tail_chars,
                ..
            } => Some((*body_chars, *tail_chars)),
            _ => None,
        });
        assert_eq!(split, Some((2, 1)), "route {route}");
        trace.borrow_mut().clear();
    }
    read(msime_client_destroy(handle));
}

#[test]
fn a_korean_syllable_that_starts_the_next_commits_with_continues() {
    let dir = tempfile::tempdir().unwrap();
    let handle = korean_host(dir.path());
    let trace = record(handle, 0);
    for key in b"gks" {
        letter(handle, *key);
    }
    trace.borrow_mut().clear();
    let next = letter(handle, b'r');
    assert_eq!(next["value"]["commit"], "한");
    assert_eq!(
        names(&trace),
        [
            "key",
            "input",
            "before_commit",
            "commit{SchemeText,continues}",
            "update"
        ]
    );
    read(msime_client_destroy(handle));
}

#[test]
fn escape_from_the_hanja_list_only_hides_the_candidates() {
    let dir = tempfile::tempdir().unwrap();
    let handle = korean_host(dir.path());
    let trace = record(handle, 0);
    for key in b"gks" {
        letter(handle, *key);
    }
    trace.borrow_mut().clear();
    msime_client_key(handle, 0);
    read(msime_client_command(handle, 16));
    assert_eq!(names(&trace), ["key", "input", "shown"]);
    msime_client_key(handle, 0);
    let cancelled = read(msime_client_command(handle, 3));
    assert_eq!(cancelled["value"]["view"]["preedit"], "한");
    assert_eq!(names(&trace), ["key", "input{Cancel}", "hidden{Cancel}"]);
    read(msime_client_destroy(handle));
}

/// Windows 选词后对同一个键再发一次 MSIME_CANCEL：第二次派发属于同一轮，组字已经结束，只有 input。
#[test]
fn a_cancel_after_a_pick_in_the_same_turn_only_reports_its_input() {
    let dir = tempfile::tempdir().unwrap();
    let handle = pinyin_host(dir.path());
    let trace = record(handle, 0);
    let mut typed = Value::Null;
    for key in b"nihao" {
        typed = letter(handle, *key);
    }
    trace.borrow_mut().clear();
    let generation = typed["value"]["view"]["generation"].as_u64().unwrap();
    msime_client_key(handle, 0x80);
    let selected = read(msime_client_select(handle, generation, 0));
    assert_eq!(selected["value"]["commit"], "本地");
    read(msime_client_command(handle, 3));
    assert_eq!(
        names(&trace),
        [
            "key",
            "input",
            "selected{Select}",
            "before_commit",
            "commit{Selection}",
            "end{Commit}",
            "hidden{Commit}",
            "input{Cancel}",
        ]
    );
    read(msime_client_destroy(handle));
}

#[test]
fn a_host_commit_cancel_ends_the_composition_as_a_host_commit() {
    let dir = tempfile::tempdir().unwrap();
    let handle = pinyin_host(dir.path());
    let trace = record(handle, 0);
    for key in b"nihao" {
        letter(handle, *key);
    }
    trace.borrow_mut().clear();
    let discarded = read(msime_client_command(handle, 19));
    assert_eq!(discarded["ok"], true);
    assert!(discarded["value"]["commit"].is_null());
    assert_eq!(discarded["value"]["view"]["preedit"], "");
    assert_eq!(
        names(&trace),
        ["input{HostCommit}", "end{HostCommit}", "hidden{HostCommit}"]
    );
    read(msime_client_destroy(handle));
}

#[test]
fn focus_loss_ends_the_composition_before_focus_out_and_its_commit_is_silent() {
    let dir = tempfile::tempdir().unwrap();
    let handle = pinyin_host(dir.path());
    let trace = record(handle, FEEDBACK);
    for key in b"nihao" {
        letter(handle, *key);
    }
    trace.borrow_mut().clear();
    read(msime_client_focus(handle, false));
    assert_eq!(names(&trace), ["end{Focus}", "hidden{Focus}", "focus_out"]);
    read(msime_client_focus(handle, true));
    assert_eq!(names(&trace), ["focus_in"]);
    read(msime_client_destroy(handle));

    // 韩文失焦上屏开着的音节：只有 commit{FocusLoss}，没有 before_commit，也不画特效。
    let dir = tempfile::tempdir().unwrap();
    let mut preferences = Preferences {
        scheme: InputScheme::Korean,
        ..chinese_preferences()
    };
    preferences.plugins.effect_style = EffectStyle::Flash;
    let handle = test_host_preferences(dir.path(), preferences);
    read(msime_client_focus(handle, true));
    let trace = record(handle, FEEDBACK);
    // 前半段的拼音也要过按键音，先清掉。
    CUES.with(|cues| cues.borrow_mut().clear());
    for key in b"rk" {
        letter(handle, *key);
    }
    trace.borrow_mut().clear();
    let typed = CUES.with(|cues| std::mem::take(&mut *cues.borrow_mut()));
    assert_eq!(typed.len(), 2, "each letter asked for its key sound");
    let left = read(msime_client_focus(handle, false));
    assert_eq!(left["value"]["commit"], "가");
    assert!(
        left["value"].get("feedback").is_none(),
        "focus answers carry no feedback"
    );
    assert_eq!(
        CUES.with(|cues| std::mem::take(&mut *cues.borrow_mut())),
        [],
        "a focus-loss commit asks for no commit sound"
    );
    let events = trace.borrow().clone();
    assert_eq!(
        events.iter().map(name).collect::<Vec<_>>(),
        ["commit{FocusLoss}", "end{Focus}", "focus_out"]
    );
    read(msime_client_destroy(handle));
}

/// 命令 20 是宿主替失焦结束组字（macOS 的 `commitComposition:`）：对会话同命令 9，上的是同一段文字，但来源记 FocusLoss，不可改写、不出提交音也不画；同一段组字用命令 9 结束则照常出声、画。
#[test]
fn a_finish_on_focus_loss_commits_like_a_finish_but_silently() {
    let dir = tempfile::tempdir().unwrap();
    let handle = test_host_with_pinyin_fixture(dir.path(), effect_preferences());
    read(msime_client_focus(handle, true));
    let trace = record(handle, FEEDBACK);
    msime_client_set_field_traits(handle, TRAIT_KNOWN);
    let mut texts = Vec::new();
    for command in [9, 20] {
        for key in b"nihao" {
            letter(handle, *key);
        }
        trace.borrow_mut().clear();
        CUES.with(|cues| cues.borrow_mut().clear());
        let finished = read(msime_client_command(handle, command));
        assert_eq!(finished["ok"], true, "{finished}");
        texts.push(finished["value"]["commit"].as_str().unwrap().to_owned());
        assert_eq!(finished["value"]["view"]["preedit"], "");
        let cues = CUES.with(|cues| std::mem::take(&mut *cues.borrow_mut()));
        let commit = trace.borrow().iter().find_map(|event| match event {
            LifecycleEvent::BeforeCommit {
                origin, rewritable, ..
            } => Some((*origin, *rewritable)),
            _ => None,
        });
        let effect = finished["value"]["feedback"]["typing_effect"]
            .as_u64()
            .unwrap();
        if command == 9 {
            assert_eq!(cues, [crate::lifecycle::Cue::Commit]);
            assert_ne!(effect, 0, "a finish draws its commit");
            assert_ne!(commit.unwrap().0, CommitOrigin::FocusLoss);
        } else {
            assert_eq!(cues, [], "a focus-loss finish asks for no commit sound");
            assert_eq!(effect, 0, "a focus-loss finish draws nothing");
            assert_eq!(commit, Some((CommitOrigin::FocusLoss, false)));
            assert!(trace.borrow().iter().any(|event| matches!(
                event,
                LifecycleEvent::Input {
                    class: InputClass::FocusLoss,
                    ..
                }
            )));
        }
    }
    assert_eq!(texts[0], texts[1], "both commit the same text");
    read(msime_client_destroy(handle));
}

/// 命令 20 时高亮的是脚本插件的行：用户这一次没在看候选窗，写进文档的是他打的 `/uuid`，生命周期照旧记 `commit{FocusLoss}`；命令 9（按键结束）上屏高亮的插件行。
#[cfg(any(
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", not(target_env = "ohos"))
))]
#[test]
fn a_finish_on_focus_loss_commits_the_typed_command_over_a_plugin_row() {
    let dir = tempfile::tempdir().unwrap();
    let mut preferences = chinese_preferences();
    preferences.local_modes.command = true;
    let handle = test_host_preferences(dir.path(), preferences);
    assert_eq!(read(msime_client_focus(handle, true))["ok"], true);
    msime_client_set_field_traits(handle, TRAIT_KNOWN);
    let trace = record(handle, FEEDBACK);
    attach_plugin_rows(handle, &["甲乙"]);
    for (command, expected) in [(20, "/uuid"), (9, "甲乙")] {
        for key in b"/uuid" {
            letter(handle, *key);
        }
        trace.borrow_mut().clear();
        let finished = read(msime_client_command(handle, command));
        assert_eq!(finished["ok"], true, "{finished}");
        assert_eq!(finished["value"]["commit"], expected, "command {command}");
        assert_eq!(finished["value"]["view"]["local_mode"], "none");
        let commits: Vec<String> = names(&trace)
            .into_iter()
            .filter(|name| name.starts_with("commit"))
            .collect();
        if command == 20 {
            assert_eq!(commits, ["commit{FocusLoss}"]);
        } else {
            assert_eq!(commits.len(), 1);
            assert_ne!(commits[0], "commit{FocusLoss}");
        }
    }
    read(msime_client_destroy(handle));
}

#[test]
fn switching_to_dedicated_english_mid_composition_ends_it() {
    let dir = tempfile::tempdir().unwrap();
    let handle = pinyin_host(dir.path());
    let trace = record(handle, 0);
    for key in b"nihao" {
        letter(handle, *key);
    }
    trace.borrow_mut().clear();
    let switched = read(msime_client_set_english_mode(handle, true));
    assert_eq!(switched["value"]["preedit"], "", "{switched}");
    assert_eq!(names(&trace), ["end{Other}", "hidden{Other}"]);
    // 组字已经没了，再切回来没有任何事件。
    read(msime_client_set_english_mode(handle, false));
    assert_eq!(names(&trace), Vec::<String>::new());
    read(msime_client_destroy(handle));
}

#[test]
fn switching_to_dedicated_english_discards_held_phrase_progress() {
    for empty_reading in [false, true] {
        let dir = tempfile::tempdir().unwrap();
        let handle =
            golden_pinyin_host_with_phrase_preedit(dir.path(), chinese_preferences(), true);
        let trace = record(handle, 0);
        for &key in b"nihao" {
            letter(handle, key);
        }
        let view = read(msime_client_view(handle))["value"].clone();
        let seat = view["candidates"]
            .as_array()
            .unwrap()
            .iter()
            .position(|candidate| candidate["text"] == "你")
            .unwrap_or_else(|| panic!("{view}"));
        let held = read(msime_client_select(
            handle,
            view["generation"].as_u64().unwrap(),
            seat,
        ));
        assert_eq!(held["value"]["commit"], Value::Null, "{held}");
        assert_eq!(held["value"]["view"]["phrase_prefix"], "你", "{held}");
        assert_eq!(held["value"]["view"]["editing_text"], "hao", "{held}");
        let generation = held["value"]["view"]["generation"].as_u64().unwrap();
        if empty_reading {
            let kept = read(msime_client_command(handle, 12));
            assert_eq!(kept["value"]["view"]["phrase_prefix"], "你", "{kept}");
            assert_eq!(kept["value"]["view"]["editing_text"], "", "{kept}");
            assert_eq!(kept["value"]["commit"], Value::Null, "{kept}");
        }
        trace.borrow_mut().clear();

        let switched = read(msime_client_set_english_mode(handle, true));

        assert_eq!(switched["ok"], true, "{switched}");
        assert_eq!(switched["value"]["dedicated_english"], true, "{switched}");
        assert!(
            switched["value"]["phrase_prefix"]
                .as_str()
                .unwrap_or_default()
                .is_empty(),
            "{switched}"
        );
        assert_eq!(switched["value"]["editing_text"], "", "{switched}");
        assert_eq!(
            names(&trace),
            if empty_reading {
                vec!["end{Other}"]
            } else {
                vec!["end{Other}", "hidden{Other}"]
            }
        );
        assert_eq!(
            read(msime_client_select(handle, generation, 0))["ok"],
            false
        );
        let backspace = read(msime_client_command(handle, 0));
        assert_eq!(
            backspace["value"]["view"]["editing_text"], "",
            "{backspace}"
        );
        assert!(
            backspace["value"]["view"]["phrase_prefix"]
                .as_str()
                .unwrap_or_default()
                .is_empty(),
            "{backspace}"
        );
        letter(handle, b'a');
        let committed = read(msime_client_command(handle, 2));
        assert_eq!(committed["value"]["commit"], "a", "{committed}");
        assert!(
            committed["value"]["view"]["phrase_prefix"]
                .as_str()
                .unwrap_or_default()
                .is_empty(),
            "{committed}"
        );
        read(msime_client_destroy(handle));
    }
}

#[test]
fn setting_the_same_language_preserves_held_phrase_retraction() {
    let dir = tempfile::tempdir().unwrap();
    let handle = golden_pinyin_host_with_phrase_preedit(dir.path(), chinese_preferences(), true);
    for &key in b"nihao" {
        letter(handle, key);
    }
    let view = read(msime_client_view(handle))["value"].clone();
    let seat = view["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .position(|candidate| candidate["text"] == "你")
        .unwrap_or_else(|| panic!("{view}"));
    read(msime_client_select(
        handle,
        view["generation"].as_u64().unwrap(),
        seat,
    ));

    let unchanged = read(msime_client_set_english_mode(handle, false));

    assert_eq!(unchanged["value"]["phrase_prefix"], "你", "{unchanged}");
    assert_eq!(unchanged["value"]["editing_text"], "hao", "{unchanged}");
    let kept = read(msime_client_command(handle, 12));
    assert_eq!(kept["value"]["view"]["phrase_prefix"], "你", "{kept}");
    assert_eq!(kept["value"]["view"]["editing_text"], "", "{kept}");
    let restored = read(msime_client_command(handle, 0));
    assert!(
        restored["value"]["view"]["phrase_prefix"]
            .as_str()
            .unwrap_or_default()
            .is_empty(),
        "{restored}"
    );
    assert_eq!(
        restored["value"]["view"]["editing_text"], "ni",
        "{restored}"
    );
    read(msime_client_destroy(handle));
}

/// 失焦取消了组字之后 runtime 才报错（刷新失败）：这次调用照样推导 end、hidden 和 focus_out，下一键的 start 才有对应的 end。runtime 的真实引擎没法在这一步注入失败，这里按 `msime_client_focus` 的顺序直接驱动会话。
#[test]
fn a_call_that_fails_after_ending_the_composition_still_ends_it() {
    let dir = tempfile::tempdir().unwrap();
    let handle = pinyin_host(dir.path());
    let trace = record(handle, 0);
    for key in b"nihao" {
        letter(handle, *key);
    }
    trace.borrow_mut().clear();
    SESSIONS.with(|sessions| {
        let mut sessions = sessions.borrow_mut();
        let session = sessions.get_mut(&handle).unwrap();
        let pending = session.lifecycle_begin(Call::Focus(false));
        session.runtime.focus(false).unwrap();
        let failed: Result<(), String> = Err("refresh failed".into());
        assert!(session.lifecycle_on_error(pending, failed).is_err());
    });
    assert_eq!(names(&trace), ["end{Focus}", "hidden{Focus}", "focus_out"]);
    read(msime_client_focus(handle, true));
    trace.borrow_mut().clear();
    letter(handle, b'n');
    assert_eq!(names(&trace), ["key", "input", "start", "update"]);
    read(msime_client_destroy(handle));
}

/// 经 C ABI 报错的派发（过期的候选号）：只有派发前的 input，组字原样留着，之后的事件照常。
#[test]
fn a_failed_dispatch_reports_only_its_input() {
    let dir = tempfile::tempdir().unwrap();
    let handle = pinyin_host(dir.path());
    let trace = record(handle, 0);
    let mut typed = Value::Null;
    for key in b"nihao" {
        typed = letter(handle, *key);
    }
    trace.borrow_mut().clear();
    let generation = typed["value"]["view"]["generation"].as_u64().unwrap();
    let stale = read(msime_client_select(handle, generation + 1, 0));
    assert_eq!(stale["ok"], false, "{stale}");
    assert_eq!(names(&trace), ["input"]);
    read(msime_client_command(handle, 3));
    assert_eq!(
        names(&trace),
        ["input{Cancel}", "end{Cancel}", "hidden{Cancel}"]
    );
    read(msime_client_destroy(handle));
}

#[test]
fn destroying_a_composing_session_ends_it_first() {
    let dir = tempfile::tempdir().unwrap();
    let handle = pinyin_host(dir.path());
    let trace = record(handle, 0);
    for key in b"nihao" {
        letter(handle, *key);
    }
    trace.borrow_mut().clear();
    assert_eq!(read(msime_client_destroy(handle))["ok"], true);
    assert_eq!(
        names(&trace),
        ["end{Destroyed}", "hidden{Destroyed}", "session_end"]
    );
}

#[test]
fn every_start_is_paired_with_an_end_over_a_mixed_session() {
    let dir = tempfile::tempdir().unwrap();
    let handle = pinyin_host(dir.path());
    let trace = record(handle, 0);
    let mut all = Vec::new();
    for round in 0..5 {
        for key in b"nihao" {
            letter(handle, *key);
        }
        match round {
            0 => {
                read(msime_client_command(handle, 1));
            }
            1 => {
                read(msime_client_command(handle, 3));
            }
            2 => {
                for _ in 0..5 {
                    read(msime_client_command(handle, 0));
                }
            }
            3 => {
                read(msime_client_focus(handle, false));
                read(msime_client_focus(handle, true));
            }
            // IBus 的 Ctrl+Shift+E 不先上屏就切专用英文，组字被丢掉。
            _ => {
                read(msime_client_set_english_mode(handle, true));
                read(msime_client_set_english_mode(handle, false));
            }
        }
        all.extend(names(&trace));
    }
    let (starts, ends) = starts_and_ends(&all);
    assert_eq!((starts, ends), (5, 5), "{all:?}");
    // 开始与结束交替出现，从不连续两次开始或两次结束。
    let mut open = false;
    for name in &all {
        if name == "start" {
            assert!(!open, "{all:?}");
            open = true;
        } else if name.starts_with("end{") {
            assert!(open, "{all:?}");
            open = false;
        }
    }
    read(msime_client_destroy(handle));
}

/// 没选择加入的会话和选择加入但不开 FEEDBACK 的会话，同一串调用的回复逐字节相同（会话号除外）；开了 FEEDBACK 的只多一个 `feedback` 键。
#[test]
fn opting_in_changes_no_byte_of_the_transition_json() {
    fn run(flags: Option<u32>) -> Vec<String> {
        let dir = tempfile::tempdir().unwrap();
        let handle = pinyin_host(dir.path());
        if let Some(flags) = flags {
            msime_client_set_lifecycle(handle, flags);
        }
        let raw = |pointer: *mut c_char| {
            // SAFETY: 本库刚分配的回复字符串，只在这里释放一次。
            let string = unsafe { CString::from_raw(pointer) };
            string.into_string().unwrap()
        };
        let mut answers = Vec::new();
        for key in b"nihao" {
            msime_client_key(handle, 0);
            answers.push(raw(msime_client_character(handle, *key, false)));
        }
        answers.push(raw(msime_client_command(handle, 102)));
        answers.push(raw(msime_client_command(handle, 1)));
        for key in b"ni" {
            answers.push(raw(msime_client_character(handle, *key, false)));
        }
        answers.push(raw(msime_client_punctuation(handle, b',')));
        for key in b"ni" {
            answers.push(raw(msime_client_character(handle, *key, false)));
        }
        answers.push(raw(msime_client_command(handle, 3)));
        answers.push(raw(msime_client_rerank_settled(handle)));
        answers.push(raw(msime_client_focus(handle, false)));
        read(msime_client_destroy(handle));
        let session = format!("\"session\":{handle}");
        answers
            .into_iter()
            .map(|answer| answer.replace(&session, "\"session\":0"))
            .collect()
    }
    let without = run(None);
    assert_eq!(without, run(Some(0)));
    assert_eq!(without, run(Some(COMMIT_OBSERVE_ONLY)));
    let feedback = run(Some(FEEDBACK));
    assert_eq!(without.len(), feedback.len());
    for (plain, fed) in without.iter().zip(&feedback) {
        let mut fed: Value = serde_json::from_str(fed).unwrap();
        if let Some(value) = fed["value"].as_object_mut() {
            value.remove("feedback");
        }
        let plain: Value = serde_json::from_str(plain).unwrap();
        assert_eq!(plain, fed);
    }
}

#[test]
fn a_fullwidth_commit_splits_after_the_conversion() {
    let dir = tempfile::tempdir().unwrap();
    let handle = pinyin_host(dir.path());
    read(msime_client_set_character_width(handle, true));
    let trace = record(handle, 0);
    for key in b"nihao" {
        letter(handle, *key);
    }
    trace.borrow_mut().clear();
    let finished = read(msime_client_punctuation_ascii(handle, b','));
    let commit = finished["value"]["commit"].as_str().unwrap().to_owned();
    assert_eq!(commit, "本地\u{ff0c}");
    let split = SESSIONS.with(|sessions| {
        let sessions = sessions.borrow();
        let facts = sessions[&handle].runtime.facts();
        let (body, tail) = facts.split_commit(&commit);
        (body.to_owned(), tail.to_owned())
    });
    assert_eq!(split, ("本地".to_owned(), "\u{ff0c}".to_owned()));
    let before = trace.borrow().iter().find_map(|event| match event {
        LifecycleEvent::BeforeCommit {
            body_chars,
            tail_chars,
            ..
        } => Some((*body_chars, *tail_chars)),
        _ => None,
    });
    assert_eq!(before, Some((2, 1)));
    read(msime_client_destroy(handle));
}

fn effect_preferences() -> Preferences {
    let mut preferences = chinese_preferences();
    preferences.plugins.effect_style = EffectStyle::Flash;
    preferences.plugins.combo_counter = true;
    preferences
}

const FLASH: u64 = 1 << 17;

#[test]
fn feedback_moves_the_effects_into_the_core_and_silences_the_old_calls() {
    let dir = tempfile::tempdir().unwrap();
    let handle = test_host_with_pinyin_fixture(dir.path(), effect_preferences());
    read(msime_client_focus(handle, true));
    // 选择加入之前：旧调用照常。
    assert_eq!(u64::from(msime_client_typing_effect(handle, 0)), FLASH | 1);
    assert_eq!(msime_client_key(handle, 0), 0, "not opted in");
    assert_eq!(msime_client_set_lifecycle(handle, FEEDBACK), FEEDBACK);
    // 选择加入之后：0-4 和三个声音调用都是空操作，5 仍由宿主调用。
    for event in 0..=4 {
        assert_eq!(msime_client_typing_effect(handle, event), 0, "{event}");
    }
    assert!(!msime_client_key_sound(handle, 0));
    assert!(!msime_client_commit_sound(handle));
    assert!(!msime_client_music_set_active(handle, true));
    assert_eq!(u64::from(msime_client_typing_effect(handle, 5)), FLASH);
    // 按键特效来自 msime_client_key，上屏特效写进派发回复的 feedback。
    let mut typed = Value::Null;
    for (index, key) in b"nihao".iter().enumerate() {
        assert_eq!(
            u64::from(msime_client_key(handle, 0)),
            FLASH | (index as u64 + 1)
        );
        typed = read(msime_client_character(handle, *key, false));
        assert_eq!(typed["value"]["feedback"]["typing_effect"], 0);
    }
    assert!(typed["value"]["commit"].is_null());
    msime_client_key(handle, 1);
    let picked = read(msime_client_command(handle, 1));
    assert_eq!(picked["value"]["commit"], "本地");
    assert_eq!(picked["value"]["feedback"]["typing_effect"], FLASH | 6);
    read(msime_client_destroy(handle));
}

/// 宿主暂存部分选词（Windows）时，部分选词的那一段还没进文档：经真实的派发（会改写上屏的那条路径）也不出上屏音、`feedback` 为 0，补齐整个词的那次上屏才出一次。
#[test]
fn a_piece_the_host_holds_sounds_only_when_the_whole_phrase_goes_out() {
    let commits = || {
        CUES.with(|cues| std::mem::take(&mut *cues.borrow_mut()))
            .into_iter()
            .filter(|cue| *cue == Cue::Commit)
            .count()
    };
    let dir = tempfile::tempdir().unwrap();
    let handle = golden_pinyin_host_with(dir.path(), effect_preferences());
    let flags = FEEDBACK | HOST_HOLDS_PIECES;
    assert_eq!(msime_client_set_lifecycle(handle, flags), flags);
    msime_client_set_field_traits(handle, TRAIT_KNOWN);
    let mut typed = Value::Null;
    for key in b"nihao" {
        typed = letter(handle, *key);
    }
    commits();
    // 选"你"只用掉 ni，读音还剩 hao：部分选词。
    let view = &typed["value"]["view"];
    let generation = view["generation"].as_u64().unwrap();
    let ni = view["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .position(|candidate| candidate["text"] == "你")
        .unwrap();
    msime_client_key(handle, 0x80);
    let piece = read(msime_client_select(handle, generation, ni));
    assert_eq!(piece["value"]["commit"], "你", "{piece}");
    assert_eq!(piece["value"]["feedback"]["typing_effect"], 0);
    assert_eq!(commits(), 0, "a held piece asks for no commit sound");
    // 补齐整个词的那次上屏：宿主把暂存的"你"接在前面写进文档，这时出一次。
    msime_client_key(handle, 1);
    let whole = read(msime_client_command(handle, 1));
    assert_eq!(whole["value"]["commit"], "好", "{whole}");
    assert_ne!(whole["value"]["feedback"]["typing_effect"], 0);
    assert_eq!(commits(), 1);
    read(msime_client_destroy(handle));
}

#[test]
fn a_secure_field_gets_no_feedback_at_all() {
    let dir = tempfile::tempdir().unwrap();
    let handle = test_host_with_pinyin_fixture(dir.path(), effect_preferences());
    read(msime_client_focus(handle, true));
    msime_client_set_lifecycle(handle, FEEDBACK);
    assert_eq!(
        msime_client_set_field_traits(handle, TRAIT_KNOWN | TRAIT_SECURE),
        TRAIT_KNOWN | TRAIT_SECURE
    );
    for key in b"nihao" {
        assert_eq!(msime_client_key(handle, 0), 0);
        read(msime_client_character(handle, *key, false));
    }
    assert_eq!(msime_client_key(handle, 1), 0);
    let picked = read(msime_client_command(handle, 1));
    assert_eq!(picked["value"]["commit"], "本地");
    assert_eq!(picked["value"]["feedback"]["typing_effect"], 0);
    // 旧调用在 FEEDBACK 下本来就是空操作，SECURE 里同样什么都不出。
    assert!(!msime_client_key_sound(handle, 0));
    assert_eq!(msime_client_typing_effect(handle, 0), 0);
    read(msime_client_destroy(handle));
}

/// 一个必定 panic 的消费者不影响上屏：panic 被它自己那层 `catch_unwind` 接住，后面的内置消费者照常运行。
#[test]
fn a_panicking_consumer_never_reaches_the_commit() {
    let dir = tempfile::tempdir().unwrap();
    let handle = test_host_with_pinyin_fixture(dir.path(), effect_preferences());
    read(msime_client_focus(handle, true));
    record(handle, FEEDBACK);
    SESSIONS.with(|sessions| {
        let mut sessions = sessions.borrow_mut();
        let state = sessions
            .get_mut(&handle)
            .unwrap()
            .lifecycle
            .as_deref_mut()
            .unwrap();
        state.probe = Some(Box::new(|_| panic!("a consumer that always fails")));
    });
    for (index, key) in b"nihao".iter().enumerate() {
        assert_eq!(
            u64::from(msime_client_key(handle, 0)),
            FLASH | (index as u64 + 1)
        );
        assert_eq!(
            read(msime_client_character(handle, *key, false))["ok"],
            true
        );
    }
    msime_client_key(handle, 1);
    let picked = read(msime_client_command(handle, 1));
    assert_eq!(picked["ok"], true);
    assert_eq!(picked["value"]["commit"], "本地");
    assert_eq!(picked["value"]["feedback"]["typing_effect"], FLASH | 6);
    assert_eq!(read(msime_client_focus(handle, false))["ok"], true);
    assert_eq!(read(msime_client_destroy(handle))["ok"], true);
}

#[test]
fn the_new_calls_refuse_an_unknown_session() {
    assert_eq!(msime_client_set_lifecycle(u64::MAX, FEEDBACK), 0);
    assert_eq!(msime_client_key(u64::MAX, 0), 0);
    assert_eq!(msime_client_set_field_traits(u64::MAX, TRAIT_KNOWN), 0);
    assert!(!msime_client_set_active(u64::MAX, true));
    // 没选择加入的会话：key、traits、active 什么都不做。
    let dir = tempfile::tempdir().unwrap();
    let handle = test_host(dir.path());
    assert_eq!(msime_client_key(handle, 0), 0);
    assert_eq!(msime_client_set_field_traits(handle, TRAIT_KNOWN), 0);
    assert!(!msime_client_set_active(handle, false));
    assert_eq!(
        msime_client_set_lifecycle(handle, COMMIT_OBSERVE_ONLY),
        COMMIT_OBSERVE_ONLY
    );
    assert!(msime_client_set_active(handle, false));
    read(msime_client_destroy(handle));
}

#[test]
fn a_selection_fact_reaches_the_event_unchanged() {
    let dir = tempfile::tempdir().unwrap();
    let handle = pinyin_host(dir.path());
    let trace = record(handle, 0);
    let mut typed = Value::Null;
    for key in b"nihao" {
        typed = letter(handle, *key);
    }
    let generation = typed["value"]["view"]["generation"].as_u64().unwrap();
    trace.borrow_mut().clear();
    read(msime_client_select(handle, generation, 1));
    let selection = trace.borrow().iter().find_map(|event| match event {
        LifecycleEvent::CandidateSelected(selection) => Some(*selection),
        _ => None,
    });
    let selection = selection.expect("a selection");
    assert_eq!(selection.seat, 1);
    assert_eq!(selection.how, SelectionHow::Select);
    assert_eq!(selection.outcome, SelectionOutcome::Committed);
    let origin = trace.borrow().iter().find_map(|event| match event {
        LifecycleEvent::Commit { origin, .. } => Some(*origin),
        _ => None,
    });
    assert_eq!(origin, Some(CommitOrigin::Selection));
    read(msime_client_destroy(handle));
}
