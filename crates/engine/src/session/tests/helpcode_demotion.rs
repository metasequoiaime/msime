use super::*;

#[test]
fn xiaohe_helpcode_demotes_the_previous_first_through_the_session() {
    let fixture = Fixture::new(
        "CREATE TABLE tbl_1_h(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
         INSERT INTO tbl_1_h VALUES('he','h','合',200),('he','h','何',100);\
         CREATE TABLE tbl_1_s(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
         INSERT INTO tbl_1_s VALUES('shi','s','是',200),('shi','s','时',100);",
    );
    std::fs::write(
        fixture.path().join("helpcodes/helpcode.txt"),
        "合=rk\n何=rk\n是=or\n时=oc\n",
    )
    .unwrap();
    for mid in [false, true] {
        for (raw, typed, first, expected) in [
            ("he", "R", "合", "何"),
            ("he", "RK", "合", "何"),
            ("he", "rK", "合", "何"),
            ("ui", "O", "是", "时"),
        ] {
            // 末尾直接协议的两码顺序是 rK；句中大写段的两码顺序是 RK。
            if !mid && typed == "RK" {
                continue;
            }
            let mut session = fixture.session_with(|options| {
                options.scheme = SchemeType::Shuangpin;
                options.shuangpin_profile = ShuangpinProfileKind::Xiaohe;
                options.shuangpin_helpcode_modes.mid_sentence = mid;
                options.shuangpin_helpcode_modes.mid_sentence_uppercase = mid;
            });
            for _ in 0..2 {
                type_text(&mut session, raw);
                assert_eq!(words(&session)[0], first);
                for byte in typed.bytes() {
                    assert!(session.character(byte, byte.is_ascii_uppercase()).handled);
                }
                assert_eq!(words(&session)[0], expected, "{raw}{typed}, mid={mid}");
                for _ in typed.bytes() {
                    session.command(Command::Backspace);
                }
                assert_eq!(session.snapshot().editing_text, raw);
                assert_eq!(words(&session)[0], first);
                session.command(Command::Cancel);
            }
            type_text(&mut session, raw);
            for byte in typed.bytes() {
                session.character(byte, byte.is_ascii_uppercase());
            }
            assert_eq!(session.select(0).commit.as_deref(), Some(expected));
        }
    }
}
