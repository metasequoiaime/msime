//! `web` 子命令：从词库 release 的 `msime-pinyin.db`（全拼表与 `quick_parases`）和 `msime-wubi.db`（`wubi86`、`wubi98`）裁出网页内置输入法用的两个词库。两个输入先在临时副本里合成拆分前那种单个主库的布局，再按下面的规则裁剪，所以输出的表结构和拆分前一样。
//!
//! - `msime-pinyin.db`：全拼与双拼共用。保留全部单字表 `tbl_1_*`，多字表 `tbl_{2..7,others}_*` 只保留全局按权重排名前 N 行，清空 `wubi86`、`wubi98` 和 `quick_parases`。
//! - `msime-wubi86.db`：只保留 `wubi86`，清空全部全拼表、`wubi98` 和 `quick_parases`。
//!
//! 两个库都保留全部表结构和索引，被清空的表查询时返回空结果而不是报错。输出逐字节可复现：同一个输入跑两次得到相同的 sha256。
//!
//! - `msime-japanese.dat`（给了 `--japanese` 时）：词库 release 的日语模型只保留词条成本最低的 N 条（成本相同按文件里的先后），顺序和连接矩阵不变，字符串表按保留的词条重新写。完整的模型有约 128 万条、66 MB，网页上下载和常驻内存都太大；整句转换缺词时仍有假名兜底。

use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

use anyhow::{bail, Context, Result};
use msime_engine::format::{quanpin_table, SHIPPED_INITIALS};
use rusqlite::types::Value;
use rusqlite::Connection;

use crate::japanese;
use crate::msime::quanpin_tables;
use crate::sqlite;

pub const PINYIN: &str = "msime-pinyin.db";
pub const WUBI86: &str = "msime-wubi86.db";
pub const JAPANESE: &str = msime_engine::assets::JAPANESE_MODEL;

/// 日语模型默认保留的词条数：gzip 后约 5.7 MB，和拼音库同一量级。
pub const DEFAULT_KEEP_JAPANESE: usize = 250_000;

/// 默认保留的多字词行数，对应评测里的 d200000。
pub const DEFAULT_KEEP_MULTI: usize = 200_000;

/// 除全拼表以外，裁剪时认识的表；遇到其它表直接失败，免得把不认识的数据原样带进网页词库。
const WUBI86_TABLE: &str = "wubi86";
const WUBI98_TABLE: &str = "wubi98";
const EMPTIED_TABLES: [&str; 2] = [WUBI98_TABLE, "quick_parases"];

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

/// 两个输入库，都只读打开。
#[derive(Clone, Copy)]
pub struct Inputs<'a> {
    /// release 的 `msime-pinyin.db`。
    pub pinyin: &'a Path,
    /// release 的 `msime-wubi.db`。
    pub wubi: &'a Path,
}

pub fn build(inputs: Inputs<'_>, out_dir: &Path, keep_multi: usize) -> Result<Vec<Summary>> {
    fs::create_dir_all(out_dir).with_context(|| format!("creating {}", out_dir.display()))?;
    Ok(vec![
        write(inputs, &out_dir.join(PINYIN), Flavour::Pinyin, keep_multi)?,
        write(inputs, &out_dir.join(WUBI86), Flavour::Wubi86, keep_multi)?,
    ])
}

/// 裁出的日语模型的统计，供命令行打印。
pub struct JapaneseSummary {
    pub path: PathBuf,
    pub kept: usize,
    pub tokens: usize,
    pub bytes: u64,
}

impl std::fmt::Display for JapaneseSummary {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            formatter,
            "{}: {} of {} tokens, {} bytes",
            self.path.display(),
            self.kept,
            self.tokens,
            self.bytes
        )
    }
}

/// 从词库 release 的 `msime-japanese.dat` 裁出网页用的那份，写到 `out_dir`。
pub fn build_japanese(input: &Path, out_dir: &Path, keep: usize) -> Result<JapaneseSummary> {
    fs::create_dir_all(out_dir).with_context(|| format!("creating {}", out_dir.display()))?;
    let bytes = crate::sources::read_private(input)
        .with_context(|| format!("reading {}", input.display()))?;
    let (tokens, size, costs) =
        japanese::unpack(&bytes).with_context(|| format!("reading {}", input.display()))?;
    let kept = keep_cheapest(&tokens, keep);
    let out = out_dir.join(JAPANESE);
    let packed = japanese::pack(&kept, size, &costs)?;
    japanese::write_model(&out, &packed)?;
    Ok(JapaneseSummary {
        path: out,
        kept: kept.len(),
        tokens: tokens.len(),
        bytes: packed.len() as u64,
    })
}

/// 成本最低的 `keep` 条词，成本相同时先到先留，按原来的顺序（读法有序，解码器靠它二分）返回。
fn keep_cheapest(tokens: &[japanese::Token], keep: usize) -> Vec<japanese::Token> {
    let mut order: Vec<usize> = (0..tokens.len()).collect();
    order.sort_by_key(|&index| (tokens[index].cost, index));
    order.truncate(keep);
    order.sort_unstable();
    order
        .into_iter()
        .map(|index| tokens[index].clone())
        .collect()
}

fn write(inputs: Inputs<'_>, out: &Path, flavour: Flavour, keep_multi: usize) -> Result<Summary> {
    let mut scratch = out.as_os_str().to_owned();
    scratch.push(".scratch");
    let scratch = PathBuf::from(scratch);
    // `VACUUM INTO` 拒绝写入已有内容的文件，上一次运行留下的产物先删掉。
    remove_if_present(&scratch)?;
    remove_if_present(out)?;

    let source = open_read_only(inputs.pinyin)?;
    vacuum_into(&source, &scratch)?;
    drop(source);

    let mut connection = sqlite::open(&scratch)?;
    merge_wubi(&mut connection, inputs.wubi)?;
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

    let output = sqlite::open_read_only(out)?;
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

fn open_read_only(path: &Path) -> Result<Connection> {
    sqlite::open_read_only(path).with_context(|| format!("opening {}", path.display()))
}

/// 把 `msime-wubi.db` 的表和索引原样复制进 `msime-pinyin.db` 的临时副本，得到拆分前单个主库的布局。行按 rowid 顺序复制：运行时反查五笔编码以 rowid 作最后的排序键。`msime-wubi.db` 里只能有 `wubi86`、`wubi98`（和 SQLite 自己的统计表），拼音库里也不能已有同名表，否则说明两个输入给反了或不是拆分后的 release。
fn merge_wubi(connection: &mut Connection, wubi: &Path) -> Result<()> {
    let source = open_read_only(wubi)?;
    let mut statement = source.prepare(
        "SELECT type, name, sql FROM sqlite_master WHERE sql IS NOT NULL AND name NOT LIKE 'sqlite_%' ORDER BY type = 'index', name",
    )?;
    let schema = statement
        .query_map([], |row| {
            Ok((
                row.get::<_, String>(0)?,
                row.get::<_, String>(1)?,
                row.get::<_, String>(2)?,
            ))
        })?
        .collect::<rusqlite::Result<Vec<_>>>()?;
    drop(statement);
    if !schema
        .iter()
        .any(|(kind, name, _)| kind == "table" && name == WUBI86_TABLE)
    {
        bail!("{}: no wubi86 table", wubi.display());
    }
    let transaction = connection.transaction()?;
    for (kind, name, sql) in &schema {
        if kind == "table" {
            if name != WUBI86_TABLE && name != WUBI98_TABLE {
                bail!("{}: unexpected table {name}", wubi.display());
            }
            let present: bool = transaction.query_row(
                "SELECT EXISTS(SELECT 1 FROM sqlite_master WHERE name = ?1)",
                [name],
                |row| row.get(0),
            )?;
            if present {
                bail!("the pinyin dictionary already has {name}; pass the msime-pinyin.db and msime-wubi.db of a split release");
            }
        }
        transaction.execute_batch(sql)?;
        if kind == "table" {
            copy_rows(&source, &transaction, name)?;
        }
    }
    transaction.commit()?;
    Ok(())
}

fn copy_rows(source: &Connection, target: &Connection, table: &str) -> Result<()> {
    let mut select = source.prepare(&format!("SELECT * FROM \"{table}\" ORDER BY rowid"))?;
    let columns = select.column_count();
    let placeholders = vec!["?"; columns].join(", ");
    let mut insert = target.prepare(&format!("INSERT INTO \"{table}\" VALUES ({placeholders})"))?;
    let mut rows = select.query([])?;
    while let Some(row) = rows.next()? {
        let values = (0..columns)
            .map(|index| row.get::<_, Value>(index))
            .collect::<rusqlite::Result<Vec<_>>>()?;
        insert.execute(rusqlite::params_from_iter(values))?;
    }
    Ok(())
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
        bail!("no single-character quanpin tables in the pinyin dictionary or no wubi86 table in the wubi dictionary");
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

#[cfg(test)]
mod tests {
    use super::build_japanese;

    #[cfg(unix)]
    #[test]
    fn japanese_input_rejects_a_fifo_without_blocking() {
        use std::io::Write;
        use std::os::unix::fs::OpenOptionsExt;
        use std::sync::mpsc;
        use std::time::Duration;

        let directory = tempfile::tempdir().unwrap();
        let input = directory.path().join("msime-japanese.dat");
        let output = directory.path().join("web");
        assert!(std::process::Command::new("mkfifo")
            .arg(&input)
            .status()
            .unwrap()
            .success());

        let (done, result) = mpsc::channel();
        let worker_input = input.clone();
        let worker_output = output.clone();
        let worker = std::thread::spawn(move || {
            done.send(build_japanese(&worker_input, &worker_output, 1).is_err())
                .unwrap();
        });
        let completed_without_release = match result.recv_timeout(Duration::from_millis(100)) {
            Ok(_) => true,
            Err(mpsc::RecvTimeoutError::Timeout) => {
                let mut writer = std::fs::OpenOptions::new()
                    .write(true)
                    .custom_flags(libc::O_NONBLOCK)
                    .open(&input)
                    .unwrap();
                writer.write_all(b"synthetic-invalid-model").unwrap();
                drop(writer);
                result.recv_timeout(Duration::from_secs(1)).unwrap();
                false
            }
            Err(error) => panic!("Japanese model reader failed to report: {error}"),
        };
        worker.join().unwrap();
        assert!(
            completed_without_release,
            "FIFO Japanese model must be rejected without blocking"
        );
    }
}
