//! The Dachen bopomofo editor with libchewing's semantics: keys fill the pending syllable, a tone key completes it against the syllable inventory, and the completed syllables are reconverted after every change. Candidates come from a list the user opens; choosing one pins that span's text and never commits. Text leaves the editor only through Enter, Shift punctuation, auto-shift past `MAX_SYLLABLES` and `take_text`.
//!
//! 九键模式（`set_nine_key`）下不读大千键：数字串按 `nine_key::KEYPAD` 记下符号位置，声调键结束音节，这个位置的读音是数字串加声调对应的全部合法音节；转换在每个位置的读音里一起挑，用户可以经 `choose_spelling` 逐个钉住目标音节的读音。

use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};
use std::sync::Arc;

use super::conversion::{self, Span, MAX_SYLLABLES};
use super::layout::{self, DACHEN_SYMBOLS, IDLE_SYMBOLS, SHIFT_PUNCTUATION};
use super::nine_key::{self, NineKeyIndex};
use super::syllable::PendingSyllable;
use crate::error::Result;
use crate::language_dictionary::{LanguageDictionary, LanguageEntry};
use crate::types::{QueryRequest, SchemeType};

/// The non-letter keys the editor still claims while the list is open: the phonetic keys that are not selection digits. Digits 1–9 and Space go to selection.
pub const LIST_OPEN_SYMBOLS: &str = "0,./;-";

/// A key as the editor sees it. The session maps host keys and command 16 (`Command::ConvertHanja`, "open the candidate list") onto these.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ZhuyinKey {
    /// A printable ASCII key: a Dachen key, a tone key, or a Shift punctuation key.
    Char(u8),
    /// Open the list, or close it when it is open.
    OpenList,
    Enter,
    Escape,
    Backspace,
}

/// One row of the candidate list: `text` for the syllables from `start` to the end of the composition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListCandidate {
    pub text: String,
    pub start: usize,
    /// 这一行在词库里的键（带调音节以空格连接），选中后随 pin 一起保存，说明被覆盖的音节用的是哪个读音。
    pub key: String,
}

/// 一个已完成的音节和打出它的键。
#[derive(Debug, Clone, PartialEq, Eq)]
struct Syllable {
    /// 大千下是大千键加声调键，九键下是数字串加声调键。
    keys: String,
    /// 这个位置合法的带调音节。大千下只有一个；九键下是这串数字和声调对应的全部音节。
    readings: Arc<[String]>,
    /// 九键下用户钉住的读音在 `readings` 里的下标；大千下恒为 `None`。
    locked: Option<usize>,
}

impl Syllable {
    /// 转换时这个位置允许的读音：钉住的那一个，否则全部。
    fn allowed(&self) -> &[String] {
        match self.locked {
            Some(index) => std::slice::from_ref(&self.readings[index]),
            None => &self.readings,
        }
    }
}

pub struct ZhuyinScheme {
    dictionary: LanguageDictionary,
    syllables: Vec<Syllable>,
    /// Spans whose text the user chose from the list; non-overlapping.
    pins: Vec<Span>,
    pending: PendingSyllable,
    /// The current conversion of `syllables`, recomputed after every change to them or to `pins`.
    conversion: Vec<Span>,
    list_open: bool,
    /// The list rows while `list_open`, empty otherwise.
    list: Vec<ListCandidate>,
    /// Text the last key committed, waiting for the session to hand it to the host.
    committed: String,
    /// 组字过程中每段位置描述（各位置允许的读音以 `|` 连接，位置之间用空格）的最重词条及其键。词库只读，所以条目一直有效；九键下每次钉读音都会产生新的描述，缓存在组字结束和每次 auto-shift 时清空。
    best: HashMap<String, Option<(String, LanguageEntry)>>,
    /// 注音九键模式：数字键拼音节，`zxcvb` 和空格是声调键。
    nine_key: bool,
    /// 九键模式第一次处理按键时从音节表建出的索引，之后一直保留。
    nine_key_index: Option<NineKeyIndex>,
    /// 九键下还没按声调键的数字串。
    pending_digits: Vec<u8>,
    /// 九键下供用户钉读音的目标音节的候选读音，当前转换用的排在最前；最近一次转换后算出，列表打开时不对外提供。
    spellings: Vec<String>,
    /// `spellings` 对应的音节下标。
    spelling_target: Option<usize>,
}

impl ZhuyinScheme {
    /// An idle editor reading `dictionary` (`msime-zhuyin.db`).
    pub fn new(dictionary: LanguageDictionary) -> Self {
        Self {
            dictionary,
            syllables: Vec::new(),
            pins: Vec::new(),
            pending: PendingSyllable::default(),
            conversion: Vec::new(),
            list_open: false,
            list: Vec::new(),
            committed: String::new(),
            best: HashMap::new(),
            nine_key: false,
            nine_key_index: None,
            pending_digits: Vec::new(),
            spellings: Vec::new(),
            spelling_target: None,
        }
    }

    /// 切换注音九键模式，丢掉正在进行的组字。`reset` 不改变模式。
    pub fn set_nine_key(&mut self, enabled: bool) {
        self.clear_composition();
        self.nine_key = enabled;
    }

    pub fn nine_key(&self) -> bool {
        self.nine_key
    }

    /// The `msime-zhuyin.db` connection, given back when the editor is replaced so the next one reuses it.
    pub fn into_dictionary(self) -> LanguageDictionary {
        self.dictionary
    }

    /// Drops the composition and any text not yet taken.
    pub fn reset(&mut self) {
        self.clear_composition();
        self.committed.clear();
    }

    pub fn is_composing(&self) -> bool {
        !self.syllables.is_empty() || !self.pending.is_empty() || !self.pending_digits.is_empty()
    }

    /// Handles one key and returns whether the editor claimed it. An unclaimed key is left to the session and host (idle tone digits and Space type themselves; selection digits and Space while the list is open select through the session). Text the key committed is in `take_committed`.
    pub fn handle_key(&mut self, key: ZhuyinKey) -> Result<bool> {
        match key {
            ZhuyinKey::Char(byte) => self.handle_char(byte),
            _ if !self.is_composing() => Ok(false),
            ZhuyinKey::OpenList => {
                if self.list_open {
                    self.close_list();
                } else {
                    self.open_list()?;
                }
                Ok(true)
            }
            ZhuyinKey::Enter => {
                let text = self.take_text();
                self.committed.push_str(&text);
                Ok(true)
            }
            ZhuyinKey::Escape => {
                if self.list_open {
                    self.close_list();
                } else {
                    self.clear_composition();
                }
                Ok(true)
            }
            ZhuyinKey::Backspace => {
                if self.list_open {
                    self.close_list();
                } else if self.pending_digits.pop().is_none() && !self.pending.pop() {
                    let end = self.syllables.len();
                    self.syllables.pop();
                    self.pins.retain(|pin| pin.end != end);
                    self.reconvert()?;
                    if !self.is_composing() {
                        self.clear_composition();
                    }
                }
                Ok(true)
            }
        }
    }

    /// Pins the text of list row `index` over its span and closes the list. Nothing is committed. Returns false when the list is closed or has no such row.
    pub fn select(&mut self, index: usize) -> Result<bool> {
        if !self.list_open {
            return Ok(false);
        }
        let Some(candidate) = self.list.get(index).cloned() else {
            return Ok(false);
        };
        let pin = Span {
            start: candidate.start,
            end: self.syllables.len(),
            key: candidate.key,
            text: candidate.text,
        };
        self.pins
            .retain(|other| !other.overlaps(pin.start, pin.end));
        self.pins.push(pin);
        self.pins.sort_by_key(|pin| pin.start);
        self.close_list();
        self.reconvert()?;
        Ok(true)
    }

    /// The converted text, ending the composition; the pending syllable is dropped. This is what blur, a scheme switch and Enter commit.
    pub fn take_text(&mut self) -> String {
        let text = self.converted_text();
        self.clear_composition();
        text
    }

    /// The text the last key committed; empty when it committed none.
    pub fn take_committed(&mut self) -> String {
        std::mem::take(&mut self.committed)
    }

    /// 打出组字的键，按顺序，供光标锁定的编辑文本使用；九键下是数字串加声调键，例如 `28c39c4`。
    pub fn editing_text(&self) -> String {
        let mut keys = build_editing_keys(&self.syllables, &self.pending);
        keys.extend(self.pending_digits.iter().map(|digit| char::from(*digit)));
        keys
    }

    /// 转换后的文字加上还在拼的部分：大千是待定的注音符号，例如 `你好ㄇㄚ`；九键是还没按声调的数字，例如 `你好28`。
    pub fn reading(&self) -> String {
        let mut reading = self.converted_text();
        reading.push_str(&self.pending.bopomofo());
        reading.extend(self.pending_digits.iter().map(|digit| char::from(*digit)));
        reading
    }

    /// What the session shows and tests for a composition: the reading, which is empty exactly when nothing is composing.
    pub fn preedit(&self) -> String {
        self.reading()
    }

    /// The request the session keeps for the composition. Nothing is queried with it, since the list rows come from the editor itself; `raw_input` is the typed keys the caret-locked editing text shows and `normalized_segmentation` the reading the snapshot draws.
    pub fn build_request(&self) -> QueryRequest {
        let keys = self.editing_text();
        QueryRequest {
            scheme: SchemeType::Zhuyin,
            raw_input: keys.clone(),
            raw_input_with_cases: keys.clone(),
            normalized_input: keys.clone(),
            raw_segmentation: keys,
            normalized_segmentation: self.reading(),
            valid: self.is_composing(),
            ..QueryRequest::default()
        }
    }

    pub fn converted_text(&self) -> String {
        build_converted_text(&self.conversion)
    }

    pub fn list_open(&self) -> bool {
        self.list_open
    }

    /// The list rows; empty unless the list is open.
    pub fn candidates(&self) -> &[ListCandidate] {
        &self.list
    }

    /// 九键下供用户钉读音的候选读音（带调注音，一声不带符号），当前转换用的排在最前；列表打开、没有歧义音节或在大千模式时为空。
    pub fn spellings(&self) -> &[String] {
        if self.list_open {
            return &[];
        }
        &self.spellings
    }

    /// 把 `spellings()[index]` 钉为目标音节的读音并重新转换，不提交任何文字。读音即使与当前转换相同也照样钉住，目标随之移到下一个歧义音节，用户可以逐个确认。列表打开、没有目标或下标越界时返回 false。
    pub fn choose_spelling(&mut self, index: usize) -> Result<bool> {
        if self.list_open {
            return Ok(false);
        }
        let (Some(target), Some(reading)) = (self.spelling_target, self.spellings.get(index))
        else {
            return Ok(false);
        };
        let Some(syllable) = self.syllables.get_mut(target) else {
            return Ok(false);
        };
        let Some(position) = syllable.readings.iter().position(|other| other == reading) else {
            return Ok(false);
        };
        syllable.locked = Some(position);
        self.reconvert()?;
        Ok(true)
    }

    /// The non-letter keys the editor claims in its current state, in `DACHEN_SYMBOLS` order; 九键下见 `nine_key` 的三个常量。
    pub fn spelling_symbols(&self) -> &'static str {
        if self.nine_key {
            return if !self.is_composing() {
                nine_key::IDLE_SYMBOLS
            } else if self.list_open {
                nine_key::LIST_OPEN_SYMBOLS
            } else {
                nine_key::SYMBOLS
            };
        }
        if !self.is_composing() {
            IDLE_SYMBOLS
        } else if self.list_open {
            LIST_OPEN_SYMBOLS
        } else {
            DACHEN_SYMBOLS
        }
    }

    fn handle_char(&mut self, byte: u8) -> Result<bool> {
        if let Some((_, mark)) = SHIFT_PUNCTUATION.iter().find(|(key, _)| *key == byte) {
            let text = self.take_text();
            self.committed.push_str(&text);
            self.committed.push(*mark);
            return Ok(true);
        }
        if self.nine_key {
            return self.handle_nine_key_char(byte);
        }
        if self.list_open {
            if !byte.is_ascii_lowercase() && !LIST_OPEN_SYMBOLS.as_bytes().contains(&byte) {
                return Ok(false);
            }
            self.close_list();
        }
        // Every phonetic key is a lowercase letter or one of IDLE_SYMBOLS, so each starts a composition from idle.
        if let Some((symbol, kind)) = layout::symbol(byte) {
            self.pending.insert(symbol, kind);
            return Ok(true);
        }
        let Some(mark) = layout::tone_mark(byte) else {
            return Ok(false);
        };
        if !self.is_composing() {
            return Ok(false);
        }
        let Some(toned) = self.pending.toned(mark) else {
            // A tone with nothing pending: Space opens the list, the tone digits are swallowed.
            if byte == b' ' {
                self.open_list()?;
            }
            return Ok(true);
        };
        // A syllable the inventory does not know stays pending; libchewing beeps here.
        if !self.dictionary.has_syllable(&toned)? {
            return Ok(true);
        }
        if self.syllables.len() == MAX_SYLLABLES {
            self.shift_leftmost_word();
        }
        let mut keys = self.pending.keys();
        keys.push(char::from(byte));
        self.syllables.push(Syllable {
            keys,
            readings: Arc::from([toned]),
            locked: None,
        });
        self.pending.clear();
        self.reconvert()?;
        Ok(true)
    }

    /// 九键模式的字符键。列表打开时，数字和声调字母关闭列表后照常处理，其余键（包括空格）不认领，交给运行时选行。数字总是认领，只有还能拼成某个音节时才追加，否则吞掉（libchewing 在这里会响铃）。声调键结束一个音节：空闲时声调字母认领但什么都不做，空格不认领；有组字但没有数字时空格打开列表、声调字母被吞掉；数字串加这个声调没有合法音节时数字留着。其他键（包括全部字母）不认领，九键模式不读大千键。
    fn handle_nine_key_char(&mut self, byte: u8) -> Result<bool> {
        let mark = nine_key::tone_mark(byte);
        if self.list_open {
            if !byte.is_ascii_digit() && (mark.is_none() || byte == b' ') {
                return Ok(false);
            }
            self.close_list();
        }
        if byte.is_ascii_digit() {
            let mut next = self.pending_digits.clone();
            next.push(byte);
            if next.len() <= nine_key::MAX_DIGITS && self.nine_key_index()?.accepts(&next) {
                self.pending_digits = next;
            }
            return Ok(true);
        }
        let Some(mark) = mark else {
            return Ok(false);
        };
        if !self.is_composing() {
            return Ok(byte != b' ');
        }
        if self.pending_digits.is_empty() {
            if byte == b' ' {
                self.open_list()?;
            }
            return Ok(true);
        }
        let digits = std::mem::take(&mut self.pending_digits);
        let Some(readings) = self.nine_key_index()?.readings(&digits, mark) else {
            self.pending_digits = digits;
            return Ok(true);
        };
        if self.syllables.len() == MAX_SYLLABLES {
            self.shift_leftmost_word();
        }
        let mut keys: String = digits.iter().map(|digit| char::from(*digit)).collect();
        keys.push(char::from(byte));
        self.syllables.push(Syllable {
            keys,
            readings,
            locked: None,
        });
        self.reconvert()?;
        Ok(true)
    }

    /// 九键索引，第一次用到时从 `msime-zhuyin.db` 的音节表建立，大千用户不付出这份代价。
    fn nine_key_index(&mut self) -> Result<&NineKeyIndex> {
        let index = match self.nine_key_index.take() {
            Some(index) => index,
            None => NineKeyIndex::new(self.dictionary.syllables()?),
        };
        Ok(self.nine_key_index.insert(index))
    }

    /// Commits the first converted word and drops its syllables, making room for one more.
    fn shift_leftmost_word(&mut self) {
        let Some(word) = self.conversion.first() else {
            return;
        };
        let end = word.end;
        self.committed.push_str(&word.text);
        self.syllables.drain(..end);
        self.best.clear();
        self.pins.retain(|pin| pin.start >= end);
        for pin in &mut self.pins {
            pin.start -= end;
            pin.end -= end;
        }
    }

    /// Lists every suffix span of the syllables, longest first, each span's entries by weight. Every entry is listed, since the list is the only way to choose a character; common syllables such as ㄧˋ have over 200. The list stays closed when it would be empty.
    fn open_list(&mut self) -> Result<()> {
        let count = self.syllables.len();
        self.list.clear();
        let positions: Vec<&[String]> = self.syllables.iter().map(Syllable::allowed).collect();
        let mut seen = HashSet::new();
        for start in 0..count {
            let entries = self
                .dictionary
                .lookup_readings(&positions[start..], usize::MAX)?;
            self.list.reserve(entries.len());
            // 九键下同一个字可能在同一位置的两个读音下各有一条，只留较重的那条。
            seen.clear();
            for (key, entry) in entries {
                if !seen.insert(entry.text.clone()) {
                    continue;
                }
                self.list.push(ListCandidate {
                    text: entry.text,
                    start,
                    key,
                });
            }
        }
        self.list_open = !self.list.is_empty();
        Ok(())
    }

    fn close_list(&mut self) {
        self.list_open = false;
        self.list.clear();
    }

    fn clear_composition(&mut self) {
        self.syllables.clear();
        self.pins.clear();
        self.pending.clear();
        self.pending_digits.clear();
        self.spellings.clear();
        self.spelling_target = None;
        self.conversion.clear();
        self.close_list();
        self.best.clear();
    }

    fn reconvert(&mut self) -> Result<()> {
        let positions: Vec<&[String]> = self.syllables.iter().map(Syllable::allowed).collect();
        // 歧义按打字时的读音算，不按钉住后剩下的：钉住当前用的读音不该放出被下限挡住的冷僻词（是之 → 適之）。
        let ambiguous: Vec<bool> = self
            .syllables
            .iter()
            .map(|syllable| syllable.readings.len() > 1)
            .collect();
        let dictionary = &self.dictionary;
        let best = &mut self.best;
        // 每个位置单字最重词条的权重，只在九键下有位置打出来不止一个读音时才用得到。
        let mut singles = Vec::new();
        if ambiguous.iter().any(|&ambiguous| ambiguous) {
            singles.reserve_exact(positions.len());
            for index in 0..positions.len() {
                let weight = cached_best(dictionary, best, &positions[index..=index])?
                    .map_or(0, |(_, entry)| entry.weight);
                singles.push(weight);
            }
        }
        self.conversion = conversion::convert(
            positions.len(),
            &self.pins,
            |start, end| {
                let entry = cached_best(dictionary, best, &positions[start..end])?;
                Ok(entry.filter(|(_, entry)| {
                    clears_ambiguous_word_floor(&ambiguous[start..end], &singles, start, entry)
                }))
            },
            |index| positions[index][0].clone(),
        )?;
        self.refresh_spellings()
    }

    /// 重算钉读音的目标和它的候选读音。目标是第一个不被任何 pin 覆盖、没有钉住、且有不止一个读音的音节；读音按当前转换用的那个、单字最重词条的权重（从重到轻）、字典序排列。没有目标时为空，大千模式下永远为空。
    fn refresh_spellings(&mut self) -> Result<()> {
        self.spellings.clear();
        self.spelling_target = None;
        if !self.nine_key {
            return Ok(());
        }
        let Some(target) = self
            .syllables
            .iter()
            .enumerate()
            .position(|(index, syllable)| {
                syllable.locked.is_none()
                    && syllable.readings.len() > 1
                    && !self.pins.iter().any(|pin| pin.overlaps(index, index + 1))
            })
        else {
            return Ok(());
        };
        let current = self
            .conversion
            .iter()
            .find(|span| span.overlaps(target, target + 1))
            .and_then(|span| span.key.split(' ').nth(target - span.start))
            .map(str::to_owned);
        let readings = Arc::clone(&self.syllables[target].readings);
        let mut ranked = Vec::with_capacity(readings.len());
        for reading in readings.iter() {
            let weight = cached_best(
                &self.dictionary,
                &mut self.best,
                &[std::slice::from_ref(reading)],
            )?
            .map_or(i64::MIN, |(_, entry)| entry.weight);
            ranked.push((
                current.as_deref() != Some(reading.as_str()),
                Reverse(weight),
                reading,
            ));
        }
        ranked.sort();
        self.spellings
            .extend(ranked.into_iter().map(|(_, _, reading)| reading.clone()));
        self.spelling_target = Some(target);
        Ok(())
    }
}

/// How many times lighter than the weakest single character it spans a multi-syllable word may be when it matches through a position with more than one allowed reading.
///
/// Conversion ranks paths by word length first (libchewing's score), which suits Dachen, where each position has exactly one reading. A nine-key position allows 4 to 23 readings, so the combined reading sets of two or three positions match some obscure word almost everywhere, and length-first alone lets 監聽器 (weight 9) beat 今天 (25469) + 去 (28394). Such a word therefore takes part only when its weight times this factor reaches the smallest single-character weight over its positions. 1000 was chosen against a rebuild of the libchewing-derived dictionary: it drops 監聽器, 趕明兒 and 禮教 from 我們今天去學校, 這個東西很便宜 and 請問你叫什麼名字, and of the sampled counted 2 to 4 syllable words that convert to themselves without the floor all but one still do, while a factor of 300 already loses about one in eight of them.
const AMBIGUOUS_WORD_FLOOR: i64 = 1000;

/// Whether `entry`, the heaviest entry for the span starting at syllable `start` whose positions are flagged in `ambiguous`, may take part in conversion. Single syllables and spans where no position was typed with more than one reading (all of Dachen) always may, so Dachen conversion is unchanged; otherwise see `AMBIGUOUS_WORD_FLOOR`. Ambiguity is how the syllable was typed, not what is left after the user pinned a reading: pinning the reading the conversion already uses must not let a word the floor held back win (是之 turning into 適之). `singles` holds every position's single-character weight over its allowed readings and is empty when no position is ambiguous.
fn clears_ambiguous_word_floor(
    ambiguous: &[bool],
    singles: &[i64],
    start: usize,
    entry: &LanguageEntry,
) -> bool {
    if ambiguous.len() < 2 || !ambiguous.iter().any(|&ambiguous| ambiguous) {
        return true;
    }
    let weakest = singles[start..start + ambiguous.len()]
        .iter()
        .copied()
        .min()
        .unwrap_or(0);
    entry.weight.saturating_mul(AMBIGUOUS_WORD_FLOOR) >= weakest
}

/// `positions` 的最重词条及其键，先查缓存。缓存键是各位置允许的读音以 `|` 连接、位置之间用空格；大千下每个位置只有一个读音，所以就是词库键本身。
fn cached_best(
    dictionary: &LanguageDictionary,
    best: &mut HashMap<String, Option<(String, LanguageEntry)>>,
    positions: &[&[String]],
) -> Result<Option<(String, LanguageEntry)>> {
    let description = describe(positions);
    if let Some(entry) = best.get(&description) {
        return Ok(entry.clone());
    }
    let entry = dictionary.lookup_readings(positions, 1)?.into_iter().next();
    best.insert(description, entry.clone());
    Ok(entry)
}

/// `cached_best` 的缓存键。
fn describe(positions: &[&[String]]) -> String {
    positions
        .iter()
        .map(|readings| readings.join("|"))
        .collect::<Vec<_>>()
        .join(" ")
}

fn build_editing_keys(syllables: &[Syllable], pending: &PendingSyllable) -> String {
    let pending_capacity = usize::from(pending.initial.is_some())
        + usize::from(pending.medial.is_some())
        + usize::from(pending.rime.is_some());
    let capacity = syllables
        .iter()
        .map(|syllable| syllable.keys.len())
        .sum::<usize>()
        + pending_capacity;
    let mut keys = String::with_capacity(capacity);
    for syllable in syllables {
        keys.push_str(&syllable.keys);
    }
    pending.append_keys(&mut keys);
    keys
}

fn build_converted_text(spans: &[Span]) -> String {
    let capacity = spans.iter().map(|span| span.text.len()).sum();
    let mut text = String::with_capacity(capacity);
    for span in spans {
        text.push_str(&span.text);
    }
    text
}

#[cfg(test)]
mod tests {
    use rusqlite::Connection;

    use super::*;
    use crate::language_dictionary::{
        open_read_only, FORMAT_VERSION, METADATA_FORMAT_VERSION, SCHEMA,
    };

    const ENTRIES: [(&str, &str, i64); 15] = [
        ("ㄋㄧˇ", "你", 1000),
        ("ㄋㄧˇ", "妳", 300),
        ("ㄋㄧˇ", "擬", 50),
        ("ㄏㄠˇ", "好", 2000),
        ("ㄏㄠˇ", "郝", 10),
        ("ㄋㄧˇ ㄏㄠˇ", "你好", 500),
        ("ㄊㄞˊ", "台", 900),
        ("ㄊㄞˊ", "臺", 400),
        ("ㄨㄢ", "彎", 500),
        ("ㄨㄢ", "灣", 300),
        ("ㄊㄞˊ ㄨㄢ", "臺灣", 800),
        ("ㄊㄞˊ ㄨㄢ", "台灣", 600),
        ("ㄇㄚ˙", "嗎", 800),
        ("ㄇㄚ", "媽", 700),
        ("ㄢ", "安", 100),
    ];

    fn syllable(keys: &str, readings: &[&str]) -> Syllable {
        Syllable {
            keys: keys.to_owned(),
            readings: readings
                .iter()
                .map(|reading| (*reading).to_owned())
                .collect(),
            locked: None,
        }
    }

    #[test]
    fn editing_keys_append_syllables_and_pending_keys_in_order() {
        let syllables = vec![syllable("su3", &["ㄋㄧˇ"]), syllable("lc3", &["ㄏㄠˇ"])];
        let pending = PendingSyllable {
            initial: Some('ㄇ'),
            medial: Some('ㄚ'),
            rime: None,
        };

        assert_eq!(build_editing_keys(&syllables, &pending), "su3lc3a8");
    }

    // 大千下每个位置只有一个读音，缓存键就是词库键；九键的多读音位置以 `|` 连接，钉住后只剩钉住的那个。
    #[test]
    fn cache_keys_describe_the_allowed_readings_in_order() {
        let mut syllables = [
            syllable("su3", &["ㄋㄧˇ"]),
            syllable("lc3", &["ㄏㄠˇ"]),
            syllable("28c", &["ㄋㄧˇ", "ㄌㄧˇ"]),
        ];
        let positions: Vec<&[String]> = syllables.iter().map(Syllable::allowed).collect();
        assert_eq!(describe(&positions[..2]), "ㄋㄧˇ ㄏㄠˇ");
        assert_eq!(describe(&positions), "ㄋㄧˇ ㄏㄠˇ ㄋㄧˇ|ㄌㄧˇ");
        syllables[2].locked = Some(1);
        let positions: Vec<&[String]> = syllables.iter().map(Syllable::allowed).collect();
        assert_eq!(describe(&positions[1..]), "ㄏㄠˇ ㄌㄧˇ");
    }

    #[test]
    fn converted_text_appends_spans_in_order() {
        let spans = vec![
            Span {
                start: 0,
                end: 2,
                key: "ㄋㄧˇ ㄏㄠˇ".to_owned(),
                text: "你好".to_owned(),
            },
            Span {
                start: 2,
                end: 3,
                key: "ㄇㄚ˙".to_owned(),
                text: "嗎".to_owned(),
            },
        ];

        assert_eq!(build_converted_text(&spans), "你好嗎");
    }

    fn scheme() -> (tempfile::TempDir, ZhuyinScheme) {
        scheme_with(&ENTRIES)
    }

    fn scheme_with(entries: &[(&str, &str, i64)]) -> (tempfile::TempDir, ZhuyinScheme) {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("msime-zhuyin.db");
        let connection = Connection::open(&path).unwrap();
        connection.execute_batch(SCHEMA).unwrap();
        connection
            .execute(
                "INSERT INTO metadata VALUES (?1, ?2)",
                (METADATA_FORMAT_VERSION, FORMAT_VERSION.to_string()),
            )
            .unwrap();
        for &(key, text, weight) in entries {
            connection
                .execute(
                    "INSERT INTO entries VALUES (?1, ?2, ?3)",
                    (key, text, weight),
                )
                .unwrap();
            for syllable in key.split(' ') {
                connection
                    .execute("INSERT OR IGNORE INTO syllables VALUES (?1)", (syllable,))
                    .unwrap();
            }
        }
        drop(connection);
        let scheme = ZhuyinScheme::new(open_read_only(&path).unwrap());
        (dir, scheme)
    }

    /// Types `keys` as characters and returns what each claimed.
    fn type_keys(scheme: &mut ZhuyinScheme, keys: &str) -> Vec<bool> {
        keys.bytes()
            .map(|byte| scheme.handle_key(ZhuyinKey::Char(byte)).unwrap())
            .collect()
    }

    fn texts(scheme: &ZhuyinScheme) -> Vec<(&str, usize)> {
        scheme
            .candidates()
            .iter()
            .map(|candidate| (candidate.text.as_str(), candidate.start))
            .collect()
    }

    #[test]
    fn idle_claims_phonetic_keys_but_not_tone_keys() {
        let (_dir, mut scheme) = scheme();
        assert_eq!(scheme.spelling_symbols(), IDLE_SYMBOLS);
        for byte in *b"3467 " {
            assert!(!scheme.handle_key(ZhuyinKey::Char(byte)).unwrap());
            assert!(!scheme.is_composing());
        }
        for key in [
            ZhuyinKey::Enter,
            ZhuyinKey::Escape,
            ZhuyinKey::Backspace,
            ZhuyinKey::OpenList,
        ] {
            assert!(!scheme.handle_key(key).unwrap());
        }
        // Uppercase letters and other ASCII are not Dachen keys.
        assert_eq!(type_keys(&mut scheme, "A!'"), [false, false, false]);
        for start in ["1", ",", "-", "a"] {
            scheme.reset();
            assert_eq!(type_keys(&mut scheme, start), [true]);
            assert!(scheme.is_composing());
            assert_eq!(scheme.spelling_symbols(), DACHEN_SYMBOLS);
        }
        assert_eq!(scheme.reading(), "ㄇ");
        assert_eq!(scheme.editing_text(), "a");
    }

    #[test]
    fn slots_are_replaced_by_keys_of_the_same_kind() {
        let (_dir, mut scheme) = scheme();
        type_keys(&mut scheme, "sxuj");
        assert_eq!(scheme.reading(), "ㄌㄨ");
        assert_eq!(scheme.editing_text(), "xj");
        type_keys(&mut scheme, "sul3");
        assert_eq!(scheme.converted_text(), "");
        assert_eq!(scheme.reading(), "ㄋㄧㄠ");
        type_keys(&mut scheme, "c");
        type_keys(&mut scheme, "j");
        assert_eq!(scheme.reading(), "ㄏㄨㄠ");
    }

    #[test]
    fn tone_keys_complete_only_known_syllables() {
        let (_dir, mut scheme) = scheme();
        // ㄋˇ is not in the inventory: the key is consumed and ㄋ stays pending.
        assert_eq!(type_keys(&mut scheme, "s3"), [true, true]);
        assert_eq!(scheme.reading(), "ㄋ");
        assert_eq!(scheme.editing_text(), "s");
        type_keys(&mut scheme, "u3");
        assert_eq!(scheme.reading(), "你");
        assert_eq!(scheme.editing_text(), "su3");
        // A tone digit with nothing pending is swallowed.
        assert_eq!(type_keys(&mut scheme, "4"), [true]);
        assert_eq!(scheme.reading(), "你");
        // Space is tone 1 and 7 the neutral tone.
        type_keys(&mut scheme, "a8 a87");
        assert_eq!(scheme.reading(), "你媽嗎");
        assert_eq!(scheme.editing_text(), "su3a8 a87");
        assert_eq!(scheme.take_committed(), "");
    }

    #[test]
    fn conversion_prefers_words() {
        let (_dir, mut scheme) = scheme();
        type_keys(&mut scheme, "su3cl3");
        assert_eq!(scheme.converted_text(), "你好");
        type_keys(&mut scheme, "a8");
        assert_eq!(scheme.reading(), "你好ㄇㄚ");
        scheme.reset();
        type_keys(&mut scheme, "w96j0 ");
        assert_eq!(scheme.converted_text(), "臺灣");
    }

    #[test]
    fn the_list_holds_suffix_spans_longest_first() {
        let (_dir, mut scheme) = scheme();
        type_keys(&mut scheme, "w96j0 ");
        assert!(scheme.handle_key(ZhuyinKey::OpenList).unwrap());
        assert!(scheme.list_open());
        assert_eq!(scheme.spelling_symbols(), LIST_OPEN_SYMBOLS);
        assert_eq!(
            texts(&scheme),
            [("臺灣", 0), ("台灣", 0), ("彎", 1), ("灣", 1)]
        );
        // Command 16 again closes it.
        assert!(scheme.handle_key(ZhuyinKey::OpenList).unwrap());
        assert!(!scheme.list_open());
        assert!(scheme.candidates().is_empty());
        // Space with nothing pending opens it too.
        assert!(scheme.handle_key(ZhuyinKey::Char(b' ')).unwrap());
        assert!(scheme.list_open());
    }

    #[test]
    fn the_list_holds_every_entry_of_a_span() {
        // ㄧˋ has 215 single characters in the real data; the list must not cut off the light ones.
        let texts: Vec<String> = (0..215u32)
            .map(|index| char::from_u32(0x4E00 + index).unwrap().to_string())
            .collect();
        let entries: Vec<(&str, &str, i64)> = texts
            .iter()
            .zip((1..=215i64).rev())
            .map(|(text, weight)| ("ㄧˋ", text.as_str(), weight))
            .collect();
        let (_dir, mut scheme) = scheme_with(&entries);
        type_keys(&mut scheme, "u4");
        assert!(scheme.handle_key(ZhuyinKey::OpenList).unwrap());
        assert_eq!(scheme.candidates().len(), 215);
        assert_eq!(scheme.list.capacity(), scheme.list.len());
        assert_eq!(scheme.candidates()[214].text, texts[214]);
    }

    #[test]
    fn reopening_the_list_reuses_its_row_storage() {
        let texts: Vec<String> = (0..215u32)
            .map(|index| char::from_u32(0x4E00 + index).unwrap().to_string())
            .collect();
        let mut entries: Vec<(&str, &str, i64)> = texts
            .iter()
            .zip((1..=215i64).rev())
            .map(|(text, weight)| ("ㄧˋ", text.as_str(), weight))
            .collect();
        entries.push(("ㄋㄧˇ", "你", 1));
        let (_dir, mut scheme) = scheme_with(&entries);

        type_keys(&mut scheme, "u4");
        assert!(scheme.handle_key(ZhuyinKey::OpenList).unwrap());
        let capacity = scheme.list.capacity();
        assert_eq!(scheme.list.len(), 215);
        assert!(scheme.handle_key(ZhuyinKey::OpenList).unwrap());

        scheme.reset();
        type_keys(&mut scheme, "su3");
        assert!(scheme.handle_key(ZhuyinKey::OpenList).unwrap());
        assert_eq!(scheme.list.len(), 1);
        assert!(scheme.list.capacity() >= capacity);
    }

    #[test]
    fn select_pins_without_committing_and_pins_survive_reconversion() {
        let (_dir, mut scheme) = scheme();
        type_keys(&mut scheme, "su3 ");
        assert_eq!(texts(&scheme), [("你", 0), ("妳", 0), ("擬", 0)]);
        assert!(scheme.select(1).unwrap());
        assert!(!scheme.list_open());
        assert_eq!(scheme.take_committed(), "");
        assert_eq!(scheme.reading(), "妳");
        // The pinned 妳 holds even though 你好 is the better word.
        type_keys(&mut scheme, "cl3");
        assert_eq!(scheme.converted_text(), "妳好");
        // Choosing the longer span replaces the pin it overlaps.
        type_keys(&mut scheme, " ");
        assert_eq!(texts(&scheme), [("你好", 0), ("好", 1), ("郝", 1)]);
        assert!(scheme.select(2).unwrap());
        assert_eq!(scheme.converted_text(), "妳郝");
        type_keys(&mut scheme, " ");
        assert!(scheme.select(0).unwrap());
        assert_eq!(scheme.converted_text(), "你好");
        type_keys(&mut scheme, "w96");
        assert_eq!(scheme.converted_text(), "你好台");
        assert!(!scheme.select(0).unwrap());
        assert!(!scheme.list_open());
        // Backspace drops the last syllable; the pin on the others stays.
        scheme.handle_key(ZhuyinKey::Backspace).unwrap();
        assert_eq!(scheme.converted_text(), "你好");
        assert_eq!(scheme.take_committed(), "");
    }

    #[test]
    fn list_keys_close_or_fall_through() {
        let (_dir, mut scheme) = scheme();
        type_keys(&mut scheme, "su3 ");
        assert!(scheme.list_open());
        // Selection digits and Space belong to the session while the list is open.
        assert_eq!(type_keys(&mut scheme, "1 3"), [false, false, false]);
        assert!(scheme.list_open());
        // 0 closes the list and types ㄢ.
        assert_eq!(type_keys(&mut scheme, "0"), [true]);
        assert!(!scheme.list_open());
        assert_eq!(scheme.reading(), "你ㄢ");
        type_keys(&mut scheme, " ");
        assert_eq!(scheme.reading(), "你安");
        // Esc and Backspace only close an open list.
        type_keys(&mut scheme, " ");
        assert!(scheme.handle_key(ZhuyinKey::Escape).unwrap());
        assert!(!scheme.list_open());
        assert_eq!(scheme.reading(), "你安");
        type_keys(&mut scheme, " ");
        assert!(scheme.handle_key(ZhuyinKey::Backspace).unwrap());
        assert_eq!(scheme.reading(), "你安");
        // A letter closes it and composes.
        type_keys(&mut scheme, " ");
        assert_eq!(type_keys(&mut scheme, "c"), [true]);
        assert!(!scheme.list_open());
        assert_eq!(scheme.reading(), "你安ㄏ");
    }

    #[test]
    fn enter_commits_the_conversion_and_esc_clears() {
        let (_dir, mut scheme) = scheme();
        type_keys(&mut scheme, "su3cl3a");
        assert!(scheme.handle_key(ZhuyinKey::Enter).unwrap());
        assert_eq!(scheme.take_committed(), "你好");
        assert!(!scheme.is_composing());

        type_keys(&mut scheme, "su3cl3");
        assert!(scheme.handle_key(ZhuyinKey::Escape).unwrap());
        assert!(!scheme.is_composing());
        assert_eq!(scheme.take_committed(), "");

        type_keys(&mut scheme, "su3");
        assert_eq!(scheme.take_text(), "你");
        assert!(!scheme.is_composing());
    }

    #[test]
    fn backspace_drops_a_symbol_then_a_syllable() {
        let (_dir, mut scheme) = scheme();
        type_keys(&mut scheme, "su3cl3ju");
        let mut shown = vec![scheme.reading()];
        while scheme.is_composing() {
            assert!(scheme.handle_key(ZhuyinKey::Backspace).unwrap());
            shown.push(scheme.reading());
        }
        assert_eq!(shown, ["你好ㄧ", "你好", "你", ""]);
        assert_eq!(scheme.spelling_symbols(), IDLE_SYMBOLS);
    }

    #[test]
    fn shift_punctuation_commits_then_inserts_the_mark() {
        let (_dir, mut scheme) = scheme();
        type_keys(&mut scheme, "su3cl3a");
        assert_eq!(type_keys(&mut scheme, "<"), [true]);
        assert_eq!(scheme.take_committed(), "你好，");
        assert!(!scheme.is_composing());
        assert_eq!(type_keys(&mut scheme, "["), [true]);
        assert_eq!(scheme.take_committed(), "「");
        type_keys(&mut scheme, "su3 ");
        assert_eq!(type_keys(&mut scheme, "?"), [true]);
        assert_eq!(scheme.take_committed(), "你？");
        assert!(!scheme.list_open());
    }

    #[test]
    fn the_21st_syllable_commits_the_leftmost_word() {
        let (_dir, mut scheme) = scheme();
        type_keys(&mut scheme, &"su3cl3".repeat(10));
        assert_eq!(scheme.converted_text(), "你好".repeat(10));
        assert_eq!(scheme.take_committed(), "");
        type_keys(&mut scheme, "su");
        assert_eq!(scheme.take_committed(), "");
        type_keys(&mut scheme, "3");
        assert_eq!(scheme.take_committed(), "你好");
        assert_eq!(scheme.converted_text(), format!("{}你", "你好".repeat(9)));
        assert_eq!(scheme.editing_text(), format!("{}su3", "su3cl3".repeat(9)));
        // The lookup cache keeps no key longer than the syllables that are left.
        let longest = scheme.best.keys().map(|key| key.split(' ').count()).max();
        assert_eq!(longest, Some(scheme.syllables.len()));

        // A pin to the right of the shifted word moves with its syllables.
        scheme.reset();
        type_keys(&mut scheme, &"su3cl3".repeat(10));
        type_keys(&mut scheme, " ");
        assert!(scheme.select(2).unwrap());
        assert_eq!(scheme.converted_text(), format!("{}你郝", "你好".repeat(9)));
        type_keys(&mut scheme, "su3");
        assert_eq!(scheme.take_committed(), "你好");
        assert_eq!(
            scheme.converted_text(),
            format!("{}你郝你", "你好".repeat(8))
        );
    }

    // ---- 注音九键 ----

    /// 大千夹具加上与 ㄋㄧˇ 同为 `28` + ˇ 的 ㄌㄧˇ、ㄉㄧˇ，以及三个符号的 ㄏㄨㄚ。
    fn nine_key_scheme() -> (tempfile::TempDir, ZhuyinScheme) {
        let mut entries = ENTRIES.to_vec();
        entries.extend([
            ("ㄌㄧˇ", "李", 1200),
            ("ㄉㄧˇ", "底", 600),
            ("ㄏㄨㄚ", "花", 500),
        ]);
        let (dir, mut scheme) = scheme_with(&entries);
        scheme.set_nine_key(true);
        (dir, scheme)
    }

    fn spellings(scheme: &ZhuyinScheme) -> Vec<&str> {
        scheme.spellings().iter().map(String::as_str).collect()
    }

    #[test]
    fn nine_key_tone_keys_end_a_syllable_against_every_matching_reading() {
        let (_dir, mut scheme) = nine_key_scheme();
        assert_eq!(scheme.spelling_symbols(), nine_key::IDLE_SYMBOLS);
        assert_eq!(type_keys(&mut scheme, "28"), [true, true]);
        assert_eq!(scheme.reading(), "28");
        assert_eq!(scheme.spelling_symbols(), nine_key::SYMBOLS);
        assert!(scheme.spellings().is_empty());
        // ˇ 结束音节：ㄋㄧˇ、ㄌㄧˇ、ㄉㄧˇ 都在 28 上，单字里 李 最重。
        assert_eq!(type_keys(&mut scheme, "c"), [true]);
        assert_eq!(scheme.reading(), "李");
        assert_eq!(scheme.editing_text(), "28c");
        assert_eq!(spellings(&scheme), ["ㄌㄧˇ", "ㄋㄧˇ", "ㄉㄧˇ"]);
        // 你好 够重，过得了九键的权重下限，双音节词按长度平方胜出；当前用的读音排到最前，其余按单字权重。
        type_keys(&mut scheme, "39c");
        assert_eq!(scheme.converted_text(), "你好");
        assert_eq!(scheme.editing_text(), "28c39c");
        assert_eq!(spellings(&scheme), ["ㄋㄧˇ", "ㄌㄧˇ", "ㄉㄧˇ"]);
        assert_eq!(scheme.conversion[0].key, "ㄋㄧˇ ㄏㄠˇ");
        assert_eq!(scheme.take_committed(), "");
    }

    /// 今天 + 去 的读音和冷僻的 監聽器 落在同样的数字串上：ㄐㄧㄣ/ㄐㄧㄢ 都是 480，ㄊㄧㄢ/ㄊㄧㄥ 都是 280，ㄑㄩˋ/ㄑㄧˋ 都是 48ˋ。
    const AMBIGUOUS_WORD_ENTRIES: [(&str, &str, i64); 8] = [
        ("ㄐㄧㄣ", "今", 33812),
        ("ㄊㄧㄢ", "天", 31487),
        ("ㄑㄩˋ", "去", 28394),
        ("ㄐㄧㄣ ㄊㄧㄢ", "今天", 25469),
        ("ㄐㄧㄢ", "監", 100),
        ("ㄊㄧㄥ", "聽", 3000),
        ("ㄑㄧˋ", "器", 900),
        ("ㄐㄧㄢ ㄊㄧㄥ ㄑㄧˋ", "監聽器", 9),
    ];

    #[test]
    fn nine_key_light_long_words_do_not_beat_heavy_shorter_ones() {
        let (_dir, mut scheme) = scheme_with(&AMBIGUOUS_WORD_ENTRIES);
        scheme.set_nine_key(true);
        type_keys(&mut scheme, "480 280 48v");
        // 只比长度的话三音节的 監聽器（9）会压过 今天（25469）+ 去（28394）。
        assert_eq!(scheme.converted_text(), "今天去");
        // 钉住 ㄐㄧㄢ 之后今天不再匹配，这个位置的单字只剩 監（100），監聽器 过得了下限。
        let jian = scheme
            .spellings()
            .iter()
            .position(|reading| reading == "ㄐㄧㄢ")
            .unwrap();
        assert!(scheme.choose_spelling(jian).unwrap());
        assert_eq!(scheme.converted_text(), "監聽器");

        // 大千下每个位置只有一个读音，照旧按长度：打出 監聽器 的读音就得到 監聽器。
        let (_dir, mut scheme) = scheme_with(&AMBIGUOUS_WORD_ENTRIES);
        type_keys(&mut scheme, "ru0 wu/ fu4");
        assert_eq!(scheme.converted_text(), "監聽器");
    }

    #[test]
    fn nine_key_confirming_the_readings_in_use_keeps_the_text() {
        // 適之 和 是 + 之 用的是同一组读音，权重却低得过不了下限；5 上 ㄓˋ/ㄕˋ、ㄓ/ㄔ 两两同键，两个位置打出来都有歧义。
        let (_dir, mut scheme) = scheme_with(&[
            ("ㄕˋ", "是", 160_000),
            ("ㄓ", "之", 120_000),
            ("ㄕˋ ㄓ", "適之", 1),
            ("ㄓˋ", "至", 500),
            ("ㄔ", "吃", 800),
        ]);
        scheme.set_nine_key(true);
        type_keys(&mut scheme, "5v5 ");
        assert_eq!(scheme.converted_text(), "是之");
        // 读音条第一个就是当前用的读音；逐个确认，歧义仍按打字时算，適之 不会因为两个位置都钉住而冒出来。
        let mut confirmed = 0;
        while !scheme.spellings().is_empty() {
            assert!(scheme.choose_spelling(0).unwrap());
            assert_eq!(scheme.converted_text(), "是之");
            confirmed += 1;
        }
        assert_eq!(confirmed, 2);
    }

    #[test]
    fn nine_key_space_and_z_are_the_first_tone() {
        let (_dir, mut scheme) = nine_key_scheme();
        type_keys(&mut scheme, "29x80 ");
        assert_eq!(scheme.converted_text(), "臺灣");
        assert_eq!(scheme.editing_text(), "29x80 ");
        scheme.reset();
        assert_eq!(type_keys(&mut scheme, "29x80z"), [true; 6]);
        assert_eq!(scheme.converted_text(), "臺灣");
        scheme.reset();
        type_keys(&mut scheme, "17b17 ");
        assert_eq!(scheme.converted_text(), "嗎媽");
        assert!(scheme.nine_key());
    }

    #[test]
    fn nine_key_unknown_codes_stay_pending_and_extra_digits_are_swallowed() {
        let (_dir, mut scheme) = nine_key_scheme();
        // 28 加 ˋ 没有合法音节：声调键被认领，数字留着。
        assert_eq!(type_keys(&mut scheme, "28v"), [true, true, true]);
        assert_eq!(scheme.reading(), "28");
        // 282 不是任何音节的前缀，第三个数字被吞掉。
        assert_eq!(type_keys(&mut scheme, "2"), [true]);
        assert_eq!(scheme.reading(), "28");
        scheme.reset();
        // 三个符号已满，第四个数字被吞掉。
        type_keys(&mut scheme, "3871");
        assert_eq!(scheme.reading(), "387");
        type_keys(&mut scheme, " ");
        assert_eq!(scheme.reading(), "花");
        // 数字从空闲开始组字；夹具里没有 ㄐㄑㄒ 的音节，4 打不出任何音节，只被吞掉。
        scheme.reset();
        assert_eq!(type_keys(&mut scheme, "4"), [true]);
        assert!(!scheme.is_composing());
    }

    #[test]
    fn nine_key_claims_tone_letters_and_digits_but_no_other_letters() {
        let (_dir, mut scheme) = nine_key_scheme();
        // 空闲时：声调字母认领但什么都不做，空格和其他字母不认领。
        assert_eq!(type_keys(&mut scheme, "zxcvb"), [true; 5]);
        assert!(!scheme.is_composing());
        assert_eq!(type_keys(&mut scheme, " sa"), [false, false, false]);
        // 组字中：大千键和其他字母都不认领。
        type_keys(&mut scheme, "28c");
        assert_eq!(type_keys(&mut scheme, "sau,"), [false; 4]);
        assert_eq!(scheme.reading(), "李");
        // 没有数字时声调字母被吞掉。
        assert_eq!(type_keys(&mut scheme, "x"), [true]);
        assert_eq!(scheme.reading(), "李");
        // Shift 标点照常提交转换文字再加标点。
        type_keys(&mut scheme, "39c2");
        assert_eq!(type_keys(&mut scheme, "?"), [true]);
        assert_eq!(scheme.take_committed(), "你好？");
        assert!(!scheme.is_composing());
    }

    #[test]
    fn nine_key_choosing_a_spelling_pins_the_reading() {
        let (_dir, mut scheme) = nine_key_scheme();
        type_keys(&mut scheme, "28c39c");
        assert!(!scheme.choose_spelling(3).unwrap());
        assert!(scheme.choose_spelling(1).unwrap());
        assert_eq!(scheme.converted_text(), "李好");
        assert_eq!(scheme.take_committed(), "");
        // 39c 只有一个读音，没有下一个目标。
        assert!(scheme.spellings().is_empty());
        assert!(!scheme.choose_spelling(0).unwrap());
        // 确认与当前转换相同的读音也会钉住，目标右移。
        type_keys(&mut scheme, "28c");
        assert_eq!(spellings(&scheme), ["ㄌㄧˇ", "ㄋㄧˇ", "ㄉㄧˇ"]);
        assert!(scheme.choose_spelling(0).unwrap());
        assert_eq!(scheme.converted_text(), "李好李");
        assert!(scheme.spellings().is_empty());
        assert_eq!(scheme.syllables[2].allowed(), ["ㄌㄧˇ"]);
        // Enter 提交转换文字，丢掉还没结束的数字。
        type_keys(&mut scheme, "2");
        assert_eq!(scheme.reading(), "李好李2");
        assert!(scheme.handle_key(ZhuyinKey::Enter).unwrap());
        assert_eq!(scheme.take_committed(), "李好李");
        assert!(!scheme.is_composing());
    }

    #[test]
    fn nine_key_list_rows_carry_keys_and_hide_the_spellings() {
        let (_dir, mut scheme) = nine_key_scheme();
        type_keys(&mut scheme, "28c39c");
        // 没有数字时空格打开列表。
        assert_eq!(type_keys(&mut scheme, " "), [true]);
        assert!(scheme.list_open());
        assert_eq!(scheme.spelling_symbols(), nine_key::LIST_OPEN_SYMBOLS);
        assert!(scheme.spellings().is_empty());
        assert!(!scheme.choose_spelling(0).unwrap());
        let rows: Vec<(&str, usize, &str)> = scheme
            .candidates()
            .iter()
            .map(|row| (row.text.as_str(), row.start, row.key.as_str()))
            .collect();
        assert_eq!(
            rows,
            [
                ("你好", 0, "ㄋㄧˇ ㄏㄠˇ"),
                ("好", 1, "ㄏㄠˇ"),
                ("郝", 1, "ㄏㄠˇ")
            ]
        );
        // 列表打开时空格留给运行时选行；数字关闭列表继续拼写。
        assert_eq!(type_keys(&mut scheme, " "), [false]);
        assert_eq!(type_keys(&mut scheme, "2"), [true]);
        assert!(!scheme.list_open());
        assert_eq!(scheme.reading(), "你好2");
        // 声调字母同样关闭列表。
        scheme.handle_key(ZhuyinKey::Backspace).unwrap();
        type_keys(&mut scheme, " ");
        assert!(scheme.list_open());
        assert_eq!(type_keys(&mut scheme, "c"), [true]);
        assert!(!scheme.list_open());
        assert_eq!(spellings(&scheme), ["ㄋㄧˇ", "ㄌㄧˇ", "ㄉㄧˇ"]);
    }

    #[test]
    fn nine_key_list_selection_resolves_the_syllables_it_covers() {
        let (_dir, mut scheme) = nine_key_scheme();
        type_keys(&mut scheme, "28c ");
        let rows: Vec<(&str, &str)> = scheme
            .candidates()
            .iter()
            .map(|row| (row.text.as_str(), row.key.as_str()))
            .collect();
        assert_eq!(
            rows,
            [
                ("李", "ㄌㄧˇ"),
                ("你", "ㄋㄧˇ"),
                ("底", "ㄉㄧˇ"),
                ("妳", "ㄋㄧˇ"),
                ("擬", "ㄋㄧˇ")
            ]
        );
        assert!(scheme.select(1).unwrap());
        assert_eq!(scheme.converted_text(), "你");
        assert_eq!(scheme.pins[0].key, "ㄋㄧˇ");
        // 被 pin 覆盖的音节算已解析，不再是钉读音的目标，也不写 `locked`。
        assert!(scheme.spellings().is_empty());
        assert_eq!(scheme.syllables[0].locked, None);
        type_keys(&mut scheme, "39c");
        assert_eq!(scheme.converted_text(), "你好");
        assert!(scheme.spellings().is_empty());
        // 删掉 pin 结尾的音节时 pin 一起删掉，音节恢复歧义。
        scheme.handle_key(ZhuyinKey::Backspace).unwrap();
        scheme.handle_key(ZhuyinKey::Backspace).unwrap();
        assert!(!scheme.is_composing());
        type_keys(&mut scheme, "28c ");
        assert!(scheme.select(1).unwrap());
        type_keys(&mut scheme, "39c");
        scheme.handle_key(ZhuyinKey::Backspace).unwrap();
        assert_eq!(scheme.converted_text(), "你");
        assert!(scheme.spellings().is_empty());
        // 再删 39c 之后的那一步删掉的是 28c 和以它结尾的 pin。
        scheme.handle_key(ZhuyinKey::Backspace).unwrap();
        type_keys(&mut scheme, "28c");
        assert_eq!(scheme.converted_text(), "李");
        assert_eq!(spellings(&scheme), ["ㄌㄧˇ", "ㄋㄧˇ", "ㄉㄧˇ"]);
    }

    #[test]
    fn nine_key_backspace_drops_a_digit_then_a_syllable_with_its_lock() {
        let (_dir, mut scheme) = nine_key_scheme();
        type_keys(&mut scheme, "28c39c");
        assert!(scheme.choose_spelling(1).unwrap());
        type_keys(&mut scheme, "28");
        let mut shown = vec![(scheme.reading(), scheme.spelling_symbols())];
        while scheme.is_composing() {
            assert!(scheme.handle_key(ZhuyinKey::Backspace).unwrap());
            shown.push((scheme.reading(), scheme.spelling_symbols()));
        }
        assert_eq!(
            shown,
            [
                ("李好28".to_owned(), nine_key::SYMBOLS),
                ("李好2".to_owned(), nine_key::SYMBOLS),
                ("李好".to_owned(), nine_key::SYMBOLS),
                ("李".to_owned(), nine_key::SYMBOLS),
                (String::new(), nine_key::IDLE_SYMBOLS),
            ]
        );
        // 钉住的读音随音节一起删掉。
        type_keys(&mut scheme, "28c39c");
        assert_eq!(scheme.converted_text(), "你好");
        assert_eq!(spellings(&scheme), ["ㄋㄧˇ", "ㄌㄧˇ", "ㄉㄧˇ"]);
        // 列表打开时 Backspace 只关闭列表。
        type_keys(&mut scheme, " ");
        assert!(scheme.handle_key(ZhuyinKey::Backspace).unwrap());
        assert!(!scheme.list_open());
        assert_eq!(scheme.converted_text(), "你好");
    }

    #[test]
    fn nine_key_locks_stay_aligned_across_an_auto_shift() {
        let (_dir, mut scheme) = nine_key_scheme();
        type_keys(&mut scheme, &"28c39c".repeat(10));
        assert_eq!(scheme.converted_text(), "你好".repeat(10));
        // 先确认第一个音节的读音，目标移到第三个音节，再把它钉成 ㄌㄧˇ。
        assert!(scheme.choose_spelling(0).unwrap());
        assert_eq!(scheme.spelling_target, Some(2));
        assert!(scheme.choose_spelling(1).unwrap());
        assert_eq!(scheme.spelling_target, Some(4));
        assert_eq!(
            scheme.converted_text(),
            format!("你好李好{}", "你好".repeat(8))
        );
        type_keys(&mut scheme, "28c");
        assert_eq!(scheme.take_committed(), "你好");
        assert_eq!(scheme.syllables.len(), MAX_SYLLABLES - 1);
        assert_eq!(scheme.syllables[0].allowed(), ["ㄌㄧˇ"]);
        assert_eq!(
            scheme.converted_text(),
            format!("李好{}李", "你好".repeat(8))
        );
        assert_eq!(scheme.spelling_target, Some(2));
    }

    #[test]
    fn nine_key_mode_switches_clear_the_composition_and_reset_keeps_the_mode() {
        let (_dir, mut scheme) = nine_key_scheme();
        type_keys(&mut scheme, "28c3");
        scheme.reset();
        assert!(scheme.nine_key());
        assert!(!scheme.is_composing());
        type_keys(&mut scheme, "28c3");
        scheme.set_nine_key(false);
        assert!(!scheme.is_composing());
        assert!(!scheme.nine_key());
        // 回到大千：数字按大千键处理，没有候选读音。
        assert_eq!(scheme.spelling_symbols(), IDLE_SYMBOLS);
        type_keys(&mut scheme, "su3");
        assert_eq!(scheme.reading(), "你");
        assert!(scheme.spellings().is_empty());
        assert!(!scheme.choose_spelling(0).unwrap());
        scheme.set_nine_key(true);
        assert!(!scheme.is_composing());
        assert_eq!(scheme.spelling_symbols(), nine_key::IDLE_SYMBOLS);
    }
}
