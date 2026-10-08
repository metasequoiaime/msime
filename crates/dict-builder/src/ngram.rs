//! `msime-bigram.bin` and `msime-trigram.bin`: the lattice's context tables, counted over the pinned zhwiki dump.
//!
//! Text is segmented with the greedy longest match the decoder's vocabulary implies, and each entry holds an increment rather than a probability: `log(P(next | previous) / P(next))` for pairs, `log(P(next | before, previous) / P(next | previous))` for triples. An absent entry therefore contributes nothing, and the decoder can add both tables to its unigram score. Bonuses are shrunk toward zero by `count / (count + 5)` and clamped to ±3 so rare pairs adjust the ranking rather than replace it.
//!
//! File layout, little-endian: `MSNG`, version (u32 = 1), entry count (u32), reserved (u32), the sorted FNV-1a 64 keys of the `\0`-joined UTF-8 words, then one f32 bonus per key.
//!
//! The corpus pass reproduces the Python builder's reading exactly (1 Mi-character chunks, the `<text>` extraction and noise stripping per chunk, the 120M Han-character stop, singleton pruning past 12M entries, and insertion-ordered tie-breaking at the 1M-entry cut), because each of those decides which entries ship.

use std::collections::HashMap;
use std::fs::{File, OpenOptions};
use std::io::{BufReader, Read};
use std::path::Path;
use std::sync::LazyLock;

use anyhow::{bail, Context, Result};
use indexmap::IndexMap;
use regex::Regex;
use rusqlite::Connection;

pub const MAGIC: &[u8; 4] = b"MSNG";
pub const VERSION: u32 = 1;
const MAX_CHARS: usize = 120_000_000;
const MIN_COUNT: u32 = 4;
const SHRINKAGE: f64 = 5.0;
const CLAMP: f64 = 3.0;
const LIMIT: usize = 1_000_000;
const PRUNE_ABOVE: usize = 12_000_000;
/// One Han character is one syllable, and the dictionary stops at eight-syllable phrases.
const MAX_WORD_CHARS: usize = 8;
const CHUNK_CHARS: usize = 1 << 20;
const TAIL_CHARS: usize = 1 << 16;
/// Stands in for the start of a sentence so its first word gets a context too.
const START: u32 = 0;
const START_TEXT: &str = "\u{1}";

// `<text\b[^>]*>` spelled without `\b`, which would push the regex engine off its DFA on non-ASCII text: after `<text` comes either `>` or a non-word character, then the rest of the tag.
static WIKI_TEXT: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r"(?s)<text(?:>|[^\w>][^>]*>).*?</text>").expect("valid regex"));
static WIKI_NOISE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"(?s)\{\{[^{}]*\}\}|<ref[^>]*>.*?</ref>|<[^>]+>|\[\[[^\]|]*\||[\[\]{}|']")
        .expect("valid regex")
});

fn is_han(c: char) -> bool {
    ('\u{4e00}'..='\u{9fff}').contains(&c)
}

pub struct Vocabulary {
    ids: HashMap<String, u32>,
    words: Vec<String>,
    longest: usize,
}

impl Vocabulary {
    /// Every pinyin-table value of one to eight Han characters.
    pub fn load(msime: &Connection) -> Result<Self> {
        let tables: Vec<String> = msime
            .prepare("select name from sqlite_master where type='table' and name like 'tbl_%'")?
            .query_map([], |row| row.get(0))?
            .collect::<rusqlite::Result<_>>()?;
        let mut words = Vec::new();
        for table in tables {
            let mut statement = msime.prepare(&format!("select value from {table}"))?;
            let mut rows = statement.query([])?;
            while let Some(row) = rows.next()? {
                if let Some(value) = row.get::<_, Option<String>>(0)? {
                    words.push(value);
                }
            }
        }
        Ok(Self::new(words))
    }

    pub fn new(values: impl IntoIterator<Item = String>) -> Self {
        let mut vocabulary = Self {
            ids: HashMap::new(),
            words: vec![START_TEXT.to_owned()],
            longest: 0,
        };
        for value in values {
            let length = value.chars().count();
            if !(1..=MAX_WORD_CHARS).contains(&length)
                || !value.chars().all(is_han)
                || vocabulary.ids.contains_key(&value)
            {
                continue;
            }
            vocabulary.longest = vocabulary.longest.max(length);
            vocabulary
                .ids
                .insert(value.clone(), vocabulary.words.len() as u32);
            vocabulary.words.push(value);
        }
        vocabulary
    }

    pub fn len(&self) -> usize {
        self.ids.len()
    }

    /// Greedy longest match over a run of Han characters (three UTF-8 bytes each). `None` marks a character the dictionary does not know, which breaks the chain.
    fn segment(&self, run: &str, words: &mut Vec<Option<u32>>) {
        words.clear();
        let characters = run.len() / 3;
        let mut position = 0;
        while position < characters {
            let found = (1..=self.longest.min(characters - position))
                .rev()
                .find_map(|length| {
                    self.ids
                        .get(&run[position * 3..(position + length) * 3])
                        .map(|id| (length, *id))
                });
            match found {
                Some((length, id)) => {
                    words.push(Some(id));
                    position += length;
                }
                None => {
                    words.push(None);
                    position += 1;
                }
            }
        }
    }
}

/// Decodes a byte stream as UTF-8, dropping invalid sequences (`errors="ignore"`), and hands out chunks of exactly `CHUNK_CHARS` characters (fewer only at the end).
struct CharChunks<R> {
    reader: R,
    undecoded: Vec<u8>,
    decoded: String,
    decoded_chars: usize,
    eof: bool,
}

impl<R: Read> CharChunks<R> {
    fn new(reader: R) -> Self {
        Self {
            reader,
            undecoded: Vec::new(),
            decoded: String::new(),
            decoded_chars: 0,
            eof: false,
        }
    }

    fn decode(&mut self) {
        let mut rest: &[u8] = &self.undecoded;
        let mut keep = Vec::new();
        loop {
            match std::str::from_utf8(rest) {
                Ok(valid) => {
                    self.decoded_chars += valid.chars().count();
                    self.decoded.push_str(valid);
                    break;
                }
                Err(error) => {
                    let (valid, after) = rest.split_at(error.valid_up_to());
                    let valid = std::str::from_utf8(valid).unwrap_or_default();
                    self.decoded_chars += valid.chars().count();
                    self.decoded.push_str(valid);
                    match error.error_len() {
                        Some(length) => rest = &after[length..],
                        None => {
                            // An incomplete sequence at the end: wait for more bytes, or drop it at end of input.
                            if !self.eof {
                                keep = after.to_vec();
                            }
                            break;
                        }
                    }
                }
            }
        }
        self.undecoded = keep;
    }

    fn next_chunk(&mut self) -> Result<Option<String>> {
        let mut buffer = vec![0u8; 1 << 20];
        while self.decoded_chars < CHUNK_CHARS && !self.eof {
            let read = self.reader.read(&mut buffer)?;
            if read == 0 {
                self.eof = true;
            } else {
                self.undecoded.extend_from_slice(&buffer[..read]);
            }
            self.decode();
        }
        if self.decoded.is_empty() {
            return Ok(None);
        }
        if self.decoded_chars <= CHUNK_CHARS {
            self.decoded_chars = 0;
            return Ok(Some(std::mem::take(&mut self.decoded)));
        }
        let split = self
            .decoded
            .char_indices()
            .nth(CHUNK_CHARS)
            .map_or(self.decoded.len(), |(index, _)| index);
        let rest = self.decoded.split_off(split);
        self.decoded_chars -= CHUNK_CHARS;
        Ok(Some(std::mem::replace(&mut self.decoded, rest)))
    }
}

fn last_chars(text: &str, count: usize) -> &str {
    match text.char_indices().rev().nth(count - 1) {
        Some((index, _)) => &text[index..],
        None => text,
    }
}

/// Calls `visit` with each maximal run of two or more Han characters in the dump's article text, stopping once `max_chars` characters have been visited.
fn for_each_han_run(
    reader: impl Read,
    max_chars: usize,
    mut visit: impl FnMut(&str),
) -> Result<()> {
    let mut chunks = CharChunks::new(reader);
    let mut buffer = String::new();
    let mut produced = 0;
    while let Some(chunk) = chunks.next_chunk()? {
        buffer.push_str(&chunk);
        let pieces: Vec<String> = WIKI_TEXT
            .find_iter(&buffer)
            .map(|found| {
                let tag = found.as_str();
                let open = tag.find('>').map_or(0, |index| index + 1);
                WIKI_NOISE
                    .replace_all(&tag[open..tag.len() - "</text>".len()], " ")
                    .into_owned()
            })
            .collect();
        buffer = match buffer.rfind("</text>") {
            Some(cut) => buffer[cut + "</text>".len()..].to_owned(),
            None => last_chars(&buffer, TAIL_CHARS).to_owned(),
        };
        let mut body = pieces.join(" ");
        body.push('\n');
        let mut run_start = None;
        for (index, c) in body.char_indices() {
            if is_han(c) {
                run_start.get_or_insert(index);
                continue;
            }
            if let Some(start) = run_start.take() {
                let run = &body[start..index];
                if run.len() >= 6 {
                    produced += run.len() / 3;
                    visit(run);
                    if produced >= max_chars {
                        return Ok(());
                    }
                }
            }
        }
    }
    Ok(())
}

pub struct Counts {
    unigrams: Vec<u64>,
    bigrams: IndexMap<(u32, u32), u32>,
    trigrams: IndexMap<(u32, u32, u32), u32>,
    pub characters: usize,
}

impl Counts {
    fn new(vocabulary: &Vocabulary) -> Self {
        Self {
            unigrams: vec![0; vocabulary.words.len()],
            bigrams: IndexMap::new(),
            trigrams: IndexMap::new(),
            characters: 0,
        }
    }

    fn count_run(&mut self, vocabulary: &Vocabulary, run: &str, words: &mut Vec<Option<u32>>) {
        self.characters += run.len() / 3;
        let (mut before, mut previous) = (START, START);
        self.unigrams[START as usize] += 1;
        vocabulary.segment(run, words);
        for word in words.iter() {
            let Some(word) = *word else {
                if previous != START {
                    self.unigrams[START as usize] += 1;
                }
                (before, previous) = (START, START);
                continue;
            };
            self.unigrams[word as usize] += 1;
            *self.bigrams.entry((previous, word)).or_insert(0) += 1;
            *self.trigrams.entry((before, previous, word)).or_insert(0) += 1;
            (before, previous) = (previous, word);
        }
        // A sequence seen once will not survive the minimum count anyway, and holding all of them is what exhausts memory on a full dump.
        if self.bigrams.len() > PRUNE_ABOVE {
            self.bigrams.retain(|_, count| *count > 1);
        }
        if self.trigrams.len() > PRUNE_ABOVE {
            self.trigrams.retain(|_, count| *count > 1);
        }
    }
}

pub fn count_corpus(vocabulary: &Vocabulary, corpus: &Path) -> Result<Counts> {
    if vocabulary.len() == 0 {
        bail!("empty vocabulary");
    }
    let file = open_corpus(corpus).with_context(|| format!("opening {}", corpus.display()))?;
    let reader = bzip2::read::MultiBzDecoder::new(BufReader::new(file));
    let mut counts = Counts::new(vocabulary);
    let mut words = Vec::new();
    for_each_han_run(reader, MAX_CHARS, |run| {
        counts.count_run(vocabulary, run, &mut words)
    })?;
    Ok(counts)
}

fn open_corpus(path: &Path) -> std::io::Result<File> {
    let mut options = OpenOptions::new();
    options.read(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.custom_flags(libc::O_NOFOLLOW | libc::O_CLOEXEC | libc::O_NONBLOCK);
    }
    #[cfg(windows)]
    {
        use std::os::windows::fs::OpenOptionsExt;
        const FILE_FLAG_OPEN_REPARSE_POINT: u32 = 0x0020_0000;
        options.custom_flags(FILE_FLAG_OPEN_REPARSE_POINT);
    }
    let file = options.open(path)?;
    if !file.metadata()?.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::InvalidInput,
            "ngram corpus is not a regular file",
        ));
    }
    Ok(file)
}

fn fnv1a64(words: &[&str]) -> u64 {
    let mut digest: u64 = 0xcbf2_9ce4_8422_2325;
    for (index, word) in words.iter().enumerate() {
        let separator: &[u8] = if index == 0 { &[] } else { &[0] };
        for byte in separator.iter().chain(word.as_bytes()) {
            digest = (digest ^ u64::from(*byte)).wrapping_mul(0x100_0000_01b3);
        }
    }
    digest
}

fn bonus(count: u32, conditional: f64, marginal: f64) -> f64 {
    let bonus = (conditional / marginal).ln() * (f64::from(count) / (f64::from(count) + SHRINKAGE));
    bonus.clamp(-CLAMP, CLAMP)
}

/// The packed table for word pairs (`order` 2) or triples (`order` 3).
pub fn pack(vocabulary: &Vocabulary, counts: &Counts, order: u8) -> Result<Vec<u8>> {
    // The start token is a context, never an outcome, so it stays out of the marginal.
    let total: u64 = counts.unigrams.iter().skip(1).sum();
    if total == 0 {
        bail!("corpus produced no words");
    }
    let word = |id: u32| vocabulary.words[id as usize].as_str();
    let mut entries: Vec<(u32, u64, f64)> = Vec::new();
    if order == 3 {
        for (&(before, previous, following), &count) in &counts.trigrams {
            if count < MIN_COUNT {
                continue;
            }
            let context = counts
                .bigrams
                .get(&(before, previous))
                .copied()
                .unwrap_or(0);
            let shorter = counts
                .bigrams
                .get(&(previous, following))
                .copied()
                .unwrap_or(0);
            let previous_count = counts.unigrams[previous as usize];
            // Pruning can leave a triple whose contexts did not survive; a ratio against a missing denominator is no estimate.
            if context < count || shorter == 0 || previous_count == 0 {
                continue;
            }
            let conditional = f64::from(count) / f64::from(context);
            let marginal = f64::from(shorter) / previous_count as f64;
            entries.push((
                count,
                fnv1a64(&[word(before), word(previous), word(following)]),
                bonus(count, conditional, marginal),
            ));
        }
    } else {
        for (&(previous, following), &count) in &counts.bigrams {
            if count < MIN_COUNT {
                continue;
            }
            let conditional = f64::from(count) / counts.unigrams[previous as usize] as f64;
            let marginal = counts.unigrams[following as usize] as f64 / total as f64;
            entries.push((
                count,
                fnv1a64(&[word(previous), word(following)]),
                bonus(count, conditional, marginal),
            ));
        }
    }
    // Stable, so equal counts keep the order the sequences were first seen in: that order decides which ties make the cut.
    entries.sort_by_key(|entry| std::cmp::Reverse(entry.0));
    entries.truncate(LIMIT);
    let mut packed: Vec<(u64, f32)> = entries
        .into_iter()
        .map(|(_, key, value)| (key, value as f32))
        .collect();
    packed.sort_by(|a, b| {
        a.0.cmp(&b.0)
            .then(a.1.partial_cmp(&b.1).unwrap_or(std::cmp::Ordering::Equal))
    });

    let mut output = Vec::with_capacity(16 + packed.len() * 12);
    output.extend_from_slice(MAGIC);
    output.extend_from_slice(&VERSION.to_le_bytes());
    output.extend_from_slice(&u32::try_from(packed.len())?.to_le_bytes());
    output.extend_from_slice(&0u32.to_le_bytes());
    for (key, _) in &packed {
        output.extend_from_slice(&key.to_le_bytes());
    }
    for (_, value) in &packed {
        output.extend_from_slice(&value.to_le_bytes());
    }
    Ok(output)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[cfg(unix)]
    #[test]
    fn corpus_reader_rejects_a_symlink() {
        use std::os::unix::fs::symlink;

        let outside = tempfile::tempdir().unwrap();
        let root = tempfile::tempdir().unwrap();
        let target = outside.path().join("corpus.xml.bz2");
        let mut encoder = bzip2::write::BzEncoder::new(
            std::fs::File::create(&target).unwrap(),
            bzip2::Compression::default(),
        );
        std::io::Write::write_all(&mut encoder, "<text>配置</text>".as_bytes()).unwrap();
        encoder.finish().unwrap();
        let linked = root.path().join("corpus.xml.bz2");
        symlink(&target, &linked).unwrap();

        let vocabulary = Vocabulary::new(["配置".to_owned()]);
        assert!(count_corpus(&vocabulary, &linked).is_err());
    }

    fn vocabulary() -> Vocabulary {
        Vocabulary::new(
            [
                "配置",
                "权限",
                "与",
                "于",
                "配置与",
                "abc",
                "很长很长很长很长很长",
            ]
            .map(str::to_owned),
        )
    }

    #[test]
    fn the_vocabulary_keeps_short_han_words() {
        let vocabulary = vocabulary();
        assert_eq!(vocabulary.len(), 5);
        assert_eq!(vocabulary.longest, 3);
    }

    #[test]
    fn segmentation_is_greedy_longest_match() {
        let vocabulary = vocabulary();
        let mut words = Vec::new();
        vocabulary.segment("配置与权限甲于", &mut words);
        let spelled: Vec<_> = words
            .iter()
            .map(|word| word.map(|id| vocabulary.words[id as usize].as_str()))
            .collect();
        assert_eq!(spelled, [Some("配置与"), Some("权限"), None, Some("于")]);
    }

    #[test]
    fn runs_come_from_article_text_only() {
        let dump = "<page><title>标题不算</title><text bytes=\"1\" xml:space=\"preserve\">中文维基{{模板内容}}百科[[链接|显示文字]]单</text></page><text>第二篇</text>";
        let mut runs = Vec::new();
        for_each_han_run(dump.as_bytes(), usize::MAX, |run| runs.push(run.to_owned())).unwrap();
        assert_eq!(runs, ["中文维基", "百科", "显示文字", "第二篇"]);

        let mut limited = Vec::new();
        for_each_han_run(dump.as_bytes(), 5, |run| limited.push(run.to_owned())).unwrap();
        assert_eq!(limited, ["中文维基", "百科"]);
    }

    #[test]
    fn invalid_utf8_is_dropped_the_way_python_ignores_it() {
        let mut bytes = b"<text>".to_vec();
        bytes.extend_from_slice("中文".as_bytes());
        bytes.push(0xff);
        bytes.extend_from_slice("维基".as_bytes());
        bytes.extend_from_slice(&"百".as_bytes()[..2]);
        bytes.extend_from_slice(b"</text>");
        let mut runs = Vec::new();
        for_each_han_run(bytes.as_slice(), usize::MAX, |run| {
            runs.push(run.to_owned())
        })
        .unwrap();
        assert_eq!(runs, ["中文维基"]);
    }

    #[test]
    fn chunks_are_counted_in_characters() {
        let text = "中".repeat(CHUNK_CHARS + 3);
        let mut chunks = CharChunks::new(text.as_bytes());
        assert_eq!(
            chunks.next_chunk().unwrap().unwrap().chars().count(),
            CHUNK_CHARS
        );
        assert_eq!(chunks.next_chunk().unwrap().unwrap(), "中中中");
        assert!(chunks.next_chunk().unwrap().is_none());
        assert_eq!(last_chars("abcdef", 2), "ef");
        assert_eq!(last_chars("ab", 5), "ab");
    }

    #[test]
    fn fnv_matches_the_reference_packer() {
        // fnv1a64(b"") and fnv1a64("a\0b".encode()) from the Python builder.
        assert_eq!(fnv1a64(&[""]), 0xcbf2_9ce4_8422_2325);
        let mut expected: u64 = 0xcbf2_9ce4_8422_2325;
        for byte in b"a\0b" {
            expected = (expected ^ u64::from(*byte)).wrapping_mul(0x100_0000_01b3);
        }
        assert_eq!(fnv1a64(&["a", "b"]), expected);
    }

    #[test]
    fn tables_hold_clamped_shrunk_log_ratios() {
        let vocabulary = vocabulary();
        let mut counts = Counts::new(&vocabulary);
        let mut words = Vec::new();
        for _ in 0..10 {
            counts.count_run(&vocabulary, "配置与权限", &mut words);
        }
        counts.count_run(&vocabulary, "于权限", &mut words);
        let bigram = pack(&vocabulary, &counts, 2).unwrap();
        assert_eq!(&bigram[..4], MAGIC);
        let entries = u32::from_le_bytes(bigram[8..12].try_into().unwrap()) as usize;
        // (START, 配置与) and (配置与, 权限) reach the minimum count; (START, 于) and (于, 权限) do not.
        assert_eq!(entries, 2);
        assert_eq!(bigram.len(), 16 + entries * 12);
        let keys: Vec<u64> = (0..entries)
            .map(|i| u64::from_le_bytes(bigram[16 + i * 8..24 + i * 8].try_into().unwrap()))
            .collect();
        assert!(keys.windows(2).all(|pair| pair[0] <= pair[1]));
        let key = fnv1a64(&["配置与", "权限"]);
        let index = keys.iter().position(|candidate| *candidate == key).unwrap();
        let value = f32::from_le_bytes(
            bigram[16 + entries * 8 + index * 4..][..4]
                .try_into()
                .unwrap(),
        );
        // P(权限 | 配置与) = 1, P(权限) = 11/22, shrunk by 10/15.
        let expected = ((1.0f64 / (11.0 / 22.0)).ln() * (10.0 / 15.0)) as f32;
        assert_eq!(value, expected);

        let trigram = pack(&vocabulary, &counts, 3).unwrap();
        // Only (START, 配置与, 权限): a triple after two start tokens has no counted context.
        assert_eq!(u32::from_le_bytes(trigram[8..12].try_into().unwrap()), 1);
    }
}
