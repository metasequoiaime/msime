//! 词库自带的粗俗词表（`msime-pinyin.db` 的 `sentence_excluded`，见 `lattice::offensive`）在会话里的效果：新引擎配新词库用词库的表，新引擎配旧词库退回内嵌表，旧引擎配新词库只是多了一张不读的表。

use super::*;

/// 联想和整句共用的词：上屏 巨 联想 乳 或 人；全拼 womenyoujuru 能拼出 我们有巨乳。权重取自 dict-v2.0.39。
const ROWS: &str = "CREATE TABLE tbl_1_j(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_1_j VALUES('ju','j','巨',1000);
INSERT INTO tbl_1_j VALUES('ju','j','据',323831);
CREATE TABLE tbl_2_j(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_2_j VALUES('ju''ru','jr','巨乳',3174);
INSERT INTO tbl_2_j VALUES('ju''ren','jr','巨人',100);
CREATE TABLE tbl_1_w(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_1_w VALUES('wo','w','我',30000000);
CREATE TABLE tbl_1_m(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_1_m VALUES('men','m','们',3000000);
CREATE TABLE tbl_2_w(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_2_w VALUES('wo''men','wm','我们',13696325);
CREATE TABLE tbl_3_w(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_3_w VALUES('wo''men''you','wmy','我们有',83600);
CREATE TABLE tbl_1_y(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_1_y VALUES('you','y','有',15615006);
CREATE TABLE tbl_2_y(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_2_y VALUES('you''ju','yj','邮局',22016);
CREATE TABLE tbl_1_r(key TEXT,jp TEXT,value TEXT,weight INTEGER);
INSERT INTO tbl_1_r VALUES('ru','r','如',2514076);
INSERT INTO tbl_1_r VALUES('ru','r','乳',26706);";

/// 一张只列 巨人 和 邮局 的表：内嵌表里的 巨乳 不在其中。
fn with_table() -> String {
    format!(
        "{ROWS}\n{}\nINSERT INTO sentence_excluded VALUES('巨人', 0);\nINSERT INTO sentence_excluded VALUES('邮局', 0);",
        crate::format::SENTENCE_EXCLUDED_SCHEMA
    )
}

fn predictions_after_ju(fixture: &Fixture) -> Vec<String> {
    let mut session = fixture.session_with(|options| {
        options.next_word_prediction = true;
        options.learning = false;
    });
    type_text(&mut session, "ju");
    assert_eq!(
        select_word(&mut session, "巨").commit.as_deref(),
        Some("巨")
    );
    assert!(session.snapshot().prediction);
    words(&session)
}

fn sentence_candidates(fixture: &Fixture) -> Vec<String> {
    let mut session = fixture.session_with(|options| options.learning = false);
    type_text(&mut session, "womenyoujuru");
    words(&session)
}

/// 新引擎配旧词库：没有 `sentence_excluded` 表，联想和整句照旧按内嵌表拦 巨乳。
#[test]
fn a_dictionary_without_the_table_keeps_the_embedded_list() {
    let fixture = Fixture::new(ROWS);
    let predictions = predictions_after_ju(&fixture);
    assert!(
        predictions.iter().any(|word| word == "人"),
        "{predictions:?}"
    );
    assert!(
        predictions.iter().all(|word| word != "乳"),
        "{predictions:?}"
    );
    let candidates = sentence_candidates(&fixture);
    assert!(!candidates.is_empty());
    assert!(
        candidates.iter().all(|word| !word.contains("巨乳")),
        "{candidates:?}"
    );
}

/// 新引擎配新词库：表整个取代内嵌表。联想不给 人（巨人 在表里），给 乳（巨乳 不在表里）；整句也照这张表拼。
#[test]
fn a_dictionary_with_the_table_replaces_the_embedded_list() {
    let fixture = Fixture::new(&with_table());
    let predictions = predictions_after_ju(&fixture);
    assert!(
        predictions.iter().any(|word| word == "乳"),
        "{predictions:?}"
    );
    assert!(
        predictions.iter().all(|word| word != "人"),
        "{predictions:?}"
    );
    let candidates = sentence_candidates(&fixture);
    assert!(
        candidates.iter().any(|word| word == "我们有巨乳"),
        "{candidates:?}"
    );
    assert!(
        candidates.iter().all(|word| !word.contains("邮局")),
        "{candidates:?}"
    );
}

/// 旧引擎配新词库：带表的发布文件仍是当前格式版本（新表不改 `FORMAT_VERSION`），准备代次时整个复制进代次，其余的表照常查。旧引擎没有读这张表的代码，按名字查表，不枚举未知的表，所以它看到的就是一个照常可用的词库；新引擎从代次副本里读到同一张表。
#[test]
fn a_release_with_the_table_prepares_and_reads_like_any_other() {
    let root = tempfile::tempdir().unwrap();
    let resources = root.path().join("resources");
    std::fs::create_dir_all(&resources).unwrap();
    let main = resources.join(assets::MAIN_DICTIONARY);
    let connection = Connection::open(&main).unwrap();
    connection.execute_batch(&with_table()).unwrap();
    connection
        .execute_batch(crate::format::METADATA_SCHEMA)
        .unwrap();
    connection
        .execute(
            "INSERT INTO metadata(name, value) VALUES(?1, ?2)",
            rusqlite::params![
                crate::format::METADATA_FORMAT_VERSION,
                crate::format::FORMAT_VERSION.to_string()
            ],
        )
        .unwrap();
    assert_eq!(
        crate::format::check_format_version(&connection, assets::MAIN_DICTIONARY).unwrap(),
        crate::format::FORMAT_VERSION
    );
    drop(connection);
    crate::ensure_english_schema(&resources.join(assets::ENGLISH_DICTIONARY)).unwrap();
    let user = root.path().join("user");
    std::fs::create_dir_all(&user).unwrap();
    let prepared =
        crate::prepare_runtime_paths(&resources, &user, &root.path().join("cache"), "v1").unwrap();
    let copy = Connection::open(prepared.dictionary(assets::MAIN_DICTIONARY)).unwrap();
    let rows: i64 = copy
        .query_row("SELECT COUNT(*) FROM sentence_excluded", [], |row| {
            row.get(0)
        })
        .unwrap();
    assert_eq!(rows, 2);
    drop(copy);

    let mut options = SessionOptions::new(prepared);
    options.learning = false;
    options.next_word_prediction = true;
    let mut session = Session::new(options).unwrap();
    type_text(&mut session, "ju");
    let candidates = words(&session);
    assert!(
        candidates.iter().any(|word| word == "据") && candidates.iter().any(|word| word == "巨"),
        "{candidates:?}"
    );
    assert_eq!(
        select_word(&mut session, "巨").commit.as_deref(),
        Some("巨")
    );
    let predictions = words(&session);
    assert!(
        predictions.iter().any(|word| word == "乳"),
        "{predictions:?}"
    );
    assert!(
        predictions.iter().all(|word| word != "人"),
        "{predictions:?}"
    );
}
