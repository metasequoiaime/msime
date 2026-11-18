//! 混合声母整句通过共享会话消费实际按键，并按完整读音学习。

use super::*;

#[test]
fn mixed_initial_sentence_selection_consumes_keys_and_learns_full_reading() {
    for kind in [
        ShuangpinProfileKind::Xiaohe,
        ShuangpinProfileKind::Ziranma,
        ShuangpinProfileKind::Shoudao,
        ShuangpinProfileKind::Microsoft,
        ShuangpinProfileKind::SmartAbc,
    ] {
        let profile = crate::shuangpin::profile::profile(kind).unwrap();
        let encoded = crate::shuangpin::utils::encode_pinyin("ni'hao'te'le", profile).unwrap();
        let mut parts: Vec<_> = encoded.split('\'').map(str::to_owned).collect();
        parts[1].truncate(1);
        parts[3].truncate(1);
        let typed = parts.join("'");
        let fixture = Fixture::new(&format!(
            "{QUANPIN_FIXTURE} CREATE TABLE tbl_4_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);"
        ));
        let mut session = fixture.session_with(|options| {
            options.scheme = SchemeType::Shuangpin;
            options.shuangpin_profile = kind;
            options.helpcode = false;
            options.autocorrect_types = 0;
        });
        type_text(&mut session, &typed);
        assert_eq!(words(&session)[0], "你好特乐", "{kind:?}");
        let snapshot = session.snapshot();
        assert_eq!(snapshot.candidates[0].canonical_pinyin, "ni'hao'te'le");
        assert_eq!(snapshot.candidates[0].pinyin, typed.replace('\'', ""));
        let selected = session.select(0);
        assert_eq!(selected.diagnostic, None);
        assert_eq!(selected.commit.as_deref(), Some("你好特乐"));
        assert!(session.snapshot().editing_text.is_empty());
        let journal = Connection::open(fixture.journal()).unwrap();
        let learned: i64 = journal
            .query_row(
                "SELECT count(*) FROM user_dictionary_operations \
             WHERE key='ni''hao''te''le' AND value='你好特乐' AND operation<>'delete'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert!(learned > 0, "{kind:?}");
    }
}
