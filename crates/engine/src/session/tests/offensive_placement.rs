//! 粗俗词不当首选、不进歧义输入的首屏（`offensive_placement`），以及中文状态下英文坐优先位置的门槛（`english::takes_priority_slot`）。用例是 2026-11-04 在 dict-v2.0.41 上用真实词库实测到的问题，权重取自同一份词库，词库里只放到能复现的那几行。

use std::collections::BTreeMap;

use super::*;

/// `(全拼读音, 词, 权重)` 写成 `msime-pinyin.db` 的表：表名按音节数和首字母，`jp` 是各音节的首字母。
fn dictionary(rows: &[(&str, &str, i64)]) -> String {
    let mut tables: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for (key, value, weight) in rows {
        let syllables: Vec<&str> = key.split('\'').collect();
        let table = format!("tbl_{}_{}", syllables.len(), &key[..1]);
        let jp: String = syllables.iter().map(|syllable| &syllable[..1]).collect();
        tables.entry(table).or_default().push(format!(
            "('{}','{jp}','{value}',{weight})",
            key.replace('\'', "''")
        ));
    }
    tables
        .iter()
        .map(|(table, values)| {
            format!(
                "CREATE TABLE {table}(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO {table} VALUES{};",
                values.join(",")
            )
        })
        .collect()
}

/// 每页候选数。词库只放了几行，列表比默认的 6 行短，挪出首屏的行会落在列表末尾、看起来还在首屏里，所以按 3 行一页测。
const PAGE: usize = 3;

const ROWS: &[(&str, &str, i64)] = &[
    // 各串数字前缀上的常用单字，让歧义码的列表长过一页。
    ("mei", "没", 7_000_000),
    ("ne", "呢", 3_000_000),
    ("hao", "好", 9_000_000),
    ("gao", "高", 2_000_000),
    ("si", "四", 1_000_000),
    ("pi", "皮", 200_000),
    ("bu", "不", 30_000_000),
    ("cu", "粗", 100_000),
    ("ai", "爱", 3_000_000),
    ("ni", "你", 20_000_000),
    ("mi", "米", 500_000),
    ("sha", "杀", 488_163),
    ("nei", "内", 1_362_462),
    ("rong", "容", 27_657),
    ("nei'rong", "内容", 4_197_712),
    ("gan", "干", 2_000_000),
    ("ga", "尬", 9_000),
    ("gan'ga", "尴尬", 202_740),
    ("shi", "是", 60_000_000),
    ("tu", "图", 300_000),
    ("shi'tu", "试图", 412_344),
    ("shi'tu", "视图", 52_284),
    ("chu", "出", 1_316_134),
    ("ci", "次", 1_222_933),
    ("chu'ci", "初次", 60_582),
    ("chu'ci", "楚辞", 15_447),
    ("gen", "跟", 4_360_801),
    ("ta", "他", 13_506_151),
    ("gen'ta", "跟他", 198_748),
    ("gen'ta", "跟她", 90_243),
    ("bie", "别", 624_141),
    ("ji", "及", 2_726_986),
    ("ji", "鸡", 7_677),
    ("ba", "把", 8_805_251),
    ("ba", "吧", 8_469_475),
    ("bie'ji", "别急", 31_574),
    ("ji'ba", "鸡巴", 2_190),
    ("bie'ji'ba", "别鸡巴", 100),
    ("bie'ji'ba", "别急吧", 100),
    ("niu", "牛", 268_903),
    ("bi", "比", 4_971_047),
    ("niu'bi", "牛逼", 451_545),
    ("niu'bi", "纽币", 984),
    ("suo", "所", 2_142_240),
    ("yi", "以", 5_874_915),
    ("suo'yi", "所以", 15_339_675),
    ("suo'yi", "蓑衣", 2_714),
    ("xiang", "想", 3_225_417),
    ("xiang", "相", 262_451),
    ("he", "和", 62_462_588),
    ("he", "何", 180_016),
    ("sha", "啥", 497_574),
    ("sha", "沙", 46_827),
    ("sha'bi", "傻逼", 108_330),
    ("sha'bi", "沙币", 1_362),
    ("sha'bao", "沙包", 40_000),
    ("sui'bian", "随便", 738_263),
    ("she'bei", "设备", 723_644),
    ("na", "那", 5_580_163),
    ("na", "拿", 1_345_242),
    ("bo", "波", 300_000),
    ("le", "了", 90_000_000),
    ("bo'le", "拨了", 17_793),
    ("bo'le", "伯乐", 4_729),
];

const ENGLISH_ROWS: &str = "INSERT INTO english_words VALUES\
('negro','Negro',2489080),('gangbang','gangbang',10767141),('shitty','shitty',730189),\
('bitch','bitch',8302447),('hentai','hentai',34116380),('stowing','stowing',49889),('xiaohongshu','Xiaohongshu',1),\
('he','he',842847219),('na','na',44784650),('bole','bole',160649);";

fn fixture() -> Fixture {
    Fixture::new(&dictionary(ROWS)).with_english(ENGLISH_ROWS)
}

fn mixed_session(fixture: &Fixture) -> Session {
    fixture.session_with(|options| {
        options.english.mixed_candidates = true;
        options.helpcode = false;
        options.first_page_rows = PAGE;
    })
}

fn first_page(session: &Session) -> Vec<String> {
    words(session).into_iter().take(PAGE).collect()
}

fn nine_key_session(fixture: &Fixture) -> Session {
    let mut session = mixed_session(fixture);
    session.set_nine_key_enabled(true);
    session
}

/// 九键上整段命中的粗俗词、轻度粗口和英文粗俗词都不在首屏，往后翻照样有。
#[test]
fn nine_key_codes_keep_vulgar_words_off_the_first_page() {
    let fixture = fixture();
    for (digits, lead, hidden) in [
        ("63476", "内容", "Negro"),
        ("42642", "尴尬", "gangbang"),
        ("74488", "试图", "shitty"),
        ("24824", "初次", "bitch"),
        ("43682", "跟他", "hentai"),
        ("2435422", "别急吧", "别鸡巴"),
        ("64824", "纽币", "牛逼"),
    ] {
        let mut session = nine_key_session(&fixture);
        type_text(&mut session, digits);
        let page = first_page(&session);
        assert_eq!(
            page.first().map(String::as_str),
            Some(lead),
            "{digits}: {page:?}"
        );
        assert!(
            !page.iter().any(|word| word == hidden),
            "{digits}: {page:?}"
        );
        assert!(
            words(&session).iter().any(|word| word == hidden),
            "{digits} 翻页后应该还能选到 {hidden}：{:?}",
            words(&session)
        );
    }
}

/// 14 键也是一个键几个字母，同样处理。
#[test]
fn fourteen_key_codes_keep_vulgar_words_off_the_first_page() {
    let fixture = fixture();
    for (letters, lead, hidden) in [
        ("neirong", "内容", "Negro"),
        ("biejiba", "别急吧", "别鸡巴"),
    ] {
        let mut session = mixed_session(&fixture);
        session.set_key_grid(Some(KeyGrid::FourteenKey));
        for letter in letters.bytes() {
            assert!(session.grid_key(letter).handled, "{letters}");
        }
        let page = first_page(&session);
        assert_eq!(
            page.first().map(String::as_str),
            Some(lead),
            "{letters}: {page:?}"
        );
        assert!(
            !page.iter().any(|word| word == hidden),
            "{letters}: {page:?}"
        );
    }
}

/// 九键第 2 位只给常见的英文：SCOWL 补进来的冷僻词（所以 → stowing）和没有计数的贡献词（想 → Xiaohongshu）接在拼音后面，常见词照旧坐第 2 位。
#[test]
fn the_nine_key_second_slot_takes_only_common_english_words() {
    let fixture = fixture();
    for (digits, lead, rare) in [("78694", "所以", "stowing"), ("94264", "想", "Xiaohongshu")] {
        let mut session = nine_key_session(&fixture);
        type_text(&mut session, digits);
        let listed = words(&session);
        assert_eq!(listed[0], lead, "{digits}: {listed:?}");
        assert_ne!(listed[1], rare, "{digits}: {listed:?}");
        assert_eq!(
            listed.last().map(String::as_str),
            Some(rare),
            "{digits}: {listed:?}"
        );
    }
    let mut session = nine_key_session(&fixture);
    type_text(&mut session, "43");
    assert_eq!(&words(&session)[..2], ["和", "he"]);
}

/// 26 键上把粗俗词的完整拼写打全时它只让出首选；简拼和没打完的拼写查出来的不进首屏。轻度粗口在 26 键上不动。
#[test]
fn typed_pinyin_keeps_vulgar_words_off_the_lead_and_partial_inputs_off_the_page() {
    let fixture = fixture();
    for scheme in [SchemeType::Quanpin, SchemeType::Shuangpin] {
        let mut session = fixture.session_with(|options| {
            options.scheme = scheme;
            options.helpcode = false;
            options.first_page_rows = PAGE;
        });
        let full = if scheme == SchemeType::Quanpin {
            "shabi".to_owned()
        } else {
            crate::shuangpin::utils::encode_pinyin("sha'bi", session.input.profile)
                .expect("sha'bi 在默认双拼方案里有码")
                .replace('\'', "")
        };
        type_text(&mut session, &full);
        let page = first_page(&session);
        assert_ne!(page[0], "傻逼", "{scheme:?} {full}: {page:?}");
        assert!(
            page.iter().any(|word| word == "傻逼"),
            "{scheme:?} {full}: {page:?}"
        );
        session.command(Command::Cancel);
    }
    let mut session = fixture.session_with(|options| {
        options.helpcode = false;
        options.first_page_rows = PAGE;
    });
    for partial in ["sb", "shab"] {
        type_text(&mut session, partial);
        let page = first_page(&session);
        assert!(
            !page.iter().any(|word| word == "傻逼"),
            "{partial}: {page:?}"
        );
        assert!(
            words(&session).iter().any(|word| word == "傻逼"),
            "{partial}: {:?}",
            words(&session)
        );
        session.command(Command::Cancel);
    }
    type_text(&mut session, "niubi");
    assert_eq!(words(&session)[0], "牛逼");
}

/// 超级简拼（`J` 模式）查出来的也是共享词库的行：粗俗词不进首屏，往后翻照样有。
#[test]
fn super_jianpin_keeps_vulgar_words_off_the_first_page() {
    let fixture = fixture();
    let mut session = fixture.session_with(|options| {
        options.helpcode = false;
        options.first_page_rows = PAGE;
    });
    session.character(b'J', true);
    type_text(&mut session, "sb");
    assert_eq!(session.snapshot().local_mode, LocalInputMode::SuperJianpin);
    let page = first_page(&session);
    assert!(!page.iter().any(|word| word == "傻逼"), "{page:?}");
    assert!(
        words(&session).iter().any(|word| word == "傻逼"),
        "{:?}",
        words(&session)
    );
}

/// 26 键上两到四个字母的拼音，只有常见的英文坐首选之后的位置（na），冷僻的（bole）接在列表末尾；拼不成音节、整段就是一个英文粗俗词时，它不领先。
#[test]
fn short_typed_pinyin_seats_only_common_english_words() {
    let fixture = fixture();
    let mut session = mixed_session(&fixture);
    type_text(&mut session, "na");
    assert_eq!(&words(&session)[..2], ["那", "na"]);
    session.command(Command::Cancel);
    type_text(&mut session, "bole");
    let listed = words(&session);
    assert_eq!(listed[0], "拨了", "{listed:?}");
    assert_ne!(listed[1], "bole", "{listed:?}");
    assert_eq!(
        listed.last().map(String::as_str),
        Some("bole"),
        "{listed:?}"
    );
    session.command(Command::Cancel);
    // 拼写打全的英文粗俗词与打全拼写的中文粗俗词一样，只让出首选、留在首屏。
    type_text(&mut session, "shitty");
    let listed = words(&session);
    assert_ne!(listed[0], "shitty", "{listed:?}");
    assert_eq!(listed[1], "shitty", "{listed:?}");
    session.command(Command::Cancel);
    // 没打全时它不坐优先位置。
    type_text(&mut session, "shitt");
    let page = first_page(&session);
    assert!(!page.iter().any(|word| word == "shitty"), "{page:?}");
}

/// 用户自己加进个人词库的英文词（默认权重 10 万，低于常见度门槛）打全拼写时照样坐九键第 2 位和 26 键首选之后的位置；词库里同样冷僻的词不坐。
#[test]
fn the_users_own_english_words_keep_the_priority_slot() {
    let fixture = fixture();
    for word in ["bold", "bole"] {
        let entry = crate::types::PersonalDictionaryEntry {
            kind: crate::types::PersonalDictionaryKind::English,
            key: word.to_owned(),
            value: word.to_owned(),
            weight: crate::types::PersonalDictionaryEntry::DEFAULT_WEIGHT,
        };
        crate::user_dictionary::personal::edit_personal_dictionary_with(
            &fixture.paths(),
            true,
            None,
            Some(&entry),
            word,
        )
        .expect("add the English word");
    }
    // bole 原有的 16 万词库权重被用户的 10 万盖掉，两个词都低于门槛，只因为是用户的词才坐得上。
    let mut session = nine_key_session(&fixture);
    type_text(&mut session, "2653");
    let listed = words(&session);
    assert!(matches!(listed[1].as_str(), "bold" | "bole"), "{listed:?}");
    let mut session = mixed_session(&fixture);
    type_text(&mut session, "bole");
    let listed = words(&session);
    assert_eq!(listed[0], "拨了", "{listed:?}");
    assert_eq!(listed[1], "bole", "{listed:?}");
}

/// 九键没有拼音读法、列表只有英文时（77 这类拼不出音节的码），英文粗俗词照样不进首屏。
#[test]
fn an_english_only_nine_key_list_keeps_vulgar_words_off_the_first_page() {
    let fixture = Fixture::new(&dictionary(&[("ni", "你", 1)])).with_english(
        "INSERT INTO english_words VALUES('fuck','fuck',900000000),('duck','duck',50000000),('dual','dual',40000000),('fvck','fvck',2),('dvcj','dvcj',1);",
    );
    let mut session = nine_key_session(&fixture);
    type_text(&mut session, "3825");
    let listed = words(&session);
    assert_eq!(listed[..PAGE], ["duck", "dual", "fvck"], "{listed:?}");
    assert!(listed.iter().any(|word| word == "fuck"), "{listed:?}");
}
