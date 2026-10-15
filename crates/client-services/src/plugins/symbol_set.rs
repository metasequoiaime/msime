//! `kind = "symbol_set"`：符号集，给各宿主的符号面板追加符号组或颜文字组，全部写在清单里，没有数据文件。
//!
//! ```toml
//! [[groups]]
//! tab = "symbols"             # "symbols" 或 "kaomoji"
//! title = "箭头"
//! keywords = "jiantou arrow"  # 可选，用于搜索
//! items = ["→", "←", "⇒"]
//! ```
//!
//! 一个包 1 到 32 组，合计最多 2048 项。`title` 必填，1 到 48 字节，非空白、不含控制字符，同一标签页内不重复（不同标签页可以同名）；`keywords` 可选，写了就必须非空白，最多 256 字节，不含控制字符；`items` 只能是字符串，每组 1 到 512 个，每个 1 到 64 个 UTF-16 单元，非空白、不含控制字符，同组内不重复。组里只有这四个键。
//!
//! 没有选择：装上就显示，卸载就消失。宿主把插件组追加在内置组之后：`symbols` 组放在以包名为上级分类的分组下，`kaomoji` 组放在颜文字的 All 之后；不跨包、也不与内置目录去重。

use serde::Serialize;
use std::collections::HashSet;
use std::path::Path;
use toml::Value;

use super::{only_keys, PluginContent, PluginKind, PluginSummary};

pub(crate) const MANIFEST_KEYS: [&str; 1] = ["groups"];

/// 一个包的组数上限。
pub const MAX_GROUPS: usize = 32;
/// 一组的项数上限。
pub const MAX_GROUP_ITEMS: usize = 512;
/// 一个包合计的项数上限。
pub const MAX_ITEMS: usize = 2048;
/// 组标题的字节上限。
pub const MAX_TITLE_BYTES: usize = 48;
/// 搜索关键词的字节上限。
pub const MAX_KEYWORDS_BYTES: usize = 256;
/// 一项的 UTF-16 单元上限。
pub const MAX_ITEM_UTF16: usize = 64;

/// 符号组出现在符号面板的哪个标签页。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum SymbolTab {
    Symbols,
    Kaomoji,
}

impl SymbolTab {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Symbols => "symbols",
            Self::Kaomoji => "kaomoji",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SymbolGroup {
    pub tab: SymbolTab,
    pub title: String,
    /// 没写时为空。
    pub keywords: String,
    pub items: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct SymbolSet {
    pub groups: Vec<SymbolGroup>,
}

pub(crate) fn parse(table: &toml::map::Map<String, Value>) -> Result<SymbolSet, String> {
    let entries = table
        .get("groups")
        .and_then(Value::as_array)
        .ok_or("符号集缺少 groups")?;
    if entries.is_empty() || entries.len() > MAX_GROUPS {
        return Err(format!("符号集必须有 1 到 {MAX_GROUPS} 组"));
    }
    let mut groups = Vec::with_capacity(entries.len());
    let mut total = 0usize;
    // 同一个包里同一标签页的组标题不能重复：宿主用（包、标签页、标题）区分插件组。不同标签页可以同名。
    let mut titles = HashSet::with_capacity(entries.len());
    for (index, entry) in entries.iter().enumerate() {
        let number = index + 1;
        let group = entry.as_table().ok_or("每组符号都必须是一个表")?;
        only_keys(
            group,
            &["tab", "title", "keywords", "items"],
            "a symbol group",
        )?;
        let tab = match group.get("tab").and_then(Value::as_str) {
            Some("symbols") => SymbolTab::Symbols,
            Some("kaomoji") => SymbolTab::Kaomoji,
            _ => return Err(format!("第 {number} 组的 tab 只能是 symbols 或 kaomoji")),
        };
        let title = group
            .get("title")
            .and_then(Value::as_str)
            .ok_or_else(|| format!("第 {number} 组需要字符串 title"))?;
        if title.trim().is_empty() || !crate::text::is_bounded_text(title, MAX_TITLE_BYTES) {
            return Err(format!(
                "第 {number} 组的 title 为空、超过 {MAX_TITLE_BYTES} 字节或含有控制字符"
            ));
        }
        if !titles.insert((tab, title)) {
            return Err(format!(
                "第 {number} 组的 title「{title}」与同一标签页的另一组重复了"
            ));
        }
        let keywords = match group.get("keywords") {
            None => String::new(),
            Some(value) => {
                let keywords = value
                    .as_str()
                    .ok_or_else(|| format!("第 {number} 组的 keywords 必须是字符串"))?;
                if keywords.trim().is_empty()
                    || !crate::text::is_bounded_text(keywords, MAX_KEYWORDS_BYTES)
                {
                    return Err(format!(
                        "第 {number} 组的 keywords 为空、超过 {MAX_KEYWORDS_BYTES} 字节或含有控制字符"
                    ));
                }
                keywords.to_owned()
            }
        };
        let values = group
            .get("items")
            .and_then(Value::as_array)
            .ok_or_else(|| format!("第 {number} 组需要 items"))?;
        if values.is_empty() || values.len() > MAX_GROUP_ITEMS {
            return Err(format!("第 {number} 组必须有 1 到 {MAX_GROUP_ITEMS} 项"));
        }
        total += values.len();
        if total > MAX_ITEMS {
            return Err(format!("符号集合计超过 {MAX_ITEMS} 项"));
        }
        let mut items = Vec::with_capacity(values.len());
        let mut seen = HashSet::with_capacity(values.len());
        for value in values {
            let item = value
                .as_str()
                .ok_or_else(|| format!("第 {number} 组的每一项都必须是字符串"))?;
            if item.trim().is_empty()
                || !crate::text::is_bounded_utf16(item, MAX_ITEM_UTF16)
                || crate::text::has_disallowed_control_with_allowed(item, &[])
            {
                return Err(format!(
                    "第 {number} 组有一项为空、超过 {MAX_ITEM_UTF16} 个 UTF-16 单元或含有控制字符"
                ));
            }
            if !seen.insert(item) {
                return Err(format!("第 {number} 组里「{item}」重复了"));
            }
            items.push(item.to_owned());
        }
        groups.push(SymbolGroup {
            tab,
            title: title.to_owned(),
            keywords,
            items,
        });
    }
    Ok(SymbolSet { groups })
}

/// 宿主符号面板要追加的一组插件符号。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PluginSymbolGroup {
    /// 插件 id。
    pub pack: String,
    /// 插件名：`symbols` 组在面板里的上级分类。
    pub pack_name: String,
    pub tab: SymbolTab,
    pub title: String,
    pub keywords: String,
    pub items: Vec<String>,
}

fn group_capacity(packages: &[PluginSummary]) -> usize {
    packages.iter().fold(0, |capacity, package| {
        capacity.saturating_add(match &package.content {
            PluginContent::SymbolSet(set) => set.groups.len(),
            _ => 0,
        })
    })
}

/// `root` 下每个能载入的符号集的全部组，包按名字和 id 排序，组按清单顺序。载入失败的包不贡献任何组，插件页会报告它。
pub fn plugin_symbol_groups(root: &Path) -> Vec<PluginSymbolGroup> {
    let mut packages = super::scan_kind_packages(root, PluginKind::SymbolSet);
    packages.sort_by(|a, b| (&a.name, &a.id).cmp(&(&b.name, &b.id)));
    let mut groups = Vec::with_capacity(group_capacity(&packages));
    for package in packages {
        let PluginContent::SymbolSet(set) = package.content else {
            continue;
        };
        groups.extend(set.groups.into_iter().map(|group| PluginSymbolGroup {
            pack: package.id.clone(),
            pack_name: package.name.clone(),
            tab: group.tab,
            title: group.title,
            keywords: group.keywords,
            items: group.items,
        }));
    }
    groups
}

#[cfg(test)]
mod tests {
    use super::*;

    fn summary(group_count: usize) -> PluginSummary {
        PluginSummary {
            id: "synthetic".into(),
            name: "Synthetic".into(),
            version: "1".into(),
            license: "CC0-1.0".into(),
            author: None,
            description: None,
            builtin: false,
            directory: Path::new("/synthetic").to_owned(),
            content: PluginContent::SymbolSet(SymbolSet {
                groups: (0..group_count)
                    .map(|index| SymbolGroup {
                        tab: SymbolTab::Symbols,
                        title: format!("group-{index}"),
                        keywords: String::new(),
                        items: vec!["x".into()],
                    })
                    .collect(),
            }),
        }
    }

    #[test]
    fn group_capacity_counts_groups_across_packages() {
        let packages = [summary(2), summary(3)];
        assert_eq!(group_capacity(&packages), 5);
    }
}
