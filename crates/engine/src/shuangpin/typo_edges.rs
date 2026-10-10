//! 双拼的纠错整句边。双拼每个音节两键，按错一个键多半仍是合法编码（小鹤 `hc` 是 hao，`hv` 是 hui），全拼那张按非法拼写生成的纠错表用不上，只能像全拼的纠错整句那样，让语言模型在合法音节之间比较。变体按当前方案的两键编码生成：一个键换成 QWERTY 邻键，或两键对调，再用同一方案解码成音节；之后的弱位置优先、跨度、预算、查词和罚分都走 `quanpin::typo_edges` 的同一套。

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

use super::utils::cvt_single_sp_to_pinyin;
use super::ShuangpinProfile;
use crate::cache::FifoCache;
use crate::dictionary::pinyin::PinyinDatabase;
use crate::dictionary::DictRow;
use crate::lattice::{SentencePath, TypoEdge};
use crate::pinyin::syllables::normalized_syllable;
use crate::pinyin::typos::{keys_adjacent, SyllableTypo, SyllableTypoKind};
use crate::quanpin::typo_edges::collect_span_typo_edges;
use crate::types::autocorrect_type;

/// 双拼会生成的纠错类型。漏键和多键会让后面每一对键都错位，不在这里处理。
pub const SHUANGPIN_TYPO_TYPES: u32 = autocorrect_type::TRANSPOSITION | autocorrect_type::NEIGHBOR;

/// 编码表里出现的键：26 个字母，加上微软双拼的 `;`。
const CODE_KEYS: &[u8] = b"abcdefghijklmnopqrstuvwxyz;";

/// `codes[i]` 是 `segments[i]` 的两键编码。用户接受过的纠错次数记在全拼的纠错档案里，那是按全拼字母误触统计的，套到双拼的键位上没有意义，所以这里不打折，罚分就是基础价。
pub fn collect_shuangpin_typo_edges(
    database: &PinyinDatabase,
    span_cache: &mut FifoCache<String, Vec<DictRow>>,
    profile: &'static ShuangpinProfile,
    codes: &[&str],
    segments: &[String],
    literal_best: &SentencePath,
    autocorrect_types: u32,
) -> Vec<TypoEdge> {
    debug_assert_eq!(codes.len(), segments.len());
    let table = typo_table(profile);
    collect_span_typo_edges(
        database,
        span_cache,
        segments,
        literal_best,
        autocorrect_types & SHUANGPIN_TYPO_TYPES,
        |position| table_typos(table, codes[position]),
        |_, _| 0,
    )
}

type TypoTable = HashMap<String, Vec<SyllableTypo>>;

/// 一个两键编码在方案下可能想打的音节，便宜的类型在前；不是合法编码时为空。
fn table_typos(table: &'static TypoTable, code: &str) -> &'static [SyllableTypo] {
    table.get(code).map_or(&[], Vec::as_slice)
}

/// 方案的变体表，第一次用到时整表算好。按方案的地址区分而不是按 `kind`：自定义方案的 `kind` 都是 `Custom`，键位却各不相同；`custom::custom_profile` 让同一张表总是同一个地址、换了键位就是另一个地址，所以每张表只算一次，改了键位也不会拿到旧键位算出的变体。表和自定义方案一样留到进程结束，数量不超过进程里实际用来打字的方案数。
fn typo_table(profile: &'static ShuangpinProfile) -> &'static TypoTable {
    static TABLES: Mutex<Vec<(&'static ShuangpinProfile, &'static TypoTable)>> =
        Mutex::new(Vec::new());
    let mut tables = TABLES.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(&(_, table)) = tables
        .iter()
        .find(|(known, _)| std::ptr::eq(*known, profile))
    {
        return table;
    }
    let mut table = HashMap::new();
    for &first in CODE_KEYS {
        for &second in CODE_KEYS {
            let code = [first, second];
            let typos = compute_code_typos(profile, &code);
            if !typos.is_empty() {
                table.insert(String::from_utf8_lossy(&code).into_owned(), typos);
            }
        }
    }
    let table: &'static TypoTable = Box::leak(Box::new(table));
    tables.push((profile, table));
    table
}

fn compute_code_typos(profile: &ShuangpinProfile, code: &[u8; 2]) -> Vec<SyllableTypo> {
    let Some(typed) = decode(profile, code) else {
        return Vec::new();
    };
    let mut variants = Vec::new();
    if code[0] != code[1] {
        add_variant(
            &mut variants,
            profile,
            &typed,
            [code[1], code[0]],
            SyllableTypoKind::Transposition,
        );
    }
    for index in 0..2 {
        for key in b'a'..=b'z' {
            if !keys_adjacent(key, code[index]) {
                continue;
            }
            let mut substituted = *code;
            substituted[index] = key;
            add_variant(
                &mut variants,
                profile,
                &typed,
                substituted,
                SyllableTypoKind::Neighbor,
            );
        }
    }
    variants.sort_by_key(|variant| variant.kind);
    variants
}

/// 词库键用的规范拼写（`lve`、`ju`），解不出合法音节时为 `None`。
fn decode(profile: &ShuangpinProfile, code: &[u8; 2]) -> Option<String> {
    let code = std::str::from_utf8(code).ok()?;
    let syllable = cvt_single_sp_to_pinyin(code, profile);
    (!syllable.is_empty()).then(|| normalized_syllable(&syllable))
}

/// 同一个音节由两种改动得到时只留便宜的那种。
fn add_variant(
    variants: &mut Vec<SyllableTypo>,
    profile: &ShuangpinProfile,
    typed: &str,
    candidate: [u8; 2],
    kind: SyllableTypoKind,
) {
    let Some(intended) = decode(profile, &candidate) else {
        return;
    };
    if intended == typed {
        return;
    }
    if let Some(existing) = variants
        .iter_mut()
        .find(|variant| variant.syllable == intended)
    {
        existing.kind = existing.kind.min(kind);
        return;
    }
    variants.push(SyllableTypo {
        syllable: intended,
        kind,
    });
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shuangpin::custom::custom_profile;
    use crate::shuangpin::profile::profile;
    use crate::types::{ShuangpinCustomTable, ShuangpinProfileKind};

    fn code_typos(profile: &'static ShuangpinProfile, code: &str) -> &'static [SyllableTypo] {
        table_typos(typo_table(profile), code)
    }

    fn syllables(kind: ShuangpinProfileKind, code: &str) -> Vec<(&'static str, SyllableTypoKind)> {
        code_typos(profile(kind).unwrap(), code)
            .iter()
            .map(|typo| (typo.syllable.as_str(), typo.kind))
            .collect()
    }

    #[test]
    fn neighbour_keys_decode_under_the_profile() {
        // 小鹤 hc 是 hao：c 的邻键 x、v、d、f 依次是 hua、hui、hai、hen，h 的邻键 g、b、n、y、u 依次是 gao、bao、nao、yao、shao（j 不能接 ao）。对调成 ch 是 cang。
        let xiaohe = syllables(ShuangpinProfileKind::Xiaohe, "hc");
        assert_eq!(
            xiaohe.first(),
            Some(&("cang", SyllableTypoKind::Transposition))
        );
        for expected in [
            "hua", "hui", "hai", "hen", "gao", "bao", "nao", "yao", "shao",
        ] {
            assert!(
                xiaohe
                    .iter()
                    .any(|&(syllable, kind)| syllable == expected
                        && kind == SyllableTypoKind::Neighbor),
                "{expected} missing from {xiaohe:?}"
            );
        }
        assert!(xiaohe.iter().all(|&(syllable, _)| syllable != "hao"));
        assert!(xiaohe.windows(2).all(|pair| pair[0].1 <= pair[1].1));

        // 自然码的 hao 是 hk，同样的 c 在自然码里是 iao，所以 hc 不是合法编码，没有变体。
        assert!(syllables(ShuangpinProfileKind::Ziranma, "hc").is_empty());
        let ziranma = syllables(ShuangpinProfileKind::Ziranma, "hk");
        assert!(ziranma.iter().any(|&(syllable, _)| syllable == "hai"));
    }

    #[test]
    fn variants_use_the_dictionary_spelling() {
        // 小鹤 t 是 ue/ve：lr（luan）的 r 按成邻键 t 时解成 lve，不出现排除表里的 lue。
        let xiaohe = syllables(ShuangpinProfileKind::Xiaohe, "lr");
        assert!(xiaohe.iter().any(|&(syllable, _)| syllable == "lve"));
        assert!(xiaohe.iter().all(|&(syllable, _)| syllable != "lue"));
        // 非法编码和未知输入都没有变体。
        assert!(syllables(ShuangpinProfileKind::Xiaohe, "q;").is_empty());
        assert!(syllables(ShuangpinProfileKind::Xiaohe, "abc").is_empty());
    }

    #[test]
    fn each_variant_is_listed_once_and_differs_from_the_typed_syllable() {
        for kind in [
            ShuangpinProfileKind::Xiaohe,
            ShuangpinProfileKind::Ziranma,
            ShuangpinProfileKind::Shoudao,
            ShuangpinProfileKind::Microsoft,
        ] {
            let selected = profile(kind).unwrap();
            for &first in CODE_KEYS {
                for &second in CODE_KEYS {
                    let code = String::from_utf8(vec![first, second]).unwrap();
                    let typed = cvt_single_sp_to_pinyin(&code, selected);
                    let typos = code_typos(selected, &code);
                    if typed.is_empty() {
                        assert!(typos.is_empty(), "{kind:?} {code}");
                        continue;
                    }
                    let typed = normalized_syllable(&typed);
                    for (index, typo) in typos.iter().enumerate() {
                        assert_ne!(typo.syllable, typed, "{kind:?} {code}");
                        assert!(
                            typos[index + 1..]
                                .iter()
                                .all(|other| other.syllable != typo.syllable),
                            "{kind:?} {code}"
                        );
                    }
                }
            }
        }
    }

    /// 小鹤的表，把 zh 换到 a 键，零声母改用 o 引导；`ing_on_semicolon` 时再把 ing 换到 `;`。
    fn custom_xiaohe(ing_on_semicolon: bool) -> &'static ShuangpinProfile {
        fn owned(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
            pairs
                .iter()
                .map(|(unit, key)| ((*unit).to_owned(), (*key).to_owned()))
                .collect()
        }
        let xiaohe = profile(ShuangpinProfileKind::Xiaohe).unwrap();
        let mut table = ShuangpinCustomTable {
            initials: owned(&[("zh", "a"), ("ch", "i"), ("sh", "u")]),
            finals: owned(xiaohe.finals),
            zero_initials: owned(&[
                ("a", "oa"),
                ("ai", "od"),
                ("an", "oj"),
                ("ang", "oh"),
                ("ao", "oc"),
                ("e", "oe"),
                ("ei", "ow"),
                ("en", "of"),
                ("eng", "og"),
                ("er", "or"),
                ("o", "oo"),
                ("ou", "oz"),
            ]),
        };
        if ing_on_semicolon {
            table
                .finals
                .iter_mut()
                .find(|(unit, _)| unit == "ing")
                .unwrap()
                .1 = ";".to_owned();
        }
        custom_profile(&table).unwrap()
    }

    #[test]
    fn a_custom_profile_gets_variants_for_its_own_layout() {
        // zh 在 a 键上：ag 是 zheng，g 的邻键 h 得到 zhang，对调成 ga；原来 zh 所在的 v 键不再是声母，vh 没有变体。
        let moved = custom_xiaohe(false);
        assert_eq!(moved.kind, ShuangpinProfileKind::Custom);
        let typos = code_typos(moved, "ag");
        assert!(typos
            .iter()
            .any(|typo| typo.syllable == "zhang" && typo.kind == SyllableTypoKind::Neighbor));
        assert!(typos
            .iter()
            .any(|typo| typo.syllable == "ga" && typo.kind == SyllableTypoKind::Transposition));
        assert!(code_typos(moved, "vh").is_empty());
        assert!(code_typos(moved, "x;").is_empty());

        // 再换一张表：同是 `Custom`，变体按这张表的键位算，不沿用上一张表的。ing 在 `;` 上，x; 是 xing，x 的邻键 d 得到 ding（`;` 不在字母区，没有邻键）。
        let semicolon = custom_xiaohe(true);
        assert!(!std::ptr::eq(moved, semicolon));
        assert!(code_typos(semicolon, "x;")
            .iter()
            .any(|typo| typo.syllable == "ding" && typo.kind == SyllableTypoKind::Neighbor));
        assert!(code_typos(moved, "x;").is_empty());
    }
}
