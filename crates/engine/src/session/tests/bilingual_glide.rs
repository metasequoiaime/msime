//! 同笔中英识别、候选选择和组字生命周期的行为回归。

use super::*;

#[test]
fn bilingual_glide_keeps_complete_readings_instead_of_dictionary_prefix_completions() {
    use crate::pinyin::glide::tests::{keyboard, stroke};
    let fixture = Fixture::new(
        "CREATE TABLE tbl_2_w(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_2_w VALUES('wo''men','wm','我们',1000);\
CREATE TABLE tbl_3_w(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_3_w VALUES('wo''men''de','wmd','我们的',100000);",
    )
    .with_english("INSERT INTO english_words VALUES('women','women',1000);");
    let mut session = fixture.session();
    assert!(
        session
            .bilingual_glide(&keyboard(), &stroke("women", 0.0, &[]))
            .handled
    );
    let candidates = words(&session);
    assert!(candidates.contains(&"我们".to_owned()), "{candidates:?}");
    assert!(candidates.contains(&"women".to_owned()), "{candidates:?}");
    assert!(
        !candidates.contains(&"我们的".to_owned()),
        "没滑入 de：{candidates:?}"
    );
}

#[test]
fn an_offset_english_glide_is_not_displaced_by_an_incomplete_chinese_reading() {
    use crate::pinyin::glide::tests::{keyboard, stroke};
    let fixture = Fixture::new(
        "CREATE TABLE tbl_2_h(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_2_h VALUES('he''long','hl','合拢',100000);",
    )
    .with_english("INSERT INTO english_words VALUES('hello','hello',1000);");
    let keyboard = keyboard();
    let mut points = stroke("helo", 0.0, &[]);
    for point in &mut points {
        point.x += keyboard.key_width * 0.24;
        point.y -= keyboard.key_height * 0.12;
    }
    let mut session = fixture.session();
    assert!(session.bilingual_glide(&keyboard, &points).handled);
    assert_eq!(words(&session).first().map(String::as_str), Some("hello"));
    assert!(!words(&session).contains(&"合拢".to_owned()));
}

#[test]
fn consecutive_chinese_glides_do_not_complete_an_unswiped_suffix() {
    use crate::pinyin::glide::tests::{keyboard, stroke};
    let fixture = Fixture::new(
        "CREATE TABLE tbl_1_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_n VALUES('ni','n','你',1000);\
CREATE TABLE tbl_1_h(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_h VALUES('hao','h','好',1000);\
CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_2_n VALUES('ni''hao','nh','你好',2000);\
CREATE TABLE tbl_3_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_3_n VALUES('ni''hao''ma','nhm','你好吗',100000);",
    );
    let mut session = fixture.session();
    for spelling in ["ni", "hao"] {
        let result = session.bilingual_glide(&keyboard(), &stroke(spelling, 0.0, &[]));
        assert!(result.handled);
        assert_eq!(result.commit, None);
    }
    let candidates = words(&session);
    assert!(candidates.contains(&"你好".to_owned()), "{candidates:?}");
    assert!(
        !candidates.contains(&"你好吗".to_owned()),
        "没滑入 ma：{candidates:?}"
    );
    assert_eq!(
        session.command(Command::CommitCandidate).commit.as_deref(),
        Some("你好")
    );
}

#[test]
fn bilingual_glides_offer_both_languages_and_commit_without_switching() {
    use crate::pinyin::glide::tests::{keyboard, stroke};
    let fixture = Fixture::new(QUANPIN_FIXTURE).with_english(
        "INSERT INTO english_words VALUES('hello','hello',1000),('world','world',900);",
    );
    let mut session = fixture.session();
    assert!(
        session
            .bilingual_glide(&keyboard(), &stroke("nihao", 0.05, &[]))
            .handled
    );
    assert!(
        words(&session).contains(&"你好".to_owned()),
        "{:?}",
        words(&session)
    );
    assert_eq!(
        select_word(&mut session, "你好").commit.as_deref(),
        Some("你好")
    );
    assert!(
        session
            .bilingual_glide(&keyboard(), &stroke("hello", 0.05, &[]))
            .handled
    );
    assert!(
        words(&session).contains(&"hello".to_owned()),
        "{:?}",
        words(&session)
    );
    assert_eq!(
        select_word(&mut session, "hello").commit.as_deref(),
        Some("hello ")
    );
    assert!(!session.snapshot().dedicated_english);
    assert!(
        session
            .bilingual_glide(&keyboard(), &stroke("nihao", 0.05, &[]))
            .handled
    );
    assert!(words(&session).contains(&"你好".to_owned()));
}

#[test]
fn bilingual_glide_preserves_identical_spelling_choices_and_clears_on_edit() {
    use crate::pinyin::glide::tests::{keyboard, stroke};
    let fixture = Fixture::new(
        "CREATE TABLE tbl_2_w(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_2_w VALUES('wo''men','wm','我们',1000);",
    )
    .with_english("INSERT INTO english_words VALUES('women','women',900);");
    let mut session = fixture.session();
    assert!(
        session
            .bilingual_glide(&keyboard(), &stroke("women", 0.05, &[]))
            .handled
    );
    assert!(
        words(&session).contains(&"我们".to_owned()),
        "{:?}",
        words(&session)
    );
    assert!(words(&session).contains(&"women".to_owned()));
    let mut invalid = keyboard();
    invalid.key_width = 0.0;
    let before = session.snapshot();
    assert!(
        !session
            .bilingual_glide(&invalid, &stroke("hello", 0.0, &[]))
            .handled
    );
    assert_eq!(session.snapshot().candidates, before.candidates);
    session.command(Command::Backspace);
    assert!(session.input.glide_candidates.is_none());
    session.command(Command::Cancel);
    assert!(session.snapshot().candidates.is_empty());
}

#[test]
fn a_following_bilingual_glide_waits_for_explicit_phrase_selection() {
    use crate::pinyin::glide::tests::{keyboard, stroke};
    let fixture = Fixture::new(HELLO_FIXTURE).with_english(
        "INSERT INTO english_words VALUES('hello','hello',1000),('world','world',900);",
    );
    let mut session = fixture.session();
    assert!(
        session
            .bilingual_glide(&keyboard(), &stroke("hello", 0.0, &[]))
            .handled
    );
    assert_eq!(
        session
            .bilingual_glide(&keyboard(), &stroke("world", 0.0, &[]))
            .commit
            .as_deref(),
        None
    );
    assert_eq!(
        select_word(&mut session, "hello world").commit.as_deref(),
        Some("hello world ")
    );
    assert!(session.snapshot().preedit.is_empty());
    assert!(
        session
            .bilingual_glide(&keyboard(), &stroke("hello", 0.0, &[]))
            .handled
    );
    let before = session.snapshot().candidates;
    assert!(!session.select(usize::MAX).handled);
    assert_eq!(session.snapshot().candidates, before);
    assert!(session.finish(usize::MAX).handled);
    assert!(!session.snapshot().preedit.is_empty());
    session.command(Command::Cancel);
    assert!(session.snapshot().preedit.is_empty());
}

#[test]
fn consecutive_chinese_glides_keep_the_whole_phrase_uncommitted() {
    use crate::pinyin::glide::tests::{keyboard, stroke};
    let fixture = Fixture::new(QUANPIN_FIXTURE)
        .with_english("INSERT INTO english_words VALUES('hello','hello',1000);");
    let mut session = fixture.session();
    for (spelling, expected) in [("ni", "ni"), ("hao", "nihao"), ("nihao", "nihaonihao")] {
        let result = session.bilingual_glide(&keyboard(), &stroke(spelling, 0.0, &[]));
        assert!(result.handled);
        assert_eq!(result.commit, None, "中文下一笔不能替用户确认上一笔");
        assert_eq!(session.input.editing_text().replace('\'', ""), expected);
    }
    assert!(
        words(&session).contains(&"你好你好".to_owned()),
        "{:?}",
        words(&session)
    );
    assert_eq!(
        select_word(&mut session, "你好你好").commit.as_deref(),
        Some("你好你好")
    );
    assert!(session.snapshot().preedit.is_empty());
}

#[test]
fn pending_chinese_keeps_an_ambiguous_next_stroke_in_the_phrase() {
    use crate::pinyin::glide::tests::{keyboard, stroke};
    let fixture = Fixture::new(
        "CREATE TABLE tbl_1_w(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_w VALUES('wo','w','我',1000);\
CREATE TABLE tbl_1_m(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_m VALUES('men','m','们',1000);\
CREATE TABLE tbl_2_w(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_2_w VALUES('wo''men','wm','我们',2000);",
    )
    .with_english("INSERT INTO english_words VALUES('men','men',1000),('hello','hello',1000);");
    let mut session = fixture.session();
    assert!(
        session
            .bilingual_glide(&keyboard(), &stroke("wo", 0.0, &[]))
            .handled
    );
    let result = session.bilingual_glide(&keyboard(), &stroke("men", 0.0, &[]));
    assert_eq!(result.commit, None);
    assert!(
        words(&session).contains(&"我们".to_owned()),
        "{:?}",
        words(&session)
    );
    let english = session.bilingual_glide(&keyboard(), &stroke("hello", 0.0, &[]));
    assert_eq!(english.commit, None);
    assert!(words(&session).contains(&"我们 hello".to_owned()));
}

#[test]
fn bilingual_glide_never_commits_at_a_language_boundary_or_implicit_finish() {
    use crate::pinyin::glide::tests::{keyboard, stroke};
    let fixture = Fixture::new(QUANPIN_FIXTURE).with_english(
        "INSERT INTO english_words VALUES('hello','hello',1000),('world','world',900);",
    );
    for (strokes, expected) in [
        (vec!["nihao", "hello"], "你好 hello "),
        (vec!["hello", "nihao"], "hello 你好"),
        (vec!["hello", "world", "nihao"], "hello world 你好"),
    ] {
        let mut session = fixture.session();
        for spelling in strokes {
            let result = session.bilingual_glide(&keyboard(), &stroke(spelling, 0.0, &[]));
            assert!(result.handled);
            assert_eq!(result.commit, None, "滑行不能隐式上屏：{spelling}");
        }
        let before = session.snapshot().preedit;
        for command in [
            Command::CommitRaw,
            Command::CommitRawWithoutLearning,
            Command::CommitReading,
        ] {
            let result = session.command(command);
            assert!(result.handled);
            assert_eq!(result.commit, None);
            assert_eq!(session.snapshot().preedit, before);
        }
        assert_eq!(session.finish(0).commit, None);
        assert_eq!(session.punctuation(b',').commit, None);
        assert_eq!(session.snapshot().preedit, before);
        let candidate = expected.trim_end();
        assert!(
            words(&session).contains(&candidate.to_owned()),
            "{:?}",
            words(&session)
        );
        assert_eq!(
            select_word(&mut session, candidate).commit.as_deref(),
            Some(expected)
        );
        assert!(session.snapshot().preedit.is_empty());
    }
}

#[test]
fn editing_bilingual_glide_still_requires_selection_and_cancel_restores_normal_typing() {
    use crate::pinyin::glide::tests::{keyboard, stroke};
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    session.bilingual_glide(&keyboard(), &stroke("nihao", 0.0, &[]));
    session.command(Command::Backspace);
    assert_eq!(session.command(Command::CommitRaw).commit, None);
    assert_eq!(session.character(b'o', false).commit, None);
    assert_eq!(session.finish(0).commit, None);
    session.command(Command::Cancel);
    type_text(&mut session, "nihao");
    assert_eq!(
        session.command(Command::CommitCandidate).commit.as_deref(),
        Some("你好")
    );
    session.bilingual_glide(&keyboard(), &stroke("nihao", 0.0, &[]));
    while !session.snapshot().preedit.is_empty() {
        session.command(Command::Backspace);
    }
    assert!(!session.requires_candidate_selection());
    type_text(&mut session, "nihao");
    assert_eq!(
        session.command(Command::CommitCandidate).commit.as_deref(),
        Some("你好")
    );
}

#[test]
fn space_explicitly_selects_the_current_whole_bilingual_glide_candidate() {
    use crate::pinyin::glide::tests::{keyboard, stroke};
    let fixture = Fixture::new(QUANPIN_FIXTURE).with_english(
        "INSERT INTO english_words VALUES('hello','hello',1000),('world','world',900);",
    );
    for spellings in [
        ["ni", "hao"],
        ["hello", "world"],
        ["nihao", "hello"],
        ["hello", "nihao"],
    ] {
        let mut session = fixture.session();
        for spelling in spellings {
            assert_eq!(
                session
                    .bilingual_glide(&keyboard(), &stroke(spelling, 0.0, &[]))
                    .commit,
                None
            );
        }
        let mut expected = words(&session)[0].clone();
        if expected
            .as_bytes()
            .last()
            .is_some_and(u8::is_ascii_alphabetic)
        {
            expected.push(' ');
        }
        assert_eq!(
            session.command(Command::CommitCandidate).commit.as_deref(),
            Some(expected.as_str())
        );
        assert!(session.snapshot().preedit.is_empty());
        assert_eq!(session.command(Command::CommitCandidate).commit, None);
    }
}

#[test]
fn editing_a_chinese_glide_does_not_make_the_next_stroke_commit_the_prefix() {
    use crate::pinyin::glide::tests::{keyboard, stroke};
    let fixture = Fixture::new(QUANPIN_FIXTURE);
    let mut session = fixture.session();
    assert!(
        session
            .bilingual_glide(&keyboard(), &stroke("nihao", 0.0, &[]))
            .handled
    );
    session.command(Command::Backspace);
    assert!(session.input.glide_candidates.is_none());
    let before = session.input.editing_text().to_owned();
    let result = session.bilingual_glide(&keyboard(), &stroke("hao", 0.0, &[]));
    assert!(result.handled);
    assert_eq!(result.commit, None);
    assert!(session.input.editing_text().starts_with(&before));
    assert!(session.input.editing_text().len() > before.len());
}
