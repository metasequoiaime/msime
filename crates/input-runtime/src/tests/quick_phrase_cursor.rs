use super::*;

fn phrase_runtime(root: &std::path::Path, phrase: &str, mixed: bool) -> Runtime {
    let mut options = real_engine_options(root);
    options.quick_phrase_candidates = mixed;
    options.quick_phrase_table = vec![msime_engine::QuickPhraseEntry {
        key: "ab".into(),
        text: phrase.into(),
    }];
    let mut runtime = Runtime::new(msime_engine::host::Session::new(&options).unwrap(), 6).unwrap();
    runtime.focus(true).unwrap();
    type_characters(&mut runtime, if mixed { "ab" } else { "Kab" });
    runtime
}

#[test]
fn quick_phrase_cursor_reaches_every_candidate_commit_route() {
    for mixed in [false, true] {
        for route in 0..7 {
            let root = tempfile::tempdir().unwrap();
            let mut runtime = phrase_runtime(root.path(), "title:\"${cursor}\"", mixed);
            let candidate = runtime
                .all_candidates()
                .candidates
                .into_iter()
                .find(|candidate| {
                    candidate.source == msime_engine::CandidateSource::QuickPhrase as u8
                })
                .unwrap();
            assert_eq!(candidate.text, "title:\"\"");
            let seat = candidate.id.index;
            for _ in 0..seat {
                runtime.dispatch(Action::NextCandidate).unwrap();
            }
            let id = runtime.all_candidates().candidates[seat].id;
            let action = match route {
                0 => Action::Select(id),
                1 => Action::SelectAnyCandidate(id),
                2 => Action::SelectHighlighted,
                3 => Action::Finish,
                4 => Action::Command(Command::CommitCandidate),
                5 => Action::Punctuation(b','),
                _ => Action::PunctuationAscii(b','),
            };
            let committed = runtime.dispatch(action).unwrap();
            let tail = match route {
                5 => "，",
                6 => ",",
                _ => "",
            };
            assert_eq!(
                committed.commit.as_deref(),
                Some(format!("title:\"\"{tail}").as_str()),
                "mixed={mixed}, route={route}: {committed:?}"
            );
            assert_eq!(
                committed.commit_cursor_suffix.as_deref(),
                Some(format!("\"{tail}").as_str())
            );
            assert_eq!(runtime.selected_text(), Some("title:\"${cursor}\""));
            assert_eq!(runtime.ai_context, "title:\"");
            let wire = serde_json::to_value(&committed).unwrap();
            assert_eq!(wire["commit_cursor_suffix"], format!("\"{tail}"));
            assert!(character(&mut runtime, b'n').commit_cursor_suffix.is_none());
        }
    }
}

#[test]
fn quick_phrase_cursor_keeps_empty_end_and_unicode_suffix_distinct_from_no_request() {
    for (phrase, text, suffix) in [
        ("普通短语", "普通短语", None),
        ("$${cursor}", "${cursor}", None),
        ("末尾${cursor}", "末尾", Some("")),
        ("${cursor}", "", Some("")),
        (
            "前${cursor}𠀀👩‍💻e\u{301}",
            "前𠀀👩‍💻e\u{301}",
            Some("𠀀👩‍💻e\u{301}"),
        ),
        ("${cursor}后${cursor}", "后${cursor}", Some("后${cursor}")),
    ] {
        let root = tempfile::tempdir().unwrap();
        let mut runtime = phrase_runtime(root.path(), phrase, false);
        let id = runtime.view().candidates[0].id;
        let committed = runtime.dispatch(Action::Select(id)).unwrap();
        assert_eq!(committed.commit.as_deref(), Some(text), "{phrase}");
        assert_eq!(
            committed.commit_cursor_suffix.as_deref(),
            suffix,
            "{phrase}"
        );
        assert_eq!(
            serde_json::to_value(&committed)
                .unwrap()
                .get("commit_cursor_suffix")
                .is_some(),
            suffix.is_some()
        );
    }
}

#[test]
fn quick_phrase_cursor_follows_numeric_selection_only_when_engine_declines_the_digit() {
    let fixture = Fixture {
        words: vec!["前${cursor}后".into()],
        sources: vec![msime_engine::CandidateSource::QuickPhrase as u8],
        local_mode: "none".into(),
        ..Default::default()
    };
    let mut runtime = Runtime::new(fixture, 6).unwrap();
    runtime.focus(true).unwrap();
    runtime
        .dispatch(Action::Character {
            value: b'a',
            shift: false,
        })
        .unwrap();
    let committed = runtime
        .dispatch(Action::Character {
            value: b'1',
            shift: false,
        })
        .unwrap();
    assert_eq!(committed.commit.as_deref(), Some("前后"));
    assert_eq!(committed.commit_cursor_suffix.as_deref(), Some("后"));
    assert_eq!(runtime.facts().selection.unwrap().how, SelectionHow::Digit);

    // K 模式的真实引擎先把数字当作结束原文，不能为了定位覆盖已经产生的提交。
    let root = tempfile::tempdir().unwrap();
    let mut runtime = phrase_runtime(root.path(), "前${cursor}后", false);
    let committed = character(&mut runtime, b'1');
    assert_eq!(committed.commit.as_deref(), Some("Kab"));
    assert!(committed.commit_cursor_suffix.is_none());
}

#[test]
fn quick_phrase_cursor_does_not_expand_other_sources_or_raw_commit() {
    for source in [
        msime_engine::CandidateSource::Database,
        msime_engine::CandidateSource::Plugin,
    ] {
        let fixture = Fixture {
            words: vec!["title:\"${cursor}\"".into()],
            sources: vec![source as u8],
            local_mode: "none".into(),
            ..Default::default()
        };
        let mut runtime = Runtime::new(fixture, 6).unwrap();
        runtime.focus(true).unwrap();
        runtime
            .dispatch(Action::Character {
                value: b'a',
                shift: false,
            })
            .unwrap();
        assert_eq!(runtime.view().candidates[0].text, "title:\"${cursor}\"");
        let id = runtime.view().candidates[0].id;
        let committed = runtime.dispatch(Action::Select(id)).unwrap();
        assert_eq!(committed.commit.as_deref(), Some("title:\"${cursor}\""));
        assert!(committed.commit_cursor_suffix.is_none());
    }
    let root = tempfile::tempdir().unwrap();
    let mut runtime = phrase_runtime(root.path(), "title:\"${cursor}\"", false);
    let committed = runtime
        .dispatch(Action::Command(Command::CommitRaw))
        .unwrap();
    assert_eq!(committed.commit.as_deref(), Some("Kab"));
    assert!(committed.commit_cursor_suffix.is_none());
}
