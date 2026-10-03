//! 产品版本（edition）。
//!
//! 版本表 `shared/contracts/editions.json` 是各版本的单一事实源：每个版本提供哪些输入方案、默认方案是什么、在 `Preferences::default()` 之上叠加哪些默认值、随包带哪些资源和功能。full 是现有产品本身，所有值都等于今天写死在代码里的那个；其他版本是它的收窄。多个版本可以同时安装，彼此完全隔离，因此这里只描述一个版本自己的样子，不涉及版本之间的共享。
//!
//! 版本表在编译期嵌入，结构由本模块的类型解析，跨字段和跨文件的约束（资源组件与锁文件一致、功能依赖的组件、冻结基线等）由 `scripts/test-editions.py` 检查。各平台的身份标识（`platforms` 段）由平台构建脚本读取，本模块不解析。

use crate::preferences::InputScheme;
use serde::Deserialize;
use std::sync::OnceLock;

const EDITIONS_JSON: &str = include_str!("../../../shared/contracts/editions.json");

/// 本模块认识的版本表格式版本。
const SCHEMA_VERSION: u32 = 1;

/// 一个产品版本。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct Edition {
    /// 版本 id，例如 `full`、`pinyin`、`wubi`。写入后不再改名。
    pub id: String,
    /// 系统输入法列表和设置应用里显示的产品名。所有版本共用同一个图标。
    pub display_name: DisplayName,
    /// 本版本提供的输入方案，顺序与 `host_surface::compiled_input_schemes()` 一致。不在列表里的方案在本版本中不存在。
    pub input_schemes: Vec<InputScheme>,
    /// 默认方案，也是偏好里的方案不在 `input_schemes` 里时的回退值。
    pub default_scheme: InputScheme,
    /// 叠加在 `Preferences::default()` 之上的版本默认值，用户之后仍可修改。
    pub preference_defaults: PreferenceDefaults,
    /// 随包带的资源。
    pub resources: EditionResources,
    /// 本版本是否提供这些可选功能。
    pub features: EditionFeatures,
    /// 本版本需要的语言词库，取值是 `resources/language-dictionaries.lock.json` 里的条目名。
    pub language_dictionaries: Vec<String>,
}

/// 版本的产品名。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DisplayName {
    #[serde(rename = "zh-Hans")]
    pub zh_hans: String,
    pub en: String,
}

/// 版本默认值。`None` 表示沿用 `Preferences::default()`。
///
/// 拒绝未知字段：拼错的键如果被静默忽略，这个版本就会带着错误的默认值发出去。
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PreferenceDefaults {
    /// 五笔混拼的默认值。
    #[serde(default)]
    pub wubi_mixed_pinyin: Option<bool>,
}

/// 版本随包带的资源。
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditionResources {
    /// 版本表 `resource_components` 里的组件名。
    pub components: Vec<String>,
}

/// 版本提供的可选功能。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EditionFeatures {
    /// 临时日文。
    pub temporary_japanese: bool,
    /// 神经网络整句重排。
    pub neural_keyboard: bool,
}

#[derive(Deserialize)]
struct EditionTable {
    schema_version: u32,
    editions: Vec<Edition>,
}

fn editions() -> &'static [Edition] {
    static EDITIONS: OnceLock<Vec<Edition>> = OnceLock::new();
    EDITIONS.get_or_init(|| {
        let table: EditionTable =
            serde_json::from_str(EDITIONS_JSON).expect("shared/contracts/editions.json is valid");
        assert_eq!(
            table.schema_version, SCHEMA_VERSION,
            "shared/contracts/editions.json schema_version"
        );
        table.editions
    })
}

impl Edition {
    /// full 版本的 id。没有任何版本信息的文档和调用方都按 full 处理。
    pub const FULL_ID: &'static str = "full";

    /// 全部版本，第一个是 full。
    pub fn all() -> &'static [Edition] {
        editions()
    }

    /// 按 id 查找版本；版本表里没有这个 id 时返回 `None`。
    pub fn by_id(id: &str) -> Option<&'static Edition> {
        editions().iter().find(|edition| edition.id == id)
    }

    /// full 版本，即现有产品本身。
    pub fn full() -> &'static Edition {
        Self::by_id(Self::FULL_ID).expect("shared/contracts/editions.json defines the full edition")
    }

    /// 本版本是否提供这个方案。
    pub fn offers(&self, scheme: InputScheme) -> bool {
        self.input_schemes.contains(&scheme)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::host_surface::compiled_input_schemes;
    use crate::preferences::Preferences;
    use std::collections::BTreeSet;

    #[test]
    fn the_table_parses_and_full_comes_first() {
        let ids: Vec<&str> = Edition::all()
            .iter()
            .map(|edition| edition.id.as_str())
            .collect();
        assert_eq!(ids, ["full", "pinyin", "wubi"]);
        assert_eq!(ids.iter().collect::<BTreeSet<_>>().len(), ids.len());
    }

    #[test]
    fn full_matches_the_product_as_it_is_today() {
        let full = Edition::full();
        assert_eq!(full.id, Edition::FULL_ID);
        assert_eq!(full.input_schemes, compiled_input_schemes());
        assert_eq!(full.default_scheme, InputScheme::Quanpin);
        assert_eq!(full.default_scheme, Preferences::default().scheme);
        assert_eq!(full.preference_defaults, PreferenceDefaults::default());
        assert_eq!(
            full.features,
            EditionFeatures {
                temporary_japanese: true,
                neural_keyboard: true
            }
        );
        assert_eq!(full.display_name.zh_hans, "水杉输入法");
        assert_eq!(full.display_name.en, "MSIME");
    }

    #[test]
    fn pinyin_offers_quanpin_and_shuangpin_with_temporary_japanese() {
        let pinyin = Edition::by_id("pinyin").unwrap();
        assert_eq!(
            pinyin.input_schemes,
            [InputScheme::Quanpin, InputScheme::Shuangpin]
        );
        assert_eq!(pinyin.default_scheme, InputScheme::Quanpin);
        assert!(pinyin.features.temporary_japanese);
        assert_eq!(pinyin.display_name.zh_hans, "水杉拼音");
        assert_eq!(pinyin.display_name.en, "MSIME Pinyin");
    }

    #[test]
    fn wubi_offers_only_wubi_with_mixed_pinyin_on_by_default() {
        let wubi = Edition::by_id("wubi").unwrap();
        assert_eq!(wubi.input_schemes, [InputScheme::Wubi]);
        assert_eq!(wubi.default_scheme, InputScheme::Wubi);
        assert_eq!(wubi.preference_defaults.wubi_mixed_pinyin, Some(true));
        assert!(!wubi.features.temporary_japanese);
        assert!(wubi.offers(InputScheme::Wubi));
        assert!(!wubi.offers(InputScheme::Quanpin));
        assert_eq!(wubi.display_name.zh_hans, "水杉五笔");
        assert_eq!(wubi.display_name.en, "MSIME Wubi");
    }

    #[test]
    fn every_edition_defaults_to_a_scheme_it_offers() {
        for edition in Edition::all() {
            assert!(edition.offers(edition.default_scheme), "{}", edition.id);
            assert!(
                edition
                    .input_schemes
                    .iter()
                    .all(|scheme| compiled_input_schemes().contains(scheme)),
                "{}",
                edition.id
            );
        }
    }

    #[test]
    fn an_unknown_id_is_not_an_edition() {
        assert!(Edition::by_id("unknown").is_none());
        assert!(Edition::by_id("").is_none());
    }

    #[test]
    fn a_misspelled_preference_default_is_rejected() {
        let error = serde_json::from_str::<PreferenceDefaults>(r#"{"wubi_mixed_pinyn": true}"#)
            .unwrap_err();
        assert!(error.to_string().contains("wubi_mixed_pinyn"), "{error}");
    }
}
