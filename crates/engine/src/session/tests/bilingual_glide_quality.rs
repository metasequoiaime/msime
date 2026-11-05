//! 连滑保留容量、双向语言弱先验与英文缩略的行为验证。

use super::*;
use crate::pinyin::glide::tests::{keyboard, stroke};

fn swipe(session: &mut Session, spelling: &str) {
    let result = session.bilingual_glide(&keyboard(), &stroke(spelling, 0.0, &[]));
    assert!(result.handled, "{spelling}");
    assert_eq!(result.commit, None, "下一笔不能自动上屏：{spelling}");
    assert!(session.requires_candidate_selection());
    assert!(words(session).len() <= 24);
}

#[test]
fn pending_language_hint_follows_the_last_word_and_allows_both_switch_directions() {
    let fixture = Fixture::new(&format!(
        "{QUANPIN_FIXTURE}\
CREATE TABLE tbl_2_w(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_2_w VALUES('wo''men','wm','我们',1000);"
    ))
    .with_english("INSERT INTO english_words VALUES('hello','hello',1000),('women','women',1000);");
    for (spellings, expected) in [
        (vec!["hello", "women"], "hello women"),
        (vec!["nihao", "women"], "你好我们"),
        (vec!["nihao", "hello", "women"], "你好 hello women"),
        (vec!["hello", "nihao", "women"], "hello 你好我们"),
    ] {
        let mut session = fixture.session();
        for spelling in spellings {
            swipe(&mut session, spelling);
        }
        assert_eq!(words(&session)[0], expected);
        let committed = session.command(Command::CommitCandidate).commit.unwrap();
        assert_eq!(committed.trim_end(), expected);
        assert!(session.snapshot().preedit.is_empty());
    }
}

#[test]
fn a_fourth_earlier_alternative_survives_three_swipes() {
    let fixture = Fixture::new(QUANPIN_FIXTURE).with_english(
        "INSERT INTO english_words VALUES\
('hello','hello',1000),('hello','Hello',900),('hello','HELLO',800),('hello','HeLLo',700),\
('world','world',1000),('thanks','thanks',1000);",
    );
    let mut session = fixture.session();
    for spelling in ["hello", "world", "thanks"] {
        swipe(&mut session, spelling);
    }
    assert!(words(&session).contains(&"HeLLo world thanks".to_owned()));
    assert_eq!(
        select_word(&mut session, "HeLLo world thanks")
            .commit
            .as_deref(),
        Some("HeLLo world thanks ")
    );
}

#[test]
fn a_seventh_following_alternative_survives_the_next_swipe() {
    let fixture = Fixture::new(QUANPIN_FIXTURE).with_english(
        "INSERT INTO english_words VALUES\
('hello','hello',1000),('hello','Hello',900),('hello','HELLO',800),\
('thanks','thanks',1000),\
('world','world',1000),('world','World',900),('world','WORLD',800),\
('world','wOrld',700),('world','woRld',600),('world','worLd',500),('world','worlD',400);",
    );
    let mut session = fixture.session();
    for spelling in ["hello", "world"] {
        swipe(&mut session, spelling);
    }
    assert_eq!(
        &words(&session)[..5],
        [
            "hello world",
            "hello World",
            "hello WORLD",
            "hello wOrld",
            "hello woRld"
        ]
    );
    swipe(&mut session, "thanks");
    assert!(words(&session).contains(&"hello worlD thanks".to_owned()));
}

#[test]
fn full_chinese_results_do_not_fill_every_slot_and_hide_english_combinations() {
    let mut sql = String::from(
        "CREATE TABLE tbl_1_w(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_w VALUES('wo','w','我',1000);\
CREATE TABLE tbl_1_m(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_m VALUES('men','m','们',1000);\
CREATE TABLE tbl_2_w(key TEXT,jp TEXT,value TEXT,weight INTEGER);",
    );
    for index in 0..24 {
        let value = if index == 0 {
            "我们".to_owned()
        } else {
            format!("合成词{index}")
        };
        sql.push_str(&format!(
            "INSERT INTO tbl_2_w VALUES('wo''men','wm','{value}',{});",
            2000 - index
        ));
    }
    let fixture =
        Fixture::new(&sql).with_english("INSERT INTO english_words VALUES('men','men',1000);");
    let mut session = fixture.session();
    swipe(&mut session, "wo");
    swipe(&mut session, "men");
    assert_eq!(words(&session)[0], "我们");
    assert!(
        words(&session).contains(&"我 men".to_owned()),
        "{:?}",
        words(&session)
    );
    assert_eq!(
        select_word(&mut session, "我 men").commit.as_deref(),
        Some("我 men ")
    );
}

#[test]
fn common_contractions_restore_punctuation_and_capital_i_when_selected() {
    let fixture = Fixture::new(QUANPIN_FIXTURE).with_english(
        "INSERT INTO english_words VALUES\
('do','do',1000),('can','can',1000),('me','me',1000),('it','it',1000),\
('we','we',1000),('you','you',1000),('will','will',1000),('let','let',1000),\
('ill','ill',1000),('its','its',1000),('were','were',1000);",
    );
    for (spelling, expected, plain) in [
        ("dont", "don't", None),
        ("cant", "can't", None),
        ("im", "I'm", None),
        ("ill", "I'll", Some("ill")),
        ("ive", "I've", None),
        ("its", "it's", Some("its")),
        ("were", "we're", Some("were")),
        ("youre", "you're", None),
        ("wont", "won't", None),
        ("lets", "let's", None),
    ] {
        let mut session = fixture.session();
        swipe(&mut session, spelling);
        assert!(
            words(&session).contains(&expected.to_owned()),
            "{spelling}: {:?}",
            words(&session)
        );
        if let Some(plain) = plain {
            assert!(words(&session).contains(&plain.to_owned()));
        }
        assert_eq!(
            select_word(&mut session, expected).commit,
            Some(format!("{expected} "))
        );
        assert!(session.snapshot().preedit.is_empty());
    }
}

#[test]
fn contraction_aliases_require_an_available_base_word() {
    let fixture = Fixture::new(QUANPIN_FIXTURE).with_english("");
    let mut session = fixture.session();
    session.bilingual_glide(&keyboard(), &stroke("dont", 0.0, &[]));
    assert!(!words(&session).contains(&"don't".to_owned()));
}
