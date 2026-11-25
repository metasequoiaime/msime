//! Quanpin autocorrect (quanpin.md §7.1-§7.4). The static correction tables are generated at first use rather than ported from `autocorrect_table.h`: each syllable of the intact list yields its one-edit variants under four rules (swap two adjacent letters, replace a letter with a QWERTY neighbour, drop a letter, insert a repeat or neighbour of an adjacent letter), and every variant of three to six letters that is not itself a syllable becomes a `(wrong, correct)` row of that rule's table. These are the rules of the reference build's table generator, which the `GEN` citations below give by line; it left the tree with the C++ engine. A unit test pins the per-type row counts (807 / 4442 / 340 / 11939) and rows against a fixture dumped from that header.

#[cfg(test)]
use std::collections::BTreeSet;
use std::collections::{HashMap, HashSet};
use std::sync::OnceLock;

use super::active_helpcode::strip_active_helpcodes;
#[cfg(test)]
use super::syllables::is_intact;
use super::syllables::{intact_piece, intact_pinyin_list, is_prefix_piece, MAX_SYLLABLE_LENGTH};
use crate::types::autocorrect_type;

/// Tie-break weights; the corrected-edge count is always the primary key (QUH:73-84).
pub const TRANSPOSITION_WEIGHT: i32 = 10;
pub const DELETION_WEIGHT: i32 = 11;
pub const INSERTION_WEIGHT: i32 = 12;
/// A repeated adjacent key is a common physical-keyboard slip and should win over a generic extra-letter correction.
pub const REPEATED_INSERTION_WEIGHT: i32 = 10;
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

/// 将 2 到 `MAX_SYLLABLE_LENGTH` 个小写 ASCII 字母按 MSB-first 5 bit 打包为 `u32`，与 ASCII 字典序严格同构。
fn pack_letters(bytes: &[u8]) -> Option<u32> {
    if !(2..=MAX_SYLLABLE_LENGTH).contains(&bytes.len()) {
        return None;
    }
    let mut packed = 0u32;
    for (i, &byte) in bytes.iter().enumerate() {
        if !byte.is_ascii_lowercase() {
            return None;
        }
        packed |= u32::from(byte - b'a' + 1) << (25 - 5 * i);
    }
    Some(packed)
}

/// 在栈缓冲区上枚举 `syllable` 经单步编辑规则 `correction_type` 产生的所有变体（含重复项），避免中间堆分配（GEN:176-224）。
fn for_each_rule_variant(syllable: &[u8], correction_type: u32, mut visit: impl FnMut(&[u8])) {
    let len = syllable.len();
    debug_assert!(len <= MAX_SYLLABLE_LENGTH);
    let mut buf = [0u8; MAX_SYLLABLE_LENGTH + 1];
    match correction_type {
        autocorrect_type::TRANSPOSITION => {
            if len < 2 {
                return;
            }
            buf[..len].copy_from_slice(syllable);
            for i in 0..len - 1 {
                buf.swap(i, i + 1);
                visit(&buf[..len]);
                buf.swap(i, i + 1);
            }
        }
        autocorrect_type::NEIGHBOR => {
            buf[..len].copy_from_slice(syllable);
            for (i, &letter) in syllable.iter().enumerate() {
                for &neighbor in neighbors(letter) {
                    buf[i] = neighbor;
                    visit(&buf[..len]);
                }
                buf[i] = letter;
            }
        }
        autocorrect_type::DELETION => {
            if len == 0 {
                return;
            }
            for i in 0..len {
                buf[..i].copy_from_slice(&syllable[..i]);
                buf[i..len - 1].copy_from_slice(&syllable[i + 1..]);
                visit(&buf[..len - 1]);
            }
        }
        autocorrect_type::INSERTION => {
            // 真实的误触多键通常是重复相邻字母或碰到了相邻键；无约束插入会额外引入约 6 万条低置信错键。
            for position in 0..=len {
                buf[..position].copy_from_slice(&syllable[..position]);
                buf[position + 1..len + 1].copy_from_slice(&syllable[position..]);
                if position > 0 {
                    let left = syllable[position - 1];
                    buf[position] = left;
                    visit(&buf[..len + 1]);
                    for &neighbor in neighbors(left) {
                        buf[position] = neighbor;
                        visit(&buf[..len + 1]);
                    }
                }
                if position < len {
                    let right = syllable[position];
                    buf[position] = right;
                    visit(&buf[..len + 1]);
                    for &neighbor in neighbors(right) {
                        buf[position] = neighbor;
                        visit(&buf[..len + 1]);
                    }
                }
            }
        }
        other => panic!("not a single autocorrect type bit: {other}"),
    }
}

/// 按参考生成器的产出方式生成单类纠错规则的排序 `(wrong, correct)` 列表（GEN:141-171, 270-285）；仅保留三到六字母且非合法音节的错键。
///
/// 读不出来的两字母串另有一张表，见 `two_letter_table`。
#[cfg(test)]
fn generate_table(correction_type: u32) -> Vec<(String, String)> {
    let mut pairs = BTreeSet::new();
    for &syllable in intact_pinyin_list() {
        if syllable.len() < 2 {
            continue;
        }
        let syllable_bytes = syllable.as_bytes();
        for_each_rule_variant(syllable_bytes, correction_type, |variant| {
            if variant == syllable_bytes
                || !(3..=MAX_SYLLABLE_LENGTH).contains(&variant.len())
                || variant.contains(&b'\'')
            {
                return;
            }
            if let Ok(variant_str) = std::str::from_utf8(variant) {
                if !is_intact(variant_str) {
                    pairs.insert((variant_str.to_owned(), syllable.to_owned()));
                }
            }
        });
    }
    pairs.into_iter().collect()
}

/// `(wrong, correct)` rows of one correction type, in the generator's sorted row order, which is the runtime's final tie-break. `correction_type` must be exactly one of the four `autocorrect_type` bits; anything else is a caller bug and panics.
#[cfg(test)]
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

/// 两个字母的串能否按简拼或音节前缀读出来：整串是某个音节的前缀（`zh`、`ba`），或两个字母各自是音节的开头（`nh` 读作 n'h）。
fn two_letters_readable(letters: &[u8]) -> bool {
    is_prefix_piece(letters) || (is_prefix_piece(&letters[..1]) && is_prefix_piece(&letters[1..]))
}

/// 两字母纠错键（#10124）：两字母音节换一个邻键或调换两个字母，得到的串既不是音节、又按简拼和音节前缀都读不出来时，记一行 `(错键, 音节)`。只覆盖邻键和乱序两类，按行序排好。
///
/// 参考生成器只收三到六个字母的错键，因为两字母串属于简拼（`nh`、`zh`、`wd`），把它们当错键会抢走简拼。但 `wi`、`hi`、`ia` 这类串里有 `i`/`u`/`v` 这种不能开始音节的字母，既不是简拼也不是任何音节的前缀，整段输入因此一个读法都没有：`wiaini` 想打 `woaini`，本地候选是空的。这里只收这种读不出来的串，所以不会和简拼争读法；固定参考表的那四张表和它们的行数不变。
#[cfg(test)]
fn two_letter_table(correction_type: u32) -> &'static [(String, String)] {
    static TRANSPOSITION: OnceLock<Vec<(String, String)>> = OnceLock::new();
    static NEIGHBOR: OnceLock<Vec<(String, String)>> = OnceLock::new();
    let table = match correction_type {
        autocorrect_type::TRANSPOSITION => &TRANSPOSITION,
        autocorrect_type::NEIGHBOR => &NEIGHBOR,
        _ => return &[],
    };
    table.get_or_init(|| {
        let mut pairs = BTreeSet::new();
        for &syllable in intact_pinyin_list() {
            if syllable.len() != 2 {
                continue;
            }
            for_each_rule_variant(syllable.as_bytes(), correction_type, |variant| {
                if variant.len() != 2 || two_letters_readable(variant) {
                    return;
                }
                if let Ok(variant_str) = std::str::from_utf8(variant) {
                    pairs.insert((variant_str.to_owned(), syllable.to_owned()));
                }
            });
        }
        pairs.into_iter().collect()
    })
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CompactTarget {
    syllable_index: u16,
    /// 该错键映射所属的全部纠错类型位掩码，查询时按启用的子集取最低权重。
    type_bits: u8,
}

impl CompactTarget {
    fn syllable(self) -> &'static str {
        intact_pinyin_list()[usize::from(self.syllable_index)]
    }

    fn type_bits(self) -> u32 {
        u32::from(self.type_bits)
    }
}

/// 扁平 CSR 纠错索引：`keys` 为严格升序的打包错键，`offsets` 切片指向 `targets` 中按权重与表序排好的目标。
struct CompactCorrectionIndex {
    keys: Vec<u32>,
    offsets: Vec<u32>,
    targets: Vec<CompactTarget>,
}

impl CompactCorrectionIndex {
    fn get(&self, piece: &[u8]) -> Option<&[CompactTarget]> {
        let packed = pack_letters(piece)?;
        let idx = self.keys.binary_search(&packed).ok()?;
        let start = self.offsets[idx] as usize;
        let end = self.offsets[idx + 1] as usize;
        Some(&self.targets[start..end])
    }
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

/// 判断字节串 `wrong` 是否由 `correct` 重复某个相邻字母得到。
fn is_repeated_insertion_bytes(wrong: &[u8], correct: &[u8]) -> bool {
    if wrong.len() != correct.len() + 1 {
        return false;
    }
    (0..=correct.len()).any(|position| {
        wrong[..position] == correct[..position]
            && wrong[position + 1..] == correct[position..]
            && ((position > 0 && wrong[position] == correct[position - 1])
                || (position < correct.len() && wrong[position] == correct[position]))
    })
}

/// 判断 `wrong` 是否由 `correct` 重复某个相邻字母得到。
#[cfg(test)]
fn is_repeated_insertion(wrong: &str, correct: &str) -> bool {
    is_repeated_insertion_bytes(wrong.as_bytes(), correct.as_bytes())
}

fn correction_weight_bytes(wrong: &[u8], correct: &str, enabled_bits: u32) -> i32 {
    if enabled_bits & autocorrect_type::INSERTION != 0
        && is_repeated_insertion_bytes(wrong, correct.as_bytes())
    {
        REPEATED_INSERTION_WEIGHT
    } else {
        min_enabled_correction_weight(enabled_bits)
    }
}

#[cfg(test)]
fn correction_weight(wrong: &str, correct: &str, enabled_bits: u32) -> i32 {
    correction_weight_bytes(wrong.as_bytes(), correct, enabled_bits)
}

/// 错键到候选目标的紧凑索引（QU:599-637，不依赖 `correction_table` / `two_letter_table` 常驻表）；按权重顺序（乱序、漏键、多键、邻键）合并，重复映射按位或合入 `type_bits`。
fn correction_index() -> &'static CompactCorrectionIndex {
    static INDEX: OnceLock<CompactCorrectionIndex> = OnceLock::new();
    INDEX.get_or_init(|| {
        let syllables = intact_pinyin_list();
        let mut per_type: Vec<(u32, u16)> = Vec::with_capacity(16_384);
        let mut all_rows: Vec<(u32, u8, u16, u8)> = Vec::with_capacity(20_480);

        for (type_rank, type_bit) in [
            autocorrect_type::TRANSPOSITION,
            autocorrect_type::DELETION,
            autocorrect_type::INSERTION,
            autocorrect_type::NEIGHBOR,
        ]
        .into_iter()
        .enumerate()
        {
            per_type.clear();
            let include_two_letter = matches!(
                type_bit,
                autocorrect_type::TRANSPOSITION | autocorrect_type::NEIGHBOR
            );
            for (syllable_index, &syllable) in syllables.iter().enumerate() {
                if syllable.len() < 2 {
                    continue;
                }
                let syllable_bytes = syllable.as_bytes();
                let syllable_idx =
                    u16::try_from(syllable_index).expect("音节表条目数不超过 u16::MAX");
                let collect_two = include_two_letter && syllable.len() == 2;
                for_each_rule_variant(syllable_bytes, type_bit, |variant| {
                    if (3..=MAX_SYLLABLE_LENGTH).contains(&variant.len()) {
                        if variant != syllable_bytes && intact_piece(variant).is_none() {
                            if let Some(packed) = pack_letters(variant) {
                                per_type.push((packed, syllable_idx));
                            }
                        }
                    } else if collect_two && variant.len() == 2 && !two_letters_readable(variant) {
                        if let Some(packed) = pack_letters(variant) {
                            per_type.push((packed, syllable_idx));
                        }
                    }
                });
            }
            per_type.sort_unstable();
            per_type.dedup();
            let rank = u8::try_from(type_rank).expect("纠错规则数量固定为 4");
            let bit = u8::try_from(type_bit).expect("纠错位掩码不超过 u8::MAX");
            for &(packed, syllable_idx) in &per_type {
                all_rows.push((packed, rank, syllable_idx, bit));
            }
        }
        drop(per_type);

        // 按 `(packed, type_rank, syllable_idx)` 排序，既使 `keys` 严格有序，又保证单键目标顺序与四张表按权重顺序依次并入完全一致。
        all_rows
            .sort_unstable_by_key(|&(packed, rank, syllable_idx, _)| (packed, rank, syllable_idx));

        let mut keys = Vec::with_capacity(16_384);
        let mut offsets = Vec::with_capacity(16_385);
        let mut targets: Vec<CompactTarget> = Vec::with_capacity(all_rows.len());

        let mut cursor = 0;
        while cursor < all_rows.len() {
            let key = all_rows[cursor].0;
            let group_start = targets.len();
            keys.push(key);
            offsets.push(u32::try_from(group_start).expect("纠错目标数不超过 u32::MAX"));
            while cursor < all_rows.len() && all_rows[cursor].0 == key {
                let (_, _, syllable_index, type_bit) = all_rows[cursor];
                match targets[group_start..]
                    .iter_mut()
                    .find(|target| target.syllable_index == syllable_index)
                {
                    Some(existing) => existing.type_bits |= type_bit,
                    None => targets.push(CompactTarget {
                        syllable_index,
                        type_bits: type_bit,
                    }),
                }
                cursor += 1;
            }
        }
        offsets.push(u32::try_from(targets.len()).expect("纠错目标数不超过 u32::MAX"));
        drop(all_rows);

        keys.shrink_to_fit();
        offsets.shrink_to_fit();
        targets.shrink_to_fit();
        CompactCorrectionIndex {
            keys,
            offsets,
            targets,
        }
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
            best: (0..=length).map(|_| Vec::new()).collect(),
            arrival: 0,
            sequences: HashMap::new(),
            k,
        }
    }

    fn finalize(&mut self, position: usize) {
        let list = &mut self.best[position];
        if list.len() < 2 {
            return;
        }
        let rank = |hypothesis: &Hypothesis| {
            (hypothesis.edge_count, hypothesis.weight, hypothesis.arrival)
        };
        if self.k == 1 {
            // 只取首选时直接扫描最低成本，省去排序和去重。
            list[0] = *list
                .iter()
                .min_by_key(|hypothesis| rank(hypothesis))
                .unwrap();
            list.truncate(1);
            return;
        }
        // arrival 全局唯一，完整排序键没有平局；原地排序保持原有排名且不分配。
        list.sort_unstable_by_key(rank);
        // 去重状态只保留最终需要的结果，常用 k=9 即使入束很大也无需哈希表。
        let mut seen = (list.len().min(self.k) > SMALL_FINALIZE_BEAM)
            .then(|| HashSet::with_capacity(list.len().min(self.k)));
        let mut write = 0;
        for read in 0..list.len() {
            let sequence = list[read].sequence;
            let duplicate = match &mut seen {
                Some(seen) => !seen.insert(sequence),
                None => list[..write]
                    .iter()
                    .any(|hypothesis| hypothesis.sequence == sequence),
            };
            if duplicate {
                continue;
            }
            if write != read {
                list.swap(write, read);
            }
            write += 1;
            if write == self.k {
                break;
            }
        }
        list.truncate(write);
    }

    /// 整条读法用到了两字母纠错键，却没有一个照原样打对的音节时为真。两个字母本身的证据太少：单独的 `hi`、`wi` 和 `wifi` 这类整串都要改的输入多半是英文或别的意思，只有旁边有打对的音节（`wiaini` 的 `ai`、`ni`）才说明这一段是拼音打错了。只在到达末尾时沿前驱链查一次，前面各位置的列表定稿后不再变化。
    fn short_keys_unsupported(
        &self,
        start: usize,
        parent_index: usize,
        raw_length: usize,
        corrected: bool,
    ) -> bool {
        let mut short_key = corrected && raw_length == 2;
        let mut literal = !corrected;
        let mut position = start;
        let mut current = self.best[position][parent_index];
        while current.prev_index != NO_PREDECESSOR {
            short_key |= current.corrected && current.raw_length == 2;
            literal |= !current.corrected;
            position -= current.raw_length;
            current = self.best[position][current.prev_index];
        }
        short_key && !literal
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
            if end == length
                && self.short_keys_unsupported(start, parent_index, raw_length, corrected)
            {
                continue;
            }
            if self.sequences.is_empty() {
                self.sequences.reserve(length.saturating_mul(self.k));
            }
            let next_sequence = self.sequences.len() + 1;
            let sequence = *self
                .sequences
                .entry((parent.sequence, syllable))
                .or_insert(next_sequence);
            let arrival = self.arrival;
            self.arrival += 1;
            if self.best[end].capacity() == 0 {
                self.best[end].reserve_exact(self.k);
            }
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
            // 合法音节片段不作为纠错键处理。
            if let Some(syllable) = intact_piece(piece) {
                search.extend(length, start, raw_length, syllable, false, 0);
                continue;
            }
            // 两字母错键落在输入末尾时，第二个字母若能开始音节（`hauz` 的 `uz`），它更像下一个音节的简拼开头，交给「纠错头 + 简拼尾」去读（hua'z），不读成 ha'zu。
            if raw_length == 2 && start + 2 == length && is_prefix_piece(&piece[1..]) {
                continue;
            }
            let Some(targets) = index.get(piece) else {
                continue;
            };
            for target in targets {
                let enabled_bits = autocorrect_types & target.type_bits();
                if enabled_bits == 0 {
                    continue;
                }
                let syllable = target.syllable();
                search.extend(
                    length,
                    start,
                    raw_length,
                    syllable,
                    true,
                    correction_weight_bytes(piece, syllable, enabled_bits),
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
    // 全辅音串一个音节也匹配不上，这里有意让它保持可纠错：它可能是 `bqng`（bang）这种按偏了韵母的误触，也可能是 `qjn`、`zhr` 这种多字母简拼（含 zh/ch/sh 简拼单位）。两种读法都留着，先后由词典定：字面简拼读满全部按键的多字词条排在纠错读法前面，见 `quanpin::series::lead_literal_abbreviation_phrases`。
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
        assert!(search.best.iter().all(|slot| slot.capacity() == 0));
        assert_eq!(search.sequences.capacity(), 0);
    }

    #[test]
    fn search_defers_beam_allocations_until_a_position_receives_a_path() {
        let (search, allocations) =
            crate::ime::personal_rerank::allocations::count(|| Search::new(64, 9));
        assert!(search.best.iter().all(|slot| slot.capacity() == 0));
        assert!(
            allocations <= 3,
            "empty position beams should not allocate: {allocations}"
        );
    }

    #[test]
    fn search_defers_sequence_interning_allocation_until_a_path_exists() {
        let search = Search::new(64, 9);

        assert!(search.sequences.is_empty());
        assert_eq!(search.sequences.capacity(), 0);
    }

    #[test]
    fn search_reserves_a_beam_when_the_first_path_arrives() {
        let mut search = Search::new(1, 3);
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

        search.extend(1, 0, 1, "shi", true, TRANSPOSITION_WEIGHT);

        assert_eq!(search.best[1].len(), 1);
        assert_eq!(search.best[1].capacity(), 3);
        assert!(search.sequences.capacity() >= 3);
    }

    #[test]
    fn unreadable_input_does_not_allocate_unreachable_position_beams() {
        let _ = autocorrect_cut_kbest("qqqqqqqq", ALL_FOUR, 9);
        let (cuts, allocations) = crate::ime::personal_rerank::allocations::count(|| {
            autocorrect_cut_kbest("qqqqqqqq", ALL_FOUR, 9)
        });

        assert!(cuts.is_empty());
        assert_eq!(allocations, 2);
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
        let targets = correction_index().get(b"ahan").expect("ahan targets");
        let syllables: Vec<_> = targets.iter().map(|target| target.syllable()).collect();
        assert_eq!(syllables[..2], ["shan", "zhan"]);
    }

    #[test]
    fn repeated_adjacent_keys_get_the_dedicated_insertion_weight() {
        assert!(is_repeated_insertion("shanng", "shang"));
        assert!(is_repeated_insertion("shangg", "shang"));
        assert!(!is_repeated_insertion("shabg", "shang"));
        assert_eq!(
            correction_weight("shanng", "shang", INSERTION | DELETION),
            REPEATED_INSERTION_WEIGHT
        );
        assert_eq!(
            correction_weight("shabg", "shang", INSERTION | DELETION),
            DELETION_WEIGHT
        );
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

    #[test]
    fn two_letter_keys_cover_only_strings_without_a_reading() {
        for (type_bit, expected) in [(NEIGHBOR, ("wi", "wo")), (TRANSPOSITION, ("ia", "ai"))] {
            let table = two_letter_table(type_bit);
            assert!(
                table.contains(&(expected.0.to_owned(), expected.1.to_owned())),
                "{expected:?}"
            );
            for (wrong, correct) in table {
                assert_eq!(wrong.len(), 2, "{wrong} -> {correct}");
                assert_eq!(correct.len(), 2, "{wrong} -> {correct}");
                assert!(is_intact(correct), "{wrong} -> {correct}");
                assert!(
                    !is_intact(wrong) && !two_letters_readable(wrong.as_bytes()),
                    "a two-letter key must not shadow a syllable or jianpin: {wrong}"
                );
            }
        }
        // 简拼和音节前缀照旧不是错键。
        for jianpin in ["nh", "zh", "wd", "sh", "ba", "ne"] {
            assert!(
                correction_index().get(jianpin.as_bytes()).is_none(),
                "{jianpin}"
            );
        }
        assert!(two_letter_table(DELETION).is_empty());
        assert!(two_letter_table(INSERTION).is_empty());
    }

    #[test]
    fn a_two_letter_typo_reads_with_the_syllables_typed_right() {
        for types in [BOTH, ALL_FOUR, NEIGHBOR] {
            let wiaini = readings("wiaini", types);
            assert!(
                wiaini.contains(&"wo'ai'ni".to_owned()),
                "{types}: {wiaini:?}"
            );
            let wiai = readings("wiai", types);
            assert!(wiai.contains(&"wo'ai".to_owned()), "{types}: {wiai:?}");
        }
        assert_eq!(
            segments_of("wiaini", NEIGHBOR)[1..],
            [segment("ai", "ai", 2, false), segment("ni", "ni", 4, false)]
        );
        assert!(readings("iahao", TRANSPOSITION).contains(&"ai'hao".to_owned()));
        // 邻键开关关掉时 `wi` 不改。
        assert!(readings("wiaini", TRANSPOSITION).is_empty());
        // 只有两字母键、没有打对的音节时不纠：单独的 `wi`、`hi` 和整串都要改的 `wifi` 保持原样。
        for input in ["wi", "hi", "wifi", "hihi"] {
            assert!(readings(input, ALL_FOUR).is_empty(), "{input}");
            assert!(autocorrect_cut_detail(input, ALL_FOUR).is_none(), "{input}");
        }
        // 末尾的两字母键第二个字母能开始音节时让给简拼尾：`hauz` 不读成 ha'zu；`niwi` 的 `i` 开始不了音节，照纠。
        assert!(readings("hauz", ALL_FOUR).is_empty());
        assert!(readings("iahauz", ALL_FOUR).is_empty());
        assert!(readings("niwi", NEIGHBOR).contains(&"ni'wo".to_owned()));
        for input in ["wiaini", "wiai", "iahao", "niwi"] {
            let best = autocorrect_cut_kbest(input, ALL_FOUR, 1);
            let wider = autocorrect_cut_kbest(input, ALL_FOUR, 9);
            assert_eq!(best.first(), wider.first(), "k = 1 must agree for {input}");
        }
    }

    #[test]
    fn repeated_key_at_a_syllable_boundary_prefers_the_dropped_letter() {
        assert_eq!(cut("shannghai", ALL_FOUR), "shang'hai");
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
        // A repeated key at the boundary gets the dedicated insertion weight and beats a generic deletion.
        assert_eq!(
            segments_of("shangghao", ALL_FOUR),
            [
                segment("shang", "shangg", 0, true),
                segment("hao", "hao", 6, false)
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

    #[test]
    fn intact_pinyin_list_is_strictly_lexicographically_sorted() {
        let list = intact_pinyin_list();
        assert!(
            list.windows(2).all(|pair| pair[0] < pair[1]),
            "intact_pinyin_list 必须严格按 ASCII 字典序排列"
        );
    }

    #[test]
    fn compact_index_matches_reference_tables_exhaustively_and_fits_memory_budget() {
        let mut reference: HashMap<String, Vec<(&'static str, u32)>> = HashMap::new();
        for type_bit in [TRANSPOSITION, DELETION, INSERTION, NEIGHBOR] {
            for (wrong, correct) in correction_table(type_bit)
                .iter()
                .chain(two_letter_table(type_bit))
            {
                let correct_static = intact_piece(correct.as_bytes()).expect("合法音节");
                let targets = reference.entry(wrong.clone()).or_default();
                match targets
                    .iter_mut()
                    .find(|(syllable, _)| *syllable == correct_static)
                {
                    Some((_, bits)) => *bits |= type_bit,
                    None => targets.push((correct_static, type_bit)),
                }
            }
        }

        let compact = correction_index();
        assert_eq!(compact.keys.len(), reference.len());
        assert_eq!(compact.offsets.len(), compact.keys.len() + 1);

        for (wrong, expected_targets) in &reference {
            let actual = compact
                .get(wrong.as_bytes())
                .unwrap_or_else(|| panic!("紧凑索引缺少错键: {wrong}"));
            let actual_pairs: Vec<(&'static str, u32)> = actual
                .iter()
                .map(|target| (target.syllable(), target.type_bits()))
                .collect();
            assert_eq!(actual_pairs, *expected_targets, "错键 `{wrong}` 目标不匹配");
        }

        let backing_bytes = compact.keys.capacity() * std::mem::size_of::<u32>()
            + compact.offsets.capacity() * std::mem::size_of::<u32>()
            + compact.targets.capacity() * std::mem::size_of::<CompactTarget>();
        assert!(
            backing_bytes < 256 * 1024,
            "紧凑索引常驻堆内存 {backing_bytes} 字节超出 256 KiB 上限"
        );
    }
}

#[cfg(test)]
#[path = "autocorrect/finalize_tests.rs"]
mod finalize_tests;
