//! 不含全拼、双拼、五笔的方案集合只带自己的数据，经公开 API 准备代次、建会话、编辑个人词典和重置（docs/data-formats.md 的「每个方案的最小数据集」）。
//!
//! 韩语、越南语、藏文自己不读任何词库，日文只读 `msime-japanese.dat`（缺了只出假名），粤语、注音、笔画只读各自的语言库（会话从 `SessionOptions` 的路径直接读，不经过代次）。这些集合不要求 `msime-pinyin.db` 和 `msime-english.db`；带了 `msime-english.db` 时它照样进代次，专用英文有补全、回车学词。

use std::path::{Path, PathBuf};

use msime_engine::assets;
use msime_engine::{
    edit_personal_dictionary, personal_dictionary_entries, prepare_runtime_paths,
    prepare_runtime_paths_for, reset_learned_data_for, Command, PersonalDictionaryEntry,
    PersonalDictionaryKind, RuntimePaths, SchemeSet, SchemeType, Session, SessionOptions,
};
use rusqlite::Connection;

struct Roots {
    _root: tempfile::TempDir,
    resources: PathBuf,
    user_data: PathBuf,
    cache: PathBuf,
}

/// 三个互不重叠的根目录，资源目录是空的。
fn roots() -> Roots {
    let root = tempfile::tempdir().unwrap();
    let resources = root.path().join("resources");
    std::fs::create_dir_all(&resources).unwrap();
    Roots {
        resources,
        user_data: root.path().join("user"),
        cache: root.path().join("cache"),
        _root: root,
    }
}

impl Roots {
    fn prepare(&self, content_id: &str, schemes: SchemeSet) -> msime_engine::Result<RuntimePaths> {
        prepare_runtime_paths_for(
            &self.resources,
            &self.user_data,
            &self.cache,
            content_id,
            schemes,
        )
    }

    fn ship_english(&self) {
        let english = self.resources.join(assets::ENGLISH_DICTIONARY);
        msime_engine::ensure_english_schema(&english).unwrap();
        Connection::open(&english)
            .unwrap()
            .execute_batch("INSERT INTO english_words VALUES('hello','Hello',100);")
            .unwrap();
    }
}

fn session(paths: &RuntimePaths, scheme: SchemeType, schemes: SchemeSet) -> Session {
    let mut options = SessionOptions::new(paths.clone());
    options.scheme = scheme;
    options.enabled_schemes = schemes;
    Session::new(options).unwrap()
}

fn type_keys(session: &mut Session, keys: &str) -> String {
    let mut committed = String::new();
    for byte in keys.bytes() {
        let result = session.character(byte, false);
        assert!(result.handled, "{:?} of {keys:?}", byte as char);
        committed.push_str(result.commit.as_deref().unwrap_or_default());
    }
    committed
}

fn english_words(database: &Path, word: &str) -> i64 {
    Connection::open(database)
        .unwrap()
        .query_row(
            "SELECT count(*) FROM english_words WHERE word=?1",
            [word],
            |row| row.get(0),
        )
        .unwrap()
}

fn english_entry(word: &str) -> PersonalDictionaryEntry {
    PersonalDictionaryEntry {
        kind: PersonalDictionaryKind::English,
        key: word.to_ascii_lowercase(),
        value: word.to_owned(),
        weight: 10,
    }
}

/// 只有韩语的会话在空的资源目录上准备、建会话、打字；专用英文里回车照常上屏、不学词，也不在代次里建英文库。个人词典无处可存，重置只清空日志。同一个空目录给中文方案准备仍然失败。
#[test]
fn korean_only_needs_no_dictionary_files() {
    let roots = roots();
    let korean = SchemeSet::of(&[SchemeType::Korean]);
    let paths = roots.prepare("v1", korean).unwrap();
    assert!(paths.dictionaries.join(assets::GENERATION_READY).is_file());
    assert!(!paths.dictionaries.join(assets::MAIN_DICTIONARY).exists());
    assert!(!paths.dictionaries.join(assets::ENGLISH_DICTIONARY).exists());

    let mut session = session(&paths, SchemeType::Korean, korean);
    assert_eq!(type_keys(&mut session, "dkssud"), "안");
    assert_eq!(
        session.command(Command::CommitRaw).commit.as_deref(),
        Some("녕")
    );

    session.set_dedicated_english(true);
    type_keys(&mut session, "hello");
    assert!(session
        .snapshot()
        .candidates
        .iter()
        .all(|row| row.word != "Hello"));
    let result = session.command(Command::CommitRaw);
    assert_eq!(result.commit.as_deref(), Some("hello"));
    assert_eq!(result.diagnostic, None);
    assert!(!paths.dictionaries.join(assets::ENGLISH_DICTIONARY).exists());
    drop(session);

    // 就绪的代次再准备一次照样不要求任何词库。
    assert_eq!(roots.prepare("v1", korean).unwrap(), paths);

    let error =
        edit_personal_dictionary(&paths, None, Some(&english_entry("Zyzzyva")), "").unwrap_err();
    assert!(
        error.to_string().contains("no English dictionary"),
        "{error}"
    );
    assert!(!paths.dictionaries.join(assets::ENGLISH_DICTIONARY).exists());
    assert!(personal_dictionary_entries(&paths, 0, 10, false)
        .unwrap()
        .entries
        .is_empty());

    reset_learned_data_for(&paths, korean).unwrap();
    assert!(paths.user(assets::USER_JOURNAL).is_file());
    assert!(!paths.dictionaries.join(assets::ENGLISH_DICTIONARY).exists());

    assert!(prepare_runtime_paths(&roots.resources, &roots.user_data, &roots.cache, "v2").is_err());
    assert!(roots
        .prepare("v3", SchemeSet::of(&[SchemeType::Quanpin]))
        .is_err());
}

/// 只有日文、没带 `msime-japanese.dat` 时只出假名，越南语、藏文同样在空目录上建得起来。
#[test]
fn japanese_vietnamese_and_tibetan_run_on_their_own_files() {
    let roots = roots();
    let schemes = SchemeSet::of(&[
        SchemeType::JapaneseRomaji,
        SchemeType::Vietnamese,
        SchemeType::Tibetan,
    ]);
    let paths = roots.prepare("v1", schemes).unwrap();

    let mut japanese = session(&paths, SchemeType::JapaneseRomaji, schemes);
    type_keys(&mut japanese, "ka");
    assert!(japanese
        .snapshot()
        .candidates
        .iter()
        .any(|row| row.word == "か"));
    drop(japanese);

    let mut vietnamese = session(&paths, SchemeType::Vietnamese, schemes);
    type_keys(&mut vietnamese, "vieetj");
    assert_eq!(vietnamese.snapshot().preedit, "việt");
    drop(vietnamese);

    let mut tibetan = session(&paths, SchemeType::Tibetan, schemes);
    type_keys(&mut tibetan, "ka");
    assert!(!tibetan.snapshot().preedit.is_empty());
}

/// 不要求英文库的集合带了 `msime-english.db` 时它照样进代次：专用英文有补全、回车学词，个人词典能存英文词，重置换回随包的那份。
#[test]
fn a_shipped_english_dictionary_still_serves_dedicated_english() {
    let roots = roots();
    roots.ship_english();
    let korean = SchemeSet::of(&[SchemeType::Korean]);
    let paths = roots.prepare("v1", korean).unwrap();
    let english = paths.dictionaries.join(assets::ENGLISH_DICTIONARY);
    assert!(english.is_file());

    let mut session = session(&paths, SchemeType::Korean, korean);
    session.set_dedicated_english(true);
    type_keys(&mut session, "hel");
    assert!(session
        .snapshot()
        .candidates
        .iter()
        .any(|row| row.word == "Hello"));
    session.command(Command::Cancel);
    type_keys(&mut session, "Zyzzyva");
    assert_eq!(
        session.command(Command::CommitRaw).commit.as_deref(),
        Some("Zyzzyva")
    );
    assert_eq!(english_words(&english, "zyzzyva"), 1);
    drop(session);

    edit_personal_dictionary(&paths, None, Some(&english_entry("Ferris")), "").unwrap();
    assert_eq!(english_words(&english, "ferris"), 1);

    reset_learned_data_for(&paths, korean).unwrap();
    assert_eq!(english_words(&english, "zyzzyva"), 0);
    assert_eq!(english_words(&english, "ferris"), 0);
    assert_eq!(english_words(&english, "hello"), 1);
}
