//! `msime-zhuyin.db`：注音方案的注音词库，按 `msime_engine::language_dictionary` 定义的结构写出。数据是 libchewing-data（LGPL-2.1-or-later，见 `resources/licenses/libchewing-data-LGPL-2.1.txt`）在 `resources/dictionary-sources.lock.json` 的 `libchewing-data` 引用所记提交的 `dict/chewing/tsi.csv`、`dict/chewing/word.csv`，由 msime-dictionary 原样收在 `sources/zhuyin/` 下；构建从 `--dictionary` checkout 读取，并按它的 `upstream.lock.json` 校验。
//!
//! Three files are read, all `text,frequency,reading` CSV: `tsi.csv` (phrases and characters with their use counts), `word.csv` (every character with each of its readings, all at frequency 0), and the McBopomofo phrase supplement (frequency 0). The scheme types toned syllables, so an entry's key is its syllables joined by one space as the files write them (`ㄋㄧˇ ㄏㄠˇ`): tone 1 is unmarked and ˊ ˇ ˋ ˙ follow the letters. A row appearing more than once keeps its largest frequency, so a `word.csv` character and a supplement phrase weigh 0 unless `tsi.csv` gives the same combination a count.
//!
//! About a third of the phrases still weigh 0 that way: the whole supplement and the `tsi.csv` rows chewing never counted. McBopomofo's `phrase.occ` (MIT, its own corpus counts, one `phrase count` per line, msime-dictionary keeps it verbatim in `sources/zhuyin/`) fills them in. A phrase of two or more characters that weighs 0 and that no row counts under any reading takes its `phrase.occ` count times `occurrence_scale`, the median ratio between the chewing count and the `phrase.occ` count of the phrases both count, which puts the counts on chewing's scale. A text some row counts is left alone, since its other readings at 0 are ones chewing chose not to count, and single characters too, since `phrase.occ` counts a character across all its readings. The filled weight never takes a key's first place from an entry chewing counts: on a key that has one, it is capped one below the largest counted weight there, because `phrase.occ` counts substrings of running text, which inflates fragments such as 小的 against the words of the same reading (曉得).
//!
//! 单字的权重是 chewing 手调的，同一个键上常常并列或只差 1（ㄒㄧㄢˋ 下 獻 限 陷 都是 15437，現 線 縣 都是 15436；ㄓㄨˇ 下 煮 5413、主 5412），并列时按字的码位排，于是 獻 排在 現 前面，凍 排在 動 前面。`break_character_ties` 用 `phrase.occ` 打破这种并列：一个键上从最重的单字起，权重不比它轻 1 以上的单字算一组，组里只有一个读音的字（整个库里只在这一个键上有单字条目）按 `phrase.occ` 的次数重排，多音字留在原来的位置，因为 `phrase.occ` 把一个字所有读音的次数算在一起。重排之后整个键上的单字按新次序重新给权重（`renumber`）：第一位保持这个键原来最重的单字权重（九键注音的 `AMBIGUOUS_WORD_FLOOR` 拿它比），之后每一位不超过这一位原来的权重，也不超过上一位的新权重，引擎按码位排并列会把次序排反时再减 1。早先的做法是组内逐个减 1、到组外下一个字的权重加 1 为止，组里的字多于空出来的权重时，底下几个字就并列在一起，引擎按码位排它们，`phrase.occ` 的次序被打乱（ㄒㄧㄥˊ 下 型 34993、行 34993、刑 34992、形 34992、邢 34990，型 被排到 刑 后面）。按新次序往下推时，紧贴在下面的字会跟着挪 1 到 2；单字权重密集的冷僻字一端可能被推到 0 以下，这时从最下面的组起放弃重排，直到没有负权重（`dict-v2.0.56` 的输入上有 3 个这样的组，组里最重的字都不到 10 分）。
//!
//! The syllable inventory is every syllable some key uses. A syllable must be at most one initial, one medial and one rime in that order, then an optional tone mark, because that is all the Dachen editor can compose; anything else fails the build. The only rows left out are the four tone marks listed as their own text and reading, which have no letters to type.

use std::collections::{BTreeMap, BTreeSet, HashMap, HashSet};
use std::path::Path;

use anyhow::{bail, Context, Result};
use msime_engine::zhuyin::layout::{self, Kind};

pub const PHRASES: &str = "sources/zhuyin/tsi.csv";
pub const CHARACTERS: &str = "sources/zhuyin/word.csv";
pub const SUPPLEMENT: &str = "sources/zhuyin/mcbopomofo-supplement.txt";
pub const OCCURRENCES: &str = "sources/zhuyin/phrase.occ";
/// The sources lock reference whose commit is recorded as the database's `source_commit`.
pub const REFERENCE: &str = "libchewing-data";
/// 记入数据库 `license` 的 SPDX 表达式，每个进入库里的来源各一项：`tsi.csv` 与 `word.csv` 的词条按文件头声明是 LGPL-2.1-or-later；McBopomofo 补充表的词条源自 libtabe 的 `tsi.src`（BSD-3-Clause），McBopomofo 的改动和用来补权重的 `phrase.occ` 是 MIT。随库分发的 `LICENSE_NAME` 带着这三项的声明。
pub const LICENSE: &str = "LGPL-2.1-or-later AND BSD-3-Clause AND MIT";
pub const DATABASE: &str = "msime-zhuyin.db";
/// The licence text in `resources/licenses/` and the name it ships under beside the database.
pub const LICENSE_SOURCE: &str = "libchewing-data-LGPL-2.1.txt";
pub const LICENSE_NAME: &str = "msime-libchewing_data_LICENSE.txt";

/// The floors `verify` enforces on a release build.
pub const FLOORS: Floors = Floors {
    syllables: 1_300,
    characters: 13_000,
    phrases: 120_000,
};

/// 这些键的前 `within` 条里必须有对应的词，用来发现排序上的大错；后四条守住 `break_character_ties`：chewing 手调的并列和差 1 让 獻、凍、煮 排在前三个键的首位；ㄒㄧㄥˊ 下 型 要排在第三（形、多音字 行 之后），重新给权重时如果让 型 和 刑 并列，引擎按码位会把 型 排到第四。
pub const EXPECTED: [Expected; 6] = [
    Expected {
        key: "ㄋㄧˇ ㄏㄠˇ",
        texts: &["你好"],
        within: 1,
    },
    Expected {
        key: "ㄊㄞˊ ㄨㄢ",
        texts: &["臺灣", "台灣"],
        within: 2,
    },
    Expected {
        key: "ㄒㄧㄢˋ",
        texts: &["現"],
        within: 1,
    },
    Expected {
        key: "ㄉㄨㄥˋ",
        texts: &["動"],
        within: 1,
    },
    Expected {
        key: "ㄓㄨˇ",
        texts: &["主"],
        within: 1,
    },
    Expected {
        key: "ㄒㄧㄥˊ",
        texts: &["型"],
        within: 3,
    },
];

#[derive(Debug, Clone, Copy)]
pub struct Expected {
    pub key: &'static str,
    pub texts: &'static [&'static str],
    pub within: usize,
}

#[derive(Debug, Clone, Copy)]
pub struct Floors {
    pub syllables: usize,
    pub characters: usize,
    pub phrases: usize,
}

/// One row of `tsi.csv` or `word.csv`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Row {
    pub text: String,
    /// Toned syllables, one per character of `text`.
    pub syllables: Vec<String>,
    pub frequency: i64,
}

#[derive(Debug, Default)]
pub struct Dictionary {
    pub syllables: BTreeSet<String>,
    /// `(key, text)` to weight.
    pub entries: BTreeMap<(String, String), i64>,
    /// Entries that took their weight from `phrase.occ`, how many of them the cap held below a counted entry, and the factor that put the counts on chewing's scale.
    pub filled: usize,
    pub capped: usize,
    pub occurrence_scale: i64,
    /// `break_character_ties` 重排了单字次序的键数。
    pub reordered: usize,
}

/// Where a symbol sits in a syllable: initial, then medial, then rime.
fn slot(symbol: char) -> Option<u8> {
    let (_, _, kind) = layout::DACHEN
        .iter()
        .find(|(_, dachen_symbol, _)| *dachen_symbol == symbol)?;
    Some(match kind {
        Kind::Initial => 0,
        Kind::Medial => 1,
        Kind::Rime => 2,
    })
}

/// Whether the Dachen editor can compose `syllable`: one to three bopomofo letters in initial, medial, rime order, each slot at most once, then an optional tone mark.
fn is_syllable(syllable: &str) -> bool {
    let letters = layout::TONE_MARKS[1..]
        .iter()
        .find_map(|mark| syllable.strip_suffix(mark))
        .unwrap_or(syllable);
    let mut previous = None;
    !letters.is_empty()
        && letters.chars().all(|symbol| {
            let Some(slot) = slot(symbol) else {
                return false;
            };
            let ordered = previous.is_none_or(|previous| slot > previous);
            previous = Some(slot);
            ordered
        })
}

/// `text,frequency,reading` rows of the file `name`, skipping `#` lines and naming the 1-based line of a malformed row.
pub fn parse(name: &str, source: &str) -> Result<Vec<Row>> {
    let mut reader = csv::ReaderBuilder::new()
        .has_headers(false)
        .flexible(true)
        .from_reader(source.as_bytes());
    let mut rows = Vec::new();
    for record in reader.records() {
        let record = record.with_context(|| format!("reading {name}"))?;
        // `#` lines are skipped here rather than through the reader's comment option, which leaves them out of its line count.
        if record.get(0).is_some_and(|first| first.starts_with('#')) {
            continue;
        }
        let line = record.position().map_or(0, csv::Position::line);
        let [text, frequency, reading] = record.iter().collect::<Vec<_>>()[..] else {
            bail!("{name}: line {line} is not text,frequency,reading");
        };
        // The tone marks list themselves so chewing can type them; the editor has no letterless syllable to reach them with.
        if text == reading && layout::TONE_MARKS[1..].contains(&reading) {
            continue;
        }
        let frequency = frequency
            .parse::<i64>()
            .ok()
            .filter(|frequency| *frequency >= 0)
            .with_context(|| format!("{name}: line {line}: {frequency:?} is not a frequency"))?;
        // Two tsi.csv rows (博客來, 新媒體) start their reading with a space, so syllables split on any run of spaces.
        let syllables: Vec<String> = reading.split_whitespace().map(str::to_owned).collect();
        if let Some(bad) = syllables.iter().find(|syllable| !is_syllable(syllable)) {
            bail!("{name}: line {line}: {bad:?} is not a bopomofo syllable");
        }
        if text.is_empty() || text.chars().count() != syllables.len() {
            bail!("{name}: line {line}: {text:?} does not have one syllable per character in {reading:?}");
        }
        rows.push(Row {
            text: text.to_owned(),
            syllables,
            frequency,
        });
    }
    Ok(rows)
}

/// `phrase.occ`: `phrase count` per line. The file separates with a space although McBopomofo's data notes say a tab, so either is accepted. A phrase listed twice keeps its larger count.
pub fn parse_occurrences(source: &str) -> Result<HashMap<&str, i64>> {
    let mut counts = HashMap::new();
    for (index, line) in source.lines().enumerate() {
        let parsed = line.split_once([' ', '\t']).and_then(|(text, count)| {
            Some((text, count.parse::<i64>().ok().filter(|count| *count >= 0)?))
        });
        let Some((text, count)) = parsed.filter(|(text, _)| !text.is_empty()) else {
            bail!("{OCCURRENCES}: line {} is not phrase count", index + 1);
        };
        let kept = counts.entry(text).or_insert(count);
        *kept = (*kept).max(count);
    }
    Ok(counts)
}

/// The lower median of `chewing weight / phrase.occ count` (integer division) over the phrases of two or more characters both count, or 1 when they share none.
fn occurrence_scale(
    entries: &BTreeMap<(String, String), i64>,
    occurrences: &HashMap<&str, i64>,
) -> i64 {
    let mut ratios: Vec<i64> = entries
        .iter()
        .filter(|((_, text), weight)| **weight > 0 && text.chars().count() >= 2)
        .filter_map(|((_, text), weight)| {
            let count = *occurrences.get(text.as_str())?;
            (count > 0).then(|| weight / count)
        })
        .collect();
    ratios.sort_unstable();
    ratios
        .get(ratios.len().saturating_sub(1) / 2)
        .copied()
        .unwrap_or(1)
        .max(1)
}

/// The dictionary of the parsed rows of all three files, with `phrase.occ` filling in the phrases no row counts. A `(key, text)` pair keeps the largest frequency any row gives it.
pub fn build(rows: &[Row], occurrences: &HashMap<&str, i64>) -> Dictionary {
    let mut dictionary = Dictionary::default();
    for row in rows {
        dictionary.syllables.extend(row.syllables.iter().cloned());
        let weight = dictionary
            .entries
            .entry((row.syllables.join(" "), row.text.clone()))
            .or_insert(row.frequency);
        *weight = (*weight).max(row.frequency);
    }
    dictionary.occurrence_scale = occurrence_scale(&dictionary.entries, occurrences);
    let counted: HashSet<&str> = rows
        .iter()
        .filter(|row| row.frequency > 0)
        .map(|row| row.text.as_str())
        .collect();
    // The largest counted weight on each key, which a filled weight stays below.
    let mut top: HashMap<&str, i64> = HashMap::new();
    for ((key, _), weight) in &dictionary.entries {
        if *weight > 0 {
            let slot = top.entry(key.as_str()).or_insert(0);
            *slot = (*slot).max(*weight);
        }
    }
    let mut fills = Vec::new();
    for ((key, text), weight) in &dictionary.entries {
        if *weight != 0 || text.chars().count() < 2 || counted.contains(text.as_str()) {
            continue;
        }
        let Some(&count) = occurrences.get(text.as_str()).filter(|count| **count > 0) else {
            continue;
        };
        let wanted = count.saturating_mul(dictionary.occurrence_scale);
        let filled = top
            .get(key.as_str())
            .map_or(wanted, |top| wanted.min(top - 1));
        if filled > 0 {
            fills.push(((key.clone(), text.clone()), filled, filled < wanted));
        }
    }
    for (entry, weight, capped) in fills {
        dictionary.entries.insert(entry, weight);
        dictionary.filled += 1;
        dictionary.capped += usize::from(capped);
    }
    dictionary.reordered = break_character_ties(&mut dictionary.entries, occurrences);
    dictionary
}

/// 按 `phrase.occ` 打破单字在同一个键上的并列和差 1，规则见模块文档。返回重排了次序的键数。
fn break_character_ties(
    entries: &mut BTreeMap<(String, String), i64>,
    occurrences: &HashMap<&str, i64>,
) -> usize {
    let mut readings: HashMap<&str, usize> = HashMap::new();
    let mut by_key: BTreeMap<&str, Vec<(&str, i64)>> = BTreeMap::new();
    for ((key, text), weight) in entries.iter() {
        if text.chars().nth(1).is_none() {
            *readings.entry(text.as_str()).or_insert(0) += 1;
            by_key
                .entry(key.as_str())
                .or_default()
                .push((text.as_str(), *weight));
        }
    }
    let count = |text: &str| occurrences.get(text).copied().unwrap_or(0);
    let mut updates: Vec<((String, String), i64)> = Vec::new();
    let mut reordered = 0;
    for (key, mut characters) in by_key {
        // 引擎的顺序：权重从大到小，同权重按字排。
        characters.sort_by(|(left, left_weight), (right, right_weight)| {
            right_weight.cmp(left_weight).then(left.cmp(right))
        });
        // 每个要重排的组：组在 `characters` 里的起止和重排后的字。
        let mut groups: Vec<(usize, usize, Vec<&str>)> = Vec::new();
        let mut start = 0;
        while start < characters.len() {
            let top = characters[start].1;
            let end = start
                + characters[start..]
                    .iter()
                    .take_while(|(_, weight)| *weight >= top - 1)
                    .count();
            let group = &characters[start..end];
            let movable: Vec<usize> = (0..group.len())
                .filter(|&index| readings[group[index].0] == 1 && count(group[index].0) > 0)
                .collect();
            let mut sorted = movable.clone();
            sorted.sort_by_key(|&index| std::cmp::Reverse(count(group[index].0)));
            if sorted != movable {
                let mut order: Vec<&str> = group.iter().map(|(text, _)| *text).collect();
                for (slot, index) in movable.iter().zip(&sorted) {
                    order[*slot] = group[*index].0;
                }
                groups.push((start, end, order));
            }
            start = end;
        }
        // 重新给权重会把下面的字往下推；推到 0 以下时，从最下面的组起放弃重排，直到不再出现负权重。
        while let Some(weights) = renumber(&characters, &groups) {
            if weights.iter().all(|(_, weight)| *weight >= 0) {
                for (text, weight) in weights {
                    if characters.iter().any(|(original, original_weight)| {
                        *original == text && *original_weight != weight
                    }) {
                        updates.push(((key.to_owned(), text.to_owned()), weight));
                    }
                }
                reordered += 1;
                break;
            }
            groups.pop();
        }
    }
    for (entry, weight) in updates {
        entries.insert(entry, weight);
    }
    reordered
}

/// 按 `groups` 重排之后的次序给一个键上的单字重新定权重，`characters` 是按引擎顺序排好的原始单字和权重；没有要重排的组时返回 `None`。第一位保持原来最重的权重，之后每一位取「这一位原来的权重」和「上一位的新权重」中较小的一个，引擎按码位排并列时会把上一位排到后面的话再减 1。这样重排过的字不会并列在码位决定的次序上，下面的字只在必须让位时才往下挪。
fn renumber<'a>(
    characters: &[(&'a str, i64)],
    groups: &[(usize, usize, Vec<&'a str>)],
) -> Option<Vec<(&'a str, i64)>> {
    if groups.is_empty() {
        return None;
    }
    let mut order: Vec<&str> = characters.iter().map(|(text, _)| *text).collect();
    for (start, end, texts) in groups {
        order[*start..*end].copy_from_slice(texts);
    }
    let mut weights: Vec<(&str, i64)> = Vec::with_capacity(order.len());
    for (position, text) in order.into_iter().enumerate() {
        let slot = characters[position].1;
        let weight = match weights.last() {
            None => slot,
            Some((previous, previous_weight)) if *previous < text => slot.min(*previous_weight),
            Some((_, previous_weight)) => slot.min(previous_weight - 1),
        };
        weights.push((text, weight));
    }
    Some(weights)
}

/// Writes `dictionary` to `path` in the engine's language dictionary schema, replacing any earlier file, and freezes it so the same inputs give the same bytes.
pub fn write(dictionary: &Dictionary, path: &Path, source_commit: &str) -> Result<()> {
    crate::language_writer::write(
        path,
        source_commit,
        LICENSE,
        dictionary.syllables.iter().map(String::as_str),
        &dictionary.entries,
    )
}

/// What `verify` counted in a written database.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Counts {
    pub syllables: usize,
    pub characters: usize,
    pub phrases: usize,
}

/// Opens a written database the way the engine does and fails when it is below `floors` or a key of `expected` lacks its text among its first entries.
pub fn verify(path: &Path, floors: Floors, expected: &[Expected]) -> Result<Counts> {
    let (dictionary, connection) = crate::language_verify::open(path)?;
    let count = |sql| crate::language_verify::count(&connection, sql);
    let counts = Counts {
        syllables: count("SELECT count(*) FROM syllables")?,
        characters: count("SELECT count(*) FROM entries WHERE length(text) = 1")?,
        phrases: count("SELECT count(*) FROM entries WHERE length(text) > 1")?,
    };
    crate::language_verify::check_floors(
        path,
        &[
            ("syllables", counts.syllables, floors.syllables),
            ("character entries", counts.characters, floors.characters),
            ("phrase entries", counts.phrases, floors.phrases),
        ],
    )?;
    for check in expected {
        let top = dictionary
            .lookup(check.key, check.within)
            .map_err(|error| anyhow::anyhow!("{}: {error}", path.display()))?;
        if !top
            .iter()
            .any(|entry| check.texts.contains(&entry.text.as_str()))
        {
            let found: Vec<&str> = top.iter().map(|entry| entry.text.as_str()).collect();
            bail!(
                "{}: {:?} offers {found:?} first, without {}",
                path.display(),
                check.key,
                check.texts.join(" or ")
            );
        }
    }
    Ok(counts)
}

#[cfg(test)]
mod tests {
    use super::*;
    use msime_engine::language_dictionary;

    const HEADER: &str = "# dc:title,內建詞庫,\n# dc:rights,Copyright (c) 2025 libchewing Core Team,\n# dc:license,LGPL-2.1-or-later,\n# dc:identifier,2025.11.17,\n";

    fn phrases_source() -> String {
        format!("{HEADER}ˇ,1,ˇ\nㄅ,0,ㄅ\n你,89941,ㄋㄧˇ\n妳,16127,ㄋㄧˇ\n好,30909,ㄏㄠˇ\n好,309,ㄏㄠˋ\n你好,1227,ㄋㄧˇ ㄏㄠˇ\n妳好,427,ㄋㄧˇ ㄏㄠˇ\n台灣,124258,ㄊㄞˊ ㄨㄢ\n臺灣,15908,ㄊㄞˊ ㄨㄢ\n博客來,6,  ㄅㄛˊ ㄎㄜˋ ㄌㄞˊ\n你好,1000,ㄋㄧˇ ㄏㄠˇ\n")
    }

    fn characters_source() -> String {
        format!("{HEADER}ˋ,0,ˋ\n你,0,ㄋㄧˇ\n妳,0,ㄋㄞˇ\n兒,0,ㄦ\n誒,0,ㄝˋ\n")
    }

    fn sample() -> Dictionary {
        let mut rows = parse(PHRASES, &phrases_source()).unwrap();
        rows.extend(parse(CHARACTERS, &characters_source()).unwrap());
        build(&rows, &HashMap::new())
    }

    fn entries(dictionary: &Dictionary) -> Vec<(&str, &str, i64)> {
        dictionary
            .entries
            .iter()
            .map(|((key, text), weight)| (key.as_str(), text.as_str(), *weight))
            .collect()
    }

    #[test]
    fn rows_after_the_header_keep_their_toned_syllables() {
        let rows = parse(PHRASES, &phrases_source()).unwrap();
        // The tone mark row is left out.
        assert_eq!(rows.len(), 11);
        assert_eq!(
            rows[0],
            Row {
                text: "ㄅ".to_owned(),
                syllables: vec!["ㄅ".to_owned()],
                frequency: 0
            }
        );
        assert_eq!(rows[5].syllables, ["ㄋㄧˇ", "ㄏㄠˇ"]);
        // Tone 1 is unmarked.
        assert_eq!(rows[7].syllables, ["ㄊㄞˊ", "ㄨㄢ"]);
        // The leading spaces of the reading do not make an empty syllable.
        assert_eq!(rows[9].syllables, ["ㄅㄛˊ", "ㄎㄜˋ", "ㄌㄞˊ"]);
    }

    #[test]
    fn a_row_keeps_its_largest_frequency_across_both_files() {
        assert_eq!(
            entries(&sample()),
            [
                ("ㄅ", "ㄅ", 0),
                ("ㄅㄛˊ ㄎㄜˋ ㄌㄞˊ", "博客來", 6),
                ("ㄊㄞˊ ㄨㄢ", "台灣", 124258),
                ("ㄊㄞˊ ㄨㄢ", "臺灣", 15908),
                // word.csv gives 妳 this reading only, so it keeps the word.csv weight of 0.
                ("ㄋㄞˇ", "妳", 0),
                // tsi.csv counts 你 under the reading word.csv also lists, so the count wins over 0.
                ("ㄋㄧˇ", "你", 89941),
                ("ㄋㄧˇ", "妳", 16127),
                // 你好 is listed twice; the larger count wins.
                ("ㄋㄧˇ ㄏㄠˇ", "你好", 1227),
                ("ㄋㄧˇ ㄏㄠˇ", "妳好", 427),
                ("ㄏㄠˇ", "好", 30909),
                ("ㄏㄠˋ", "好", 309),
                ("ㄝˋ", "誒", 0),
                ("ㄦ", "兒", 0),
            ]
        );
    }

    #[test]
    fn phrase_occ_fills_the_phrases_no_row_counts() {
        let rows = parse(
            PHRASES,
            &format!("{HEADER}你好,1227,ㄋㄧˇ ㄏㄠˇ\n台灣,124258,ㄊㄞˊ ㄨㄢ\n曉得,480,ㄒㄧㄠˇ ㄉㄜ˙\n黃埔江,8,ㄏㄨㄤˊ ㄆㄨˇ ㄐㄧㄤ\n鬱卒,0,ㄩˋ ㄗㄨˊ\n一一,0,ㄧ ㄧ\n一一,2095,ㄧˊ ㄧˊ\n好,30909,ㄏㄠˇ\n"),
        )
        .unwrap();
        let mut rows = rows;
        rows.extend(
            parse(
                SUPPLEMENT,
                "# supplement\n小的,0,ㄒㄧㄠˇ ㄉㄜ˙\n黃浦江,0,ㄏㄨㄤˊ ㄆㄨˇ ㄐㄧㄤ\n冷僻詞,0,ㄌㄥˇ ㄆㄧˋ ㄘˊ\n",
            )
            .unwrap(),
        );
        rows.extend(parse(CHARACTERS, &format!("{HEADER}好,0,ㄏㄠˋ\n")).unwrap());
        // 一一 (ㄧˊ ㄧˊ) 2095/762, 你好 1227/300 and 台灣 124258/24111 give ratios 2, 4 and 5 (曉得 and 黃埔江 have no phrase.occ count); the lower median is 4.
        let occurrences = parse_occurrences(
            "你好 300\n台灣 24111\n小的 673\n黃浦江 21\n鬱卒 7\n一一 762\n好 99999\n冷僻詞 0\n",
        )
        .unwrap();
        let dictionary = build(&rows, &occurrences);
        assert_eq!(dictionary.occurrence_scale, 4);
        let weight = |key: &str, text: &str| dictionary.entries[&(key.to_owned(), text.to_owned())];
        // A key without a counted entry takes the scaled count.
        assert_eq!(weight("ㄩˋ ㄗㄨˊ", "鬱卒"), 28);
        // 黃浦江 (84) would pass 黃埔江 (8), so it is held one below it.
        assert_eq!(weight("ㄏㄨㄤˊ ㄆㄨˇ ㄐㄧㄤ", "黃浦江"), 7);
        assert_eq!(weight("ㄒㄧㄠˇ ㄉㄜ˙", "小的"), 479);
        // 一一 is counted under another reading, a single character is never filled, and a 0 count fills nothing.
        assert_eq!(weight("ㄧ ㄧ", "一一"), 0);
        assert_eq!(weight("ㄏㄠˋ", "好"), 0);
        assert_eq!(weight("ㄌㄥˇ ㄆㄧˋ ㄘˊ", "冷僻詞"), 0);
        assert_eq!((dictionary.filled, dictionary.capped), (3, 2));
    }

    #[test]
    fn phrase_occ_breaks_ties_between_single_reading_characters() {
        let mut rows = parse(
            PHRASES,
            &format!("{HEADER}獻,15437,ㄒㄧㄢˋ\n限,15437,ㄒㄧㄢˋ\n現,15436,ㄒㄧㄢˋ\n線,15436,ㄒㄧㄢˋ\n憲,15000,ㄒㄧㄢˋ\n型,34993,ㄒㄧㄥˊ\n行,34993,ㄒㄧㄥˊ\n刑,34992,ㄒㄧㄥˊ\n形,34992,ㄒㄧㄥˊ\n邢,34990,ㄒㄧㄥˊ\n主,5413,ㄓㄨˇ\n煮,5412,ㄓㄨˇ\n拄,5412,ㄓㄨˇ\n"),
        )
        .unwrap();
        rows.extend(parse(CHARACTERS, &format!("{HEADER}行,0,ㄏㄤˊ\n")).unwrap());
        let occurrences = parse_occurrences(
            "現 40718\n限 6209\n線 8770\n獻 2141\n憲 3000\n型 8593\n形 10460\n刑 1631\n邢 300\n行 43589\n主 35453\n煮 607\n",
        )
        .unwrap();
        let dictionary = build(&rows, &occurrences);
        let order = |key: &str| {
            let mut found: Vec<(&str, i64)> = dictionary
                .entries
                .iter()
                .filter(|((entry_key, _), _)| entry_key == key)
                .map(|((_, text), weight)| (text.as_str(), *weight))
                .collect();
            found.sort_by(|left, right| right.1.cmp(&left.1).then(left.0.cmp(right.0)));
            found
        };
        // 四个字都只有一个读音，按 phrase.occ 重排：現 保持 15437；線 的码位在 現 之后，并列也排在它后面，可以同为 15437；限 取它这一位原来的 15436；獻 的码位在 限 之前，并列会排到 限 前面，所以再减 1。憲 不在组里，不动。
        assert_eq!(
            order("ㄒㄧㄢˋ"),
            [
                ("現", 15437),
                ("線", 15437),
                ("限", 15436),
                ("獻", 15435),
                ("憲", 15000)
            ]
        );
        // 行 还读 ㄏㄤˊ，留在原来的第二位；型 刑 形 在剩下的位置上按 phrase.occ 排。这是 dict-v2.0.56 的真实权重：组下面紧贴着 邢 34990，原来的做法到 34991 就停，型 和 刑 并列，按码位 刑 排到了 型 前面。现在 型 34992、刑 34991，邢 仍是 34990。
        assert_eq!(
            order("ㄒㄧㄥˊ"),
            [
                ("形", 34993),
                ("行", 34993),
                ("型", 34992),
                ("刑", 34991),
                ("邢", 34990)
            ]
        );
        // 主 已经在 煮 前面；拄 没有 phrase.occ 的次数，不参与重排，这一组不动。
        assert_eq!(order("ㄓㄨˇ"), [("主", 5413), ("拄", 5412), ("煮", 5412)]);
        assert_eq!(dictionary.reordered, 2);
    }

    #[test]
    fn a_reordered_group_pushes_the_characters_below_it_instead_of_tying() {
        let rows = parse(
            PHRASES,
            &format!("{HEADER}甲,101,ㄐㄧㄚˇ\n乙,101,ㄐㄧㄚˇ\n丙,100,ㄐㄧㄚˇ\n丁,100,ㄐㄧㄚˇ\n戊,99,ㄐㄧㄚˇ\n己,90,ㄐㄧㄚˇ\n"),
        )
        .unwrap();
        // 甲 乙 丙 丁 一组，要排成码位从大到小的次序，并列时引擎会把它们排反，所以每一位都比上一位轻 1；戊 99 不在组里，被推到 98，与 丁 并列时码位在后，次序仍对；己 不用让位。
        let occurrences = parse_occurrences("甲 40\n乙 30\n丙 20\n丁 10\n戊 99999\n").unwrap();
        let dictionary = build(&rows, &occurrences);
        let weight = |text: &str| dictionary.entries[&("ㄐㄧㄚˇ".to_owned(), text.to_owned())];
        assert_eq!(
            ["甲", "乙", "丙", "丁", "戊", "己"].map(weight),
            [101, 100, 99, 98, 98, 90]
        );
        assert_eq!(dictionary.reordered, 1);
    }

    #[test]
    fn a_reorder_that_would_push_a_weight_below_zero_is_left_alone() {
        let rows = parse(
            PHRASES,
            &format!("{HEADER}甲,1,ㄐㄧㄚˇ\n乙,1,ㄐㄧㄚˇ\n丙,0,ㄐㄧㄚˇ\n"),
        )
        .unwrap();
        // 甲 要排到 乙 前面：乙 只能降到 0，丙 的码位在 乙 之前，就得降到 -1，所以这一组不重排。
        let occurrences = parse_occurrences("甲 20\n乙 10\n").unwrap();
        let dictionary = build(&rows, &occurrences);
        let weight = |text: &str| dictionary.entries[&("ㄐㄧㄚˇ".to_owned(), text.to_owned())];
        assert_eq!(["甲", "乙", "丙"].map(weight), [1, 1, 0]);
        assert_eq!(dictionary.reordered, 0);
    }

    #[test]
    fn phrase_occ_lines_are_phrase_and_count() {
        let counts = parse_occurrences("一一 762\n一一\t800\nㄅ 41\n").unwrap();
        assert_eq!((counts["一一"], counts["ㄅ"]), (800, 41));
        for source in ["一一\n", "一一 x\n", " 3\n", "一一 -1\n"] {
            let error = parse_occurrences(source).unwrap_err().to_string();
            assert!(
                error.contains("phrase.occ: line 1 is not phrase count"),
                "{error}"
            );
        }
    }

    #[test]
    fn the_inventory_holds_every_syllable_a_key_uses() {
        let dictionary = sample();
        let syllables: Vec<&str> = dictionary.syllables.iter().map(String::as_str).collect();
        assert_eq!(
            syllables,
            [
                "ㄅ", "ㄅㄛˊ", "ㄊㄞˊ", "ㄋㄞˇ", "ㄋㄧˇ", "ㄌㄞˊ", "ㄎㄜˋ", "ㄏㄠˇ", "ㄏㄠˋ",
                "ㄝˋ", "ㄦ", "ㄨㄢ"
            ]
        );
    }

    #[test]
    fn syllables_follow_the_slots_the_editor_fills() {
        for syllable in [
            "ㄅ",
            "ㄧ",
            "ㄚ",
            "ㄓ",
            "ㄋㄧˇ",
            "ㄓㄨㄤ",
            "ㄌㄩㄝˋ",
            "ㄇㄚ˙",
            "ㄦˊ",
        ] {
            assert!(is_syllable(syllable), "{syllable}");
        }
        for syllable in [
            "",
            "ˇ",
            "ㄧㄋ",
            "ㄋㄏ",
            "ㄚㄛ",
            "ㄋㄧˇˇ",
            "ˇㄋㄧ",
            "ㄋㄧˉ",
            "ㄋa",
        ] {
            assert!(!is_syllable(syllable), "{syllable}");
        }
    }

    #[test]
    fn malformed_rows_name_their_file_and_line() {
        for (source, expected) in [
            (
                format!("{HEADER}你,1\n"),
                "tsi.csv: line 5 is not text,frequency,reading",
            ),
            (format!("{HEADER}你,1,ㄋㄧˇ,x\n"), "line 5 is not"),
            (
                format!("{HEADER}你,-1,ㄋㄧˇ\n"),
                "line 5: \"-1\" is not a frequency",
            ),
            (
                format!("{HEADER}你,many,ㄋㄧˇ\n"),
                "\"many\" is not a frequency",
            ),
            (
                format!("{HEADER}你,1,ni3\n"),
                "line 5: \"ni3\" is not a bopomofo syllable",
            ),
            (
                format!("{HEADER}你,1,ㄧㄋˇ\n"),
                "\"ㄧㄋˇ\" is not a bopomofo syllable",
            ),
            (
                format!("{HEADER}你好,1,ㄋㄧˇ\n"),
                "line 5: \"你好\" does not have one syllable per character",
            ),
            (format!("{HEADER},1,\n"), "\"\" does not have one syllable"),
            // A tone mark whose text is something else is not the tone mark listing itself.
            (
                format!("{HEADER}啊,1,ˇ\n"),
                "\"ˇ\" is not a bopomofo syllable",
            ),
        ] {
            let error = parse(PHRASES, &source).unwrap_err().to_string();
            assert!(error.contains(expected), "{error}");
        }
    }

    #[test]
    fn the_inputs_are_recorded_upstream_copies() {
        let lock = crate::sources::Lock::load(
            &Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../resources/dictionary-sources.lock.json"),
        )
        .unwrap();
        assert_eq!(lock.references[REFERENCE].commit.len(), 40);
        for input in [PHRASES, CHARACTERS] {
            assert_eq!(
                crate::sources::upstream_reference(input),
                Some(REFERENCE),
                "{input}"
            );
        }
        for input in [SUPPLEMENT, OCCURRENCES] {
            assert_eq!(
                crate::sources::upstream_reference(input),
                Some("McBopomofo"),
                "{input}"
            );
        }
        assert!(!lock
            .files
            .iter()
            .any(|file| file.path.starts_with("sources/zhuyin/")));
    }

    fn written(dictionary: &Dictionary) -> (tempfile::TempDir, std::path::PathBuf) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join(DATABASE);
        write(
            dictionary,
            &path,
            "c44e81aef24b06f1509f19e1be54c99812d0c43f",
        )
        .unwrap();
        (dir, path)
    }

    /// 补充表的词条源自 libtabe 的 `tsi.src`（BSD-3-Clause，二进制分发要附上版权、条款和免责声明），McBopomofo 的改动和 `phrase.occ` 的权重是 MIT，所以 `license` 三项都要写，随库分发的许可证文本也要带着两份声明的原文。
    #[test]
    fn the_licence_names_every_source_the_entries_and_weights_come_from() {
        assert_eq!(LICENSE, "LGPL-2.1-or-later AND BSD-3-Clause AND MIT");
        let text = std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../resources/licenses")
                .join(LICENSE_SOURCE),
        )
        .unwrap();
        for phrase in [
            "GNU LESSER GENERAL PUBLIC LICENSE",
            "Copyrighy (c) 1999 TaBE Project.",
            "Copyright (c) 1999 Computer Systems and Communication Lab,",
            "Redistributions in binary form must reproduce the above copyright",
            "Copyright 1996 Chih-Hao Tsai @ Beckman Institute, University of Illinois",
            "Copyright (c) 2011-2026 Mengjuei Hsieh et al.",
            "Permission is hereby granted, free of charge",
        ] {
            assert!(text.contains(phrase), "{LICENSE_SOURCE} lacks {phrase:?}");
        }
    }

    #[test]
    fn the_engine_reads_what_is_written_and_the_bytes_repeat() {
        let dictionary = sample();
        let (_dir, path) = written(&dictionary);
        let engine = language_dictionary::open_read_only(&path).unwrap();
        let texts: Vec<String> = engine
            .lookup("ㄋㄧˇ", 10)
            .unwrap()
            .into_iter()
            .map(|entry| entry.text)
            .collect();
        assert_eq!(texts, ["你", "妳"]);
        assert!(engine.has_syllable("ㄨㄢ").unwrap());
        assert!(!engine.has_syllable("ˇ").unwrap());
        assert_eq!(
            engine
                .metadata(language_dictionary::METADATA_SOURCE_COMMIT)
                .unwrap()
                .as_deref(),
            Some("c44e81aef24b06f1509f19e1be54c99812d0c43f")
        );
        assert_eq!(
            engine
                .metadata(language_dictionary::METADATA_LICENSE)
                .unwrap()
                .as_deref(),
            Some("LGPL-2.1-or-later AND BSD-3-Clause AND MIT")
        );
        let first = std::fs::read(&path).unwrap();
        write(
            &dictionary,
            &path,
            "c44e81aef24b06f1509f19e1be54c99812d0c43f",
        )
        .unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), first);
    }

    #[test]
    fn verify_enforces_the_floors_and_the_expected_entries() {
        let (_dir, path) = written(&sample());
        // 合成的样例只有前两条要检查的词，后三条单字检查对着真实的库。
        let sample_expected = &EXPECTED[..2];
        let floors = Floors {
            syllables: 12,
            characters: 8,
            phrases: 5,
        };
        assert_eq!(
            verify(&path, floors, sample_expected).unwrap(),
            Counts {
                syllables: 12,
                characters: 8,
                phrases: 5
            }
        );
        for (below, what) in [
            (
                Floors {
                    syllables: 13,
                    ..floors
                },
                "12 syllables, below the floor of 13",
            ),
            (
                Floors {
                    characters: 9,
                    ..floors
                },
                "8 character entries, below the floor of 9",
            ),
            (
                Floors {
                    phrases: 6,
                    ..floors
                },
                "5 phrase entries, below the floor of 6",
            ),
        ] {
            let error = verify(&path, below, sample_expected)
                .unwrap_err()
                .to_string();
            assert!(error.contains(what), "{error}");
        }
        let error = verify(&path, FLOORS, sample_expected)
            .unwrap_err()
            .to_string();
        assert!(error.contains("below the floor"), "{error}");

        // 妳好 outranking 你好 fails the first-place check.
        let mut rows = parse(PHRASES, &phrases_source()).unwrap();
        rows.push(Row {
            text: "妳好".to_owned(),
            syllables: vec!["ㄋㄧˇ".to_owned(), "ㄏㄠˇ".to_owned()],
            frequency: 5000,
        });
        let (_dir, path) = written(&build(&rows, &HashMap::new()));
        let none = Floors {
            syllables: 0,
            characters: 0,
            phrases: 0,
        };
        let error = verify(&path, none, sample_expected)
            .unwrap_err()
            .to_string();
        assert!(error.contains("[\"妳好\"] first, without 你好"), "{error}");
        // Either spelling of Taiwan within the first two passes.
        assert!(verify(&path, none, &EXPECTED[1..2]).is_ok());
    }
}
