//! 双拼与句中声母混输的合成词库回归。

use super::*;

const MIXED_FIXTURE: &str = "CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
     INSERT INTO tbl_2_n VALUES('ni''hao','nh','你好',1000);\
     CREATE TABLE tbl_2_c(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
     INSERT INTO tbl_2_c VALUES('ce''ke','ck','测科',900),\
     ('che''ke','ck','车科',90000),('ce''ken','ck','测肯',80000);\
     CREATE TABLE tbl_2_g(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
     INSERT INTO tbl_2_g VALUES('gong''ju','gj','工具',800);";

#[test]
fn middle_initial_and_incomplete_tail_compose_the_whole_sentence() {
    let fixture = Fixture::new(MIXED_FIXTURE);
    let mut engine = fixture.engine(ShuangpinProfileKind::Xiaohe);
    for _ in 0..2 {
        for typed in ["nihcckegsju", "nihcckegsj", "ni'hc'c'ke'gs'j"] {
            let rows = engine.query(&request(typed, false), None);
            assert_eq!(words(&rows)[0], "你好测科工具", "{typed}");
            assert_eq!(rows[0].canonical_pinyin, "ni'hao'ce'ke'gong'ju");
            assert_eq!(rows[0].pinyin, typed.replace('\'', ""));
            assert!(rows[0].corrected_from.is_empty());
            assert!(rows[0].sentence_association);
            assert_eq!(rows[0].word.chars().count(), 6);
            assert!(words(&rows).contains(&"你好"));
        }
    }
}

#[test]
fn mixed_initials_follow_every_builtin_profile_with_manual_boundaries() {
    let fixture = Fixture::new(MIXED_FIXTURE);
    for kind in [
        ShuangpinProfileKind::Xiaohe,
        ShuangpinProfileKind::Ziranma,
        ShuangpinProfileKind::Shoudao,
        ShuangpinProfileKind::Microsoft,
        ShuangpinProfileKind::SmartAbc,
    ] {
        let profile = profile(kind).unwrap();
        let full = super::super::utils::encode_pinyin("ni'hao'ce'ke'gong'ju", profile).unwrap();
        let mut parts: Vec<_> = full.split('\'').map(str::to_owned).collect();
        parts[2].truncate(1);
        parts[5].truncate(1);
        let typed = parts.join("'");
        let mut engine = fixture.engine(kind);
        let rows = engine.query(&request(&typed, false), None);
        assert_eq!(words(&rows)[0], "你好测科工具", "{kind:?}: {typed}");
        assert_eq!(rows[0].canonical_pinyin, "ni'hao'ce'ke'gong'ju");
    }
}

#[test]
fn mixed_initial_sentences_obey_the_lattice_switch() {
    let fixture = Fixture::new(MIXED_FIXTURE);
    let mut engine = fixture.engine(ShuangpinProfileKind::Xiaohe);
    let mut typed = request("nihcckegsj", false);
    typed.sentence_association.word_lattice = false;
    assert!(!engine
        .query(&typed, None)
        .iter()
        .any(|row| row.sentence_association));
    typed.sentence_association.word_lattice = true;
    assert_eq!(words(&engine.query(&typed, None))[0], "你好测科工具");
    typed.sentence_association.word_lattice = false;
    assert!(!engine
        .query(&typed, None)
        .iter()
        .any(|row| row.sentence_association));
}

#[test]
fn mixed_sentence_prefix_keeps_its_consumed_range_before_an_unmatched_tail() {
    let fixture = Fixture::new(MIXED_FIXTURE);
    let mut engine = fixture.engine(ShuangpinProfileKind::Xiaohe);
    let rows = engine.query(&request("nihcckegsjux", false), None);
    assert_eq!(words(&rows)[0], "你好测科工具");
    assert_eq!(rows[0].pinyin, "nihcckegsju");
}

#[test]
fn mixed_initial_sentence_filters_helpcodes_before_decoding() {
    let fixture = Fixture::new(MIXED_FIXTURE);
    Connection::open(fixture.paths.dictionary(assets::MAIN_DICTIONARY))
        .unwrap()
        .execute_batch("INSERT INTO tbl_2_c VALUES('ce''ke','ck','策科',1);")
        .unwrap();
    let codes = keymap(&[("测", "ab"), ("策", "cd")]);
    let mut typed = request("nihcckegsj", false);
    typed.shuangpin_helpcode_modes.mid_sentence = true;
    typed.syllable_helpcodes = vec![crate::types::SyllableHelpcode {
        syllable: 2,
        first: b'c',
        second: 0,
    }];
    let mut engine = fixture.engine(ShuangpinProfileKind::Xiaohe);
    let rows = engine.query(&typed, Some(&codes));
    assert_eq!(words(&rows)[0], "你好策科工具");
    assert!(!words(&rows).contains(&"你好测科工具"));
    typed.syllable_helpcodes.clear();
    let rows = engine.query(&typed, Some(&codes));
    assert_eq!(words(&rows)[0], "你好测科工具");
}

// #11766 的复现：7 个声母、没有任何完整音节。旧版有 Google 整句，Rust 侧只把带完整音节的
// 混输交给词网格，纯声母串一条整句边都没有，首选退回 `mtt` 前缀组的三字词。
const PURE_INITIAL_FIXTURE: &str =
    "CREATE TABLE tbl_2_m(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
     INSERT INTO tbl_2_m VALUES('ming''tian','mt','明天',9000);\
     CREATE TABLE tbl_2_t(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
     INSERT INTO tbl_2_t VALUES('tian''qi','tq','天气',9000);\
     CREATE TABLE tbl_3_z(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
     INSERT INTO tbl_3_z VALUES('zen''me''yang','zmy','怎么样',9000);";

#[test]
fn pure_initial_string_decodes_the_whole_sentence() {
    let fixture = Fixture::new(PURE_INITIAL_FIXTURE);
    let mut engine = fixture.engine(ShuangpinProfileKind::Xiaohe);
    let candidate_pinyin = "ming'tian'tian'qi'zen'me'yang";
    for typed in ["mttqzmy", "m't't'q'z'm'y", "mttqizmy"] {
        let rows = engine.query(&request(typed, false), None);
        assert_eq!(words(&rows)[0], "明天天气怎么样", "{typed}");
        assert_eq!(rows[0].canonical_pinyin, candidate_pinyin, "{typed}");
        assert!(rows[0].sentence_association, "{typed}");
        assert_eq!(rows[0].word.chars().count(), 7, "{typed}");
    }
}

#[test]
fn pure_initial_strings_obey_the_lattice_switch() {
    let fixture = Fixture::new(PURE_INITIAL_FIXTURE);
    let mut engine = fixture.engine(ShuangpinProfileKind::Xiaohe);
    let mut typed = request("mttqzmy", false);
    typed.sentence_association.word_lattice = false;
    assert!(!engine
        .query(&typed, None)
        .iter()
        .any(|row| row.sentence_association));
    typed.sentence_association.word_lattice = true;
    assert_eq!(words(&engine.query(&typed, None))[0], "明天天气怎么样");
}
