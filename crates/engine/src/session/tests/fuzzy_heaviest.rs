use super::*;

#[test]
fn selecting_the_heaviest_fuzzy_row_records_learning_after_the_trigger() {
    for mode in [
        FrequencyAdjustmentMode::Pin,
        FrequencyAdjustmentMode::Promote,
    ] {
        let fixture = Fixture::new(
            "CREATE TABLE tbl_2_z(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_2_z VALUES('zi''qian','zq','自谦',300),('zi''qian','zq','自前',200);\
INSERT INTO tbl_2_z VALUES('zhi''qian','zq','之前',1000),('zhi''qian','zq','支前',900);",
        );
        let create = || {
            Fixture::session_with(&fixture, |options| {
                options.helpcode = false;
                options.personal_context = false;
                options.fuzzy_pinyin.rules = crate::types::fuzzy_rule::Z_ZH;
                options.frequency.mode = mode;
                options.frequency.trigger_count = 2;
            })
        };
        for round in 0..2 {
            let mut session = create();
            type_text(&mut session, "ziqian");
            assert!(index_of(&session, "之前") >= 2, "触发前保留字面优先");
            let result = select_word(&mut session, "之前");
            assert_eq!(result.commit.as_deref(), Some("之前"));
            assert_eq!(result.diagnostic, None);
            let journal = Connection::open(fixture.journal()).unwrap();
            let recorded: i64 = journal.query_row(
                "SELECT count(*) FROM user_dictionary_operations WHERE key='zhi''qian' AND value='之前' AND weight=1000",
                [], |row| row.get(0),
            ).unwrap();
            assert_eq!(recorded, i64::from(round == 1), "遵守选择次数阈值");
        }
        let mut session = create();
        type_text(&mut session, "ziqian");
        assert_eq!(words(&session)[0], "之前", "最高权重的模糊行也要能跨组");
        assert!(
            index_of(&session, "支前") > index_of(&session, "自前"),
            "未学习模糊行不随同提升"
        );
    }
}
