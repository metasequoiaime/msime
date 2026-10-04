//! Fixture staging, as the recorder's `prepare_fixture` does it: `english_schema` first, then each database's SQL verbatim, then the files, then an empty `msime-pinyin.db` and a schema-only `msime-english.db` for whichever is still missing.

use std::fs;
use std::path::Path;

use msime_engine::assets::{ENGLISH_DICTIONARY, MAIN_DICTIONARY};
use rusqlite::Connection;
use serde_json::Value;

pub fn stage_fixture(fixture: &Value, resources: &Path) {
    assert!(
        fixture.get("resources").is_none(),
        "scripted scenarios use inline fixtures; real resources go through the real sets"
    );
    fs::create_dir_all(resources).unwrap();
    let english = resources.join(ENGLISH_DICTIONARY);
    // Reference tests that call EnglishDictionary::ensure_schema before inserting their English rows set this.
    if fixture["english_schema"].as_bool().unwrap_or(false) {
        msime_engine::ensure_english_schema(&english)
            .unwrap_or_else(|error| panic!("english schema failed: {error}"));
    }
    if let Some(databases) = fixture.get("databases").and_then(Value::as_object) {
        for (name, sql) in databases {
            let sql = sql
                .as_str()
                .unwrap_or_else(|| panic!("fixture SQL for {name} is not a string"));
            exec_sql(&resources.join(name), sql);
        }
    }
    if let Some(files) = fixture.get("files").and_then(Value::as_object) {
        for (name, text) in files {
            let path = resources.join(name);
            fs::create_dir_all(path.parent().unwrap()).unwrap();
            let text = text
                .as_str()
                .unwrap_or_else(|| panic!("fixture file {name} is not a string"));
            fs::write(&path, text)
                .unwrap_or_else(|error| panic!("cannot write fixture file {name}: {error}"));
        }
    }
    let main = resources.join(MAIN_DICTIONARY);
    if !main.exists() {
        // The recorder's statements, so the file carries a real SQLite header rather than zero bytes.
        exec_sql(
            &main,
            "CREATE TABLE golden_empty(x); DROP TABLE golden_empty;",
        );
    }
    if !english.exists() {
        msime_engine::ensure_english_schema(&english)
            .unwrap_or_else(|error| panic!("english schema failed: {error}"));
    }
}

/// Run fixture SQL on a database file, creating it.
pub fn exec_sql(path: &Path, sql: &str) {
    fs::create_dir_all(path.parent().unwrap()).unwrap();
    let connection = Connection::open(path)
        .unwrap_or_else(|error| panic!("cannot open {}: {error}", path.display()));
    if let Err(error) = connection.execute_batch(sql) {
        panic!(
            "fixture SQL failed on {}: {error}",
            path.file_name().unwrap().to_string_lossy()
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::golden_support::golden_dir;
    use crate::golden_support::snapshot::query_rows;
    use serde_json::json;

    fn scenario(name: &str) -> Value {
        let path = golden_dir().join("scenarios").join(format!("{name}.json"));
        serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap()
    }

    #[test]
    fn stages_databases_files_and_a_schema_only_english_dictionary() {
        let dir = tempfile::tempdir().unwrap();
        let resources = dir.path().join("resources");
        stage_fixture(
            &scenario("qp_paging_and_page_selection")["fixture"],
            &resources,
        );
        assert_eq!(
            query_rows(
                &resources.join(MAIN_DICTIONARY),
                "SELECT value FROM tbl_2_n ORDER BY weight DESC"
            ),
            json!([["你好"], ["拟好"], ["𠀀方案𠮷"], ["C语言 2"], ["GitHub"]])
        );
        assert_eq!(
            fs::read_to_string(resources.join("helpcodes/zrm_helpcode_big_unique.txt")).unwrap(),
            "你=cb\n拟=ad\n好=ef\n"
        );
        assert_eq!(
            query_rows(
                &resources.join(ENGLISH_DICTIONARY),
                "SELECT count(*) FROM english_words"
            ),
            json!([[0]])
        );
    }

    #[test]
    fn english_schema_runs_before_the_fixture_rows() {
        let dir = tempfile::tempdir().unwrap();
        let fixture = json!({
            "english_schema": true,
            "databases": {"msime-english.db": "INSERT INTO english_words(word, display, weight) VALUES('codex', 'Codex', 10);"},
        });
        stage_fixture(&fixture, dir.path());
        assert_eq!(
            query_rows(
                &dir.path().join(ENGLISH_DICTIONARY),
                "SELECT word, display, weight FROM english_words"
            ),
            json!([["codex", "Codex", 10]])
        );
        assert_eq!(
            query_rows(
                &dir.path().join(MAIN_DICTIONARY),
                "SELECT count(*) FROM sqlite_master"
            ),
            json!([[0]])
        );
    }

    #[test]
    fn every_scenario_fixture_stages() {
        let dir = tempfile::tempdir().unwrap();
        for path in crate::golden_support::replay::selected_scenarios_from(None) {
            let spec: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
            let resources = dir.path().join(spec["name"].as_str().unwrap());
            stage_fixture(&spec["fixture"], &resources);
            assert!(resources.join(MAIN_DICTIONARY).exists());
            assert!(resources.join(ENGLISH_DICTIONARY).exists());
        }
    }

    #[test]
    #[should_panic(expected = "fixture SQL failed on msime-pinyin.db")]
    fn bad_fixture_sql_fails_the_scenario() {
        let dir = tempfile::tempdir().unwrap();
        exec_sql(&dir.path().join(MAIN_DICTIONARY), "CREATE TABLE");
    }
}
