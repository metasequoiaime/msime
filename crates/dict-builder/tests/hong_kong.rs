//! 用实际构建的双方案词库输入、选词，测试数据仅为官方公开地名。

use std::path::PathBuf;

use msime_engine::{SchemeSet, SchemeType, Session, SessionOptions};

#[test]
#[ignore = "先构建词库，并设置 MSIME_HONG_KONG_TEST_DATA 指向含 pinyin/ 和 languages/ 的目录"]
fn built_place_names_can_be_typed_and_committed() -> Result<(), Box<dyn std::error::Error>> {
    let data =
        PathBuf::from(std::env::var_os("MSIME_HONG_KONG_TEST_DATA").ok_or("missing test data")?);
    let temp = tempfile::tempdir()?;
    let resources = temp.path().join("resources");
    std::fs::create_dir(&resources)?;
    for (source, name) in [
        ("pinyin/msime-pinyin.db", "msime-pinyin.db"),
        ("languages/msime-cantonese.db", "msime-cantonese.db"),
    ] {
        std::fs::copy(data.join(source), resources.join(name))?;
    }
    msime_engine::ensure_english_schema(&resources.join("msime-english.db"))?;
    let schemes = SchemeSet::EMPTY
        .with(SchemeType::Quanpin)
        .with(SchemeType::Cantonese);
    let paths = msime_engine::prepare_runtime_paths_for(
        &resources,
        &temp.path().join("user"),
        &temp.path().join("cache"),
        "hong-kong-test",
        schemes,
    )?;
    let mut options = SessionOptions::new(paths);
    options.enabled_schemes = schemes;
    options.cantonese_dictionary = resources.join("msime-cantonese.db");
    let samples = [
        ("砵典乍街", "砵典乍街", "butdinzaagaai", "bodianzhajie"),
        ("彌敦道", "弥敦道", "neideondou", "midundao"),
        ("窩打老道", "窝打老道", "wodaaloudou", "wodalaodao"),
        ("上海街", "上海街", "soenghoigaai", "shanghaijie"),
        ("保安道", "保安道", "bouondou", "baoandao"),
        ("蘇杭街", "苏杭街", "souhonggaai", "suhangjie"),
        ("電氣道", "电气道", "dinheidou", "dianqidao"),
        ("渣甸街", "渣甸街", "zaadingaai", "zhadianjie"),
        ("楊屋道", "杨屋道", "joengukdou", "yangwudao"),
        ("廟街", "庙街", "miugaai", "miaojie"),
        ("忠孝街", "忠孝街", "zunghaaugaai", "zhongxiaojie"),
        ("大強街", "大强街", "daaikoenggaai", "daqiangjie"),
        (
            "列拿士地臺",
            "列拿士地台",
            "litnaasideitoi",
            "lienashiditai",
        ),
        ("尖沙咀", "尖沙咀", "zimsaazeoi", "jianshazui"),
        ("深水埗", "深水埗", "samseoibou", "shenshuibu"),
        ("鰂魚涌", "鲗鱼涌", "zakjyucung", "zeyuchong"),
        ("大埔", "大埔", "daaibou", "dabu"),
        ("荃灣", "荃湾", "cyunwaan", "quanwan"),
        ("長洲", "长洲", "coengzau", "changzhou"),
        ("䃟頭", "䃟头", "saantau", "santou"),
    ];
    for (traditional, simplified, yue, cmn) in samples {
        for (scheme, code, expected) in [
            (SchemeType::Cantonese, yue, traditional),
            (SchemeType::Quanpin, cmn, simplified),
        ] {
            options.scheme = scheme;
            let mut session = Session::new(options.clone())?;
            for byte in code.bytes() {
                let result = session.character(byte, false);
                assert!(result.commit.is_none(), "unexpected partial commit: {code}");
            }
            let snapshot = session.snapshot();
            let index = snapshot
                .candidates
                .iter()
                .position(|row| row.word == expected)
                .unwrap_or_else(|| {
                    panic!(
                        "{scheme:?} {code}: missing {expected}; preedit={:?}; top={:?}",
                        snapshot.preedit,
                        snapshot
                            .candidates
                            .iter()
                            .take(15)
                            .map(|r| (&r.word, &r.pinyin))
                            .collect::<Vec<_>>()
                    )
                });
            assert_eq!(
                session.select(index).commit.as_deref(),
                Some(expected),
                "{code}"
            );
        }
    }
    Ok(())
}
