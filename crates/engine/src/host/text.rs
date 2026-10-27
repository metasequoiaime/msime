//! Thin string-path wrappers the bridge exposed over engine helpers.

use std::path::Path;

use super::options::{runtime_paths, EngineOptions};
use crate::assets;
use crate::error::Result;
use crate::local::catalog::{self, EmojiCatalogItem, EmojiCatalogSlice, EmojiSymbolGroup};
use crate::shuangpin::hints::ShuangpinKeyHint;
use crate::types::ShuangpinProfileKind;
use crate::wubi::lookup::WubiCodeLookup;

/// Over the generation's working `msime-pinyin.db`.
pub fn hanzi_to_pinyin(options: &EngineOptions, text: &str) -> String {
    crate::dictionary::hanzi::hanzi_to_pinyin(
        &runtime_paths(options).dictionary(assets::MAIN_DICTIONARY),
        text,
    )
}

/// 一批联系人姓名共用词库连接与整词查询，顺序与输入一致。
pub fn hanzi_to_pinyin_batch(options: &EngineOptions, texts: &[&str]) -> Vec<String> {
    crate::dictionary::hanzi::hanzi_to_pinyin_batch(
        &runtime_paths(options).dictionary(assets::MAIN_DICTIONARY),
        texts,
    )
}

/// 五笔查码：`text` 里每个不同汉字（以及整段作为一个词）在 86 和 98 码表里的编码，读会话所用的同一份五笔码表（`wubi_database` 选出的文件，含用户加的五笔词）。
pub fn wubi_code_lookup(options: &EngineOptions, text: &str) -> WubiCodeLookup {
    let database = crate::ime::registry::wubi_database(&runtime_paths(options));
    // 下载的码表（`EngineOptions::wubi_dictionary`）要等下一次建会话才并进代次；在那之前查码直接读下载的那份，查码本来就只读码表。
    let database = if !options.wubi_dictionary.is_empty()
        && !crate::user_dictionary::generation::has_split_wubi_tables(&database).unwrap_or(false)
    {
        std::path::PathBuf::from(&options.wubi_dictionary)
    } else {
        database
    };
    crate::wubi::lookup::lookup_codes(&database, text)
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

/// 双拼认的全拼音节，是与方案无关的同一张表：内置方案编不出来的音节（比如首道双拼的 `den`）仍算音节。评测工具按它把不带分隔的全拼切成音节，再交给 [`encode_shuangpin`]，这样切法有没有歧义在每个方案上判断得一样。
pub fn is_shuangpin_syllable(syllable: &str) -> bool {
    crate::shuangpin::profile::accepted_syllables().contains(syllable)
}

/// 把已规范化、用 `'` 分隔的全拼编成内置 26 键双拼方案 `profile`（方案名，同 [`shuangpin_key_hints`]）的两键码，仍用 `'` 分隔。方案名不认识或是 `custom`（没有内置的表）、有音节编不出来或编出来解不回原读音时为 `None`。评测工具用它把全拼用例换成双拼按键，与引擎解码用的是同一张表。
pub fn encode_shuangpin(pinyin: &str, profile: &str) -> Option<String> {
    let profile =
        ShuangpinProfileKind::from_name(profile).and_then(crate::shuangpin::profile::profile)?;
    crate::shuangpin::utils::encode_pinyin(pinyin, profile)
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
