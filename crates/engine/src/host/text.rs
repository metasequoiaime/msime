//! Thin string-path wrappers the bridge exposed over engine helpers.

use std::path::{Path, PathBuf};

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
    crate::dictionary::hanzi::hanzi_to_pinyin_batch(&main_dictionary_path(options), texts)
}

/// `hanzi_to_pinyin` 和 `hanzi_to_pinyin_batch` 查读音用的主词库文件（代次里的工作副本）。宿主在进程里缓存联系人读音时，拿这个文件的戳判断读音还算不算数。
pub fn main_dictionary_path(options: &EngineOptions) -> PathBuf {
    runtime_paths(options).dictionary(assets::MAIN_DICTIONARY)
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

/// 触屏键盘画「双拼键位提示」用的名字：粤语双拼不是普通话双拼方案，没有 `ShuangpinProfileKind`，宿主传这个名字给 [`shuangpin_key_hints`] 取它的键面。
pub const CANTONESE_SHUANGPIN_HINTS: &str = "cantonese";

/// 普通话双拼方案名（`xiaohe`、`microsoft` 等）给那个方案的键面；[`CANTONESE_SHUANGPIN_HINTS`] 给粤语双拼的键面。其他名字（包括 `custom`）什么也不给。`cantonese` 不是 `ShuangpinProfileKind` 的名字，[`shuangpin_zero_initials`]、[`encode_shuangpin`]、[`shuangpin_uses_semicolon`] 都不认它。
pub fn shuangpin_key_hints(profile: &str) -> Vec<ShuangpinKeyHint> {
    if profile == CANTONESE_SHUANGPIN_HINTS {
        return crate::cantonese::shuangpin::key_hints();
    }
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

/// 内置 26 键双拼方案 `profile`（方案名，同 [`shuangpin_key_hints`]）用不用 `;` 键，与会话决定宿主要不要显示 `;`（`EngineSnapshot::microsoft_shuangpin`）的是同一条判据。方案名不认识或是 `custom`（光有名字拿不到用户的表）时为 `None`。评测工具按它给方案配带不带 `;` 的那份键位几何。
pub fn shuangpin_uses_semicolon(profile: &str) -> Option<bool> {
    ShuangpinProfileKind::from_name(profile)
        .and_then(crate::shuangpin::profile::profile)
        .map(|profile| profile.uses_semicolon_key())
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
