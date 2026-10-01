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

/// Converts `syllables` (toned, as `zhuyin.db` keys them) into the best sequence of spans covering all of them. `best` returns the heaviest entry for a key (the syllables joined by a space). `pins` are non-overlapping spans that must appear as given; no other span may cross them. A single syllable with no entry converts to itself, so there is always a path.
pub fn convert(
    syllables: &[&str],
    pins: &[Span],
    mut best: impl FnMut(&str) -> Result<Option<LanguageEntry>>,
) -> Result<Vec<Span>> {
    let count = syllables.len();
    // `paths[i]` is the best way to convert `syllables[..i]` and the span that ends it.
    let mut paths: Vec<Option<(Score, Option<Span>)>> = vec![None; count + 1];
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
        let mut arrivals = Vec::with_capacity(arrival_capacity(count, start, pin.is_some()));
        if let Some(pin) = pin {
            arrivals.push((score.add(pin.len(), 0), pin.clone()));
        } else {
            for end in start + 1..=count {
                if pins.iter().any(|pin| pin.overlaps(start, end)) {
                    break;
                }
                let key = build_dictionary_key(&syllables[start..end]);
                let span = match best(&key)? {
                    Some(entry) => (
                        score.add(end - start, entry.weight),
                        Span {
                            start,
                            end,
                            text: entry.text,
                        },
                    ),
                    None if end == start + 1 => (
                        score.add(1, 0),
                        Span {
                            start,
                            end,
                            text: syllables[start].to_owned(),
                        },
                    ),
                    None => continue,
                };
                arrivals.push(span);
            }
        }
        for (arrival, span) in arrivals {
            let end = span.end;
            if paths[end]
                .as_ref()
                .is_none_or(|(current, _)| arrival > *current)
            {
                paths[end] = Some((arrival, Some(span)));
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

fn arrival_capacity(count: usize, start: usize, pinned: bool) -> usize {
    if pinned {
        1
    } else {
        count.saturating_sub(start)
    }
}

fn build_dictionary_key(syllables: &[&str]) -> String {
    let capacity = syllables
        .iter()
        .map(|syllable| syllable.len())
        .sum::<usize>()
        .saturating_add(syllables.len().saturating_sub(1));
    let mut key = String::with_capacity(capacity);
    for (index, syllable) in syllables.iter().enumerate() {
        if index > 0 {
            key.push(' ');
        }
        key.push_str(syllable);
    }
    key
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
        convert(syllables, pins, |key| Ok(best.get(key).cloned())).unwrap()
    }

    #[test]
    fn dictionary_key_joins_syllables_in_order() {
        assert_eq!(build_dictionary_key(&["ㄋㄧˇ", "ㄏㄠˇ"]), "ㄋㄧˇ ㄏㄠˇ");
    }

    #[test]
    fn arrivals_capacity_matches_possible_dictionary_ends() {
        assert_eq!(arrival_capacity(4, 0, false), 4);
        assert_eq!(arrival_capacity(4, 2, false), 2);
        assert_eq!(arrival_capacity(4, 2, true), 1);
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
        let pin = Span {
            start: 1,
            end: 2,
            text: "郝".to_owned(),
        };
        let spans = run(
            &["ㄋㄧˇ", "ㄏㄠˇ", "ㄇㄚ˙"],
            std::slice::from_ref(&pin),
            &ENTRIES,
        );
        assert_eq!(texts(&spans), ["你", "郝", "嗎"]);
        assert_eq!(spans[1], pin);

        // A pinned word holds even where the dictionary has no such entry.
        let pin = Span {
            start: 0,
            end: 2,
            text: "妳好".to_owned(),
        };
        let spans = run(&["ㄋㄧˇ", "ㄏㄠˇ", "ㄊㄞˊ", "ㄨㄢ"], &[pin], &ENTRIES);
        assert_eq!(texts(&spans), ["妳好", "臺灣"]);
    }
}
