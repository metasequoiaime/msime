//! Conversion of the completed syllables into text: a Viterbi pass over every way of cutting the sequence into dictionary words, with the spans the user pinned through the candidate list kept as chosen.

use crate::error::Result;
use crate::language_dictionary::LanguageEntry;

/// The most syllables a composition holds. A tone key that would complete one more first commits the leftmost converted word (libchewing's auto-shift).
pub const MAX_SYLLABLES: usize = 20;

/// Text covering the syllables `start..end`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub end: usize,
    /// 这段文字在词库里的键：所用的带调音节以单个空格连接。九键下一个位置可能有多个读音，键说明转换实际用了哪一个。
    pub key: String,
    pub text: String,
}

impl Span {
    pub fn len(&self) -> usize {
        self.end - self.start
    }

    pub fn is_empty(&self) -> bool {
        self.start == self.end
    }

    pub fn overlaps(&self, start: usize, end: usize) -> bool {
        self.start < end && start < self.end
    }
}

/// How good a path is, compared field by field. Longer words come first, as in libchewing: each span adds the square of its length, so one two-syllable word (4) beats two single characters (2) whatever their weights. Among paths with equally long words the summed entry weight decides.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord)]
struct Score {
    length: u64,
    weight: i64,
}

impl Score {
    fn add(self, span_len: usize, weight: i64) -> Self {
        let span_len = span_len as u64;
        Self {
            length: self.length + span_len * span_len,
            weight: self.weight.saturating_add(weight),
        }
    }
}

/// 把 `count` 个音节转换成覆盖全部音节的最佳文字段序列。`best(start, end)` 返回音节 `start..end` 的最重词条及其键；`pins` 是互不重叠、必须原样出现的段，其他段不得跨过它们。没有词条的单个音节转换成 `fallback(i)` 给出的读音本身，所以总有一条路径。
pub fn convert(
    count: usize,
    pins: &[Span],
    mut best: impl FnMut(usize, usize) -> Result<Option<(String, LanguageEntry)>>,
    mut fallback: impl FnMut(usize) -> String,
) -> Result<Vec<Span>> {
    // `paths[i]` 是转换前 `i` 个音节的最佳路径及结束它的那一段。
    // 组合长度内的路径表放在栈上，超长直接调用仍使用动态缓冲。
    let mut stack_paths = [const { None }; MAX_SYLLABLES + 1];
    let mut heap_paths;
    let paths: &mut [Option<(Score, Option<Span>)>] = if count <= MAX_SYLLABLES {
        &mut stack_paths[..=count]
    } else {
        heap_paths = vec![None; count + 1];
        &mut heap_paths
    };
    paths[0] = Some((
        Score {
            length: 0,
            weight: 0,
        },
        None,
    ));
    for start in 0..count {
        let Some((score, _)) = &paths[start] else {
            continue;
        };
        let score = *score;
        let pin = pins.iter().find(|pin| pin.start == start);
        if let Some(pin) = pin {
            consider_span(paths, score, pin.clone(), 0);
        } else {
            for end in start + 1..=count {
                if pins.iter().any(|pin| pin.overlaps(start, end)) {
                    break;
                }
                let (key, text, weight) = match best(start, end)? {
                    Some((key, entry)) => (key, entry.text, entry.weight),
                    None if end == start + 1 => {
                        let reading = fallback(start);
                        (reading.clone(), reading, 0)
                    }
                    None => continue,
                };
                consider_span(
                    paths,
                    score,
                    Span {
                        start,
                        end,
                        key,
                        text,
                    },
                    weight,
                );
            }
        }
    }
    let mut spans = Vec::with_capacity(count);
    let mut end = count;
    while end > 0 {
        let span = paths[end]
            .as_mut()
            .and_then(|(_, span)| span.take())
            .expect("every position is reachable through single-syllable spans or pins");
        end = span.start;
        spans.push(span);
    }
    spans.reverse();
    Ok(spans)
}

fn consider_span(
    paths: &mut [Option<(Score, Option<Span>)>],
    score: Score,
    span: Span,
    weight: i64,
) {
    let arrival = score.add(span.len(), weight);
    let end = span.end;
    if paths[end]
        .as_ref()
        .is_none_or(|(current, _)| arrival > *current)
    {
        paths[end] = Some((arrival, Some(span)));
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use super::*;

    fn dictionary(entries: &[(&str, &str, i64)]) -> HashMap<String, LanguageEntry> {
        let mut best: HashMap<String, LanguageEntry> = HashMap::new();
        for (key, text, weight) in entries {
            let entry = LanguageEntry {
                text: (*text).to_owned(),
                weight: *weight,
            };
            if best
                .get(*key)
                .is_none_or(|current| current.weight < *weight)
            {
                best.insert((*key).to_owned(), entry);
            }
        }
        best
    }

    fn texts(spans: &[Span]) -> Vec<&str> {
        spans.iter().map(|span| span.text.as_str()).collect()
    }

    fn run(syllables: &[&str], pins: &[Span], entries: &[(&str, &str, i64)]) -> Vec<Span> {
        let best = dictionary(entries);
        convert(
            syllables.len(),
            pins,
            |start, end| {
                let key = syllables[start..end].join(" ");
                Ok(best.get(&key).cloned().map(|entry| (key, entry)))
            },
            |index| syllables[index].to_owned(),
        )
        .unwrap()
    }

    fn span(start: usize, end: usize, key: &str, text: &str) -> Span {
        Span {
            start,
            end,
            key: key.to_owned(),
            text: text.to_owned(),
        }
    }

    #[test]
    fn bounded_conversion_does_not_allocate_path_storage() {
        let pin = span(0, MAX_SYLLABLES, "合成讀音", "合成文字");
        let (spans, allocations) = crate::ime::personal_rerank::allocations::count(|| {
            convert(
                MAX_SYLLABLES,
                std::slice::from_ref(&pin),
                |_, _| panic!("完整钉住的组合不应查询词库"),
                |_| panic!("完整钉住的组合不应生成兜底读音"),
            )
            .unwrap()
        });
        assert_eq!(allocations, 3, "只应分配返回向量和钉住词条的两个字符串");
        assert_eq!(spans, [pin]);

        let (spans, allocations) = crate::ime::personal_rerank::allocations::count(|| {
            convert(
                0,
                &[],
                |_, _| panic!("空组合不应查询词库"),
                |_| panic!("空组合不应生成兜底读音"),
            )
            .unwrap()
        });
        assert!(spans.is_empty());
        assert_eq!(allocations, 0);
    }

    #[test]
    fn conversion_accepts_more_than_the_composition_limit() {
        let syllables = vec!["ㄋㄧˇ"; MAX_SYLLABLES + 1];
        let spans = run(&syllables, &[], &ENTRIES);
        assert_eq!(spans.len(), syllables.len());
        assert!(spans.iter().all(|span| span.text == "你"));
        assert_eq!((spans[0].start, spans[0].end), (0, 1));
        assert_eq!(spans.last().unwrap().end, syllables.len());
    }

    #[test]
    fn direct_arrival_updates_keep_the_best_path() {
        let mut paths = vec![None; 2];
        let score = Score {
            length: 0,
            weight: 0,
        };
        consider_span(&mut paths, score, span(0, 1, "ㄉㄧ", "低"), 1);
        consider_span(&mut paths, score, span(0, 1, "ㄍㄠ", "高"), 2);
        assert_eq!(
            paths[1].as_ref().map(|(score, _)| *score),
            Some(score.add(1, 2))
        );
        assert_eq!(
            paths[1]
                .as_ref()
                .and_then(|(_, span)| span.as_ref())
                .map(|span| span.text.as_str()),
            Some("高")
        );
    }

    const ENTRIES: [(&str, &str, i64); 7] = [
        ("ㄋㄧˇ", "你", 1000),
        ("ㄏㄠˇ", "好", 2000),
        ("ㄋㄧˇ ㄏㄠˇ", "你好", 10),
        ("ㄇㄚ˙", "嗎", 800),
        ("ㄊㄞˊ", "臺", 400),
        ("ㄨㄢ", "灣", 300),
        ("ㄊㄞˊ ㄨㄢ", "臺灣", 800),
    ];

    #[test]
    fn longer_words_win_over_heavier_characters() {
        let spans = run(&["ㄋㄧˇ", "ㄏㄠˇ", "ㄇㄚ˙"], &[], &ENTRIES);
        assert_eq!(texts(&spans), ["你好", "嗎"]);
        assert_eq!((spans[0].start, spans[0].end), (0, 2));
        assert_eq!((spans[1].start, spans[1].end), (2, 3));
        // 每段带着命中的键。
        assert_eq!(spans[0].key, "ㄋㄧˇ ㄏㄠˇ");
        assert_eq!(spans[1].key, "ㄇㄚ˙");
    }

    // 九键下一个位置有多个读音：`best` 自己在读音里挑，返回的键说明用了哪个；没有词条的单音节用调用方给的读音兜底。
    #[test]
    fn spans_carry_the_key_best_chose_and_the_fallback_reading() {
        let positions: [&[&str]; 3] = [&["ㄌㄧˇ", "ㄋㄧˇ"], &["ㄏㄠˇ"], &["ㄅㄧㄤ", "ㄆㄧㄤ"]];
        let best = dictionary(&ENTRIES);
        let spans = convert(
            positions.len(),
            &[],
            |start, end| {
                // 逐个组合查，取最重的。
                let mut keys = vec![String::new()];
                for readings in &positions[start..end] {
                    keys = keys
                        .iter()
                        .flat_map(|prefix| {
                            readings.iter().map(move |reading| {
                                if prefix.is_empty() {
                                    (*reading).to_owned()
                                } else {
                                    format!("{prefix} {reading}")
                                }
                            })
                        })
                        .collect();
                }
                Ok(keys
                    .into_iter()
                    .filter_map(|key| best.get(&key).cloned().map(|entry| (key, entry)))
                    .max_by_key(|(_, entry)| entry.weight))
            },
            |index| positions[index][1].to_owned(),
        )
        .unwrap();
        assert_eq!(
            spans,
            [
                span(0, 2, "ㄋㄧˇ ㄏㄠˇ", "你好"),
                span(2, 3, "ㄆㄧㄤ", "ㄆㄧㄤ")
            ]
        );
    }

    #[test]
    fn weights_decide_between_equally_long_words() {
        // Two segmentations of the same lengths: the heavier total wins.
        let entries = [
            ("a", "A", 1),
            ("b c", "BC", 5),
            ("a b", "AB", 9),
            ("c", "C", 1),
        ];
        assert_eq!(texts(&run(&["a", "b", "c"], &[], &entries)), ["AB", "C"]);
    }

    #[test]
    fn unknown_syllables_convert_to_themselves() {
        let spans = run(&["ㄅㄧㄤ", "ㄋㄧˇ"], &[], &ENTRIES);
        assert_eq!(texts(&spans), ["ㄅㄧㄤ", "你"]);
        assert_eq!(spans.capacity(), spans.len());
        assert!(run(&[], &[], &ENTRIES).is_empty());
    }

    #[test]
    fn pins_are_kept_and_never_crossed() {
        let pin = span(1, 2, "ㄏㄠˇ", "郝");
        let spans = run(
            &["ㄋㄧˇ", "ㄏㄠˇ", "ㄇㄚ˙"],
            std::slice::from_ref(&pin),
            &ENTRIES,
        );
        assert_eq!(texts(&spans), ["你", "郝", "嗎"]);
        assert_eq!(spans[1], pin);

        // A pinned word holds even where the dictionary has no such entry.
        let pin = span(0, 2, "ㄋㄧˇ ㄏㄠˇ", "妳好");
        let spans = run(&["ㄋㄧˇ", "ㄏㄠˇ", "ㄊㄞˊ", "ㄨㄢ"], &[pin], &ENTRIES);
        assert_eq!(texts(&spans), ["妳好", "臺灣"]);
    }
}
