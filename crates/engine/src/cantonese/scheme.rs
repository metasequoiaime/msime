//! The Jyutping composition: typed letters and `'` boundaries, read as syllables against the inventory, and the candidates `msime-cantonese.db` has for them. A candidate may cover only the leading syllables; selecting it takes those letters out of the composition and leaves the rest composing, with nothing held back as phrase progress.

use std::collections::HashSet;
use std::sync::Arc;

use super::shuangpin;
use super::syllable::{self, Inventory, Segmentation};
use crate::error::Result;
use crate::language_dictionary::{LanguageDictionary, LanguageEntry};
use crate::types::{QueryRequest, SchemeKey, SchemeType};

/// Entries read for one span of complete syllables. It covers every row of the heaviest key in `msime-cantonese.db` (`zi` has 386 in dict-v2.0.48), so a character at the end of a key can still be paged to; at 200 the 839 rows past it on ten keys were never offered (#10448). `msime-dict-build languages` fails a build in which some key has more rows than this.
pub const SPAN_LIMIT: usize = 512;
/// Entries read for a reading whose last syllable is still a prefix.
pub const COMPLETION_LIMIT: usize = 50;
const SMALL_CANDIDATE_DEDUP: usize = 64;

/// One candidate row, with what selecting it consumes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CantoneseCandidate {
    pub text: String,
    pub weight: i64,
    /// The `entries.key` the row is stored under; for a completed prefix it spells out the last syllable in full.
    pub key: String,
    /// How many syllables of the reading it covers, counted from the first.
    pub syllables: usize,
    /// The byte length of the typed input it covers, apostrophes inside it included.
    pub end: usize,
}

#[derive(Debug, Clone)]
pub struct CantoneseScheme {
    /// Lowercase letters and single `'` boundaries, never starting with one.
    input: String,
    inventory: Arc<Inventory>,
    /// 首个完整双拼码被识别后保持该输入段按粤语双拼解释；普通粤拼仍优先保留。
    shuangpin: bool,
    /// 独立双拼入口从第一键起按双拼解释，即使两键也是合法粤拼音节。
    force_shuangpin: bool,
}

/// 双拼码展开后的粤拼读法。
#[derive(Debug, Clone, PartialEq, Eq)]
struct ShuangpinExpansion {
    /// 各完整双拼码的读音组合，音节间以 `'` 分隔；输入只有一个末尾单键时是一个空串。
    readings: Vec<String>,
    /// 末尾单独按下的双拼键，以及它在粤拼里可能的开头拼写（`shuangpin::initial_prefixes`）。
    trailing: Option<(u8, Vec<String>)>,
}

/// 会话查询复用的字典行和跨度键；不属于方案的组字状态。
#[derive(Default)]
pub(crate) struct CantoneseQueryBuffer {
    entries: Vec<LanguageEntry>,
    completions: Vec<(String, LanguageEntry)>,
    key: String,
}

impl CantoneseScheme {
    pub fn new(inventory: Arc<Inventory>) -> Self {
        Self {
            input: String::new(),
            inventory,
            shuangpin: false,
            force_shuangpin: false,
        }
    }

    /// 切换入口时设置解释方式；调用方在空组字上使用。
    pub fn set_force_shuangpin(&mut self, enabled: bool) {
        self.force_shuangpin = enabled;
        self.reset();
    }

    pub fn reset(&mut self) {
        self.input.clear();
        self.shuangpin = self.force_shuangpin;
    }

    /// Lowercase letters append and Backspace removes the last key. `'` appends only between letters: a boundary before the first syllable or right after another boundary marks nothing. Uppercase letters are not part of Jyutping and are ignored, like every other key.
    pub fn handle_key(&mut self, key: SchemeKey) -> bool {
        match key {
            SchemeKey::Letter(letter) if letter.is_ascii_lowercase() => {
                self.input.push(char::from(letter));
                self.maybe_enable_shuangpin();
                true
            }
            SchemeKey::Apostrophe => {
                if !self.input.is_empty() && !self.input.ends_with('\'') {
                    self.input.push('\'');
                    true
                } else {
                    false
                }
            }
            SchemeKey::Semicolon if self.shuangpin && self.accepts_semicolon() => {
                self.input.push(';');
                true
            }
            SchemeKey::Semicolon if self.input.len() == 1 => {
                let code = [self.input.as_bytes()[0], b';'];
                if !shuangpin::decode(&self.inventory, code).is_empty() {
                    self.input.push(';');
                    self.shuangpin = true;
                    true
                } else {
                    false
                }
            }
            SchemeKey::Semicolon => false,
            SchemeKey::Backspace => {
                let removed = self.input.pop().is_some();
                if self.input.len() < 2 {
                    self.shuangpin = self.force_shuangpin;
                }
                removed
            }
            SchemeKey::Letter(_) | SchemeKey::Minus | SchemeKey::Symbol(_) | SchemeKey::Requery => {
                false
            }
        }
    }

    fn maybe_enable_shuangpin(&mut self) {
        if self.shuangpin || self.input.len() != 2 || self.input.contains('\'') {
            return;
        }
        let code = [self.input.as_bytes()[0], self.input.as_bytes()[1]];
        if !self.inventory.contains(&self.input)
            && !shuangpin::decode(&self.inventory, code).is_empty()
        {
            self.shuangpin = true;
        }
    }

    pub fn is_shuangpin(&self) -> bool {
        self.shuangpin
    }

    /// 当前组字是否应把 `;` 交给粤拼双拼作为第二键。
    pub fn accepts_semicolon(&self) -> bool {
        if self.shuangpin {
            return self
                .input
                .rsplit('\'')
                .next()
                .is_some_and(|chunk| !chunk.is_empty() && chunk.len() % 2 == 1);
        }
        self.input.len() == 1
            && !shuangpin::decode(&self.inventory, [self.input.as_bytes()[0], b';']).is_empty()
    }

    /// Replaces the composition with a host edit: letters are lowercased, `'` and the spaces `editing_text` shows at syllable boundaries are both boundaries, anything else is dropped, and boundaries are normalized as typing would leave them. Reading a space as a boundary keeps the syllables the user saw when an edit changes the letters around them (`ngo oi` edited to `ngo i` stays two syllables rather than becoming `ngoi`).
    pub fn set_raw_input(&mut self, raw: &str) {
        self.input.clear();
        self.shuangpin = self.force_shuangpin;
        self.input.reserve(raw.len());
        for character in raw.chars() {
            if character.is_ascii_alphabetic() {
                self.input.push(character.to_ascii_lowercase());
                self.maybe_enable_shuangpin();
            } else if character == ';' {
                self.handle_key(SchemeKey::Semicolon);
            } else if character == '\'' || character == ' ' {
                self.handle_key(SchemeKey::Apostrophe);
            }
        }
    }

    /// The typed letters and boundaries.
    pub fn input(&self) -> &str {
        &self.input
    }

    pub fn is_empty(&self) -> bool {
        self.input.is_empty()
    }

    /// The reading the composition shows and is looked up by (`syllable::best`).
    pub fn segmentation(&self) -> Segmentation {
        syllable::best(&self.input, &self.inventory)
    }

    /// The typed letters with a space at each syllable boundary (`nei hou`). Letters no syllable reads follow after a space as typed, and a trailing `'` stays visible so the key shows an effect.
    pub fn editing_text(&self) -> String {
        if self.shuangpin {
            let mut text = String::with_capacity(self.input.len() + self.input.len() / 2);
            for (chunk_index, chunk) in self.input.split('\'').enumerate() {
                if chunk_index > 0 {
                    text.push('\'');
                    if !chunk.is_empty() {
                        text.push(' ');
                    }
                }
                for (index, character) in chunk.chars().enumerate() {
                    if index > 0 && index % 2 == 0 {
                        text.push(' ');
                    }
                    text.push(character);
                }
            }
            return text;
        }
        let reading = self.segmentation();
        let mut text = String::new();
        self.editing_text_into(&reading, &mut text);
        text
    }

    fn editing_text_into(&self, reading: &Segmentation, text: &mut String) {
        if self.shuangpin {
            text.clear();
            for (chunk_index, chunk) in self.input.split('\'').enumerate() {
                if chunk_index > 0 {
                    text.push('\'');
                    if !chunk.is_empty() {
                        text.push(' ');
                    }
                }
                for (index, character) in chunk.chars().enumerate() {
                    if index > 0 && index % 2 == 0 {
                        text.push(' ');
                    }
                    text.push(character);
                }
            }
            return;
        }
        let prefix_capacity = reading
            .syllables
            .iter()
            .map(|syllable| syllable.end - syllable.start)
            .sum::<usize>()
            .saturating_add(reading.syllables.len().saturating_sub(1));
        let rest = self.input[reading.end()..].trim_start_matches('\'');
        let suffix_capacity = if rest.is_empty() {
            usize::from(self.input.ends_with('\''))
        } else {
            rest.len().saturating_add(usize::from(prefix_capacity > 0))
        };
        text.clear();
        let capacity = prefix_capacity.saturating_add(suffix_capacity);
        if text.capacity() == 0 {
            text.reserve_exact(capacity);
        } else {
            text.reserve(capacity);
        }
        for (index, syllable) in reading.syllables.iter().enumerate() {
            if index > 0 {
                text.push(' ');
            }
            text.push_str(&self.input[syllable.start..syllable.end]);
        }
        if !rest.is_empty() {
            if !text.is_empty() {
                text.push(' ');
            }
            text.push_str(rest);
        } else if self.input.ends_with('\'') {
            text.push('\'');
        }
    }

    /// The request the session refreshes with. `raw_input` is the typed letters and boundaries, which the provider reads again through the same inventory; `segmentation` is the dictionary key of the reading and `normalized_segmentation` the text the composition shows.
    pub fn build_request(&self) -> QueryRequest {
        let mut request = QueryRequest::default();
        self.build_request_into(&mut request);
        request
    }

    pub fn build_request_into(&self, request: &mut QueryRequest) {
        let reading = self.segmentation();
        request.scheme = SchemeType::Cantonese;
        request.raw_input.clone_from(&self.input);
        request.raw_input_with_cases.clone_from(&self.input);
        request.normalized_input.clear();
        request.normalized_input.reserve(self.input.len());
        request
            .normalized_input
            .extend(self.input.chars().filter(|&character| character != '\''));
        request.raw_segmentation.clone_from(&self.input);
        self.editing_text_into(&reading, &mut request.normalized_segmentation);
        reading.key_into(
            &self.input,
            reading.syllables.len(),
            &mut request.segmentation,
        );
        request.valid = !self.input.is_empty();
    }

    /// The letters and boundaries as typed, which is what Enter commits; the spaced form the composition shows is `editing_text`.
    pub fn preedit(&self) -> String {
        self.input.clone()
    }

    /// The candidates for the composition, each text listed once per span length:
    /// 1. When the reading covers every letter, the entries of the whole reading, or when its last syllable is still a prefix, the entries that complete it. A last syllable that is complete but also starts a longer one (`ho` of `hou`) lists its exact entries and then the entries that complete it, so a word does not drop out of the list while its last syllable is half typed.
    /// 2. Then each leading span of complete syllables, longest first, each span's entries heaviest first.
    pub fn candidates(&self, dictionary: &LanguageDictionary) -> Result<Vec<CantoneseCandidate>> {
        let mut candidates = Vec::new();
        self.candidates_into(
            dictionary,
            &mut CantoneseQueryBuffer::default(),
            &mut candidates,
        )?;
        candidates.shrink_to_fit();
        Ok(candidates)
    }

    /// 将候选写入已有缓冲，并复用字典行和候选行中的字符串容量。
    pub(crate) fn candidates_into(
        &self,
        dictionary: &LanguageDictionary,
        buffer: &mut CantoneseQueryBuffer,
        candidates: &mut Vec<CantoneseCandidate>,
    ) -> Result<()> {
        if let Some(expansion) = self.shuangpin_expansions() {
            candidates.clear();
            let mut expanded = Vec::new();
            let mut temporary = self.clone();
            temporary.shuangpin = false;
            let mut rows = Vec::new();
            // 末尾单键的各个前缀先收窄成只落在本键上的段，各读音共用。
            let trailing = expansion.trailing.as_ref().map(|(key, prefixes)| {
                let groups = prefixes
                    .iter()
                    .map(|prefix| shuangpin::key_spans(&self.inventory, prefix, *key))
                    .collect::<Vec<_>>();
                (*key, groups)
            });
            for reading in &expansion.readings {
                if let Some((key, groups)) = &trailing {
                    self.trailing_initial_candidates(
                        dictionary,
                        buffer,
                        reading,
                        *key,
                        groups,
                        &mut expanded,
                    )?;
                }
                if reading.is_empty() {
                    continue;
                }
                temporary.input.clone_from(reading);
                // 每两键已是一个完整音节，末音节不再补全成更长的音节（`da` 只读 `daa`，不补出 `daap`）。
                temporary.candidates_into_jyutping(dictionary, buffer, &mut rows, false)?;
                for mut row in rows.drain(..) {
                    row.end = self.shuangpin_code_end(row.syllables);
                    expanded.push(row);
                }
            }
            // 同一双拼码的各个读音合在一起排：先比覆盖的音节数，再比权重，排序稳定，同权重保持读音的字节序。
            expanded.sort_by(|left, right| {
                right
                    .syllables
                    .cmp(&left.syllables)
                    .then(right.weight.cmp(&left.weight))
            });
            deduplicate_candidates(&mut expanded);
            candidates.extend(expanded);
            return Ok(());
        }
        self.candidates_into_jyutping(dictionary, buffer, candidates, true)
    }

    /// 末尾只按了一个双拼键：前面的完整音节照常定死，最后一个音节只知道声母。`groups` 的每一组来自一个 `initial_prefixes` 前缀，组内各段由 `shuangpin::key_spans` 收窄过，段下没有别的键的音节，别的键上的重行不会占满查询上限。每组合起来最多留最重的 `SPAN_LIMIT` 行。
    fn trailing_initial_candidates(
        &self,
        dictionary: &LanguageDictionary,
        buffer: &mut CantoneseQueryBuffer,
        reading: &str,
        key: u8,
        groups: &[Vec<(String, bool)>],
        expanded: &mut Vec<CantoneseCandidate>,
    ) -> Result<()> {
        let syllables = reading.split('\'').filter(|part| !part.is_empty()).count() + 1;
        let end = self.shuangpin_code_end(syllables);
        buffer.key.clear();
        for part in reading.split('\'').filter(|part| !part.is_empty()) {
            buffer.key.push_str(part);
            buffer.key.push(' ');
        }
        let base = buffer.key.len();
        for spans in groups {
            let start = expanded.len();
            for (span, whole) in spans {
                buffer.key.truncate(base);
                buffer.key.push_str(span);
                if *whole {
                    dictionary.lookup_into(&buffer.key, SPAN_LIMIT, &mut buffer.entries)?;
                    for entry in &buffer.entries {
                        expanded.push(CantoneseCandidate {
                            text: entry.text.clone(),
                            weight: entry.weight,
                            key: buffer.key.clone(),
                            syllables,
                            end,
                        });
                    }
                    continue;
                }
                dictionary.lookup_completions_into(
                    &buffer.key,
                    SPAN_LIMIT,
                    &mut buffer.completions,
                )?;
                for (entry_key, entry) in &buffer.completions {
                    // 段下没有别的键的音节；这里筛掉 `encode` 不认识的口语音（`deu`、`gep` 一类）。
                    let last = entry_key.rsplit(' ').next().unwrap_or_default();
                    if shuangpin::encode(last).is_some_and(|code| code[0] == key) {
                        expanded.push(CantoneseCandidate {
                            text: entry.text.clone(),
                            weight: entry.weight,
                            key: entry_key.clone(),
                            syllables,
                            end,
                        });
                    }
                }
            }
            if spans.len() > 1 {
                // 与单段区间查询的顺序一致：权重从重到轻，同权重按文字。
                expanded[start..].sort_by(|left, right| {
                    right
                        .weight
                        .cmp(&left.weight)
                        .then_with(|| left.text.cmp(&right.text))
                });
                expanded.truncate(start + SPAN_LIMIT);
            }
        }
        Ok(())
    }

    fn shuangpin_code_end(&self, syllables: usize) -> usize {
        if syllables == 0 {
            return 0;
        }
        let mut count = 0;
        let mut position = 0;
        while position < self.input.len() {
            if self.input.as_bytes()[position] == b'\'' {
                position += 1;
                continue;
            }
            // 末尾单独一个声母键也算一个音节，但它只占一个字节。
            position = (position + 2).min(self.input.len());
            count += 1;
            if count == syllables {
                return position;
            }
        }
        self.input.len()
    }

    /// 把双拼码展开成粤拼读法。每个音节之间都写上 `'`，让临时粤拼按双拼的两键一音节切分，不再交给最长匹配重切（`danz` 展开的 `daanei` 会被切成 `daan ei`）。
    ///
    /// 独立双拼入口下最后一段以单个键结尾时，那个键只定下声母，放在 `trailing` 里交给补全；中间段不允许奇数长度。任一码在库存里解不出音节、或末尾单键没有对应音节时返回 `None`，由调用方按普通粤拼读。
    fn shuangpin_expansions(&self) -> Option<ShuangpinExpansion> {
        if !self.shuangpin || self.input.is_empty() {
            return None;
        }
        let chunks = self.input.split('\'').collect::<Vec<_>>();
        let last = chunks.len() - 1;
        // 输入不以 `'` 开头、也不含连续的 `'`，空段只可能是末尾刚敲下的分隔符。
        if chunks[..last]
            .iter()
            .any(|chunk| chunk.is_empty() || chunk.len() % 2 != 0)
        {
            return None;
        }
        let trailing = if chunks[last].len() % 2 != 0 {
            // 自动识别时第三键也可能是粤拼在继续拼同一个音节（`gw` 之后的 `gwo`），奇数长度仍交给普通粤拼；只有独立双拼入口把它当下一个音节的声母。
            if !self.force_shuangpin {
                return None;
            }
            let key = chunks[last].as_bytes()[chunks[last].len() - 1];
            let prefixes = shuangpin::initial_prefixes(&self.inventory, key);
            if prefixes.is_empty() {
                return None;
            }
            Some((key, prefixes))
        } else {
            None
        };
        let mut values = vec![String::new()];
        for chunk in &chunks {
            for offset in (0..chunk.len() / 2 * 2).step_by(2) {
                let code = [chunk.as_bytes()[offset], chunk.as_bytes()[offset + 1]];
                let syllables = shuangpin::decode(&self.inventory, code);
                if syllables.is_empty() {
                    return None;
                }
                let mut combined = Vec::new();
                for prefix in &values {
                    for syllable in &syllables {
                        if combined.len() >= 64 {
                            break;
                        }
                        let mut value = prefix.clone();
                        if !value.is_empty() {
                            value.push('\'');
                        }
                        value.push_str(syllable);
                        combined.push(value);
                    }
                }
                values = combined;
            }
        }
        Some(ShuangpinExpansion {
            readings: values,
            trailing,
        })
    }

    /// `complete_trailing` 为假时，整段读法的末音节即使还是更长音节的开头也不再补全；双拼展开出的音节都是按键定死的。
    fn candidates_into_jyutping(
        &self,
        dictionary: &LanguageDictionary,
        buffer: &mut CantoneseQueryBuffer,
        candidates: &mut Vec<CantoneseCandidate>,
        complete_trailing: bool,
    ) -> Result<()> {
        let input = self.input.as_str();
        let reading = self.segmentation();
        let count = reading.syllables.len();
        let full = !reading.syllables.is_empty()
            && input[reading.end()..].bytes().all(|byte| byte == b'\'');
        let mut length = 0;
        let mut spans = count;
        if full {
            write_key(&reading, input, count, &mut buffer.key);
            if reading.ends_in_prefix() {
                dictionary.lookup_completions_into(
                    &buffer.key,
                    COMPLETION_LIMIT,
                    &mut buffer.completions,
                )?;
                reserve_candidate_batch(candidates, length, buffer.completions.len());
                for (key, entry) in &buffer.completions {
                    push_candidate(
                        candidates,
                        &mut length,
                        key,
                        &entry.text,
                        entry.weight,
                        count,
                        &reading,
                    );
                }
            } else {
                dictionary.lookup_into(&buffer.key, SPAN_LIMIT, &mut buffer.entries)?;
                reserve_candidate_batch(candidates, length, buffer.entries.len());
                for entry in &buffer.entries {
                    push_candidate(
                        candidates,
                        &mut length,
                        &buffer.key,
                        &entry.text,
                        entry.weight,
                        count,
                        &reading,
                    );
                }
                let last = reading.texts(input).last().unwrap_or_default();
                if complete_trailing && self.inventory.is_prefix(last) {
                    dictionary.lookup_completions_into(
                        &buffer.key,
                        COMPLETION_LIMIT,
                        &mut buffer.completions,
                    )?;
                    reserve_candidate_batch(candidates, length, buffer.completions.len());
                    for (key, entry) in &buffer.completions {
                        push_candidate(
                            candidates,
                            &mut length,
                            key,
                            &entry.text,
                            entry.weight,
                            count,
                            &reading,
                        );
                    }
                }
            }
            spans = count - 1;
        }
        for span_length in (1..=spans).rev() {
            write_key(&reading, input, span_length, &mut buffer.key);
            dictionary.lookup_into(&buffer.key, SPAN_LIMIT, &mut buffer.entries)?;
            reserve_candidate_batch(candidates, length, buffer.entries.len());
            for entry in &buffer.entries {
                push_candidate(
                    candidates,
                    &mut length,
                    &buffer.key,
                    &entry.text,
                    entry.weight,
                    span_length,
                    &reading,
                );
            }
        }
        candidates.truncate(length);
        deduplicate_candidates(candidates);
        Ok(())
    }

    /// Takes the letters `candidate` covers out of the composition, with the boundary after them. Returns whether letters are left composing; the caller commits the candidate's text either way.
    pub fn select(&mut self, candidate: &CantoneseCandidate) -> bool {
        self.select_end(candidate.end)
    }

    /// 按候选覆盖的字节数删掉组字前缀，避免提交路径复制整行候选。
    pub fn select_end(&mut self, end: usize) -> bool {
        self.input.drain(..end);
        let boundaries = self.input.len() - self.input.trim_start_matches('\'').len();
        self.input.drain(..boundaries);
        !self.input.is_empty()
    }
}

fn reserve_candidate_batch(candidates: &mut Vec<CantoneseCandidate>, length: usize, batch: usize) {
    let required = length + batch;
    if required > candidates.capacity() {
        candidates.reserve_exact(required - candidates.len());
    }
}

fn push_candidate(
    candidates: &mut Vec<CantoneseCandidate>,
    length: &mut usize,
    key: &str,
    text: &str,
    weight: i64,
    syllables: usize,
    reading: &Segmentation,
) {
    let end = reading.syllables[syllables - 1].end;
    if let Some(candidate) = candidates.get_mut(*length) {
        candidate.text.clear();
        candidate.text.push_str(text);
        candidate.key.clear();
        candidate.key.push_str(key);
        candidate.weight = weight;
        candidate.syllables = syllables;
        candidate.end = end;
    } else {
        candidates.push(CantoneseCandidate {
            text: text.to_owned(),
            weight,
            key: key.to_owned(),
            syllables,
            end,
        });
    }
    *length += 1;
}

fn write_key(reading: &Segmentation, input: &str, count: usize, destination: &mut String) {
    destination.clear();
    for (index, syllable) in reading.syllables.iter().take(count).enumerate() {
        if index > 0 {
            destination.push(' ');
        }
        destination.push_str(&input[syllable.start..syllable.end]);
    }
}

fn deduplicate_candidates(candidates: &mut Vec<CantoneseCandidate>) {
    if candidates.len() <= SMALL_CANDIDATE_DEDUP {
        let mut write = 0;
        for read in 0..candidates.len() {
            if candidates[..write].iter().any(|existing| {
                existing.text == candidates[read].text
                    && existing.syllables == candidates[read].syllables
            }) {
                continue;
            }
            if write != read {
                candidates.swap(write, read);
            }
            write += 1;
        }
        candidates.truncate(write);
        return;
    }
    let mut seen = HashSet::with_capacity(candidates.len());
    let duplicates = candidates
        .iter()
        .enumerate()
        .filter_map(|(index, candidate)| {
            (!seen.insert((candidate.text.as_str(), candidate.syllables))).then_some(index)
        })
        .collect::<Vec<_>>();
    drop(seen);
    let mut duplicates = duplicates.into_iter().peekable();
    let mut write = 0;
    for read in 0..candidates.len() {
        if duplicates.peek() == Some(&read) {
            duplicates.next();
            continue;
        }
        if write != read {
            candidates.swap(write, read);
        }
        write += 1;
    }
    candidates.truncate(write);
}

#[cfg(test)]
mod tests {
    use std::path::Path;

    use rusqlite::Connection;

    use super::*;
    use crate::cantonese::syllable::tests::{inventory, SYLLABLES};
    use crate::language_dictionary::{
        open_read_only, FORMAT_VERSION, METADATA_FORMAT_VERSION, METADATA_LICENSE,
        METADATA_SOURCE_COMMIT, SCHEMA,
    };

    const ENTRIES: [(&str, &str, i64); 17] = [
        ("nei hou", "你好", 900),
        ("nei hou", "妳好", 40),
        ("nei hoeng", "你向", 3),
        ("nei", "你", 5000),
        ("nei", "妳", 300),
        ("hou", "好", 4000),
        ("hou", "號", 500),
        ("gwong dung waa", "廣東話", 800),
        ("gwong dung", "廣東", 1200),
        ("gwong", "光", 2000),
        ("gwong", "廣", 900),
        ("dung", "東", 1500),
        ("waa", "話", 3000),
        ("ngo", "我", 6000),
        ("oi", "愛", 2500),
        ("ngoi", "外", 1000),
        // The same text under a different span is listed again, so 光 stays selectable for `gwong` alone.
        ("gwong dung waa", "光", 1),
    ];

    fn build(path: &Path) {
        let connection = Connection::open(path).unwrap();
        connection.execute_batch(SCHEMA).unwrap();
        for (name, value) in [
            (METADATA_FORMAT_VERSION, FORMAT_VERSION.to_string()),
            (
                METADATA_SOURCE_COMMIT,
                "259f0e48bba840c3a2e0d117539e96937f3d89bc".to_owned(),
            ),
            (METADATA_LICENSE, "CC-BY-4.0".to_owned()),
        ] {
            connection
                .execute("INSERT INTO metadata VALUES (?1, ?2)", (name, value))
                .unwrap();
        }
        for syllable in SYLLABLES {
            connection
                .execute("INSERT INTO syllables VALUES (?1)", (syllable,))
                .unwrap();
        }
        for (key, text, weight) in ENTRIES {
            connection
                .execute(
                    "INSERT INTO entries VALUES (?1, ?2, ?3)",
                    (key, text, weight),
                )
                .unwrap();
        }
    }

    struct Fixture {
        _dir: tempfile::TempDir,
        dictionary: LanguageDictionary,
    }

    fn fixture() -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("msime-cantonese.db");
        build(&path);
        Fixture {
            dictionary: open_read_only(&path).unwrap(),
            _dir: dir,
        }
    }

    fn typed(keys: &str) -> CantoneseScheme {
        let mut scheme = CantoneseScheme::new(Arc::new(inventory()));
        for byte in keys.bytes() {
            scheme.handle_key(if byte == b'\'' {
                SchemeKey::Apostrophe
            } else {
                SchemeKey::Letter(byte)
            });
        }
        scheme
    }

    /// Each candidate as `text/syllables`.
    fn listed(scheme: &CantoneseScheme, dictionary: &LanguageDictionary) -> Vec<String> {
        scheme
            .candidates(dictionary)
            .unwrap()
            .iter()
            .map(|candidate| format!("{}/{}", candidate.text, candidate.syllables))
            .collect()
    }

    fn candidate(text: &str, syllables: usize) -> CantoneseCandidate {
        CantoneseCandidate {
            text: text.to_owned(),
            weight: 1,
            key: "nei".to_owned(),
            syllables,
            end: 3,
        }
    }

    #[test]
    fn editing_text_growing_buffer_allocates_once() {
        for suffix in ["", "zz", "'"] {
            let input = format!("{}{suffix}", "nei".repeat(20));
            let scheme = typed(&input);
            let reading = scheme.segmentation();
            let expected_suffix = if suffix == "zz" { " zz" } else { suffix };
            let expected = format!("{}{expected_suffix}", vec!["nei"; 20].join(" "));
            for capacity in [16, 0, 128] {
                let mut text = String::with_capacity(capacity);
                if capacity > 0 {
                    text.push_str("舊值");
                }
                let old_capacity = text.capacity();
                let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
                    scheme.editing_text_into(&reading, &mut text);
                });
                assert_eq!(text, expected);
                assert_eq!(allocations, usize::from(old_capacity < expected.len()));
                let capacity = text.capacity();
                let empty = typed("");
                let reading = empty.segmentation();
                let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
                    empty.editing_text_into(&reading, &mut text);
                });
                assert!(text.is_empty());
                assert_eq!(allocations, 0);
                assert_eq!(text.capacity(), capacity);
            }
        }
    }

    #[test]
    fn editing_text_suffix_cold_buffer_allocates_once() {
        for suffix in ["zz", "'"] {
            let scheme = typed(&format!("{}{suffix}", "nei".repeat(20)));
            let reading = scheme.segmentation();
            let mut text = String::new();
            let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
                scheme.editing_text_into(&reading, &mut text);
            });
            let suffix = if suffix == "zz" { " zz" } else { suffix };
            assert_eq!(text, format!("{}{suffix}", vec!["nei"; 20].join(" ")));
            assert_eq!(allocations, 1, "尾部与分隔符应纳入首次容量预留");
        }
    }

    #[test]
    fn short_candidate_dedup_keeps_first_rows_without_temporary_heap_state() {
        let mut candidates = vec![
            candidate("你", 1),
            candidate("妳", 1),
            candidate("你", 1),
            candidate("你", 2),
        ];
        let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
            deduplicate_candidates(&mut candidates);
        });
        assert_eq!(allocations, 0);
        assert_eq!(
            candidates
                .iter()
                .map(|candidate| (candidate.text.as_str(), candidate.syllables))
                .collect::<Vec<_>>(),
            [("你", 1), ("妳", 1), ("你", 2)]
        );

        let mut large = (0..=SMALL_CANDIDATE_DEDUP)
            .map(|index| candidate(&format!("字{index}"), 1))
            .collect::<Vec<_>>();
        large.push(candidate("字0", 1));
        deduplicate_candidates(&mut large);
        assert_eq!(large.len(), SMALL_CANDIDATE_DEDUP + 1);
        assert_eq!(large[0].text, "字0");
    }

    #[test]
    fn the_whole_reading_comes_first_then_leading_spans() {
        let fixture = fixture();
        let scheme = typed("neihou");
        assert_eq!(scheme.editing_text(), "nei hou");
        assert_eq!(
            listed(&scheme, &fixture.dictionary),
            ["你好/2", "妳好/2", "你/1", "妳/1"]
        );
        let scheme = typed("gwongdungwaa");
        assert_eq!(scheme.editing_text(), "gwong dung waa");
        assert_eq!(
            listed(&scheme, &fixture.dictionary),
            ["廣東話/3", "光/3", "廣東/2", "光/1", "廣/1"]
        );
    }

    #[test]
    fn two_key_codes_expand_against_the_existing_cantonese_inventory() {
        let fixture = fixture();
        let scheme = typed("nzhb");
        assert!(scheme.is_shuangpin());
        assert_eq!(scheme.input(), "nzhb");
        assert_eq!(scheme.editing_text(), "nz hb");
        let candidates = scheme.candidates(&fixture.dictionary).unwrap();
        assert_eq!(
            candidates
                .iter()
                .map(|candidate| (candidate.text.as_str(), candidate.end))
                .collect::<Vec<_>>(),
            [("你好", 4), ("妳好", 4), ("你", 2), ("妳", 2)]
        );
    }

    #[test]
    fn explicit_shuangpin_mode_decodes_a_code_that_is_also_jyutping() {
        let inventory = Arc::new(inventory());
        let mut plain = CantoneseScheme::new(inventory.clone());
        plain.set_raw_input("oi");
        assert!(!plain.is_shuangpin());
        assert_eq!(plain.shuangpin_expansions(), None);

        let mut selected = CantoneseScheme::new(inventory);
        selected.set_force_shuangpin(true);
        selected.set_raw_input("oi");
        assert!(selected.is_shuangpin());
        assert_eq!(
            selected.shuangpin_expansions(),
            Some(ShuangpinExpansion {
                readings: vec!["i".to_owned()],
                trailing: None,
            })
        );
        selected.reset();
        selected.set_raw_input("oi");
        assert!(selected.is_shuangpin());
    }

    /// 双拼回归用的小词库：读音和权重取自 #10085 报告里的输入，`ngo daap`、`daan` 这类行专门用来暴露旧的补全和重切问题。音节库存与发布的词库一样覆盖全部词条用到的音节（`maa` 只出现在额外写入的行里）。
    const SHUANGPIN_SYLLABLES: [&str; 19] = [
        "ngo", "ngoi", "ng", "daa", "daai", "daap", "daan", "dei", "ei", "nei", "soeng", "soek",
        "cyun", "cyut", "cuk", "m", "maa", "aa", "i",
    ];
    const SHUANGPIN_ENTRIES: [(&str, &str, i64); 22] = [
        ("ngo", "我", 6000),
        ("ngoi", "外", 1000),
        ("ngo daap", "我答", 7385),
        ("ngo daai", "我帶", 1690),
        ("ngo dei", "我哋", 8000),
        ("ngo nei", "我呢", 300),
        ("ngo ngoi", "我外", 200),
        ("ngo ng", "我五", 100),
        ("daa", "打", 4689),
        ("daap", "答", 900),
        ("daai", "大", 3500),
        ("daan", "蛋", 2000),
        ("ei", "誒", 10),
        ("nei", "你", 5000),
        ("daa nei", "打你", 500),
        ("soek", "削", 200),
        ("soeng", "上", 9000),
        ("soeng", "相", 5000),
        ("soeng cyun", "相傳", 978),
        ("cyun", "傳", 3000),
        ("cuk", "速", 2000),
        ("m", "唔", 7000),
    ];

    fn shuangpin_fixture() -> Fixture {
        shuangpin_fixture_with(&[])
    }

    /// 在双拼小词库之外再写入 `extra` 行。
    fn shuangpin_fixture_with(extra: &[(&str, String, i64)]) -> Fixture {
        let mut entries: Vec<(&str, &str, i64)> = SHUANGPIN_ENTRIES.to_vec();
        entries.extend(
            extra
                .iter()
                .map(|(key, text, weight)| (*key, text.as_str(), *weight)),
        );
        fixture_from(&SHUANGPIN_SYLLABLES, &entries)
    }

    /// 只含 `syllables` 和 `entries` 的词库。
    fn fixture_from(syllables: &[&str], entries: &[(&str, &str, i64)]) -> Fixture {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("msime-cantonese.db");
        let connection = Connection::open(&path).unwrap();
        connection.execute_batch(SCHEMA).unwrap();
        connection
            .execute(
                "INSERT INTO metadata VALUES (?1, ?2)",
                (METADATA_FORMAT_VERSION, FORMAT_VERSION.to_string()),
            )
            .unwrap();
        for syllable in syllables {
            connection
                .execute("INSERT INTO syllables VALUES (?1)", (syllable,))
                .unwrap();
        }
        for (key, text, weight) in entries {
            connection
                .execute(
                    "INSERT INTO entries VALUES (?1, ?2, ?3)",
                    (key, text, weight),
                )
                .unwrap();
        }
        drop(connection);
        Fixture {
            dictionary: open_read_only(&path).unwrap(),
            _dir: dir,
        }
    }

    /// `force` 对应独立「粤语双拼」入口，否则是「粤语」入口的自动识别。
    fn shuangpin_typed(keys: &str, force: bool) -> CantoneseScheme {
        shuangpin_typed_with(&SHUANGPIN_SYLLABLES, keys, force)
    }

    fn shuangpin_typed_with(syllables: &[&str], keys: &str, force: bool) -> CantoneseScheme {
        let mut scheme = CantoneseScheme::new(Arc::new(Inventory::new(syllables.iter().copied())));
        scheme.set_force_shuangpin(force);
        for byte in keys.bytes() {
            scheme.handle_key(if byte == b'\'' {
                SchemeKey::Apostrophe
            } else {
                SchemeKey::Letter(byte)
            });
        }
        scheme
    }

    /// 把每个候选写成 `文字/音节数/末端位置`。
    fn listed_with_end(scheme: &CantoneseScheme, dictionary: &LanguageDictionary) -> Vec<String> {
        scheme
            .candidates(dictionary)
            .unwrap()
            .iter()
            .map(|candidate| {
                format!(
                    "{}/{}/{}",
                    candidate.text, candidate.syllables, candidate.end
                )
            })
            .collect()
    }

    #[test]
    fn shuangpin_does_not_complete_a_syllable_its_two_keys_already_fixed() {
        let fixture = shuangpin_fixture();
        // `da` 只能是 `daa`；`daap` 在 `c` 键上，`ngo daap`、`ngo daai` 不能靠补全混进来。
        for force in [true, false] {
            let scheme = shuangpin_typed("voda", force);
            assert!(scheme.is_shuangpin());
            assert_eq!(
                listed_with_end(&scheme, &fixture.dictionary),
                ["我/1/2", "外/1/2"],
                "force={force}"
            );
        }
    }

    #[test]
    fn shuangpin_expansions_keep_each_code_as_one_syllable() {
        let fixture = shuangpin_fixture();
        // 选「我」之后剩下的 `danz` 是 `daa nei`，不能被最长匹配重切成 `daan ei`。
        let mut scheme = shuangpin_typed("vodanz", true);
        let candidates = scheme.candidates(&fixture.dictionary).unwrap();
        let me = candidates.iter().find(|row| row.text == "我").unwrap();
        assert_eq!(me.end, 2);
        assert!(scheme.select(me));
        assert_eq!(scheme.input(), "danz");
        assert_eq!(
            listed_with_end(&scheme, &fixture.dictionary),
            ["打你/2/4", "打/1/2"]
        );
        // 自动识别入口进入双拼之后同样按两键一音节读。
        let scheme = shuangpin_typed("danz", false);
        assert!(scheme.is_shuangpin());
        assert_eq!(
            listed_with_end(&scheme, &fixture.dictionary),
            ["打你/2/4", "打/1/2"]
        );
        // 末尾刚敲下的分隔符不让整段退回普通粤拼。
        assert_eq!(
            listed_with_end(&shuangpin_typed("nz'", true), &fixture.dictionary),
            ["你/1/2"]
        );
    }

    #[test]
    fn readings_of_one_code_merge_by_coverage_then_weight() {
        let fixture = shuangpin_fixture();
        // `sx` 解出 `soek`、`soeng`，字节序是 `soek` 在前，但「上」「相」更常用。
        assert_eq!(
            listed_with_end(&shuangpin_typed("sx", true), &fixture.dictionary),
            ["上/1/2", "相/1/2", "削/1/2"]
        );
        // `cr` 解出 `cuk`、`cyun`、`cyut`；覆盖两个音节的「相傳」排最前，单字仍按权重。
        let mut scheme = shuangpin_typed("sxcr", false);
        assert_eq!(
            listed_with_end(&scheme, &fixture.dictionary),
            ["相傳/2/4", "上/1/2", "相/1/2", "削/1/2"]
        );
        // 点选「上」只拿走前两键，剩下的 `cr` 继续按双拼出「傳」。
        let up = scheme
            .candidates(&fixture.dictionary)
            .unwrap()
            .into_iter()
            .find(|row| row.text == "上")
            .unwrap();
        assert!(scheme.select(&up));
        assert_eq!(scheme.input(), "cr");
        assert!(scheme.is_shuangpin());
        assert_eq!(
            listed_with_end(&scheme, &fixture.dictionary),
            ["傳/1/2", "速/1/2"]
        );
    }

    #[test]
    fn a_reviewed_three_syllable_word_comes_first_for_its_codes() {
        // #10369：`zk gt lp` 解出 zau/zaau、git/gek、lun/lut/lung/leon。词库收进「周杰倫」整词后，它覆盖全部三个音节，排在只覆盖一个音节的单字前面；权重 711 低于「杰倫」和各个单字也不影响。
        const SYLLABLES: [&str; 8] = ["zau", "zaau", "git", "gek", "leon", "lun", "lut", "lung"];
        let entries = [
            ("zau", "周", 9000),
            ("zaau", "找", 8000),
            ("git", "杰", 3000),
            ("gek", "屐", 100),
            ("leon", "倫", 2000),
            ("lung", "龍", 5000),
            ("git leon", "杰倫", 1182),
            ("zau git leon", "周杰倫", 711),
        ];
        let fixture = fixture_from(&SYLLABLES, &entries);
        for force in [true, false] {
            let scheme = shuangpin_typed_with(&SYLLABLES, "zkgtlp", force);
            assert!(scheme.is_shuangpin(), "force={force}");
            assert_eq!(
                listed_with_end(&scheme, &fixture.dictionary),
                ["周杰倫/3/6", "周/1/2", "找/1/2"],
                "force={force}"
            );
        }
        // 没有整词时要先选「周」，剩下的 `gtlp` 再出「杰倫」，这是 #10369 报告的两次选择。
        let fixture = fixture_from(&SYLLABLES, &entries[..entries.len() - 1]);
        let scheme = shuangpin_typed_with(&SYLLABLES, "zkgtlp", true);
        assert_eq!(
            listed_with_end(&scheme, &fixture.dictionary),
            ["周/1/2", "找/1/2"]
        );
    }

    #[test]
    fn a_trailing_single_key_completes_from_its_initial_in_forced_shuangpin() {
        let fixture = shuangpin_fixture();
        let scheme = shuangpin_typed("vod", true);
        assert_eq!(scheme.editing_text(), "vo d");
        assert_eq!(
            listed_with_end(&scheme, &fixture.dictionary),
            ["我哋/2/3", "我答/2/3", "我帶/2/3", "我/1/2", "外/1/2"]
        );
        // `n` 前缀也会补出 `ngoi`、`ng`，它们在双拼里属于 `v`、`o` 键，必须筛掉。
        assert_eq!(
            listed_with_end(&shuangpin_typed("von", true), &fixture.dictionary),
            ["我呢/2/3", "我/1/2", "外/1/2"]
        );
        // 零声母键 `o` 补出鼻音音节 `ng`，不补出 `ngoi`。
        assert_eq!(
            listed_with_end(&shuangpin_typed("voo", true), &fixture.dictionary),
            ["我五/2/3", "我/1/2", "外/1/2"]
        );
        // 只有一个键时直接按声母列单音节词，`daai`、`daan` 都在 `d` 下。
        assert_eq!(
            listed_with_end(&shuangpin_typed("d", true), &fixture.dictionary),
            ["打/1/1", "大/1/1", "蛋/1/1", "答/1/1"]
        );
        // 选中覆盖末尾单键的候选会清空组字。
        let mut scheme = shuangpin_typed("vod", true);
        let ours = scheme.candidates(&fixture.dictionary).unwrap().remove(0);
        assert!(!scheme.select(&ours));
        assert!(scheme.is_empty());
        // 自动识别入口的奇数长度仍交给普通粤拼，避免把正在拼的粤拼音节当成下一个声母。
        assert_eq!(shuangpin_typed("vod", false).shuangpin_expansions(), None);
    }

    #[test]
    fn a_trailing_single_key_keeps_its_rows_when_other_keys_share_the_spelling() {
        // `n` 键的 `nei` 与 `v` 键的 `ngo` 同以 `n` 开头，零声母 `o` 键的 `m` 与 `m` 键的 `maa` 同以 `m` 开头；别的键上比上限还多的重行不能把本键的行挤掉。
        let mut extra = Vec::new();
        for index in 0..SPAN_LIMIT + 10 {
            let weight = 20_000 + i64::try_from(index).unwrap();
            extra.push(("ngo", format!("我{index}"), weight));
            extra.push(("maa", format!("媽{index}"), weight));
        }
        let fixture = shuangpin_fixture_with(&extra);
        assert_eq!(
            listed_with_end(&shuangpin_typed("n", true), &fixture.dictionary),
            ["你/1/1"]
        );
        let rows = listed_with_end(&shuangpin_typed("o", true), &fixture.dictionary);
        assert!(rows.contains(&"唔/1/1".to_owned()), "{rows:?}");
        assert!(rows.contains(&"誒/1/1".to_owned()), "{rows:?}");
        assert!(
            !rows
                .iter()
                .any(|row| row.starts_with('我') || row.starts_with('媽')),
            "{rows:?}"
        );
    }

    #[test]
    fn every_row_of_a_key_up_to_the_span_limit_is_offered() {
        // 发布库里条目最多的键（zi、ji、jyu）都在上限以内，构建器的 verify 保证这一点；上限内排在最后的一行也要能翻到（#10448 的「逾」曾排在 jyu 的第 201 位而打不出）。
        let rows: Vec<(&str, String, i64)> = (0..=SPAN_LIMIT)
            .map(|index| {
                let weight = 100_000 - i64::try_from(index).unwrap();
                ("jyu", format!("字{index}"), weight)
            })
            .collect();
        let entries: Vec<(&str, &str, i64)> = rows
            .iter()
            .map(|(key, text, weight)| (*key, text.as_str(), *weight))
            .collect();
        let fixture = fixture_from(&["jyu"], &entries);
        let code = shuangpin::encode("jyu").unwrap();
        let code = std::str::from_utf8(&code).unwrap();
        for (keys, force) in [("jyu", false), (code, true)] {
            let texts: Vec<String> = shuangpin_typed_with(&["jyu"], keys, force)
                .candidates(&fixture.dictionary)
                .unwrap()
                .into_iter()
                .map(|candidate| candidate.text)
                .collect();
            assert_eq!(texts.len(), SPAN_LIMIT, "{keys}");
            assert_eq!(texts.last().unwrap(), &format!("字{}", SPAN_LIMIT - 1));
        }
    }

    #[test]
    fn apostrophe_boundaries_reset_pairing_and_candidate_span() {
        let fixture = fixture();
        let scheme = typed("nz'hb");
        assert!(scheme.is_shuangpin());
        assert_eq!(scheme.editing_text(), "nz' hb");
        let candidates = scheme.candidates(&fixture.dictionary).unwrap();
        assert_eq!(candidates[0].text, "你好");
        assert_eq!(candidates[0].end, 5);
    }

    #[test]
    fn host_edits_rebuild_the_double_pinyin_state() {
        let mut scheme = CantoneseScheme::new(Arc::new(inventory()));
        scheme.set_raw_input("nz'hb");
        assert!(scheme.is_shuangpin());
        assert_eq!(scheme.editing_text(), "nz' hb");
    }

    #[test]
    fn candidates_reserve_each_dictionary_batch() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("msime-cantonese.db");
        build(&path);
        let connection = Connection::open(&path).unwrap();
        connection.execute("DELETE FROM entries", []).unwrap();
        for index in 0..23 {
            connection
                .execute(
                    "INSERT INTO entries VALUES (?1, ?2, ?3)",
                    ("nei haa", format!("字{index:02}"), index),
                )
                .unwrap();
        }
        drop(connection);
        let dictionary = open_read_only(&path).unwrap();

        let candidates = typed("neih").candidates(&dictionary).unwrap();
        assert_eq!(candidates.len(), 23);
        assert_eq!(candidates.capacity(), candidates.len());
    }

    #[test]
    fn candidates_into_reuses_candidate_rows_on_requery() {
        let fixture = fixture();
        let scheme = typed("neihou");
        let mut buffer = CantoneseQueryBuffer::default();
        let mut candidates = Vec::new();
        scheme
            .candidates_into(&fixture.dictionary, &mut buffer, &mut candidates)
            .unwrap();
        let pointers = candidates
            .iter()
            .map(|candidate| (candidate.text.as_ptr(), candidate.key.as_ptr()))
            .collect::<Vec<_>>();

        let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
            scheme
                .candidates_into(&fixture.dictionary, &mut buffer, &mut candidates)
                .unwrap();
        });
        assert!(
            allocations <= 8,
            "重复查询只应保留分段所需的少量分配：{allocations}"
        );
        assert_eq!(
            candidates
                .iter()
                .map(|candidate| (candidate.text.as_ptr(), candidate.key.as_ptr()))
                .collect::<Vec<_>>(),
            pointers
        );
    }

    #[test]
    fn ime_scheme_exposes_the_active_cantonese_state() {
        let scheme = crate::ime::scheme::Scheme::Cantonese(typed("neih"));
        assert_eq!(scheme.as_cantonese().unwrap().input(), "neih");
    }

    #[test]
    fn a_trailing_prefix_is_completed() {
        let fixture = fixture();
        let scheme = typed("neih");
        assert_eq!(scheme.editing_text(), "nei h");
        let candidates = scheme.candidates(&fixture.dictionary).unwrap();
        let rows: Vec<_> = candidates
            .iter()
            .map(|candidate| (candidate.text.as_str(), candidate.key.as_str()))
            .collect();
        assert_eq!(
            rows,
            [
                ("你好", "nei hou"),
                ("妳好", "nei hou"),
                ("你向", "nei hoeng"),
                ("你", "nei"),
                ("妳", "nei")
            ]
        );
        assert_eq!(candidates[0].end, 4);
        assert_eq!(candidates[3].end, 3);
        // A prefix as the only syllable completes against single-syllable keys only.
        assert_eq!(listed(&typed("gw"), &fixture.dictionary), ["光/1", "廣/1"]);
    }

    #[test]
    fn a_complete_trailing_syllable_that_starts_a_longer_one_is_completed_too() {
        let fixture = fixture();
        // `ho` is a syllable with no `nei ho` entry; 你好 must stay listed between `neih` and `neihou`.
        let syllables = Inventory::new(SYLLABLES.iter().copied().chain(["ho"]));
        let mut scheme = CantoneseScheme::new(Arc::new(syllables));
        scheme.set_raw_input("neiho");
        assert_eq!(scheme.editing_text(), "nei ho");
        assert_eq!(
            listed(&scheme, &fixture.dictionary),
            ["你好/2", "妳好/2", "你向/2", "你/1", "妳/1"]
        );
        // Exact entries of the whole reading come before its completions: 我 for `ngo`, then 外 for `ngoi`.
        let scheme = typed("ngo");
        assert_eq!(listed(&scheme, &fixture.dictionary), ["我/1", "外/1"]);
    }

    #[test]
    fn the_apostrophe_and_the_longer_syllable() {
        let fixture = fixture();
        let scheme = typed("ngo'oi");
        assert_eq!(scheme.editing_text(), "ngo oi");
        assert_eq!(listed(&scheme, &fixture.dictionary), ["我/1"]);
        let scheme = typed("ngoi");
        assert_eq!(scheme.editing_text(), "ngoi");
        assert_eq!(listed(&scheme, &fixture.dictionary), ["外/1"]);
        let scheme = typed("nei'");
        assert_eq!(scheme.editing_text(), "nei'");
        assert_eq!(listed(&scheme, &fixture.dictionary), ["你/1", "妳/1"]);
    }

    #[test]
    fn unread_letters_leave_the_leading_spans() {
        let fixture = fixture();
        let scheme = typed("neihoux");
        assert_eq!(scheme.editing_text(), "nei hou x");
        assert_eq!(
            listed(&scheme, &fixture.dictionary),
            ["你好/2", "妳好/2", "你/1", "妳/1"]
        );
        let scheme = typed("xx");
        assert_eq!(scheme.editing_text(), "xx");
        assert!(listed(&scheme, &fixture.dictionary).is_empty());
        let scheme = typed("nei'qq");
        assert_eq!(scheme.editing_text(), "nei qq");
    }

    #[test]
    fn selecting_a_partial_span_keeps_the_rest_composing() {
        let fixture = fixture();
        let mut scheme = typed("nei'hou");
        let candidates = scheme.candidates(&fixture.dictionary).unwrap();
        let you = candidates.iter().find(|c| c.text == "你").unwrap();
        assert_eq!(you.syllables, 1);
        assert!(scheme.select(you));
        // The boundary after the selected letters goes with them.
        assert_eq!(scheme.input(), "hou");
        assert_eq!(listed(&scheme, &fixture.dictionary), ["好/1", "號/1"]);
        let good = scheme.candidates(&fixture.dictionary).unwrap().remove(0);
        assert!(!scheme.select(&good));
        assert!(scheme.is_empty());

        let mut scheme = typed("neihou");
        let whole = scheme.candidates(&fixture.dictionary).unwrap().remove(0);
        assert_eq!(whole.text, "你好");
        assert!(!scheme.select(&whole));
        assert!(scheme.is_empty());

        // Letters no syllable reads stay composing after a selection.
        let mut scheme = typed("neihoux");
        let first = scheme.candidates(&fixture.dictionary).unwrap().remove(0);
        assert!(scheme.select(&first));
        assert_eq!(scheme.input(), "x");
    }

    #[test]
    fn keys_and_host_edits() {
        let mut scheme = typed("'nei''");
        assert_eq!(scheme.input(), "nei'");
        for key in [
            SchemeKey::Letter(b'N'),
            SchemeKey::Letter(b'1'),
            SchemeKey::Semicolon,
            SchemeKey::Minus,
            SchemeKey::Symbol(b' '),
            SchemeKey::Requery,
        ] {
            scheme.handle_key(key);
        }
        assert_eq!(scheme.input(), "nei'");
        scheme.handle_key(SchemeKey::Backspace);
        scheme.handle_key(SchemeKey::Backspace);
        assert_eq!(scheme.input(), "ne");

        scheme.set_raw_input("'Nei'' hou1");
        assert_eq!(scheme.input(), "nei'hou");
        let editing = scheme.editing_text();
        assert_eq!(editing, "nei hou");
        assert_eq!(editing.capacity(), editing.len());
        scheme.reset();
        assert!(scheme.is_empty());
        assert_eq!(scheme.editing_text(), "");
        let fixture = fixture();
        assert!(scheme.candidates(&fixture.dictionary).unwrap().is_empty());
    }

    #[test]
    fn set_raw_input_reserves_source_capacity() {
        let source: String = (0..100)
            .map(|index| if index % 5 == 3 { ' ' } else { 'a' })
            .collect();
        let mut scheme = CantoneseScheme::new(Arc::new(inventory()));
        scheme.set_raw_input(&source);
        assert_eq!(scheme.input.len(), source.len());
        assert_eq!(scheme.input.capacity(), source.len());
    }

    #[test]
    fn build_request_reuses_request_strings() {
        let mut scheme = typed("nei'hou");
        let mut request = scheme.build_request();
        let pointers = [
            request.raw_input.as_ptr(),
            request.raw_input_with_cases.as_ptr(),
            request.normalized_input.as_ptr(),
            request.raw_segmentation.as_ptr(),
            request.normalized_segmentation.as_ptr(),
            request.segmentation.as_ptr(),
        ];

        scheme.handle_key(SchemeKey::Requery);
        scheme.build_request_into(&mut request);

        assert_eq!(
            [
                request.raw_input.as_ptr(),
                request.raw_input_with_cases.as_ptr(),
                request.normalized_input.as_ptr(),
                request.raw_segmentation.as_ptr(),
                request.normalized_segmentation.as_ptr(),
                request.segmentation.as_ptr(),
            ],
            pointers
        );
    }

    #[test]
    fn an_edit_of_the_spaced_text_keeps_the_shown_boundaries() {
        // `ngo'oi` shows `ngo oi`; deleting its `o` must leave `ngo` and `i`, not the single syllable `ngoi`.
        let mut scheme = typed("ngo'oi");
        let mut text = scheme.editing_text();
        text.remove(4);
        assert_eq!(text, "ngo i");
        scheme.set_raw_input(&text);
        assert_eq!(scheme.input(), "ngo'i");
        assert_eq!(scheme.editing_text(), "ngo i");
        // A spaced reading read back is the same reading, with the boundaries made explicit.
        let mut scheme = typed("neihou");
        let text = scheme.editing_text();
        scheme.set_raw_input(&text);
        assert_eq!(scheme.input(), "nei'hou");
        assert_eq!(scheme.editing_text(), "nei hou");
    }

    #[test]
    fn the_dictionary_opens_with_its_inventory() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("msime-cantonese.db");
        build(&path);
        let opened = crate::cantonese::CantoneseDictionary::open(&path).unwrap();
        let mut scheme = CantoneseScheme::new(opened.inventory());
        scheme.set_raw_input("gwongdungwaa");
        assert_eq!(
            listed(&scheme, opened.dictionary()),
            ["廣東話/3", "光/3", "廣東/2", "光/1", "廣/1"]
        );
        assert!(
            crate::cantonese::CantoneseDictionary::open(&dir.path().join("missing.db")).is_err()
        );
    }
}
