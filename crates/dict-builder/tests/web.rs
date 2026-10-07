//! `msime-dict-build web` 的端到端测试：用合成的小号 msime-pinyin.db 和 msime-wubi.db 跑编译出的二进制，检查裁剪结果。

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

/// 按词库 release 的结构建两个小库：`msime-pinyin.db` 里每张全拼表都有两个索引，`quick_parases` 有一个索引；`msime-wubi.db` 里 `wubi86`、`wubi98` 各有一个索引。
fn fixture(pinyin: &Path, wubi: &Path) {
    let mut sql = String::from("BEGIN;");
    for table in quanpin_tables() {
        let suffix = table.trim_start_matches("tbl_");
        sql.push_str(&format!(
            "CREATE TABLE {table} {QUANPIN_SCHEMA}; CREATE INDEX idx_key_{suffix} ON {table}(key); CREATE INDEX idx_jp_{suffix} ON {table}(jp);"
        ));
    }
    sql.push_str(&keyed_table("quick_parases"));
    sql.push_str(
        "INSERT INTO tbl_1_a VALUES ('a', 'a', '啊', 10), ('ai', 'a', '爱', 1);
         INSERT INTO tbl_1_b VALUES ('ba', 'b', '把', 0);
         INSERT INTO tbl_2_n VALUES ('ni''hao', 'nh', '你好', 900), ('ni''men', 'nm', '你们', 500), ('nan''ren', 'nr', '男人', 100);
         INSERT INTO tbl_2_w VALUES ('wo''men', 'wm', '我们', 800), ('wan''shang', 'ws', '晚上', 100);
         INSERT INTO tbl_3_z VALUES ('zhong''guo''ren', 'zgr', '中国人', 700), ('zao''shang''hao', 'zsh', '早上好', 100);
         INSERT INTO tbl_others_x VALUES ('xi''huan''ni''men''de''ge''qu''ya', 'xhnmdgqy', '喜欢你们的歌曲呀', 100);
         INSERT INTO quick_parases VALUES ('dz', '地址', 1);
         COMMIT;",
    );
    Connection::open(pinyin)
        .unwrap()
        .execute_batch(&sql)
        .unwrap();
    let mut sql = String::from("BEGIN;");
    sql.push_str(&keyed_table("wubi86"));
    sql.push_str(&keyed_table("wubi98"));
    sql.push_str(
        "INSERT INTO wubi86 VALUES ('wqiy', '你', 3), ('q', '我', 9), ('ggll', '一', 1);
         INSERT INTO wubi98 VALUES ('wqiy', '你', 3);
         COMMIT;",
    );
    Connection::open(wubi).unwrap().execute_batch(&sql).unwrap();
}

fn keyed_table(table: &str) -> String {
    format!(
        "CREATE TABLE {table} {KEYED_SCHEMA}; CREATE INDEX idx_{table}_key_weight ON {table}(\"key\", \"weight\" DESC);"
    )
}

fn command(pinyin: &Path, wubi: &Path, out_dir: &Path, keep_multi: Option<usize>) -> Command {
    let mut command = Command::new(env!("CARGO_BIN_EXE_msime-dict-build"));
    command
        .arg("web")
        .arg("--pinyin")
        .arg(pinyin)
        .arg("--wubi")
        .arg(wubi)
        .arg("--out-dir")
        .arg(out_dir);
    if let Some(keep) = keep_multi {
        command.arg("--keep-multi").arg(keep.to_string());
    }
    command
}

fn run(pinyin: &Path, wubi: &Path, out_dir: &Path, keep_multi: Option<usize>) {
    let output = command(pinyin, wubi, out_dir, keep_multi).output().unwrap();
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
    pinyin: PathBuf,
    wubi: PathBuf,
    out: PathBuf,
}

fn inputs() -> (tempfile::TempDir, PathBuf, PathBuf) {
    let dir = tempfile::tempdir().unwrap();
    let pinyin = dir.path().join("msime-pinyin.db");
    let wubi = dir.path().join("msime-wubi.db");
    fixture(&pinyin, &wubi);
    (dir, pinyin, wubi)
}

fn built(keep_multi: Option<usize>) -> Built {
    let (dir, pinyin, wubi) = inputs();
    let out = dir.path().join("web");
    run(&pinyin, &wubi, &out, keep_multi);
    Built {
        pinyin,
        wubi,
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
    let source = open(&built.pinyin);
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
    let union = |kind: &str| {
        let mut all = names(&open(&built.pinyin), kind);
        all.extend(names(&open(&built.wubi), kind));
        all.sort();
        all
    };
    for name in ["msime-pinyin.db", "msime-wubi86.db"] {
        let db = open(&built.out.join(name));
        assert_eq!(names(&db, "table"), union("table"), "{name}");
        assert_eq!(names(&db, "index"), union("index"), "{name}");
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
    run(&built.pinyin, &built.wubi, &built.out, Some(3));
    let second: Vec<String> = ["msime-pinyin.db", "msime-wubi86.db"]
        .iter()
        .map(|name| sha256(&built.out.join(name)))
        .collect();
    assert_eq!(first, second);
    let elsewhere = built.out.with_file_name("again");
    run(&built.pinyin, &built.wubi, &elsewhere, Some(3));
    let third: Vec<String> = ["msime-pinyin.db", "msime-wubi86.db"]
        .iter()
        .map(|name| sha256(&elsewhere.join(name)))
        .collect();
    assert_eq!(first, third);
}

#[test]
fn the_inputs_are_left_untouched() {
    let (dir, pinyin, wubi) = inputs();
    let before = [sha256(&pinyin), sha256(&wubi)];
    run(&pinyin, &wubi, &dir.path().join("web"), Some(1));
    assert_eq!([sha256(&pinyin), sha256(&wubi)], before);
}

#[test]
fn swapped_inputs_are_rejected() {
    let (dir, pinyin, wubi) = inputs();
    let output = command(&wubi, &pinyin, &dir.path().join("web"), None)
        .output()
        .unwrap();
    assert!(!output.status.success());
}

// ---- 日语模型 ----

/// MSJPDT1 文件：`(读法, 词, 成本)` 按读法排好，左右上下文都是 0，1×1 的连接矩阵。
fn japanese_model(entries: &[(&str, &str, i32)]) -> Vec<u8> {
    let mut strings = Vec::new();
    let mut tokens = Vec::new();
    for &(reading, surface, cost) in entries {
        for text in [reading, surface] {
            tokens.extend_from_slice(&(strings.len() as u32).to_le_bytes());
            tokens.extend_from_slice(&(text.len() as u16).to_le_bytes());
            strings.extend_from_slice(text.as_bytes());
        }
        tokens.extend_from_slice(&[0, 0, 0, 0]);
        tokens.extend_from_slice(&cost.to_le_bytes());
    }
    let connection_offset = 56 + tokens.len() as u64;
    let mut file = b"MSJPDT1\0".to_vec();
    for value in [1u32, entries.len() as u32, 1, 0] {
        file.extend_from_slice(&value.to_le_bytes());
    }
    for value in [
        56,
        connection_offset,
        connection_offset + 2,
        strings.len() as u64,
    ] {
        file.extend_from_slice(&value.to_le_bytes());
    }
    file.extend_from_slice(&tokens);
    file.extend_from_slice(&7i16.to_le_bytes());
    file.extend_from_slice(&strings);
    file
}

/// 读出模型里的 `(读法, 词, 成本)` 和连接矩阵的第一个值。
fn japanese_entries(bytes: &[u8]) -> (Vec<(String, String, i32)>, i16) {
    let u32_at = |at: usize| u32::from_le_bytes(bytes[at..at + 4].try_into().unwrap());
    let u64_at = |at: usize| u64::from_le_bytes(bytes[at..at + 8].try_into().unwrap()) as usize;
    let u16_at = |at: usize| u16::from_le_bytes(bytes[at..at + 2].try_into().unwrap());
    let (tokens, connection, strings) = (u64_at(24), u64_at(32), u64_at(40));
    let text = |offset: u32, length: u16| {
        let start = strings + offset as usize;
        std::str::from_utf8(&bytes[start..start + length as usize])
            .unwrap()
            .to_owned()
    };
    let entries = (0..u32_at(12) as usize)
        .map(|index| {
            let at = tokens + index * 20;
            (
                text(u32_at(at), u16_at(at + 4)),
                text(u32_at(at + 6), u16_at(at + 10)),
                i32::from_le_bytes(bytes[at + 16..at + 20].try_into().unwrap()),
            )
        })
        .collect();
    (
        entries,
        i16::from_le_bytes(bytes[connection..connection + 2].try_into().unwrap()),
    )
}

fn run_japanese(keep: Option<usize>) -> (tempfile::TempDir, PathBuf) {
    let (dir, pinyin, wubi) = inputs();
    let model = dir.path().join("msime-japanese.dat");
    std::fs::write(
        &model,
        japanese_model(&[
            ("あい", "愛", 300),
            ("あい", "藍", 900),
            ("かな", "仮名", 100),
            ("かな", "金", 300),
            ("ひと", "人", 50),
        ]),
    )
    .unwrap();
    let out = dir.path().join("web");
    let mut command = command(&pinyin, &wubi, &out, None);
    command.arg("--japanese").arg(&model);
    if let Some(keep) = keep {
        command.arg("--keep-japanese").arg(keep.to_string());
    }
    let output = command.output().unwrap();
    assert!(
        output.status.success(),
        "msime-dict-build web failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    (dir, out.join("msime-japanese.dat"))
}

#[test]
fn japanese_keeps_the_cheapest_tokens_in_reading_order() {
    let (_dir, model) = run_japanese(Some(3));
    let (entries, connection) = japanese_entries(&std::fs::read(&model).unwrap());
    // 成本 50、100 和 300 的第一条（あい 愛 在文件里先于 かな 金）；顺序仍按读法。
    let entries: Vec<_> = entries
        .iter()
        .map(|(reading, surface, cost)| (reading.as_str(), surface.as_str(), *cost))
        .collect();
    assert_eq!(
        entries,
        [
            ("あい", "愛", 300),
            ("かな", "仮名", 100),
            ("ひと", "人", 50)
        ]
    );
    assert_eq!(connection, 7);
}

#[test]
fn japanese_default_keeps_a_small_model_whole_and_reproducibly() {
    let (_first_dir, first) = run_japanese(None);
    let (_second_dir, second) = run_japanese(None);
    assert_eq!(japanese_entries(&std::fs::read(&first).unwrap()).0.len(), 5);
    assert_eq!(sha256(&first), sha256(&second));
}

#[test]
fn without_japanese_no_model_is_written() {
    let built = built(None);
    assert!(!built.out.join("msime-japanese.dat").exists());
}
