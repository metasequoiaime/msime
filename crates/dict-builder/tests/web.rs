//! `msime-dict-build web` 的端到端测试：用合成的小号 msime.db 跑编译出的二进制，检查裁剪结果。

use std::path::{Path, PathBuf};
use std::process::Command;

use msime_engine::format::{quanpin_table, MAXIMUM_NUMBERED_SYLLABLES, SHIPPED_INITIALS};
use rusqlite::{Connection, OpenFlags};
use sha2::{Digest, Sha256};

const QUANPIN_SCHEMA: &str =
    "(\"key\" text, \"jp\" text, \"value\" text, \"weight\" integer default 0)";
const KEYED_SCHEMA: &str = "(\"key\" TEXT NOT NULL, \"value\" TEXT NOT NULL, \"weight\" INTEGER NOT NULL DEFAULT 0, UNIQUE(\"key\", \"value\"))";

fn quanpin_tables() -> Vec<String> {
    (1..=MAXIMUM_NUMBERED_SYLLABLES + 1)
        .flat_map(|count| {
            SHIPPED_INITIALS
                .bytes()
                .filter_map(move |initial| quanpin_table(count, initial))
        })
        .collect()
}

/// 按真实 msime.db 的结构建一个小库：每张全拼表都有两个索引，`wubi86`、`wubi98`、`quick_parases` 各有一个索引。
fn fixture(path: &Path) {
    let connection = Connection::open(path).unwrap();
    let mut sql = String::from("BEGIN;");
    for table in quanpin_tables() {
        let suffix = table.trim_start_matches("tbl_");
        sql.push_str(&format!(
            "CREATE TABLE {table} {QUANPIN_SCHEMA}; CREATE INDEX idx_key_{suffix} ON {table}(key); CREATE INDEX idx_jp_{suffix} ON {table}(jp);"
        ));
    }
    for table in ["wubi86", "wubi98", "quick_parases"] {
        sql.push_str(&format!(
            "CREATE TABLE {table} {KEYED_SCHEMA}; CREATE INDEX idx_{table}_key_weight ON {table}(\"key\", \"weight\" DESC);"
        ));
    }
    sql.push_str(
        "INSERT INTO tbl_1_a VALUES ('a', 'a', '啊', 10), ('ai', 'a', '爱', 1);
         INSERT INTO tbl_1_b VALUES ('ba', 'b', '把', 0);
         INSERT INTO tbl_2_n VALUES ('ni''hao', 'nh', '你好', 900), ('ni''men', 'nm', '你们', 500), ('nan''ren', 'nr', '男人', 100);
         INSERT INTO tbl_2_w VALUES ('wo''men', 'wm', '我们', 800), ('wan''shang', 'ws', '晚上', 100);
         INSERT INTO tbl_3_z VALUES ('zhong''guo''ren', 'zgr', '中国人', 700), ('zao''shang''hao', 'zsh', '早上好', 100);
         INSERT INTO tbl_others_x VALUES ('xi''huan''ni''men''de''ge''qu''ya', 'xhnmdgqy', '喜欢你们的歌曲呀', 100);
         INSERT INTO wubi86 VALUES ('wqiy', '你', 3), ('q', '我', 9), ('ggll', '一', 1);
         INSERT INTO wubi98 VALUES ('wqiy', '你', 3);
         INSERT INTO quick_parases VALUES ('dz', '地址', 1);
         COMMIT;",
    );
    connection.execute_batch(&sql).unwrap();
}

fn run(input: &Path, out_dir: &Path, keep_multi: Option<usize>) {
    let mut command = Command::new(env!("CARGO_BIN_EXE_msime-dict-build"));
    command
        .arg("web")
        .arg("--input")
        .arg(input)
        .arg("--out-dir")
        .arg(out_dir);
    if let Some(keep) = keep_multi {
        command.arg("--keep-multi").arg(keep.to_string());
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "msime-dict-build web failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
}

fn open(path: &Path) -> Connection {
    Connection::open_with_flags(path, OpenFlags::SQLITE_OPEN_READ_ONLY).unwrap()
}

fn rows(connection: &Connection, table: &str) -> Vec<(String, i64)> {
    connection
        .prepare(&format!(
            "SELECT value, weight FROM \"{table}\" ORDER BY rowid"
        ))
        .unwrap()
        .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

fn count(connection: &Connection, table: &str) -> i64 {
    connection
        .query_row(&format!("SELECT count(*) FROM \"{table}\""), [], |row| {
            row.get(0)
        })
        .unwrap()
}

fn names(connection: &Connection, kind: &str) -> Vec<String> {
    connection
        .prepare("SELECT name FROM sqlite_master WHERE type = ?1 AND name NOT LIKE 'sqlite_%' ORDER BY name")
        .unwrap()
        .query_map([kind], |row| row.get(0))
        .unwrap()
        .map(Result::unwrap)
        .collect()
}

fn sha256(path: &Path) -> String {
    hex::encode(Sha256::digest(std::fs::read(path).unwrap()))
}

struct Built {
    _dir: tempfile::TempDir,
    input: PathBuf,
    out: PathBuf,
}

fn built(keep_multi: Option<usize>) -> Built {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("msime.db");
    fixture(&input);
    let out = dir.path().join("web");
    run(&input, &out, keep_multi);
    Built {
        input,
        out,
        _dir: dir,
    }
}

#[test]
fn pinyin_keeps_every_single_and_the_top_multi_rows() {
    let built = built(Some(4));
    let db = open(&built.out.join("msime-pinyin.db"));
    assert_eq!(rows(&db, "tbl_1_a"), [("啊".into(), 10), ("爱".into(), 1)]);
    assert_eq!(rows(&db, "tbl_1_b"), [("把".into(), 0)]);
    assert_eq!(
        rows(&db, "tbl_2_n"),
        [("你好".into(), 900), ("你们".into(), 500)]
    );
    assert_eq!(rows(&db, "tbl_2_w"), [("我们".into(), 800)]);
    assert_eq!(rows(&db, "tbl_3_z"), [("中国人".into(), 700)]);
    assert_eq!(count(&db, "tbl_others_x"), 0);
    for table in ["wubi86", "wubi98", "quick_parases"] {
        assert_eq!(count(&db, table), 0, "{table} should be empty");
    }
}

#[test]
fn pinyin_breaks_weight_ties_by_key() {
    // 四行同为权重 100 的多字词里，按 key 排序取前两行：nan'ren 和 wan'shang。
    let built = built(Some(6));
    let db = open(&built.out.join("msime-pinyin.db"));
    assert_eq!(
        rows(&db, "tbl_2_n"),
        [
            ("你好".into(), 900),
            ("你们".into(), 500),
            ("男人".into(), 100)
        ]
    );
    assert_eq!(
        rows(&db, "tbl_2_w"),
        [("我们".into(), 800), ("晚上".into(), 100)]
    );
    assert_eq!(rows(&db, "tbl_3_z"), [("中国人".into(), 700)]);
    assert_eq!(count(&db, "tbl_others_x"), 0);
}

#[test]
fn pinyin_default_keeps_every_row_of_a_small_input() {
    let built = built(None);
    let db = open(&built.out.join("msime-pinyin.db"));
    let source = open(&built.input);
    for table in quanpin_tables() {
        assert_eq!(count(&db, &table), count(&source, &table), "{table}");
    }
    assert_eq!(count(&db, "wubi86"), 0);
}

#[test]
fn wubi86_keeps_only_the_wubi86_table() {
    let built = built(None);
    let db = open(&built.out.join("msime-wubi86.db"));
    assert_eq!(
        rows(&db, "wubi86"),
        [("你".into(), 3), ("我".into(), 9), ("一".into(), 1)]
    );
    for table in quanpin_tables() {
        assert_eq!(count(&db, &table), 0, "{table} should be empty");
    }
    assert_eq!(count(&db, "wubi98"), 0);
    assert_eq!(count(&db, "quick_parases"), 0);
}

#[test]
fn both_outputs_keep_every_table_and_index() {
    let built = built(Some(2));
    let source = open(&built.input);
    for name in ["msime-pinyin.db", "msime-wubi86.db"] {
        let db = open(&built.out.join(name));
        assert_eq!(names(&db, "table"), names(&source, "table"), "{name}");
        assert_eq!(names(&db, "index"), names(&source, "index"), "{name}");
        assert!(names(&db, "index").contains(&"idx_wubi86_key_weight".to_owned()));
        let mode: String = db
            .query_row("PRAGMA journal_mode", [], |row| row.get(0))
            .unwrap();
        assert_eq!(mode, "delete", "{name}");
        let free: i64 = db
            .query_row("PRAGMA freelist_count", [], |row| row.get(0))
            .unwrap();
        assert_eq!(free, 0, "{name}");
        let stat4: i64 = db
            .query_row(
                "SELECT count(*) FROM sqlite_master WHERE name = 'sqlite_stat4'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(stat4, 0, "{name}");
    }
    assert!(!built.out.join("msime-pinyin.db.scratch").exists());
    assert!(!built.out.join("msime-wubi86.db.scratch").exists());
}

#[test]
fn two_runs_give_the_same_bytes() {
    let built = built(Some(3));
    let first: Vec<String> = ["msime-pinyin.db", "msime-wubi86.db"]
        .iter()
        .map(|name| sha256(&built.out.join(name)))
        .collect();
    // 第二次写到已有输出的同一目录，同时验证覆盖旧文件。
    run(&built.input, &built.out, Some(3));
    let second: Vec<String> = ["msime-pinyin.db", "msime-wubi86.db"]
        .iter()
        .map(|name| sha256(&built.out.join(name)))
        .collect();
    assert_eq!(first, second);
    let elsewhere = built.out.with_file_name("again");
    run(&built.input, &elsewhere, Some(3));
    let third: Vec<String> = ["msime-pinyin.db", "msime-wubi86.db"]
        .iter()
        .map(|name| sha256(&elsewhere.join(name)))
        .collect();
    assert_eq!(first, third);
}

#[test]
fn the_input_is_left_untouched() {
    let dir = tempfile::tempdir().unwrap();
    let input = dir.path().join("msime.db");
    fixture(&input);
    let before = sha256(&input);
    run(&input, &dir.path().join("web"), Some(1));
    assert_eq!(sha256(&input), before);
}
