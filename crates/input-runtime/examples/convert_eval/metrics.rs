//! Metric accumulation for pinyin-to-text conversion, kept free of Engine types so it can be
//! unit-tested without a dictionary.

use std::collections::BTreeMap;

/// Where a candidate came from. Mirrors the Engine's CandidateSource discriminants, which the
/// runtime passes through untouched as `Candidate::source`.
pub fn source_name(source: u8) -> &'static str {
    // Mirrors msime_engine::CandidateSource, in declaration order.
    match source {
        0 => "database",
        1 => "user-database",
        2 => "cloud",
        3 => "ai",
        4 => "english",
        5 => "quick-phrase",
        6 => "emoji",
        7 => "kaomoji",
        8 => "generated",
        9 => "fallback",
        10 => "neural-desktop",
        11 => "neural-keyboard",
        _ => "unknown",
    }
}

#[derive(Default, Clone)]
pub struct Bucket {
    pub cases: usize,
    pub top1: usize,
    pub top5: usize,
    pub top9: usize,
    pub found: usize,
    /// Summed 1/rank over cases where the gold answer appears at all.
    pub reciprocal_rank: f64,
    /// Summed longest-common-prefix character ratio against the gold answer for the top-1 text.
    pub top1_char_prefix: f64,
}

impl Bucket {
    fn observe(&mut self, rank: Option<usize>, top1_prefix: f64) {
        self.cases += 1;
        self.top1_char_prefix += top1_prefix;
        if let Some(rank) = rank {
            self.found += 1;
            self.reciprocal_rank += 1.0 / (rank as f64);
            if rank == 1 {
                self.top1 += 1;
            }
            if rank <= 5 {
                self.top5 += 1;
            }
            if rank <= 9 {
                self.top9 += 1;
            }
        }
    }

    pub fn rate(hits: usize, cases: usize) -> f64 {
        if cases == 0 {
            0.0
        } else {
            hits as f64 / cases as f64
        }
    }
}

#[derive(Default)]
pub struct Report {
    pub overall: Bucket,
    /// Keyed by tag, then by syllable count, then by the source of the gold-matching candidate.
    pub by_tag: BTreeMap<String, Bucket>,
    pub by_syllables: BTreeMap<usize, Bucket>,
    /// How often the gold answer, when found at all, came from each candidate source.
    ///
    /// This is the measurement that matters for lattice work. For three or more complete syllables
    /// the Engine adds one lattice path and then, *if the Google-Pinyin fallback produced a
    /// sentence*, moves that fallback row to index 0. Whether a lattice change can move top-1
    /// therefore depends on which source actually occupies position 1, which varies by input —
    /// so both are recorded rather than assumed.
    pub gold_source: BTreeMap<&'static str, usize>,
    /// What sat at position 1, by source, whether or not it was correct.
    pub top1_source: BTreeMap<&'static str, usize>,
    pub failures: Vec<Failure>,
}

pub struct Failure {
    pub id: String,
    pub input: String,
    pub gold: String,
    pub got: String,
    pub rank: Option<usize>,
}

/// One case as the report sees it. The fields travel together, so they are one value rather than
/// eight positional arguments that are easy to transpose at the call site.
pub struct Observation<'a> {
    pub id: &'a str,
    pub input: &'a str,
    pub gold: &'a str,
    pub tags: &'a [String],
    pub syllables: usize,
    /// The full ordered candidate list as (text, source), position 1 first.
    pub candidates: &'a [(String, u8)],
    pub keep_failure: bool,
}

impl Report {
    pub fn observe(&mut self, case: Observation<'_>) {
        let Observation {
            id,
            input,
            gold,
            tags,
            syllables,
            candidates,
            keep_failure,
        } = case;
        let rank = candidates
            .iter()
            .position(|(text, _)| text == gold)
            .map(|i| i + 1);
        let top1_text = candidates.first().map(|(t, _)| t.as_str()).unwrap_or("");
        let prefix = char_prefix_ratio(top1_text, gold);

        self.overall.observe(rank, prefix);
        self.by_syllables
            .entry(syllables)
            .or_default()
            .observe(rank, prefix);
        for tag in tags {
            self.by_tag
                .entry(tag.clone())
                .or_default()
                .observe(rank, prefix);
        }
        if let Some((_, source)) = candidates.first() {
            *self.top1_source.entry(source_name(*source)).or_default() += 1;
        }
        if let Some(rank) = rank {
            let (_, source) = &candidates[rank - 1];
            *self.gold_source.entry(source_name(*source)).or_default() += 1;
        } else if keep_failure {
            self.failures.push(Failure {
                id: id.to_string(),
                input: input.to_string(),
                gold: gold.to_string(),
                got: top1_text.to_string(),
                rank,
            });
        }
        if keep_failure && rank.is_some_and(|r| r > 1) {
            self.failures.push(Failure {
                id: id.to_string(),
                input: input.to_string(),
                gold: gold.to_string(),
                got: top1_text.to_string(),
                rank,
            });
        }
    }
}

/// 双拼触点评测的一个分桶。召回按金标准在整张候选表里的名次算（第 1、前 3、第一页即前 9）；`top1_changed` 和 `gold_demoted` 拿同一条用例按想按的键原样打出的名次作对照，量的是「这次按到的键」让结果变了多少。
#[derive(Default, Clone)]
pub struct TouchBucket {
    pub cases: usize,
    pub top1: usize,
    pub top3: usize,
    pub top9: usize,
    pub found: usize,
    pub reciprocal_rank: f64,
    /// 对照的首选是金标准，这次不是。
    pub top1_changed: usize,
    /// 对照里找得到金标准，这次名次更靠后或找不到了。
    pub gold_demoted: usize,
}

impl TouchBucket {
    fn observe(&mut self, rank: Option<usize>, reference: Option<usize>) {
        self.cases += 1;
        if let Some(rank) = rank {
            self.found += 1;
            self.reciprocal_rank += 1.0 / rank as f64;
            self.top1 += usize::from(rank == 1);
            self.top3 += usize::from(rank <= 3);
            self.top9 += usize::from(rank <= 9);
        }
        if reference == Some(1) && rank != Some(1) {
            self.top1_changed += 1;
        }
        if let Some(reference) = reference {
            if rank.is_none_or(|rank| rank > reference) {
                self.gold_demoted += 1;
            }
        }
    }
}

/// 首选规则（#10018 PR3）在一组配置下的统计。`first_changed` 比的是实际首选的文字（与 `--assert-top1-unchanged` 相同），对照是同一条用例按想按的键原样、不带触点打出的首选；`confident` 是首选为按首选规则坐上去的纠错行（视图的 `confident_correction`），`confident_gold` 是其中正好是金标准的。
#[derive(Default)]
pub struct FirstSeat {
    pub first_changed: usize,
    pub confident: usize,
    pub confident_gold: usize,
    /// clean 集里首选被改的用例，逐条列出供人工复核；别的模式不列。
    pub changed: Vec<ChangedFirst>,
}

/// clean 集里首选被改的一条用例。
pub struct ChangedFirst {
    pub id: String,
    /// 送进去的按键。
    pub keys: String,
    pub gold: String,
    /// 原样输入的首选。
    pub was: Option<String>,
    /// 这次的首选。
    pub now: Option<String>,
    pub confident: bool,
}

impl FirstSeat {
    /// `changed` 给了时（clean 集）首选被改的这条用例记进清单。
    pub fn observe(
        &mut self,
        was: Option<&str>,
        now: Option<&str>,
        confident: bool,
        gold: bool,
        changed: Option<impl FnOnce() -> ChangedFirst>,
    ) {
        self.confident += usize::from(confident);
        self.confident_gold += usize::from(confident && gold);
        if was != now {
            self.first_changed += 1;
            if let Some(changed) = changed {
                self.changed.push(changed());
            }
        }
    }
}

/// 一组（方案、触点模式、σ、偏置）的双拼触点评测结果，按音节档和错键数分桶。
#[derive(Default)]
pub struct TouchReport {
    pub overall: TouchBucket,
    pub by_syllables: BTreeMap<&'static str, TouchBucket>,
    pub by_errors: BTreeMap<&'static str, TouchBucket>,
    pub first_seat: FirstSeat,
}

/// 一条双拼用例在一组配置下的结果。
pub struct TouchObservation {
    pub syllables: usize,
    /// 按到的键与想按的键不同的位置数。
    pub errors: usize,
    /// 金标准在这次候选表里的名次，从 1 起。
    pub rank: Option<usize>,
    /// 同一条用例按想按的键原样打出时金标准的名次。
    pub reference: Option<usize>,
}

impl TouchReport {
    pub fn observe(&mut self, case: TouchObservation) {
        let TouchObservation {
            syllables,
            errors,
            rank,
            reference,
        } = case;
        self.overall.observe(rank, reference);
        self.by_syllables
            .entry(syllable_band(syllables))
            .or_default()
            .observe(rank, reference);
        self.by_errors
            .entry(error_band(errors))
            .or_default()
            .observe(rank, reference);
    }
}

/// 音节档：1、2、3–5、6 及以上。1–2 个音节走不到纠错整句（至少 3 个音节），6 个以上是长句。
pub fn syllable_band(syllables: usize) -> &'static str {
    match syllables {
        0 | 1 => "1",
        2 => "2",
        3..=5 => "3-5",
        _ => "6+",
    }
}

/// 错键数档：0、1、2、3 及以上。纠错整句一个跨度只换一个音节，错两处以上召回会明显变低。
pub fn error_band(errors: usize) -> &'static str {
    match errors {
        0 => "0",
        1 => "1",
        2 => "2",
        _ => "3+",
    }
}

/// Fraction of the gold answer's characters that the produced text reproduces from the start.
/// A whole-sentence decoder that gets the first half right is meaningfully closer than one that
/// gets nothing right, and top-1 exact match alone cannot see that.
pub fn char_prefix_ratio(got: &str, gold: &str) -> f64 {
    let gold_chars: Vec<char> = gold.chars().collect();
    if gold_chars.is_empty() {
        return 0.0;
    }
    let shared = got
        .chars()
        .zip(gold_chars.iter().copied())
        .take_while(|(a, b)| a == b)
        .count();
    shared as f64 / gold_chars.len() as f64
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prefix_ratio_is_the_shared_leading_fraction() {
        assert_eq!(char_prefix_ratio("你好", "你好"), 1.0);
        assert_eq!(char_prefix_ratio("你号", "你好"), 0.5);
        assert_eq!(char_prefix_ratio("拟好", "你好"), 0.0);
        assert_eq!(char_prefix_ratio("", "你好"), 0.0);
        assert_eq!(char_prefix_ratio("你好", ""), 0.0);
        // A longer wrong answer still only scores its shared prefix.
        assert_eq!(char_prefix_ratio("你好吗", "你好"), 1.0);
    }

    #[test]
    fn rank_drives_every_threshold_and_reciprocal_rank() {
        let mut report = Report::default();
        let candidates: Vec<(String, u8)> = (1..=9).map(|i| (format!("c{i}"), 0u8)).collect();
        report.observe(Observation {
            id: "a",
            input: "x",
            gold: "c1",
            tags: &[],
            syllables: 2,
            candidates: &candidates,
            keep_failure: false,
        });
        report.observe(Observation {
            id: "b",
            input: "x",
            gold: "c5",
            tags: &[],
            syllables: 2,
            candidates: &candidates,
            keep_failure: false,
        });
        report.observe(Observation {
            id: "c",
            input: "x",
            gold: "zz",
            tags: &[],
            syllables: 2,
            candidates: &candidates,
            keep_failure: false,
        });
        assert_eq!(report.overall.cases, 3);
        assert_eq!(report.overall.top1, 1);
        assert_eq!(report.overall.top5, 2);
        assert_eq!(report.overall.found, 2);
        assert!((report.overall.reciprocal_rank - (1.0 + 0.2)).abs() < 1e-9);
    }

    #[test]
    fn touch_buckets_compare_against_the_typed_as_intended_rank() {
        let mut report = TouchReport::default();
        let observe = |report: &mut TouchReport, errors, rank, reference| {
            report.observe(TouchObservation {
                syllables: 4,
                errors,
                rank,
                reference,
            })
        };
        // 首选被换掉：对照第 1，这次第 2。
        observe(&mut report, 1, Some(2), Some(1));
        // 金标准丢了：对照第 3，这次找不到。
        observe(&mut report, 2, None, Some(3));
        // 名次反而靠前，不算降位。
        observe(&mut report, 1, Some(1), Some(2));
        // 对照里本来就没有，什么也不记。
        observe(&mut report, 0, None, None);
        let overall = &report.overall;
        assert_eq!(overall.cases, 4);
        assert_eq!(
            (overall.top1, overall.top3, overall.top9, overall.found),
            (1, 2, 2, 2)
        );
        assert_eq!(overall.top1_changed, 1);
        assert_eq!(overall.gold_demoted, 2);
        assert!((overall.reciprocal_rank - 1.5).abs() < 1e-9);
        assert_eq!(
            report.by_syllables.keys().copied().collect::<Vec<_>>(),
            ["3-5"]
        );
        assert_eq!(
            report.by_errors.keys().copied().collect::<Vec<_>>(),
            ["0", "1", "2"]
        );
        assert_eq!(report.by_errors["1"].cases, 2);
    }

    #[test]
    fn first_seat_counts_changed_firsts_and_lists_them_only_when_asked() {
        let mut seat = FirstSeat::default();
        let changed = |id: &str| {
            let id = id.to_owned();
            move || ChangedFirst {
                id,
                keys: "mwgfxi".into(),
                gold: "没关系".into(),
                was: Some("没跟系".into()),
                now: Some("没关系".into()),
                confident: true,
            }
        };
        // 首选没变：什么也不记，即使这次首选是纠错行（不会发生，但计数各自独立）。
        seat.observe(Some("甲"), Some("甲"), false, true, Some(changed("a")));
        // 纠错行坐上首选、正好是金标准。
        seat.observe(
            Some("没跟系"),
            Some("没关系"),
            true,
            true,
            Some(changed("b")),
        );
        // 首选变了但不是纠错行，也不列清单（noisy 集）。
        seat.observe(
            Some("甲"),
            Some("乙"),
            false,
            false,
            None::<fn() -> ChangedFirst>,
        );
        // 从空表变成有候选也算变了。
        seat.observe(None, Some("乙"), true, false, Some(changed("d")));
        assert_eq!(seat.first_changed, 3);
        assert_eq!(seat.confident, 2);
        assert_eq!(seat.confident_gold, 1);
        assert_eq!(
            seat.changed
                .iter()
                .map(|case| case.id.as_str())
                .collect::<Vec<_>>(),
            ["b", "d"]
        );
    }

    #[test]
    fn bands_cover_every_count() {
        let syllables: Vec<_> = [1, 2, 3, 5, 6, 12].map(syllable_band).into();
        assert_eq!(syllables, ["1", "2", "3-5", "3-5", "6+", "6+"]);
        let errors: Vec<_> = [0, 1, 2, 3, 9].map(error_band).into();
        assert_eq!(errors, ["0", "1", "2", "3+", "3+"]);
    }

    #[test]
    fn gold_source_records_where_the_correct_answer_came_from() {
        let mut report = Report::default();
        // Position 1 is a fallback sentence; the correct answer is the lattice row behind it.
        let candidates = vec![("错的".to_string(), 9u8), ("对的".to_string(), 8u8)];
        report.observe(Observation {
            id: "a",
            input: "x",
            gold: "对的",
            tags: &[],
            syllables: 2,
            candidates: &candidates,
            keep_failure: false,
        });
        assert_eq!(report.gold_source.get("generated"), Some(&1));
        assert_eq!(report.top1_source.get("fallback"), Some(&1));
        assert_eq!(report.overall.top1, 0);
        assert_eq!(report.overall.top5, 1);
    }
}
