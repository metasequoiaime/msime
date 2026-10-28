//! 每块打完后记下的事实，以及怎么把它们加成报告里的数。纯函数，不碰 Runtime。
//!
//! 口径（都只数 dev、test 两部分的汉字，预热不计）：
//!
//! - `cfa` 逐字首选率（主指标）：点选时座位是 0、而且这一块里在它之前没有做过纠正的那些选择上屏的汉字数 ÷ 全部汉字。部分选择也算，只要被点时它在第一位；走失败路径的字算 0。「纠正」指改变了引擎手里输入的动作：补完停在声母上的音节，或者在拼写列里锁定音节。用户在候选栏和面板里都找不到才会做这两件事，之后再点第一位，是用户多按了键、替引擎说清了读音才换来的，不算首选。
//!   点了第一位之外的候选、开面板、翻页都不算纠正：它们不改输入，那次点选本身已经记为不是首选，之后剩下的键上引擎排第一的仍是它自己对同一串输入的判断。`--slip` 点错后整块重打也不算：重打的是同一串键，引擎没有多拿到信息，点错的那次本身记为不是首选。
//! - `cfa_lenient` 宽口径的逐字首选率：只看点选时座位是不是 0，不管之前有没有纠正。第一轮报告里的 `cfa` 就是这个数，留着作对照；两者之差是「纠正之后再点第一位」的字。
//! - `cca` 整块首选率：打完一块的键、第一次看候选时，第一个候选正好是整块的那些块的字数 ÷ 全部汉字。拆开选不算，这是最严的那个数。
//! - `chunk_top1` 同一件事按块数算。
//! - `zcr` 零修正块率：只点过第一位、没开面板、没锁拼写、没补键、没点错就打完的块 ÷ 全部块。
//! - `cpr` 每次上屏首选率：只数让文字上屏的那次选择（暂存的部分选择不算），座位是 0 的 ÷ 全部。和设备上「打字统计」的 `ranks[0]` ÷ 总数同口径（host-api 只在 `commit` 非空时记位次）。
//! - `kspc` 每字按键：输入键（含补完简拼的键）加上点选、开面板、翻页、锁拼写、点错的次数 ÷ 汉字数。
//! - `key_saving` 1 − 实际打的输入键 ÷ 打全拼要按的键，和设备上效率统计的 1 − typed/spelled 同口径。

use std::collections::BTreeMap;

use crate::corpus::Split;

/// 一次点选。
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Selection {
    /// 整张候选表里的下标。
    pub seat: usize,
    /// 盖住了几个金标准的字；点错的那次是 0。
    pub chars: usize,
    /// 是在展开面板里点的。
    pub panel: bool,
    /// 这次点选让文字上屏了（`Transition::commit` 非空）。
    pub committed: bool,
    /// 这一块里在这次点选之前补完过简拼或者锁过拼写。
    pub after_correction: bool,
}

/// 金标准打不出来时停在哪一步。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Unreachable {
    /// 点了文字对的候选，剩下的键却和剩余金标准的键对不上。
    Misaligned,
    /// 拼写列里没有下一个音节可锁。
    NoSpelling,
    /// 全键盘没有拼写列，候选表里也找不到。
    NotFound,
}

impl Unreachable {
    pub fn name(self) -> &'static str {
        match self {
            Unreachable::Misaligned => "misaligned",
            Unreachable::NoSpelling => "no_spelling",
            Unreachable::NotFound => "not_found",
        }
    }
}

/// 打完一块之后的全部事实。
#[derive(Clone, Debug, Default)]
pub struct ChunkRecord {
    pub doc: String,
    pub chars: usize,
    /// 实际按的输入键，含补完简拼的键和点错之后重打的键。
    pub keys_typed: usize,
    /// 把这块打全要按的键。
    pub keys_spelled: usize,
    pub abbreviated: bool,
    /// 第一次看候选时第一个就是整块。
    pub top1_exact: bool,
    pub selections: Vec<Selection>,
    pub panel_opens: usize,
    pub page_turns: usize,
    pub locks: usize,
    pub completion_keys: usize,
    pub unreachable: Option<Unreachable>,
    pub unreachable_chars: usize,
    pub slipped: bool,
    /// 这串键在本次运行里第一次出现。
    pub first_occurrence: bool,
    /// 这串键在预热里出现过。
    pub seen_in_warmup: bool,
    /// 这串键在整份集合里对应不止一种金标准。
    pub ambiguous: bool,
    /// 块的文字在整份集合里按出现次数排在前 200。
    pub common: bool,
    /// 本次运行在这块之前已经计入了多少字。
    pub offset: usize,
}

impl ChunkRecord {
    /// 主口径：第一位，而且之前没有纠正过。
    pub fn first_choice_chars(&self) -> usize {
        self.selections
            .iter()
            .filter(|selection| selection.seat == 0 && !selection.after_correction)
            .map(|selection| selection.chars)
            .sum()
    }

    /// 宽口径：只看是不是第一位。
    pub fn first_choice_chars_lenient(&self) -> usize {
        self.selections
            .iter()
            .filter(|selection| selection.seat == 0)
            .map(|selection| selection.chars)
            .sum()
    }

    pub fn zero_correction(&self) -> bool {
        self.unreachable.is_none()
            && !self.slipped
            && self.panel_opens == 0
            && self.locks == 0
            && self.completion_keys == 0
            && self.selections.iter().all(|selection| selection.seat == 0)
    }

    pub fn taps(&self) -> usize {
        self.selections.len() + self.panel_opens + self.page_turns + self.locks
    }
}

/// 一组块加起来的计数。
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Totals {
    pub chunks: usize,
    pub chars: usize,
    pub first_choice_chars: usize,
    pub first_choice_chars_lenient: usize,
    pub top1_exact_chars: usize,
    pub top1_exact_chunks: usize,
    pub zero_correction_chunks: usize,
    pub selections: usize,
    pub strip_selections: usize,
    pub seat_sum: usize,
    pub commits: usize,
    pub first_choice_commits: usize,
    pub taps: usize,
    pub keys_typed: usize,
    pub keys_spelled: usize,
    pub panel_opens: usize,
    pub page_turns: usize,
    pub locks: usize,
    pub completion_keys: usize,
    pub unreachable_chars: usize,
    pub unreachable: BTreeMap<&'static str, usize>,
    pub slips: usize,
    pub single_selections: usize,
    pub single_first: usize,
    pub multi_selections: usize,
    pub multi_first: usize,
}

impl Totals {
    pub fn add(&mut self, record: &ChunkRecord) {
        self.chunks += 1;
        self.chars += record.chars;
        self.first_choice_chars += record.first_choice_chars();
        self.first_choice_chars_lenient += record.first_choice_chars_lenient();
        if record.top1_exact {
            self.top1_exact_chars += record.chars;
            self.top1_exact_chunks += 1;
        }
        if record.zero_correction() {
            self.zero_correction_chunks += 1;
        }
        for selection in &record.selections {
            self.selections += 1;
            self.seat_sum += selection.seat;
            if !selection.panel {
                self.strip_selections += 1;
            }
            if selection.committed {
                self.commits += 1;
                if selection.seat == 0 {
                    self.first_choice_commits += 1;
                }
            }
            match selection.chars {
                0 => {}
                1 => {
                    self.single_selections += 1;
                    self.single_first += usize::from(selection.seat == 0);
                }
                _ => {
                    self.multi_selections += 1;
                    self.multi_first += usize::from(selection.seat == 0);
                }
            }
        }
        self.taps += record.taps();
        self.keys_typed += record.keys_typed;
        self.keys_spelled += record.keys_spelled;
        self.panel_opens += record.panel_opens;
        self.page_turns += record.page_turns;
        self.locks += record.locks;
        self.completion_keys += record.completion_keys;
        self.unreachable_chars += record.unreachable_chars;
        if let Some(kind) = record.unreachable {
            *self.unreachable.entry(kind.name()).or_default() += record.unreachable_chars;
        }
        self.slips += usize::from(record.slipped);
    }

    pub fn merge(&mut self, other: &Totals) {
        self.chunks += other.chunks;
        self.chars += other.chars;
        self.first_choice_chars += other.first_choice_chars;
        self.first_choice_chars_lenient += other.first_choice_chars_lenient;
        self.top1_exact_chars += other.top1_exact_chars;
        self.top1_exact_chunks += other.top1_exact_chunks;
        self.zero_correction_chunks += other.zero_correction_chunks;
        self.selections += other.selections;
        self.strip_selections += other.strip_selections;
        self.seat_sum += other.seat_sum;
        self.commits += other.commits;
        self.first_choice_commits += other.first_choice_commits;
        self.taps += other.taps;
        self.keys_typed += other.keys_typed;
        self.keys_spelled += other.keys_spelled;
        self.panel_opens += other.panel_opens;
        self.page_turns += other.page_turns;
        self.locks += other.locks;
        self.completion_keys += other.completion_keys;
        self.unreachable_chars += other.unreachable_chars;
        for (kind, chars) in &other.unreachable {
            *self.unreachable.entry(kind).or_default() += chars;
        }
        self.slips += other.slips;
        self.single_selections += other.single_selections;
        self.single_first += other.single_first;
        self.multi_selections += other.multi_selections;
        self.multi_first += other.multi_first;
    }

    pub fn cfa(&self) -> f64 {
        ratio(self.first_choice_chars, self.chars)
    }

    pub fn cfa_lenient(&self) -> f64 {
        ratio(self.first_choice_chars_lenient, self.chars)
    }

    pub fn cca(&self) -> f64 {
        ratio(self.top1_exact_chars, self.chars)
    }

    pub fn cpr(&self) -> f64 {
        ratio(self.first_choice_commits, self.commits)
    }

    pub fn zcr(&self) -> f64 {
        ratio(self.zero_correction_chunks, self.chunks)
    }

    pub fn to_json(&self) -> serde_json::Value {
        let per_char = |count: usize| round(ratio(count, self.chars));
        serde_json::json!({
            "chunks": self.chunks,
            "chars": self.chars,
            "cfa": round(self.cfa()),
            "cfa_lenient": round(self.cfa_lenient()),
            "cca": round(self.cca()),
            "chunk_top1": round(ratio(self.top1_exact_chunks, self.chunks)),
            "zcr": round(self.zcr()),
            "cpr": round(self.cpr()),
            "strip_hit": round(ratio(self.strip_selections, self.selections)),
            "mean_seat": round(ratio(self.seat_sum, self.selections)),
            "selections_per_char": per_char(self.selections),
            "kspc": per_char(self.keys_typed + self.taps),
            "input_keys_per_char": per_char(self.keys_typed),
            "key_saving": round(1.0 - ratio(self.keys_typed, self.keys_spelled)),
            "chars_per_commit": round(ratio(self.chars, self.commits)),
            "panel_opens_per_100": round(100.0 * ratio(self.panel_opens, self.chars)),
            "page_turns_per_100": round(100.0 * ratio(self.page_turns, self.chars)),
            "locks_per_100": round(100.0 * ratio(self.locks, self.chars)),
            "completion_keys_per_100": round(100.0 * ratio(self.completion_keys, self.chars)),
            "unreachable": round(ratio(self.unreachable_chars, self.chars)),
            "unreachable_by_kind": self.unreachable.iter().map(|(kind, chars)| (kind.to_string(), serde_json::json!(round(ratio(*chars, self.chars))))).collect::<serde_json::Map<_, _>>(),
            "slips": self.slips,
            "single_char_selections": self.single_selections,
            "single_char_first": round(ratio(self.single_first, self.single_selections)),
            "multi_char_selections": self.multi_selections,
            "multi_char_first": round(ratio(self.multi_first, self.multi_selections)),
        })
    }
}

pub fn ratio(numerator: usize, denominator: usize) -> f64 {
    if denominator == 0 {
        0.0
    } else {
        numerator as f64 / denominator as f64
    }
}

/// 三位小数：足够看出真实的变化，又不让一条用例的差别改写整份基线。
pub fn round(value: f64) -> f64 {
    (value * 1000.0).round() / 1000.0
}

/// 一次运行里计入的全部块，按部分和文档归好。
#[derive(Default)]
pub struct Run {
    pub records: Vec<(Split, ChunkRecord)>,
}

impl Run {
    pub fn totals(&self, split: Option<Split>) -> Totals {
        let mut totals = Totals::default();
        for (_, record) in self.selected(split) {
            totals.add(record);
        }
        totals
    }

    fn selected(&self, split: Option<Split>) -> impl Iterator<Item = &(Split, ChunkRecord)> {
        self.records
            .iter()
            .filter(move |(at, _)| split.is_none_or(|split| *at == split))
    }

    /// 按 `key` 分桶加起来。
    pub fn buckets(
        &self,
        split: Option<Split>,
        key: impl Fn(&ChunkRecord) -> String,
    ) -> BTreeMap<String, Totals> {
        let mut buckets: BTreeMap<String, Totals> = BTreeMap::new();
        for (_, record) in self.selected(split) {
            buckets.entry(key(record)).or_default().add(record);
        }
        buckets
    }

    /// 按文档做 cluster bootstrap：有放回地抽文档，重算 `metric`，取 2.5% 和 97.5% 分位。种子固定，同一份记录永远给出同一个区间。
    pub fn interval(
        &self,
        split: Option<Split>,
        metric: fn(&Totals) -> f64,
        draws: usize,
        seed: u64,
    ) -> (f64, f64) {
        let documents: Vec<Totals> = self
            .buckets(split, |record| record.doc.clone())
            .into_values()
            .collect();
        bootstrap(&documents, metric, draws, seed)
    }
}

pub fn bootstrap(
    documents: &[Totals],
    metric: fn(&Totals) -> f64,
    draws: usize,
    seed: u64,
) -> (f64, f64) {
    use rand::{RngExt, SeedableRng};
    if documents.is_empty() || draws == 0 {
        return (0.0, 0.0);
    }
    let mut rng = rand_pcg::Pcg64::seed_from_u64(seed);
    let mut values: Vec<f64> = (0..draws)
        .map(|_| {
            let mut sample = Totals::default();
            for _ in 0..documents.len() {
                sample.merge(&documents[rng.random_range(0..documents.len())]);
            }
            metric(&sample)
        })
        .collect();
    values.sort_by(f64::total_cmp);
    let at = |q: f64| values[((values.len() - 1) as f64 * q).round() as usize];
    (round(at(0.025)), round(at(0.975)))
}

/// 块长分桶：1、2、3、4、5–8 字，9 字以上。
pub fn length_band(chars: usize) -> &'static str {
    match chars {
        0 | 1 => "1",
        2 => "2",
        3 => "3",
        4 => "4",
        5..=8 => "5-8",
        _ => "9+",
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn selection(seat: usize, chars: usize, committed: bool) -> Selection {
        Selection {
            seat,
            chars,
            panel: false,
            committed,
            after_correction: false,
        }
    }

    fn corrected(seat: usize, chars: usize, committed: bool) -> Selection {
        Selection {
            after_correction: true,
            ..selection(seat, chars, committed)
        }
    }

    #[test]
    fn partial_first_choices_count_per_character_but_not_as_a_whole_chunk() {
        // 「吃饭了」：先点第一位的「吃饭」（暂存），再点第三位的「了」（上屏）。
        let record = ChunkRecord {
            doc: "d".into(),
            chars: 3,
            keys_typed: 8,
            keys_spelled: 8,
            selections: vec![selection(0, 2, false), selection(2, 1, true)],
            ..ChunkRecord::default()
        };
        let mut totals = Totals::default();
        totals.add(&record);
        assert_eq!(totals.first_choice_chars, 2);
        assert!((totals.cfa() - 2.0 / 3.0).abs() < 1e-9);
        assert_eq!(totals.cca(), 0.0);
        // 只有上屏的那次计入每次上屏首选率，它在第三位。
        assert_eq!((totals.commits, totals.first_choice_commits), (1, 0));
        assert!(!record.zero_correction());
        assert_eq!(totals.taps, 2);
        assert_eq!(
            totals.to_json()["kspc"],
            serde_json::json!(round(10.0 / 3.0))
        );
    }

    #[test]
    fn a_whole_chunk_on_top_is_a_hit_on_every_metric() {
        let record = ChunkRecord {
            doc: "d".into(),
            chars: 2,
            keys_typed: 4,
            keys_spelled: 6,
            abbreviated: true,
            top1_exact: true,
            selections: vec![selection(0, 2, true)],
            ..ChunkRecord::default()
        };
        let mut totals = Totals::default();
        totals.add(&record);
        assert_eq!(
            (totals.cfa(), totals.cca(), totals.cpr(), totals.zcr()),
            (1.0, 1.0, 1.0, 1.0)
        );
        assert_eq!(
            totals.to_json()["key_saving"],
            serde_json::json!(round(1.0 / 3.0))
        );
        assert_eq!(totals.to_json()["chars_per_commit"], serde_json::json!(2.0));
    }

    #[test]
    fn unreachable_characters_stay_in_the_denominator() {
        let record = ChunkRecord {
            doc: "d".into(),
            chars: 4,
            selections: vec![selection(0, 1, false)],
            locks: 2,
            unreachable: Some(Unreachable::NoSpelling),
            unreachable_chars: 3,
            ..ChunkRecord::default()
        };
        let mut totals = Totals::default();
        totals.add(&record);
        assert_eq!(totals.cfa(), 0.25);
        assert_eq!(totals.to_json()["unreachable"], serde_json::json!(0.75));
        assert_eq!(totals.unreachable["no_spelling"], 3);
        assert!(!record.zero_correction());
        // 没有上屏过：暂存的那段随失败路径丢掉了。
        assert_eq!(totals.commits, 0);
    }

    #[test]
    fn zero_correction_needs_first_seats_and_no_extra_actions() {
        let base = ChunkRecord {
            doc: "d".into(),
            chars: 3,
            selections: vec![selection(0, 2, false), selection(0, 1, true)],
            ..ChunkRecord::default()
        };
        assert!(base.zero_correction());
        for spoiled in [
            ChunkRecord {
                completion_keys: 2,
                ..base.clone()
            },
            ChunkRecord {
                locks: 1,
                ..base.clone()
            },
            ChunkRecord {
                panel_opens: 1,
                ..base.clone()
            },
            ChunkRecord {
                slipped: true,
                ..base.clone()
            },
        ] {
            assert!(!spoiled.zero_correction());
        }
    }

    #[test]
    fn first_seats_after_a_correction_count_only_on_the_lenient_metric() {
        // 「辞掉」打成 `ci'd`，候选栏里没有，补完成 `ci'diao` 后第一位就是它；「了」之后锁过拼写，再点第一位。
        let record = ChunkRecord {
            doc: "d".into(),
            chars: 5,
            completion_keys: 3,
            locks: 1,
            selections: vec![
                selection(0, 1, false),
                corrected(0, 2, false),
                corrected(3, 1, false),
                corrected(0, 1, true),
            ],
            ..ChunkRecord::default()
        };
        assert_eq!(record.first_choice_chars(), 1);
        assert_eq!(record.first_choice_chars_lenient(), 4);
        let mut totals = Totals::default();
        totals.add(&record);
        assert_eq!(totals.cfa(), 0.2);
        assert_eq!(totals.cfa_lenient(), 0.8);
        let json = totals.to_json();
        assert_eq!(
            (json["cfa"].clone(), json["cfa_lenient"].clone()),
            (serde_json::json!(0.2), serde_json::json!(0.8))
        );
        // 每次上屏首选率和设备统计同口径，不分纠正前后。
        assert_eq!((totals.commits, totals.first_choice_commits), (1, 1));
        assert!(!record.zero_correction());
    }

    #[test]
    fn without_corrections_both_metrics_agree() {
        let record = ChunkRecord {
            doc: "d".into(),
            chars: 4,
            panel_opens: 1,
            selections: vec![selection(9, 2, false), selection(0, 2, true)],
            ..ChunkRecord::default()
        };
        // 开面板不算纠正：面板里那次点选已经记为不是首选，之后的第一位照算。
        assert_eq!(record.first_choice_chars(), 2);
        assert_eq!(record.first_choice_chars_lenient(), 2);
    }

    #[test]
    fn merging_equals_adding_one_by_one() {
        let records = [
            ChunkRecord {
                doc: "a".into(),
                chars: 2,
                selections: vec![selection(0, 2, true)],
                top1_exact: true,
                ..ChunkRecord::default()
            },
            ChunkRecord {
                doc: "b".into(),
                chars: 3,
                selections: vec![selection(1, 3, true)],
                panel_opens: 1,
                ..ChunkRecord::default()
            },
            ChunkRecord {
                doc: "c".into(),
                chars: 2,
                locks: 1,
                selections: vec![corrected(0, 2, true)],
                ..ChunkRecord::default()
            },
        ];
        let mut added = Totals::default();
        let mut merged = Totals::default();
        for record in &records {
            added.add(record);
            let mut one = Totals::default();
            one.add(record);
            merged.merge(&one);
        }
        assert_eq!(added, merged);
    }

    #[test]
    fn bootstrap_is_reproducible_and_brackets_the_estimate() {
        let mut run = Run::default();
        for index in 0..40 {
            run.records.push((
                Split::Test,
                ChunkRecord {
                    doc: format!("d{index}"),
                    chars: 2,
                    selections: vec![selection(usize::from(index % 4 == 0), 2, true)],
                    ..ChunkRecord::default()
                },
            ));
        }
        let estimate = run.totals(Some(Split::Test)).cfa();
        assert_eq!(estimate, 0.75);
        let first = run.interval(Some(Split::Test), Totals::cfa, 500, 9);
        assert_eq!(first, run.interval(Some(Split::Test), Totals::cfa, 500, 9));
        assert!(first.0 < estimate && estimate < first.1, "{first:?}");
        assert_eq!(run.totals(Some(Split::Dev)).chunks, 0);
    }
}
