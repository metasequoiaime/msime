//! `web` 子命令：从完整的 `msime.db` 裁出网页内置输入法用的两个词库。
//!
//! - `msime-pinyin.db`：全拼与双拼共用。保留全部单字表 `tbl_1_*`，多字表 `tbl_{2..7,others}_*` 只保留全局按权重排名前 N 行，清空 `wubi86`、`wubi98` 和 `quick_parases`。
//! - `msime-wubi86.db`：只保留 `wubi86`，清空全部全拼表、`wubi98` 和 `quick_parases`。
//!
//! 两个库都保留全部表结构和索引，被清空的表查询时返回空结果而不是报错。输出逐字节可复现：同一个输入跑两次得到相同的 sha256。

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use msime_engine::format::{quanpin_table, SHIPPED_INITIALS};
use rusqlite::{Connection, OpenFlags};

use crate::msime::quanpin_tables;
use crate::sqlite;

pub const PINYIN: &str = "msime-pinyin.db";
pub const WUBI86: &str = "msime-wubi86.db";

/// 默认保留的多字词行数，对应评测里的 d200000。
pub const DEFAULT_KEEP_MULTI: usize = 200_000;

/// 除全拼表以外，裁剪时认识的表；遇到其它表直接失败，免得把不认识的数据原样带进网页词库。
const WUBI86_TABLE: &str = "wubi86";
const EMPTIED_TABLES: [&str; 2] = ["wubi98", "quick_parases"];

#[derive(Clone, Copy, PartialEq, Eq)]
enum Flavour {
    Pinyin,
    Wubi86,
}

/// 一个输出库的统计，供命令行打印。
pub struct Summary {
    pub path: PathBuf,
    pub single_rows: u64,
    pub multi_rows: u64,
    pub wubi86_rows: u64,
    pub bytes: u64,
}

impl std::fmt::Display for Summary {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{}: {} single-character rows, {} multi-character rows, {} wubi86 rows, {} bytes",
            self.path.display(),
            self.single_rows,
            self.multi_rows,
            self.wubi86_rows,
            self.bytes
        )
    }
}

/// 输入库里每张表的归类。
struct Tables {
    single: Vec<String>,
    multi: Vec<String>,
}

pub fn build(input: &Path, out_dir: &Path, keep_multi: usize) -> Result<Vec<Summary>> {
    fs::create_dir_all(out_dir).with_context(|| format!("creating {}", out_dir.display()))?;
    Ok(vec![
        write(input, &out_dir.join(PINYIN), Flavour::Pinyin, keep_multi)?,
        write(input, &out_dir.join(WUBI86), Flavour::Wubi86, keep_multi)?,
    ])
}

fn write(input: &Path, out: &Path, flavour: Flavour, keep_multi: usize) -> Result<Summary> {
    let mut scratch = out.as_os_str().to_owned();
    scratch.push(".scratch");
    let scratch = PathBuf::from(scratch);
    // `VACUUM INTO` 拒绝写入已有内容的文件，上一次运行留下的产物先删掉。
    remove_if_present(&scratch)?;
    remove_if_present(out)?;

    let source = Connection::open_with_flags(input, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .with_context(|| format!("opening {}", input.display()))?;
    vacuum_into(&source, &scratch)?;
    drop(source);

    let mut connection = sqlite::open(&scratch)?;
    let tables = classify(&connection)?;
    let transaction = connection.transaction()?;
    match flavour {
        Flavour::Pinyin => {
            keep_top_multi(&transaction, &tables.multi, keep_multi)?;
            empty(
                &transaction,
                [WUBI86_TABLE].into_iter().chain(EMPTIED_TABLES),
            )?;
        }
        Flavour::Wubi86 => {
            empty(
                &transaction,
                tables
                    .single
                    .iter()
                    .chain(&tables.multi)
                    .map(String::as_str)
                    .chain(EMPTIED_TABLES),
            )?;
        }
    }
    transaction.commit()?;
    sqlite::analyze(&connection, false)?;
    vacuum_into(&connection, out)?;
    drop(connection);
    fs::remove_file(&scratch).with_context(|| format!("removing {}", scratch.display()))?;

    let output = Connection::open_with_flags(out, OpenFlags::SQLITE_OPEN_READ_ONLY)?;
    sqlite::integrity_check(&output)?;
    let mode: String = output.query_row("PRAGMA journal_mode", [], |row| row.get(0))?;
    if !mode.eq_ignore_ascii_case("delete") {
        bail!(
            "{}: expected a rollback-journal database, got journal mode {mode}",
            out.display()
        );
    }
    Ok(Summary {
        path: out.to_owned(),
        single_rows: count(&output, &tables.single)?,
        multi_rows: count(&output, &tables.multi)?,
        wubi86_rows: count(&output, &[WUBI86_TABLE])?,
        bytes: fs::metadata(out)?.len(),
    })
}

fn remove_if_present(path: &Path) -> Result<()> {
    match fs::remove_file(path) {
        Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
            Err(error).with_context(|| format!("removing {}", path.display()))
        }
        _ => Ok(()),
    }
}

fn vacuum_into(connection: &Connection, path: &Path) -> Result<()> {
    let target = path
        .to_str()
        .with_context(|| format!("{} is not valid UTF-8", path.display()))?;
    connection
        .execute("VACUUM INTO ?1", [target])
        .with_context(|| format!("VACUUM INTO {}", path.display()))?;
    Ok(())
}

/// 按名字把表分成单字表和多字表；`wubi86`、`wubi98`、`quick_parases` 和 SQLite 自己的统计表之外，出现任何不认识的表都报错。
fn classify(connection: &Connection) -> Result<Tables> {
    let single_names: HashSet<String> = SHIPPED_INITIALS
        .bytes()
        .filter_map(|initial| quanpin_table(1, initial))
        .collect();
    let quanpin_names: HashSet<String> = quanpin_tables().into_iter().collect();
    let mut statement =
        connection.prepare("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")?;
    let names = statement
        .query_map([], |row| row.get::<_, String>(0))?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    let mut tables = Tables {
        single: Vec::new(),
        multi: Vec::new(),
    };
    let mut wubi86 = false;
    for name in names {
        if single_names.contains(&name) {
            tables.single.push(name);
        } else if quanpin_names.contains(&name) {
            tables.multi.push(name);
        } else if name == WUBI86_TABLE {
            wubi86 = true;
        } else if !EMPTIED_TABLES.contains(&name.as_str()) && !name.starts_with("sqlite_") {
            bail!("unexpected table {name} in the input dictionary");
        }
    }
    if tables.single.is_empty() || !wubi86 {
        bail!("the input is not a msime.db: no single-character quanpin tables or no wubi86 table");
    }
    Ok(tables)
}

/// 在全部多字表里按 `weight DESC, key, value, 表名, rowid` 取前 `keep` 行，其余删除。排序键是全序的，所以同权重的行也总是同一批被保留。
fn keep_top_multi(connection: &Connection, multi: &[String], keep: usize) -> Result<()> {
    if multi.is_empty() {
        return Ok(());
    }
    let union = multi
        .iter()
        .map(|table| {
            format!(
                "SELECT '{table}' AS t, rowid AS r, weight AS w, key AS k, value AS v FROM \"{table}\""
            )
        })
        .collect::<Vec<_>>()
        .join(" UNION ALL ");
    connection.execute_batch(
        "DROP TABLE IF EXISTS temp.web_keep; CREATE TEMP TABLE web_keep (t TEXT NOT NULL, r INTEGER NOT NULL, PRIMARY KEY (t, r)) WITHOUT ROWID;",
    )?;
    let limit = i64::try_from(keep).context("--keep-multi is too large")?;
    connection.execute(
        &format!(
            "INSERT INTO temp.web_keep (t, r) SELECT t, r FROM ({union}) ORDER BY w DESC, k, v, t, r LIMIT ?1"
        ),
        [limit],
    )?;
    for table in multi {
        connection.execute(
            &format!(
                "DELETE FROM \"{table}\" WHERE rowid NOT IN (SELECT r FROM temp.web_keep WHERE t = ?1)"
            ),
            [table],
        )?;
    }
    connection.execute_batch("DROP TABLE temp.web_keep")?;
    Ok(())
}

fn empty<'a>(connection: &Connection, tables: impl IntoIterator<Item = &'a str>) -> Result<()> {
    for table in tables {
        connection.execute(&format!("DELETE FROM \"{table}\""), [])?;
    }
    Ok(())
}

fn count<S: AsRef<str>>(connection: &Connection, tables: &[S]) -> Result<u64> {
    let mut total = 0u64;
    for table in tables {
        let rows: i64 = connection.query_row(
            &format!("SELECT count(*) FROM \"{}\"", table.as_ref()),
            [],
            |row| row.get(0),
        )?;
        total += u64::try_from(rows)?;
    }
    Ok(total)
}
