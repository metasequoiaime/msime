//! The desktop dictionary product: release checks, `msime-dictionary-manifest.json` and `msime-SHA256SUMS.txt`. The manifest keeps the schema clients already read (`profile`, `source.commit`, `files`, ...).

use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context, Result};
use indexmap::IndexMap;
use rusqlite::{Connection, OpenFlags};
use serde::Serialize;

use crate::japanese;
use crate::licensing;
use crate::msime::quanpin_tables;
use crate::ngram;
use crate::sources::{sha256_file, Lock, Reference};

pub const MANIFEST: &str = "msime-dictionary-manifest.json";
pub const SHIPPING_ARTIFACTS: [&str; 8] = [
    "msime-pinyin.db",
    "msime-wubi.db",
    "msime-english.db",
    "msime-others.db",
    "msime-japanese.dat",
    "msime-bigram.bin",
    "msime-trigram.bin",
    japanese::NOTICE_NAME,
];
const FEATURES: [&str; 8] = [
    "pinyin",
    "wubi",
    "quick_phrases",
    "english",
    "emoji",
    "kaomoji",
    "symbols",
    "japanese",
];
const REPOSITORY: &str = "metasequoiaime/msime";
const SOURCE_PATH: &str = "resources/dictionary-sources";
/// The dictionary source repository, pinned under this name in the sources lock. The custom words, translations and English words sit in its `custom/` directory; the base lexicons come from the same commit.
const CUSTOM_DICTIONARY: &str = "msime-dictionary";
const CUSTOM_DICTIONARY_REPOSITORY: &str = "metasequoiaime/msime-dictionary";
const CUSTOM_DICTIONARY_PATH: &str = "custom";

#[derive(Serialize)]
struct Manifest {
    manifest_version: u32,
    profile: &'static str,
    format_version: i32,
    engine_compatibility: EngineCompatibility,
    source: Provenance,
    sqlite_journal_mode: &'static str,
    format_contract_commit: String,
    custom_dictionary_commit: String,
    custom_dictionary_repository: &'static str,
    custom_dictionary_path: &'static str,
    references: IndexMap<String, Reference>,
    mozc_revision: String,
    features: [&'static str; 8],
    licensing: Licensing,
    files: IndexMap<&'static str, FileEntry>,
}

#[derive(Serialize)]
struct EngineCompatibility {
    dictionary_format: i32,
    japanese_model_magic: &'static str,
}

#[derive(Serialize)]
struct Provenance {
    repository: &'static str,
    path: &'static str,
    commit: String,
    dirty: bool,
}

#[derive(Serialize)]
struct Licensing {
    includes_unlicensed_inputs: bool,
    excluded_inputs: Vec<&'static str>,
}

#[derive(Serialize)]
struct FileEntry {
    sha256: String,
    size: u64,
}

fn git(repository: &Path, arguments: &[&str]) -> Result<String> {
    let output = Command::new("git")
        .arg("-C")
        .arg(repository)
        .args(arguments)
        .output()
        .context("running git")?;
    if !output.status.success() {
        bail!(
            "git {} failed: {}",
            arguments.join(" "),
            String::from_utf8_lossy(&output.stderr).trim()
        );
    }
    Ok(String::from_utf8(output.stdout)?.trim().to_owned())
}

/// The msime commit the build came from, and whether the builder or its inputs had uncommitted changes: in the msime checkout, and with `--dictionary` also in the msime-dictionary checkout's `sources/` and `custom/`.
fn provenance(repository: &Path, dictionary: Option<&Path>) -> Result<Provenance> {
    let commit = git(repository, &["rev-parse", "HEAD"])?;
    let changes = git(
        repository,
        &[
            "status",
            "--porcelain",
            "--",
            SOURCE_PATH,
            "resources/dictionary-sources.lock.json",
            "crates/dict-builder",
            "crates/engine/src/format.rs",
        ],
    )?;
    let mut dirty = !changes.is_empty();
    if let Some(checkout) = dictionary {
        let changes = git(
            checkout,
            &["status", "--porcelain", "--", "sources", "custom"],
        )?;
        dirty |= !changes.is_empty();
    }
    Ok(Provenance {
        repository: REPOSITORY,
        path: SOURCE_PATH,
        commit,
        dirty,
    })
}

/// The msime-dictionary commit the build read: the lock's reference, or the HEAD of the `--dictionary` checkout the files actually came from.
fn custom_dictionary_reference(lock: &Lock, dictionary: Option<&Path>) -> Result<Reference> {
    let pinned = lock
        .references
        .get(CUSTOM_DICTIONARY)
        .with_context(|| format!("{CUSTOM_DICTIONARY} is not pinned in the sources lock"))?;
    Ok(match dictionary {
        Some(checkout) => Reference {
            repository: pinned.repository.clone(),
            commit: git(checkout, &["rev-parse", "HEAD"])?,
        },
        None => pinned.clone(),
    })
}

fn row_count(connection: &Connection, table: &str) -> Result<i64> {
    Ok(
        connection.query_row(&format!("SELECT COUNT(*) FROM \"{table}\""), [], |row| {
            row.get(0)
        })?,
    )
}

/// 把五笔码表从拼音工作库拆进单独发布的 `msime-wubi.db`。各阶段共用 `msime-pinyin.db` 一个连接，自定义词检查与 n-gram 权重看到的是完整来源，产品检查与清单之前再拆出只读的五笔资源。只搬这次构建在拼音库里留下的表：`--only`/`--skip` 只跑部分阶段、或拼音库已经拆过时，拼音库里没有五笔表，`msime-wubi.db` 里已有的表原样保留；拼音库不存在时什么也不做。
pub fn split_wubi_database(out: &Path) -> Result<()> {
    let pinyin_path = out.join("msime-pinyin.db");
    if !pinyin_path.is_file() {
        return Ok(());
    }
    let pinyin = Connection::open_with_flags(&pinyin_path, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    let mut tables = Vec::new();
    for name in ["wubi86", "wubi98"] {
        let present: bool = pinyin.query_row(
            "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?1)",
            [name],
            |row| row.get(0),
        )?;
        if !present {
            continue;
        }
        // 按 rowid 读写：运行时反查五笔编码以 rowid 作最后的排序键，拆分前后顺序要一致。
        let mut statement = pinyin.prepare(&format!(
            "SELECT \"key\", \"value\", \"weight\" FROM {name} ORDER BY rowid"
        ))?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })?
            .collect::<rusqlite::Result<Vec<_>>>()?;
        tables.push((name, rows));
    }
    drop(pinyin);
    if tables.is_empty() {
        return Ok(());
    }

    let mut wubi = Connection::open(out.join("msime-wubi.db"))?;
    wubi.execute_batch("PRAGMA journal_mode=delete; PRAGMA synchronous=off;")?;
    let transaction = wubi.transaction()?;
    for (name, rows) in &tables {
        transaction.execute_batch(&format!(
            "DROP TABLE IF EXISTS {name}; CREATE TABLE {name} (\"key\" TEXT NOT NULL, \"value\" TEXT NOT NULL, \"weight\" INTEGER NOT NULL DEFAULT 0, UNIQUE(\"key\", \"value\")); CREATE INDEX idx_{name}_key_weight ON {name}(\"key\", \"weight\" DESC);"
        ))?;
        let mut insert = transaction.prepare(&format!(
            "INSERT INTO {name} (\"key\", \"value\", \"weight\") VALUES (?, ?, ?)"
        ))?;
        for (key, value, weight) in rows {
            insert.execute(rusqlite::params![key, value, weight])?;
        }
    }
    transaction.commit()?;
    crate::sqlite::analyze(&wubi, true)?;
    crate::sqlite::integrity_check(&wubi)?;
    drop(wubi);

    let pinyin = Connection::open(&pinyin_path)?;
    for (name, _) in &tables {
        pinyin.execute_batch(&format!("DROP TABLE {name};"))?;
    }
    crate::sqlite::integrity_check(&pinyin)?;
    Ok(())
}

/// Every shipping table exists with at least a floor number of rows, far below today's counts: these catch a table that came out empty because an input silently changed shape, not ordinary dictionary edits. A licensed build legitimately has fewer rows.
pub fn verify(out: &Path, complete: bool) -> Result<()> {
    let floors: [(&str, &[(&str, i64)]); 4] = [
        ("msime-wubi.db", &[("wubi86", 50_000), ("wubi98", 50_000)]),
        ("msime-pinyin.db", &[("quick_parases", 1)]),
        (
            "msime-english.db",
            &[
                ("english_words", if complete { 100_000 } else { 15_000 }),
                ("en_zh_glosses", if complete { 50_000 } else { 15_000 }),
                ("zh_en_glosses", if complete { 20_000 } else { 15_000 }),
            ],
        ),
        (
            "msime-others.db",
            &[
                ("emoji", 1_000),
                ("emoji_pinyin", 1_000),
                ("kaomoji", 500),
                ("kaomoji_catalog", 500),
                ("symbol_catalog", 1_000),
            ],
        ),
    ];
    for (database, tables) in floors {
        let connection =
            Connection::open_with_flags(out.join(database), OpenFlags::SQLITE_OPEN_READ_ONLY)?;
        crate::sqlite::integrity_check(&connection).with_context(|| database.to_owned())?;
        for (table, minimum) in tables {
            let count = row_count(&connection, table)
                .with_context(|| format!("{database}: table {table}"))?;
            if count < *minimum {
                bail!("{database}: {table} has {count} rows, expected at least {minimum}");
            }
        }
    }

    let msime = Connection::open_with_flags(
        out.join("msime-pinyin.db"),
        OpenFlags::SQLITE_OPEN_READ_ONLY,
    )?;
    let mut quanpin_rows = 0;
    for table in quanpin_tables() {
        quanpin_rows += row_count(&msime, &table)
            .with_context(|| format!("msime-pinyin.db: quanpin table {table}"))?;
    }
    let minimum = if complete { 1_000_000 } else { 800_000 };
    if quanpin_rows < minimum {
        bail!("msime-pinyin.db: quanpin rows total {quanpin_rows}, expected at least {minimum}");
    }

    let model = std::fs::read(out.join("msime-japanese.dat"))?;
    if model.len() < 32 * 1024 * 1024 || !model.starts_with(japanese::MAGIC) {
        bail!(
            "msime-japanese.dat is not a complete MSJPDT1 model ({} bytes)",
            model.len()
        );
    }
    for name in ["msime-bigram.bin", "msime-trigram.bin"] {
        let table = std::fs::read(out.join(name))?;
        let field = |offset: usize| {
            table
                .get(offset..offset + 4)
                .map(|bytes| u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
        };
        if !table.starts_with(ngram::MAGIC)
            || field(4) != Some(ngram::VERSION)
            || field(8).unwrap_or(0) < 100_000
        {
            bail!("{name} is not a packed table of at least 100000 entries");
        }
    }
    let notice = std::fs::read_to_string(out.join(japanese::NOTICE_NAME))?.to_lowercase();
    for term in ["ipadic", "icot", "okinawa"] {
        if !notice.contains(term) {
            bail!(
                "{} does not mention {term}; msime-japanese.dat must not ship without it",
                japanese::NOTICE_NAME
            );
        }
    }
    Ok(())
}

/// Writes the manifest and checksums. `dictionary` is the `--dictionary` checkout the msime-dictionary files were read from, if any; the manifest then names its HEAD as the msime-dictionary commit.
pub fn write_manifest(
    out: &Path,
    repository: &Path,
    dictionary: Option<&Path>,
    lock: &Lock,
    complete: bool,
) -> Result<()> {
    let source = provenance(repository, dictionary)?;
    let custom_dictionary = custom_dictionary_reference(lock, dictionary)?;
    let mut files = IndexMap::new();
    for name in SHIPPING_ARTIFACTS {
        let path = out.join(name);
        files.insert(
            name,
            FileEntry {
                sha256: sha256_file(&path)?,
                size: std::fs::metadata(&path)?.len(),
            },
        );
    }
    let manifest = Manifest {
        manifest_version: 1,
        profile: "desktop",
        format_version: msime_engine::format::FORMAT_VERSION,
        engine_compatibility: EngineCompatibility {
            dictionary_format: msime_engine::format::FORMAT_VERSION,
            japanese_model_magic: "MSJPDT1",
        },
        sqlite_journal_mode: "delete",
        format_contract_commit: source.commit.clone(),
        custom_dictionary_commit: custom_dictionary.commit.clone(),
        custom_dictionary_repository: CUSTOM_DICTIONARY_REPOSITORY,
        custom_dictionary_path: CUSTOM_DICTIONARY_PATH,
        references: lock
            .references
            .iter()
            .map(|(name, reference)| {
                let reference = if name == CUSTOM_DICTIONARY {
                    custom_dictionary.clone()
                } else {
                    reference.clone()
                };
                (name.clone(), reference)
            })
            .collect(),
        mozc_revision: lock.mozc.commit.clone(),
        features: FEATURES,
        licensing: Licensing {
            includes_unlicensed_inputs: complete,
            excluded_inputs: if complete {
                Vec::new()
            } else {
                licensing::UNLICENSED_INPUTS
                    .iter()
                    .map(|(input, _, _)| *input)
                    .collect()
            },
        },
        files,
        source,
    };
    let mut text = serde_json::to_string_pretty(&manifest)?;
    text.push('\n');
    std::fs::write(out.join(MANIFEST), text)?;

    let mut sums = String::new();
    for name in SHIPPING_ARTIFACTS.iter().chain([&MANIFEST]) {
        sums.push_str(&format!("{}  {name}\n", sha256_file(&out.join(name))?));
    }
    std::fs::write(out.join("msime-SHA256SUMS.txt"), sums)?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::BTreeMap;

    fn git_repository_with_commit(directory: &Path) -> String {
        let run = |arguments: &[&str]| {
            let status = Command::new("git")
                .arg("-C")
                .arg(directory)
                .args([
                    "-c",
                    "user.name=fixture",
                    "-c",
                    "user.email=fixture@example.invalid",
                    "-c",
                    "commit.gpgsign=false",
                ])
                .args(arguments)
                .status()
                .unwrap();
            assert!(status.success(), "git {arguments:?}");
        };
        run(&["init", "--quiet"]);
        std::fs::create_dir_all(directory.join("custom")).unwrap();
        std::fs::write(directory.join("custom/words.txt"), "词\tci\t1\n").unwrap();
        run(&["add", "custom/words.txt"]);
        run(&["commit", "--quiet", "-m", "fixture"]);
        git(directory, &["rev-parse", "HEAD"]).unwrap()
    }

    /// With `--dictionary` the manifest names the checkout's HEAD, not the lock's commit, and an uncommitted change to its sources or custom files marks the build dirty; without it the lock's reference is reported unchanged.
    #[test]
    fn a_dictionary_checkout_is_the_recorded_provenance() {
        let msime = tempfile::tempdir().unwrap();
        let checkout = tempfile::tempdir().unwrap();
        git_repository_with_commit(msime.path());
        let head = git_repository_with_commit(checkout.path());
        let pinned = Reference {
            repository: "https://github.com/metasequoiaime/msime-dictionary.git".into(),
            commit: "a".repeat(40),
        };
        let lock = Lock {
            references: BTreeMap::from([(CUSTOM_DICTIONARY.to_owned(), pinned.clone())]),
            mozc: pinned.clone(),
            files: Vec::new(),
        };

        let reference = custom_dictionary_reference(&lock, Some(checkout.path())).unwrap();
        assert_eq!(reference.commit, head);
        assert_eq!(reference.repository, pinned.repository);
        let reference = custom_dictionary_reference(&lock, None).unwrap();
        assert_eq!(reference.commit, pinned.commit);

        assert!(
            !provenance(msime.path(), Some(checkout.path()))
                .unwrap()
                .dirty
        );
        std::fs::write(checkout.path().join("custom/words.txt"), "changed\n").unwrap();
        assert!(
            provenance(msime.path(), Some(checkout.path()))
                .unwrap()
                .dirty
        );
        assert!(!provenance(msime.path(), None).unwrap().dirty);
    }

    #[test]
    fn split_wubi_moves_tables_to_the_named_database() {
        let directory = tempfile::tempdir().unwrap();
        let pinyin = Connection::open(directory.path().join("msime-pinyin.db")).unwrap();
        pinyin
            .execute_batch(
                "CREATE TABLE tbl_1_a(key TEXT,jp TEXT,value TEXT,weight INTEGER); CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER); CREATE TABLE wubi98(key TEXT,value TEXT,weight INTEGER); INSERT INTO wubi86 VALUES('aaa','甲',10); INSERT INTO wubi98 VALUES('bbb','乙',20);",
            )
            .unwrap();
        drop(pinyin);

        split_wubi_database(directory.path()).unwrap();

        let pinyin = Connection::open(directory.path().join("msime-pinyin.db")).unwrap();
        assert!(pinyin
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE type = 'table' AND name IN ('wubi86', 'wubi98')",
                [],
                |row| row.get::<_, i64>(0),
            )
            .unwrap() == 0);
        let wubi = Connection::open(directory.path().join("msime-wubi.db")).unwrap();
        assert_eq!(
            wubi.query_row("SELECT value FROM wubi86 WHERE key = 'aaa'", [], |row| {
                row.get::<_, String>(0)
            })
            .unwrap(),
            "甲"
        );
        assert_eq!(
            wubi.query_row("SELECT value FROM wubi98 WHERE key = 'bbb'", [], |row| {
                row.get::<_, String>(0)
            })
            .unwrap(),
            "乙"
        );
    }

    /// 只跑部分阶段时拼音库里没有、或只有一部分五笔表：已拆出的表原样保留，这次重建的表整张替换，顺序按原 rowid；输出目录里还没有拼音库时不报错。
    #[test]
    fn split_wubi_keeps_tables_this_build_did_not_rebuild() {
        let directory = tempfile::tempdir().unwrap();
        split_wubi_database(directory.path()).unwrap();
        assert!(!directory.path().join("msime-wubi.db").exists());

        let pinyin_path = directory.path().join("msime-pinyin.db");
        Connection::open(&pinyin_path)
            .unwrap()
            .execute_batch(
                "CREATE TABLE tbl_1_a(key TEXT,jp TEXT,value TEXT,weight INTEGER); CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER); CREATE TABLE wubi98(key TEXT,value TEXT,weight INTEGER); INSERT INTO wubi86 VALUES('aaa','甲',10); INSERT INTO wubi98 VALUES('bbb','乙',20);",
            )
            .unwrap();
        split_wubi_database(directory.path()).unwrap();
        // 再跑一次（例如 `--only emoji`）：拼音库里已经没有五笔表，什么都不变。
        split_wubi_database(directory.path()).unwrap();

        // `--only wubi` 只重建了 86 表：它被替换，98 表保留。
        Connection::open(&pinyin_path)
            .unwrap()
            .execute_batch(
                "CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER); INSERT INTO wubi86(rowid,key,value,weight) VALUES(9,'ccc','丙',5),(2,'ddd','丁',5);",
            )
            .unwrap();
        split_wubi_database(directory.path()).unwrap();

        let wubi = Connection::open(directory.path().join("msime-wubi.db")).unwrap();
        let values = |table: &str| -> Vec<String> {
            wubi.prepare(&format!("SELECT value FROM {table} ORDER BY rowid"))
                .unwrap()
                .query_map([], |row| row.get(0))
                .unwrap()
                .collect::<rusqlite::Result<_>>()
                .unwrap()
        };
        assert_eq!(values("wubi86"), ["丁", "丙"]);
        assert_eq!(values("wubi98"), ["乙"]);
        let left: i64 = Connection::open(&pinyin_path)
            .unwrap()
            .query_row(
                "SELECT COUNT(*) FROM sqlite_master WHERE name LIKE 'wubi%'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(left, 0);
    }
}
