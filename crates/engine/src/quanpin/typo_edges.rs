//! Typo edges for the lattice's typo sentence (quanpin.md §10.5, QD:1056-1182): legal-to-legal syllable swaps, weak positions first, priced by kind and discounted by how often the user accepted that typo; plus re-segmented spans where one typo split a syllable into several legal ones (`plan_resegmented_keys`).

use crate::cache::FifoCache;
use crate::dictionary::pinyin::PinyinDatabase;
use crate::dictionary::DictRow;
use crate::lattice::{SentencePath, TypoEdge};
use crate::pinyin::autocorrect::{autocorrect_cut_kbest, has_correction_key};
use crate::pinyin::syllables::MAX_SYLLABLE_LENGTH;
use crate::pinyin::typos::{
    autocorrect_bit, base_cost, discounted, syllable_typos, SyllableTypo, SyllableTypoKind,
};
use crate::text::count_utf8_chars;
use crate::user_dictionary::typo_profile::PersonalTypoProfile;

/// Typo edges are looked up for at most this many variant keys per query, most of them cache hits while typing on.
pub const TYPO_KEY_BUDGET: usize = 96;
/// A variant is only looked up as part of a phrase. Replacing a lone character almost never beats the literal reading, and those lookups were the costly half: a syllable's table holds hundreds of rows per key, all read to find the top few. Dropping them halved the lookup time and cost 0.1 point of recall on the typo eval set (QD:37-41).
pub const MIN_TYPO_SPAN_SYLLABLES: usize = 2;
pub const MAX_TYPO_SPAN_SYLLABLES: usize = 3;
pub const TYPO_ROWS_PER_KEY: usize = 4;

struct PlannedKey {
    start: usize,
    end: usize,
    key: String,
    penalty: f64,
}

enum WeakPositions {
    Bits(u64),
    Heap(Vec<bool>),
}

impl WeakPositions {
    fn new(length: usize) -> Self {
        if length <= u64::BITS as usize {
            Self::Bits(0)
        } else {
            Self::Heap(vec![false; length])
        }
    }

    fn set(&mut self, position: usize) {
        match self {
            Self::Bits(bits) => *bits |= 1u64 << position,
            Self::Heap(weak) => weak[position] = true,
        }
    }

    fn clear(&mut self) {
        match self {
            Self::Bits(bits) => *bits = 0,
            Self::Heap(weak) => weak.fill(false),
        }
    }

    fn get(&self, position: usize) -> bool {
        match self {
            Self::Bits(bits) => bits & (1u64 << position) != 0,
            Self::Heap(weak) => weak[position],
        }
    }
}

/// 按 `TYPO_KEY_BUDGET` 规划跨度键，通过 `span_cache` 查询并缓存空结果，逐行产生纠错边。
pub fn collect_typo_edges(
    database: &PinyinDatabase,
    span_cache: &mut FifoCache<String, Vec<DictRow>>,
    profile: &PersonalTypoProfile,
    segments: &[String],
    literal_best: &SentencePath,
    autocorrect_types: u32,
) -> Vec<TypoEdge> {
    let mut planned = plan_keys(profile, segments, literal_best, autocorrect_types);
    plan_resegmented_keys(segments, autocorrect_types, &mut planned);
    // 全拼的纠错整句只在扣掉罚分后胜过字面整句时才出现，按参考实现的原价（每处 20 到 24）几乎永远胜不过：合成的合法拼音打错集上找回不到一成。见 `QUANPIN_TYPO_PENALTY_SCALE`。
    for entry in &mut planned {
        entry.penalty *= QUANPIN_TYPO_PENALTY_SCALE;
    }
    lookup_planned_edges(database, span_cache, &planned)
}

/// 全拼纠错边的罚分打这个折扣（双拼另有自己的计价，不受影响）。在合法拼音打错集（sentences-v1 和手写聊天集各删、换、调、重按一个字母，改完仍读得成合法音节的 1812 条）和同一批原句（466 条）上，开纠错、挂逐键模型扫 1、0.75、0.5：0.75 时打错集前五 0.157 → 0.227，原句首选和前五一条不变；0.5 时原句首选开始掉（0.704 → 0.695）。
pub const QUANPIN_TYPO_PENALTY_SCALE: f64 = 0.75;

/// 重切分纠错最多看几个相邻的字面音节。乱序、漏字母、多字母可能把一个音节打成几个合法音节（`zai` 打成 `zia`，读成 zi'a；`xiao` 打成 `xioa`，读成 xi'o'a），这时整串都是合法拼音，`resolve_series_query` 不会去纠错，一换一的音节替换也表达不了。
pub const MAX_RESEGMENTED_LITERAL_SYLLABLES: usize = 3;
/// 每段字面音节取几条纠错切分。一段只有几个字母，只改一处的读法很少超过这个数。
const RESEGMENTED_CUTS_PER_SPAN: usize = 4;
/// 重切分键在 `TYPO_KEY_BUDGET` 之外单独的预算，一换一的键吃光预算时它们照样能查到。
pub const RESEGMENTED_KEY_BUDGET: usize = 32;

/// 规划重切分纠错键：相邻 2 到 3 个字面音节的字母串，按 `autocorrect_cut_kbest` 只改一处后读成音节数不同的读法（zi'a → zai），再向两边补字面音节凑成 2 到 3 个音节的词键（xian'zai）。边的 `start..end` 是它盖住的字面音节，读法来自词库行，所以纠错整句带的是改正后的读音。这条边能不能出现在候选里，仍由纠错整句的解码决定：带罚分之后要胜过字面整句。
fn plan_resegmented_keys(
    segments: &[String],
    autocorrect_types: u32,
    planned: &mut Vec<PlannedKey>,
) {
    let n = segments.len();
    if autocorrect_types == 0 {
        return;
    }
    let mut added = 0;
    // 几个音节的字母拼在栈上；一段里没有任何错键时不跑搜索，按键路径上绝大多数输入因此不分配。
    let mut buffer = [0u8; MAX_RESEGMENTED_LITERAL_SYLLABLES * MAX_SYLLABLE_LENGTH];
    for start in 0..n {
        let mut length = 0;
        for end in start + 1..=n.min(start + MAX_RESEGMENTED_LITERAL_SYLLABLES) {
            let segment = segments[end - 1].as_bytes();
            let Some(slot) = buffer.get_mut(length..length + segment.len()) else {
                break;
            };
            slot.copy_from_slice(segment);
            length += segment.len();
            if end - start < 2 || !has_correction_key(&buffer[..length]) {
                continue;
            }
            let Ok(letters) = std::str::from_utf8(&buffer[..length]) else {
                break;
            };
            for cut in autocorrect_cut_kbest(letters, autocorrect_types, RESEGMENTED_CUTS_PER_SPAN)
            {
                // 音节数不变的是一换一的替换，`plan_keys` 已经按音节规划过。
                if cut.edge_count != 1 || cut.segments.len() == end - start {
                    continue;
                }
                let Some(penalty) = cut
                    .segments
                    .iter()
                    .find(|segment| segment.corrected)
                    .map(|segment| base_cost(edit_kind(&segment.raw_text, &segment.syllable)))
                else {
                    continue;
                };
                let corrected = cut.segments.len();
                if corrected > MAX_TYPO_SPAN_SYLLABLES {
                    continue;
                }
                for left in 0..=start.min(MAX_TYPO_SPAN_SYLLABLES - corrected) {
                    for right in 0..=(n - end).min(MAX_TYPO_SPAN_SYLLABLES - corrected - left) {
                        if left + corrected + right < MIN_TYPO_SPAN_SYLLABLES {
                            continue;
                        }
                        let syllables = segments[start - left..start]
                            .iter()
                            .map(String::as_str)
                            .chain(cut.segments.iter().map(|segment| segment.syllable.as_str()))
                            .chain(segments[end..end + right].iter().map(String::as_str));
                        let key = syllables.collect::<Vec<_>>().join("'");
                        let (key_start, key_end) = (start - left, end + right);
                        if planned.iter().any(|entry| {
                            entry.key == key && entry.start == key_start && entry.end == key_end
                        }) {
                            continue;
                        }
                        planned.push(PlannedKey {
                            start: key_start,
                            end: key_end,
                            key,
                            penalty,
                        });
                        added += 1;
                        if added >= RESEGMENTED_KEY_BUDGET {
                            return;
                        }
                    }
                }
            }
        }
    }
}

/// 把 `typed` 改成 `intended` 的那一处编辑属于哪一类：长度不同是漏字母或多字母，字母相同只是次序不同是乱序，其余是按到了邻键。
fn edit_kind(typed: &str, intended: &str) -> SyllableTypoKind {
    if typed.len() != intended.len() {
        return SyllableTypoKind::MissingOrExtra;
    }
    let mut typed = typed.as_bytes().to_vec();
    let mut intended = intended.as_bytes().to_vec();
    typed.sort_unstable();
    intended.sort_unstable();
    if typed == intended {
        SyllableTypoKind::Transposition
    } else {
        SyllableTypoKind::Neighbor
    }
}

/// 全拼和双拼共用的规划与查词。`typos_at(i)` 给出第 i 个音节可能想打的音节（便宜的类型在前），`accepted(i, typo)` 给出用户接受过这条纠错的次数；全拼按音节拼写生成变体，双拼按当前方案的两键编码生成，之后的弱位置优先、跨度、预算、查词和罚分完全相同。
pub(crate) fn collect_span_typo_edges<'t>(
    database: &PinyinDatabase,
    span_cache: &mut FifoCache<String, Vec<DictRow>>,
    segments: &[String],
    literal_best: &SentencePath,
    autocorrect_types: u32,
    typos_at: impl Fn(usize) -> &'t [SyllableTypo],
    accepted: impl Fn(usize, &SyllableTypo) -> i32,
) -> Vec<TypoEdge> {
    let planned = plan_keys_with(
        segments,
        literal_best,
        autocorrect_types,
        typos_at,
        accepted,
    );
    lookup_planned_edges(database, span_cache, &planned)
}

fn lookup_planned_edges(
    database: &PinyinDatabase,
    span_cache: &mut FifoCache<String, Vec<DictRow>>,
    planned: &[PlannedKey],
) -> Vec<TypoEdge> {
    let mut misses: Vec<String> = Vec::new();
    for entry in planned {
        // 重切分键可能和一换一的键同一个键、跨度不同（xian'zai 盖住 0..2 和 0..3）。同一个键只查一次：重复的那次在 `fetched` 里已经取不到，会拿空行把刚写进缓存的结果覆盖掉。
        if span_cache.get_ref(&entry.key).is_none() && !misses.contains(&entry.key) {
            if misses.is_empty() {
                misses = Vec::with_capacity(planned.len());
            }
            misses.push(entry.key.clone());
        }
    }
    if !misses.is_empty() {
        let mut fetched = database.query_exact_keys_per_key(&misses, TYPO_ROWS_PER_KEY);
        for key in misses {
            let key_rows = fetched.remove(&key).unwrap_or_default();
            span_cache.insert(key, key_rows);
        }
    }

    // 纠错键的查词页可能只有一两行；按缓存中的实际行数预留，避免把 96 个键的上限乘进返回缓冲。
    let edge_count = planned
        .iter()
        .filter_map(|entry| span_cache.get_ref(&entry.key))
        .fold(0usize, |count, rows| count.saturating_add(rows.len()));
    let mut edges = Vec::with_capacity(edge_count);
    for entry in planned {
        let Some(found) = span_cache.get_ref(&entry.key) else {
            continue;
        };
        edges.extend(found.iter().map(|row| TypoEdge {
            start: entry.start,
            end: entry.end,
            key: row.key.clone(),
            value: row.value.clone(),
            weight: row.weight,
            penalty: entry.penalty,
        }));
    }
    edges
}

/// 按弱位置优先规划并去重纠错跨度键，最多保留 `TYPO_KEY_BUDGET` 项。
fn plan_keys(
    profile: &PersonalTypoProfile,
    segments: &[String],
    literal_best: &SentencePath,
    autocorrect_types: u32,
) -> Vec<PlannedKey> {
    plan_keys_with(
        segments,
        literal_best,
        autocorrect_types,
        |position| syllable_typos(&segments[position]),
        |position, typo| profile.accepted(&segments[position], &typo.syllable),
    )
}

fn plan_keys_with<'t>(
    segments: &[String],
    literal_best: &SentencePath,
    autocorrect_types: u32,
    typos_at: impl Fn(usize) -> &'t [SyllableTypo],
    accepted: impl Fn(usize, &SyllableTypo) -> i32,
) -> Vec<PlannedKey> {
    let n = segments.len();
    // A syllable the best literal path spells as a lone character is where a typo most likely broke a phrase, so those positions are tried first. Word widths follow the character counts of the path's words; a path that does not tile the input marks nothing weak.
    let mut weak = WeakPositions::new(n);
    let mut covered = 0;
    for word in &literal_best.words {
        let width = count_utf8_chars(word);
        if width == 0 || covered + width > n {
            covered = n + 1;
            break;
        }
        if width == 1 {
            weak.set(covered);
        }
        covered += width;
    }
    if covered != n {
        weak.clear();
    }
    let positions = (0..n)
        .filter(|&i| weak.get(i))
        .chain((0..n).filter(|&i| !weak.get(i)));

    let mut planned = Vec::new();
    let mut variants = Vec::new();
    'positions: for position in positions {
        if planned.len() >= TYPO_KEY_BUDGET {
            break;
        }
        let typos = typos_at(position);
        variants.clear();
        variants.extend(
            typos
                .iter()
                .filter(|typo| autocorrect_types & autocorrect_bit(typo.kind) != 0)
                .map(|typo| (typo, accepted(position, typo))),
        );
        // Kinds are already cheapest first, so a stable sort on the personal count keeps that as the tie-break.
        variants.sort_by_key(|variant| std::cmp::Reverse(variant.1));
        for &(typo, accepted) in &variants {
            let penalty = discounted(base_cost(typo.kind), accepted);
            for length in MIN_TYPO_SPAN_SYLLABLES..=MAX_TYPO_SPAN_SYLLABLES.min(n) {
                let first = (position + 1).saturating_sub(length);
                for start in first..=position {
                    if start + length > n {
                        break;
                    }
                    let key = typo_span_key(
                        segments,
                        start,
                        start + length,
                        position - start,
                        &typo.syllable,
                    );
                    if planned_key_seen(&planned, &key) {
                        continue;
                    }
                    if planned.is_empty() {
                        planned = Vec::with_capacity(TYPO_KEY_BUDGET);
                    }
                    planned.push(PlannedKey {
                        start,
                        end: start + length,
                        key,
                        penalty,
                    });
                    if planned.len() >= TYPO_KEY_BUDGET {
                        break 'positions;
                    }
                }
            }
        }
    }
    planned
}

/// 按触点计过价的一个变体：第 i 个音节可能想打的音节，以及这次改动的罚分。
pub(crate) struct PricedTypo<'a> {
    pub syllable: &'a str,
    pub penalty: f64,
}

/// 双拼带触点时的规划与查词。`variants_at(i)` 给出第 i 个音节已经按纠错开关筛过、按罚分计过价的变体。与 [`collect_span_typo_edges`] 不同，所有位置的所有变体按罚分全局升序排，罚分相同时弱位置优先，所以 96 个键的预算不会被前一两个位置的便宜类型吃光；跨度、去重、查词和缓存相同。
pub(crate) fn collect_priced_span_typo_edges<'t>(
    database: &PinyinDatabase,
    span_cache: &mut FifoCache<String, Vec<DictRow>>,
    segments: &[String],
    literal_best: &SentencePath,
    variants_at: impl Fn(usize) -> &'t [PricedTypo<'t>],
) -> Vec<TypoEdge> {
    let planned = plan_priced_keys(segments, literal_best, variants_at);
    lookup_planned_edges(database, span_cache, &planned)
}

fn plan_priced_keys<'t>(
    segments: &[String],
    literal_best: &SentencePath,
    variants_at: impl Fn(usize) -> &'t [PricedTypo<'t>],
) -> Vec<PlannedKey> {
    let n = segments.len();
    // 弱位置的判定与 `plan_keys_with` 相同：最优字面路径里单独成字的音节最可能是按错打断了词。
    let mut weak = WeakPositions::new(n);
    let mut covered = 0;
    for word in &literal_best.words {
        let width = count_utf8_chars(word);
        if width == 0 || covered + width > n {
            covered = n + 1;
            break;
        }
        if width == 1 {
            weak.set(covered);
        }
        covered += width;
    }
    if covered != n {
        weak.clear();
    }
    let mut order: Vec<(f64, bool, usize, &PricedTypo<'t>)> = Vec::new();
    for position in 0..n {
        for typo in variants_at(position) {
            order.push((typo.penalty, !weak.get(position), position, typo));
        }
    }
    // 稳定排序：同价同强弱时按位置，同一位置内保持给出的次序。
    order.sort_by(|left, right| {
        left.0
            .total_cmp(&right.0)
            .then(left.1.cmp(&right.1))
            .then(left.2.cmp(&right.2))
    });

    let mut planned = Vec::new();
    'variants: for (penalty, _, position, typo) in order {
        for length in MIN_TYPO_SPAN_SYLLABLES..=MAX_TYPO_SPAN_SYLLABLES.min(n) {
            let first = (position + 1).saturating_sub(length);
            for start in first..=position {
                if start + length > n {
                    break;
                }
                let key = typo_span_key(
                    segments,
                    start,
                    start + length,
                    position - start,
                    typo.syllable,
                );
                if planned_key_seen(&planned, &key) {
                    continue;
                }
                if planned.is_empty() {
                    planned = Vec::with_capacity(TYPO_KEY_BUDGET);
                }
                planned.push(PlannedKey {
                    start,
                    end: start + length,
                    key,
                    penalty,
                });
                if planned.len() >= TYPO_KEY_BUDGET {
                    break 'variants;
                }
            }
        }
    }
    planned
}

fn planned_key_seen(planned: &[PlannedKey], key: &str) -> bool {
    planned.iter().any(|entry| entry.key == key)
}

/// 直接拼接替换后的短音节范围，避免先复制整个范围再改写一个位置。
fn typo_span_key(
    segments: &[String],
    start: usize,
    end: usize,
    replaced: usize,
    replacement: &str,
) -> String {
    debug_assert!(start < end);
    debug_assert!(replaced < end - start);
    let original_bytes = segments[start..end].iter().map(String::len).sum::<usize>();
    let capacity =
        original_bytes - segments[start + replaced].len() + replacement.len() + end - start - 1;
    let mut key = String::with_capacity(capacity);
    for (offset, segment) in segments[start..end].iter().enumerate() {
        if offset > 0 {
            key.push('\'');
        }
        if offset == replaced {
            key.push_str(replacement);
        } else {
            key.push_str(segment);
        }
    }
    key
}

#[cfg(test)]
#[path = "typo_edges/lazy_plan_tests.rs"]
mod lazy_plan_tests;

#[cfg(test)]
#[path = "typo_edges/lazy_buffer_tests.rs"]
mod lazy_buffer_tests;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn planning_reuses_the_variant_buffer_across_positions() {
        let directory = tempfile::tempdir().unwrap();
        let profile = PersonalTypoProfile::shared(&directory.path().join("journal.db"));
        let segments = vec!["zhuang".to_owned(); 6];
        let literal_best = SentencePath {
            sentence: "一二三四五六".to_owned(),
            key: "zhuang'zhuang'zhuang'zhuang'zhuang'zhuang".to_owned(),
            log_prob: 0.0,
            words: vec!["一".to_owned(); 6],
            typo_edges: 0,
        };

        let _ = syllable_typos("zhuang");
        let (planned, allocations) = crate::ime::personal_rerank::allocations::count(|| {
            plan_keys(
                &profile,
                &segments,
                &literal_best,
                crate::types::autocorrect_type::TRANSPOSITION
                    | crate::types::autocorrect_type::NEIGHBOR
                    | crate::types::autocorrect_type::MISSING_OR_EXTRA,
            )
        });

        assert_eq!(planned.len(), 20);
        assert!(allocations <= 91, "{allocations}");
    }

    fn resegmented(segments: &[&str]) -> Vec<(usize, usize, String, f64)> {
        let segments: Vec<String> = segments.iter().map(|&segment| segment.to_owned()).collect();
        let mut planned = Vec::new();
        plan_resegmented_keys(
            &segments,
            crate::types::autocorrect_type::TRANSPOSITION
                | crate::types::autocorrect_type::NEIGHBOR
                | crate::types::autocorrect_type::MISSING_OR_EXTRA,
            &mut planned,
        );
        planned
            .into_iter()
            .map(|entry| (entry.start, entry.end, entry.key, entry.penalty))
            .collect()
    }

    #[test]
    fn a_syllable_typed_as_several_legal_ones_is_planned_as_one() {
        // zai 打成 zia，读成 zi'a：盖住字面音节 1..3 的是一个音节，向左补上 xian 凑成词键。
        let planned = resegmented(&["xian", "zi", "a", "wo"]);
        assert!(
            planned.contains(&(0, 3, "xian'zai".to_owned(), TRANSPOSITION_COST_FOR_TEST)),
            "{planned:?}"
        );
        assert!(planned.contains(&(1, 4, "zai'wo".to_owned(), TRANSPOSITION_COST_FOR_TEST)));
        // 一个音节单独成不了词键，只和相邻的字面音节一起查。
        assert!(planned.iter().all(|(_, _, key, _)| key != "zai"));

        // xiao 打成 xioa，读成 xi'o'a：三个字面音节合成一个。
        let planned = resegmented(&["dian", "xi", "o", "a", "guo"]);
        assert!(planned.contains(&(1, 5, "xiao'guo".to_owned(), TRANSPOSITION_COST_FOR_TEST)));
        assert!(planned.contains(&(0, 4, "dian'xiao".to_owned(), TRANSPOSITION_COST_FOR_TEST)));
    }

    #[test]
    fn resegmented_keys_respect_the_switches_and_skip_one_to_one_swaps() {
        let segments: Vec<String> = ["xian", "zi", "a"].map(str::to_owned).to_vec();
        let mut planned = Vec::new();
        plan_resegmented_keys(
            &segments,
            crate::types::autocorrect_type::NEIGHBOR,
            &mut planned,
        );
        assert!(
            planned.iter().all(|entry| entry.key != "xian'zai"),
            "a transposition was planned with only the neighbour switch on"
        );
        // 音节数不变的读法交给 `plan_keys`，这里规划的每个键盖住的字面音节数都和它自己的音节数不同。
        for segments in [
            &["ni", "hao", "ma"][..],
            &["xian", "zi", "a", "wo"],
            &["dian", "xi", "o", "a", "guo"],
        ] {
            for (start, end, key, _) in resegmented(segments) {
                assert_ne!(
                    key.split('\'').count(),
                    end - start,
                    "{key} at {start}..{end}"
                );
            }
        }
    }

    const TRANSPOSITION_COST_FOR_TEST: f64 = crate::pinyin::typos::TRANSPOSITION_COST;

    fn path(words: &[&str]) -> SentencePath {
        SentencePath {
            sentence: words.concat(),
            key: String::new(),
            log_prob: 0.0,
            words: words.iter().map(|word| (*word).to_owned()).collect(),
            typo_edges: 0,
        }
    }

    #[test]
    fn priced_planning_orders_every_variant_by_penalty_then_weak_positions() {
        let segments: Vec<String> = ["ni", "hao", "ma", "ba"].map(str::to_owned).to_vec();
        let variants = [
            vec![PricedTypo {
                syllable: "li",
                penalty: 20.0,
            }],
            vec![PricedTypo {
                syllable: "hai",
                penalty: 12.0,
            }],
            vec![PricedTypo {
                syllable: "na",
                penalty: 12.0,
            }],
            vec![PricedTypo {
                syllable: "pa",
                penalty: 4.0,
            }],
        ];
        // 你好 是词，吗 和 吧 单独成字，是弱位置。
        let planned = plan_priced_keys(&segments, &path(&["你好", "吗", "吧"]), |position| {
            variants[position].as_slice()
        });
        let replaced: Vec<(usize, usize, &str, f64)> = planned
            .iter()
            .map(|entry| (entry.start, entry.end, entry.key.as_str(), entry.penalty))
            .collect();
        // 最便宜的 pa 先排（位置 3 在末尾，只有两种跨度）；同价 12 的弱位置 2（na）排在强位置 1（hai）前面；最贵的 li 最后。
        assert_eq!(
            replaced.iter().map(|entry| entry.2).collect::<Vec<_>>(),
            [
                "ma'pa",
                "hao'ma'pa",
                "hao'na",
                "na'ba",
                "ni'hao'na",
                "hao'na'ba",
                "ni'hai",
                "hai'ma",
                "ni'hai'ma",
                "hai'ma'ba",
                "li'hao",
                "li'hao'ma",
            ]
        );
        assert!(replaced.windows(2).all(|pair| pair[0].3 <= pair[1].3));
    }

    #[test]
    fn priced_planning_stops_at_the_key_budget() {
        // 各音节互不相同，换掉不同位置得到的跨度键也互不相同，只有预算能让规划停下。
        let segments: Vec<String> = (0..40).map(|index| format!("s{index}")).collect();
        let variants: Vec<PricedTypo<'static>> = ["chuang", "shuang", "zhang"]
            .into_iter()
            .map(|syllable| PricedTypo {
                syllable,
                penalty: 20.0,
            })
            .collect();
        let planned = plan_priced_keys(&segments, &path(&[]), |_| variants.as_slice());
        assert_eq!(planned.len(), TYPO_KEY_BUDGET);
    }

    #[test]
    fn weak_position_bits_cover_the_boundary_without_heap_state() {
        let (mut weak, allocations) =
            crate::ime::personal_rerank::allocations::count(|| WeakPositions::new(64));
        weak.set(0);
        weak.set(63);

        assert_eq!(allocations, 0);
        assert!(weak.get(0));
        assert!(weak.get(63));
        assert!(!weak.get(1));
    }

    #[test]
    fn weak_position_storage_falls_back_for_long_internal_inputs() {
        let mut weak = WeakPositions::new(65);
        weak.set(64);
        assert!(weak.get(64));
        weak.clear();
        assert!(!weak.get(64));
    }

    #[test]
    fn planned_key_lookup_scans_owned_keys() {
        let planned = vec![PlannedKey {
            start: 0,
            end: 2,
            key: "ni'hao".to_owned(),
            penalty: 1.0,
        }];
        assert!(planned_key_seen(&planned, "ni'hao"));
        assert!(!planned_key_seen(&planned, "ni'he"));
    }

    #[test]
    fn planned_key_scan_uses_no_temporary_heap_state() {
        let planned = vec![PlannedKey {
            start: 0,
            end: 2,
            key: "ni'hao".to_owned(),
            penalty: 1.0,
        }];
        let (found, allocations) = crate::ime::personal_rerank::allocations::count(|| {
            planned_key_seen(&planned, "ni'hao")
        });

        assert!(found);
        assert_eq!(allocations, 0);
    }

    #[test]
    fn typo_span_key_replaces_one_segment_without_a_temporary_vector() {
        let segments = ["ni".to_owned(), "hao".to_owned(), "ma".to_owned()];

        let (key, allocations) = crate::ime::personal_rerank::allocations::count(|| {
            typo_span_key(&segments, 0, 3, 1, "he")
        });

        assert_eq!(key, "ni'he'ma");
        assert_eq!(key.capacity(), key.len());
        assert_eq!(allocations, 1);
    }

    #[test]
    fn typo_span_keys_match_cloned_ranges_at_every_replacement_position() {
        let segments = [
            "ni".to_owned(),
            "hao".to_owned(),
            "ma".to_owned(),
            "ba".to_owned(),
        ];
        for start in 0..segments.len() {
            for end in start + 1..=segments.len() {
                for replaced in 0..end - start {
                    for replacement in ["", "he", "shang"] {
                        let mut expected = segments[start..end].to_vec();
                        expected[replaced] = replacement.to_owned();
                        let key = typo_span_key(&segments, start, end, replaced, replacement);
                        assert_eq!(key, expected.join("'"));
                        assert_eq!(key.capacity(), key.len());
                    }
                }
            }
        }
    }
}
