//! Typo edges for the lattice's typo sentence (quanpin.md §10.5, QD:1056-1182): legal-to-legal syllable swaps, weak positions first, priced by kind and discounted by how often the user accepted that typo.

use crate::cache::FifoCache;
use crate::dictionary::pinyin::PinyinDatabase;
use crate::dictionary::DictRow;
use crate::lattice::{SentencePath, TypoEdge};
use crate::pinyin::typos::{autocorrect_bit, base_cost, discounted, syllable_typos};
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
    let planned = plan_keys(profile, segments, literal_best, autocorrect_types);

    let mut misses = Vec::new();
    for entry in &planned {
        if span_cache.get_ref(&entry.key).is_none() {
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

    let mut edges = Vec::new();
    for entry in &planned {
        let Some(found) = span_cache.get_ref(&entry.key) else {
            continue;
        };
        if edges.is_empty() && !found.is_empty() {
            edges = Vec::with_capacity(planned.len() * TYPO_ROWS_PER_KEY);
        }
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

/// Every span key the typo decode may use, weak positions first, deduplicated, at most `TYPO_KEY_BUDGET`.
fn plan_keys(
    profile: &PersonalTypoProfile,
    segments: &[String],
    literal_best: &SentencePath,
    autocorrect_types: u32,
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

    let mut planned = Vec::with_capacity(TYPO_KEY_BUDGET);
    let mut variants = Vec::new();
    'positions: for position in positions {
        if planned.len() >= TYPO_KEY_BUDGET {
            break;
        }
        let typed = &segments[position];
        let typos = syllable_typos(typed);
        variants.clear();
        variants.extend(
            typos
                .iter()
                .filter(|typo| autocorrect_types & autocorrect_bit(typo.kind) != 0)
                .map(|typo| (typo, profile.accepted(typed, &typo.syllable))),
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
