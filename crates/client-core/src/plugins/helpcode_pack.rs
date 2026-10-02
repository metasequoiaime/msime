//! `kind = "helpcode"`：辅助码表，替换全拼或双拼方案的辅助码。
//!
//! ```toml
//! [helpcode]
//! table = "table.txt"
//! ```
//!
//! `[helpcode]` 只有 `table` 一个键，点名包里的一个 `.txt` 数据文件：1 字节到 1 MiB（Engine 的 `MAX_HELPCODE_BYTES`），UTF-8，可以带 BOM。按 `\n` 分行，每行去掉一个行尾的 `\r`，其余位置的 `\r` 算控制字符。每行是空行、`#` 开头的注释，或者 `<字>=<码>`：`<字>` 恰好是一个非 ASCII、非空白、非控制字符的 Unicode 标量，`<码>` 恰好是 1 到 2 个小写 ASCII 字母，等号两边不能有空格。码更长就拒绝，而不是像 Engine 读内置表那样截断；只有空格的行也拒绝。一张表 1 到 30000 条，同一个字不能出现两次。
//!
//! 包只携带数据：宿主用 [`load_codes`] 读出码表交给 Engine，选中它的方案（`plugins.helpcode_pack_quanpin` / `helpcode_pack_shuangpin`）用它替换原来的 `schema`；包载入失败时宿主退回原来的方案。

use serde::Serialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use toml::Value;

use super::{data_lines, only_keys, read_file, valid_file_name, PluginContent, PluginKind};

pub(crate) const MANIFEST_KEYS: [&str; 1] = ["helpcode"];

/// 码表文件的字节上限，即 Engine 的 `MAX_HELPCODE_BYTES`。
pub const MAX_TABLE_BYTES: u64 = 1024 * 1024;
/// 一张表的条数上限。
pub const MAX_ENTRIES: usize = 30_000;
/// 详情里预览的条数。
pub const PREVIEW_ENTRIES: usize = 8;
/// 码表文件的扩展名。
pub const TABLE_EXTENSION: &str = "txt";

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HelpcodeEntry {
    pub character: String,
    pub code: String,
}

/// 设置页看到的辅助码表：文件名、条数和前几条，不带整张表。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct HelpcodePack {
    pub table: String,
    pub entries: usize,
    pub preview: Vec<HelpcodeEntry>,
}

/// 清单里点名的码表文件名；内容由 [`read`] 在文件检查之后读取。
pub(crate) fn parse(table: &toml::map::Map<String, Value>) -> Result<String, String> {
    let helpcode = table
        .get("helpcode")
        .and_then(Value::as_table)
        .ok_or("辅助码表缺少 helpcode 表")?;
    only_keys(helpcode, &["table"], "helpcode")?;
    let name = helpcode
        .get("table")
        .and_then(Value::as_str)
        .ok_or("helpcode.table 必须是字符串")?;
    if !valid_file_name(name) {
        return Err(format!("helpcode.table 的文件名 {name} 无效"));
    }
    Ok(name.to_owned())
}

/// 读取并严格解析包目录里的码表。
pub(crate) fn read(directory: &Path, name: &str) -> Result<HelpcodePack, String> {
    let entries = read_entries(directory, name)?;
    Ok(HelpcodePack {
        table: name.to_owned(),
        entries: entries.len(),
        preview: entries.into_iter().take(PREVIEW_ENTRIES).collect(),
    })
}

fn read_entries(directory: &Path, name: &str) -> Result<Vec<HelpcodeEntry>, String> {
    let bytes = read_file(directory, name, MAX_TABLE_BYTES)
        .map_err(|_| format!("{name} 无法读取或太大"))?;
    parse_table(&bytes, name)
}

/// 码表文本的严格解析，规则见模块文档。
pub(crate) fn parse_table(bytes: &[u8], name: &str) -> Result<Vec<HelpcodeEntry>, String> {
    let mut entries = Vec::new();
    let mut seen = HashSet::new();
    for (number, line) in data_lines(bytes, name)? {
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        let invalid = || format!("{name} 第 {number} 行不是「字=码」");
        let (character, code) = line.split_once('=').ok_or_else(invalid)?;
        let mut scalars = character.chars();
        let (Some(scalar), None) = (scalars.next(), scalars.next()) else {
            return Err(format!("{name} 第 {number} 行等号左边必须恰好是一个字"));
        };
        if scalar.is_ascii() || scalar.is_whitespace() || scalar.is_control() {
            return Err(format!(
                "{name} 第 {number} 行的字不能是 ASCII、空白或控制字符"
            ));
        }
        if !(1..=2).contains(&code.len()) || !code.bytes().all(|byte| byte.is_ascii_lowercase()) {
            return Err(format!("{name} 第 {number} 行的码必须是 1 到 2 个小写字母"));
        }
        if !seen.insert(scalar) {
            return Err(format!("{name} 里「{scalar}」出现了不止一次"));
        }
        if entries.len() == MAX_ENTRIES {
            return Err(format!("{name} 的条数超过 {MAX_ENTRIES}"));
        }
        entries.push(HelpcodeEntry {
            character: character.to_owned(),
            code: code.to_owned(),
        });
    }
    if entries.is_empty() {
        return Err(format!("{name} 里没有任何辅助码"));
    }
    Ok(entries)
}

/// 已安装的辅助码表包 `id` 的整张码表（字到码），按 `scan` 列出包的规则校验过。宿主用它构建 Engine 的辅助码表；包缺失或载入失败时返回原因，宿主据此退回方案原来的 `schema`。读一个最大 1 MiB 的文件：不要在按键路径上调用。
pub fn load_codes(root: &Path, id: &str) -> Result<HashMap<String, String>, String> {
    let package = super::load_package(root, None, PluginKind::Helpcode, id)?;
    let PluginContent::Helpcode(pack) = package.content else {
        unreachable!("load_package checks the kind");
    };
    Ok(read_entries(&package.directory, &pack.table)?
        .into_iter()
        .map(|entry| (entry.character, entry.code))
        .collect())
}
