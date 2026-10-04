//! Bundled dictionary rows through the management request: found by code, re-weighted or deleted, journaled so the change survives replay onto a freshly installed dictionary, and exported with the learned pinyin weights.

use msime_client_core::dictionary::access::DictionaryAccess;
use msime_host_api::dictionary_request_json;
use serde_json::{json, Value};
use std::path::PathBuf;

struct Fixture {
    _directory: tempfile::TempDir,
    root: PathBuf,
}

impl Fixture {
    fn new() -> Self {
        let directory = tempfile::tempdir().unwrap();
        let root = directory.path().canonicalize().unwrap();
        for name in ["resources", "user", "cache", "dictionaries"] {
            std::fs::create_dir_all(root.join(name)).unwrap();
        }
        let resources = root.join("resources");
        rusqlite::Connection::open(resources.join("msime-pinyin.db"))
            .unwrap()
            .execute_batch(
                "CREATE TABLE tbl_1_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
                 INSERT INTO tbl_1_n VALUES('ni','n','你',900000);\
                 CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
                 INSERT INTO tbl_2_n VALUES('ni''hao','nh','你好',500000);\
                 INSERT INTO tbl_2_n VALUES('ni''hao','nh','拟好',1000);\
                 INSERT INTO tbl_2_n VALUES('ni''men','nm','你们',400000);\
                 CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER,UNIQUE(key,value));\
                 INSERT INTO wubi86 VALUES('wqvb','你好',300);\
                 CREATE TABLE quick_parases(key TEXT,value TEXT,weight INTEGER,UNIQUE(key,value));\
                 INSERT INTO quick_parases VALUES('dh','电话',1);",
            )
            .unwrap();
        rusqlite::Connection::open(resources.join("msime-english.db"))
            .unwrap()
            .execute_batch(
                "CREATE TABLE english_words(word TEXT COLLATE BINARY NOT NULL,display TEXT NOT NULL,weight INTEGER NOT NULL DEFAULT 0,PRIMARY KEY(word,display)) WITHOUT ROWID;\
                 CREATE TABLE en_zh_glosses(english TEXT PRIMARY KEY,chinese_gloss TEXT);\
                 CREATE TABLE zh_en_glosses(chinese TEXT PRIMARY KEY,english_gloss TEXT);\
                 INSERT INTO english_words VALUES('hello','hello',611054034);",
            )
            .unwrap();
        let fixture = Self {
            _directory: directory,
            root,
        };
        fixture.install_packaged_dictionaries();
        fixture
    }

    fn path(&self, name: &str) -> PathBuf {
        self.root.join(name)
    }

    /// What an upgrade does: fresh copies of the packaged dictionaries, then the journal replayed onto them.
    fn install_packaged_dictionaries(&self) {
        for name in ["msime-pinyin.db", "msime-english.db"] {
            std::fs::copy(
                self.path("resources").join(name),
                self.path("dictionaries").join(name),
            )
            .unwrap();
        }
    }

    fn upgrade(&self) {
        self.install_packaged_dictionaries();
        let text = |path: PathBuf| path.to_str().unwrap().to_owned();
        let (applied, _skipped, failed, error) = msime_engine::host::replay_user_dictionary(
            &text(self.path("user").join("msime_user.db")),
            &text(self.path("dictionaries").join("msime-pinyin.db")),
            &text(self.path("dictionaries").join("msime-english.db")),
        );
        assert!(applied > 0 && failed == 0 && error.is_empty(), "{error}");
    }

    fn request(&self, action: Value) -> Result<Value, String> {
        let path = |name: &str| self.path(name).to_str().unwrap().to_owned();
        let request = json!({
            "options": {
                "api_version": 1,
                "resources": path("resources"),
                "user_data": path("user"),
                "cache": path("cache"),
                "dictionaries": path("dictionaries"),
                "preferences": msime_client_core::preferences::Preferences::default(),
            },
            "action": action,
        });
        dictionary_request_json(&serde_json::to_vec(&request).unwrap())
    }

    fn lookup(&self, kind: &str, query: &str) -> Vec<Value> {
        let page = self
            .request(json!({"operation": "list", "offset": 0, "limit": 100, "kind": kind, "query": query}))
            .unwrap();
        page["entries"].as_array().unwrap().clone()
    }

    fn edit(
        &self,
        previous: &Value,
        replacement: Value,
        request_id: &str,
    ) -> Result<Value, String> {
        self.request(json!({
            "operation": "edit",
            "previous": previous,
            "replacement": replacement,
            "request_id": request_id,
        }))
    }

    fn export(&self, kind: &str) -> String {
        let page = self
            .request(json!({"operation": "export", "kind": kind, "format": "windows", "offset": 0, "limit": 1000}))
            .unwrap();
        page["text"].as_str().unwrap().to_owned()
    }
}

fn row<'a>(rows: &'a [Value], value: &str) -> Option<&'a Value> {
    rows.iter().find(|row| row["value"] == value)
}

#[test]
fn a_bundled_word_is_found_reweighted_and_deleted_and_the_change_survives_an_upgrade() {
    let fixture = Fixture::new();

    // Found by the stored code, without separators, and by a partial code; bundled rows say so and come by weight.
    for query in ["ni'hao", "nihao", "NiHao", "ni hao"] {
        let rows = fixture.lookup("pinyin", query);
        let values: Vec<&str> = rows
            .iter()
            .map(|row| row["value"].as_str().unwrap())
            .collect();
        assert_eq!(values, ["你好", "拟好"], "{query}");
        assert!(rows.iter().all(|row| row["source"] == "bundled"), "{query}");
    }
    let partial = fixture.lookup("pinyin", "nih");
    assert!(row(&partial, "你好").is_some() && row(&partial, "你们").is_none());
    assert_eq!(fixture.lookup("pinyin", "hao"), Vec::<Value>::new());
    // The unfiltered page is still the user's own words only.
    let unfiltered = fixture
        .request(json!({"operation": "list", "offset": 0, "limit": 100}))
        .unwrap();
    assert_eq!(unfiltered["entries"], json!([]));

    // The lookup takes the shared lock, so it works while a keyboard session holds the dictionaries; an edit needs them to itself.
    let session =
        DictionaryAccess::try_session(&fixture.path("user"), &fixture.path("dictionaries"))
            .unwrap()
            .unwrap();
    let rows = fixture.lookup("pinyin", "nihao");
    let nihao = row(&rows, "你好").unwrap().clone();
    assert_eq!(nihao["weight"], 500000);
    assert_eq!(
        fixture.edit(&nihao, json!(null), "busy-bundled"),
        Err("dictionary maintenance busy".into())
    );
    drop(session);

    // A bundled word keeps its code and text: only the weight can change.
    let mut renamed = nihao.clone();
    renamed["value"] = json!("您好");
    assert_eq!(
        fixture.edit(&nihao, renamed, "rename-bundled"),
        Err("bundled dictionary entry is read-only".into())
    );
    let mut too_heavy = nihao.clone();
    too_heavy["weight"] = json!(100_000_001);
    assert_eq!(
        fixture.edit(&nihao, too_heavy, "heavy-bundled"),
        Err("invalid dictionary entry: weight is outside 1 to 100000000".into())
    );

    // Demote it; a retry with the same request ID is a no-op success.
    let mut demoted = nihao.clone();
    demoted["weight"] = json!(1);
    fixture
        .edit(&nihao, demoted.clone(), "demote-bundled")
        .unwrap();
    fixture
        .edit(&nihao, demoted.clone(), "demote-bundled")
        .unwrap();
    let rows = fixture.lookup("pinyin", "nihao");
    assert_eq!(rows[0]["value"], "拟好");
    assert_eq!(row(&rows, "你好").unwrap()["weight"], 1);
    assert_eq!(row(&rows, "你好").unwrap()["source"], "bundled");
    // The listed weight is now stale.
    assert_eq!(
        fixture.edit(&nihao, json!(null), "stale-bundled"),
        Err("dictionary edit rejected".into())
    );

    // A learned single character is journaled too, but the pinyin export leaves it out, as the reference's does.
    let single = fixture.lookup("pinyin", "ni");
    let ni = row(&single, "你").unwrap().clone();
    let mut ni_demoted = ni.clone();
    ni_demoted["weight"] = json!(2);
    fixture.edit(&ni, ni_demoted, "demote-single").unwrap();
    let exported = fixture.export("pinyin");
    assert!(
        exported.lines().any(|line| line == "ni'hao\t你好\t1"),
        "{exported}"
    );
    assert!(
        !exported.lines().any(|line| line.contains("\t你\t")),
        "{exported}"
    );
    // The other exports stay the user's own words: a re-weighted bundled wubi row is not one.
    let wubi = fixture.lookup("wubi", "wqv");
    let wubi = row(&wubi, "你好").unwrap().clone();
    assert_eq!(wubi["source"], "bundled");
    let mut wubi_demoted = wubi.clone();
    wubi_demoted["weight"] = json!(3);
    fixture.edit(&wubi, wubi_demoted, "demote-wubi").unwrap();
    assert_eq!(fixture.export("wubi"), "");

    // The demotion is in the journal, so an upgrade that installs the packaged dictionary again keeps it.
    fixture.upgrade();
    let rows = fixture.lookup("pinyin", "nihao");
    assert_eq!(row(&rows, "你好").unwrap()["weight"], 1);
    assert_eq!(
        row(&fixture.lookup("wubi", "wqvb"), "你好").unwrap()["weight"],
        3
    );

    // Delete a bundled word: it is gone, and stays gone after the next upgrade.
    let nihao = row(&rows, "你好").unwrap().clone();
    fixture.edit(&nihao, json!(null), "delete-bundled").unwrap();
    assert!(row(&fixture.lookup("pinyin", "nihao"), "你好").is_none());
    fixture.upgrade();
    let rows = fixture.lookup("pinyin", "nihao");
    assert!(row(&rows, "你好").is_none());
    assert!(row(&rows, "拟好").is_some());
    assert!(!fixture.export("pinyin").contains("你好"));
}

#[test]
fn user_words_lead_the_lookup_and_stay_user_words() {
    let fixture = Fixture::new();
    let user = json!({"kind": "pinyin", "key": "nihao", "value": "妳好", "weight": 10});
    fixture.edit(&json!(null), user, "add-user").unwrap();
    let rows = fixture.lookup("pinyin", "nihao");
    assert_eq!(rows[0]["value"], "妳好");
    assert_eq!(rows[0]["source"], "user");
    assert_eq!(rows[0]["key"], "ni'hao");
    assert!(rows[1..].iter().all(|row| row["source"] == "bundled"));
    // A user word marked as bundled is not edited as one.
    let mut disguised = rows[0].clone();
    disguised["source"] = json!("bundled");
    assert_eq!(
        fixture.edit(&disguised, json!(null), "disguised-user"),
        Err("dictionary edit rejected".into())
    );
    // A new entry cannot claim to be bundled.
    let claimed = json!({"kind": "pinyin", "key": "ni'hao", "value": "伱好", "weight": 10, "source": "bundled"});
    assert_eq!(
        fixture.edit(&json!(null), claimed, "claimed-bundled"),
        Err("bundled dictionary entry is read-only".into())
    );
    // The user word is still edited the ordinary way, including a new text.
    let mut renamed = rows[0].clone();
    renamed["value"] = json!("伱好");
    fixture.edit(&rows[0], renamed, "rename-user").unwrap();
    let rows = fixture.lookup("pinyin", "nihao");
    assert_eq!(rows[0]["value"], "伱好");
    assert_eq!(rows[0]["source"], "user");
}

#[test]
fn quick_phrases_list_whole_and_english_words_are_found_by_prefix() {
    let fixture = Fixture::new();
    let phrases = fixture.lookup("quick_phrase", "");
    assert_eq!(
        phrases,
        [
            json!({"kind": "quick_phrase", "key": "dh", "value": "电话", "weight": 1, "source": "bundled"})
        ]
    );
    assert_eq!(fixture.lookup("quick_phrase", "D").len(), 1);
    assert!(fixture.lookup("quick_phrase", "x").is_empty());
    // A shipped English weight can lie above the range a user entry may have; the row is still listed and can be deleted.
    let words = fixture.lookup("english", "hel");
    assert_eq!(
        words,
        [
            json!({"kind": "english", "key": "hello", "value": "hello", "weight": 611054034, "source": "bundled"})
        ]
    );
    assert!(fixture.lookup("english", "help").is_empty());
    fixture
        .edit(&words[0], json!(null), "delete-english")
        .unwrap();
    assert!(fixture.lookup("english", "hel").is_empty());
    fixture.upgrade();
    assert!(fixture.lookup("english", "hel").is_empty());
    // A code no dictionary of the kind can hold matches nothing rather than failing.
    assert!(fixture.lookup("wubi", "你").is_empty());
}

#[test]
fn a_user_only_search_finds_the_user_word_by_code_and_leaves_the_bundled_ones_out() {
    let fixture = Fixture::new();
    fixture
        .edit(
            &json!(null),
            json!({"kind": "pinyin", "key": "ni'hao", "value": "你号", "weight": 100000}),
            "user-word",
        )
        .unwrap();
    fixture
        .edit(
            &json!(null),
            json!({"kind": "quick_phrase", "key": "nh", "value": "你好短语", "weight": 100000}),
            "user-phrase",
        )
        .unwrap();
    let search = |kind: Value, query: &str| {
        let page = fixture
            .request(json!({"operation": "list", "offset": 0, "limit": 100, "kind": kind, "query": query, "user_only": true}))
            .unwrap();
        page["entries"]
            .as_array()
            .unwrap()
            .iter()
            .map(|row| row["value"].as_str().unwrap().to_owned())
            .collect::<Vec<_>>()
    };

    // Without user_only the same code also reaches the bundled 你好 and 拟好.
    assert_eq!(search(json!("pinyin"), "nihao"), ["你号"]);
    assert_eq!(search(json!(null), "n"), ["你号", "你好短语"]);
    assert_eq!(search(json!("quick_phrase"), ""), ["你好短语"]);
    assert_eq!(search(json!("wubi"), ""), Vec::<String>::new());
    assert_eq!(fixture.lookup("pinyin", "nihao").len(), 3);
}
