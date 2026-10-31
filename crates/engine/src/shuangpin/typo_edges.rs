//! 双拼的纠错整句边。双拼每个音节两键，按错一个键多半仍是合法编码（小鹤 `hc` 是 hao，`hv` 是 hui），全拼那张按非法拼写生成的纠错表用不上，只能像全拼的纠错整句那样，让语言模型在合法音节之间比较。变体按当前方案的两键编码生成：一个键换成 QWERTY 邻键，或两键对调，再用同一方案解码成音节；之后的弱位置优先、跨度、预算、查词和罚分都走 `quanpin::typo_edges` 的同一套。
//!
//! 触屏宿主送来了触点时（`collect_priced_shuangpin_typo_edges`），每个变体按落点计价（`touch::KeyGeometry::penalty`）：平面表原有的邻键永不丢弃，只按罚分排先后；平面表里没有的键（真实布局下的斜邻键、`;`）罚分不超过 `P_CUT` 时也加进来；对调和没有触点的位置照旧付 20。用户接受过的那对两键码（`LearnedTypos`）再按接受次数打折，这只发生在触点路径上，平面表照旧不打折。

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

use super::utils::cvt_single_sp_to_pinyin;
use super::ShuangpinProfile;
use crate::cache::FifoCache;
use crate::dictionary::pinyin::PinyinDatabase;
use crate::dictionary::DictRow;
use crate::lattice::{SentencePath, TypoEdge};
use crate::pinyin::syllables::normalized_syllable;
use crate::pinyin::typos::{
    discounted, keys_adjacent, SyllableTypo, SyllableTypoKind, NEIGHBOR_COST, TRANSPOSITION_COST,
};
use crate::quanpin::typo_edges::{
    collect_priced_span_typo_edges, collect_span_typo_edges, PricedTypo,
};
use crate::touch::{KeyGeometry, KeyTouch, TouchTuning};
use crate::types::{autocorrect_type, ShuangpinProfileKind};
use crate::user_dictionary::typo_profile::PersonalTypoProfile;

/// 双拼会生成的纠错类型。漏键和多键会让后面每一对键都错位，不在这里处理。
pub const SHUANGPIN_TYPO_TYPES: u32 = autocorrect_type::TRANSPOSITION | autocorrect_type::NEIGHBOR;

/// 编码表里出现的键：26 个字母，加上微软双拼的 `;`。
const CODE_KEYS: &[u8] = b"abcdefghijklmnopqrstuvwxyz;";

/// `codes[i]` 是 `segments[i]` 的两键编码。全拼的纠错档案是按全拼字母误触统计的，套到双拼的键位上没有意义；双拼按两键码学到的次数（`LearnedTypos`）只给触点路径打折，桌面没有触点，所以这里不打折，罚分就是基础价。
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
    let table = &profile_tables(profile).typos;
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

/// 用户在当前方案下接受过的双拼纠错次数，按两键码对查（`PersonalTypoProfile::shuangpin_counts`）。自定义方案、档案里一条双拼记录都没有时为空，计价不查也不打折。
#[derive(Clone, Copy, Default)]
pub struct LearnedTypos<'a> {
    source: Option<(&'a PersonalTypoProfile, ShuangpinProfileKind)>,
}

impl<'a> LearnedTypos<'a> {
    pub fn new(profile: &'a PersonalTypoProfile, kind: ShuangpinProfileKind) -> Self {
        let learnable = kind != ShuangpinProfileKind::Custom && profile.has_shuangpin_counts();
        Self {
            source: learnable.then_some((profile, kind)),
        }
    }

    fn accepted(&self, typed: [u8; 2], intended: [u8; 2]) -> i32 {
        self.source.map_or(0, |(profile, kind)| {
            profile.shuangpin_counts(kind, typed, intended).accepted
        })
    }
}

/// 一对两键码被接受过 `accepted` 次时罚分减去多少：与全拼按接受次数给邻键打的折相同，`NEIGHBOR_COST − discounted(NEIGHBOR_COST, n)`，接受 1 次约 2.8、3 次约 5.5，最多 12。
pub fn learned_discount(accepted: i32) -> f64 {
    NEIGHBOR_COST - discounted(NEIGHBOR_COST, accepted)
}

/// 打过折的罚分，不低于 `P_MIN`（本来就低于它的不往上抬）。没接受过时原样返回。
fn discounted_penalty(penalty: f64, accepted: i32, tuning: &TouchTuning) -> f64 {
    if accepted <= 0 {
        return penalty;
    }
    (penalty - learned_discount(accepted)).max(tuning.min_penalty.min(penalty))
}

/// 带触点时每个两键码按落点计过价的变体，`[i]` 对应 `codes[i]`。`touches` 与这段输入逐字节对齐：第 i 个两键码 `codes[i]` 是第 2i、2i+1 个按键。纠错边的规划（[`collect_priced_shuangpin_typo_edges`]）和落点支持的判断（[`touch_supports_correction`]）共用这一份，不重复计价。
pub(crate) fn priced_shuangpin_variants(
    profile: &'static ShuangpinProfile,
    codes: &[&str],
    touches: &[Option<KeyTouch>],
    geometry: &KeyGeometry,
    tuning: &TouchTuning,
    learned: LearnedTypos<'_>,
    autocorrect_types: u32,
) -> Vec<Vec<PricedTypo<'static>>> {
    debug_assert_eq!(touches.len(), codes.len() * 2);
    let decoded = &profile_tables(profile).decoded;
    let types = autocorrect_types & SHUANGPIN_TYPO_TYPES;
    codes
        .iter()
        .enumerate()
        .map(|(position, code)| {
            let touched = [touches[2 * position], touches[2 * position + 1]];
            priced_variants(
                decoded,
                code.as_bytes(),
                touched,
                geometry,
                tuning,
                learned,
                types,
            )
        })
        .collect()
}

/// 带触点时的纠错边。`variants` 是 [`priced_shuangpin_variants`] 给出的每个位置的变体，交给按罚分全局排序的规划（`collect_priced_span_typo_edges`）。
pub(crate) fn collect_priced_shuangpin_typo_edges(
    database: &PinyinDatabase,
    span_cache: &mut FifoCache<String, Vec<DictRow>>,
    variants: &[Vec<PricedTypo<'static>>],
    segments: &[String],
    literal_best: &SentencePath,
) -> Vec<TypoEdge> {
    debug_assert_eq!(variants.len(), segments.len());
    collect_priced_span_typo_edges(database, span_cache, segments, literal_best, |position| {
        variants[position].as_slice()
    })
}

/// 纠错整句改动过的每个音节，在这一键的落点下改过去的价都不超过 `TouchTuning::show_penalty`：落点本身支持这次改正。`variants` 是规划纠错边时算好的那一份（[`priced_shuangpin_variants`]）。`corrected_key` 是纠错整句的读音（逐音节以 `'` 相连），音节数与 `segments` 不同、或某个改动不在这个两键码的变体里时为假。
pub(crate) fn touch_supports_correction(
    variants: &[Vec<PricedTypo<'static>>],
    tuning: &TouchTuning,
    segments: &[String],
    corrected_key: &str,
) -> bool {
    let corrected: Vec<&str> = corrected_key.split('\'').collect();
    if corrected.len() != segments.len() || variants.len() != segments.len() {
        return false;
    }
    corrected
        .iter()
        .zip(segments)
        .zip(variants)
        .all(|((corrected, literal), variants)| {
            let corrected = normalized_syllable(corrected);
            if corrected == normalized_syllable(literal) {
                return true;
            }
            variants.iter().any(|variant| {
                variant.syllable == corrected && variant.penalty <= tuning.show_penalty
            })
        })
}

/// 首选规则 (c)：纠错整句改动过的每个音节，在这一键的落点下改过去的价都不超过 `TouchTuning::lead_penalty`，也就是触点本身支持这次改正。按不打学习折扣的原价算：接受过的改法分数更高、更容易留在座位上，但按在键心的字母（改到邻键的原价接近 B + s）永远不会被首选改掉。`codes[i]` 是第 i 个音节的两键码，`touches` 与它们逐字节对齐（第 2i、2i+1 个按键）；对调和没有触点的位置照平面表付 20。`corrected_key` 是纠错整句的读音（逐音节以 `'` 相连），音节数对不上、或某个改动不在这个两键码的变体里时为假。
pub(crate) fn touch_leads_correction(
    profile: &'static ShuangpinProfile,
    codes: &[&str],
    touches: &[Option<KeyTouch>],
    geometry: &KeyGeometry,
    tuning: &TouchTuning,
    corrected_key: &str,
    autocorrect_types: u32,
) -> bool {
    let corrected: Vec<&str> = corrected_key.split('\'').collect();
    if corrected.len() != codes.len() || touches.len() != codes.len() * 2 {
        return false;
    }
    let decoded = &profile_tables(profile).decoded;
    let types = autocorrect_types & SHUANGPIN_TYPO_TYPES;
    corrected
        .iter()
        .zip(codes)
        .enumerate()
        .all(|(position, (corrected, code))| {
            let &[first, second] = code.as_bytes() else {
                return false;
            };
            let Some(literal) = decoded_syllable(decoded, [first, second]) else {
                return false;
            };
            let corrected = normalized_syllable(corrected);
            if corrected == literal {
                return true;
            }
            let touched = [touches[2 * position], touches[2 * position + 1]];
            priced_code_variants(
                decoded,
                [first, second],
                touched,
                geometry,
                tuning,
                LearnedTypos::default(),
                types,
            )
            .iter()
            .any(|variant| variant.syllable == corrected && variant.penalty <= tuning.lead_penalty)
        })
}

/// 一条纠错在某个音节上的改动：第 `position` 个两键码 `typed` 被改成了 `intended`，解出 `syllable`。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CorrectionPair {
    pub position: usize,
    pub typed: [u8; 2],
    pub intended: [u8; 2],
    pub syllable: &'static str,
}

/// 用户接受或拒绝一条触点纠错时要记的键码对。`typed` 是这条纠错回答的按键（不含 `'`，每个音节两键），`touches` 与它逐字节对齐，`corrected_key` 是纠错的读音（逐音节以 `'` 相连）。每个改动过的音节取本次变体集里解码到纠正后音节、罚分最低的那个两键码，不从音节反推：零声母的一个音节在同一方案里可能有几个码。读音和按键对不上，或某个改动不在这个两键码的变体里（例如模糊音的替代拼写），整条不记，返回 `None`。
#[allow(clippy::too_many_arguments)]
pub fn touch_correction_pairs(
    profile: &'static ShuangpinProfile,
    typed: &str,
    touches: &[Option<KeyTouch>],
    geometry: &KeyGeometry,
    tuning: &TouchTuning,
    learned: LearnedTypos<'_>,
    corrected_key: &str,
    autocorrect_types: u32,
) -> Option<Vec<CorrectionPair>> {
    let corrected: Vec<&str> = corrected_key.split('\'').collect();
    if typed.len() != corrected.len() * 2 || touches.len() != typed.len() {
        return None;
    }
    let decoded = &profile_tables(profile).decoded;
    let types = autocorrect_types & SHUANGPIN_TYPO_TYPES;
    let mut pairs = Vec::new();
    for (position, (code, corrected)) in typed.as_bytes().chunks_exact(2).zip(corrected).enumerate()
    {
        let code = [code[0], code[1]];
        let literal = decoded_syllable(decoded, code);
        let corrected = normalized_syllable(corrected);
        if Some(corrected.as_str()) == literal {
            continue;
        }
        let touched = [touches[2 * position], touches[2 * position + 1]];
        let best = priced_code_variants(decoded, code, touched, geometry, tuning, learned, types)
            .into_iter()
            .filter(|variant| variant.syllable == corrected)
            .min_by(|left, right| left.penalty.total_cmp(&right.penalty))?;
        pairs.push(CorrectionPair {
            position,
            typed: code,
            intended: best.code,
            syllable: best.syllable,
        });
    }
    Some(pairs)
}

/// 一个两键码按触点计价的变体，罚分低的在前，同一个音节只留最低的那个价。
fn priced_variants(
    decoded: &'static DecodeTable,
    code: &[u8],
    touches: [Option<KeyTouch>; 2],
    geometry: &KeyGeometry,
    tuning: &TouchTuning,
    learned: LearnedTypos<'_>,
    types: u32,
) -> Vec<PricedTypo<'static>> {
    let [first, second] = *code else {
        return Vec::new();
    };
    let mut variants: Vec<PricedTypo<'static>> = Vec::new();
    for variant in priced_code_variants(
        decoded,
        [first, second],
        touches,
        geometry,
        tuning,
        learned,
        types,
    ) {
        if let Some(existing) = variants
            .iter_mut()
            .find(|existing| existing.syllable == variant.syllable)
        {
            existing.penalty = existing.penalty.min(variant.penalty);
            continue;
        }
        variants.push(PricedTypo {
            syllable: variant.syllable,
            penalty: variant.penalty,
        });
    }
    variants.sort_by(|left, right| left.penalty.total_cmp(&right.penalty));
    variants
}

/// 一个变体两键码、它解出的音节和它的价。
struct PricedCode {
    code: [u8; 2],
    syllable: &'static str,
    penalty: f64,
}

/// 一个两键码的每一种改法（对调在前，再按位置、按 `CODE_KEYS` 的次序换键），按触点计价、按学到的次数打折；解不出音节或与输入的音节相同的不算。同一个音节可能出现几次，由调用方取最低价。
fn priced_code_variants(
    decoded: &'static DecodeTable,
    code: [u8; 2],
    touches: [Option<KeyTouch>; 2],
    geometry: &KeyGeometry,
    tuning: &TouchTuning,
    learned: LearnedTypos<'_>,
    types: u32,
) -> Vec<PricedCode> {
    let [first, second] = code;
    let typed = decoded_syllable(decoded, code);
    let mut variants = Vec::new();
    let mut push = |substituted: [u8; 2], penalty: f64| {
        if let Some(syllable) = decoded_syllable(decoded, substituted).filter(|s| Some(*s) != typed)
        {
            variants.push(PricedCode {
                code: substituted,
                syllable,
                penalty,
            });
        }
    };
    if types & autocorrect_type::TRANSPOSITION != 0 && first != second {
        let swapped = [second, first];
        push(
            swapped,
            discounted_penalty(TRANSPOSITION_COST, learned.accepted(code, swapped), tuning),
        );
    }
    if types & autocorrect_type::NEIGHBOR != 0 {
        for (index, touch) in touches.into_iter().enumerate() {
            let pressed = code[index];
            for &key in CODE_KEYS {
                if key == pressed {
                    continue;
                }
                let mut substituted = code;
                substituted[index] = key;
                let flat = keys_adjacent(key, pressed);
                // 学到的折扣先打，再和 P_CUT 比：接受过的斜邻键也能进来。
                let learned_penalty = |penalty: f64| {
                    discounted_penalty(penalty, learned.accepted(code, substituted), tuning)
                };
                let penalty = match touch
                    .map(|touch| (touch, geometry.penalty(pressed, touch, key, tuning)))
                {
                    // 平面表的邻键永不丢弃，只按落点排先后；分离键盘上分在两半的那几对不算邻键。
                    Some((touch, Some(penalty))) if flat => {
                        if !geometry.adjacent(pressed, touch, key) {
                            continue;
                        }
                        learned_penalty(penalty)
                    }
                    // 平面表里没有的键，落点足够近才加。
                    Some((_, Some(penalty))) => {
                        let penalty = learned_penalty(penalty);
                        if penalty > tuning.cut {
                            continue;
                        }
                        penalty
                    }
                    // 这一键没有触点，或这个键不在几何里：照平面表。
                    _ if flat => learned_penalty(NEIGHBOR_COST),
                    _ => continue,
                };
                push(substituted, penalty);
            }
        }
    }
    variants
}

/// 方案下每个两键码解成的音节（词库拼写），按 `CODE_KEYS` 的下标排成 27×27 的表；解不出时为 `None`。
type DecodeTable = Vec<Option<String>>;

fn code_index(key: u8) -> Option<usize> {
    CODE_KEYS.iter().position(|&candidate| candidate == key)
}

fn decoded_syllable(table: &'static DecodeTable, [first, second]: [u8; 2]) -> Option<&'static str> {
    let index = code_index(first)? * CODE_KEYS.len() + code_index(second)?;
    table[index].as_deref()
}

type TypoTable = HashMap<String, Vec<SyllableTypo>>;

/// 一个两键编码在方案下可能想打的音节，便宜的类型在前；不是合法编码时为空。
fn table_typos(table: &'static TypoTable, code: &str) -> &'static [SyllableTypo] {
    table.get(code).map_or(&[], Vec::as_slice)
}

/// 一个方案的两张表：平面变体表（没有触点时用）和两键码解码表（按触点计价时用）。
struct ProfileTables {
    typos: TypoTable,
    decoded: DecodeTable,
}

/// 方案的变体表和解码表，第一次用到时整表算好，同一个条目、同一把锁。按方案的地址区分而不是按 `kind`：自定义方案的 `kind` 都是 `Custom`，键位却各不相同；`custom::custom_profile` 让同一张表总是同一个地址、换了键位就是另一个地址，所以每张表只算一次，改了键位也不会拿到旧键位算出的变体。表和自定义方案一样留到进程结束，数量不超过进程里实际用来打字的方案数。
fn profile_tables(profile: &'static ShuangpinProfile) -> &'static ProfileTables {
    static TABLES: Mutex<Vec<(&'static ShuangpinProfile, &'static ProfileTables)>> =
        Mutex::new(Vec::new());
    let mut tables = TABLES.lock().unwrap_or_else(PoisonError::into_inner);
    if let Some(&(_, entry)) = tables
        .iter()
        .find(|(known, _)| std::ptr::eq(*known, profile))
    {
        return entry;
    }
    let mut typos = HashMap::new();
    let mut decoded = Vec::with_capacity(CODE_KEYS.len() * CODE_KEYS.len());
    for &first in CODE_KEYS {
        for &second in CODE_KEYS {
            let code = [first, second];
            let code_typos = compute_code_typos(profile, &code);
            if !code_typos.is_empty() {
                typos.insert(String::from_utf8_lossy(&code).into_owned(), code_typos);
            }
            decoded.push(decode(profile, &code));
        }
    }
    let entry: &'static ProfileTables = Box::leak(Box::new(ProfileTables { typos, decoded }));
    tables.push((profile, entry));
    entry
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
    use crate::touch::KeyRect;
    use crate::types::{ShuangpinCustomTable, ShuangpinProfileKind};
    use crate::user_dictionary::typo_profile::ShuangpinTypoOutcome;
    use std::sync::Arc;

    fn code_typos(profile: &'static ShuangpinProfile, code: &str) -> &'static [SyllableTypo] {
        table_typos(&profile_tables(profile).typos, code)
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
            ShuangpinProfileKind::SmartAbc,
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

    /// QWERTY 字母键，键宽 10、键高 14，每排比上一排右移半个键；`moved` 里的键挪到给定的键心。
    fn geometry(moved: &[(u8, f32, f32)], duplicates: &[(u8, KeyRect)]) -> KeyGeometry {
        let mut keys = [[0.0f32; 4]; 26];
        for (letters, left, y) in [
            ("qwertyuiop", 5.0, 7.0),
            ("asdfghjkl", 10.0, 21.0),
            ("zxcvbnm", 20.0, 35.0),
        ] {
            for (column, letter) in letters.bytes().enumerate() {
                keys[usize::from(letter - b'a')] = [left + 10.0 * column as f32, y, 10.0, 14.0];
            }
        }
        for &(key, x, y) in moved {
            keys[usize::from(key - b'a')][..2].copy_from_slice(&[x, y]);
        }
        KeyGeometry::new(1, &keys, None, duplicates).unwrap()
    }

    fn tuning() -> TouchTuning {
        TouchTuning {
            base: 12.0,
            scale: 8.0,
            sigma_x: 1.0,
            sigma_y: 1.0,
            bias_y: 0.0,
            min_penalty: 4.0,
            max_penalty: 28.0,
            cut: 16.0,
            show_margin: 0.0,
            show_penalty: f64::INFINITY,
            lead_margin: f64::INFINITY,
            lead_penalty: f64::INFINITY,
        }
    }

    fn priced(
        geometry: &KeyGeometry,
        code: &str,
        touches: [Option<KeyTouch>; 2],
    ) -> Vec<(&'static str, f64)> {
        priced_learned(geometry, code, touches, LearnedTypos::default())
    }

    fn priced_learned(
        geometry: &KeyGeometry,
        code: &str,
        touches: [Option<KeyTouch>; 2],
        learned: LearnedTypos<'_>,
    ) -> Vec<(&'static str, f64)> {
        let xiaohe = profile(ShuangpinProfileKind::Xiaohe).unwrap();
        priced_variants(
            &profile_tables(xiaohe).decoded,
            code.as_bytes(),
            touches,
            geometry,
            &tuning(),
            learned,
            SHUANGPIN_TYPO_TYPES,
        )
        .into_iter()
        .map(|typo| (typo.syllable, typo.penalty))
        .collect()
    }

    /// 一份新日志上的纠错档案，小鹤下把 `typed` 当成 `intended` 接受过 `times` 次。
    fn learned_store(
        times: usize,
        pairs: &[([u8; 2], [u8; 2])],
    ) -> (tempfile::TempDir, Arc<PersonalTypoProfile>) {
        let dir = tempfile::tempdir().unwrap();
        let store = PersonalTypoProfile::shared(&dir.path().join("msime_user.db"));
        for _ in 0..times {
            store
                .record_shuangpin(
                    ShuangpinProfileKind::Xiaohe,
                    pairs,
                    ShuangpinTypoOutcome::Accepted,
                )
                .unwrap();
        }
        (dir, store)
    }

    /// 接受过的那一对两键码按次数打折、不低于 P_MIN；同一个两键码的别的变体、自定义方案、空档案都不打折。
    #[test]
    fn learned_pairs_lower_only_their_own_price() {
        let geometry = geometry(&[], &[]);
        let touches = [None, at(-5, -10)];
        let plain = priced(&geometry, "gf", touches);
        let (_dir, store) = learned_store(3, &[(*b"gf", *b"gr")]);
        let learned = priced_learned(
            &geometry,
            "gf",
            touches,
            LearnedTypos::new(&store, ShuangpinProfileKind::Xiaohe),
        );
        let price = |variants: &[(&str, f64)], syllable: &str| {
            variants
                .iter()
                .find(|(candidate, _)| *candidate == syllable)
                .map(|(_, penalty)| *penalty)
                .unwrap()
        };
        assert_eq!(price(&plain, "guan"), 12.0);
        assert!((price(&learned, "guan") - (12.0 - learned_discount(3))).abs() < 1e-9);
        assert!((learned_discount(3) - 4.0 * 4f64.ln()).abs() < 1e-9);
        for (syllable, penalty) in &plain {
            if *syllable != "guan" {
                assert_eq!(price(&learned, syllable), *penalty, "{syllable}");
            }
        }
        // 自定义方案不学，也不查。
        assert_eq!(
            priced_learned(
                &geometry,
                "gf",
                touches,
                LearnedTypos::new(&store, ShuangpinProfileKind::Custom)
            ),
            plain
        );
        // 接受得再多也不低于 P_MIN。
        let (_dir, many) = learned_store(40, &[(*b"gf", *b"gr")]);
        let floored = priced_learned(
            &geometry,
            "gf",
            touches,
            LearnedTypos::new(&many, ShuangpinProfileKind::Xiaohe),
        );
        assert_eq!(price(&floored, "guan"), tuning().min_penalty);
        assert_eq!(floored[0], ("guan", tuning().min_penalty));
    }

    /// 平面表外的键先打折再和 P_CUT 比：接受过的斜邻键能进来，没接受过的照旧挡在外面。
    #[test]
    fn a_learned_key_off_the_flat_table_can_pass_the_cut() {
        let geometry = geometry(&[(b'e', 40.0, 7.0)], &[]);
        let has = |variants: &[(&str, f64)], syllable: &str| {
            variants.iter().any(|(candidate, _)| *candidate == syllable)
        };
        assert!(!has(&priced(&geometry, "gf", [None, at(0, 0)]), "ge"));
        let (_dir, store) = learned_store(3, &[(*b"gf", *b"ge")]);
        let learned = priced_learned(
            &geometry,
            "gf",
            [None, at(0, 0)],
            LearnedTypos::new(&store, ShuangpinProfileKind::Xiaohe),
        );
        assert!(
            learned.contains(&("ge", 20.0 - learned_discount(3))),
            "{learned:?}"
        );
    }

    /// 接受和拒绝记的码对：每个改动过的音节取变体集里解到纠正后音节、罚分最低的那个码；读音和按键对不上或改动解释不了时整条不记。
    #[test]
    fn correction_pairs_name_the_cheapest_code_of_each_changed_syllable() {
        let xiaohe = profile(ShuangpinProfileKind::Xiaohe).unwrap();
        let geometry = geometry(&[], &[]);
        let mut touches = vec![at(0, 0); 6];
        touches[3] = at(-5, -10);
        let pairs = |corrected: &str, touches: &[Option<KeyTouch>]| {
            touch_correction_pairs(
                xiaohe,
                "mwgfxi",
                touches,
                &geometry,
                &tuning(),
                LearnedTypos::default(),
                corrected,
                SHUANGPIN_TYPO_TYPES,
            )
        };
        assert_eq!(
            pairs("mei'guan'xi", &touches),
            Some(vec![CorrectionPair {
                position: 1,
                typed: *b"gf",
                intended: *b"gr",
                syllable: "guan",
            }])
        );
        assert_eq!(pairs("mei'gen'xi", &touches), Some(Vec::new()));
        assert_eq!(pairs("mei'guan", &touches), None);
        assert_eq!(pairs("mei'zhuang'xi", &touches), None);
        assert_eq!(pairs("mei'guan'xi", &touches[..4]), None);

        // 每个两键码、每个变体音节：记下的码解得出这个音节，价就是变体集给这个音节的价。
        let touched = [at(3, -4), at(-6, 2)];
        for &first in CODE_KEYS {
            for &second in CODE_KEYS {
                let code = [first, second];
                let typed = std::str::from_utf8(&code).unwrap();
                for (syllable, penalty) in priced(&geometry, typed, touched) {
                    let found = touch_correction_pairs(
                        xiaohe,
                        typed,
                        &touched,
                        &geometry,
                        &tuning(),
                        LearnedTypos::default(),
                        syllable,
                        SHUANGPIN_TYPO_TYPES,
                    )
                    .unwrap();
                    let [pair] = found.as_slice() else {
                        panic!("{typed} -> {syllable}: {found:?}");
                    };
                    assert_eq!(
                        decoded_syllable(&profile_tables(xiaohe).decoded, pair.intended),
                        Some(syllable)
                    );
                    let cheapest = priced_code_variants(
                        &profile_tables(xiaohe).decoded,
                        code,
                        touched,
                        &geometry,
                        &tuning(),
                        LearnedTypos::default(),
                        SHUANGPIN_TYPO_TYPES,
                    )
                    .into_iter()
                    .find(|variant| variant.code == pair.intended)
                    .unwrap();
                    assert_eq!(cheapest.penalty, penalty, "{typed} -> {syllable}");
                }
            }
        }
    }

    fn at(dx: i8, dy: i8) -> Option<KeyTouch> {
        Some(KeyTouch { dx, dy, copy: 0 })
    }

    #[test]
    fn touched_variants_keep_every_flat_variant_and_price_it_by_the_touch() {
        let geometry = geometry(&[], &[]);
        let xiaohe = profile(ShuangpinProfileKind::Xiaohe).unwrap();
        // 小鹤 gf 是 gen：平面表的每个变体都在，没有触点的 g 那一位和对调照旧付 20。
        let variants = priced(&geometry, "gf", [None, at(0, 0)]);
        for flat in code_typos(xiaohe, "gf") {
            assert!(
                variants
                    .iter()
                    .any(|(syllable, _)| *syllable == flat.syllable),
                "{} missing from {variants:?}",
                flat.syllable
            );
        }
        let price = |variants: &[(&str, f64)], syllable: &str| {
            variants
                .iter()
                .find(|(candidate, _)| *candidate == syllable)
                .map(|(_, penalty)| *penalty)
        };
        // 对调成 fg（feng）付 20；g 换成邻键 h 得到 hen，那一位没有触点，也付 20。
        assert_eq!(price(&variants, "feng"), Some(20.0));
        assert_eq!(price(&variants, "hen"), Some(20.0));
        // f 按在键心：换成同排的 d（d 是 ai，得到 gai）是 B + s·1 = 20，换成斜上方的 r（guan）更贵。
        assert_eq!(price(&variants, "gai"), Some(20.0));
        let centered_r = price(&variants, "guan").unwrap();
        assert!(centered_r > 20.0, "{centered_r}");
        // 落点偏向 r：这次改正降到 B，排到最前；罚分低的在前。
        let toward_r = priced(&geometry, "gf", [None, at(-5, -10)]);
        assert_eq!(toward_r[0], ("guan", 12.0), "{toward_r:?}");
        assert!(toward_r.windows(2).all(|pair| pair[0].1 <= pair[1].1));
    }

    #[test]
    fn nearby_keys_off_the_flat_table_join_only_when_the_touch_supports_them() {
        // 把 e 挪到 f 正上方：平面表里 e 不是 f 的邻键（f 上方是 r 和 t）。
        let geometry = geometry(&[(b'e', 40.0, 7.0)], &[]);
        let has = |variants: &[(&str, f64)], syllable: &str| {
            variants.iter().any(|(candidate, _)| *candidate == syllable)
        };
        // 小鹤 ge 是 ge；按在 f 键心时换成 e 要付 B + s·1 = 20，超过 P_CUT，不加。
        assert!(!has(&priced(&geometry, "gf", [None, at(0, 0)]), "ge"));
        // 落点往上偏半个键：罚分是 B，加进来。
        let leaning = priced(&geometry, "gf", [None, at(0, -10)]);
        assert!(leaning.contains(&("ge", 12.0)), "{leaning:?}");
        // 这一键没有触点时只照平面表。
        assert!(!has(&priced(&geometry, "gf", [None, None]), "ge"));
    }

    #[test]
    fn split_layouts_drop_flat_neighbours_across_the_gap() {
        // 分离键盘：右半边整体右移 60，G、V 在右半边内侧各再出现一次。
        let mut moved = Vec::new();
        for (letters, left, y) in [
            ("yuiop", 115.0, 7.0),
            ("hjkl", 120.0, 21.0),
            ("bnm", 120.0, 35.0),
        ] {
            for (column, letter) in letters.bytes().enumerate() {
                moved.push((letter, left + 10.0 * column as f32, y));
            }
        }
        let split = geometry(
            &moved,
            &[
                (b'g', [110.0, 21.0, 10.0, 14.0]),
                (b'v', [110.0, 35.0, 10.0, 14.0]),
            ],
        );
        // 小鹤 lt 是 lve，平面表把 t 的邻键 y 算上（lun）；分在两半时不算。
        let flat = code_typos(profile(ShuangpinProfileKind::Xiaohe).unwrap(), "lt");
        assert!(flat.iter().any(|typo| typo.syllable == "lun"));
        let variants = priced(&split, "lt", [None, at(0, 0)]);
        assert!(
            variants.iter().all(|(syllable, _)| *syllable != "lun"),
            "{variants:?}"
        );
        // 没有触点的那一键不知道按在哪一半，照平面表。
        let untouched = priced(&split, "lt", [None, None]);
        assert!(untouched.contains(&("lun", 20.0)), "{untouched:?}");
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
