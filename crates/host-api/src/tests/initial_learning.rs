use super::*;
use msime_client_core::preferences::ShuangpinProfile;

/// 分开随包资源、工作词库和用户日志，保留 macOS 的默认候选能力。
fn fixture(root: &Path) -> String {
    let paths = |name| {
        let path = root.join(name);
        std::fs::create_dir_all(&path).unwrap();
        path
    };
    let resources = paths("resources");
    let dictionaries = paths("user/dictionaries/generation");
    // 权重与读音来自固定公开词库；字面替换为合成值，不使用用户词库或输入记录。
    let sql = "CREATE TABLE tbl_1_l(key TEXT,jp TEXT,value TEXT,weight INTEGER);
        INSERT INTO tbl_1_l VALUES('lai','l','甲',14837007),('li','l','丁',7329328),
        ('lao','l','戊',3762961),('la','l','己',3655999),('le','l','乙',3481243),
        ('liang','l','庚',3302466),('li','l','辛',3298612),('liao','l','乙',999);
        CREATE TABLE tbl_2_l(key TEXT,jp TEXT,value TEXT,weight INTEGER);
        CREATE TABLE tbl_1_w(key TEXT,jp TEXT,value TEXT,weight INTEGER);
        INSERT INTO tbl_1_w VALUES('wan','w','丙',100);
        CREATE TABLE quick_parases(key TEXT,value TEXT,weight INTEGER);";
    for directory in [&resources, &dictionaries] {
        rusqlite::Connection::open(directory.join("msime-pinyin.db"))
            .unwrap()
            .execute_batch(sql)
            .unwrap();
    }
    let preferences = Preferences {
        scheme: InputScheme::Shuangpin,
        shuangpin_profile: ShuangpinProfile::Xiaohe,
        ..chinese_preferences()
    };
    assert!(preferences.learning);
    assert_eq!(preferences.frequency.mode.as_str(), "promote");
    assert_eq!(preferences.frequency.trigger_count, 1);
    json!({
        "api_version": 1, "resources": resources, "dictionaries": dictionaries,
        "user_data": paths("user"), "cache": paths("cache"),
        "preferences": preferences, "phrase_preedit": true, "quick_phrase_candidates": true
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

fn select(handle: u64, view: &Value, word: &str) {
    let row = view["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["text"] == word)
        .unwrap_or_else(|| panic!("合成候选缺失：{word}: {view}"));
    assert_eq!(row["source"], 0, "{row}");
    let result = read(msime_client_select(
        handle,
        row["id"]["generation"].as_u64().unwrap(),
        row["id"]["index"].as_u64().unwrap() as usize,
    ));
    assert_eq!(result["ok"], true, "{result}");
    assert_eq!(result["value"]["commit"], word, "{result}");
    assert!(result["value"]["diagnostic"].is_null(), "{result}");
}

#[test]
fn macos_initial_learning_persists_through_candidate_identity_and_separate_roots() {
    initial_learning_with_prior_context(0);
}

#[test]
fn learned_initial_leader_survives_conflicting_prior_context() {
    initial_learning_with_prior_context(10);
}

fn initial_learning_with_prior_context(prior_count: i64) {
    let directory = tempfile::tempdir().unwrap();
    let root = directory.path();
    let options = fixture(root);
    // 首次打开引擎前预置合成历史，避免已加载模型的缓存掩盖磁盘上的上下文。
    if prior_count > 0 {
        let journal = rusqlite::Connection::open(root.join("user/msime_user.db")).unwrap();
        journal.execute_batch(
            "CREATE TABLE personal_bigram(previous TEXT NOT NULL,word TEXT NOT NULL,count INTEGER NOT NULL CHECK(count>0),PRIMARY KEY(previous,word)) WITHOUT ROWID;
             CREATE TABLE personal_trigram(earlier TEXT NOT NULL,previous TEXT NOT NULL,word TEXT NOT NULL,count INTEGER NOT NULL CHECK(count>0),PRIMARY KEY(earlier,previous,word)) WITHOUT ROWID;",
        ).unwrap();
        journal
            .execute(
                "INSERT INTO personal_bigram(previous,word,count) VALUES('丙','甲',?1)",
                [prior_count],
            )
            .unwrap();
    }
    let handle = open(&options);
    let weight = |path: &Path| -> i64 {
        rusqlite::Connection::open(path)
            .unwrap()
            .query_row(
                "SELECT weight FROM tbl_1_l WHERE key='le' AND value='乙'",
                [],
                |row| row.get(0),
            )
            .unwrap()
    };
    let working_path = root.join("user/dictionaries/generation/msime-pinyin.db");
    let mut displayed_before_global = false;
    for round in 0..5 {
        let view = type_letters(handle, b"l");
        if round == 0 {
            assert_eq!(view["candidates"][0]["text"], "甲");
        }
        let before = weight(&working_path);
        if view["candidates"][0]["text"] == "乙" && before < 14837007 {
            displayed_before_global = true;
        }
        select(handle, &view, "乙");
        let after = weight(&working_path);
        if before < 14837007 {
            assert!(
                after > before,
                "显示首位也须继续全局调频：{before} → {after}"
            );
        }
        let journal = rusqlite::Connection::open(root.join("user/msime_user.db")).unwrap();
        let persisted: i64 = journal.query_row(
            "SELECT weight FROM user_dictionary_operations WHERE dictionary='pinyin' AND key='le' AND value='乙' AND operation='upsert'",
            [], |row| row.get(0),
        ).unwrap();
        assert_eq!(persisted, after, "每轮工作词库与日志须一致");
    }
    assert!(
        displayed_before_global,
        "必须覆盖显示已首位、全局词频仍落后的状态"
    );
    let view = type_letters(handle, b"wj");
    select(handle, &view, "丙");
    let view = type_letters(handle, b"l");
    assert_eq!(view["candidates"][0]["text"], "乙", "{view}");
    assert_eq!(read(msime_client_destroy(handle))["ok"], true);

    let working = weight(&working_path);
    assert!(working > 14837007, "工作词库必须记录全局提升：{working}");
    assert_eq!(weight(&root.join("resources/msime-pinyin.db")), 3481243);
    let journal = rusqlite::Connection::open(root.join("user/msime_user.db")).unwrap();
    let persisted: i64 = journal.query_row(
        "SELECT weight FROM user_dictionary_operations WHERE dictionary='pinyin' AND key='le' AND value='乙' AND operation='upsert'",
        [], |row| row.get(0),
    ).unwrap();
    assert_eq!(persisted, working);

    let reopened = open(&options);
    let view = type_letters(reopened, b"l");
    assert_eq!(view["candidates"][0]["text"], "乙", "{view}");
    select(reopened, &view, "乙");
    let view = type_letters(reopened, b"wj");
    select(reopened, &view, "丙");
    let view = type_letters(reopened, b"l");
    assert_eq!(
        view["candidates"][0]["text"], "乙",
        "重建后换上文仍须保持：{view}"
    );
    assert_eq!(read(msime_client_destroy(reopened))["ok"], true);
}
