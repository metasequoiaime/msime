//! 设置页对只读语言资源的分页浏览，不经过个人词库日志或输入会话。

use std::path::Path;

use super::EngineOptions;
use crate::dictionary::DictRow;
use crate::error::{EngineError, Result};

#[derive(Clone, Copy, Debug)]
pub enum ResourceDictionaryKind {
    Japanese,
    Cantonese,
    Zhuyin,
    Stroke,
    KoreanHanja,
}

pub struct ResourceDictionaryPage {
    pub available: bool,
    pub entries: Vec<DictRow>,
    pub has_more: bool,
}

/// 空查询列出全部，非空查询匹配编码前缀或词条文字。日语的 `weight` 是模型词成本，越低越优先；韩语汉字表没有权重。
pub fn dictionary_resource_entries(
    options: &EngineOptions,
    kind: ResourceDictionaryKind,
    query: &str,
    offset: usize,
    limit: usize,
) -> Result<ResourceDictionaryPage> {
    if !(1..=1000).contains(&limit) || offset > 1_000_000 || query.len() > 256 {
        return Err(EngineError::invalid("invalid dictionary page"));
    }
    let query = query.trim();
    let resource = |configured: &str, name: &str| {
        if configured.is_empty() {
            Path::new(&options.resources).join(name)
        } else {
            Path::new(configured).to_owned()
        }
    };
    let entries = match kind {
        ResourceDictionaryKind::Japanese => {
            let path = resource(&options.japanese_dictionary, "msime-japanese.dat");
            if !path.is_file() {
                None
            } else {
                let dictionary = crate::japanese::decoder::JapaneseDictionary::shared(&path)
                    .ok_or_else(|| EngineError::failed("dictionary read rejected"))?;
                Some(dictionary.browse(query, offset, limit + 1))
            }
        }
        ResourceDictionaryKind::KoreanHanja => {
            Some(crate::korean::hanja::browse(query, offset, limit + 1))
        }
        _ => {
            let (configured, name) = match kind {
                ResourceDictionaryKind::Cantonese => {
                    (&options.cantonese_dictionary, "msime-cantonese.db")
                }
                ResourceDictionaryKind::Zhuyin => (&options.zhuyin_dictionary, "msime-zhuyin.db"),
                ResourceDictionaryKind::Stroke => (&options.stroke_dictionary, "msime-stroke.db"),
                _ => unreachable!(),
            };
            let path = resource(configured, name);
            if !path.is_file() {
                None
            } else {
                Some(crate::language_dictionary::open_read_only(&path)?.browse(
                    query,
                    offset,
                    limit + 1,
                )?)
            }
        }
    };
    let available = entries.is_some();
    let mut entries = entries.unwrap_or_default();
    let has_more = entries.len() > limit;
    entries.truncate(limit);
    Ok(ResourceDictionaryPage {
        available,
        entries,
        has_more,
    })
}
