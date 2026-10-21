use super::*;
use msime_client_core::preferences::{FuzzyPinyinPreferences, FuzzyPinyinRule, ShuangpinProfile};

/// 词库用合成字面和权重，精确单字多于一页，不能靠两条候选掩盖模糊行落在尾部。
fn fixture(root: &std::path::Path, literal: &str, partner: &str) {
    for name in ["resources", "dictionaries"] {
        let directory = root.join(name);
        std::fs::create_dir_all(&directory).unwrap();
        let db = rusqlite::Connection::open(directory.join("msime-pinyin.db")).unwrap();
        let table = format!("tbl_1_{}", &literal[..1]);
        db.execute_batch(&format!(
            "CREATE TABLE {table}(key TEXT,jp TEXT,value TEXT,weight INTEGER);"
        ))
        .unwrap();
        for index in 0..24 {
            let word = char::from_u32(0x4e20 + index).unwrap().to_string();
            db.execute(
                &format!("INSERT INTO {table} VALUES(?1,?2,?3,?4)"),
                rusqlite::params![literal, &literal[..1], word, 100 - index],
            )
            .unwrap();
        }
        db.execute(
            &format!("INSERT INTO {table} VALUES(?1,?2,'乙',500)"),
            rusqlite::params![partner, &partner[..1]],
        )
        .unwrap();
    }
}

fn contains_target(handle: u64) -> bool {
    let all = read(msime_client_all_candidates(handle));
    assert_eq!(all["ok"], true, "{all}");
    all["value"]["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .any(|row| row["text"] == "乙")
}

#[test]
fn fuzzy_single_characters_reach_the_first_page_and_follow_live_rules() {
    for (profile, retroflex) in [
        (ShuangpinProfile::Xiaohe, ["uu", "ie", "va"]),
        (ShuangpinProfile::Ziranma, ["uu", "ie", "va"]),
        (ShuangpinProfile::Shoudao, ["eu", "ie", "va"]),
        (ShuangpinProfile::Microsoft, ["uu", "ie", "va"]),
        (ShuangpinProfile::SmartAbc, ["vu", "ee", "aa"]),
    ] {
        for (index, (plain, partner, raw, rule)) in [
            ("su", "shu", "su", FuzzyPinyinRule::SSh),
            ("ce", "che", "ce", FuzzyPinyinRule::CCh),
            ("za", "zha", "za", FuzzyPinyinRule::ZZh),
        ]
        .into_iter()
        .enumerate()
        {
            for reverse in [false, true] {
                let directory = tempfile::tempdir().unwrap();
                let (literal, fuzzy, input) = if reverse {
                    (partner, plain, retroflex[index])
                } else {
                    (plain, partner, raw)
                };
                fixture(directory.path(), literal, fuzzy);
                let mut preferences = Preferences {
                    scheme: InputScheme::Shuangpin,
                    shuangpin_profile: profile,
                    candidate_page_size: 3,
                    learning: false,
                    fuzzy_pinyin: FuzzyPinyinPreferences {
                        enabled: false,
                        rules: [rule].into_iter().collect(),
                        seeded: true,
                    },
                    ..chinese_preferences()
                };
                // 保留产品默认的辅助码开关，覆盖真实宿主的普通两键输入。
                let handle = test_host_preferences(directory.path(), preferences.clone());
                assert_eq!(read(msime_client_focus(handle, true))["ok"], true);
                type_letters(handle, input.as_bytes());
                assert!(!contains_target(handle));
                preferences.fuzzy_pinyin.enabled = true;
                let deferred = update(handle, 1, &preferences);
                assert_eq!(deferred["value"]["deferred"], true, "{deferred}");
                assert!(!contains_target(handle));
                assert_eq!(read(msime_client_command(handle, 3))["ok"], true);
                for _ in 0..2 {
                    let view = type_letters(handle, input.as_bytes());
                    assert_eq!(
                        view["candidates"][0]["text"], "乙",
                        "{profile:?}/{input}: {view}"
                    );
                    assert_eq!(view["candidates"][0]["code"], input);
                    assert_eq!(view["candidates"][0]["corrected"], false);
                    assert_eq!(view["candidates"].as_array().unwrap().len(), 3);
                    let generation = view["generation"].as_u64().unwrap();
                    let selected = read(msime_client_select(handle, generation, 0));
                    assert_eq!(selected["ok"], true, "{selected}");
                    assert_eq!(selected["value"]["commit"], "乙");
                    assert_eq!(selected["value"]["view"]["editing_text"], "");
                    assert!(selected["value"]["diagnostic"].is_null());
                }
                preferences.fuzzy_pinyin.enabled = false;
                assert_eq!(update(handle, 2, &preferences)["value"]["deferred"], false);
                type_letters(handle, input.as_bytes());
                assert!(!contains_target(handle), "关闭后不能命中旧模糊缓存");
                read(msime_client_command(handle, 3));
                preferences.fuzzy_pinyin.enabled = true;
                preferences.fuzzy_pinyin.rules = [FuzzyPinyinRule::NL].into_iter().collect();
                assert_eq!(update(handle, 3, &preferences)["ok"], true);
                type_letters(handle, input.as_bytes());
                assert!(!contains_target(handle), "未选择的规则不能生效");
                assert_eq!(read(msime_client_destroy(handle))["ok"], true);
            }
        }
    }
}
