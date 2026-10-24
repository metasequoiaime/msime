use super::*;
use msime_client_core::chinese_conversion::TraditionalVariant;
use msime_engine::host::session::MultilingualComposition;
use msime_engine::multilingual::{Conversion, Language, Segment};

const RAW: &str = "hellowangluochuugoku";
const WORD: &str = "hello网络中国";

fn metadata() -> MultilingualComposition {
    MultilingualComposition {
        raw_input: RAW.into(),
        conversion: Conversion {
            chinese_complete: false,
            segments: vec![
                Segment {
                    input: 0..5,
                    language: Language::English,
                    text: "hello".into(),
                },
                Segment {
                    input: 5..12,
                    language: Language::Chinese,
                    text: "网络".into(),
                },
                Segment {
                    input: 12..20,
                    language: Language::Japanese,
                    text: "中国".into(),
                },
            ],
        },
    }
}

fn runtime_with(words: &[&str], sources: &[u8]) -> Runtime<Fixture> {
    let mut runtime = Runtime::new(
        Fixture {
            local_mode: "none".into(),
            words: words.iter().map(|word| (*word).into()).collect(),
            sources: sources.to_vec(),
            codes: sources
                .iter()
                .map(|source| {
                    if *source == 18 {
                        RAW.into()
                    } else {
                        String::new()
                    }
                })
                .collect(),
            multilingual_conversion: Some(metadata()),
            ..Fixture::default()
        },
        9,
    )
    .unwrap();
    runtime.set_multilingual_output(true, TraditionalVariant::Standard);
    runtime.set_phrase_preedit(true);
    runtime.focus(true).unwrap();
    for value in RAW.bytes() {
        runtime
            .dispatch(Action::Character {
                value,
                shift: false,
            })
            .unwrap();
    }
    runtime
}

fn mixed_id(runtime: &Runtime<Fixture>) -> CandidateId {
    runtime
        .view()
        .candidates
        .into_iter()
        .find(|row| row.source == 18)
        .unwrap()
        .id
}

#[test]
fn multilingual_candidate_converts_chinese_and_keeps_japanese_han() {
    let mut runtime = runtime_with(&[WORD], &[18]);
    assert_eq!(runtime.view().candidates[0].text, "hello網絡中国");
    assert_eq!(runtime.all_candidates().candidates[0].text, "hello網絡中国");
    assert_eq!(runtime.cached.candidates[0], WORD, "引擎选择身份保留原词面");
    runtime.set_multilingual_output(true, TraditionalVariant::TaiwanPhrases);
    assert_eq!(runtime.view().candidates[0].text, "hello網路中国");
    runtime.set_multilingual_output(false, TraditionalVariant::Standard);
    assert_eq!(runtime.view().candidates[0].text, WORD);
}

#[test]
fn multilingual_selection_commits_the_displayed_segments_without_host_conversion() {
    let mut runtime = runtime_with(&[WORD], &[18]);
    let committed = runtime
        .dispatch(Action::Select(mixed_id(&runtime)))
        .unwrap();
    assert_eq!(committed.commit.as_deref(), Some("hello網絡中国"));
    assert!(!committed.commit_context.unwrap().script_conversion);
    assert_eq!(runtime.ai_context, "hello網絡中国");
}

#[test]
fn multilingual_punctuation_converts_the_held_chinese_prefix_and_keeps_the_tail() {
    let mut runtime = runtime_with(&["海滩", WORD], &[0, 18]);
    runtime.engine.remaining_after_select = Some(RAW.into());
    let id = runtime.view().candidates[0].id;
    let held = runtime.dispatch(Action::Select(id)).unwrap();
    assert!(held.commit.is_none());
    assert_eq!(held.view.phrase_prefix, "海滩");
    runtime.dispatch(Action::NextCandidate).unwrap();
    let committed = runtime.dispatch(Action::Punctuation(b',')).unwrap();
    assert_eq!(
        committed.commit.as_deref(),
        Some("海灘hello網絡中国-remaining-segments，")
    );
    assert!(!committed.commit_context.unwrap().script_conversion);
}

#[test]
fn multilingual_metadata_follows_the_source_after_online_seating() {
    let mut runtime = runtime_with(&["海滩", WORD, "云候选"], &[0, 18, 2]);
    let row = runtime
        .view()
        .candidates
        .into_iter()
        .find(|row| row.source == 18)
        .unwrap();
    assert_ne!(row.id.index, 1, "夹具必须让在线整理移动混合行");
    assert_eq!(row.text, "hello網絡中国");
    let committed = runtime.dispatch(Action::Select(row.id)).unwrap();
    assert_eq!(committed.commit.as_deref(), Some("hello網絡中国"));
    assert!(!committed.commit_context.unwrap().script_conversion);
}

#[test]
fn multilingual_invalid_metadata_never_triggers_whole_line_conversion() {
    for invalid in 0..4 {
        let mut runtime = runtime_with(&[WORD], &[18]);
        let mut value = metadata();
        match invalid {
            0 => value.raw_input.push('x'),
            1 => value.conversion.segments[1].input.start += 1,
            2 => value.conversion.segments[1].text = "另一个词面".into(),
            _ => value.conversion.segments[2].input.end -= 1,
        }
        runtime.cached.multilingual_conversion = Some(value.clone());
        runtime.engine.multilingual_conversion = Some(value);
        assert_eq!(runtime.view().candidates[0].text, WORD);
        let committed = runtime
            .dispatch(Action::Select(mixed_id(&runtime)))
            .unwrap();
        assert_eq!(committed.commit.as_deref(), Some(WORD));
        assert!(!committed.commit_context.unwrap().script_conversion);
    }
}

#[test]
fn multilingual_selection_state_clears_before_the_next_chinese_commit() {
    let mut runtime = runtime_with(&[WORD], &[18]);
    runtime
        .dispatch(Action::Select(mixed_id(&runtime)))
        .unwrap();
    runtime.engine.words = vec!["网络".into()];
    runtime.engine.sources = vec![0];
    runtime.engine.multilingual_conversion = None;
    type_key(&mut runtime);
    let id = runtime.view().candidates[0].id;
    let committed = runtime.dispatch(Action::Select(id)).unwrap();
    assert_eq!(committed.commit.as_deref(), Some("网络"));
    assert!(committed.commit_context.unwrap().script_conversion);
}

#[test]
fn multilingual_edge_uses_the_language_of_the_chosen_han() {
    for (edge, expected) in [
        (CandidateEdge::FirstHan, "網"),
        (CandidateEdge::LastHan, "国"),
    ] {
        let mut runtime = runtime_with(&[WORD], &[18]);
        runtime.engine.edge_commits = Some(["网".into(), "国".into()]);
        let committed = runtime
            .dispatch(Action::SelectEdge(mixed_id(&runtime), edge))
            .unwrap();
        assert_eq!(committed.commit.as_deref(), Some(expected));
        assert!(!committed.commit_context.unwrap().script_conversion);
        assert_eq!(runtime.facts.selection.unwrap().source, 18);
    }
}

#[test]
fn multilingual_edge_distinguishes_equal_characters_in_different_languages() {
    for (edge, expected) in [
        (CandidateEdge::FirstHan, "網"),
        (CandidateEdge::LastHan, "网"),
    ] {
        let mut runtime = runtime_with(&["hello网络网"], &[18]);
        let mut value = metadata();
        value.conversion.segments[2].text = "网".into();
        runtime.cached.multilingual_conversion = Some(value.clone());
        runtime.engine.multilingual_conversion = Some(value);
        runtime.engine.edge_commits = Some(["网".into(), "网".into()]);
        let committed = runtime
            .dispatch(Action::SelectEdge(mixed_id(&runtime), edge))
            .unwrap();
        assert_eq!(committed.commit.as_deref(), Some(expected));
        assert!(!committed.commit_context.unwrap().script_conversion);
    }
}

#[test]
fn multilingual_rows_do_not_inherit_a_chinese_gloss() {
    let mut runtime = runtime_with(&[WORD], &[18]);
    runtime
        .translations
        .insert(WORD.into(), "synthetic gloss".into());
    assert!(runtime.view().candidates[0].translation.is_none());
    assert!(msime_client_core::translation::source_gets_no_gloss(18));
}

#[test]
fn multilingual_real_session_preserves_identity_and_segment_output() {
    for route in 0..6 {
        let root = tempfile::tempdir().unwrap();
        let mut options = real_engine_options(root.path());
        options.mixed_english = false;
        options.multilingual_languages = vec![Language::Chinese, Language::English];
        let dictionaries = std::path::Path::new(&options.dictionaries);
        rusqlite::Connection::open(dictionaries.join("msime-pinyin.db"))
            .unwrap()
            .execute_batch(
                "CREATE TABLE tbl_2_w(key TEXT,jp TEXT,value TEXT,weight INTEGER);
                 INSERT INTO tbl_2_w VALUES('wang''luo','wl','网络',100);
                 CREATE TABLE tbl_1_h(key TEXT,jp TEXT,value TEXT,weight INTEGER);
                 INSERT INTO tbl_1_h VALUES('he','h','喝',100);",
            )
            .unwrap();
        let english = dictionaries.join("msime-english.db");
        msime_engine::ensure_english_schema(&english).unwrap();
        rusqlite::Connection::open(english)
            .unwrap()
            .execute_batch("INSERT INTO english_words VALUES('hello','hello',100);")
            .unwrap();
        let mut runtime =
            Runtime::new(msime_engine::host::Session::new(&options).unwrap(), 9).unwrap();
        runtime.set_multilingual_output(true, TraditionalVariant::Standard);
        runtime.set_phrase_preedit(true);
        runtime.focus(true).unwrap();
        for value in b"hellowangluo" {
            runtime
                .dispatch(Action::Character {
                    value: *value,
                    shift: false,
                })
                .unwrap();
        }
        let row = runtime
            .view()
            .candidates
            .into_iter()
            .find(|row| row.source == 18)
            .unwrap();
        assert_eq!(row.text, "hello網絡");
        assert_eq!(row.code, "hellowangluo");
        assert_eq!(runtime.cached.candidates[row.id.index], "hello网络");
        if route == 5 {
            let count = runtime.cached.candidates.len();
            assert!(count > 1, "重排夹具应含有实际备用行");
            assert_eq!(row.id.index, 0, "引擎首行应为混合候选");
            let snapshot = &mut runtime.cached;
            snapshot.candidates.rotate_left(1);
            snapshot.candidate_codes.rotate_left(1);
            snapshot.candidate_annotations.rotate_left(1);
            snapshot.candidate_sources.rotate_left(1);
            snapshot.candidate_positions.rotate_left(1);
            snapshot.candidate_corrected.rotate_left(1);
            snapshot.candidate_answers_key.rotate_left(1);
            snapshot.candidate_static_evidence.rotate_left(1);
            runtime.engine_order = (0..count).collect();
            runtime.engine_order.rotate_left(1);
            assert_ne!(runtime.cached.candidate_sources[0], 18);
        }
        let (action, expected) = match route {
            0 => (Action::Select(row.id), "hello網絡"),
            1 => {
                while runtime.highlighted != row.id.index {
                    runtime.dispatch(Action::NextCandidate).unwrap();
                }
                (Action::Finish, "hello網絡")
            }
            2 => (Action::SelectEdge(row.id, CandidateEdge::FirstHan), "網"),
            3 => (Action::SelectEdge(row.id, CandidateEdge::LastHan), "絡"),
            _ => (Action::Command(Command::CommitCandidate), "hello網絡"),
        };
        let expected_seat = if route == 5 {
            runtime.cached.candidates.len() - 1
        } else {
            0
        };
        let committed = runtime.dispatch(action).unwrap();
        assert_eq!(committed.commit.as_deref(), Some(expected), "route {route}");
        assert!(!committed.commit_context.unwrap().script_conversion);
        assert!(runtime.cached.multilingual_conversion.is_none());
        if route >= 4 {
            let selection = runtime.facts.selection.unwrap();
            assert_eq!(selection.source, 18);
            assert_eq!(selection.how, SelectionHow::FirstCandidateCommand);
            assert_eq!(selection.seat as usize, expected_seat);
        }
    }
}

#[test]
fn multilingual_segment_changes_invalidate_the_display_epoch() {
    let mut runtime = runtime_with(&[WORD], &[18]);
    let before = runtime.candidates_epoch;
    assert_eq!(runtime.view().candidates[0].text, "hello網絡中国");
    runtime
        .engine
        .multilingual_conversion
        .as_mut()
        .unwrap()
        .conversion
        .segments[1]
        .language = Language::Japanese;
    let refreshed = runtime.dispatch(Action::ResetCache).unwrap();
    assert_eq!(refreshed.view.candidates[0].text, WORD);
    assert_ne!(runtime.candidates_epoch, before);
}
