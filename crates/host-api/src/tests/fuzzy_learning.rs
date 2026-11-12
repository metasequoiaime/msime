use super::*;
use msime_client_core::preferences::{FuzzyPinyinPreferences, FuzzyPinyinRule};

/// #11321 的步骤，词库换成合成权重：`zi'qian` 有三个字面词，`zhi'qian` 的两个词靠 z=zh 才出现。随包资源、工作词库和用户日志分开放，和宿主一样。四音节的空表留给连选造词：同一个词连着选几次会被记成一个四字词，表不在的话那一步会报写入失败。
fn fixture(root: &Path) -> String {
    let paths = |name| {
        let path = root.join(name);
        std::fs::create_dir_all(&path).unwrap();
        path
    };
    let resources = paths("resources");
    let dictionaries = paths("user/dictionaries/generation");
    let sql = "CREATE TABLE tbl_2_z(key TEXT,jp TEXT,value TEXT,weight INTEGER);
        INSERT INTO tbl_2_z VALUES('zi''qian','zq','自谦',5000),('zi''qian','zq','自遣',3000),
        ('zi''qian','zq','自谴',100),('zhi''qian','zq','之前',4000000),('zhi''qian','zq','值钱',150);
        CREATE TABLE tbl_4_z(key TEXT,jp TEXT,value TEXT,weight INTEGER);";
    for directory in [&resources, &dictionaries] {
        rusqlite::Connection::open(directory.join("msime-pinyin.db"))
            .unwrap()
            .execute_batch(sql)
            .unwrap();
    }
    let preferences = Preferences {
        fuzzy_pinyin: FuzzyPinyinPreferences {
            enabled: true,
            rules: [FuzzyPinyinRule::ZZh].into_iter().collect(),
            seeded: true,
        },
        cloud_candidates: false,
        ..chinese_preferences()
    };
    assert_eq!(preferences.scheme, InputScheme::Quanpin);
    assert!(preferences.learning);
    assert_eq!(preferences.frequency.mode.as_str(), "promote");
    assert_eq!(preferences.frequency.trigger_count, 1);
    json!({
        "api_version": 1, "resources": resources, "dictionaries": dictionaries,
        "user_data": paths("user"), "cache": paths("cache"), "preferences": preferences
    })
    .to_string()
}

fn open(options: &str) -> u64 {
    let result = read(unsafe { msime_client_create(options.as_ptr(), options.len()) });
    assert_eq!(result["ok"], true, "{result}");
    let handle = result["value"]["session"].as_u64().unwrap();
    assert_eq!(read(msime_client_focus(handle, true))["ok"], true);
    handle
}

/// 打一遍 `ziqian`，返回全部候选的文字、各自的选词下标和这一代的编号。
fn candidates(handle: u64) -> (Vec<String>, Vec<usize>, u64) {
    assert_eq!(read(msime_client_command(handle, 3))["ok"], true);
    type_letters(handle, b"ziqian");
    let all = read(msime_client_all_candidates(handle));
    assert_eq!(all["ok"], true, "{all}");
    let rows = all["value"]["candidates"].as_array().unwrap();
    let words = rows
        .iter()
        .map(|row| row["text"].as_str().unwrap().to_owned())
        .collect();
    let indices = rows
        .iter()
        .map(|row| row["id"]["index"].as_u64().unwrap() as usize)
        .collect();
    (words, indices, all["value"]["generation"].as_u64().unwrap())
}

fn seat(words: &[String], word: &str) -> usize {
    words
        .iter()
        .position(|candidate| candidate == word)
        .unwrap_or_else(|| panic!("合成候选缺失：{word}: {words:?}"))
}

#[test]
fn a_picked_fuzzy_candidate_moves_ahead_of_the_literal_ones_and_stays_there() {
    let directory = tempfile::tempdir().unwrap();
    let options = fixture(directory.path());
    let handle = open(&options);
    let (words, _, _) = candidates(handle);
    // 高频的模糊词不用学就在冷僻的字面词前面；低频的模糊词排在它们后面。
    assert!(seat(&words, "之前") < seat(&words, "自谦"), "{words:?}");
    assert!(seat(&words, "值钱") > seat(&words, "自谴"), "{words:?}");
    for round in 0..5 {
        let (words, indices, generation) = candidates(handle);
        let index = indices[seat(&words, "值钱")];
        let selected = read(msime_client_select_any_candidate(handle, generation, index));
        assert_eq!(selected["ok"], true, "{selected}");
        assert_eq!(
            selected["value"]["commit"], "值钱",
            "第 {round} 轮: {selected}"
        );
        assert!(selected["value"]["diagnostic"].is_null(), "{selected}");
    }
    let (words, _, _) = candidates(handle);
    assert!(seat(&words, "值钱") < seat(&words, "自谦"), "{words:?}");
    assert_eq!(read(msime_client_command(handle, 3))["ok"], true);
    assert_eq!(read(msime_client_destroy(handle))["ok"], true);

    // 换一个新会话：学到的次序是从词库里读出来的，不是会话里的上下文重排。
    let handle = open(&options);
    let (words, _, _) = candidates(handle);
    assert!(seat(&words, "值钱") < seat(&words, "自谦"), "{words:?}");
    assert!(seat(&words, "值钱") < seat(&words, "自遣"), "{words:?}");
    assert_eq!(read(msime_client_destroy(handle))["ok"], true);
}
