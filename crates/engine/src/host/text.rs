//! Thin string-path wrappers the bridge exposed over engine helpers.

use std::path::Path;

use super::options::{runtime_paths, EngineOptions};
use crate::assets;
use crate::error::Result;
use crate::local::catalog::{self, EmojiCatalogItem, EmojiCatalogSlice, EmojiSymbolGroup};
use crate::shuangpin::hints::ShuangpinKeyHint;

/// Over the generation's working `msime-pinyin.db`.
pub fn hanzi_to_pinyin(options: &EngineOptions, text: &str) -> String {
    crate::dictionary::hanzi::hanzi_to_pinyin(
        &runtime_paths(options).dictionary(assets::MAIN_DICTIONARY),
        text,
    )
}

/// 少按键统计用：按会话的方案，把上屏文字用拼音逐字打出来要按的键数。只有全拼（含九键）和双拼有答案，其他方案和查不到读音的文字为 `None`。
pub fn canonical_spelling_keys(options: &EngineOptions, text: &str) -> Option<u32> {
    let double_pinyin = match crate::types::SchemeType::from_u8(options.scheme)? {
        crate::types::SchemeType::Quanpin => false,
        crate::types::SchemeType::Shuangpin => true,
        _ => return None,
    };
    crate::dictionary::hanzi::spelling_keys(
        &runtime_paths(options).dictionary(assets::MAIN_DICTIONARY),
        text,
        double_pinyin,
    )
}

pub fn normalize_full_pinyin(input: &str, expected_syllables: usize) -> String {
    crate::pinyin::normalize::normalize_full_pinyin(input, expected_syllables)
}

pub fn shuangpin_key_hints(profile: &str) -> Vec<ShuangpinKeyHint> {
    crate::shuangpin::hints::shuangpin_key_hints(profile)
}

pub fn shuangpin_zero_initials(profile: &str) -> Vec<(&'static str, &'static str)> {
    crate::shuangpin::hints::shuangpin_zero_initials(profile)
}

/// Deduplicated items of one page.
pub fn emoji_catalog_filtered_page(
    resources: &str,
    search: &str,
    category: &str,
    group: &str,
    offset: usize,
    limit: u16,
    parent: &str,
) -> Result<Vec<EmojiCatalogItem>> {
    let slice = catalog::read_emoji_catalog_slice(
        Path::new(resources),
        search,
        category,
        group,
        offset,
        usize::from(limit),
        parent,
        true,
    )?;
    Ok(slice.items)
}

pub fn emoji_catalog_slice(
    resources: &str,
    search: &str,
    category: &str,
    group: &str,
    offset: usize,
    limit: u16,
    parent: &str,
) -> Result<EmojiCatalogSlice> {
    catalog::read_emoji_catalog_slice(
        Path::new(resources),
        search,
        category,
        group,
        offset,
        usize::from(limit),
        parent,
        false,
    )
}

pub fn emoji_symbol_groups(resources: &str) -> Result<Vec<EmojiSymbolGroup>> {
    catalog::emoji_symbol_groups(Path::new(resources))
}

pub fn emoji_catalog_groups(resources: &str, category: &str) -> Result<Vec<String>> {
    catalog::emoji_catalog_groups(Path::new(resources), category)
}

/// The bridge returned a `Result` here and callers use `.ok()`; ordering itself cannot fail.
pub fn handwriting_order_candidates(candidates: &[String]) -> Result<Vec<String>> {
    Ok(crate::handwriting::order_handwriting_candidates(candidates))
}
