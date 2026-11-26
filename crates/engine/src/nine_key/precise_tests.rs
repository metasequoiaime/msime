const PRECISE_DICTIONARY: &str = "CREATE TABLE tbl_1_g(key TEXT,jp TEXT,value TEXT,weight INTEGER);
     INSERT INTO tbl_1_g VALUES('ge','g','歌',100),('gen','g','根',1000);
     CREATE TABLE tbl_2_g(key TEXT,jp TEXT,value TEXT,weight INTEGER);
     INSERT INTO tbl_2_g VALUES('ge''men','gm','哥们',100000),('gan''en','ge','感恩',1000);
     CREATE TABLE tbl_2_h(key TEXT,jp TEXT,value TEXT,weight INTEGER);
     INSERT INTO tbl_2_h VALUES('hen''e','he','很饿',100),('hen''duo','hd','很多',200);
     CREATE TABLE tbl_4_w(key TEXT,jp TEXT,value TEXT,weight INTEGER);
     INSERT INTO tbl_4_w VALUES('wo''men''jin''tian','wmjt','我们今天',10000),
                                ('wo''men''jin''ti','wmjt','我们今提',100);";

#[test]
fn precise_nine_key_column_lists_only_typed_syllables_and_first_key_letters() {
    let fixture = fixture_with(PRECISE_DICTIONARY);
    let mut session = open(&fixture.paths, false, EnglishInputOptions::default());
    type_digits(&mut session, "2");
    assert!(
        session.spellings.iter().any(|item| item == "cong"),
        "出厂继续提供预测音节"
    );
    session.command(Command::Cancel);
    session.set_precise_nine_key(true);

    for (digits, expected) in [
        ("2", &["A", "B", "C"][..]),
        ("63", &["me", "ne", "M", "N", "O"][..]),
        ("4363", &["gen", "hen", "ge", "he", "G", "H"][..]),
    ] {
        type_digits(&mut session, digits);
        let mut actual: Vec<_> = session.spellings.iter().map(String::as_str).collect();
        actual.sort_unstable();
        let mut expected = expected.to_vec();
        expected.sort_unstable();
        assert_eq!(actual, expected, "{digits}");
        session.command(Command::Cancel);
    }
}

#[test]
fn precise_nine_key_letter_lock_consumes_one_key_in_mixed_initials() {
    let fixture = fixture_with(PRECISE_DICTIONARY);
    let mut session = open(&fixture.paths, false, EnglishInputOptions::default());
    session.set_precise_nine_key(true);
    type_digits(&mut session, "436");
    assert!(
        session
            .choose_spelling(spelling_index(&session, "G"))
            .handled
    );
    assert_eq!(session.snapshot().editing_text, "436");
    assert_eq!(session.locked, ["g"]);
    assert_eq!(session.snapshot().nine_key_reading, "g'en");
    assert!(session.spellings.iter().any(|item| item == "en"));
    assert!(!session.spellings.iter().any(|item| item == "gen"));
    let index = index_of(&session, "感恩");
    assert_eq!(session.candidates[index].pinyin.as_str(), "436");
    assert_eq!(session.select(index).commit.as_deref(), Some("感恩"));
    assert!(!session.active());
}

#[test]
fn precise_nine_key_keeps_mixed_initials_and_ranks_exact_before_completion() {
    let fixture = fixture_with(PRECISE_DICTIONARY);
    let mut session = open(&fixture.paths, false, EnglishInputOptions::default());
    session.set_precise_nine_key(true);
    type_digits(&mut session, "4363");
    assert!(words(&session).contains(&"很饿".to_owned()));
    assert!(words(&session).contains(&"很多".to_owned()));
    assert!(!words(&session).contains(&"哥们".to_owned()));
    session.command(Command::Cancel);

    type_digits(&mut session, "9663654684");
    assert!(index_of(&session, "我们今提") < index_of(&session, "我们今天"));
    let snapshot = session.snapshot();
    assert!(snapshot.candidate_answers_key[index_of(&session, "我们今提")]);
    assert!(!snapshot.candidate_answers_key[index_of(&session, "我们今天")]);
    type_digits(&mut session, "2");
    assert!(words(&session).contains(&"我们今天".to_owned()));
}
