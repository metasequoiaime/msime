//! Legal-to-legal syllable typos for the lattice's typo sentence and for typo learning (quanpin.md §7.9). This keyboard adjacency is not the generator's neighbour table; both are kept.

use std::sync::OnceLock;

use super::syllables::{intact_pinyin_list, is_intact, normalized_syllable};
use crate::types::autocorrect_type;

/// Cheapest first; the order is the sort key.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub enum SyllableTypoKind {
    Transposition,
    Neighbor,
    MissingOrExtra,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SyllableTypo {
    /// The syllable the user probably meant.
    pub syllable: String,
    pub kind: SyllableTypoKind,
}

/// Lattice edge penalties per kind before the personal discount (WLH:145-178).
pub const TRANSPOSITION_COST: f64 = 20.0;
pub const NEIGHBOR_COST: f64 = 20.0;
pub const MISSING_OR_EXTRA_COST: f64 = 24.0;

const KEYBOARD_ROWS: [&[u8]; 3] = [b"qwertyuiop", b"asdfghjkl", b"zxcvbnm"];

fn key_position(key: u8) -> Option<(usize, usize)> {
    KEYBOARD_ROWS.iter().enumerate().find_map(|(row, keys)| {
        keys.iter()
            .position(|&candidate| candidate == key)
            .map(|column| (row, column))
    })
}

/// Keeps `candidate` when it is a legal syllable of two or more letters that is not the typed one; a variant reached twice keeps its cheaper kind (ST:37-54).
fn add_variant(
    variants: &mut Vec<SyllableTypo>,
    typed: &str,
    candidate: &[u8],
    kind: SyllableTypoKind,
) {
    let Ok(candidate) = std::str::from_utf8(candidate) else {
        return;
    };
    if candidate.len() < 2 || !is_intact(candidate) {
        return;
    }
    let intended = normalized_syllable(candidate);
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

/// ST:56-99：按原始拼写编辑，以规范拼写比较变体与输入，排除只是别名不同的同一音节；复用调用方提供的缓冲。
fn compute_typos_into(syllable: &str, variants: &mut Vec<SyllableTypo>, edited: &mut Vec<u8>) {
    variants.clear();
    let typed = normalized_syllable(syllable);
    if typed.len() < 2 {
        return;
    }
    let letters = syllable.as_bytes();
    if edited.capacity() < letters.len() + 1 {
        edited.reserve(letters.len() + 1 - edited.len());
    }
    for i in 0..letters.len() - 1 {
        if letters[i] == letters[i + 1] {
            continue;
        }
        edited.clear();
        edited.extend_from_slice(letters);
        edited.swap(i, i + 1);
        add_variant(variants, &typed, edited, SyllableTypoKind::Transposition);
    }
    for i in 0..letters.len() {
        for key in b'a'..=b'z' {
            if key == letters[i] || !keys_adjacent(key, letters[i]) {
                continue;
            }
            edited.clear();
            edited.extend_from_slice(letters);
            edited[i] = key;
            add_variant(variants, &typed, edited, SyllableTypoKind::Neighbor);
        }
    }
    for i in 0..letters.len() {
        edited.clear();
        edited.extend_from_slice(letters);
        edited.remove(i);
        add_variant(variants, &typed, edited, SyllableTypoKind::MissingOrExtra);
    }
    for i in 0..=letters.len() {
        for key in b'a'..=b'z' {
            edited.clear();
            edited.extend_from_slice(letters);
            edited.insert(i, key);
            add_variant(variants, &typed, edited, SyllableTypoKind::MissingOrExtra);
        }
    }
    variants.sort_by_key(|variant| variant.kind);
}

/// ST:56-99：按原始拼写编辑，以规范拼写比较变体与输入，排除只是别名不同的同一音节。
#[cfg(test)]
fn compute_typos(syllable: &str) -> Vec<SyllableTypo> {
    let typed = normalized_syllable(syllable);
    if typed.len() < 2 {
        return Vec::new();
    }
    let letters = syllable.as_bytes();
    let neighbor_capacity = letters
        .iter()
        .map(|&letter| {
            (b'a'..=b'z')
                .filter(|&key| keys_adjacent(key, letter))
                .count()
        })
        .sum::<usize>();
    let capacity = letters.len().saturating_sub(1)
        + neighbor_capacity
        + letters.len()
        + letters.len().saturating_add(1).saturating_mul(26);
    let mut variants = Vec::with_capacity(capacity);
    // 所有编辑从原字节恢复；多出的一个字节容纳插入，循环中不重新申请存储。
    let mut edited = Vec::with_capacity(letters.len() + 1);
    compute_typos_into(syllable, &mut variants, &mut edited);
    variants
}

struct CompactSyllableTypoTable {
    /// 与 `intact_pinyin_list()` 一一对应的偏移表。
    offsets: Box<[u32]>,
    /// 所有合法音节的一步编辑变体连续平铺，避免 421 个过量预留的 `Vec` 长期滞留堆内存。
    items: Box<[SyllableTypo]>,
}

impl CompactSyllableTypoTable {
    fn build() -> Self {
        let syllables = intact_pinyin_list();
        let mut offsets = Vec::with_capacity(syllables.len() + 1);
        let mut items = Vec::new();
        let mut scratch = Vec::with_capacity(32);
        let mut edited = Vec::with_capacity(8);
        offsets.push(0);
        for &syllable in syllables {
            compute_typos_into(syllable, &mut scratch, &mut edited);
            items.append(&mut scratch);
            offsets.push(u32::try_from(items.len()).expect("音节错键条目数超出 u32 范围"));
        }
        Self {
            offsets: offsets.into_boxed_slice(),
            items: items.into_boxed_slice(),
        }
    }

    fn get(&self, typed: &str) -> &[SyllableTypo] {
        let Ok(index) = intact_pinyin_list().binary_search(&typed) else {
            return &[];
        };
        let start = self.offsets[index] as usize;
        let end = self.offsets[index + 1] as usize;
        &self.items[start..end]
    }
}

/// The precomputed variants of an intact syllable, cheapest kind first; empty for unknown input (ST:56-110).
pub fn syllable_typos(typed: &str) -> &'static [SyllableTypo] {
    static TABLE: OnceLock<CompactSyllableTypoTable> = OnceLock::new();
    TABLE
        .get_or_init(CompactSyllableTypoTable::build)
        .get(typed)
}

/// The kind of single edit that turns `typed` into `intended`, if both are intact and one edit apart.
pub fn syllable_typo_kind(typed: &str, intended: &str) -> Option<SyllableTypoKind> {
    syllable_typos(typed)
        .iter()
        .find(|typo| typo.syllable == intended)
        .map(|typo| typo.kind)
}

/// Same row neighbours, or diagonal neighbours on adjacent rows with the half-key shift (ST:145-162).
pub fn keys_adjacent(a: u8, b: u8) -> bool {
    if a == b {
        return false;
    }
    let (Some(first), Some(second)) = (key_position(a), key_position(b)) else {
        return false;
    };
    let ((upper_row, upper_column), (lower_row, lower_column)) = if first.0 <= second.0 {
        (first, second)
    } else {
        (second, first)
    };
    if upper_row == lower_row {
        return upper_column.abs_diff(lower_column) == 1;
    }
    // Each lower row is shifted half a key right, so key j sits below keys j and j + 1 of the row above.
    lower_row == upper_row + 1 && (upper_column == lower_column || upper_column == lower_column + 1)
}

/// The autocorrect type bits a kind belongs to (ST:131-143).
pub fn autocorrect_bit(kind: SyllableTypoKind) -> u32 {
    match kind {
        SyllableTypoKind::Transposition => autocorrect_type::TRANSPOSITION,
        SyllableTypoKind::Neighbor => autocorrect_type::NEIGHBOR,
        SyllableTypoKind::MissingOrExtra => autocorrect_type::MISSING_OR_EXTRA,
    }
}

pub fn base_cost(kind: SyllableTypoKind) -> f64 {
    match kind {
        SyllableTypoKind::Transposition => TRANSPOSITION_COST,
        SyllableTypoKind::Neighbor => NEIGHBOR_COST,
        SyllableTypoKind::MissingOrExtra => MISSING_OR_EXTRA_COST,
    }
}

/// `base - (base / 5) * min(ln(1 + accepted), 3)` for a positive count (TP:173-178).
pub fn discounted(base: f64, accepted: i32) -> f64 {
    if accepted <= 0 {
        return base;
    }
    base - (base / 5.0) * f64::from(accepted).ln_1p().min(3.0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// test_typo_correction_input_session.cpp:338-347: the discount never rises with more accepted typos and settles on its 8.0 floor.
    #[test]
    fn discount_is_monotonic_down_to_its_floor() {
        let mut previous = discounted(20.0, 0);
        assert_eq!(previous, 20.0);
        for accepted in 1..=2000 {
            let current = discounted(20.0, accepted);
            assert!(current <= previous && current >= 8.0 - 1e-9, "{accepted}");
            previous = current;
        }
        assert!(previous < 8.0 + 1e-9);
    }

    #[test]
    fn keyboard_adjacency_includes_the_staggered_diagonals() {
        assert!(keys_adjacent(b'q', b'w'));
        assert!(keys_adjacent(b'w', b'q'));
        assert!(!keys_adjacent(b'q', b'e'));
        // `a` sits below `q` and `w`.
        assert!(keys_adjacent(b'q', b'a'));
        assert!(keys_adjacent(b'w', b'a'));
        assert!(!keys_adjacent(b'e', b'a'));
        assert!(keys_adjacent(b'a', b'z'));
        assert!(keys_adjacent(b's', b'z'));
        assert!(!keys_adjacent(b'q', b'z'));
        assert!(!keys_adjacent(b'a', b'a'));
        assert!(!keys_adjacent(b'a', b'1'));
    }

    #[test]
    fn typos_are_legal_normalised_and_cheapest_first() {
        let guan = syllable_typos("guan");
        assert!(guan.windows(2).all(|pair| pair[0].kind <= pair[1].kind));
        assert!(guan
            .iter()
            .all(|typo| is_intact(&typo.syllable) && typo.syllable != "guan"));
        assert_eq!(
            syllable_typo_kind("gan", "guan"),
            Some(SyllableTypoKind::MissingOrExtra)
        );
        assert_eq!(
            syllable_typo_kind("guan", "gan"),
            Some(SyllableTypoKind::MissingOrExtra)
        );
        assert_eq!(syllable_typo_kind("gua", "gau"), None);
        assert_eq!(syllable_typo_kind("hua", "hau"), None);
        assert_eq!(
            syllable_typo_kind("nei", "nie"),
            Some(SyllableTypoKind::Transposition)
        );
        assert_eq!(
            syllable_typo_kind("bing", "ning"),
            Some(SyllableTypoKind::Neighbor)
        );
        assert_eq!(syllable_typo_kind("ni", "hao"), None);
        // Variants carry the dictionary spelling.
        assert!(syllable_typos("le")
            .iter()
            .any(|typo| typo.syllable == "lve"));
        assert!(!syllable_typos("le")
            .iter()
            .any(|typo| typo.syllable == "lue"));
        // Both spellings are table keys; their edits differ because they start from different letters.
        assert!(!syllable_typos("lue").is_empty() && !syllable_typos("lve").is_empty());
        assert!(syllable_typos("a").is_empty());
        assert!(syllable_typos("xyz").is_empty());
    }

    #[test]
    fn a_variant_reached_twice_is_listed_once() {
        // `lue` and `lve` both normalise to `lve`, so `le` reaches it through two insertions.
        for &syllable in intact_pinyin_list() {
            let typos = syllable_typos(syllable);
            for (index, typo) in typos.iter().enumerate() {
                assert!(
                    typos[index + 1..]
                        .iter()
                        .all(|other| other.syllable != typo.syllable),
                    "{syllable}"
                );
            }
        }
        assert_eq!(
            syllable_typos("le")
                .iter()
                .filter(|typo| typo.syllable == "lve")
                .count(),
            1
        );
    }

    #[test]
    fn costs_and_bits() {
        assert_eq!(
            autocorrect_bit(SyllableTypoKind::Transposition),
            autocorrect_type::TRANSPOSITION
        );
        assert_eq!(
            autocorrect_bit(SyllableTypoKind::Neighbor),
            autocorrect_type::NEIGHBOR
        );
        assert_eq!(
            autocorrect_bit(SyllableTypoKind::MissingOrExtra),
            autocorrect_type::MISSING_OR_EXTRA
        );
        assert_eq!(base_cost(SyllableTypoKind::MissingOrExtra), 24.0);
        assert_eq!(discounted(20.0, 0), 20.0);
        assert_eq!(discounted(20.0, -3), 20.0);
        assert!((discounted(20.0, 1) - (20.0 - 4.0 * 2f64.ln())).abs() < 1e-12);
        // The discount is capped at three fifths of the base.
        assert!((discounted(20.0, 1000) - 8.0).abs() < 1e-12);
    }

    #[test]
    fn compact_syllable_typo_table_matches_compute_typos_for_all_syllables() {
        for &syllable in intact_pinyin_list() {
            assert_eq!(
                syllable_typos(syllable),
                compute_typos(syllable).as_slice(),
                "{syllable:?}"
            );
        }
        for invalid in ["", "a", "xyz"] {
            assert!(syllable_typos(invalid).is_empty(), "{invalid:?}");
        }
    }
}

#[cfg(test)]
#[path = "typos/edit_buffer_tests.rs"]
mod edit_buffer_tests;
