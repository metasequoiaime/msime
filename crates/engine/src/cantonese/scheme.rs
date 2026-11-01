//! The Jyutping composition: typed letters and `'` boundaries, read as syllables against the inventory, and the candidates `msime-cantonese.db` has for them. A candidate may cover only the leading syllables; selecting it takes those letters out of the composition and leaves the rest composing, with nothing held back as phrase progress.

use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use super::shuangpin;
use super::syllable::{self, Inventory, Segmentation};
use crate::error::Result;
use crate::language_dictionary::{LanguageDictionary, LanguageEntry};
use crate::types::{QueryRequest, SchemeKey, SchemeType};
use crate::zhuyin::conversion::{self, Lookup};

/// Entries read for one span of complete syllables.
pub const SPAN_LIMIT: usize = 200;
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
    /// 组句时各音节区间的最重词条，键是区间各位置的读音（`describe`）。词库在会话内只读，缓存按内容寻址，连续打字时只有新音节结尾的区间要查。
    best: HashMap<String, Option<(String, LanguageEntry)>>,
    description: String,
}

/// 组句的音节数上限，与注音组字的上限相同；超过时不出整句行，候选照旧。
pub const SENTENCE_MAX_SYLLABLES: usize = conversion::MAX_SYLLABLES;
/// 组句区间缓存的条目上限，超过就整体清空；一句 20 个音节最多 210 个区间。
const SENTENCE_CACHE_LIMIT: usize = 2048;

/// 组句的一个位置：这个音节允许的读音（双拼码可能解出几个），以及它在输入里结束的字节位置。
struct SentencePosition {
    readings: Vec<String>,
    end: usize,
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

    /// 粤语双拼组字展开成粤拼的显示文字（`ngo daa nei`），只给宿主画在屏幕上，编辑和回车提交仍按原始按键。`leading` 是首选候选的词库键和它覆盖的输入字节数：被它覆盖的完整码按它用的读音写，其余的码取库存里解出的第一个读音；末尾单独的声母键、以及分隔符照原样保留，与 `editing_text` 的写法对应。不在双拼解释下时返回 `None`。
    pub fn jyutping_display(&self, leading: Option<(&str, usize)>) -> Option<String> {
        if !self.shuangpin {
            return None;
        }
        // 自动识别入口里奇数长度的输入按普通粤拼查，候选的键与两键一码对不上，这时不借用首选的读音。
        let leading = leading.filter(|_| self.shuangpin_expansions().is_some());
        let mut leading_syllables = leading
            .map(|(key, end)| (key.split(' '), end))
            .into_iter()
            .flat_map(|(syllables, end)| syllables.map(move |syllable| (syllable, end)));
        let mut text = String::with_capacity(self.input.len() * 2);
        let mut offset = 0;
        for (chunk_index, chunk) in self.input.split('\'').enumerate() {
            if chunk_index > 0 {
                text.push('\'');
                offset += 1;
                if !chunk.is_empty() {
                    text.push(' ');
                }
            }
            for (index, code) in chunk.as_bytes().chunks(2).enumerate() {
                if index > 0 {
                    text.push(' ');
                }
                offset += code.len();
                let covered = leading_syllables
                    .next()
                    .filter(|(_, end)| offset <= *end)
                    .map(|(syllable, _)| syllable);
                match (code, covered) {
                    ([_, _], Some(syllable)) => text.push_str(syllable),
                    ([first, second], None) => {
                        match shuangpin::decode(&self.inventory, [*first, *second]).first() {
                            Some(syllable) => text.push_str(syllable),
                            None => text.push_str(&chunk[index * 2..index * 2 + 2]),
                        }
                    }
                    _ => text.push_str(&chunk[index * 2..]),
                }
            }
        }
        Some(text)
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
    ///
    /// 没有词条覆盖全部完整音节时，再插入一行由词库里的词和单字拼成的整句（`insert_sentence`），排在覆盖音节更多的行之后、其余行之前。
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
        let shuangpin = self.shuangpin_expansions();
        let expanded = shuangpin.is_some();
        if let Some(expansion) = shuangpin {
            self.shuangpin_candidates_into(dictionary, buffer, &expansion, candidates)?;
        } else {
            self.candidates_into_jyutping(dictionary, buffer, candidates, true)?;
        }
        self.insert_sentence(dictionary, buffer, expanded, candidates)
    }

    /// 双拼码的各个粤拼读法分别查词，合在一起按覆盖音节数和权重排。
    fn shuangpin_candidates_into(
        &self,
        dictionary: &LanguageDictionary,
        buffer: &mut CantoneseQueryBuffer,
        expansion: &ShuangpinExpansion,
        candidates: &mut Vec<CantoneseCandidate>,
    ) -> Result<()> {
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
        Ok(())
    }

    /// 没有哪一行正好覆盖全部完整音节时，把组出来的整句插在覆盖音节数不超过它的第一行前面。覆盖更多音节的行（末尾单键、末尾半个音节的补全）仍在它前面，连续打字时首选不在整句和补全之间来回跳。
    fn insert_sentence(
        &self,
        dictionary: &LanguageDictionary,
        buffer: &mut CantoneseQueryBuffer,
        shuangpin: bool,
        candidates: &mut Vec<CantoneseCandidate>,
    ) -> Result<()> {
        // 先数位置，已有整段词条（最常见的情形）时不必再构造读音。
        let count = if shuangpin {
            self.input.split('\'').map(|chunk| chunk.len() / 2).sum()
        } else {
            self.segmentation()
                .syllables
                .iter()
                .take_while(|syllable| syllable.complete)
                .count()
        };
        if !(2..=SENTENCE_MAX_SYLLABLES).contains(&count)
            || candidates.iter().any(|row| row.syllables == count)
        {
            return Ok(());
        }
        let positions = self.sentence_positions(shuangpin);
        debug_assert_eq!(positions.len(), count);
        let Some(sentence) = compose_sentence(
            dictionary,
            buffer,
            &positions,
            SENTENCE_AMBIGUOUS_WORD_FLOOR,
        )?
        else {
            return Ok(());
        };
        let index = candidates
            .iter()
            .position(|row| row.syllables <= count)
            .unwrap_or(candidates.len());
        candidates.insert(index, sentence);
        Ok(())
    }

    /// 组句用的完整音节：双拼时每个完整两键码一个位置，读音是它在库存里解出的全部音节；普通粤拼时是切分出的完整音节，各一个读音。末尾单独的声母键和还没拼完的音节不算。
    fn sentence_positions(&self, shuangpin: bool) -> Vec<SentencePosition> {
        let mut positions = Vec::new();
        if shuangpin {
            let mut offset = 0;
            for chunk in self.input.split('\'') {
                for code in chunk.as_bytes().chunks_exact(2) {
                    offset += 2;
                    positions.push(SentencePosition {
                        readings: shuangpin::decode(&self.inventory, [code[0], code[1]]),
                        end: offset,
                    });
                }
                // 奇数段的末尾单键和段后的分隔符。
                offset += chunk.len() % 2 + 1;
            }
            return positions;
        }
        let reading = self.segmentation();
        for syllable in &reading.syllables {
            if !syllable.complete {
                break;
            }
            positions.push(SentencePosition {
                readings: vec![self.input[syllable.start..syllable.end].to_owned()],
                end: syllable.end,
            });
        }
        positions
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

/// 多音节词经过一个解出多个读音的双拼码时，词频最多可以比它覆盖的最弱单字轻多少倍。取值见 `.agents/notes/implemented/feature/2026-11-02-cantonese-word-lattice-sentence.md` 的留一评估。
const SENTENCE_AMBIGUOUS_WORD_FLOOR: i64 = 1000;

/// 用注音的词图转换（`zhuyin::conversion`）把各位置拼成一句：每段取区间内最重的词条，长词优先，同长度比权重和。整句只有一段（本来就是整段词条）、或有位置连单字都查不到时不出整句。`floor` 见 `SENTENCE_AMBIGUOUS_WORD_FLOOR`。
fn compose_sentence(
    dictionary: &LanguageDictionary,
    buffer: &mut CantoneseQueryBuffer,
    positions: &[SentencePosition],
    floor: i64,
) -> Result<Option<CantoneseCandidate>> {
    if buffer.best.len() > SENTENCE_CACHE_LIMIT {
        buffer.best.clear();
    }
    let readings = positions
        .iter()
        .map(|position| position.readings.as_slice())
        .collect::<Vec<_>>();
    let count = readings.len();
    let ambiguous = readings
        .iter()
        .map(|readings| readings.len() > 1)
        .collect::<Vec<_>>();
    // 每个位置单字最重词条的权重，只在有位置解出不止一个读音时才用得到。
    let mut singles = vec![0; count];
    if ambiguous.iter().any(|&ambiguous| ambiguous) {
        for (index, single) in singles.iter_mut().enumerate() {
            *single = cached_best(dictionary, buffer, &readings[index..=index], |entry| {
                entry.map_or(0, |(_, entry)| entry.weight)
            })?;
        }
    }
    let spans = conversion::convert_filtered(
        count,
        &[],
        |start, end, can_improve| {
            cached_best(
                dictionary,
                buffer,
                &readings[start..end],
                |entry| match entry.filter(|(_, entry)| {
                    clears_ambiguous_word_floor(
                        &ambiguous[start..end],
                        &singles[start..end],
                        entry,
                        floor,
                    )
                }) {
                    Some((key, entry)) if can_improve(entry.weight) => {
                        Lookup::Found(key.clone(), entry.clone())
                    }
                    Some(_) => Lookup::Rejected,
                    None => Lookup::Missing,
                },
            )
        },
        |index| readings[index][0].clone(),
    )?;
    // 兜底段的文字就是读音本身，整句里混进拉丁字母没有用处。
    if spans.len() < 2 || spans.iter().any(|span| span.text == span.key) {
        return Ok(None);
    }
    let mut sentence = CantoneseCandidate {
        text: String::new(),
        weight: 0,
        key: String::new(),
        syllables: count,
        end: positions[count - 1].end,
    };
    for span in &spans {
        sentence.text.push_str(&span.text);
        if !sentence.key.is_empty() {
            sentence.key.push(' ');
        }
        sentence.key.push_str(&span.key);
        // 各段都来自缓存里的最重词条，这里再读一次只是取回它的权重。
        let weight = cached_best(
            dictionary,
            buffer,
            &readings[span.start..span.end],
            |entry| entry.map_or(0, |(_, entry)| entry.weight),
        )?;
        sentence.weight = sentence.weight.saturating_add(weight);
    }
    Ok(Some(sentence))
}

/// 区间里的多音节词能否参与组句。单个音节、以及每个位置都只有一个读音的区间（普通粤拼全部如此）总是可以；经过多读音位置的词，词频乘 `floor` 要不低于区间内最弱的单字词频，挡住因读音组合多而碰巧凑上的冷僻词。
fn clears_ambiguous_word_floor(
    ambiguous: &[bool],
    singles: &[i64],
    entry: &LanguageEntry,
    floor: i64,
) -> bool {
    if ambiguous.len() < 2 || !ambiguous.iter().any(|&ambiguous| ambiguous) {
        return true;
    }
    let weakest = singles.iter().copied().min().unwrap_or(0);
    entry.weight.saturating_mul(floor) >= weakest
}

/// 查 `positions` 区间最重的词条并交给 `consume`；同一组读音在会话内只查一次。
fn cached_best<T>(
    dictionary: &LanguageDictionary,
    buffer: &mut CantoneseQueryBuffer,
    positions: &[&[String]],
    consume: impl FnOnce(Option<&(String, LanguageEntry)>) -> T,
) -> Result<T> {
    describe(positions, &mut buffer.description);
    if let Some(entry) = buffer.best.get(buffer.description.as_str()) {
        return Ok(consume(entry.as_ref()));
    }
    let entry = dictionary.lookup_readings(positions, 1)?.into_iter().next();
    let description = buffer.description.clone();
    Ok(consume(
        buffer.best.entry(description).or_insert(entry).as_ref(),
    ))
}

/// `cached_best` 的缓存键：各位置的读音以 `|` 连接，位置之间用空格。
fn describe(positions: &[&[String]], description: &mut String) {
    description.clear();
    for (index, readings) in positions.iter().enumerate() {
        if index > 0 {
            description.push(' ');
        }
        for (reading_index, reading) in readings.iter().enumerate() {
            if reading_index > 0 {
                description.push('|');
            }
            description.push_str(reading);
        }
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
        // `da` 只能是 `daa`；`daap` 在 `c` 键上，`ngo daap`、`ngo daai` 不能靠补全混进来。首位是「我」「打」组出的整句。
        for force in [true, false] {
            let scheme = shuangpin_typed("voda", force);
            assert!(scheme.is_shuangpin());
            assert_eq!(
                listed_with_end(&scheme, &fixture.dictionary),
                ["我打/2/4", "我/1/2", "外/1/2"],
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

    /// #10557：整段读音不是词库里的一个词时，由已有的词和单字组出整句放在首位。
    #[test]
    fn shuangpin_sentence_is_composed_from_dictionary_words() {
        let fixture = shuangpin_fixture();
        for force in [true, false] {
            let mut scheme = shuangpin_typed("vodanz", force);
            let candidates = scheme.candidates(&fixture.dictionary).unwrap();
            assert_eq!(
                listed_with_end(&scheme, &fixture.dictionary),
                ["我打你/3/6", "我/1/2", "外/1/2"],
                "force={force}"
            );
            let sentence = &candidates[0];
            assert_eq!(sentence.key, "ngo daa nei");
            assert_eq!(sentence.weight, 6000 + 500);
            // 选中整句一次上屏，组字清空。
            assert!(!scheme.select(sentence));
            assert!(scheme.is_empty());
        }
        // 整段本来就是一个词时不另加整句行。
        assert_eq!(
            listed_with_end(&shuangpin_typed("danz", true), &fixture.dictionary),
            ["打你/2/4", "打/1/2"]
        );
    }

    #[test]
    fn a_trailing_single_key_row_stays_ahead_of_the_sentence() {
        let fixture = shuangpin_fixture_with(&[("ngo daa nei dei", "我打你哋".to_owned(), 5)]);
        // 末尾单键补全出的行覆盖四个音节，排在只覆盖前三个码的整句前面。
        assert_eq!(
            listed_with_end(&shuangpin_typed("vodanzd", true), &fixture.dictionary),
            ["我打你哋/4/7", "我打你/3/6", "我/1/2", "外/1/2"]
        );
        // 末尾单键补不出东西时，整句就是首选。
        assert_eq!(
            listed_with_end(&shuangpin_typed("vodanzc", true), &fixture.dictionary)[0],
            "我打你/3/6"
        );
        // 分隔符不改变整句覆盖的位置。
        let scheme = shuangpin_typed("vo'danz", true);
        let sentence = scheme.candidates(&fixture.dictionary).unwrap().remove(0);
        assert_eq!((sentence.text.as_str(), sentence.end), ("我打你", 7));
    }

    #[test]
    fn a_rare_word_through_an_ambiguous_code_does_not_win_the_sentence() {
        // `vo` 解出 ngo、ngoi。权重 1 的「外打」靠长词优先会压过「我」「打」两个常用单字，经过多读音码时被下限挡住。
        let fixture = shuangpin_fixture_with(&[("ngoi daa", "外打".to_owned(), 1)]);
        assert_eq!(
            listed_with_end(&shuangpin_typed("vodavo", true), &fixture.dictionary)[0],
            "我打我/3/6"
        );
        // 普通粤拼每个位置只有一个读音，用户明确打了 ngoi，词条照常参与。
        let scheme = shuangpin_typed("ngoi'daa'ngo", false);
        assert!(!scheme.is_shuangpin());
        assert_eq!(
            listed_with_end(&scheme, &fixture.dictionary)[0],
            "外打我/3/12"
        );
    }

    #[test]
    fn plain_jyutping_composes_a_sentence_too() {
        let fixture = fixture();
        let scheme = typed("neihouwaa");
        assert_eq!(
            listed(&scheme, &fixture.dictionary),
            ["你好話/3", "你好/2", "妳好/2", "你/1", "妳/1"]
        );
        assert_eq!(
            scheme.candidates(&fixture.dictionary).unwrap()[0].key,
            "nei hou waa"
        );
        // 末尾还没拼完的音节不参与组句。
        let scheme = typed("waaneih");
        let rows = listed(&scheme, &fixture.dictionary);
        assert_eq!(rows[0], "話你/2", "{rows:?}");
        // 有音节连单字都查不到时不出整句，拉丁字母不混进句子。
        assert!(listed(&typed("hoeng'nei"), &fixture.dictionary).is_empty());
        // 超过上限不组句。
        let long = "nei'".repeat(SENTENCE_MAX_SYLLABLES) + "hou";
        let rows = typed(&long).candidates(&fixture.dictionary).unwrap();
        assert!(rows.iter().all(|row| row.syllables < 3), "{rows:?}");
        let long = "nei'".repeat(SENTENCE_MAX_SYLLABLES - 1) + "hou";
        let rows = typed(&long).candidates(&fixture.dictionary).unwrap();
        assert_eq!(rows[0].syllables, SENTENCE_MAX_SYLLABLES);
        assert_eq!(
            rows[0].text,
            "你".repeat(SENTENCE_MAX_SYLLABLES - 2) + "你好"
        );
    }

    #[test]
    fn jyutping_display_expands_codes_with_the_leading_reading() {
        let inventory = Arc::new(Inventory::new(SHUANGPIN_SYLLABLES.iter().copied()));
        let mut scheme = CantoneseScheme::new(inventory);
        scheme.set_force_shuangpin(true);
        scheme.set_raw_input("vodanz");
        assert_eq!(
            scheme.jyutping_display(Some(("ngo daa nei", 6))).as_deref(),
            Some("ngo daa nei")
        );
        // 首选只覆盖开头时，其余码取库存里解出的第一个读音。
        assert_eq!(
            scheme.jyutping_display(Some(("ngoi", 2))).as_deref(),
            Some("ngoi daa nei")
        );
        assert_eq!(
            scheme.jyutping_display(None).as_deref(),
            Some("ngo daa nei")
        );
        // 末尾单独的声母键和分隔符照原样保留。
        scheme.set_raw_input("vo'danzd");
        assert_eq!(
            scheme
                .jyutping_display(Some(("ngo daa nei dei", 8)))
                .as_deref(),
            Some("ngo' daa nei d")
        );
        scheme.set_raw_input("vo'");
        assert_eq!(scheme.jyutping_display(None).as_deref(), Some("ngo'"));
        // 编辑文字仍是原始按键。
        scheme.set_raw_input("vodanz");
        assert_eq!(scheme.editing_text(), "vo da nz");
        // 不在双拼解释下没有展开文字。
        let plain = typed("neihou");
        assert_eq!(plain.jyutping_display(None), None);
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
        // 没有整词时由「周」和「杰倫」组出整句排在首位，不必像 #10369 报告的那样分两次选。
        let fixture = fixture_from(&SYLLABLES, &entries[..entries.len() - 1]);
        let scheme = shuangpin_typed_with(&SYLLABLES, "zkgtlp", true);
        assert_eq!(
            listed_with_end(&scheme, &fixture.dictionary),
            ["周杰倫/3/6", "周/1/2", "找/1/2"]
        );
        let sentence = scheme.candidates(&fixture.dictionary).unwrap().remove(0);
        assert_eq!(sentence.key, "zau git leon");
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
        // 两个完整音节没有整段词条，组出的整句排在单字前面。
        assert_eq!(listed(&scheme, &fixture.dictionary), ["我愛/2", "我/1"]);
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

    /// 留一评估：从发布词库里取 3 到 6 个音节的词条，把它们从词库副本里删掉，再按双拼和普通粤拼打出它们的读音，看组出的首选能否还原原词。这是合成的回归集，不是母语者审校的开发集。另外逐键打较长的输入，报告每次按键的查询耗时。
    #[test]
    #[ignore = "需要通过 MSIME_TEST_CANTONESE_DICTIONARY 指定发布的粤语词库，用 --release 单独运行"]
    fn shipped_dictionary_leave_one_out_sentences() {
        let source =
            std::env::var_os("MSIME_TEST_CANTONESE_DICTIONARY").expect("指定发布的粤语词库");
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("msime-cantonese.db");
        std::fs::copy(source, &path).unwrap();
        let connection = Connection::open(&path).unwrap();
        // 每个键只看最重的词面；词面的字数要等于音节数，每个音节都要能编成双拼码。
        let mut rows = connection
            .prepare(
                "SELECT key, text, weight FROM entries e WHERE weight = (SELECT max(weight) FROM entries WHERE key = e.key) ORDER BY weight DESC, key ASC, text ASC",
            )
            .unwrap()
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, i64>(2)?,
                ))
            })
            .unwrap()
            .map(Result::unwrap)
            .filter(|(key, text, _)| {
                let syllables = key.split(' ').collect::<Vec<_>>();
                (3..=6).contains(&syllables.len())
                    && text.chars().count() == syllables.len()
                    && syllables.iter().all(|syllable| shuangpin::encode(syllable).is_some())
            })
            .collect::<Vec<_>>();
        rows.dedup_by(|left, right| left.0 == right.0);
        // 两组：最常用的 300 条，以及在其余词条里等距取的 300 条。
        let common = rows.iter().take(300).cloned().collect::<Vec<_>>();
        let step = (rows.len() - 300) / 300;
        let spread = rows[300..]
            .iter()
            .step_by(step.max(1))
            .take(300)
            .cloned()
            .collect::<Vec<_>>();
        for (key, _, _) in common.iter().chain(&spread) {
            connection
                .execute("DELETE FROM entries WHERE key = ?1", (key,))
                .unwrap();
        }
        // 第二类：把常用的双音节词两两、三三接起来，看组句能否按词还原。接起来的读音本身是词条时跳过。
        let words = connection
            .prepare(
                "SELECT key, text FROM entries e WHERE length(key) - length(replace(key, ' ', '')) = 1 AND weight = (SELECT max(weight) FROM entries WHERE key = e.key) ORDER BY weight DESC, key ASC, text ASC LIMIT 2000",
            )
            .unwrap()
            .query_map([], |row| Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?)))
            .unwrap()
            .map(Result::unwrap)
            .filter(|(key, text)| {
                text.chars().count() == 2
                    && key.split(' ').all(|syllable| shuangpin::encode(syllable).is_some())
            })
            .collect::<Vec<_>>();
        let mut sequences = Vec::new();
        for (length, take) in [(2, 300), (3, 200)] {
            for group in words.chunks_exact(length).take(take) {
                let key = group
                    .iter()
                    .map(|(key, _)| key.as_str())
                    .collect::<Vec<_>>()
                    .join(" ");
                let text = group
                    .iter()
                    .map(|(_, text)| text.as_str())
                    .collect::<String>();
                let exists: bool = connection
                    .query_row(
                        "SELECT EXISTS(SELECT 1 FROM entries WHERE key = ?1)",
                        (&key,),
                        |row| row.get(0),
                    )
                    .unwrap();
                if !exists {
                    sequences.push((key, text, 0));
                }
            }
        }
        drop(connection);
        let opened = crate::cantonese::CantoneseDictionary::open(&path).unwrap();
        let dictionary = opened.dictionary();
        let floors = [1, 10, 100, 1000, 10_000, i64::MAX];
        for (name, set) in [
            ("common", &common),
            ("spread", &spread),
            ("words", &sequences),
        ] {
            let mut first_shuangpin = 0;
            let mut first_plain = 0;
            let mut by_floor = [0; 6];
            for (key, text, _) in set.iter() {
                let mut buffer = CantoneseQueryBuffer::default();
                let codes = key
                    .split(' ')
                    .map(|syllable| {
                        let code = shuangpin::encode(syllable).unwrap();
                        String::from_utf8(code.to_vec()).unwrap()
                    })
                    .collect::<String>();
                let mut scheme = CantoneseScheme::new(opened.inventory());
                scheme.set_force_shuangpin(true);
                scheme.set_raw_input(&codes);
                let mut candidates = Vec::new();
                scheme
                    .candidates_into(dictionary, &mut buffer, &mut candidates)
                    .unwrap();
                if candidates.first().is_some_and(|row| &row.text == text) {
                    first_shuangpin += 1;
                } else if std::env::var_os("MSIME_TEST_CANTONESE_FAILURES").is_some() {
                    println!(
                        "  {name} 未还原：{text} {key} -> {:?}",
                        candidates
                            .first()
                            .map(|row| (&row.text, &row.key, row.syllables))
                    );
                }
                let positions = scheme.sentence_positions(true);
                for (index, floor) in floors.iter().enumerate() {
                    let sentence =
                        compose_sentence(dictionary, &mut buffer, &positions, *floor).unwrap();
                    if sentence.is_some_and(|sentence| &sentence.text == text) {
                        by_floor[index] += 1;
                    }
                }
                let mut plain = CantoneseScheme::new(opened.inventory());
                plain.set_raw_input(&key.replace(' ', ""));
                if plain
                    .candidates(dictionary)
                    .unwrap()
                    .first()
                    .is_some_and(|row| &row.text == text)
                {
                    first_plain += 1;
                }
            }
            println!(
                "{name}: {} 条，双拼首选还原 {first_shuangpin}，普通粤拼首选还原 {first_plain}；双拼整句按下限 {floors:?} 还原 {by_floor:?}",
                set.len()
            );
        }

        // 逐键打 16 个码：把几条常用词条的码接起来，每次按键都像会话那样整段重查。
        let mut warm = Vec::new();
        let mut cold = Vec::new();
        let mut baseline = Vec::new();
        for group in common.chunks(4).take(40) {
            let codes = group
                .iter()
                .flat_map(|(key, _, _)| key.split(' '))
                .take(16)
                .map(|syllable| {
                    String::from_utf8(shuangpin::encode(syllable).unwrap().to_vec()).unwrap()
                })
                .collect::<String>();
            let mut buffer = CantoneseQueryBuffer::default();
            let mut candidates = Vec::new();
            let mut scheme = CantoneseScheme::new(opened.inventory());
            scheme.set_force_shuangpin(true);
            for length in 1..=codes.len() {
                scheme.set_raw_input(&codes[..length]);
                let started = std::time::Instant::now();
                scheme
                    .candidates_into(dictionary, &mut buffer, &mut candidates)
                    .unwrap();
                warm.push(started.elapsed());
                let started = std::time::Instant::now();
                scheme
                    .candidates_into(
                        dictionary,
                        &mut CantoneseQueryBuffer::default(),
                        &mut candidates,
                    )
                    .unwrap();
                cold.push(started.elapsed());
                // 不组句时的同一次查询，作对照。
                let started = std::time::Instant::now();
                match scheme.shuangpin_expansions() {
                    Some(expansion) => scheme
                        .shuangpin_candidates_into(
                            dictionary,
                            &mut buffer,
                            &expansion,
                            &mut candidates,
                        )
                        .unwrap(),
                    None => scheme
                        .candidates_into_jyutping(dictionary, &mut buffer, &mut candidates, true)
                        .unwrap(),
                }
                baseline.push(started.elapsed());
            }
        }
        for (name, samples) in [
            ("连续打字（缓存）", &mut warm),
            ("每键冷缓存", &mut cold),
            ("不组句的对照", &mut baseline),
        ] {
            samples.sort();
            let at = |fraction: f64| samples[((samples.len() - 1) as f64 * fraction) as usize];
            println!(
                "{name}: {} 次按键，p50 {:?}，p95 {:?}，p99 {:?}，最大 {:?}",
                samples.len(),
                at(0.5),
                at(0.95),
                at(0.99),
                samples[samples.len() - 1]
            );
        }
    }
}
