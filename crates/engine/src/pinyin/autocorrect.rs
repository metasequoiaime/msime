//! Quanpin autocorrect (quanpin.md §7.1-§7.4). The static correction tables are generated at first use rather than ported from `autocorrect_table.h`: each syllable of the intact list yields its one-edit variants under four rules (swap two adjacent letters, replace a letter with a QWERTY neighbour, drop a letter, insert a repeat or neighbour of an adjacent letter), and every variant of three to six letters that is not itself a syllable becomes a `(wrong, correct)` row of that rule's table. These are the rules of the reference build's table generator, which the `GEN` citations below give by line; it left the tree with the C++ engine. A unit test pins the per-type row counts (807 / 4442 / 340 / 11939) and rows against a fixture dumped from that header.

use std::collections::{BTreeSet, HashMap, HashSet};
use std::sync::OnceLock;

use super::active_helpcode::strip_active_helpcodes;
use super::syllables::{intact_piece, intact_pinyin_list, is_intact, MAX_SYLLABLE_LENGTH};
use crate::types::autocorrect_type;

/// Tie-break weights; the corrected-edge count is always the primary key (QUH:73-84).
pub const TRANSPOSITION_WEIGHT: i32 = 10;
pub const DELETION_WEIGHT: i32 = 11;
pub const INSERTION_WEIGHT: i32 = 12;
pub const NEIGHBOR_WEIGHT: i32 = 13;
pub const MAX_AUTOCORRECT_EDGES: usize = 3;
pub const MAX_AUTOCORRECT_INPUT_LENGTH: usize = 64;

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AutocorrectCutSegment {
    pub syllable: String,
    /// The typed letters this syllable was read from.
    pub raw_text: String,
    pub start: usize,
    pub corrected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct AutocorrectCut {
    pub segments: Vec<AutocorrectCutSegment>,
    pub edge_count: usize,
    pub weight: i32,
}

impl AutocorrectCut {
    pub fn is_empty(&self) -> bool {
        self.segments.is_empty()
    }

    pub fn same_cost_as(&self, other: &AutocorrectCut) -> bool {
        self.edge_count == other.edge_count && self.weight == other.weight
    }

    pub fn into_syllables(self) -> Vec<String> {
        self.segments
            .into_iter()
            .map(|segment| segment.syllable)
            .collect()
    }
}

/// QWERTY letter-key neighbours, the generator's finger-movement table (GEN:36-63), indexed by `letter - b'a'`.
const QWERTY_NEIGHBORS: [&[u8]; 26] = [
    b"qwsz",   // a
    b"vghn",   // b
    b"xdfv",   // c
    b"serfcx", // d
    b"wrsd",   // e
    b"drtgvc", // f
    b"ftyhbv", // g
    b"gyujnb", // h
    b"uojk",   // i
    b"huikmn", // j
    b"jiolm",  // k
    b"kop",    // l
    b"njk",    // m
    b"bhjm",   // n
    b"ipkl",   // o
    b"ol",     // p
    b"wa",     // q
    b"etdf",   // r
    b"awdexz", // s
    b"ryfg",   // t
    b"yihj",   // u
    b"cfgb",   // v
    b"qesa",   // w
    b"zsdc",   // x
    b"tugh",   // y
    b"asx",    // z
];

fn neighbors(letter: u8) -> &'static [u8] {
    if letter.is_ascii_lowercase() {
        QWERTY_NEIGHBORS[usize::from(letter - b'a')]
    } else {
        &[]
    }
}

fn insertion_letters_capacity(syllable: &[u8], position: usize) -> usize {
    let mut capacity = 0;
    if position > 0 {
        capacity += 1 + neighbors(syllable[position - 1]).len();
    }
    if position < syllable.len() {
        capacity += 1 + neighbors(syllable[position]).len();
    }
    capacity
}

/// Every one-edit variant of `syllable` a rule produces, duplicates included (GEN:176-224).
fn rule_variants(syllable: &[u8], correction_type: u32) -> Vec<Vec<u8>> {
    let capacity = match correction_type {
        autocorrect_type::TRANSPOSITION => syllable.len().saturating_sub(1),
        autocorrect_type::NEIGHBOR => syllable.iter().map(|&letter| neighbors(letter).len()).sum(),
        autocorrect_type::DELETION => syllable.len(),
        autocorrect_type::INSERTION => (0..=syllable.len())
            .map(|position| insertion_letters_capacity(syllable, position))
            .sum(),
        _ => 0,
    };
    let mut variants = Vec::with_capacity(capacity);
    match correction_type {
        autocorrect_type::TRANSPOSITION => {
            for i in 0..syllable.len() - 1 {
                let mut swapped = syllable.to_vec();
                swapped.swap(i, i + 1);
                variants.push(swapped);
            }
        }
        autocorrect_type::NEIGHBOR => {
            for (i, &letter) in syllable.iter().enumerate() {
                for &neighbor in neighbors(letter) {
                    let mut substituted = syllable.to_vec();
                    substituted[i] = neighbor;
                    variants.push(substituted);
                }
            }
        }
        autocorrect_type::DELETION => {
            for i in 0..syllable.len() {
                let mut shortened = syllable.to_vec();
                shortened.remove(i);
                variants.push(shortened);
            }
        }
        autocorrect_type::INSERTION => {
            // A realistic extra key repeats an adjacent letter or is one of its neighbours; unconstrained insertion would add some 60k mostly implausible keys.
            for position in 0..=syllable.len() {
                let mut letters =
                    Vec::with_capacity(insertion_letters_capacity(syllable, position));
                if position > 0 {
                    let left = syllable[position - 1];
                    letters.push(left);
                    letters.extend_from_slice(neighbors(left));
                }
                if position < syllable.len() {
                    let right = syllable[position];
                    letters.push(right);
                    letters.extend_from_slice(neighbors(right));
                }
                for letter in letters {
                    let mut lengthened = syllable.to_vec();
                    lengthened.insert(position, letter);
                    variants.push(lengthened);
                }
            }
        }
        other => panic!("not a single autocorrect type bit: {other}"),
    }
    variants
}

/// One table as the generator emits it (GEN:141-171, 270-285): every `(wrong, correct)` pair, sorted. A wrong key keeps every syllable it can come from, for the runtime to settle by word frequency. Two-letter strings are jianpin space and never keys.
fn generate_table(correction_type: u32) -> Vec<(String, String)> {
    let mut pairs = BTreeSet::new();
    for &syllable in intact_pinyin_list() {
        if syllable.len() < 2 {
            continue;
        }
        for variant in rule_variants(syllable.as_bytes(), correction_type) {
            if variant == syllable.as_bytes()
                || !(3..=MAX_SYLLABLE_LENGTH).contains(&variant.len())
                || variant.contains(&b'\'')
            {
                continue;
            }
            let Ok(variant) = String::from_utf8(variant) else {
                continue;
            };
            if !is_intact(&variant) {
                pairs.insert((variant, syllable.to_owned()));
            }
        }
    }
    pairs.into_iter().collect()
}

/// `(wrong, correct)` rows of one correction type, in the generator's sorted row order, which is the runtime's final tie-break. `correction_type` must be exactly one of the four `autocorrect_type` bits; anything else is a caller bug and panics.
pub fn correction_table(correction_type: u32) -> &'static [(String, String)] {
    static TRANSPOSITION: OnceLock<Vec<(String, String)>> = OnceLock::new();
    static NEIGHBOR: OnceLock<Vec<(String, String)>> = OnceLock::new();
    static DELETION: OnceLock<Vec<(String, String)>> = OnceLock::new();
    static INSERTION: OnceLock<Vec<(String, String)>> = OnceLock::new();
    let table = match correction_type {
        autocorrect_type::TRANSPOSITION => &TRANSPOSITION,
        autocorrect_type::NEIGHBOR => &NEIGHBOR,
        autocorrect_type::DELETION => &DELETION,
        autocorrect_type::INSERTION => &INSERTION,
        other => panic!("not a single autocorrect type bit: {other}"),
    };
    table.get_or_init(|| generate_table(correction_type))
}

#[derive(Debug, Clone, Copy)]
struct CorrectionTarget {
    syllable: &'static str,
    /// Every type whose table offers this pair; the cost comes from the enabled subset at query time.
    type_bits: u32,
}

/// The cheapest weight among the enabled types a pair carries, so switching off a cheaper table lets the surviving one set the cost (QU:585-597).
fn min_enabled_correction_weight(enabled_bits: u32) -> i32 {
    [
        (autocorrect_type::TRANSPOSITION, TRANSPOSITION_WEIGHT),
        (autocorrect_type::DELETION, DELETION_WEIGHT),
        (autocorrect_type::INSERTION, INSERTION_WEIGHT),
        (autocorrect_type::NEIGHBOR, NEIGHBOR_WEIGHT),
    ]
    .into_iter()
    .filter(|(bit, _)| enabled_bits & bit != 0)
    .map(|(_, weight)| weight)
    .min()
    .unwrap_or(i32::MAX)
}

/// Wrong key to its targets (QU:599-637). The tables are merged in weight order (transposition, deletion, insertion, neighbour), so each target list is weight-sorted with row order breaking ties; a pair two tables share ORs its bits.
fn correction_index() -> &'static HashMap<&'static str, Vec<CorrectionTarget>> {
    static INDEX: OnceLock<HashMap<&'static str, Vec<CorrectionTarget>>> = OnceLock::new();
    INDEX.get_or_init(|| {
        let mut index: HashMap<&'static str, Vec<CorrectionTarget>> = HashMap::new();
        for type_bit in [
            autocorrect_type::TRANSPOSITION,
            autocorrect_type::DELETION,
            autocorrect_type::INSERTION,
            autocorrect_type::NEIGHBOR,
        ] {
            for (wrong, correct) in correction_table(type_bit) {
                let targets = index.entry(wrong.as_str()).or_default();
                match targets.iter_mut().find(|target| target.syllable == correct) {
                    Some(target) => target.type_bits |= type_bit,
                    None => targets.push(CorrectionTarget {
                        syllable: correct.as_str(),
                        type_bits: type_bit,
                    }),
                }
            }
        }
        index
    })
}

const NO_PREDECESSOR: usize = usize::MAX;
const SMALL_FINALIZE_BEAM: usize = 36;

/// One propagated cut hypothesis, ranked by (edge_count, weight, arrival): fewest corrections, then summed correction weight, then generation order (positions ascending, pieces longest first, table order).
#[derive(Debug, Clone, Copy)]
struct Hypothesis {
    edge_count: usize,
    /// Index into the predecessor position's finalised list, which never changes once its out-edges are relaxed.
    prev_index: usize,
    arrival: usize,
    weight: i32,
    /// Typed letters the edge consumed: one fewer than the syllable for a deletion, one more for an insertion.
    raw_length: usize,
    syllable: &'static str,
    corrected: bool,
    /// Interned id of the syllable sequence so far; equal ids mean equal sequences. The C++ used a rolling hash.
    sequence: usize,
}

struct Search {
    best: Vec<Vec<Hypothesis>>,
    arrival: usize,
    /// `(parent sequence, syllable) -> sequence`; the seed's empty sequence is 0.
    sequences: HashMap<(usize, &'static str), usize>,
    k: usize,
}

impl Search {
    fn new(length: usize, k: usize) -> Self {
        Self {
            best: (0..=length).map(|_| Vec::with_capacity(k)).collect(),
            arrival: 0,
            sequences: HashMap::with_capacity(length.saturating_mul(k)),
            k,
        }
    }

    fn finalize(&mut self, position: usize) {
        let list = &mut self.best[position];
        if list.len() < 2 {
            return;
        }
        list.sort_by_key(|hypothesis| {
            (hypothesis.edge_count, hypothesis.weight, hypothesis.arrival)
        });
        // The same syllables reached over different raw spans: keep the best-ranked one.
        if list.len() <= SMALL_FINALIZE_BEAM {
            let mut write = 0;
            for read in 0..list.len() {
                if list[..write]
                    .iter()
                    .any(|hypothesis| hypothesis.sequence == list[read].sequence)
                {
                    continue;
                }
                if write != read {
                    list.swap(write, read);
                }
                write += 1;
            }
            list.truncate(write);
            list.truncate(self.k);
            return;
        }
        let mut seen = HashSet::with_capacity(list.len());
        list.retain(|hypothesis| seen.insert(hypothesis.sequence));
        list.truncate(self.k);
    }

    fn extend(
        &mut self,
        length: usize,
        start: usize,
        raw_length: usize,
        syllable: &'static str,
        corrected: bool,
        edge_weight: i32,
    ) {
        let end = start + raw_length;
        for parent_index in 0..self.best[start].len() {
            let parent = self.best[start][parent_index];
            let edge_count = parent.edge_count + usize::from(corrected);
            if edge_count > MAX_AUTOCORRECT_EDGES {
                continue;
            }
            // A legal-only reading of the whole input is the caller's plain segmentation; dropping it on arrival also keeps k = 1 consistent with larger k.
            if end == length && edge_count == 0 {
                continue;
            }
            let next_sequence = self.sequences.len() + 1;
            let sequence = *self
                .sequences
                .entry((parent.sequence, syllable))
                .or_insert(next_sequence);
            let arrival = self.arrival;
            self.arrival += 1;
            self.best[end].push(Hypothesis {
                edge_count,
                prev_index: parent_index,
                arrival,
                weight: parent.weight + edge_weight,
                raw_length,
                syllable,
                corrected,
                sequence,
            });
        }
    }
}

/// The k best cuts that contain at least one and at most three corrections, sorted by (edges, weight, arrival) and deduplicated by syllable sequence (QU:683-848). Empty for `k == 0`, no types, empty input, more than 64 letters, or any `'`.
pub fn autocorrect_cut_kbest(
    pinyin: &str,
    autocorrect_types: u32,
    k: usize,
) -> Vec<AutocorrectCut> {
    if k == 0
        || autocorrect_types == 0
        || pinyin.is_empty()
        || pinyin.len() > MAX_AUTOCORRECT_INPUT_LENGTH
        || pinyin.contains('\'')
    {
        return Vec::new();
    }
    let index = correction_index();
    let bytes = pinyin.as_bytes();
    let length = bytes.len();
    let mut search = Search::new(length, k);
    search.best[0].push(Hypothesis {
        edge_count: 0,
        prev_index: NO_PREDECESSOR,
        arrival: 0,
        weight: 0,
        raw_length: 0,
        syllable: "",
        corrected: false,
        sequence: 0,
    });
    // Every edge consumes at least one letter, so one left-to-right sweep finalises each position before relaxing its out-edges.
    for start in 0..length {
        search.finalize(start);
        if search.best[start].is_empty() {
            continue;
        }
        for raw_length in (1..=(length - start).min(MAX_SYLLABLE_LENGTH)).rev() {
            let piece = &bytes[start..start + raw_length];
            // A legal piece is never also read as a correction key.
            if let Some(syllable) = intact_piece(piece) {
                search.extend(length, start, raw_length, syllable, false, 0);
                continue;
            }
            let Some(targets) = std::str::from_utf8(piece)
                .ok()
                .and_then(|piece| index.get(piece))
            else {
                continue;
            };
            for target in targets {
                let enabled_bits = autocorrect_types & target.type_bits;
                if enabled_bits == 0 {
                    continue;
                }
                search.extend(
                    length,
                    start,
                    raw_length,
                    target.syllable,
                    true,
                    min_enabled_correction_weight(enabled_bits),
                );
            }
        }
    }
    search.finalize(length);

    search.best[length]
        .iter()
        .map(|hypothesis| {
            let mut segments = Vec::with_capacity(length);
            let mut position = length;
            let mut current = *hypothesis;
            while current.prev_index != NO_PREDECESSOR {
                let start = position - current.raw_length;
                segments.push(AutocorrectCutSegment {
                    syllable: current.syllable.to_owned(),
                    raw_text: pinyin[start..position].to_owned(),
                    start,
                    corrected: current.corrected,
                });
                position = start;
                current = search.best[position][current.prev_index];
            }
            segments.reverse();
            AutocorrectCut {
                segments,
                edge_count: hypothesis.edge_count,
                weight: hypothesis.weight,
            }
        })
        .collect()
}

/// The best cut, or `None` (QU:850-856).
pub fn autocorrect_cut_detail(pinyin: &str, autocorrect_types: u32) -> Option<AutocorrectCut> {
    autocorrect_cut_kbest(pinyin, autocorrect_types, 1)
        .into_iter()
        .next()
}

/// Greedy intact syllables then at most one trailing consonant (QU:187-243): `zheg` is protected from correction, `gau` stays eligible for `gua`.
pub fn looks_like_syllable_with_jianpin_tail(pinyin: &str) -> bool {
    // Manual delimiters mark boundaries the user chose and never reach correction.
    if pinyin.is_empty() || pinyin.contains('\'') {
        return false;
    }
    let bytes = pinyin.as_bytes();
    let mut position = 0;
    while position < bytes.len() {
        let longest = MAX_SYLLABLE_LENGTH.min(bytes.len() - position);
        let Some(matched) = (1..=longest)
            .rev()
            .find(|&length| intact_piece(&bytes[position..position + length]).is_some())
        else {
            break;
        };
        position += matched;
    }
    // All-consonant strings match nothing and stay correctable on purpose: there is no multi-letter jianpin, so `bqng` can only mean `bang`.
    if position == 0 || bytes.len() - position > 1 {
        return false;
    }
    if position == bytes.len() {
        return true;
    }
    // One trailing letter is jianpin intent only as an initial (`zhe` + `g`); a lone vowel after a syllable (`ga` + `u`) is far likelier a transposition of `gua`.
    !matches!(bytes[position], b'a' | b'e' | b'i' | b'o' | b'u' | b'v')
}

/// The per-input key autocorrect suppression is recorded under: the helpcode-stripped raw input, lowercased, without `'` (TP:180-189).
pub fn autocorrect_suppression_key(raw_input: &str, raw_input_with_cases: &str) -> String {
    strip_active_helpcodes(raw_input, raw_input_with_cases)
        .chars()
        .filter(|&character| character != '\'')
        .map(|character| character.to_ascii_lowercase())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::autocorrect_type::{DELETION, INSERTION, NEIGHBOR, TRANSPOSITION};

    /// test_typo_correction_input_session.cpp:237-253: every neighbour row is one substitution of an adjacent key, by the same adjacency the typo learning uses (`typos::keys_adjacent`), and the two keyboard tables agree in both directions.
    #[test]
    fn neighbor_rows_agree_with_keys_adjacent() {
        for (wrong, correct) in correction_table(NEIGHBOR) {
            let (w, c) = (wrong.as_bytes(), correct.as_bytes());
            assert_eq!(w.len(), c.len(), "{wrong} -> {correct}");
            let diffs: Vec<usize> = (0..w.len()).filter(|&i| w[i] != c[i]).collect();
            assert_eq!(diffs.len(), 1, "{wrong} -> {correct}");
            assert!(
                crate::pinyin::typos::keys_adjacent(w[diffs[0]], c[diffs[0]]),
                "{wrong} -> {correct}"
            );
        }
        for a in b'a'..=b'z' {
            for b in b'a'..=b'z' {
                assert_eq!(
                    neighbors(a).contains(&b),
                    crate::pinyin::typos::keys_adjacent(a, b),
                    "{} {}",
                    a as char,
                    b as char
                );
            }
        }
    }

    const BOTH: u32 = TRANSPOSITION | NEIGHBOR;
    const ALL: u32 = BOTH | DELETION;
    const ALL_FOUR: u32 = ALL | INSERTION;

    /// The rows of `R/quanpin/autocorrect_table.h`, dumped per table as `wrong correct` lines under a `# <type> <count>` header.
    const FIXTURE: &str = include_str!("autocorrect_table_fixture.txt");

    fn fixture_tables() -> HashMap<String, Vec<(String, String)>> {
        let mut tables: HashMap<String, Vec<(String, String)>> = HashMap::new();
        let mut current = String::new();
        for line in FIXTURE.lines() {
            if let Some(header) = line.strip_prefix("# ") {
                current = header.split(' ').next().unwrap_or_default().to_owned();
                continue;
            }
            let (wrong, correct) = line.split_once(' ').expect("fixture row");
            tables
                .entry(current.clone())
                .or_default()
                .push((wrong.to_owned(), correct.to_owned()));
        }
        tables
    }

    #[test]
    fn search_allocates_only_position_beams() {
        let search = Search::new(4, 3);
        assert_eq!(search.best.len(), 5);
        assert!(search.best.iter().all(|slot| slot.capacity() >= 3));
        assert!(search.sequences.capacity() >= 12);
    }

    #[test]
    fn finalize_uses_bounded_dedup_state_for_position_beams() {
        let mut search = Search::new(1, 9);
        search.best[1] = (0..36)
            .map(|index| Hypothesis {
                edge_count: index % 3,
                prev_index: 0,
                arrival: index,
                weight: index as i32,
                raw_length: 1,
                syllable: "shi",
                corrected: true,
                sequence: index % 18,
            })
            .collect();

        let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
            search.finalize(1);
        });

        assert_eq!(allocations, 0);
        assert_eq!(search.best[1].len(), 9);
    }

    fn reading(cut: &AutocorrectCut) -> String {
        cut.segments
            .iter()
            .map(|segment| segment.syllable.as_str())
            .collect::<Vec<_>>()
            .join("'")
    }

    #[test]
    fn into_syllables_consumes_the_cut_segments() {
        let cut = AutocorrectCut {
            segments: vec![segment("shang", "sahng", 0, true)],
            edge_count: 1,
            weight: TRANSPOSITION_WEIGHT,
        };

        assert_eq!(cut.into_syllables(), ["shang"]);
    }

    /// `autocorrect_cut` of the C++ tests: the best cut's syllables.
    fn cut(pinyin: &str, types: u32) -> String {
        autocorrect_cut_detail(pinyin, types)
            .map(|cut| reading(&cut))
            .unwrap_or_default()
    }

    fn readings(pinyin: &str, types: u32) -> Vec<String> {
        autocorrect_cut_kbest(pinyin, types, 9)
            .iter()
            .map(reading)
            .collect()
    }

    #[test]
    fn generated_tables_match_the_reference_header_row_for_row() {
        let fixture = fixture_tables();
        for (name, type_bit, count) in [
            ("transposition", TRANSPOSITION, 807),
            ("neighbor", NEIGHBOR, 4442),
            ("deletion", DELETION, 340),
            ("insertion", INSERTION, 11939),
        ] {
            let generated = correction_table(type_bit);
            assert_eq!(generated.len(), count, "{name} row count");
            assert_eq!(generated, fixture[name].as_slice(), "{name} rows");
        }
    }

    // test_pinyin.cpp:998-1024.
    #[test]
    fn generated_tables_hold_their_invariants() {
        let mut pairs = HashSet::new();
        let mut total = 0;
        for type_bit in [TRANSPOSITION, NEIGHBOR, DELETION, INSERTION] {
            for (wrong, correct) in correction_table(type_bit) {
                assert!(
                    wrong.len() >= 3,
                    "2-letter keys belong to the jianpin space: {wrong}"
                );
                assert!(!is_intact(wrong), "a key shadows a legal syllable: {wrong}");
                assert!(
                    is_intact(correct),
                    "a target is not a legal syllable: {correct}"
                );
                assert!(
                    pairs.insert((wrong, correct)),
                    "pair repeated across tables: {wrong} {correct}"
                );
                total += 1;
            }
        }
        assert!(total >= 4700);
    }

    #[test]
    fn shared_pairs_cost_the_cheapest_enabled_type() {
        assert_eq!(
            min_enabled_correction_weight(TRANSPOSITION | NEIGHBOR),
            TRANSPOSITION_WEIGHT
        );
        assert_eq!(
            min_enabled_correction_weight(NEIGHBOR | INSERTION),
            INSERTION_WEIGHT
        );
        assert_eq!(min_enabled_correction_weight(NEIGHBOR), NEIGHBOR_WEIGHT);
        let targets = &correction_index()["ahan"];
        let syllables: Vec<_> = targets.iter().map(|target| target.syllable).collect();
        assert_eq!(syllables[..2], ["shan", "zhan"]);
    }

    // test_pinyin.cpp:751-771.
    #[test]
    fn each_switch_enables_only_its_own_family() {
        assert_eq!(cut("sahng", 0), "");
        assert_eq!(cut("sahng", TRANSPOSITION), "shang");
        assert_eq!(cut("sahng", NEIGHBOR), "");
        assert_eq!(cut("sahng", BOTH), "shang");
        assert_eq!(cut("shabg", 0), "");
        assert_eq!(cut("shabg", TRANSPOSITION), "");
        assert_eq!(cut("shabg", NEIGHBOR), "shang");
        assert_eq!(cut("shabg", BOTH), "shang");
    }

    // test_pinyin.cpp:773-801, 864-869.
    #[test]
    fn the_jianpin_shape_guard() {
        assert!(looks_like_syllable_with_jianpin_tail("zheg"));
        assert!(looks_like_syllable_with_jianpin_tail("keneng"));
        assert!(looks_like_syllable_with_jianpin_tail("zher"));
        assert!(!looks_like_syllable_with_jianpin_tail("sahng"));
        assert!(!looks_like_syllable_with_jianpin_tail("shabg"));
        assert!(!looks_like_syllable_with_jianpin_tail("xi'an"));
        assert!(!looks_like_syllable_with_jianpin_tail("wj"));
        assert!(!looks_like_syllable_with_jianpin_tail("bqng"));
        assert!(!looks_like_syllable_with_jianpin_tail("gau"));
        assert!(!looks_like_syllable_with_jianpin_tail("hau"));
        assert!(!looks_like_syllable_with_jianpin_tail(""));
        assert_eq!(cut("gau", TRANSPOSITION), "gua");
        assert_eq!(cut("bqng", NEIGHBOR), "bang");
        assert_eq!(cut("zheg", BOTH), "");
        // The insertion key `zher` is in the search space by design; the guard is the dictionary layer's job.
        assert!(autocorrect_cut_detail("zher", INSERTION).is_some());
    }

    // test_pinyin.cpp:803-862.
    #[test]
    fn deletion_and_insertion_bits_gate_their_tables() {
        assert_eq!(cut("shng", DELETION), "shang");
        assert_eq!(cut("shng", TRANSPOSITION), "");
        assert_eq!(cut("shng", NEIGHBOR), "");
        // Bits gate tables, not intents: deletion alone explains `sahng` as sa + hng.
        assert_eq!(cut("sahng", DELETION), "sa'hang");
        assert_eq!(cut("shabg", DELETION), "");
        assert_eq!(cut("shngzhk", ALL), "shang'zhi");
        assert_eq!(cut("shngzhk", BOTH), "");
        assert_eq!(cut("shangg", INSERTION), "shang");
        assert_eq!(cut("shangg", TRANSPOSITION), "");
        assert_eq!(cut("shangg", DELETION), "");
        assert_eq!(cut("shangg", NEIGHBOR), "");
        assert_eq!(cut("sjhang", INSERTION), "shang");
        assert_eq!(cut("sjhang", NEIGHBOR), "");
        assert_eq!(cut("shanggzhi", ALL_FOUR), "shang'zhi");
    }

    // test_pinyin.cpp:871-930.
    #[test]
    fn kbest_orders_by_edges_weight_and_table_order() {
        let ahan = readings("ahan", BOTH);
        assert!(
            ahan.len() >= 2 && ahan[0] == "shan" && ahan[1] == "zhan",
            "{ahan:?}"
        );
        let shng = readings("shng", ALL);
        assert!(
            shng.len() >= 2 && shng[0] == "shang" && shng.contains(&"sheng".to_owned()),
            "{shng:?}"
        );
        let sahng = readings("sahng", ALL);
        assert!(
            sahng.len() >= 2 && sahng[0] == "shang" && sahng[1] == "sa'hang",
            "{sahng:?}"
        );
        let gau = autocorrect_cut_kbest("gau", BOTH, 9);
        assert_eq!(reading(&gau[0]), "gua");
        let gai = gau
            .iter()
            .find(|cut| reading(cut) == "gai")
            .expect("gai kept");
        assert!(!gau[0].same_cost_as(gai) && gau[0].weight < gai.weight);
        assert_eq!(gau[0].weight, TRANSPOSITION_WEIGHT);
        assert_eq!(gai.weight, NEIGHBOR_WEIGHT);
        let baio = readings("baio", ALL_FOUR);
        assert!(
            baio.len() >= 3 && baio[..3] == ["biao", "bai", "bao"],
            "{baio:?}"
        );
        for (input, types) in [
            ("sahng", BOTH),
            ("shabg", BOTH),
            ("sahnguai", BOTH),
            ("ahan", BOTH),
            ("zheg", BOTH),
            ("keneng", BOTH),
            ("shng", ALL),
            ("zhngu", ALL),
            ("shangg", INSERTION),
            ("sjhang", ALL_FOUR),
        ] {
            let best = autocorrect_cut_kbest(input, types, 1);
            let wider = autocorrect_cut_kbest(input, types, 9);
            assert_eq!(best.len(), usize::from(!wider.is_empty()), "{input}");
            assert_eq!(
                best.first(),
                wider.first(),
                "k = 1 must agree with larger k for {input}"
            );
        }
    }

    #[test]
    fn kbest_rejects_what_the_contract_excludes() {
        assert!(autocorrect_cut_kbest("sahng", BOTH, 0).is_empty());
        assert!(autocorrect_cut_kbest("sahng", 0, 9).is_empty());
        assert!(autocorrect_cut_kbest("", BOTH, 9).is_empty());
        assert!(autocorrect_cut_kbest("sa'hng", BOTH, 9).is_empty());
        assert!(autocorrect_cut_kbest(&"sahng".repeat(13), ALL_FOUR, 9).is_empty());
        assert!(autocorrect_cut_kbest("keneng", BOTH, 1).is_empty());
        assert!(autocorrect_cut_kbest("sahng你", ALL_FOUR, 9).is_empty());
        // More than three corrections is never offered.
        assert!(autocorrect_cut_kbest("sahngsahngsahngsahng", BOTH, 9).is_empty());
        assert_eq!(cut("sahngsahngsahng", BOTH), "shang'shang'shang");
        let cuts = autocorrect_cut_kbest("sahnghao", ALL_FOUR, 9);
        let mut seen = HashSet::new();
        assert!(
            cuts.iter().all(|cut| seen.insert(reading(cut))),
            "sequences are unique"
        );
        assert!(cuts
            .iter()
            .all(|cut| (1..=MAX_AUTOCORRECT_EDGES).contains(&cut.edge_count)));
        assert!(cuts.windows(2).all(
            |pair| (pair[0].edge_count, pair[0].weight) <= (pair[1].edge_count, pair[1].weight)
        ));
    }

    fn segment(
        syllable: &str,
        raw_text: &str,
        start: usize,
        corrected: bool,
    ) -> AutocorrectCutSegment {
        AutocorrectCutSegment {
            syllable: syllable.to_owned(),
            raw_text: raw_text.to_owned(),
            start,
            corrected,
        }
    }

    fn segments_of(pinyin: &str, types: u32) -> Vec<AutocorrectCutSegment> {
        autocorrect_cut_detail(pinyin, types)
            .map(|cut| cut.segments)
            .unwrap_or_default()
    }

    // test_pinyin.cpp:1085-1170.
    #[test]
    fn cut_segments_carry_their_raw_spans() {
        assert_eq!(
            segments_of("sahng", BOTH),
            [segment("shang", "sahng", 0, true)]
        );
        assert_eq!(
            segments_of("shabg", BOTH),
            [segment("shang", "shabg", 0, true)]
        );
        assert_eq!(
            segments_of("sahnghao", BOTH),
            [
                segment("shang", "sahng", 0, true),
                segment("hao", "hao", 5, false)
            ]
        );
        assert!(autocorrect_cut_detail("zheg", BOTH).is_none());
        assert!(autocorrect_cut_detail("keneng", BOTH).is_none());
        assert!(autocorrect_cut_detail("sahng", 0).is_none());
        assert!(autocorrect_cut_detail("xi'an", BOTH).is_none());
        // Deletion edges consume one letter fewer than the syllable, so the next segment starts where the raw letters end.
        assert_eq!(
            segments_of("zhnggu", ALL),
            [
                segment("zhang", "zhng", 0, true),
                segment("gu", "gu", 4, false)
            ]
        );
        let zhngu = segments_of("zhngu", ALL);
        assert_eq!(zhngu.len(), 2);
        assert!(zhngu[0].raw_text == "zhn" && zhngu[0].corrected);
        assert_eq!(zhngu[1], segment("gu", "gu", 3, false));
        assert_eq!(
            segments_of("shangg", ALL_FOUR),
            [segment("shang", "shangg", 0, true)]
        );
        // Within one edge the cheaper deletion reading (11) beats the insertion reading (12).
        assert_eq!(
            segments_of("shangghao", ALL_FOUR),
            [
                segment("shang", "shang", 0, false),
                segment("gao", "ghao", 5, true)
            ]
        );
        assert_eq!(
            segments_of("shanggni", ALL_FOUR),
            [
                segment("shang", "shangg", 0, true),
                segment("ni", "ni", 6, false)
            ]
        );
    }

    #[test]
    fn suppression_keys_fold_the_stripped_input() {
        assert_eq!(autocorrect_suppression_key("sahng", "saHng"), "sahng");
        assert_eq!(autocorrect_suppression_key("sa'hng", "sa'hng"), "sahng");
        // An active helpcode letter is not part of the key.
        assert_eq!(autocorrect_suppression_key("shangs", "shangS"), "shang");
        assert_eq!(autocorrect_suppression_key("nihc", "nihC"), "nihc");
    }
}
