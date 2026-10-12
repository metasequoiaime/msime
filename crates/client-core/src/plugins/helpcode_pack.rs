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
//!
//! 移动端没有用户可见的资源目录，用户手里只有一个 `.txt` 码表：[`import_table`] 按同样的严格规则校验它，包成一个 `custom-` 开头的辅助码表包，再经 [`super::import`] 安装，同名文件再次导入时整包替换。

use serde::Serialize;
use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::path::Path;
use toml::Value;

use super::{
    data_lines, only_keys, read_file, valid_file_name, PluginContent, PluginError, PluginKind,
    PluginSummary,
};

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
    let count = entries.len();
    let mut preview = Vec::with_capacity(PREVIEW_ENTRIES.min(count));
    preview.extend(entries.into_iter().take(PREVIEW_ENTRIES));
    Ok(HelpcodePack {
        table: name.to_owned(),
        entries: count,
        preview,
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
    // 每个字第一次出现的行号，重复时一并报出两行。
    let mut seen = HashMap::new();
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
        if entries.is_empty() {
            // 最短合法行是两字节字符、等号和一个码字母；除最后一行外还要有换行。
            let capacity = (bytes.len().saturating_add(1) / 5).min(MAX_ENTRIES);
            entries.reserve_exact(capacity);
            seen.reserve(capacity);
        }
        if let Some(first) = seen.insert(scalar, number) {
            return Err(format!(
                "{name} 里「{scalar}」出现了不止一次（第 {first} 行和第 {number} 行）"
            ));
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

/// 已安装的辅助码表包 `id` 的整张码表（字到码），按 `scan` 列出包的规则校验过。宿主用它构建 Engine 的辅助码表；包缺失或载入失败时返回原因，宿主据此退回方案原来的 `schema`。读一个最大 1 MiB 的文件，只解析一遍：不要在按键路径上调用。
pub fn load_codes(root: &Path, id: &str) -> Result<HashMap<String, String>, String> {
    let package = super::load_package_unread(root, PluginKind::Helpcode, id)?;
    let PluginContent::Helpcode(pack) = package.content else {
        unreachable!("load_package_unread checks the kind");
    };
    Ok(read_entries(&package.directory, &pack.table)?
        .into_iter()
        .map(|entry| (entry.character, entry.code))
        .collect())
}

/// [`import_table`] 生成的包 id 的前缀，与社区包的 id 分开。
pub const IMPORTED_ID_PREFIX: &str = "custom-";
/// 清单 `name` 的字节上限（`plugin.toml` 的规则）。
const MAX_NAME_BYTES: usize = 80;
/// 清单 `description` 的字节上限（`plugin.toml` 的规则）。
const MAX_DESCRIPTION_BYTES: usize = 500;
/// 包里码表文件的名字。
const IMPORTED_TABLE_FILE: &str = "table.txt";

/// [`import_table`] 写出的 `plugin.toml`。经 toml 序列化而不是拼字符串：文件名和 `# name:` 里可能有引号、反斜杠。
#[derive(Serialize)]
struct ImportedManifest<'a> {
    schema_version: i64,
    kind: &'static str,
    id: &'a str,
    name: &'a str,
    version: &'static str,
    license: &'static str,
    description: &'a str,
    helpcode: ImportedTable,
}

#[derive(Serialize)]
struct ImportedTable {
    table: &'static str,
}

/// 把用户选的单个 `.txt` 码表装成辅助码表包，返回 `scan` 现在列出的样子。`file_name` 是宿主拿到的文件名（只用于报错、显示名和 id，不当路径用；为空时按 `table.txt`），`bytes` 是文件的原始字节。
///
/// - 码表按模块文档的严格规则校验，不合格时返回 `PluginError::Invalid`，原因带文件名和行号。
/// - 显示名取码表开头的 `# name:`，没有时用去掉扩展名的文件名；在字符边界上截到 80 字节以内。
/// - id 是 `custom-<小写的文件名主干>`，主干不能组成合法 id（含中文、空格，或超过 64 字节）时用 `custom-<小写主干的 SHA-256 前 12 位十六进制>`。所以同名文件再次导入时替换原来的包。
/// - 暂存目录建在 `root` 下而不是系统临时目录（Android 应用写不进 `/data/local/tmp`），用 `.staging-` 前缀，中断后留下的目录由下一次导入按一小时规则清掉；暂存好后经 [`super::import`] 校验、加锁、原子换入。
pub fn import_table(
    root: &Path,
    file_name: &str,
    bytes: &[u8],
) -> Result<PluginSummary, PluginError> {
    let file_name = if file_name.trim().is_empty() {
        IMPORTED_TABLE_FILE
    } else {
        file_name
    };
    if bytes.len() as u64 > MAX_TABLE_BYTES {
        return Err(PluginError::Invalid(format!("{file_name} 超过 1 MiB")));
    }
    parse_table(bytes, file_name).map_err(PluginError::Invalid)?;
    let stem = match file_name.rsplit_once('.') {
        Some((stem, _)) if !stem.trim().is_empty() => stem,
        _ => file_name,
    };
    // `parse_table` 已经确认整个文件是 UTF-8（BOM 也是合法的 UTF-8）。
    let text = std::str::from_utf8(bytes).unwrap_or_default();
    let (declared, _) = crate::helpcode::display_names(text);
    let name = [declared.as_str(), stem]
        .into_iter()
        .map(|candidate| bounded_text(candidate, MAX_NAME_BYTES))
        .find(|candidate| !candidate.is_empty())
        .unwrap_or_else(|| "自定义辅助码".to_owned());
    let id = imported_id(stem);
    let description = bounded_text(&format!("从 {file_name} 导入"), MAX_DESCRIPTION_BYTES);
    let manifest = toml::to_string(&ImportedManifest {
        schema_version: 1,
        kind: PluginKind::Helpcode.as_str(),
        id: &id,
        name: &name,
        version: "1",
        license: "NOASSERTION",
        description: &description,
        helpcode: ImportedTable {
            table: IMPORTED_TABLE_FILE,
        },
    })
    .map_err(|error| PluginError::Invalid(format!("plugin.toml 无法生成：{error}")))?;
    if !crate::storage::create_directory_and_check(root).map_err(|_| PluginError::Storage)? {
        return Err(PluginError::Storage);
    }
    let staging = tempfile::Builder::new()
        .prefix(".staging-")
        .tempdir_in(root)?;
    std::fs::write(staging.path().join(super::MANIFEST_FILE), manifest)?;
    std::fs::write(staging.path().join(IMPORTED_TABLE_FILE), bytes)?;
    super::import(staging.path(), root)
}

/// 导入的码表的包 id，规则见 [`import_table`]。
fn imported_id(stem: &str) -> String {
    let lower = stem.to_lowercase();
    let readable = format!("{IMPORTED_ID_PREFIX}{lower}");
    if crate::skin::catalog::safe_id(&readable) {
        return readable;
    }
    let digest = hex::encode(Sha256::digest(lower.as_bytes()));
    format!("{IMPORTED_ID_PREFIX}{}", &digest[..12])
}

/// 去掉控制字符和首尾空白，再在字符边界上截到 `max` 字节以内。
fn bounded_text(value: &str, max: usize) -> String {
    let clean: String = value
        .chars()
        .filter(|character| !character.is_control())
        .collect();
    let clean = clean.trim();
    let mut end = clean.len().min(max);
    while !clean.is_char_boundary(end) {
        end -= 1;
    }
    clean[..end].trim_end().to_owned()
}
