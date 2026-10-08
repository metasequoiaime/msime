//! The desktop dictionary product: release checks, `msime-dictionary-manifest.json` and `msime-SHA256SUMS.txt`. The manifest keeps the schema clients already read (`profile`, `source.commit`, `files`, ...).

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

use anyhow::{bail, Context, Result};
use indexmap::IndexMap;
use rusqlite::Connection;
use serde::Serialize;

use crate::english_supplement;
use crate::japanese;
use crate::licensing;
use crate::msime::quanpin_tables;
use crate::ngram;
use crate::sources::{sha256_file, Lock, Reference};

pub const MANIFEST: &str = "msime-dictionary-manifest.json";
/// 产物和 manifest 的校验和文件，与 manifest 一起写出。
pub const SUMS: &str = "msime-SHA256SUMS.txt";
pub const SHIPPING_ARTIFACTS: [&str; 10] = [
    "msime-pinyin.db",
    "msime-wubi.db",
    "msime-english.db",
    "msime-others.db",
    "msime-japanese.dat",
    "msime-bigram.bin",
    "msime-trigram.bin",
    japanese::NOTICE_NAME,
    japanese::LICENSE_NAME,
    english_supplement::NOTICE_NAME,
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
/// manifest 的 references 用这个名字列出词库源仓库，提交取 `--dictionary` checkout 的 HEAD。自定义词、翻译和英文词在它的 `custom/` 目录，基础词库来自同一个提交。
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

/// 构建所在的 msime 提交，以及构建器或其输入是否有未提交改动：检查 msime checkout，有 `--dictionary` 时还检查 msime-dictionary checkout 的 `sources/`、`custom/` 和 `upstream.lock.json`。
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
            &[
                "status",
                "--porcelain",
                "--",
                "sources",
                "custom",
                crate::sources::UPSTREAM_LOCK,
            ],
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

/// 构建读取的 msime-dictionary 提交：`--dictionary` checkout 的 HEAD。
fn custom_dictionary_reference(dictionary: &Path) -> Result<Reference> {
    Ok(Reference {
        repository: format!("https://github.com/{CUSTOM_DICTIONARY_REPOSITORY}.git"),
        commit: git(dictionary, &["rev-parse", "HEAD"])?,
    })
}

/// 生成器写进表头的 msime 提交：`provenance` 判定构建器或其输入有未提交改动时加 `-dirty` 后缀。
pub(crate) fn builder_commit(repository: &Path) -> Result<String> {
    let source = provenance(repository, None)?;
    Ok(if source.dirty {
        format!("{}-dirty", source.commit)
    } else {
        source.commit
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
    let pinyin = crate::sqlite::open_read_only(&pinyin_path)?;
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
        let connection = crate::sqlite::open_read_only(&out.join(database))?;
        crate::sqlite::integrity_check(&connection).with_context(|| database.to_owned())?;
        for (table, minimum) in tables {
            let count = row_count(&connection, table)
                .with_context(|| format!("{database}: table {table}"))?;
            if count < *minimum {
                bail!("{database}: {table} has {count} rows, expected at least {minimum}");
            }
        }
    }

    let msime = crate::sqlite::open_read_only(&out.join("msime-pinyin.db"))?;
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
    verify_notices(out)
}

/// The licence notices the data's terms require beside it: Mozc's dictionary README (IPAdic, ICOT, Okinawa) and Mozc's BSD `LICENSE` for `msime-japanese.dat`, and SCOWL's copyright notice, byte for byte the text `msime-english.db` stores, for the SCOWL words in `msime-english.db`.
fn verify_notices(out: &Path) -> Result<()> {
    let required: [(&str, &str, &[&str]); 2] = [
        (
            japanese::NOTICE_NAME,
            "msime-japanese.dat",
            &["ipadic", "icot", "okinawa"],
        ),
        (
            japanese::LICENSE_NAME,
            "msime-japanese.dat",
            &[
                "google inc.",
                "redistribution and use in source and binary forms",
            ],
        ),
    ];
    for (name, data, terms) in required {
        let path = out.join(name);
        let notice = std::fs::read_to_string(&path)
            .with_context(|| format!("{name}: {data} must not ship without it"))?
            .to_lowercase();
        for term in terms {
            if !notice.contains(term) {
                bail!("{name} does not mention {term}; {data} must not ship without it");
            }
        }
    }
    let name = english_supplement::NOTICE_NAME;
    let notice = std::fs::read(out.join(name))
        .with_context(|| format!("{name}: msime-english.db must not ship without it"))?;
    if notice != english_supplement::COPYRIGHT.as_bytes() {
        bail!("{name} differs from resources/licenses/scowl-aspell6-en-Copyright.txt, the notice msime-english.db's SCOWL words ship under");
    }
    Ok(())
}

/// 写出 manifest 和校验和。`dictionary` 是读取 msime-dictionary 文件的 `--dictionary` checkout，manifest 把它的 HEAD 记为 msime-dictionary 的提交；其余 references 和 `mozc_revision` 原样取自锁文件。`Dictionary::open` 只核对了 checkout 的 `upstream.lock.json` 也列出的上游（`mozc_revision` 对应其中的 `mozc`）：它们的 repository 和 commit 与锁文件相同；记录里没有的 reference（如 `ECDICT`）没有和任何东西比对。
pub fn write_manifest(
    out: &Path,
    repository: &Path,
    dictionary: &Path,
    lock: &Lock,
    complete: bool,
) -> Result<()> {
    let source = provenance(repository, Some(dictionary))?;
    let custom_dictionary = custom_dictionary_reference(dictionary)?;
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
        references: {
            let mut references: BTreeMap<String, Reference> = lock.references.clone();
            references.insert(CUSTOM_DICTIONARY.to_owned(), custom_dictionary.clone());
            references.into_iter().collect()
        },
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
    std::fs::write(out.join(SUMS), sums)?;
    Ok(())
}

/// 删掉 `out` 里已有的 manifest 和校验和文件，返回删掉了哪些。这次构建不写 manifest 时调用：`--out` 里的数据库已经被重新构建或冻结，留下的旧文件会写着不再相符的大小和 SHA-256。
pub fn remove_stale_manifest(out: &Path) -> Result<Vec<&'static str>> {
    let mut removed = Vec::new();
    for name in [MANIFEST, SUMS] {
        let path = out.join(name);
        match std::fs::remove_file(&path) {
            Ok(()) => removed.push(name),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => {
                return Err(error).with_context(|| format!("removing {}", path.display()))
            }
        }
    }
    Ok(removed)
}

#[cfg(test)]
mod tests {
    use super::*;

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

    /// manifest 记下 `--dictionary` checkout 的 HEAD，repository 由常量拼出，不读锁文件；checkout 的 `sources/`、`custom/` 或 `upstream.lock.json` 有未提交改动时构建记为 dirty。
    #[test]
    fn a_dictionary_checkout_is_the_recorded_provenance() {
        let msime = tempfile::tempdir().unwrap();
        let checkout = tempfile::tempdir().unwrap();
        git_repository_with_commit(msime.path());
        let head = git_repository_with_commit(checkout.path());

        let reference = custom_dictionary_reference(checkout.path()).unwrap();
        assert_eq!(reference.commit, head);
        assert_eq!(
            reference.repository,
            "https://github.com/metasequoiaime/msime-dictionary.git"
        );

        assert!(
            !provenance(msime.path(), Some(checkout.path()))
                .unwrap()
                .dirty
        );
        std::fs::write(
            checkout.path().join(crate::sources::UPSTREAM_LOCK),
            "{\"version\": 1}\n",
        )
        .unwrap();
        assert!(
            provenance(msime.path(), Some(checkout.path()))
                .unwrap()
                .dirty
        );
        std::fs::remove_file(checkout.path().join(crate::sources::UPSTREAM_LOCK)).unwrap();
        std::fs::write(checkout.path().join("custom/words.txt"), "changed\n").unwrap();
        assert!(
            provenance(msime.path(), Some(checkout.path()))
                .unwrap()
                .dirty
        );
        assert!(!provenance(msime.path(), None).unwrap().dirty);
        let msime_head = git(msime.path(), &["rev-parse", "HEAD"]).unwrap();
        assert_eq!(builder_commit(msime.path()).unwrap(), msime_head);
        std::fs::create_dir_all(msime.path().join("crates/dict-builder")).unwrap();
        std::fs::write(msime.path().join("crates/dict-builder/new.rs"), "\n").unwrap();
        assert_eq!(
            builder_commit(msime.path()).unwrap(),
            format!("{msime_head}-dirty")
        );
    }

    /// 不写 manifest 的构建删掉旧的 manifest 和校验和，不碰别的文件；两者本来就不存在时什么也不做。
    #[test]
    fn a_build_without_a_manifest_removes_the_stale_one() {
        let out = tempfile::tempdir().unwrap();
        for name in [MANIFEST, SUMS, "msime-others.db"] {
            std::fs::write(out.path().join(name), b"old").unwrap();
        }
        assert_eq!(remove_stale_manifest(out.path()).unwrap(), [MANIFEST, SUMS]);
        assert!(!out.path().join(MANIFEST).exists());
        assert!(!out.path().join(SUMS).exists());
        assert!(out.path().join("msime-others.db").is_file());
        assert!(remove_stale_manifest(out.path()).unwrap().is_empty());
    }

    /// A release missing either Mozc notice or SCOWL's, or carrying a SCOWL notice that is not the committed text, fails product verification.
    #[test]
    fn product_verification_requires_every_licence_notice() {
        let directory = tempfile::tempdir().unwrap();
        let out = directory.path();
        let write = |name: &str, text: &str| std::fs::write(out.join(name), text).unwrap();
        write(
            japanese::NOTICE_NAME,
            "IPAdic ... ICOT Free Software ... Okinawa dictionary",
        );
        write(
            japanese::LICENSE_NAME,
            "Copyright 2010-2018, Google Inc.\nRedistribution and use in source and binary forms ...",
        );
        write(
            english_supplement::NOTICE_NAME,
            english_supplement::COPYRIGHT,
        );
        verify_notices(out).unwrap();

        write(english_supplement::NOTICE_NAME, "SCOWL\n");
        let error = verify_notices(out).unwrap_err().to_string();
        assert!(error.contains(english_supplement::NOTICE_NAME), "{error}");
        std::fs::remove_file(out.join(english_supplement::NOTICE_NAME)).unwrap();
        let error = verify_notices(out).unwrap_err().to_string();
        assert!(error.contains(english_supplement::NOTICE_NAME), "{error}");
        write(
            english_supplement::NOTICE_NAME,
            english_supplement::COPYRIGHT,
        );

        std::fs::remove_file(out.join(japanese::LICENSE_NAME)).unwrap();
        let error = verify_notices(out).unwrap_err().to_string();
        assert!(error.contains(japanese::LICENSE_NAME), "{error}");
        write(japanese::LICENSE_NAME, "Copyright 2010-2018, Google Inc.\n");
        let error = verify_notices(out).unwrap_err().to_string();
        assert!(error.contains("redistribution"), "{error}");
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
