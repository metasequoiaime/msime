//! Typo edges for the lattice's typo sentence (quanpin.md §10.5, QD:1056-1182): legal-to-legal syllable swaps, weak positions first, priced by kind and discounted by how often the user accepted that typo.

use std::collections::{HashMap, HashSet};

use crate::cache::FifoCache;
use crate::dictionary::pinyin::PinyinDatabase;
use crate::dictionary::DictRow;
use crate::lattice::{SentencePath, TypoEdge};
use crate::pinyin::segment::join_segments;
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

/// Plan up to 96 span keys, look them up through `span_cache` (empty answers cached too), and emit one edge per row.
pub fn collect_typo_edges(
    database: &PinyinDatabase,
    span_cache: &mut FifoCache<String, Vec<DictRow>>,
    profile: &PersonalTypoProfile,
    segments: &[String],
    literal_best: &SentencePath,
    autocorrect_types: u32,
) -> Vec<TypoEdge> {
    let planned = plan_keys(profile, segments, literal_best, autocorrect_types);

    let mut rows: HashMap<String, Vec<DictRow>> = HashMap::with_capacity(planned.len());
    let mut misses = Vec::with_capacity(planned.len());
    for entry in &planned {
        match span_cache.get(&entry.key) {
            Some(cached) => {
                rows.insert(entry.key.clone(), cached);
            }
            None => misses.push(entry.key.clone()),
        }
    }
    if !misses.is_empty() {
        let mut fetched = database.query_exact_keys_per_key(&misses, TYPO_ROWS_PER_KEY);
        for key in misses {
            let key_rows = fetched.remove(&key).unwrap_or_default();
            span_cache.insert(key.clone(), key_rows.clone());
            rows.insert(key, key_rows);
        }
    }

    let mut edges = Vec::with_capacity(planned.len() * TYPO_ROWS_PER_KEY);
    for entry in &planned {
        let Some(found) = rows.get(&entry.key) else {
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

/// Every span key the typo decode may use, weak positions first, deduplicated, at most `TYPO_KEY_BUDGET`.
fn plan_keys(
    profile: &PersonalTypoProfile,
    segments: &[String],
    literal_best: &SentencePath,
    autocorrect_types: u32,
) -> Vec<PlannedKey> {
    let n = segments.len();
    // A syllable the best literal path spells as a lone character is where a typo most likely broke a phrase, so those positions are tried first. Word widths follow the character counts of the path's words; a path that does not tile the input marks nothing weak.
    let mut weak = vec![false; n];
    let mut covered = 0;
    for word in &literal_best.words {
        let width = count_utf8_chars(word);
        if width == 0 || covered + width > n {
            covered = n + 1;
            break;
        }
        if width == 1 {
            weak[covered] = true;
        }
        covered += width;
    }
    if covered != n {
        weak.fill(false);
    }
    let positions = (0..n)
        .filter(|&i| weak[i])
        .chain((0..n).filter(|&i| !weak[i]));

    let mut planned = Vec::with_capacity(TYPO_KEY_BUDGET);
    let mut planned_keys = HashSet::with_capacity(TYPO_KEY_BUDGET);
    'positions: for position in positions {
        if planned.len() >= TYPO_KEY_BUDGET {
            break;
        }
        let typed = &segments[position];
        let typos = syllable_typos(typed);
        let mut variants = Vec::with_capacity(typos.len());
        variants.extend(
            typos
                .iter()
                .filter(|typo| autocorrect_types & autocorrect_bit(typo.kind) != 0)
                .map(|typo| (typo, profile.accepted(typed, &typo.syllable))),
        );
        // Kinds are already cheapest first, so a stable sort on the personal count keeps that as the tie-break.
        variants.sort_by_key(|variant| std::cmp::Reverse(variant.1));
        for (typo, accepted) in variants {
            let penalty = discounted(base_cost(typo.kind), accepted);
            for length in MIN_TYPO_SPAN_SYLLABLES..=MAX_TYPO_SPAN_SYLLABLES.min(n) {
                let first = (position + 1).saturating_sub(length);
                for start in first..=position {
                    if start + length > n {
                        break;
                    }
                    let mut span = segments[start..start + length].to_vec();
                    span[position - start] = typo.syllable.clone();
                    let key = join_segments(&span);
                    if !planned_keys.insert(key.clone()) {
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

#[cfg(test)]
fn contains_planned_key(planned: &[PlannedKey], key: &str) -> bool {
    planned.iter().any(|entry| entry.key == key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn planned_key_lookup_scans_owned_keys() {
        let planned = vec![PlannedKey {
            start: 0,
            end: 2,
            key: "ni'hao".to_owned(),
            penalty: 1.0,
        }];
        assert!(contains_planned_key(&planned, "ni'hao"));
        assert!(!contains_planned_key(&planned, "ni'he"));
    }
}
