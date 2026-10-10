//! T9 digit composition over quanpin syllables plus English T9 (core-session.md §8). Active while it holds digits; `Session` routes to it then.
//!
//! The digits stay the composition: a chosen spelling only rewrites its span of digits and is remembered in `locked`, and every candidate's `pinyin` is the run of digits it consumes, so selection advances the same way whichever reading produced the row.

use std::cmp::Reverse;
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::OnceLock;

use crate::assets;
use crate::diagnostics;
use crate::dictionary::english::EnglishDictionary;
use crate::error::EngineError;
use crate::ime::queries::{insert_expressive_rows, MIXED_EXPRESSIVE_MINIMUM_INPUT};
use crate::language_dictionary::{self, LanguageDictionary};
use crate::lattice::decode::PHRASE_LENGTH_BONUS;
use crate::local::emoji::{query_emoji_readings, query_kaomoji_readings, ExpressiveRow};
use crate::paths::RuntimePaths;
use crate::pinyin::segment::{cut_one_piece_min_segments, split_segments};
use crate::pinyin::syllables::intact_pinyin_list;
use crate::quanpin::QuanpinDictionary;
use crate::session::SessionSnapshot;
use crate::stroke;
use crate::text::{count_utf8_chars, is_han_phrase};
use crate::types::{
    CandidateSource, Command, EnglishInputOptions, FrequencyAdjustmentMode,
    FrequencyAdjustmentOptions, FuzzyPinyinOptions, KeyResult, LocalInputMode,
    MixedExpressiveOptions, PersonalDictionaryKind, SchemeType, WordItem,
};
use crate::user_dictionary::positions;
use crate::user_dictionary::ranking::{self, RankingRequest};
use crate::user_dictionary::removal;

pub const PATH_LIMIT: usize = 48;
pub const DIGIT_LIMIT: usize = 32;
pub const CANDIDATE_LIMIT: usize = 128;
// 少量查询键直接扫描已有切分路径，避免刷新时为临时哈希表分配堆内存。
const SMALL_QUERY_KEY_BATCH: usize = 64;
// 少量九宫格候选直接扫描已保留词，避免排序后为一次去重分配哈希表。
const SMALL_CANDIDATE_DEDUP: usize = 64;

/// The digit printed beside each of `a..=z` (NK:37).
const KEYPAD: &[u8; 26] = b"22233344455566677778889999";
/// The letters printed on each digit key (NK:74).
const DIGIT_LETTERS: [&str; 10] = [
    "", "", "abc", "def", "ghi", "jkl", "mno", "pqrs", "tuv", "wxyz",
];
/// T9 expansion multiplies by three or four per digit, so only the leading digits become letter prefixes and the dictionary's own prefix search carries the rest (NK:189).
const ENGLISH_PREFIX_BUDGET: usize = 64;
const ENGLISH_LIMIT: usize = 5;
const ENGLISH_CANDIDATE_CAPACITY: usize = ENGLISH_PREFIX_BUDGET * ENGLISH_LIMIT;
/// 九宫格简拼一次最多展开的码数：每个数字取它键上能作音节首字母的字母，五个数字最多 4^5 = 1024 个，五字以内的词都查得到；更长的输入展开太多，不查简拼。
const INITIALS_CODE_LIMIT: usize = 1024;
/// 简拼一次最多取的行数，按权重从高到低。
const INITIALS_ROW_LIMIT: usize = 64;
/// 没打切分时，同样覆盖的词典行里最前面留给音节行（最常用的单字）的位置数；其后的音节行和简拼行按权重归并，见 `interleave_initials`。
const SYLLABLE_ROWS_BEFORE_INITIALS: usize = 3;
/// 混入 emoji、颜文字时最多按几种读法查。一串数字能拼出几十种读法，每种读法要各查一次 emoji 和颜文字；按候选列表排好先后之后只查前面这几种，最可能的读法总在里面。
const EXPRESSIVE_READING_LIMIT: usize = 8;
/// 选中整句时最多存多少个音节，与全拼键盘的 `session::learning::MAX_LEARNED_SENTENCE_SYLLABLES` 相同（`session/tests.rs` 核对两者一致）：更长的整句只用于这一次上屏。
pub(crate) const MAX_LEARNED_SENTENCE_SYLLABLES: usize = 7;

type Path = Vec<String>;

pub struct NineKeySession {
    paths: RuntimePaths,
    learning: bool,
    frequency: FrequencyAdjustmentOptions,
    fuzzy: FuzzyPinyinOptions,
    english_options: EnglishInputOptions,
    /// 候选里混入 emoji、颜文字（共享偏好 `mixed_input.emoji` / `mixed_input.kaomoji`），默认都关。
    expressive: MixedExpressiveOptions,
    digits: String,
    locked: Vec<String>,
    /// 用户用 `'` 切开音节的数字位置，升序，都在已锁定的部分之后。和锁定的拼写不同，切分只定下一个音节在哪里结束，两边数字的各种读法都还保留：`94'26` 可以是 xi'an，也可以是 yi'an，但不会是 xian。
    splits: Vec<usize>,
    spellings: Vec<String>,
    /// `SessionSnapshot::nine_key_reading`，随候选一起重建。
    reading: String,
    candidates: Vec<WordItem>,
    english_only: bool,
    /// 部分选择正在拼的词：已经选掉的各段的全拼（`'` 连接）和文字。整串数字选完时把它们连同最后一段存成用户词，下次打简拼就能出来（#5640）；有一段读不出一字一音节的全拼（英文词、模糊音行）时 `phrase_storable` 为假，这个词不存。
    phrase_pinyin: String,
    phrase_word: String,
    phrase_storable: bool,
    /// 组字光标在 `digits` 里的位置；`None` 是在末尾。用户把光标移进数字中间后（触屏点读音行、硬件键盘的方向键），数字、切分和退格都作用在光标处，用来改掉中间打错的一个数字而不必删掉后面的（#5613）。
    caret: Option<usize>,
    /// 会话允许全拼时为真。为假时九宫格只拼英文，拼音词库永远不打开。
    pinyin: bool,
    /// 只出单字：拼音读法里只收单个汉字，见 `SessionOptions::single_character_only`。
    single_character_only: bool,
    /// Opened on first use.
    dictionary: Option<QuanpinDictionary>,
    /// 每个音节及其前缀的单字频度，`SyllablePrior::from_dictionary` 在第一次查词前建好，给切分路径排序用。
    prior: Option<SyllablePrior>,
    english: Option<EnglishDictionary>,
    /// 和 `locked` 一一对应：撤销那次锁定要用的东西。部分上屏之后锁定的数字位置变了，记录随之作废成 `None`。
    lock_undo: Vec<Option<LockUndo>>,
    /// 从左列末尾选的字母：第一个未锁定的音节必须以它开头。
    initial: Option<Initial>,
    /// 候选只留单字。
    single_character: bool,
    /// 候选首字的笔顺必须以这几笔开头（`hspnz`）；空表示不按笔画筛选。
    strokes: String,
    /// `msime-stroke.db` 的位置；宿主没有时为空，笔画筛选不可用。
    stroke_dictionary: PathBuf,
    /// 第一次按笔画筛选时打开。
    stroke: Option<LanguageDictionary>,
    /// `strokes` 对应的字，随 `set_filter` 重建。
    stroke_texts: HashSet<char>,
}

/// 一次锁定之前的样子。
struct LockUndo {
    /// 这段数字锁定前的样子：锁定把它换成了拼写的编码，拼写比键入的长时还补齐了数字。
    replaced: String,
    /// 锁定时丢掉的切分，都在这段数字里。
    splits: [usize; DIGIT_LIMIT],
    split_count: usize,
    /// 锁定前选的首字母，锁定时并进了拼写。
    initial: Option<Initial>,
    /// 锁定时左列给出的选项。数字全部锁定后左列还给出这一组，选其中一项就换掉这次锁定。
    choices: Vec<String>,
}

#[derive(Clone, Copy)]
struct Initial {
    letter: u8,
    /// 选字母时的数字个数和切分个数。两者都没变时退格撤销这个字母；之后删到更少的数字就再也不撤销它。
    typed: usize,
    splits: usize,
}

/// 左列的一项：完整音节、按键上的字母（大写），或者按键本身的数字。
enum Choice {
    Syllable,
    Letter(u8),
    Digit(u8),
}

fn choice_kind(choice: &str) -> Choice {
    match choice.as_bytes() {
        [letter] if letter.is_ascii_uppercase() => Choice::Letter(letter.to_ascii_lowercase()),
        [digit] if digit.is_ascii_digit() => Choice::Digit(*digit),
        _ => Choice::Syllable,
    }
}

impl NineKeySession {
    /// English options are fixed here; the reference never updated them afterwards. `pinyin` 是会话的 `enabled_schemes` 是否含全拼：九宫格拼的是全拼音节，不含全拼时只给英文行，也不打开拼音词库。
    pub fn new(
        paths: &RuntimePaths,
        learning: bool,
        frequency: FrequencyAdjustmentOptions,
        fuzzy: FuzzyPinyinOptions,
        english: EnglishInputOptions,
        pinyin: bool,
        single_character_only: bool,
    ) -> Self {
        Self {
            paths: paths.clone(),
            learning,
            frequency,
            fuzzy,
            english_options: english,
            expressive: MixedExpressiveOptions::default(),
            digits: String::new(),
            locked: Vec::new(),
            splits: Vec::new(),
            spellings: Vec::new(),
            reading: String::new(),
            candidates: Vec::new(),
            english_only: false,
            phrase_pinyin: String::new(),
            phrase_word: String::new(),
            phrase_storable: true,
            caret: None,
            pinyin,
            single_character_only,
            dictionary: None,
            prior: None,
            english: None,
            lock_undo: Vec::new(),
            initial: None,
            single_character: false,
            strokes: String::new(),
            stroke_dictionary: PathBuf::new(),
            stroke: None,
            stroke_texts: HashSet::new(),
        }
    }

    /// 笔画筛选读的 `msime-stroke.db`；空路径表示没有，`set_filter` 带笔画时报 `LANGUAGE_DICTIONARY_UNAVAILABLE`。
    pub fn set_stroke_dictionary(&mut self, path: PathBuf) {
        if self.stroke_dictionary != path {
            self.stroke = None;
        }
        self.stroke_dictionary = path;
    }

    /// 和 26 键共用的 emoji、颜文字混排开关；与英文选项一样只在建会话时设置。
    pub fn set_mixed_expressive(&mut self, options: MixedExpressiveOptions) {
        self.expressive = options;
    }

    /// Holds digits.
    pub fn active(&self) -> bool {
        !self.digits.is_empty()
    }

    /// English-only T9 is a mode the host enters deliberately, so it is not gated on the mixed-candidate setting or on the minimum prefix a mixed list needs (nine_key_session.h:22-35).
    pub fn set_english_only(&mut self, english_only: bool) {
        if self.english_only == english_only {
            return;
        }
        self.english_only = english_only;
        self.refresh();
    }

    /// `2`..=`9`；到 32 个数字时按 `NINE_KEY_DIGIT_LIMIT` 处理。组字中按 `'` 在光标处切开音节；在同一处再切一次，或者紧跟在锁定的拼写之后切，都没有作用。光标在数字中间时数字插在光标处，光标落在锁定的拼写里时从那个音节起解除锁定。
    pub fn character(&mut self, digit: u8) -> KeyResult {
        if digit == b'\'' {
            // 英文九键的数字拼的是字母不是音节，没有可切的地方，记下的切分上屏时也只会被丢掉。
            if !self.active() || self.english_only || !self.pinyin {
                return KeyResult::unhandled();
            }
            let caret = self.caret_position();
            // 开头和锁定拼写之间的边界本来就是音节的分界，切分没有作用，锁定的拼写也不解除；只有切在一个锁定拼写中间时，才从那个音节起解除锁定，再记下切分。
            if self.on_lock_boundary(caret) || self.splits.contains(&caret) {
                return KeyResult::handled();
            }
            self.unlock_from(caret);
            let at = self.splits.partition_point(|&split| split < caret);
            self.splits.insert(at, caret);
            self.refresh();
            return KeyResult::handled();
        }
        if !(b'2'..=b'9').contains(&digit) {
            return KeyResult::unhandled();
        }
        if self.digits.len() >= DIGIT_LIMIT {
            return KeyResult::handled()
                .with_diagnostic(Some(diagnostics::NINE_KEY_DIGIT_LIMIT.to_string()));
        }
        let caret = self.caret_position();
        self.unlock_from(caret);
        if caret == self.locked_length() {
            // 插在选了首字母的那一位前面：新数字成了下一个音节的开头，首字母是给原来那一位选的。
            self.initial = None;
        }
        self.digits.insert(caret, char::from(digit));
        // 光标前紧挨着的切分留在新数字前面（`94'|26` 打 5 是 `94'5|26`），光标后的切分随数字后移。
        for split in &mut self.splits {
            if *split > caret {
                *split += 1;
            }
        }
        self.set_caret(caret + 1);
        self.refresh();
        KeyResult::handled()
    }

    /// 组字光标的位置，末尾是 `digits.len()`。
    fn caret_position(&self) -> usize {
        let length = self.digits.len();
        self.caret.map_or(length, |caret| caret.min(length))
    }

    fn set_caret(&mut self, caret: usize) {
        self.caret = (caret < self.digits.len()).then_some(caret);
    }

    /// `position` 是开头，或者正好是某个锁定拼写的结尾。
    fn on_lock_boundary(&self, position: usize) -> bool {
        let mut end = 0;
        position == 0
            || self.locked.iter().any(|spelling| {
                end += spelling.len();
                end == position
            })
    }

    /// 在 `position` 处改数字前，解除盖住它的锁定拼写和其后的全部锁定：改动之后那些拼写不一定还拼得出来。锁定少了，原来落在锁定范围里的切分（不会有，切分总在锁定之后）不受影响。解除的锁定连同撤销记录一起丢掉，数字保持锁定时的样子（拼写补齐的数字留着），不像退格撤销锁定那样换回键入的数字：光标位置是按现在的数字算的。首字母限定的是第一个未锁定的音节，锁定少了它的位置就变了，一并丢掉。
    fn unlock_from(&mut self, position: usize) {
        while self.locked_length() > position {
            self.locked.pop();
            self.lock_undo.pop();
            self.initial = None;
        }
    }

    /// 选左列的一项。音节锁进数字；字母限定下一个音节的首字母；数字直接上屏这一位。数字全部锁定时左列是最后一次锁定时的选项，选哪一项都先撤销那次锁定，相当于换选。
    pub fn choose_spelling(&mut self, index: usize) -> KeyResult {
        let Some(choice) = self.spellings.get(index).cloned() else {
            return KeyResult::unhandled();
        };
        let reselecting = self.reselecting();
        let choices = if reselecting {
            match self.undo_last_lock() {
                Some(choices) => choices,
                None => return KeyResult::unhandled(),
            }
        } else {
            Vec::new()
        };
        match choice_kind(&choice) {
            Choice::Digit(digit) => {
                // 只在没有锁定时提供：前面锁定的音节还没上屏，先上屏这一位会颠倒文字的顺序。
                self.consume(1);
                // 这一位原样上屏，和读不出全拼的一段（英文词）一样记进正在拼的词，这个词就不存了；数字上屏完时组字结束，正在拼的词随之作废。
                if self.active() {
                    self.phrase_storable = false;
                    self.phrase_word.push(char::from(digit));
                } else {
                    self.reset_phrase();
                }
                self.refresh();
                return KeyResult::committed(char::from(digit).to_string());
            }
            Choice::Letter(letter) => {
                self.initial = Some(Initial {
                    letter,
                    typed: self.digits.len(),
                    splits: self.splits.len(),
                });
            }
            Choice::Syllable => {
                let choices = if reselecting {
                    choices
                } else {
                    std::mem::take(&mut self.spellings)
                };
                let offset = self.locked_length();
                // A spelling longer than what is typed extends the digits to its whole code; the spelling list only offers ones that stay within the digit limit.
                let end = offset + choice.len().min(self.digits.len() - offset);
                let replaced = self.digits[offset..end].to_string();
                let before = self.digits.len();
                self.digits.replace_range(offset..end, &encode(&choice));
                // 拼写比已打的数字长时数字串变长；光标原来在被替换的那段之后的，跟着后移。
                if let Some(caret) = self.caret {
                    if caret >= end {
                        self.set_caret(caret + self.digits.len() - before);
                    }
                }
                self.locked.push(choice);
                let locked_length = self.locked_length();
                let mut dropped = [0; DIGIT_LIMIT];
                let mut split_count = 0;
                self.splits.retain(|&split| {
                    if split <= locked_length {
                        dropped[split_count] = split;
                        split_count += 1;
                        false
                    } else {
                        true
                    }
                });
                self.lock_undo.push(Some(LockUndo {
                    replaced,
                    splits: dropped,
                    split_count,
                    initial: self.initial.take(),
                    choices,
                }));
            }
        }
        self.refresh();
        KeyResult::handled()
    }

    /// 数字全部锁定，而且最后一次锁定还能撤销。
    fn reselecting(&self) -> bool {
        self.active()
            && self.locked_length() >= self.digits.len()
            && self.lock_undo.last().is_some_and(Option::is_some)
    }

    /// 撤销最后一次锁定，不刷新；返回那次锁定时左列的选项。
    fn undo_last_lock(&mut self) -> Option<Vec<String>> {
        let undo = self.lock_undo.last_mut()?.take()?;
        self.lock_undo.pop();
        let spelling = self.locked.pop()?;
        let offset = self.locked_length();
        let end = (offset + spelling.len()).min(self.digits.len());
        self.digits.replace_range(offset..end, &undo.replaced);
        self.splits
            .extend(undo.splits[..undo.split_count].iter().copied());
        self.splits.sort_unstable();
        self.splits.dedup();
        self.initial = undo.initial.map(|initial| Initial {
            typed: self.digits.len(),
            splits: self.splits.len(),
            ..initial
        });
        // 拼写补齐的数字换回键入的数字后数字串可能变短。这一段就是数字的末尾，光标在它前面或里面键入的那几位上时位置不变（拼写的编码以键入的数字开头），落在补齐的部分里时回到末尾。
        if let Some(caret) = self.caret {
            self.set_caret(caret);
        }
        Some(undo.choices)
    }

    pub fn select(&mut self, index: usize) -> KeyResult {
        let Some(selected) = self.candidates.get(index) else {
            return KeyResult::unhandled();
        };
        let selected_source = selected.source;
        let selected_fixed_position = selected.fixed_position;
        let selected_pinyin_length = selected.pinyin.len();
        let selected_word = selected.word.clone();
        let selected_canonical_pinyin = if self.learning
            && (selected_source.is_dictionary() || selected_source.is_sentence_learning())
        {
            selected.canonical_pinyin.clone()
        } else {
            String::new()
        };
        let mut diagnostic = if self.learning
            && self.frequency.mode != FrequencyAdjustmentMode::Disabled
            && index != 0
            && selected_fixed_position == 0
            && self.editable(index)
        {
            self.adjust_frequency(index, false)
        } else {
            None
        };
        self.consume(selected_pinyin_length);
        if self.learning {
            let learned =
                self.learn_selection(selected_source, &selected_canonical_pinyin, &selected_word);
            diagnostic = diagnostic.or(learned);
        } else {
            self.reset_phrase();
        }
        self.refresh();
        KeyResult::committed(selected_word).with_diagnostic(diagnostic)
    }

    /// 选中一行之后的造词，与全拼键盘的规则相同：选掉一部分数字时记下这一段；选完全部数字时，前面有选过的段就把各段连成一个词存起来（「我滴」+「个天呐」），没有就只在选中的是整句行（词库里没有的句子）时把整句存起来，最多 `MAX_LEARNED_SENTENCE_SYLLABLES` 个音节。词库里本来就有的词不再写。
    fn learn_selection(
        &mut self,
        selected_source: CandidateSource,
        selected_canonical_pinyin: &str,
        selected_word: &str,
    ) -> Option<String> {
        let reading = if selected_source.is_dictionary() || selected_source.is_sentence_learning() {
            selected_canonical_pinyin
        } else {
            ""
        };
        if self.active() {
            if reading.is_empty() {
                self.phrase_storable = false;
            } else if self.phrase_storable {
                if !self.phrase_pinyin.is_empty() {
                    self.phrase_pinyin.push('\'');
                }
                self.phrase_pinyin.push_str(reading);
            }
            self.phrase_word.push_str(selected_word);
            return None;
        }
        let phrase = !self.phrase_word.is_empty();
        let stored = if phrase {
            (self.phrase_storable && !reading.is_empty()).then(|| {
                (
                    format!("{}'{reading}", self.phrase_pinyin),
                    format!("{}{}", self.phrase_word, selected_word),
                )
            })
        } else {
            (selected_source.is_sentence_learning()
                && !reading.is_empty()
                && split_segments(reading).len() <= MAX_LEARNED_SENTENCE_SYLLABLES)
                .then(|| (reading.to_owned(), selected_word.to_owned()))
        };
        self.reset_phrase();
        let (pinyin, word) = stored?;
        // 写入前由 `create_word_from_canonical_pinyin` 核对一字一个完整音节（与全拼键盘存词前的检查相同）；读不出的词（夹着英文或符号）被它拒绝，这不是写入失败，不报诊断。
        match self
            .dictionary
            .get_or_insert_with(|| QuanpinDictionary::new(&self.paths))
            .create_word_from_canonical_pinyin(&pinyin, &word)
        {
            Ok(()) | Err(EngineError::InvalidArgument(_)) => None,
            Err(_) if phrase => Some(diagnostics::PHRASE_NOT_PERSISTED.to_string()),
            Err(_) => Some(diagnostics::SENTENCE_NOT_PERSISTED.to_string()),
        }
    }

    fn reset_phrase(&mut self) {
        self.phrase_pinyin.clear();
        self.phrase_word.clear();
        self.phrase_storable = true;
    }

    /// Out of range commits the digits. 与全拼键盘的 `finish_composition` 相同，余下各段逐个按 `select` 选首选，造词也一样：用户先选掉的段和替他选的余下各段连成一个词存起来（选了「我滴」再打标点，存的是「我滴个天呐」），首选是整句行时存整句。
    pub fn finish(&mut self, first_index: usize) -> KeyResult {
        if !self.active() {
            return KeyResult::unhandled();
        }
        if first_index >= self.candidates.len() {
            return self.command(Command::CommitRaw);
        }
        let mut commit = String::new();
        let mut diagnostic = None;
        let mut index = first_index;
        while self.active() {
            if self.candidates.is_empty() {
                commit.push_str(&self.digits);
                break;
            }
            let result = self.select(index);
            if diagnostic.is_none() {
                diagnostic = result.diagnostic;
            }
            if let Some(text) = result.commit {
                commit.push_str(&text);
            }
            index = 0;
        }
        self.command(Command::Cancel);
        KeyResult::committed(commit).with_diagnostic(diagnostic)
    }

    pub fn command(&mut self, command: Command) -> KeyResult {
        if !self.active() {
            return KeyResult::unhandled();
        }
        match command {
            Command::CommitCandidate => return self.select(0),
            Command::CommitRaw => {
                let raw = self.digits.clone();
                self.command(Command::Cancel);
                return KeyResult::committed(raw);
            }
            Command::Cancel => self.clear_composition(),
            // 光标在末尾时退格撤销最后一步：刚选的首字母、数字全部锁定时的最后一次锁定、末尾的切分，都没有时才删数字。锁定之后还有没锁定的数字时，删的是数字而不是锁定。
            // 光标移进数字中间时退格是在那里改字：先删光标前的切分，再删光标前的数字，光标在开头时什么也不删；不撤销首字母和锁定，那是「撤销最后一步」，而用户把光标移过去是要改那里的数字。
            Command::Backspace => {
                let caret = self.caret_position();
                let at_end = caret == self.digits.len();
                let initial_untouched = at_end
                    && self.initial.is_some_and(|initial| {
                        initial.typed == self.digits.len() && initial.splits == self.splits.len()
                    });
                if initial_untouched {
                    self.initial = None;
                } else if at_end && self.reselecting() {
                    self.undo_last_lock();
                } else if let Some(at) = self.splits.iter().position(|&split| split == caret) {
                    self.splits.remove(at);
                } else if caret > 0 {
                    self.remove_digit(caret - 1);
                    self.set_caret(caret - 1);
                }
            }
            Command::DeleteForward => {
                let caret = self.caret_position();
                if caret < self.digits.len() {
                    self.remove_digit(caret);
                    self.set_caret(caret);
                }
            }
            // 光标移动不改数字，候选不变，不必重查。
            Command::MoveLeft | Command::MoveRight | Command::MoveHome | Command::MoveEnd => {
                let caret = self.caret_position();
                let target = match command {
                    Command::MoveLeft => caret.saturating_sub(1),
                    Command::MoveRight => caret + 1,
                    Command::MoveHome => 0,
                    _ => self.digits.len(),
                };
                self.set_caret(target);
                return KeyResult::handled();
            }
            _ => return KeyResult::unhandled(),
        }
        if !self.active() {
            // 数字删光了，组字结束：锁定、切分、首字母、光标和正在拼的词都不再有意义。
            self.clear_composition();
        }
        self.refresh();
        KeyResult::handled()
    }

    /// 结束这次组字：数字连同锁定、撤销记录、切分、首字母、光标和正在拼的词一起清掉。
    fn clear_composition(&mut self) {
        self.digits.clear();
        self.locked.clear();
        self.lock_undo.clear();
        self.splits.clear();
        self.initial = None;
        self.caret = None;
        self.reset_phrase();
    }

    /// 删掉 `index` 处的数字：盖住它的锁定拼写和其后的锁定一起解除，其后的切分前移一位；前移后重合的、落到锁定范围或开头的切分丢掉。删的是选了首字母的那一位、或者删完没有未锁定的数字时，首字母一起丢掉；删的是它后面的数字时首字母留着，删到比选字母时少的数字后退格不再撤销它（与在末尾退格相同）。
    fn remove_digit(&mut self, index: usize) {
        let anchor = self.locked_length();
        self.unlock_from(index);
        self.digits.remove(index);
        for split in &mut self.splits {
            if *split > index {
                *split -= 1;
            }
        }
        let locked_length = self.locked_length();
        self.splits.retain(|&split| split > locked_length);
        self.splits.dedup();
        let length = self.digits.len();
        if index <= anchor || length <= locked_length {
            self.initial = None;
        } else if let Some(initial) = self.initial.as_mut() {
            if initial.typed > length {
                initial.typed = usize::MAX;
            }
        }
    }

    pub fn pin(&mut self, index: usize) -> KeyResult {
        if !self.editable(index) {
            return KeyResult::unhandled();
        }
        let diagnostic = self.adjust_frequency(index, true);
        self.refresh();
        KeyResult::handled().with_diagnostic(diagnostic)
    }

    pub fn remove(&mut self, index: usize) -> KeyResult {
        if !self.editable(index) || count_utf8_chars(&self.candidates[index].word) <= 1 {
            return KeyResult::unhandled();
        }
        let item = &self.candidates[index];
        let deleted = removal::delete_dictionary_candidate(
            &self.paths.dictionary(assets::MAIN_DICTIONARY),
            &self.paths.user(assets::USER_JOURNAL),
            PersonalDictionaryKind::Pinyin,
            &item.canonical_pinyin,
            &item.word,
        );
        // The reference reports every failed removal with this one message and keeps the composition (NK:434-437).
        if deleted.is_err() {
            return KeyResult::handled().with_diagnostic(Some(
                diagnostics::NINE_KEY_REMOVAL_NOT_PERSISTED.to_string(),
            ));
        }
        if let Some(dictionary) = self.dictionary.as_mut() {
            dictionary.reset_cache();
        }
        self.refresh();
        KeyResult::handled()
    }

    /// 0 clears.
    pub fn set_position(&mut self, index: usize, position: i32) -> KeyResult {
        if !self.editable(index) || !(0..=5).contains(&position) {
            return KeyResult::unhandled();
        }
        let item = &self.candidates[index];
        let journal = self.paths.user(assets::USER_JOURNAL);
        let context = self.ranking_context();
        let saved = if position == 0 {
            positions::clear_fixed_position(&journal, &context, &item.canonical_pinyin, &item.word)
        } else {
            positions::set_fixed_position(
                &journal,
                &context,
                &item.canonical_pinyin,
                &item.word,
                position,
            )
        };
        // Same rule as removal: one message for every failure, composition kept (NK:453-454).
        if saved.is_err() {
            return KeyResult::handled().with_diagnostic(Some(
                diagnostics::NINE_KEY_POSITION_NOT_PERSISTED.to_string(),
            ));
        }
        self.refresh();
        KeyResult::handled()
    }

    pub fn snapshot(&self) -> SessionSnapshot {
        let locked_length = self.locked_length();
        let mut preedit = self.locked.join("'");
        if self.active() && locked_length < self.digits.len() {
            if !preedit.is_empty() {
                preedit.push('\'');
            }
            let mut start = locked_length;
            for &split in &self.splits {
                preedit.push_str(&self.digits[start..split]);
                preedit.push('\'');
                start = split;
            }
            preedit.push_str(&self.digits[start..]);
            if let Some(initial) = self.initial {
                // 选了首字母的那一位显示成字母。
                let at = self.locked.join("'").len() + usize::from(!self.locked.is_empty());
                preedit.replace_range(at..=at, &char::from(initial.letter).to_string());
            }
        }
        SessionSnapshot {
            scheme: SchemeType::Quanpin,
            local_mode: LocalInputMode::None,
            // The grid's snapshot stands in for the whole session's while it is composing, so it carries the mode too: a host that draws its English keys from this flag would otherwise put Chinese ones back on the first digit.
            dedicated_english: self.english_only,
            preedit,
            candidates: self.candidates.clone(),
            editing_text: self.digits.clone(),
            caret_position: self.caret_position(),
            nine_key_spellings: self.spellings.clone(),
            nine_key_reading: self.reading.clone(),
            nine_key_single_character: self.single_character,
            nine_key_strokes: self.strokes.clone(),
            candidate_sources: self.candidates.iter().map(|item| item.source).collect(),
            candidate_annotations: self
                .candidates
                .iter()
                .map(|item| item.corrected_from.clone())
                .collect(),
            // The same test `select` performs: consuming the candidate's digits empties the buffer exactly when its code covers everything typed.
            candidate_answers_key: self
                .candidates
                .iter()
                .map(|item| item.pinyin.len() >= self.digits.len())
                .collect(),
            ..SessionSnapshot::default()
        }
    }

    fn refresh(&mut self) {
        self.candidates.clear();
        self.spellings.clear();
        self.reading.clear();
        if !self.active() {
            // 筛选和撤销记录只属于这一次组字。
            self.lock_undo.clear();
            self.initial = None;
            self.single_character = false;
            self.strokes.clear();
            return;
        }
        if self.english_only || !self.pinyin {
            // No syllables to offer and no pinyin to look up: the digits stand for letters only.
            self.candidates = self.english_candidates(false);
            return;
        }
        // 每个锁定都有一条撤销记录，哪怕是作废的。
        self.lock_undo.resize_with(self.locked.len(), || None);
        let table = spelling_table();
        let locked_length = self.locked_length();
        let remaining = remaining_digits(&self.digits, locked_length);
        let mut split_offsets = [0usize; DIGIT_LIMIT];
        for (index, split) in self.splits.iter().enumerate() {
            split_offsets[index] = split - locked_length;
        }
        let splits = &split_offsets[..self.splits.len()];
        let initial = self.initial.map(|initial| initial.letter);
        let starts_right =
            |piece: &str| initial.is_none_or(|letter| piece.as_bytes().first() == Some(&letter));
        let mut syllables = table.spellings_for(remaining, locked_length, splits.first().copied());
        syllables.retain(|syllable| starts_right(syllable));
        let alternatives = if remaining.is_empty() {
            vec![Vec::new()]
        } else {
            let dictionary = self
                .dictionary
                .get_or_insert_with(|| QuanpinDictionary::new(&self.paths));
            let prior = self
                .prior
                .get_or_insert_with(|| SyllablePrior::from_dictionary(dictionary, table));
            let mut alternatives = table.split_paths(remaining, splits, prior);
            alternatives.retain(|path| path.first().is_none_or(|piece| starts_right(piece)));
            // Even an unfinished or invalid tail must still offer the leading syllable for partial selection.
            alternatives.extend(
                syllables
                    .iter()
                    .filter(|spelling| spelling.len() <= remaining.len())
                    .map(|spelling| vec![spelling.clone()]),
            );
            alternatives
        };
        self.spellings = syllables;

        let mut locked_key = String::new();
        append_path_key(&mut locked_key, "", &self.locked);
        let mut dictionary = self
            .dictionary
            .get_or_insert_with(|| QuanpinDictionary::new(&self.paths))
            .row_cache_batch();
        let mut queried = (alternatives.len() > SMALL_QUERY_KEY_BATCH)
            .then(|| HashSet::with_capacity(alternatives.len()));
        let mut candidates = Vec::with_capacity(CANDIDATE_LIMIT);
        let mut key = String::with_capacity(locked_key.len() + self.digits.len() * 4 + 1);
        // 各条切分的前缀组彼此大量重复，一次刷新会推入上万行，见 `push_ranked`。
        let mut leading: HashMap<String, RankKey> = HashMap::with_capacity(CANDIDATE_LIMIT);
        // Only a split the user typed says where a syllable ends; without one, `3` must keep 的 (a completion of d) ahead of the rarer 额 (e).
        let prefer_exact = !self.splits.is_empty();
        // 每个数字都能当一个音节的首字母时也按简拼查（`68` 是 m't：明天、每天）。用户在每个数字之间都打了切分（`6'8`），说的就是简拼，简拼行排在前面；没打切分时数字也可能是完整音节（`68` 是 mu），简拼行排在同样覆盖的音节行之后。
        // 在展开面板里选定了首字母时用户是在逐个拼音节，简拼行不经过 `starts_right` 的首字母过滤，这时不查简拼。
        let initials =
            self.locked.is_empty() && initial.is_none() && initials_apply(remaining.len(), splits);
        let initials_lead = initials && !splits.is_empty();
        for (index, path) in alternatives.iter().enumerate() {
            append_path_key(&mut key, &locked_key, path);
            if key.is_empty() || !query_key_is_new(&alternatives, index, &key, queried.as_ref()) {
                continue;
            }
            let full_len = self.locked.len() + path.len();
            for mut candidate in dictionary.query(&key, &key, 0, self.fuzzy) {
                if self.single_character_only && is_han_phrase(&candidate.word) {
                    continue;
                }
                let canonical = take_candidate_canonical_pinyin(&mut candidate);
                // A row with more syllables than the path is a completion past the typed digits.
                if canonical.matches('\'').count() >= full_len {
                    continue;
                }
                let matched = if candidate.fuzzy {
                    candidate.pinyin.as_str()
                } else {
                    canonical.as_str()
                };
                let code = encode(matched);
                if code.is_empty()
                    || (!code.starts_with(&self.digits) && !self.digits.starts_with(&code))
                {
                    continue;
                }
                if !agrees_with_locked(matched, &locked_key) {
                    continue;
                }
                if let Some(letter) = initial {
                    // 模糊音可能从别的声母读到这一行（`zi` 读出 `zhi`），所以对照它实际匹配的拼写。
                    let next = matched
                        .split('\'')
                        .nth(self.locked.len())
                        .and_then(|syllable| syllable.as_bytes().first());
                    if next != Some(&letter) {
                        continue;
                    }
                }
                candidate.pinyin.clear();
                candidate
                    .pinyin
                    .push_str(&self.digits[..code.len().min(self.digits.len())]);
                candidate.canonical_pinyin = canonical;
                push_ranked(
                    &mut candidates,
                    &mut leading,
                    candidate,
                    prefer_exact,
                    initials_lead,
                );
            }
            if let Some(seen) = queried.as_mut() {
                seen.insert(key.clone());
            }
        }
        if let Some(codes) = initials
            .then(|| initials_codes(remaining, INITIALS_CODE_LIMIT))
            .flatten()
        {
            for mut candidate in dictionary.query_jianpin_codes(&codes, INITIALS_ROW_LIMIT) {
                // 没有锁定的拼音时 `remaining` 就是全部数字，简拼行一个数字一个音节，吃掉全部数字。
                candidate.pinyin = self.digits.clone();
                push_ranked(
                    &mut candidates,
                    &mut leading,
                    candidate,
                    prefer_exact,
                    initials_lead,
                );
            }
        }
        drop(dictionary);
        let filtering = self.single_character || !self.strokes.is_empty();
        if filtering {
            let strokes = (!self.strokes.is_empty()).then_some(&self.stroke_texts);
            candidates.retain(|item| passes_filter(&item.word, self.single_character, strokes));
        }
        rank_candidates(&mut candidates, prefer_exact, initials_lead);
        // emoji、颜文字按拼音查，读法的先后要参照排好的拼音候选，所以在插入英文行之前查；插入在英文行之后，它们也可以接在英文词后面。单字、笔画筛选针对的是汉字，筛选时不混入。
        let (emoji, kaomoji) = if filtering {
            (Vec::new(), Vec::new())
        } else {
            self.expressive_candidates(&alternatives, remaining.len(), &candidates)
        };

        // 没有任何拼音读法时（77 拼不出音节），列表本来是空的，混输开关和最短前缀保护的「拼音列表的可读性」无从谈起；这时照样给英文九键词，否则 QQ 这类词只能切到全键盘去打。
        let unanswered = candidates.is_empty();
        // 首字母和筛选都只针对拼音读法，英文词一概不列。
        let mut english = if initial.is_some() || filtering {
            Vec::new()
        } else {
            self.english_candidates(unanswered)
        };
        if !english.is_empty() {
            // Second place is ahead of every pinyin reading but the first, which is worth it for a word the user is plainly spelling and not for one the frequency table has never seen; a zero-weight word still belongs in the list, at its end (NK:311-318).
            let first = english.remove(0);
            let slot = if first.weight > 0 {
                candidates.len().min(1)
            } else {
                candidates.len()
            };
            candidates.insert(slot, first);
            candidates.extend(english);
        }
        let mut candidates = insert_expressive_rows(candidates, emoji, kaomoji);
        positions::apply_fixed_positions(
            &self.paths.user(assets::USER_JOURNAL),
            &self.ranking_context(),
            &mut candidates,
            false,
            None,
            false,
        );
        // Keep the most likely reading visible without requiring a horizontal scroll.
        if let Some(front) = candidates.first() {
            let offset = if locked_key.is_empty() {
                0
            } else {
                locked_key.len() + 1
            };
            if let Some(rest) = front.canonical_pinyin.get(offset..) {
                let preferred = rest.split('\'').next().unwrap_or_default();
                if let Some(found) = self.spellings.iter().position(|s| s == preferred) {
                    self.spellings[..=found].rotate_right(1);
                }
            }
        }
        self.reading = self.reading_for(candidates.first());
        self.candidates = candidates;
        let remaining = remaining_digits(&self.digits, locked_length);
        if remaining.is_empty() {
            // 数字全部锁定时，左列还是最后一次锁定时的选项，可以换选。
            if let Some(Some(undo)) = self.lock_undo.last() {
                self.spellings = undo.choices.clone();
            }
        } else {
            let choices = self.key_choices(remaining);
            self.spellings.extend(choices);
        }
    }

    /// 按数字串可能的读法查要混入的 emoji 和颜文字，开关都关、数字不足 `MIXED_EXPRESSIVE_MINIMUM_INPUT` 个时不查。
    ///
    /// 读法是锁定的拼写加上一条拼满其余数字的切分路径（`alternatives` 已经按切分和左列选的首字母筛过），只保留各个字母；只拼了开头一个音节、供部分选择用的路径不算，它对应的不是整串数字。读法按支持它的拼音候选在列表里的位置排先后（候选的全拼以这个读法开头），没有候选支持的按路径原来的先后排在后面，只查前 `EXPRESSIVE_READING_LIMIT` 种。命中的编码还要能在已确定的音节边界（锁定拼写的结尾、用户打的切分）处切成完整音节，见 `splits_into_syllables_at`。
    fn expressive_candidates(
        &self,
        alternatives: &[Path],
        remaining: usize,
        candidates: &[WordItem],
    ) -> (Vec<ExpressiveRow>, Vec<ExpressiveRow>) {
        let enabled = self.expressive.emoji_candidates || self.expressive.kaomoji_candidates;
        if !enabled || self.digits.len() < MIXED_EXPRESSIVE_MINIMUM_INPUT {
            return (Vec::new(), Vec::new());
        }
        let readings = self.expressive_readings(alternatives, remaining, candidates);
        let mut boundaries: Vec<usize> = self
            .locked
            .iter()
            .scan(0, |end, spelling| {
                *end += spelling.len();
                Some(*end)
            })
            .chain(self.splits.iter().copied())
            .collect();
        boundaries.sort_unstable();
        boundaries.dedup();
        let accept = |key: &str| splits_into_syllables_at(key, &boundaries);
        let others = self.paths.resource(assets::OTHER_DICTIONARY);
        // 与 26 键相同，资源打不开或查询失败时只是没有这些行，不报诊断。
        let mut emoji = if self.expressive.emoji_candidates {
            query_emoji_readings(&readings, &others, &accept)
        } else {
            Vec::new()
        };
        let mut kaomoji = if self.expressive.kaomoji_candidates {
            query_kaomoji_readings(&readings, &others, &accept)
        } else {
            Vec::new()
        };
        // 选中后吃掉全部数字，与覆盖整串数字的拼音候选一样结束组字。
        for row in emoji.iter_mut().chain(kaomoji.iter_mut()) {
            row.item.pinyin.clone_from(&self.digits);
        }
        (emoji, kaomoji)
    }

    /// 见 `expressive_candidates`。
    fn expressive_readings(
        &self,
        alternatives: &[Path],
        remaining: usize,
        candidates: &[WordItem],
    ) -> Vec<String> {
        let locked = self.locked.concat();
        let mut readings: Vec<String> = Vec::new();
        for path in alternatives {
            if path.iter().map(String::len).sum::<usize>() != remaining {
                continue;
            }
            let mut reading = String::with_capacity(self.digits.len());
            reading.push_str(&locked);
            path.iter().for_each(|piece| reading.push_str(piece));
            if !readings.contains(&reading) {
                readings.push(reading);
            }
        }
        let support = |reading: &String| {
            candidates
                .iter()
                .position(|item| {
                    !item.fuzzy
                        && item.pinyin.len() == self.digits.len()
                        && letters_start_with(&item.canonical_pinyin, reading)
                })
                .unwrap_or(usize::MAX)
        };
        // 稳定排序：同样的支持位置保留路径原来的先后。
        readings.sort_by_cached_key(support);
        readings.truncate(EXPRESSIVE_READING_LIMIT);
        readings
    }

    /// 左列末尾的按键选项：下一个数字键上能起头一个音节的字母（大写，和同形的音节 `o`、`a`、`e` 区分开），再是这个数字本身。数字只在没有锁定时给：前面锁定的音节还没上屏。
    fn key_choices(&self, remaining: &str) -> Vec<String> {
        let Some(&digit) = remaining.as_bytes().first() else {
            return Vec::new();
        };
        let table = spelling_table();
        let mut choices: Vec<String> = letters_for_digit(digit)
            .bytes()
            .filter(|&letter| table.starts_syllable(letter))
            .map(|letter| char::from(letter.to_ascii_uppercase()).to_string())
            .collect();
        if self.locked.is_empty() {
            choices.push(char::from(digit).to_string());
        }
        choices
    }

    /// 按单字和笔画筛选候选，作用到组字结束。笔画是 `hspnz` 组成的笔顺前缀，空表示不按笔画筛选；比较的是候选的第一个字。笔画字典打不开时报 `LANGUAGE_DICTIONARY_UNAVAILABLE`，筛选保持原样。英文九键和没有组字时不处理。
    pub fn set_filter(&mut self, single_character: bool, strokes: &str) -> KeyResult {
        if !self.active() || self.english_only || !self.pinyin {
            return KeyResult::unhandled();
        }
        if strokes.len() > stroke::MAX_STROKES || !strokes.bytes().all(stroke::is_stroke) {
            return KeyResult::unhandled();
        }
        if !strokes.is_empty() && strokes != self.strokes {
            let Some(texts) = self.stroke_texts_for(strokes) else {
                return KeyResult::handled().with_diagnostic(Some(
                    diagnostics::LANGUAGE_DICTIONARY_UNAVAILABLE.to_string(),
                ));
            };
            self.stroke_texts = texts;
        }
        if strokes.is_empty() {
            self.stroke_texts.clear();
        }
        self.single_character = single_character;
        self.strokes = strokes.to_string();
        self.refresh();
        KeyResult::handled()
    }

    fn stroke_texts_for(&mut self, strokes: &str) -> Option<HashSet<char>> {
        if self.stroke.is_none() {
            self.stroke = language_dictionary::open_read_only(&self.stroke_dictionary).ok();
        }
        let texts = self.stroke.as_ref()?.texts_with_key_prefix(strokes).ok()?;
        Some(
            texts
                .iter()
                .filter_map(|text| {
                    let mut characters = text.chars();
                    let first = characters.next()?;
                    characters.next().is_none().then_some(first)
                })
                .collect(),
        )
    }

    /// The leading row's pinyin cut to the digits it covers, then the uncovered digits with their splits: `xi'an` for 西安 over `94'26`, `yi'c` for 遗产 over `942`. Empty when an English word or nothing leads.
    fn reading_for(&self, front: Option<&WordItem>) -> String {
        let Some(front) = front else {
            return String::new();
        };
        // A fuzzy row was matched through another spelling (知 under `94` as zi), so its canonical letters do not line up one per typed digit.
        if front.fuzzy {
            return String::new();
        }
        if front.pinyin.is_empty() || !front.pinyin.bytes().all(|byte| byte.is_ascii_digit()) {
            return String::new();
        }
        // 简拼行一个数字一个音节，读音行显示每个音节的首字母（`m't`），而不是全拼的前几个字母（`me`）。
        if is_initials_row(front) {
            return front
                .canonical_pinyin
                .split('\'')
                .filter_map(|syllable| syllable.get(..1))
                .collect::<Vec<_>>()
                .join("'");
        }
        let covered = front.pinyin.len();
        let mut reading = String::new();
        let mut letters = 0;
        for character in front.canonical_pinyin.chars() {
            if letters == covered {
                break;
            }
            if character.is_ascii_lowercase() {
                letters += 1;
            }
            reading.push(character);
        }
        let reading = reading.trim_end_matches('\'').to_string();
        if reading.is_empty() {
            return String::new();
        }
        let mut reading = reading;
        let mut start = covered.min(self.digits.len());
        if start < self.digits.len() {
            reading.push('\'');
            for &split in &self.splits {
                if split > start && split < self.digits.len() {
                    reading.push_str(&self.digits[start..split]);
                    reading.push('\'');
                    start = split;
                }
            }
            reading.push_str(&self.digits[start..]);
        }
        reading
    }

    /// `unanswered`：拼音一行候选都没有，英文词就是整个答案，和英文模式一样不受混输开关和最短前缀限制；锁定了拼音时仍不给。
    fn english_candidates(&mut self, unanswered: bool) -> Vec<WordItem> {
        // In English-only mode the words are the whole answer, so neither the mixed-candidate setting nor the prefix length that keeps a mixed list readable applies: one digit already narrows the alphabet enough (NK:169-178).
        if self.english_only {
            if self.digits.is_empty() {
                return Vec::new();
            }
        } else if unanswered {
            if self.digits.is_empty() || !self.locked.is_empty() {
                return Vec::new();
            }
        } else if !self.english_options.mixed_candidates
            || !self.locked.is_empty()
            || self.digits.len() < self.english_options.minimum_prefix
        {
            return Vec::new();
        }
        let digits = self.digits.clone();
        let Some(english) = self.open_english() else {
            return Vec::new();
        };
        let prefixes = letter_prefixes(&digits, ENGLISH_PREFIX_BUDGET);
        let capacity = prefixes.len().saturating_mul(ENGLISH_LIMIT);
        let mut words = Vec::with_capacity(capacity);
        for prefix in prefixes {
            for word in english.query_prefix(&prefix, ENGLISH_LIMIT) {
                // Only a whole code that starts with the digits counts; otherwise letters beyond the expanded prefix leak in.
                // The database lookup key is the lowercase spelling in `pinyin`; `word` is the
                // display form and may intentionally contain punctuation or spaces (for example
                // the custom entry `dont` displayed as `don't`).
                if !word_matches_digits(&word.pinyin, &digits) {
                    continue;
                }
                words.push(word);
            }
        }
        deduplicate_english_words(&mut words);
        rank_english(&mut words, digits.len());
        words
    }

    /// `msime-english.db` is optional; a missing file means no English rows and is looked for again on the next refresh (NK:179-186).
    fn open_english(&mut self) -> Option<&EnglishDictionary> {
        if self.english.is_none() {
            let path = self.paths.dictionary(assets::ENGLISH_DICTIONARY);
            if !path.exists() {
                return None;
            }
            self.english = Some(EnglishDictionary::open(&path, None, None));
        }
        self.english.as_ref()
    }

    fn locked_length(&self) -> usize {
        self.locked.iter().map(String::len).sum()
    }

    /// Drop the digits a selection consumed and every locked syllable they fully cover (NK:355-361). An English row's `pinyin` is the word, which can be longer than what was typed.
    fn consume(&mut self, count: usize) {
        let count = count.min(self.digits.len());
        self.digits.drain(..count);
        // 与全拼键盘相同（`session/commit.rs` 的 `commit`）：选中一行后光标回到末尾。改完中间的数字选掉前一个词，接着打的是下一个词，应接在剩下的数字后面，而不是插在它们中间。
        self.caret = None;
        self.splits.retain_mut(|split| {
            if *split > count {
                *split -= count;
                true
            } else {
                false
            }
        });
        let mut consumed = count;
        while let Some(front) = self.locked.first() {
            if consumed < front.len() {
                break;
            }
            consumed -= front.len();
            self.locked.remove(0);
            if !self.lock_undo.is_empty() {
                self.lock_undo.remove(0);
            }
        }
        // 剩下的锁定记着的是上屏前的数字位置。
        self.lock_undo.iter_mut().for_each(|undo| *undo = None);
        self.initial = None;
    }

    fn ranking_context(&self) -> String {
        format!("nine-key:{}:{}", self.digits, self.locked.join("'"))
    }

    fn editable(&self, index: usize) -> bool {
        self.candidates
            .get(index)
            .is_some_and(|item| !item.canonical_pinyin.is_empty() && item.source.is_dictionary())
    }

    /// Learning and explicit management use canonical dictionary keys, never digit strings.
    fn adjust_frequency(&mut self, index: usize, force_top: bool) -> Option<String> {
        let item = &self.candidates[index];
        let main_db = self.paths.dictionary(assets::MAIN_DICTIONARY);
        let user_db = self.paths.user(assets::USER_JOURNAL);
        let context = self.ranking_context();
        let request = RankingRequest {
            main_db: &main_db,
            user_db: &user_db,
            context_key: &context,
            ordered: &self.candidates,
            entry_key: &item.canonical_pinyin,
            value: &item.word,
            mode: if force_top {
                FrequencyAdjustmentMode::Pin
            } else {
                self.frequency.mode
            },
            linear_step: self.frequency.linear_step,
            trigger_count: self.frequency.trigger_count,
            force_top,
            kind: PersonalDictionaryKind::Pinyin,
        };
        match ranking::adjust_candidate_ranking(&request) {
            Ok(changed) => {
                if changed {
                    if let Some(dictionary) = self.dictionary.as_mut() {
                        dictionary.reset_cache();
                    }
                }
                None
            }
            // A failed write keeps the commit and reports this one message (NK:409-414).
            Err(_) => Some(diagnostics::NINE_KEY_FREQUENCY_NOT_PERSISTED.to_string()),
        }
    }
}

fn remaining_digits(digits: &str, locked_length: usize) -> &str {
    &digits[locked_length..]
}

fn append_path_key(output: &mut String, prefix: &str, path: &[String]) {
    output.clear();
    output.push_str(prefix);
    for (index, part) in path.iter().enumerate() {
        if !prefix.is_empty() || index != 0 {
            output.push('\'');
        }
        output.push_str(part);
    }
}

/// A row read under the locked syllables must spell them, or be a whole-syllable prefix of them.
fn agrees_with_locked(matched: &str, locked_key: &str) -> bool {
    if locked_key.is_empty() || matched == locked_key {
        return true;
    }
    let under = matched
        .strip_prefix(locked_key)
        .is_some_and(|rest| rest.starts_with('\''));
    let over = locked_key
        .strip_prefix(matched)
        .is_some_and(|rest| rest.starts_with('\''));
    under || over
}

/// 去掉 `'` 之后，`pinyin` 的字母以 `reading` 开头。
fn letters_start_with(pinyin: &str, reading: &str) -> bool {
    let mut letters = pinyin.bytes().filter(|&byte| byte != b'\'');
    reading.bytes().all(|byte| letters.next() == Some(byte))
}

/// emoji、颜文字的编码（全拼连写，没有 `'`）能不能让 `boundaries`（字母位置，升序）都落在音节之间：各边界之间、以及最后一个边界之后的部分都能切成完整音节。最后一个边界正好在编码末尾时，后面什么都没有也算。没有边界时都算，与 26 键按前缀匹配相同。
///
/// 编码本身不记音节在哪里分开，`xiangjiao` 也能切成 xi'ang'jiao，所以锁定 xi 之后它仍然算；这里排除的是怎么切都对不上的编码，比如锁定 xian 之后的 `xiangjiao`（剩下的 gjiao 切不成音节）。
fn splits_into_syllables_at(key: &str, boundaries: &[usize]) -> bool {
    let mut start = 0;
    for &end in boundaries.iter().chain(std::iter::once(&key.len())) {
        let Some(piece) = key.get(start..end) else {
            return false;
        };
        if !piece.is_empty() && cut_one_piece_min_segments(piece, true).is_empty() {
            return false;
        }
        start = end;
    }
    true
}

/// 候选能否通过筛选：`single_character` 只留单字，`strokes` 是笔顺以所选几笔开头的字，比较候选的第一个字。
fn passes_filter(word: &str, single_character: bool, strokes: Option<&HashSet<char>>) -> bool {
    let mut characters = word.chars();
    let Some(first) = characters.next() else {
        return false;
    };
    if single_character && characters.next().is_some() {
        return false;
    }
    strokes.is_none_or(|texts| texts.contains(&first))
}

#[cfg(test)]
fn has_candidate_word(candidates: &[WordItem], word: &str) -> bool {
    candidates.iter().any(|candidate| candidate.word == word)
}

fn query_key_is_new(
    alternatives: &[Path],
    index: usize,
    key: &str,
    queried: Option<&HashSet<String>>,
) -> bool {
    match queried {
        Some(queried) => !queried.contains(key),
        None => !alternatives[..index]
            .iter()
            .any(|alternative| alternative == &alternatives[index]),
    }
}

/// `rank_candidates` 的排序键，小的在前。
type RankKey = (Reverse<usize>, bool, bool, bool, bool, Reverse<i64>);

/// More digits covered first. Synthesised rows (whole-sentence Generated, Fallback) score on a different scale from dictionary weights, so within one coverage bucket dictionary rows lead; then exact before fuzzy, then weight.
/// With `prefer_exact` (the user typed a split), a row the typed digits spell to its end then leads one that has to be completed past them: over `94'26` 西安 (xi'an) comes before 自从 (zi'cong), however common the longer word. Without a split the digits do not say where a syllable ends, so `3` keeps 的 (de) ahead of the rarer 额 (e) by weight.
/// 简拼行（`is_initials_row`）在同样覆盖的词典行里的位置由 `initials_lead` 决定：用户在每个数字之间都打了切分时排在前面，否则先排在音节行之后、整句行之前，去重后再由 `interleave_initials` 按权重插进音节行里。
fn rank_key(item: &WordItem, prefer_exact: bool, initials_lead: bool) -> RankKey {
    let completion = prefer_exact
        && item
            .canonical_pinyin
            .bytes()
            .filter(u8::is_ascii_lowercase)
            .count()
            > item.pinyin.len();
    let initials = is_initials_row(item);
    (
        Reverse(item.pinyin.len()),
        item.source.is_generated_or_fallback(),
        initials != initials_lead,
        completion,
        item.fuzzy,
        Reverse(comparable_weight(item)),
    )
}

/// 整句行的分是 `log_prob * 1000`，词组边每覆盖一个音节就带一份 `PHRASE_LENGTH_BONUS`。同一串音节里比较时这无关紧要，九宫格却要比较不同音节串各自解出的整句：音节越多、越能拼成词组的读法分越高，`9436364782662` 的 xi'e'meng'suan'ma（洗噩梦算吗）就会压过 zhe'neng'suan'ma（这能算吗）。这里按路径的音节数每个扣一份奖励：同一条路径内的先后不变，多切出来的音节要靠词本身的分量才能赢。只扣词组实际拿到的那部分反而更差（整句集 top-1 降了一截），见 `.agents/notes/implemented/bug-fix/2026-10-08-nine-key-path-ranking.md`。
fn comparable_weight(item: &WordItem) -> i64 {
    if item.source != CandidateSource::Generated || item.canonical_pinyin.is_empty() {
        return item.weight;
    }
    let syllables = item.canonical_pinyin.split('\'').count() as f64;
    item.weight - (PHRASE_LENGTH_BONUS * 1000.0 * syllables) as i64
}

/// 一个数字一个音节、全拼比数字长的词典行：按简拼查出来的行（`68` 的 明天 mei'tian）。只有两个及以上数字时才算，一个数字本来就按首字母补全。
fn is_initials_row(item: &WordItem) -> bool {
    let digits = item.pinyin.len();
    digits >= 2
        && !item.source.is_generated_or_fallback()
        && item.canonical_pinyin.split('\'').count() == digits
        && item
            .canonical_pinyin
            .bytes()
            .filter(u8::is_ascii_lowercase)
            .count()
            > digits
}

/// 简拼只在每个数字都能是一个音节时才查：至少两个数字，用户打的切分（`splits`，相对未锁定的数字）把数字切成的每一段都只有一个数字。末尾的切分也是一个音节的结尾：`68'` 说 68 是一个音节，不是两个首字母。
fn initials_apply(digits: usize, splits: &[usize]) -> bool {
    if digits < 2 {
        return false;
    }
    let mut start = 0;
    for &split in splits {
        if split - start > 1 {
            return false;
        }
        start = split;
    }
    splits.is_empty() || digits - start <= 1
}

/// 数字键上能作音节首字母的字母：i、u、v 不起头任何音节，词库也没有以它们命名的表，所以 4 只有 g、h，8 只有 t。
fn initials_for_digit(digit: u8) -> impl Iterator<Item = u8> {
    letters_for_digit(digit)
        .bytes()
        .filter(|letter| !matches!(letter, b'i' | b'u' | b'v'))
}

/// 每个数字当一个音节的首字母时能拼出的全部简拼（`68` → mt、nt、ot）；多于 `limit` 个，或有数字不是 2-9 时为 `None`。
fn initials_codes(digits: &str, limit: usize) -> Option<Vec<String>> {
    let mut codes = vec![String::with_capacity(digits.len())];
    for digit in digits.bytes() {
        let count = initials_for_digit(digit).count();
        if count == 0 || codes.len().saturating_mul(count) > limit {
            return None;
        }
        codes = codes
            .iter()
            .flat_map(|code| {
                initials_for_digit(digit).map(move |letter| {
                    let mut next = String::with_capacity(digits.len());
                    next.push_str(code);
                    next.push(char::from(letter));
                    next
                })
            })
            .collect();
    }
    Some(codes)
}

fn take_candidate_canonical_pinyin(candidate: &mut WordItem) -> String {
    if candidate.canonical_pinyin.is_empty() {
        candidate.pinyin.clone()
    } else {
        std::mem::take(&mut candidate.canonical_pinyin)
    }
}

/// 英文九键前缀展开最多产生 320 行；用栈上借用表和索引表去重，释放借用后再原地压缩。
fn deduplicate_english_words(words: &mut Vec<WordItem>) {
    assert!(words.len() <= ENGLISH_CANDIDATE_CAPACITY);
    let mut duplicates = [0usize; ENGLISH_CANDIDATE_CAPACITY];
    let mut duplicate_len = 0;
    {
        let mut seen: [Option<&str>; ENGLISH_CANDIDATE_CAPACITY] =
            [None; ENGLISH_CANDIDATE_CAPACITY];
        let mut seen_len = 0;
        for (index, word) in words.iter().enumerate() {
            if seen[..seen_len]
                .iter()
                .flatten()
                .any(|existing| *existing == word.word.as_str())
            {
                duplicates[duplicate_len] = index;
                duplicate_len += 1;
            } else {
                seen[seen_len] = Some(word.word.as_str());
                seen_len += 1;
            }
        }
    }
    let mut next_duplicate = 0;
    let mut write = 0;
    for read in 0..words.len() {
        if next_duplicate < duplicate_len && duplicates[next_duplicate] == read {
            next_duplicate += 1;
            continue;
        }
        if write != read {
            words.swap(write, read);
        }
        write += 1;
    }
    words.truncate(write);
}

/// 推入一行，除非同一个词已有一行排得不比它靠后。`leading` 记着每个词目前排得最靠前的那一行的排序键。被跳过的行在稳定排序后必然落在那一行之后（键更大，或键相同而推入更晚），会被 `retain_unique_words` 删掉；它也不会让别的行多删或少删，因为它能挡住的行那一行同样挡得住。所以跳过与全部推入再 `rank_candidates`，结果完全相同。前提是两边用同一个 `prefer_exact`。
fn push_ranked(
    candidates: &mut Vec<WordItem>,
    leading: &mut HashMap<String, RankKey>,
    candidate: WordItem,
    prefer_exact: bool,
    initials_lead: bool,
) {
    let key = rank_key(&candidate, prefer_exact, initials_lead);
    match leading.get_mut(candidate.word.as_str()) {
        Some(best) if *best <= key => return,
        Some(best) => *best = key,
        None => {
            leading.insert(candidate.word.clone(), key);
        }
    }
    candidates.push(candidate);
}

/// Stable sort by `rank_key`, dedup by word, capped (NK:283-307).
fn rank_candidates(candidates: &mut Vec<WordItem>, prefer_exact: bool, initials_lead: bool) {
    candidates.sort_by_key(|item| rank_key(item, prefer_exact, initials_lead));
    retain_unique_words(candidates);
    if !initials_lead {
        interleave_initials(candidates);
    }
    candidates.truncate(CANDIDATE_LIMIT);
}

/// 没打切分时，`rank_key` 把简拼行排在同样覆盖的全部音节行之后。两位数字的音节行常有几百行（`68` 的 mu、nu、nv、ou 在出货词库里有两百多个单字），截到 `CANDIDATE_LIMIT` 时简拼行会整个被截掉，明天、今天就再也出不来（#5640）。所以在截断前，每段同样覆盖的词典行里先留下最前面 `SYLLABLE_ROWS_BEFORE_INITIALS` 个音节行，其后的音节行和简拼行按权重归并：常用词排在生僻单字前面，常用单字仍排在少见的词前面。两边各自的先后不变，权重相同时音节行在前。只重排去重之后的列表，保留哪一行不受影响，`push_ranked` 的跳过规则照样成立。
fn interleave_initials(candidates: &mut Vec<WordItem>) {
    let mut start = 0;
    while start < candidates.len() {
        let coverage = candidates[start].pinyin.len();
        let generated = candidates[start].source.is_generated_or_fallback();
        let end = start
            + candidates[start..]
                .iter()
                .take_while(|item| {
                    item.pinyin.len() == coverage
                        && item.source.is_generated_or_fallback() == generated
                })
                .count();
        if !generated {
            // 这一段里音节行在前、简拼行在后（`rank_key` 的顺序）。
            let middle = start
                + candidates[start..end]
                    .iter()
                    .take_while(|item| !is_initials_row(item))
                    .count();
            let kept = start + SYLLABLE_ROWS_BEFORE_INITIALS;
            if kept < middle && middle < end {
                let mut syllables: Vec<WordItem> = candidates.drain(kept..end).collect();
                let initials = syllables.split_off(middle - kept);
                let mut syllables = syllables.into_iter().peekable();
                let mut initials = initials.into_iter().peekable();
                let mut merged = Vec::with_capacity(end - kept);
                loop {
                    let take_initial = match (syllables.peek(), initials.peek()) {
                        (Some(syllable), Some(initial)) => initial.weight > syllable.weight,
                        (None, Some(_)) => true,
                        (_, None) => false,
                    };
                    let next = if take_initial {
                        initials.next()
                    } else {
                        syllables.next()
                    };
                    match next {
                        Some(item) => merged.push(item),
                        None => break,
                    }
                }
                candidates.splice(kept..kept, merged);
            }
        }
        start = end;
    }
}

fn retain_unique_words(candidates: &mut Vec<WordItem>) {
    if candidates.len() <= SMALL_CANDIDATE_DEDUP {
        let mut write = 0;
        for read in 0..candidates.len() {
            if candidates[..write]
                .iter()
                .any(|existing| existing.word == candidates[read].word)
            {
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
        .filter_map(|(index, item)| (!seen.insert(item.word.as_str())).then_some(index))
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

/// A word spelling the typed code exactly leads, but only when people type it: 64426 is 你好 and `ogham` is the only five-letter word those keys spell, so a zero-weight exact word gets no privilege. Then weight, then shorter (NK:199-220).
fn rank_english(words: &mut Vec<WordItem>, typed: usize) {
    let exact = |item: &WordItem| item.word.len() == typed && item.weight > 0;
    words.sort_by(|a, b| {
        exact(b)
            .cmp(&exact(a))
            .then_with(|| b.weight.cmp(&a.weight))
            .then_with(|| a.word.len().cmp(&b.word.len()))
    });
    words.truncate(ENGLISH_LIMIT);
}

/// The digit code of a pinyin spelling; anything but `a..=z` (apostrophes) is skipped.
fn encode(pinyin: &str) -> String {
    pinyin
        .bytes()
        .filter(u8::is_ascii_lowercase)
        .map(|letter| char::from(KEYPAD[usize::from(letter - b'a')]))
        .collect()
}

/// The digit code of an English word, ignoring case; a word with any non-letter has none.
#[cfg(test)]
fn digits_for_word(word: &str) -> String {
    let lowered = word.to_ascii_lowercase();
    if !lowered.bytes().all(|byte| byte.is_ascii_lowercase()) {
        return String::new();
    }
    encode(&lowered)
}

fn word_matches_digits(word: &str, digits: &str) -> bool {
    let mut matched = 0;
    for byte in word.bytes() {
        let letter = byte.to_ascii_lowercase();
        if !letter.is_ascii_lowercase() {
            return false;
        }
        if let Some(&digit) = digits.as_bytes().get(matched) {
            if KEYPAD[usize::from(letter - b'a')] != digit {
                return false;
            }
            matched += 1;
        }
    }
    matched == digits.len()
}

fn letters_for_digit(digit: u8) -> &'static str {
    if (b'2'..=b'9').contains(&digit) {
        DIGIT_LETTERS[usize::from(digit - b'0')]
    } else {
        ""
    }
}

/// Every letter prefix of the leading digits, expanded while the count stays within `budget`.
fn letter_prefixes(digits: &str, budget: usize) -> Vec<String> {
    let mut prefixes = vec![String::new()];
    for digit in digits.bytes() {
        let letters = letters_for_digit(digit);
        if letters.is_empty() || prefixes.len() * letters.len() > budget {
            break;
        }
        prefixes = prefixes
            .iter()
            .flat_map(|prefix| {
                letters
                    .chars()
                    .map(move |letter| format!("{prefix}{letter}"))
            })
            .collect();
    }
    if prefixes.len() == 1 && prefixes[0].is_empty() {
        Vec::new()
    } else {
        prefixes
    }
}

/// The syllables and spelling pieces digits can stand for, built once from the intact syllable list.
struct SpellingTable {
    /// Every complete syllable with its code, in list order.
    syllables: Vec<(String, String)>,
    intact: HashSet<String>,
    /// Every prefix of every syllable, grouped by its code. Partial syllables are included, but never letter combinations pinyin does not have (NK:52-68).
    by_code: HashMap<String, Vec<String>>,
    longest_code: usize,
}

fn spelling_table() -> &'static SpellingTable {
    static TABLE: OnceLock<SpellingTable> = OnceLock::new();
    TABLE.get_or_init(|| SpellingTable::new(intact_pinyin_list()))
}

impl SpellingTable {
    fn new(syllables: &[&str]) -> Self {
        let prefix_capacity = syllables.iter().map(|syllable| syllable.len()).sum();
        let mut seen = HashSet::with_capacity(prefix_capacity);
        let mut by_code: HashMap<String, Vec<String>> = HashMap::with_capacity(prefix_capacity);
        let mut longest_code = 0;
        for syllable in syllables {
            for end in 1..=syllable.len() {
                let Some(prefix) = syllable.get(..end) else {
                    continue;
                };
                if seen.insert(prefix.to_string()) {
                    let code = encode(prefix);
                    longest_code = longest_code.max(code.len());
                    by_code.entry(code).or_default().push(prefix.to_string());
                }
            }
        }
        Self {
            syllables: syllables
                .iter()
                .map(|syllable| (syllable.to_string(), encode(syllable)))
                .collect(),
            intact: syllables
                .iter()
                .map(|syllable| syllable.to_string())
                .collect(),
            by_code,
            longest_code,
        }
    }

    /// 有音节以这个字母开头。`i`、`u`、`v` 没有。
    fn starts_syllable(&self, letter: u8) -> bool {
        self.syllables
            .iter()
            .any(|(syllable, _)| syllable.as_bytes().first() == Some(&letter))
    }

    /// Complete syllables the unlocked digits can start with, or that complete them, longest covered first (NK:238-250). Coverage is counted in digits: comparing letter counts would put a syllable that needs two digits ahead under the same digit prefix.
    /// 有切分时，只有在切分处或之前结束的音节才算。
    fn spellings_for(
        &self,
        remaining: &str,
        locked_length: usize,
        split: Option<usize>,
    ) -> Vec<String> {
        if remaining.is_empty() {
            return Vec::new();
        }
        let mut matches: Vec<&(String, String)> = self
            .syllables
            .iter()
            .filter(|(_, code)| match split {
                Some(split) => code.len() <= split && remaining.starts_with(code.as_str()),
                None => {
                    locked_length + remaining.len().max(code.len()) <= DIGIT_LIMIT
                        && (remaining.starts_with(code.as_str()) || code.starts_with(remaining))
                }
            })
            .collect();
        let covered = |code: &str| code.len().min(remaining.len());
        matches.sort_by(|(a, a_code), (b, b_code)| {
            covered(b_code).cmp(&covered(a_code)).then_with(|| a.cmp(b))
        });
        matches.into_iter().map(|(text, _)| text.clone()).collect()
    }

    /// Syllable paths spelling `digits`, built from the end. A piece may end a path early only if it is a complete syllable; each offset keeps the 48 best by fewer syllables, complete last syllable, then lexicographic (NK:122-156). 不带频度，测试用。
    #[cfg(test)]
    fn paths(&self, digits: &str) -> Vec<Path> {
        self.split_paths(digits, &[], &SyllablePrior::default())
    }

    /// 和 `paths` 一样，但每个 `splits` 位置（`digits` 里的下标）都必须有一个音节在那里结束，而且这个音节必须完整；同样音节数、同样收尾的路径按 `prior` 给的频度之和排，频度相同才按字典序。
    ///
    /// 每个位置只留 48 条，留哪些就决定了哪些读法还能查到。原先在字典序上截断，而 9 键是 wxyz、7 键是 pqrs，于是长串里 xi 开头的路径把 yi、zhe 开头的全部挤掉，`943426943426` 只剩「洗点一点」查不到「一点一点」，`7264685487` 只剩「盘后」查不到「然后」。
    fn split_paths(&self, digits: &str, splits: &[usize], prior: &SyllablePrior) -> Vec<Path> {
        let length = digits.len();
        let mut suffix: Vec<Vec<(f64, Path)>> = vec![Vec::new(); length + 1];
        suffix[length].push((0.0, Vec::new()));
        for offset in (0..length).rev() {
            let mut result = Vec::with_capacity(PATH_LIMIT);
            for end in offset + 1..=length.min(offset + self.longest_code) {
                if splits.iter().any(|&split| offset < split && split < end) {
                    break;
                }
                let Some(pieces) = self.by_code.get(&digits[offset..end]) else {
                    continue;
                };
                let must_complete = end != length || splits.contains(&end);
                for piece in pieces {
                    if must_complete && !self.intact.contains(piece) {
                        continue;
                    }
                    let score = prior.score(piece);
                    for (tail_score, tail) in &suffix[end] {
                        let mut path = Vec::with_capacity(tail.len() + 1);
                        path.push(piece.clone());
                        path.extend(tail.iter().cloned());
                        result.push((score + tail_score, path));
                    }
                }
            }
            // Pieces are visited grouped by code rather than in list order; the order below is total on distinct paths, so the kept 48 are the same either way.
            let complete = |path: &Path| path.last().is_some_and(|last| self.intact.contains(last));
            result.sort_by(|(a_score, a), (b_score, b)| {
                a.len()
                    .cmp(&b.len())
                    .then_with(|| complete(b).cmp(&complete(a)))
                    .then_with(|| b_score.total_cmp(a_score))
                    .then_with(|| a.cmp(b))
            });
            result.truncate(PATH_LIMIT);
            suffix[offset] = result;
        }
        suffix
            .swap_remove(0)
            .into_iter()
            .map(|(_, path)| path)
            .collect()
    }
}

/// 音节和音节前缀的单字频度：音节取词库里它最常用那个字的权重取对数，前缀取以它开头的音节里最高的那个。九宫格只用它决定每个位置留哪 48 条切分路径，候选本身的先后仍由词库和整句解码决定。
#[derive(Default)]
struct SyllablePrior {
    scores: HashMap<String, f64>,
}

impl SyllablePrior {
    fn from_dictionary(dictionary: &QuanpinDictionary, table: &SpellingTable) -> Self {
        let keys: Vec<String> = table
            .syllables
            .iter()
            .map(|(syllable, _)| syllable.clone())
            .collect();
        let weights = dictionary.best_weights(&keys);
        Self::from_weights(&table.syllables, &weights)
    }

    fn from_weights(syllables: &[(String, String)], weights: &HashMap<String, i64>) -> Self {
        let total: f64 = syllables
            .iter()
            .map(|(syllable, _)| weights.get(syllable).copied().unwrap_or(0).max(1) as f64)
            .sum();
        let mut scores: HashMap<String, f64> = HashMap::new();
        for (syllable, _) in syllables {
            // 没有单字行的音节和权重为 0 的一样按 1 算。
            let weight = weights.get(syllable).copied().unwrap_or(0).max(1) as f64;
            let score = (weight / total).ln();
            for end in 1..=syllable.len() {
                let Some(prefix) = syllable.get(..end) else {
                    continue;
                };
                let slot = scores.entry(prefix.to_string()).or_insert(score);
                if *slot < score {
                    *slot = score;
                }
            }
        }
        Self { scores }
    }

    fn score(&self, piece: &str) -> f64 {
        self.scores.get(piece).copied().unwrap_or(0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::CandidateSource;
    use rusqlite::Connection;

    // ---- Pure helpers ----

    #[test]
    fn codes_follow_the_keypad() {
        assert_eq!(encode("ni'hao"), "64426");
        assert_eq!(encode("zhuang"), "948264");
        assert_eq!(digits_for_word("OK"), "65");
        assert_eq!(digits_for_word("ogham"), "64426");
        assert_eq!(digits_for_word("don't"), "");
        assert_eq!(digits_for_word("café"), "");
        assert!(word_matches_digits("OK", "65"));
        assert!(word_matches_digits("ogham", "64426"));
        assert!(!word_matches_digits("ogham", "64427"));
        assert!(!word_matches_digits("don't", "3668"));
        assert!(!word_matches_digits("ogham", "644260"));
    }

    #[test]
    fn initials_codes_take_one_initial_per_digit() {
        assert_eq!(initials_codes("68", 1024).unwrap(), ["mt", "nt", "ot"]);
        // 4 上的 i、8 上的 u 和 v 不起头任何音节。
        assert_eq!(initials_codes("48", 1024).unwrap(), ["gt", "ht"]);
        assert_eq!(initials_codes("2356", 1024).unwrap().len(), 3 * 3 * 3 * 3);
        assert!(initials_codes("2356", 1024)
            .unwrap()
            .contains(&"cflm".to_string()));
        assert_eq!(initials_codes("99999", 1024).unwrap().len(), 1024);
        assert!(initials_codes("999999", 1024).is_none(), "past the limit");
        assert!(initials_codes("61", 1024).is_none(), "1 spells no letter");
    }

    #[test]
    fn initials_apply_only_when_every_digit_can_be_a_syllable() {
        assert!(
            !initials_apply(1, &[]),
            "one digit already completes by its initial"
        );
        assert!(initials_apply(2, &[]));
        assert!(initials_apply(2, &[1]));
        assert!(initials_apply(5, &[1, 2, 3, 4]));
        assert!(
            initials_apply(3, &[1, 2, 3]),
            "a trailing split after one digit"
        );
        assert!(!initials_apply(3, &[2]), "94'2: 94 is one syllable");
        assert!(!initials_apply(2, &[2]), "68': 68 is one syllable");
        assert!(!initials_apply(3, &[1]), "6'84: 84 is one syllable");
    }

    #[test]
    fn letter_prefixes_stop_at_the_budget() {
        assert_eq!(letter_prefixes("6", 64), ["m", "n", "o"]);
        let prefixes = letter_prefixes("64426", 64);
        assert_eq!(prefixes.len(), 27);
        assert_eq!(prefixes.first().map(String::as_str), Some("mgg"));
        assert_eq!(prefixes.last().map(String::as_str), Some("oii"));
        assert_eq!(letter_prefixes("79", 64).len(), 16);
        assert!(letter_prefixes("1", 64).is_empty());
        assert!(letter_prefixes("", 64).is_empty());
    }

    #[test]
    fn paths_prefer_fewer_syllables_then_a_complete_tail() {
        let table = SpellingTable::new(&["ni", "mi", "o", "ma", "hao", "gao"]);
        // Fewer syllables first; an incomplete piece (`g`, `h`) may only end the path.
        assert_eq!(
            table.paths("64"),
            [vec!["mi"], vec!["ni"], vec!["o", "g"], vec!["o", "h"]]
        );
        // `o` is a syllable, `m` and `n` are only pieces of one: a complete tail comes first at equal length.
        assert_eq!(table.paths("6"), [vec!["o"], vec!["m"], vec!["n"]]);
        let paths = table.paths("64426");
        assert_eq!(
            paths[..4],
            [
                vec!["mi", "gao"],
                vec!["mi", "hao"],
                vec!["ni", "gao"],
                vec!["ni", "hao"]
            ]
        );
        // A piece may only end the path: `ni'ha'o` would need `ha`, which is not a syllable here.
        assert_eq!(paths.len(), 4);
        assert!(table.paths("11").is_empty());
    }

    #[test]
    fn paths_keep_at_most_the_limit_per_offset() {
        let table = SpellingTable::new(&["m", "n", "o"]);
        let paths = table.paths("66666");
        assert_eq!(paths.len(), PATH_LIMIT);
        assert_eq!(paths[0], ["m", "m", "m", "m", "m"]);
        assert!(paths.iter().all(|path| path.len() == 5));
    }

    #[test]
    fn the_limit_keeps_frequent_syllables_rather_than_early_letters() {
        let table = SpellingTable::new(&["xi", "yi", "dian"]);
        let weights = HashMap::from([
            ("xi".to_string(), 1_000),
            ("yi".to_string(), 1_000_000),
            ("dian".to_string(), 10_000),
        ]);
        let prior = SyllablePrior::from_weights(&table.syllables, &weights);
        // 94 3426 重复七次，每个位置都要截到 48 条：按字典序截断时第一个音节全是 xi，一点一点……不在里面。
        let digits = "943426".repeat(7);
        let paths = table.split_paths(&digits, &[], &prior);
        assert_eq!(paths.len(), PATH_LIMIT);
        assert_eq!(paths[0], ["yi", "dian"].repeat(7));
        assert!(table
            .split_paths(&digits, &[], &SyllablePrior::default())
            .iter()
            .all(|path| path[0] == "xi"));
    }

    #[test]
    fn a_prefix_scores_as_its_best_syllable() {
        let table = SpellingTable::new(&["shi", "si", "ran"]);
        let weights = HashMap::from([("shi".to_string(), 5_000), ("si".to_string(), 50)]);
        let prior = SyllablePrior::from_weights(&table.syllables, &weights);
        // 分母是各音节权重之和，没有单字行的 ran 按 1 算：5000 + 50 + 1。
        let total = 5_051f64;
        assert_eq!(prior.score("s"), (5_000f64 / total).ln());
        assert_eq!(prior.score("sh"), (5_000f64 / total).ln());
        assert_eq!(prior.score("si"), (50f64 / total).ln());
        assert_eq!(prior.score("ran"), (1f64 / total).ln());
    }

    #[test]
    fn sentences_from_different_paths_compare_without_the_phrase_bonus() {
        let bonus = (PHRASE_LENGTH_BONUS * 1000.0) as i64;
        // 五个音节的读法每个音节都在词组里，多拿一份奖励；扣掉之后四个音节的读法分更高就该排在前面。
        let mut longer = item(
            "洗噩梦算吗",
            "9436364782662",
            5 * bonus - 30_000,
            CandidateSource::Generated,
        );
        longer.canonical_pinyin = "xi'e'meng'suan'ma".into();
        let mut shorter = item(
            "这能算吗",
            "9436364782662",
            4 * bonus - 20_000,
            CandidateSource::Generated,
        );
        shorter.canonical_pinyin = "zhe'neng'suan'ma".into();
        assert!(longer.weight > shorter.weight);
        assert_eq!(
            ranked(vec![longer, shorter], false),
            ["这能算吗", "洗噩梦算吗"]
        );
        // 词库行不带这份奖励，按原权重比。
        let mut word = item("西安", "9426", 500, CandidateSource::Database);
        word.canonical_pinyin = "xi'an".into();
        assert_eq!(comparable_weight(&word), 500);
    }

    #[test]
    fn query_key_scan_uses_no_temporary_heap_state() {
        let alternatives = vec![vec!["ni".to_owned(), "hao".to_owned()]];
        let (found, allocations) = crate::ime::personal_rerank::allocations::count(|| {
            query_key_is_new(&alternatives, 0, "ni'hao", None)
        });

        assert!(found);
        assert_eq!(allocations, 0);
    }

    #[test]
    fn spellings_sort_by_digits_covered_within_the_limit() {
        let table = SpellingTable::new(&[
            "ga", "gan", "gang", "gao", "ha", "han", "hang", "hao", "ni", "a", "ai",
        ]);
        assert_eq!(
            table.spellings_for("426", 2, None),
            ["gan", "gang", "gao", "han", "hang", "hao", "ga", "ha"]
        );
        // With 31 digits already locked, only a one-digit completion still fits in 32.
        assert_eq!(table.spellings_for("2", 31, None), ["a"]);
        assert!(table.spellings_for("", 0, None).is_empty());
        // 在两个数字之后切开，就排除了所有跨过这个位置的音节。
        assert_eq!(table.spellings_for("426", 0, Some(2)), ["ga", "ha"]);
    }

    fn item(word: &str, digits: &str, weight: i64, source: CandidateSource) -> WordItem {
        WordItem::new(digits, word, weight, source, "")
    }

    #[test]
    fn refresh_tail_is_borrowed_from_digits() {
        assert_eq!(remaining_digits("64426", 2), "426");
    }

    #[test]
    fn path_key_builder_reuses_existing_string_storage() {
        let path = vec!["hao".to_owned(), "ma".to_owned()];
        let mut key = String::with_capacity(32);
        append_path_key(&mut key, "ni", &path);
        assert_eq!(key, "ni'hao'ma");

        let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
            append_path_key(&mut key, "ni", &path);
        });

        assert_eq!(key, "ni'hao'ma");
        assert_eq!(allocations, 0);
    }

    #[test]
    fn candidate_word_lookup_scans_existing_rows() {
        let candidates = vec![item("old", "653", 1, CandidateSource::EnglishDictionary)];
        assert!(has_candidate_word(&candidates, "old"));
        assert!(!has_candidate_word(&candidates, "older"));
    }

    #[test]
    fn unique_word_retain_keeps_the_first_sorted_row() {
        let mut candidates = vec![
            item("你", "644", 10, CandidateSource::Database),
            item("你", "64", 1, CandidateSource::Generated),
            item("泥", "64", 2, CandidateSource::Database),
        ];
        retain_unique_words(&mut candidates);
        assert_eq!(
            candidates
                .iter()
                .map(|candidate| candidate.word.as_str())
                .collect::<Vec<_>>(),
            ["你", "泥"]
        );
    }

    #[test]
    fn short_unique_word_retain_avoids_temporary_heap_state() {
        let mut candidates = vec![
            item("你", "644", 10, CandidateSource::Database),
            item("你", "64", 1, CandidateSource::Generated),
            item("泥", "64", 2, CandidateSource::Database),
        ];
        let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
            retain_unique_words(&mut candidates);
        });

        assert_eq!(allocations, 0);
        assert_eq!(
            candidates
                .iter()
                .map(|candidate| candidate.word.as_str())
                .collect::<Vec<_>>(),
            ["你", "泥"]
        );
    }

    #[test]
    fn query_key_lookup_deduplicates_paths_and_hashed_keys() {
        let alternatives = vec![
            vec!["ni".to_owned(), "hao".to_owned()],
            vec!["ni".to_owned(), "hao".to_owned()],
        ];
        assert!(query_key_is_new(&alternatives, 0, "ni'hao", None));
        assert!(!query_key_is_new(&alternatives, 1, "ni'hao", None));

        let mut queried = HashSet::new();
        assert!(query_key_is_new(&alternatives, 0, "ni'hao", Some(&queried)));
        queried.insert("ni'hao".to_owned());
        assert!(!query_key_is_new(
            &alternatives,
            1,
            "ni'hao",
            Some(&queried)
        ));
    }

    #[test]
    fn query_key_dedup_switches_to_hashing_after_small_batch_boundary() {
        let alternatives = (0..=SMALL_QUERY_KEY_BATCH)
            .map(|index| vec![format!("syllable-{}", index % 32)])
            .collect::<Vec<_>>();
        let mut queried = (alternatives.len() > SMALL_QUERY_KEY_BATCH)
            .then(|| HashSet::with_capacity(alternatives.len()));
        let mut unique = 0;
        for (index, path) in alternatives.iter().enumerate() {
            let key = path[0].as_str();
            if query_key_is_new(&alternatives, index, key, queried.as_ref()) {
                unique += 1;
                if let Some(seen) = queried.as_mut() {
                    seen.insert(key.to_owned());
                }
            }
        }
        assert_eq!(unique, 32);
        assert!(queried.is_some());
    }

    #[test]
    fn candidates_rank_by_coverage_then_dictionary_rows() {
        let mut fuzzy = item("泥", "64", 900, CandidateSource::Database);
        fuzzy.fuzzy = true;
        let mut candidates = vec![
            item("你", "64", 100, CandidateSource::Database),
            item("米好", "64426", -19113, CandidateSource::Generated),
            fuzzy,
            item("米", "64", 50, CandidateSource::UserDatabase),
            item("你好", "64426", 1000, CandidateSource::Database),
            item("你", "64", 10, CandidateSource::Database),
        ];
        rank_candidates(&mut candidates, false, false);
        let words: Vec<_> = candidates.iter().map(|item| item.word.as_str()).collect();
        assert_eq!(words, ["你好", "米好", "你", "米", "泥"]);
        assert_eq!(candidates[2].weight, 100);
    }

    fn spelled(
        word: &str,
        digits: &str,
        canonical: &str,
        weight: i64,
        source: CandidateSource,
    ) -> WordItem {
        let mut row = item(word, digits, weight, source);
        row.canonical_pinyin = canonical.to_owned();
        row
    }

    fn ranked(mut candidates: Vec<WordItem>, prefer_exact: bool) -> Vec<String> {
        rank_candidates(&mut candidates, prefer_exact, false);
        candidates.into_iter().map(|item| item.word).collect()
    }

    #[test]
    fn without_a_split_a_completion_keeps_its_weight_over_an_exact_syllable() {
        // `3` alone: 的 completes d to de, 额 spells e exactly; the more common word still leads.
        let rows = vec![
            spelled("额", "3", "e", 10, CandidateSource::Database),
            spelled("的", "3", "de", 1000, CandidateSource::Database),
        ];
        assert_eq!(ranked(rows, false), ["的", "额"]);
    }

    #[test]
    fn a_synthesised_exact_row_never_leads_a_dictionary_completion() {
        for prefer_exact in [false, true] {
            let rows = vec![
                spelled("额", "3", "e", 99_999, CandidateSource::Generated),
                spelled("的", "3", "de", 1000, CandidateSource::Database),
            ];
            assert_eq!(
                ranked(rows, prefer_exact),
                ["的", "额"],
                "prefer_exact {prefer_exact}"
            );
        }
    }

    #[test]
    fn after_a_split_an_exact_reading_leads_a_completion() {
        // `94'26`: 西安 spells xi'an to its end, 自从 has to be completed to zi'cong.
        let rows = vec![
            spelled("自从", "9426", "zi'cong", 5000, CandidateSource::Database),
            spelled("西安", "9426", "xi'an", 100, CandidateSource::Database),
        ];
        assert_eq!(ranked(rows.clone(), true), ["西安", "自从"]);
        assert_eq!(ranked(rows, false), ["自从", "西安"]);
    }

    #[test]
    fn skipping_outranked_rows_ranks_like_pushing_everything() {
        let sources = [
            CandidateSource::Database,
            CandidateSource::UserDatabase,
            CandidateSource::Generated,
            CandidateSource::Fallback,
        ];
        let mut state = 0x2545_F491_4F6C_DD1D_u64;
        let mut next = |bound: u64| {
            state ^= state << 13;
            state ^= state >> 7;
            state ^= state << 17;
            state % bound
        };
        for round in 0..200 {
            let mut rows = Vec::new();
            for _ in 0..(1 + next(300)) {
                let digits = &"64426646"[..1 + next(8) as usize];
                let mut row = item(
                    &format!("w{}", next(40)),
                    digits,
                    next(5) as i64 * 10,
                    sources[next(4) as usize],
                );
                row.fuzzy = next(3) == 0;
                row.canonical_pinyin =
                    ["e", "de", "xi'an", "zi'cong", "ni'hao'ma"][next(5) as usize].to_owned();
                rows.push(row);
            }
            let prefer_exact = round % 2 == 0;
            let mut everything = rows.clone();
            rank_candidates(&mut everything, prefer_exact, false);
            let mut skipped = Vec::new();
            let mut leading = HashMap::new();
            for row in rows {
                push_ranked(&mut skipped, &mut leading, row, prefer_exact, false);
            }
            rank_candidates(&mut skipped, prefer_exact, false);
            assert_eq!(skipped, everything, "round {round}");
        }
    }

    #[test]
    fn english_exact_code_leads_only_with_a_weight() {
        let english =
            |word: &str, weight| item(word, word, weight, CandidateSource::EnglishDictionary);
        let mut words = vec![
            english("old", 1000),
            english("ok", 900),
            english("older", 800),
        ];
        rank_english(&mut words, 2);
        let order: Vec<_> = words.iter().map(|item| item.word.as_str()).collect();
        assert_eq!(order, ["ok", "old", "older"]);
        let mut words = vec![english("ogham", 0), english("oh", 5), english("ogre", 5)];
        rank_english(&mut words, 5);
        let order: Vec<_> = words.iter().map(|item| item.word.as_str()).collect();
        assert_eq!(order, ["oh", "ogre", "ogham"]);
        let mut many: Vec<_> = (0..8).map(|n| english(&format!("w{n}"), n)).collect();
        rank_english(&mut many, 2);
        assert_eq!(many.len(), ENGLISH_LIMIT);
    }

    #[test]
    fn english_dedup_keeps_the_first_row_for_each_display_word() {
        let mut words = vec![
            item("one", "one", 100, CandidateSource::EnglishDictionary),
            item("two", "two", 90, CandidateSource::EnglishDictionary),
            item("one", "one", 80, CandidateSource::EnglishDictionary),
        ];

        deduplicate_english_words(&mut words);

        assert_eq!(
            words
                .iter()
                .map(|word| (word.word.as_str(), word.weight))
                .collect::<Vec<_>>(),
            [("one", 100), ("two", 90)]
        );
    }

    #[test]
    fn locked_syllables_constrain_readings() {
        assert!(agrees_with_locked("mi", ""));
        assert!(agrees_with_locked("ni", "ni"));
        assert!(agrees_with_locked("ni'hao", "ni"));
        assert!(agrees_with_locked("ni", "ni'hao"));
        assert!(!agrees_with_locked("mi", "ni"));
        assert!(!agrees_with_locked("nian", "ni"));
        assert!(!agrees_with_locked("ni", "nian"));
    }

    fn detached() -> NineKeySession {
        NineKeySession::new(
            &RuntimePaths::default(),
            false,
            FrequencyAdjustmentOptions::default(),
            FuzzyPinyinOptions::default(),
            EnglishInputOptions::default(),
            true,
            false,
        )
    }

    #[test]
    fn consuming_digits_drops_covered_locked_syllables() {
        let mut session = detached();
        session.digits = "64426".into();
        session.locked = vec!["ni".into(), "hao".into()];
        session.consume(2);
        assert_eq!(session.digits, "426");
        assert_eq!(session.locked, ["hao"]);
        // An English row's code is the word, which may run past the digits.
        session.consume(10);
        assert!(session.digits.is_empty());
        assert!(session.locked.is_empty());
    }

    #[test]
    fn consuming_digits_reuses_split_storage() {
        let mut session = detached();
        session.digits = "64426".into();
        session.splits = vec![2, 4];
        let (_, allocations) =
            crate::ime::personal_rerank::allocations::count(|| session.consume(2));
        assert_eq!(session.splits, [2]);
        assert_eq!(allocations, 0);
    }

    #[test]
    fn snapshot_shows_locked_syllables_before_the_open_digits() {
        let mut session = detached();
        session.digits = "64426".into();
        assert_eq!(session.snapshot().preedit, "64426");
        session.locked = vec!["ni".into()];
        assert_eq!(session.snapshot().preedit, "ni'426");
        session.locked.push("hao".into());
        session.candidates = vec![
            item("你好", "64426", 1000, CandidateSource::Database),
            item("你", "64", 100, CandidateSource::Database),
        ];
        let view = session.snapshot();
        assert_eq!(view.preedit, "ni'hao");
        assert_eq!(view.editing_text, "64426");
        assert_eq!(view.caret_position, 5);
        assert_eq!(view.scheme, SchemeType::Quanpin);
        assert_eq!(view.candidate_answers_key, [true, false]);
        assert_eq!(view.candidate_sources, [CandidateSource::Database; 2]);
        assert_eq!(view.candidate_annotations, ["", ""]);
        assert_eq!(session.ranking_context(), "nine-key:64426:ni'hao");
    }

    // ---- Ported from test_nine_key_session.cpp against the session itself ----

    const MAIN_FIXTURE: &str = "CREATE TABLE tbl_1_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_1_n VALUES('ni','n','你',100);CREATE TABLE tbl_1_m(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_1_m VALUES('mi','m','米',50);CREATE TABLE tbl_1_h(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_1_h VALUES('hao','h','好',100);CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_2_n VALUES('ni''hao','nh','你好',1000);";
    const ENGLISH_FIXTURE: &str = "CREATE TABLE english_words(word TEXT,display TEXT,weight INTEGER);INSERT INTO english_words VALUES('ok','ok',900);INSERT INTO english_words VALUES('old','old',1000);INSERT INTO english_words VALUES('older','older',800);INSERT INTO english_words VALUES('ogham','ogham',0);INSERT INTO english_words VALUES('qq','QQ',500);";

    struct Fixture {
        directory: tempfile::TempDir,
        paths: RuntimePaths,
    }

    fn fixture() -> Fixture {
        fixture_with(MAIN_FIXTURE)
    }

    fn fixture_with(main: &str) -> Fixture {
        let directory = tempfile::tempdir().expect("temporary directory");
        let root = directory.path().to_path_buf();
        Connection::open(root.join(assets::MAIN_DICTIONARY))
            .and_then(|db| db.execute_batch(main))
            .expect("main fixture");
        Connection::open(root.join(assets::ENGLISH_DICTIONARY))
            .and_then(|db| db.execute_batch(ENGLISH_FIXTURE))
            .expect("english fixture");
        let paths = RuntimePaths {
            resources: root.clone(),
            user_data: root.clone(),
            cache: root.clone(),
            dictionaries: root,
        };
        Fixture { directory, paths }
    }

    fn open(paths: &RuntimePaths, learning: bool, english: EnglishInputOptions) -> NineKeySession {
        let frequency = FrequencyAdjustmentOptions {
            mode: if learning {
                FrequencyAdjustmentMode::Promote
            } else {
                FrequencyAdjustmentMode::Disabled
            },
            ..FrequencyAdjustmentOptions::default()
        };
        NineKeySession::new(
            paths,
            learning,
            frequency,
            FuzzyPinyinOptions::default(),
            english,
            true,
            false,
        )
    }

    fn mixed() -> EnglishInputOptions {
        EnglishInputOptions {
            mixed_candidates: true,
            ..EnglishInputOptions::default()
        }
    }

    fn type_digits(session: &mut NineKeySession, digits: &str) {
        for digit in digits.bytes() {
            assert!(session.character(digit).handled, "digit {digit} unhandled");
        }
    }

    fn index_of(session: &NineKeySession, word: &str) -> usize {
        session
            .snapshot()
            .candidates
            .iter()
            .position(|item| item.word == word)
            .unwrap_or_else(|| panic!("missing candidate {word}"))
    }

    fn words(session: &NineKeySession) -> Vec<String> {
        session
            .snapshot()
            .candidates
            .into_iter()
            .map(|item| item.word)
            .collect()
    }

    #[test]
    fn digits_offer_readings_and_ranked_english() {
        let fixture = fixture();
        let mut session = open(&fixture.paths, false, mixed());
        assert!(!session.character(b'0').handled && !session.character(b'1').handled);
        // old is more frequent, but 65 spells ok exactly.
        type_digits(&mut session, "65");
        assert_eq!(words(&session), ["ok", "old", "older"]);
        assert_eq!(
            session.snapshot().nine_key_spellings,
            ["o", "M", "N", "O", "6"]
        );
        session.command(Command::Cancel);

        // 64426 is ni'hao and also the only code for ogham; spelling it exactly does not earn a zero-weight word the second slot. The two-syllable lattice adds a Generated 米好 that covers every digit, so it ranks above the two-digit dictionary rows (tests-inventory.md §2.4).
        type_digits(&mut session, "64426");
        let view = session.snapshot();
        assert_eq!(words(&session), ["你好", "米好", "你", "米", "ogham"]);
        assert_eq!(view.candidate_sources[1], CandidateSource::Generated);
        assert_eq!(
            view.nine_key_spellings,
            ["ni", "mi", "o", "M", "N", "O", "6"]
        );
        assert_eq!(view.candidate_answers_key[..4], [true, true, false, false]);
        // Within one coverage bucket no dictionary row follows a synthesised one.
        for pair in view.candidates.windows(2) {
            if pair[0].pinyin.len() == pair[1].pinyin.len()
                && pair[0].source.is_generated_or_fallback()
            {
                assert!(
                    !pair[1].source.is_dictionary(),
                    "{} after {}",
                    pair[1].word,
                    pair[0].word
                );
            }
        }
        session.command(Command::Cancel);

        type_digits(&mut session, "6");
        assert_eq!(
            words(&session),
            ["你", "米"],
            "mixed English waits for the minimum prefix"
        );
        assert_eq!(session.snapshot().candidates[0].pinyin, "6");
    }

    #[test]
    fn selecting_a_nine_key_candidate_does_not_clone_unused_row_fields() {
        let fixture = fixture();
        let mut session = open(&fixture.paths, false, mixed());
        type_digits(&mut session, "64426");
        let index = index_of(&session, "你好");

        let (result, allocations) =
            crate::ime::personal_rerank::allocations::count(|| session.select(index));

        assert_eq!(result.commit.as_deref(), Some("你好"));
        assert!(
            allocations <= 1,
            "九键选择候选复制无用行字段产生了 {allocations} 次分配"
        );
    }

    #[test]
    fn choosing_a_nine_key_letter_does_not_clone_spelling_choices() {
        let fixture = fixture();
        let mut session = open(&fixture.paths, false, mixed());
        type_digits(&mut session, "64426");
        let index = session
            .snapshot()
            .nine_key_spellings
            .iter()
            .position(|spelling| spelling == "N")
            .unwrap();

        let (result, allocations) =
            crate::ime::personal_rerank::allocations::count(|| session.choose_spelling(index));

        assert!(result.handled);
        assert!(
            allocations <= 347,
            "九键选择字母复制音节列表产生了 {allocations} 次分配"
        );
    }

    #[test]
    fn split_refresh_reuses_normalized_boundaries() {
        let fixture = fixture();
        let mut plain = open(&fixture.paths, false, EnglishInputOptions::default());
        plain.digits = "644".into();
        plain.refresh();
        let (_, plain_allocations) =
            crate::ime::personal_rerank::allocations::count(|| plain.refresh());
        let mut split = open(&fixture.paths, false, EnglishInputOptions::default());
        split.digits = "644".into();
        split.splits = vec![2];
        split.refresh();
        let (_, split_allocations) =
            crate::ime::personal_rerank::allocations::count(|| split.refresh());

        assert!(
            split_allocations <= 151,
            "split refresh should keep normalized boundaries off the heap: {split_allocations}"
        );
        assert!(split_allocations < plain_allocations);
    }

    #[test]
    fn refresh_reuses_candidate_pinyin_storage() {
        let fixture = fixture();
        let mut session = open(&fixture.paths, false, EnglishInputOptions::default());
        session.digits = "64".into();
        session.refresh();
        let (_, allocations) =
            crate::ime::personal_rerank::allocations::count(|| session.refresh());
        assert!(
            allocations <= 149,
            "candidate pinyin normalization allocated {allocations} buffers"
        );
    }

    #[test]
    fn choosing_a_spelling_with_a_split_does_not_allocate_a_second_partition() {
        let fixture = fixture();
        let mut plain = open(&fixture.paths, false, mixed());
        type_digits(&mut plain, "64426");
        let index = plain
            .spellings
            .iter()
            .position(|spelling| spelling == "ni")
            .unwrap();
        let (_, plain_allocations) =
            crate::ime::personal_rerank::allocations::count(|| plain.choose_spelling(index));

        let mut session = open(&fixture.paths, false, mixed());
        type_digits(&mut session, "64426");
        session.splits = vec![2];
        session.refresh();
        let index = session
            .spellings
            .iter()
            .position(|spelling| spelling == "ni")
            .unwrap();
        let (_, allocations) =
            crate::ime::personal_rerank::allocations::count(|| session.choose_spelling(index));

        assert!(
            allocations <= plain_allocations,
            "九键选择带切分的音节分配多于无切分路径：{allocations} > {plain_allocations}"
        );
    }

    #[test]
    fn english_t9_uses_the_lookup_word_when_display_has_punctuation() {
        let fixture = fixture();
        Connection::open(fixture.paths.dictionary(assets::ENGLISH_DICTIONARY))
            .unwrap()
            .execute(
                "INSERT INTO english_words(word, display, weight) VALUES ('dont', 'don''t', 100)",
                [],
            )
            .unwrap();
        let mut session = open(
            &fixture.paths,
            false,
            EnglishInputOptions {
                mixed_candidates: false,
                ..EnglishInputOptions::default()
            },
        );
        session.set_english_only(true);
        type_digits(&mut session, "3668");
        assert!(
            words(&session).contains(&"don't".to_owned()),
            "T9 should match the lookup key even when the displayed word contains punctuation"
        );
    }

    #[test]
    fn english_digits_take_no_split() {
        let fixture = fixture();
        let mut english_only = open(&fixture.paths, false, mixed());
        english_only.set_english_only(true);
        let mut without_pinyin = NineKeySession::new(
            &fixture.paths,
            false,
            FrequencyAdjustmentOptions::default(),
            FuzzyPinyinOptions::default(),
            mixed(),
            false,
            false,
        );
        for session in [&mut english_only, &mut without_pinyin] {
            type_digits(session, "65");
            assert!(!session.character(b'\'').handled);
            assert!(session.splits.is_empty());
            assert_eq!(session.snapshot().preedit, "65");
        }
    }

    #[test]
    fn a_fuzzy_leading_row_has_no_reading() {
        let mut session = detached();
        session.digits = "94".into();
        let xi = spelled("西", "94", "xi", 100, CandidateSource::Database);
        assert_eq!(session.reading_for(Some(&xi)), "xi");
        // 知 reached through zi under `94`: cutting zhi to two letters would show zh.
        let mut zhi = spelled("知", "94", "zhi", 100, CandidateSource::Database);
        zhi.fuzzy = true;
        assert_eq!(session.reading_for(Some(&zhi)), "");
    }

    #[test]
    fn split_keeps_both_sides_open_and_backspace_removes_it_first() {
        let table = SpellingTable::new(&["xi", "yi", "an", "xian", "yan"]);
        assert_eq!(
            table
                .split_paths("9426", &[], &SyllablePrior::default())
                .first(),
            Some(&vec!["xian".to_string()])
        );
        let split = table.split_paths("9426", &[2], &SyllablePrior::default());
        assert!(!split.is_empty());
        assert!(
            split.iter().all(|path| path.len() == 2 && path[1] == "an"),
            "{split:?}"
        );

        let fixture = fixture();
        let mut session = open(&fixture.paths, false, mixed());
        assert!(!session.character(b'\'').handled, "nothing to split");
        type_digits(&mut session, "64");
        assert!(session.character(b'\'').handled);
        assert!(
            session.character(b'\'').handled,
            "a repeated split is absorbed"
        );
        assert_eq!(session.snapshot().preedit, "64'");
        assert!(
            session
                .snapshot()
                .nine_key_spellings
                .iter()
                .all(|s| s.len() <= 2),
            "no syllable runs past the split"
        );
        type_digits(&mut session, "426");
        assert_eq!(session.snapshot().preedit, "64'426");
        assert_eq!(words(&session).first().map(String::as_str), Some("你好"));
        assert_eq!(session.snapshot().nine_key_reading, "ni'hao");
        assert!(session.command(Command::Backspace).handled);
        assert_eq!(session.snapshot().preedit, "64'42");
        type_digits(&mut session, "6");
        let result = session.select(index_of(&session, "你"));
        assert_eq!(result.commit.as_deref(), Some("你"));
        assert_eq!(
            session.snapshot().preedit,
            "426",
            "the split went with the consumed digits"
        );

        session.command(Command::Cancel);
        type_digits(&mut session, "64");
        session.character(b'\'');
        assert!(session.command(Command::Backspace).handled);
        assert_eq!(
            session.snapshot().preedit,
            "64",
            "Backspace takes the split before a digit"
        );
    }

    #[test]
    fn spellings_lock_and_partial_selection_advances() {
        let fixture = fixture();
        let mut session = open(&fixture.paths, false, mixed());
        type_digits(&mut session, "64");
        let before = session.snapshot();
        assert!(!session.select(999).handled && !session.choose_spelling(999).handled);
        assert_eq!(session.snapshot(), before);
        assert_eq!(
            before.nine_key_spellings,
            [
                "ni", "mi", "mian", "miao", "mie", "min", "ming", "miu", "nian", "niang", "niao",
                "nie", "nin", "ning", "niu", "o", "M", "N", "O", "6"
            ],
            "the preferred spelling leads, the key's letters and digit follow"
        );
        assert!(session.choose_spelling(0).handled);
        assert_eq!(session.snapshot().preedit, "ni");
        assert_eq!(words(&session), ["你"], "the lock keeps 米 out");
        type_digits(&mut session, "426");
        let view = session.snapshot();
        assert_eq!(view.preedit, "ni'426");
        assert_eq!(
            view.nine_key_spellings,
            ["hao", "gan", "gang", "gao", "han", "hang", "ga", "ha", "G", "H"],
            "no i, which starts no syllable, and no digit behind a lock"
        );
        let result = session.select(index_of(&session, "你好"));
        assert_eq!(result.commit.as_deref(), Some("你好"));
        assert!(!session.active() && session.snapshot().preedit.is_empty());

        type_digits(&mut session, "64426");
        let ni = session
            .snapshot()
            .nine_key_spellings
            .iter()
            .position(|s| s == "ni")
            .expect("ni offered");
        session.choose_spelling(ni);
        let hao_choices = session.snapshot().nine_key_spellings;
        session.choose_spelling(0);
        assert_eq!(session.snapshot().preedit, "ni'hao");
        assert_eq!(
            session.snapshot().nine_key_spellings,
            hao_choices,
            "every digit locked still offers the last lock's choices"
        );
        let result = session.select(index_of(&session, "你"));
        assert_eq!(result.commit.as_deref(), Some("你"));
        let view = session.snapshot();
        assert_eq!(view.editing_text, "426");
        assert_eq!(
            view.preedit, "hao",
            "partial selection lost the locked suffix"
        );
        let result = session.finish(0);
        assert_eq!(result.commit.as_deref(), Some("好"));
        assert!(!session.active());

        // A longer spelling extends the digits to its whole code.
        type_digits(&mut session, "6");
        let ming = session
            .snapshot()
            .nine_key_spellings
            .iter()
            .position(|s| s == "ming")
            .expect("ming offered");
        session.choose_spelling(ming);
        assert_eq!(session.snapshot().editing_text, "6464");
        assert_eq!(session.snapshot().preedit, "ming");
    }

    fn spelling_index(session: &NineKeySession, spelling: &str) -> usize {
        session
            .snapshot()
            .nine_key_spellings
            .iter()
            .position(|s| s == spelling)
            .unwrap_or_else(|| panic!("missing spelling {spelling}"))
    }

    #[test]
    fn backspace_takes_back_the_last_lock_only_while_every_digit_is_locked() {
        let fixture = fixture();
        let mut session = open(&fixture.paths, false, mixed());
        type_digits(&mut session, "6464224");
        session.choose_spelling(spelling_index(&session, "ning"));
        assert_eq!(session.snapshot().preedit, "ning'224");
        let after_ning = session.snapshot().nine_key_spellings;
        assert!(after_ning.contains(&"bai".to_owned()) && after_ning.contains(&"cai".to_owned()));
        session.choose_spelling(spelling_index(&session, "bai"));
        assert_eq!(session.snapshot().preedit, "ning'bai");
        assert_eq!(session.snapshot().nine_key_spellings, after_ning);

        session.choose_spelling(spelling_index(&session, "cai"));
        assert_eq!(
            session.snapshot().preedit,
            "ning'cai",
            "picking again swaps the last lock"
        );
        assert_eq!(session.locked, ["ning", "cai"]);
        assert_eq!(session.snapshot().nine_key_spellings, after_ning);

        assert!(session.command(Command::Backspace).handled);
        assert_eq!(session.snapshot().preedit, "ning'224");
        assert_eq!(session.snapshot().nine_key_spellings, after_ning);
        assert!(session.command(Command::Backspace).handled);
        assert_eq!(
            session.locked,
            ["ning"],
            "with digits open a digit goes, not the lock"
        );
        assert_eq!(session.snapshot().preedit, "ning'22");
    }

    #[test]
    fn taking_back_a_lock_restores_the_digits_it_extended() {
        let fixture = fixture();
        let mut session = open(&fixture.paths, false, mixed());
        type_digits(&mut session, "6");
        session.choose_spelling(spelling_index(&session, "ming"));
        assert_eq!(session.snapshot().editing_text, "6464");
        session.command(Command::Backspace);
        assert_eq!(session.snapshot().editing_text, "6");
        assert!(session.locked.is_empty());
        assert!(session
            .snapshot()
            .nine_key_spellings
            .contains(&"ming".to_owned()));
    }

    #[test]
    fn a_split_dropped_by_a_lock_comes_back_with_it() {
        let fixture = fixture();
        let mut session = open(&fixture.paths, false, mixed());
        type_digits(&mut session, "64");
        session.character(b'\'');
        session.choose_spelling(spelling_index(&session, "ni"));
        assert!(session.splits.is_empty());
        assert_eq!(session.snapshot().preedit, "ni");
        session.command(Command::Backspace);
        assert_eq!(
            session.snapshot().preedit,
            "64'",
            "the split is back with the digits"
        );
        session.command(Command::Backspace);
        assert_eq!(session.snapshot().preedit, "64");
    }

    #[test]
    fn a_key_letter_narrows_the_next_syllable_and_backspace_takes_it_back() {
        let fixture = fixture();
        let mut session = open(&fixture.paths, false, mixed());
        type_digits(&mut session, "64");
        // 你好 是 `64` 的简拼行（n'h）；选了首字母后不查简拼，它和英文词一起退出。
        assert_eq!(words(&session), ["你", "米", "你好", "ogham"]);
        assert!(
            session
                .choose_spelling(spelling_index(&session, "M"))
                .handled
        );
        let view = session.snapshot();
        assert_eq!(view.preedit, "m4");
        assert_eq!(words(&session), ["米"]);
        assert!(view
            .nine_key_spellings
            .iter()
            .filter(|s| s.len() > 1)
            .all(|s| s.starts_with('m')));
        assert!(view.nine_key_spellings.ends_with(&[
            "M".into(),
            "N".into(),
            "O".into(),
            "6".into()
        ]));

        session.choose_spelling(spelling_index(&session, "mi"));
        assert_eq!(session.snapshot().preedit, "mi");
        session.command(Command::Backspace);
        assert_eq!(
            session.snapshot().preedit,
            "m4",
            "taking back the lock restores the letter"
        );
        session.command(Command::Backspace);
        assert_eq!(session.snapshot().preedit, "64");
        assert_eq!(words(&session), ["你", "米", "你好", "ogham"]);
    }

    #[test]
    fn a_letter_survives_typing_but_not_deleting_past_it() {
        let fixture = fixture();
        let mut session = open(&fixture.paths, false, mixed());
        type_digits(&mut session, "64");
        session.choose_spelling(spelling_index(&session, "N"));
        type_digits(&mut session, "4");
        session.command(Command::Backspace);
        assert_eq!(
            session.snapshot().preedit,
            "n4",
            "the typed digit goes first"
        );
        session.command(Command::Backspace);
        assert_eq!(session.snapshot().preedit, "64");

        type_digits(&mut session, "426");
        session.choose_spelling(spelling_index(&session, "N"));
        session.command(Command::Cancel);
        type_digits(&mut session, "64");
        assert_eq!(
            session.snapshot().preedit,
            "64",
            "a new composition starts without it"
        );
    }

    #[test]
    fn the_key_digit_commits_itself_and_the_rest_keeps_composing() {
        let fixture = fixture();
        let mut session = open(&fixture.paths, false, mixed());
        type_digits(&mut session, "64");
        let result = session.choose_spelling(spelling_index(&session, "6"));
        assert_eq!(result.commit.as_deref(), Some("6"));
        assert_eq!(session.snapshot().editing_text, "4");
    }

    /// 左列的数字原样上屏一位：正在拼的词里夹了这一位就不再存（否则选完时会把它前后的两段连成一个错的词）；数字上屏完时组字结束，正在拼的词随之作废。
    #[test]
    fn a_key_digit_inside_a_phrase_keeps_it_from_being_stored() {
        let fixture = fixture();
        let mut session = open(&fixture.paths, true, mixed());
        type_digits(&mut session, "64426");
        session.select(index_of(&session, "你"));
        let result = session.choose_spelling(spelling_index(&session, "4"));
        assert_eq!(result.commit.as_deref(), Some("4"));
        assert!(!session.phrase_storable);
        assert_eq!(session.phrase_word, "你4");
        session.command(Command::Cancel);

        type_digits(&mut session, "644");
        session.select(index_of(&session, "你"));
        session.choose_spelling(spelling_index(&session, "4"));
        assert!(!session.active());
        assert!(session.phrase_word.is_empty() && session.phrase_storable);
    }

    #[test]
    fn single_character_filter_keeps_single_characters_until_the_composition_ends() {
        let fixture = fixture();
        let mut session = open(&fixture.paths, false, mixed());
        assert!(!session.set_filter(true, "").handled, "nothing composing");
        type_digits(&mut session, "64426");
        assert!(session.set_filter(true, "").handled);
        assert!(session.snapshot().nine_key_single_character);
        assert_eq!(words(&session), ["你", "米"]);
        assert!(session.set_filter(false, "").handled);
        assert!(words(&session).contains(&"你好".to_owned()));
        session.set_filter(true, "");
        session.command(Command::Cancel);
        type_digits(&mut session, "64426");
        assert!(!session.snapshot().nine_key_single_character);
        assert!(words(&session).contains(&"ogham".to_owned()));
    }

    #[test]
    fn stroke_filter_matches_the_first_character_stroke_prefix() {
        let fixture = fixture();
        let strokes = fixture.directory.path().join("msime-stroke.db");
        crate::stroke::fixture::build(&strokes);
        Connection::open(&strokes)
            .unwrap()
            .execute_batch(
                "INSERT INTO entries VALUES ('pspzspn', '你', 100); INSERT INTO entries VALUES ('nphspn', '米', 100);",
            )
            .unwrap();
        let mut session = open(&fixture.paths, false, mixed());
        type_digits(&mut session, "64426");
        assert_eq!(
            session.set_filter(false, "p").diagnostic.as_deref(),
            Some(diagnostics::LANGUAGE_DICTIONARY_UNAVAILABLE),
            "no stroke dictionary yet"
        );
        assert!(session.snapshot().nine_key_strokes.is_empty());

        session.set_stroke_dictionary(strokes);
        assert!(!session.set_filter(false, "q").handled, "not a stroke");
        assert!(session.set_filter(false, "ps").handled);
        assert_eq!(session.snapshot().nine_key_strokes, "ps");
        assert_eq!(words(&session), ["你好", "你"]);
        session.set_filter(true, "n");
        assert_eq!(words(&session), ["米"]);
        session.set_filter(false, "z");
        assert!(words(&session).is_empty());
        session.set_filter(false, "");
        assert!(words(&session).contains(&"米好".to_owned()));
    }

    /// 只出单字时拼音读法只剩单字，英文行照旧；选一个字后剩下的数字接着出单字。
    #[test]
    fn single_character_only_keeps_one_character_readings() {
        let fixture = fixture();
        let mut session = NineKeySession::new(
            &fixture.paths,
            false,
            FrequencyAdjustmentOptions::default(),
            FuzzyPinyinOptions::default(),
            mixed(),
            true,
            true,
        );
        type_digits(&mut session, "64426");
        assert_eq!(words(&session), ["你", "米", "ogham"]);
        let result = session.select(index_of(&session, "你"));
        assert_eq!(result.commit.as_deref(), Some("你"));
        assert_eq!(session.snapshot().editing_text, "426");
        assert_eq!(words(&session), ["好"]);
        let result = session.finish(0);
        assert_eq!(result.commit.as_deref(), Some("好"));
        assert!(!session.active());
    }

    #[test]
    fn commands_edit_commit_and_cancel_the_digits() {
        let fixture = fixture();
        let mut session = open(&fixture.paths, false, mixed());
        assert!(!session.command(Command::Backspace).handled && !session.finish(0).handled);
        type_digits(&mut session, "64");
        session.command(Command::Backspace);
        assert_eq!(session.snapshot().editing_text, "6");
        session.command(Command::Backspace);
        assert!(
            !session.command(Command::Backspace).handled,
            "idle backspace"
        );

        type_digits(&mut session, "64");
        session.choose_spelling(0);
        session.command(Command::Backspace);
        assert!(
            session.locked.is_empty(),
            "backspace first takes back a lock that covers every digit"
        );
        assert_eq!(session.snapshot().preedit, "64");
        session.command(Command::Backspace);
        assert_eq!(session.snapshot().preedit, "6");
        session.command(Command::Cancel);

        type_digits(&mut session, "64");
        // 光标可以移进数字中间（#5613），上屏原样数字时仍是整串。
        assert!(session.command(Command::MoveLeft).handled);
        assert_eq!(session.snapshot().caret_position, 1);
        assert_eq!(
            session.command(Command::CommitRaw).commit.as_deref(),
            Some("64")
        );
        assert_eq!(
            session.snapshot().caret_position,
            0,
            "a new composition starts at its end"
        );
        assert!(!session.active());
        type_digits(&mut session, "64");
        assert_eq!(session.finish(999).commit.as_deref(), Some("64"));
        type_digits(&mut session, "64426");
        assert_eq!(
            session.command(Command::CommitCandidate).commit.as_deref(),
            Some("你好")
        );
        type_digits(&mut session, "64");
        session.command(Command::Cancel);
        let view = session.snapshot();
        assert!(
            view.preedit.is_empty()
                && view.nine_key_spellings.is_empty()
                && view.candidates.is_empty()
        );

        type_digits(&mut session, &"7".repeat(DIGIT_LIMIT));
        let result = session.character(b'7');
        assert!(result.handled);
        assert_eq!(
            result.diagnostic.as_deref(),
            Some(diagnostics::NINE_KEY_DIGIT_LIMIT)
        );
        assert_eq!(session.snapshot().editing_text.len(), DIGIT_LIMIT);
        assert!(session.snapshot().candidates.is_empty());
        // Finishing with nothing to choose commits the digits as typed.
        assert_eq!(
            session.finish(0).commit.as_deref(),
            Some("7".repeat(DIGIT_LIMIT).as_str())
        );
    }

    /// #5613：光标移进数字中间后，退格、向后删除、打数字和切分都作用在光标处，用来改掉中间打错的那个数字。
    #[test]
    fn the_caret_edits_digits_in_the_middle() {
        let fixture = fixture();
        let mut session = open(&fixture.paths, false, EnglishInputOptions::default());
        // 想打 64426（你好），中间的 4 错打成了 5。
        type_digits(&mut session, "64526");
        assert_eq!(session.snapshot().caret_position, 5);
        for _ in 0..2 {
            assert!(session.command(Command::MoveLeft).handled);
        }
        assert_eq!(session.snapshot().caret_position, 3);
        assert!(session.command(Command::Backspace).handled);
        assert_eq!(session.snapshot().editing_text, "6426");
        assert_eq!(session.snapshot().caret_position, 2);
        type_digits(&mut session, "4");
        let view = session.snapshot();
        assert_eq!(view.editing_text, "64426");
        assert_eq!(view.caret_position, 3);
        assert_eq!(
            view.candidates[0].word, "你好",
            "the whole digits are decoded again"
        );

        // 到头和到尾都停住；移到末尾后光标回到「在末尾」。
        assert!(session.command(Command::MoveHome).handled);
        assert_eq!(session.snapshot().caret_position, 0);
        assert!(session.command(Command::MoveLeft).handled);
        assert_eq!(session.snapshot().caret_position, 0);
        assert!(
            session.command(Command::Backspace).handled,
            "backspace at the start does nothing"
        );
        assert_eq!(session.snapshot().editing_text, "64426");
        assert!(session.command(Command::DeleteForward).handled);
        assert_eq!(session.snapshot().editing_text, "4426");
        assert_eq!(session.snapshot().caret_position, 0);
        type_digits(&mut session, "6");
        assert!(session.command(Command::MoveEnd).handled);
        assert!(session.command(Command::MoveRight).handled);
        assert_eq!(session.snapshot().caret_position, 5);
        assert!(
            session.command(Command::DeleteForward).handled,
            "delete at the end does nothing"
        );
        assert_eq!(session.snapshot().editing_text, "64426");
        session.command(Command::Cancel);

        // 切分打在光标处；光标紧跟在切分后面时，退格先删掉切分。
        type_digits(&mut session, "6426");
        session.command(Command::MoveLeft);
        session.command(Command::MoveLeft);
        assert!(session.character(b'\'').handled);
        assert_eq!(session.snapshot().preedit, "64'26");
        type_digits(&mut session, "4");
        assert_eq!(session.snapshot().preedit, "64'426");
        assert_eq!(session.snapshot().caret_position, 3);
        session.command(Command::MoveLeft);
        assert!(session.command(Command::Backspace).handled);
        assert_eq!(session.snapshot().preedit, "64426");
        assert_eq!(session.snapshot().caret_position, 2);
        session.command(Command::Cancel);

        // 在锁定的拼写里改数字，从那个音节起解除锁定。
        type_digits(&mut session, "64426");
        let ni = session
            .snapshot()
            .nine_key_spellings
            .iter()
            .position(|spelling| spelling == "ni")
            .expect("ni offered");
        session.choose_spelling(ni);
        assert_eq!(session.snapshot().preedit, "ni'426");
        session.command(Command::MoveHome);
        session.command(Command::MoveRight);
        type_digits(&mut session, "4");
        assert!(
            session.locked.is_empty(),
            "an edit inside a locked spelling unlocks it"
        );
        assert_eq!(session.snapshot().editing_text, "644426");

        // 选掉前面一段后光标回到末尾，接着打的数字接在剩下的数字后面。
        session.command(Command::Cancel);
        type_digits(&mut session, "64426");
        session.command(Command::MoveLeft);
        let ni = session.select(index_of(&session, "你"));
        assert_eq!(ni.commit.as_deref(), Some("你"));
        let view = session.snapshot();
        assert_eq!(view.editing_text, "426");
        assert_eq!(view.caret_position, 3);
        type_digits(&mut session, "6");
        assert_eq!(session.snapshot().editing_text, "4266");
        session.command(Command::Cancel);

        // 切在开头或锁定拼写之间的边界上没有作用，锁定的拼写都留着；切在锁定拼写中间时从那个音节起解除锁定。
        type_digits(&mut session, "64426");
        for spelling in ["ni", "hao"] {
            let index = session
                .snapshot()
                .nine_key_spellings
                .iter()
                .position(|offered| offered == spelling)
                .unwrap_or_else(|| panic!("{spelling} offered"));
            session.choose_spelling(index);
        }
        assert_eq!(session.snapshot().preedit, "ni'hao");
        let before = session.snapshot();
        session.command(Command::MoveHome);
        assert!(session.character(b'\'').handled);
        session.command(Command::MoveRight);
        session.command(Command::MoveRight);
        assert!(session.character(b'\'').handled);
        assert_eq!(session.locked, ["ni", "hao"]);
        let after = session.snapshot();
        assert_eq!(after.preedit, before.preedit);
        assert_eq!(after.candidates, before.candidates);
        assert_eq!(after.nine_key_reading, before.nine_key_reading);
        session.command(Command::MoveRight);
        assert!(session.character(b'\'').handled);
        assert_eq!(session.locked, ["ni"]);
        assert_eq!(session.snapshot().preedit, "ni'4'26");
    }

    /// #5613 的光标与左列的首字母、锁定的撤销记录和换选：退格只在光标在末尾时撤销最后一步，光标在中间时是在那里改字；改到锁定的拼写时撤销记录随锁定一起丢掉；取消把光标和这些状态一起复位。
    #[test]
    fn the_caret_meets_key_letters_and_taking_back_locks() {
        let fixture = fixture();
        let mut session = open(&fixture.paths, false, mixed());

        // 选了首字母后在中间退格，删的是光标前的数字，首字母留着。
        type_digits(&mut session, "644");
        session.choose_spelling(spelling_index(&session, "N"));
        assert_eq!(session.snapshot().preedit, "n44");
        session.command(Command::MoveLeft);
        assert!(session.command(Command::Backspace).handled);
        let view = session.snapshot();
        assert_eq!(view.preedit, "n4");
        assert_eq!(view.caret_position, 1);
        // 在选了首字母的那一位前面插数字，或者删掉那一位，首字母都不再成立。
        session.command(Command::MoveHome);
        type_digits(&mut session, "9");
        assert!(session.initial.is_none());
        assert_eq!(session.snapshot().preedit, "964");
        session.command(Command::Cancel);
        type_digits(&mut session, "644");
        session.choose_spelling(spelling_index(&session, "N"));
        session.command(Command::MoveHome);
        assert!(session.command(Command::DeleteForward).handled);
        assert!(session.initial.is_none());
        assert_eq!(session.snapshot().preedit, "44");
        session.command(Command::Cancel);

        // 数字全部锁定时在中间退格：删光标前的数字，盖住它的锁定连同撤销记录一起解除，前面的锁定留着。
        type_digits(&mut session, "6464224");
        session.choose_spelling(spelling_index(&session, "ning"));
        session.choose_spelling(spelling_index(&session, "bai"));
        assert_eq!(session.snapshot().preedit, "ning'bai");
        session.command(Command::MoveLeft);
        session.command(Command::MoveLeft);
        assert!(session.command(Command::Backspace).handled);
        assert_eq!(session.locked, ["ning"]);
        assert_eq!(session.lock_undo.len(), 1);
        let view = session.snapshot();
        assert_eq!(view.preedit, "ning'24");
        assert_eq!(view.caret_position, 4);
        session.command(Command::Cancel);

        // 在锁定的拼写里插数字后没有锁定可撤销，回到末尾退格删的是数字。
        type_digits(&mut session, "64426");
        session.choose_spelling(spelling_index(&session, "ni"));
        session.choose_spelling(spelling_index(&session, "hao"));
        session.command(Command::MoveHome);
        session.command(Command::MoveRight);
        type_digits(&mut session, "4");
        assert!(session.locked.is_empty() && session.lock_undo.is_empty());
        session.command(Command::MoveEnd);
        session.command(Command::Backspace);
        assert_eq!(session.snapshot().editing_text, "64442");
        session.command(Command::Cancel);

        // 换选先撤销最后一次锁定：光标在键入的数字上时位置不变，落在拼写补齐的数字里时回到末尾。
        type_digits(&mut session, "64");
        session.choose_spelling(spelling_index(&session, "ming"));
        assert_eq!(session.snapshot().editing_text, "6464");
        session.command(Command::MoveHome);
        session.command(Command::MoveRight);
        session.choose_spelling(spelling_index(&session, "ni"));
        let view = session.snapshot();
        assert_eq!(
            (view.preedit.as_str(), view.editing_text.as_str()),
            ("ni", "64")
        );
        assert_eq!(view.caret_position, 1);
        session.choose_spelling(spelling_index(&session, "ming"));
        assert_eq!(session.snapshot().caret_position, 1);
        session.command(Command::MoveEnd);
        session.command(Command::MoveLeft);
        assert_eq!(session.snapshot().caret_position, 3);
        session.choose_spelling(spelling_index(&session, "ni"));
        let view = session.snapshot();
        assert_eq!(view.editing_text, "64");
        assert_eq!(view.caret_position, 2);
        session.command(Command::Cancel);

        // 取消把锁定、撤销记录、首字母和光标一起清掉。
        type_digits(&mut session, "64426");
        session.choose_spelling(spelling_index(&session, "ni"));
        session.choose_spelling(spelling_index(&session, "H"));
        session.command(Command::MoveHome);
        assert!(session.command(Command::Cancel).handled);
        assert!(session.locked.is_empty() && session.lock_undo.is_empty());
        assert!(session.initial.is_none() && session.caret.is_none());
        type_digits(&mut session, "64");
        let view = session.snapshot();
        assert_eq!(view.preedit, "64");
        assert_eq!(view.caret_position, 2);
        session.command(Command::Cancel);

        // 左列的数字直接上屏第一位，与选中候选一样光标回到末尾。
        type_digits(&mut session, "64426");
        session.command(Command::MoveHome);
        session.command(Command::MoveRight);
        let result = session.choose_spelling(spelling_index(&session, "6"));
        assert_eq!(result.commit.as_deref(), Some("6"));
        let view = session.snapshot();
        assert_eq!(view.editing_text, "4426");
        assert_eq!(view.caret_position, 4);
    }

    #[test]
    fn learning_and_management_use_canonical_keys() {
        let fixture = fixture();
        let mut session = open(&fixture.paths, false, mixed());
        type_digits(&mut session, "64");
        assert_eq!(
            session.select(index_of(&session, "米")).commit.as_deref(),
            Some("米")
        );
        let mut unchanged = open(&fixture.paths, false, mixed());
        type_digits(&mut unchanged, "64");
        assert_eq!(
            words(&unchanged)[0],
            "你",
            "disabled learning changed ranking"
        );

        let mut learner = open(&fixture.paths, true, mixed());
        type_digits(&mut learner, "64");
        let learned = learner.select(index_of(&learner, "米"));
        assert_eq!(learned.commit.as_deref(), Some("米"));
        assert_eq!(learned.diagnostic, None);

        let mut managed = open(&fixture.paths, false, mixed());
        type_digits(&mut managed, "64");
        assert_eq!(words(&managed)[0], "米", "learning did not persist");
        let pinned = managed.pin(index_of(&managed, "你"));
        assert_eq!(pinned, KeyResult::handled());
        assert_eq!(managed.snapshot().editing_text, "64");
        assert_eq!(words(&managed)[0], "你");
        assert!(
            !managed.remove(index_of(&managed, "你")).handled,
            "single character was removed"
        );
        let ogham = index_of(&managed, "ogham");
        assert!(
            !managed.pin(ogham).handled,
            "an English row is not a pinyin dictionary row"
        );
        assert!(!managed.pin(999).handled && !managed.remove(999).handled);
        assert!(!managed.set_position(0, 6).handled && !managed.set_position(0, -1).handled);
        assert!(!managed.set_position(999, 0).handled);
        assert_eq!(
            managed.set_position(index_of(&managed, "米"), 1),
            KeyResult::handled()
        );
        assert_eq!(words(&managed)[0], "米", "fixed position not applied");

        let mut fixed = open(&fixture.paths, false, mixed());
        type_digits(&mut fixed, "64");
        assert_eq!(words(&fixed)[0], "米", "fixed position not persisted");
        let ni = fixed
            .snapshot()
            .nine_key_spellings
            .iter()
            .position(|s| s == "ni")
            .expect("ni offered");
        fixed.choose_spelling(ni);
        assert_eq!(
            words(&fixed)[0],
            "你",
            "fixed position escaped the spelling constraint"
        );

        assert_eq!(
            managed.set_position(index_of(&managed, "米"), 0),
            KeyResult::handled()
        );
        assert_eq!(words(&managed)[0], "你", "clear fixed position failed");
        managed.command(Command::Cancel);
        type_digits(&mut managed, "64426");
        let removed = managed.remove(index_of(&managed, "你好"));
        assert_eq!(removed, KeyResult::handled());
        assert_eq!(managed.snapshot().editing_text, "64426");

        let mut after_removal = open(&fixture.paths, false, mixed());
        type_digits(&mut after_removal, "64426");
        // Only the dictionary row is gone; the two-syllable lattice may still compose 你好 from 你 and 好.
        assert!(
            !after_removal
                .snapshot()
                .candidates
                .iter()
                .any(|item| item.word == "你好" && item.source.is_dictionary()),
            "phrase removal did not persist"
        );
    }

    #[test]
    fn failed_learning_keeps_the_commit() {
        let fixture = fixture();
        let mut paths = fixture.paths.clone();
        paths.user_data = fixture.directory.path().join("blocked");
        std::fs::create_dir_all(paths.user(assets::USER_JOURNAL)).expect("block the journal");
        let mut failing = open(&paths, true, mixed());
        type_digits(&mut failing, "64");
        let result = failing.finish(index_of(&failing, "米"));
        assert_eq!(result.commit.as_deref(), Some("米"));
        assert_eq!(
            result.diagnostic.as_deref(),
            Some(diagnostics::NINE_KEY_FREQUENCY_NOT_PERSISTED)
        );
        assert!(!failing.active() && failing.snapshot().preedit.is_empty());
    }

    #[test]
    fn english_only_spells_words_from_the_first_digit() {
        let fixture = fixture();
        let options = EnglishInputOptions {
            mixed_candidates: false,
            minimum_prefix: 2,
        };
        let mut words_only = open(&fixture.paths, false, options);
        words_only.set_english_only(true);
        type_digits(&mut words_only, "65");
        let view = words_only.snapshot();
        assert_eq!(words(&words_only), ["ok", "old", "older"]);
        assert!(
            view.nine_key_spellings.is_empty(),
            "english nine-key offered pinyin spellings"
        );
        assert!(
            view.dedicated_english,
            "the grid's snapshot dropped the English mode"
        );
        words_only.command(Command::Cancel);
        type_digits(&mut words_only, "653");
        assert_eq!(words(&words_only), ["old", "older"]);
        words_only.command(Command::Cancel);

        // One digit is enough: the prefix length that quiets a mixed list means nothing when words are the whole list.
        type_digits(&mut words_only, "6");
        let result = words_only.select(index_of(&words_only, "old"));
        assert_eq!(result.commit.as_deref(), Some("old"));
        assert!(!words_only.active());

        // Leaving English brings syllables back, and English rows stay off without mixed candidates.
        words_only.set_english_only(false);
        type_digits(&mut words_only, "64");
        // 你好 是 64 的简拼 n'h，排在音节行之后。
        assert_eq!(words(&words_only), ["你", "米", "你好"]);
        assert!(!words_only.snapshot().nine_key_spellings.is_empty());
        assert!(!words_only.snapshot().dedicated_english);
        // Switching mid-composition requeries the same digits.
        words_only.set_english_only(true);
        assert_eq!(words(&words_only), ["ogham"]);
    }

    /// 77 拼不出任何音节：混输关着也给英文词，不然这串数字一行候选都没有。拼音有读法时混输开关照旧说了算。
    #[test]
    fn digits_without_a_reading_offer_english_even_with_mixing_off() {
        let fixture = fixture();
        let mut session = open(&fixture.paths, false, EnglishInputOptions::default());
        type_digits(&mut session, "77");
        assert_eq!(words(&session), ["QQ"]);
        session.command(Command::Cancel);
        type_digits(&mut session, "64426");
        assert!(!words(&session).contains(&"ogham".to_owned()));
    }

    #[test]
    fn a_missing_english_dictionary_means_no_english_rows() {
        let fixture = fixture();
        std::fs::remove_file(fixture.paths.dictionary(assets::ENGLISH_DICTIONARY))
            .expect("remove msime-english.db");
        let mut session = open(&fixture.paths, false, mixed());
        type_digits(&mut session, "64");
        assert_eq!(words(&session), ["你", "米", "你好"]);
        session.set_english_only(true);
        assert!(words(&session).is_empty());
    }

    const INITIALS_FIXTURE: &str = "CREATE TABLE tbl_1_m(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_1_m VALUES('mu','m','木',100);CREATE TABLE tbl_1_o(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_1_o VALUES('ou','o','欧',50);CREATE TABLE tbl_1_w(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_1_w VALUES('wo','w','我',100);CREATE TABLE tbl_1_d(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_1_d VALUES('di','d','滴',60);CREATE TABLE tbl_1_g(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_1_g VALUES('ge','g','个',100);CREATE TABLE tbl_1_t(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_1_t VALUES('tian','t','天',100);CREATE TABLE tbl_1_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_1_n VALUES('na','n','呐',40);CREATE TABLE tbl_2_m(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_2_m VALUES('mei''tian','mt','每天',900);INSERT INTO tbl_2_m VALUES('ming''tian','mt','明天',800);CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_2_n VALUES('na''tian','nt','那天',300);CREATE TABLE tbl_2_w(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_2_w VALUES('wo''di','wd','我滴',30);CREATE TABLE tbl_3_g(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_3_g VALUES('ge''tian''na','gtn','个天呐',20);CREATE TABLE tbl_4_c(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_4_c VALUES('chi''fan''le''mei','cflm','吃饭了没',100);CREATE TABLE tbl_4_g(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_4_g VALUES('guan''guan''ju''jiu','ggjj','关关雎鸠',1605);CREATE TABLE tbl_5_w(key TEXT,jp TEXT,value TEXT,weight INTEGER);";

    fn type_keys(session: &mut NineKeySession, keys: &str) {
        for key in keys.bytes() {
            assert!(session.character(key).handled, "key {key} unhandled");
        }
    }

    /// #5640：每个数字当一个音节的首字母也能出词，词库里的词和用户自己存的词都算。
    #[test]
    fn digits_spell_words_by_their_initials() {
        let fixture = fixture_with(INITIALS_FIXTURE);
        let mut session = open(&fixture.paths, false, EnglishInputOptions::default());

        // 不打切分时 68 也可能是一个音节（mu、ou），音节行在前，简拼行按权重跟在后面。
        type_digits(&mut session, "68");
        assert_eq!(words(&session), ["木", "欧", "每天", "明天", "那天"]);
        assert_eq!(session.snapshot().nine_key_reading, "mu");
        session.command(Command::Cancel);

        // 每个数字之间都打了切分，说的就是简拼：简拼行排到前面，读音行显示首字母。
        type_keys(&mut session, "6'8");
        assert_eq!(words(&session)[..3], ["每天", "明天", "那天"]);
        assert_eq!(session.snapshot().nine_key_reading, "m't");
        let chosen = session.select(index_of(&session, "明天"));
        assert_eq!(chosen.commit.as_deref(), Some("明天"));
        assert!(!session.active(), "an initials row consumes every digit");

        // 没有哪条音节切分能拼满 2356，简拼的 吃饭了没 直接排第一。
        type_digits(&mut session, "2356");
        assert_eq!(words(&session)[0], "吃饭了没");
        assert_eq!(session.snapshot().nine_key_reading, "c'f'l'm");
        session.command(Command::Cancel);
        type_digits(&mut session, "4455");
        assert_eq!(words(&session)[0], "关关雎鸠");
        session.command(Command::Cancel);

        // 末尾的切分说 68 是一个音节，不是两个首字母。
        type_keys(&mut session, "68'");
        assert!(!words(&session).iter().any(|word| word == "每天"));
        session.command(Command::Cancel);
        // 6'84：84 是一个音节，也不按简拼查。
        type_keys(&mut session, "6'84");
        assert!(!words(&session).iter().any(|word| word.chars().count() == 3));
        session.command(Command::Cancel);

        // 选过拼音之后不查简拼：锁定的音节已经说明了怎么切。
        type_digits(&mut session, "68");
        let mu = session
            .snapshot()
            .nine_key_spellings
            .iter()
            .position(|spelling| spelling == "mu")
            .expect("mu offered");
        session.choose_spelling(mu);
        assert!(!words(&session).iter().any(|word| word == "每天"));
    }

    /// #5640：两位数字的音节行比候选上限还多时（出货词库里 `68` 有两百多个单字），不打切分的简拼行也不能被截掉：前三个音节行之后，其余音节行和简拼行按权重归并。
    #[test]
    fn initials_rows_survive_more_syllable_rows_than_the_limit() {
        let mut main = INITIALS_FIXTURE.to_owned();
        for index in 0..CANDIDATE_LIMIT + 72 {
            let weight = 2000 - 10 * index as i64;
            main.push_str(&format!(
                "INSERT INTO tbl_1_m VALUES('mu','m','木{index}',{weight});"
            ));
        }
        let fixture = fixture_with(&main);
        let mut session = open(&fixture.paths, false, EnglishInputOptions::default());
        type_digits(&mut session, "68");
        let listed = words(&session);
        assert_eq!(listed.len(), CANDIDATE_LIMIT);
        // 权重最高的三个单字留在最前；木3 的权重 1970 比每天高，仍在每天前面。
        assert_eq!(listed[..5], ["木0", "木1", "木2", "木3", "木4"]);
        let position = |word: &str| {
            listed
                .iter()
                .position(|candidate| candidate == word)
                .unwrap_or_else(|| panic!("{word} was cut from {listed:?}"))
        };
        // 每天 900、明天 800 排在权重比它们低的单字之前；权重相同时音节行在前。
        assert_eq!(position("每天"), position("木110") + 1);
        assert_eq!(position("明天"), position("木120") + 1);
        assert_eq!(session.snapshot().nine_key_reading, "mu");
        session.command(Command::Cancel);

        // 打了切分时简拼行照旧排第一。
        type_keys(&mut session, "6'8");
        assert_eq!(words(&session)[..3], ["每天", "明天", "那天"]);
    }

    /// #5640：九宫格里分段选出来的词和选中的整句会存成用户词，下次打简拼就能出来。
    #[test]
    fn learned_phrases_come_back_by_their_initials() {
        let fixture = fixture_with(INITIALS_FIXTURE);
        let mut session = open(&fixture.paths, true, EnglishInputOptions::default());
        // wo'di'ge'tian'na 分两段选：我滴 + 个天呐。
        type_digits(&mut session, "963443842662");
        let first = session.select(index_of(&session, "我滴"));
        assert_eq!(first.commit.as_deref(), Some("我滴"));
        assert_eq!(first.diagnostic, None);
        assert!(session.active());
        let rest = session.select(index_of(&session, "个天呐"));
        assert_eq!(rest.commit.as_deref(), Some("个天呐"));
        assert_eq!(rest.diagnostic, None);
        assert!(!session.active());

        let mut later = open(&fixture.paths, false, EnglishInputOptions::default());
        type_keys(&mut later, "9'3'4'8'6");
        assert_eq!(words(&later)[0], "我滴个天呐");
        assert_eq!(later.snapshot().nine_key_reading, "w'd'g't'n");
        later.command(Command::Cancel);
        // 不打切分也找得到。
        type_digits(&mut later, "93486");
        assert!(words(&later).iter().any(|word| word == "我滴个天呐"));
    }

    /// 不学习或取消时不造词；选中词库里本来就有的整词也不重复写。
    #[test]
    fn only_chosen_pieces_become_words() {
        let fixture = fixture_with(INITIALS_FIXTURE);
        let mut quiet = open(&fixture.paths, false, EnglishInputOptions::default());
        type_digits(&mut quiet, "963443842662");
        quiet.select(index_of(&quiet, "我滴"));
        quiet.select(index_of(&quiet, "个天呐"));
        type_digits(&mut quiet, "93486");
        assert!(
            !words(&quiet).iter().any(|word| word == "我滴个天呐"),
            "learning off still stored"
        );
        quiet.command(Command::Cancel);

        let mut session = open(&fixture.paths, true, EnglishInputOptions::default());
        type_digits(&mut session, "963443842662");
        session.select(index_of(&session, "我滴"));
        session.command(Command::Cancel);
        type_digits(&mut session, "843");
        session.command(Command::Cancel);
        type_digits(&mut session, "93486");
        assert!(
            !words(&session).iter().any(|word| word == "我滴个天呐"),
            "a cancelled composition stored a phrase"
        );
    }

    /// 与全拼键盘的 `finish_composition` 相同：先选掉一段再打标点（`finish`），用户选的段和替他选的余下部分连成一个词存起来。
    #[test]
    fn finishing_stores_the_chosen_pieces_with_the_rest() {
        let fixture = fixture_with(INITIALS_FIXTURE);
        let mut session = open(&fixture.paths, true, EnglishInputOptions::default());
        type_digits(&mut session, "963443842662");
        session.select(index_of(&session, "我滴"));
        assert_eq!(words(&session)[0], "个天呐");
        let rest = session.finish(0);
        assert_eq!(rest.commit.as_deref(), Some("个天呐"));
        assert_eq!(rest.diagnostic, None);
        assert!(!session.active());
        type_keys(&mut session, "9'3'4'8'6");
        assert_eq!(words(&session)[0], "我滴个天呐");
    }

    /// 会话不允许全拼时九宫格只拼英文：没有音节、没有拼音行，拼音词库也不打开。
    #[test]
    fn without_quanpin_the_grid_spells_english_only() {
        let fixture = fixture();
        let mut session = NineKeySession::new(
            &fixture.paths,
            false,
            FrequencyAdjustmentOptions::default(),
            FuzzyPinyinOptions::default(),
            mixed(),
            false,
            false,
        );
        type_digits(&mut session, "64");
        assert!(session.snapshot().nine_key_spellings.is_empty());
        assert!(!words(&session).contains(&"你".to_owned()));
        assert!(session.dictionary.is_none());

        let mut pinyin = open(&fixture.paths, false, mixed());
        type_digits(&mut pinyin, "64");
        assert!(words(&pinyin).contains(&"你".to_owned()));
        assert!(pinyin.dictionary.is_some());
    }

    // ---- emoji、颜文字混排（#5848） ----

    /// 每个音节都有单字，再加上 美国、警告、香蕉 这几个词，让 `634486`、`5464426`、`94264` 都有覆盖整串数字的拼音候选；`54` 是 #5667 截图里的 里 李 鸡 几。
    const EXPRESSIVE_MAIN_FIXTURE: &str = "CREATE TABLE tbl_1_m(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_1_m VALUES('mei','m','美',300);CREATE TABLE tbl_1_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_1_n VALUES('nei','n','内',200);CREATE TABLE tbl_1_g(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_1_g VALUES('guo','g','国',300),('gao','g','高',200);CREATE TABLE tbl_2_m(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_2_m VALUES('mei''guo','mg','美国',1000);CREATE TABLE tbl_1_j(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_1_j VALUES('jing','j','警',100),('ji','j','鸡',300),('ji','j','几',200);CREATE TABLE tbl_1_l(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_1_l VALUES('li','l','里',500),('li','l','李',400);CREATE TABLE tbl_2_j(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_2_j VALUES('jing''gao','jg','警告',900);CREATE TABLE tbl_1_x(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_1_x VALUES('xian','x','先',300),('xiang','x','香',200),('xi','x','西',250);CREATE TABLE tbl_2_x(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_2_x VALUES('xiang''jiao','xj','香蕉',800);";

    /// 结构与随包 `msime-others.db` 相同的编码表和目录表，🇺🇲、🇺🇸、⚠️、🍌、🐔 的编码、次序和关键词取自随包数据。`警告` 也写成一个 emoji，`meihuo` 也指向 🇺🇸，用来验证与拼音候选重复的、跨读法重复的都只出现一次。
    const EXPRESSIVE_OTHERS_FIXTURE: &str = "CREATE TABLE emoji_pinyin(key TEXT,emoji TEXT,sort_order INTEGER);INSERT INTO emoji_pinyin VALUES('meiguo','🇺🇸',1893),('meihuo','🇺🇸',1893),('meiguobentuwaixiaodaoyu','🇺🇲',1891),('jinggao','警告',1),('jinggao','⚠️',1433),('xiangjiao','🍌',725),('ji','🐔',626),('jitou','🐔',626),('laugh','😀',10),('mei','🌸',2000);CREATE TABLE emoji(emoji TEXT PRIMARY KEY,keywords TEXT);INSERT INTO emoji VALUES('🇺🇸','美国 美利坚 美利坚合众国 星条旗 flag: united states'),('🇺🇲','美国本土外小岛屿 flag: u.s. outlying islands'),('⚠️','警告 注意 危险 预警 warning'),('🍌','香蕉 banana'),('🐔','鸡 鸡头 chicken'),('😀','笑脸 laugh'),('🌸','樱花 梅花 Mei');CREATE TABLE kaomoji(pinyin TEXT,jianpin TEXT,kaomoji TEXT,sort_order INTEGER);INSERT INTO kaomoji VALUES('meiguo','mg','(•̀ᴗ•́)و',10),('jinggao','jg','(ﾟДﾟ≡ﾟдﾟ)!?',20);CREATE TABLE kaomoji_catalog(kaomoji TEXT PRIMARY KEY,keywords TEXT);INSERT INTO kaomoji_catalog VALUES('(•̀ᴗ•́)و','mei guo'),('(ﾟДﾟ≡ﾟдﾟ)!?','jing gao 警告');";

    const BOTH_EXPRESSIVE: MixedExpressiveOptions = MixedExpressiveOptions {
        emoji_candidates: true,
        kaomoji_candidates: true,
    };

    fn expressive_fixture() -> Fixture {
        let fixture = fixture_with(EXPRESSIVE_MAIN_FIXTURE);
        Connection::open(fixture.paths.resource(assets::OTHER_DICTIONARY))
            .and_then(|db| db.execute_batch(EXPRESSIVE_OTHERS_FIXTURE))
            .expect("others fixture");
        fixture
    }

    fn open_expressive(
        fixture: &Fixture,
        english: EnglishInputOptions,
        expressive: MixedExpressiveOptions,
    ) -> NineKeySession {
        let mut session = open(&fixture.paths, true, english);
        session.set_mixed_expressive(expressive);
        session
    }

    fn sources(session: &NineKeySession) -> Vec<CandidateSource> {
        session.snapshot().candidate_sources
    }

    fn has_expressive_rows(session: &NineKeySession) -> bool {
        sources(session)
            .iter()
            .any(|source| matches!(source, CandidateSource::Emoji | CandidateSource::Kaomoji))
    }

    #[test]
    fn digits_put_emoji_and_kaomoji_after_the_words_they_depict() {
        let fixture = expressive_fixture();
        let mut session =
            open_expressive(&fixture, EnglishInputOptions::default(), BOTH_EXPRESSIVE);

        // #5667：美国 后面紧跟 🇺🇸。🇺🇲 只是编码以 meiguo 开头，画的不是 美国，与接不上任何词的颜文字一起排在末尾（#5907）；mei'guo 和 mei'huo 两种读法都查到 🇺🇸，只出现一次。
        type_digits(&mut session, "634486");
        let view = session.snapshot();
        let listed = words(&session);
        assert_eq!(listed[..2], ["美国", "🇺🇸"]);
        assert_eq!(view.candidate_sources[1], CandidateSource::Emoji);
        assert_eq!(listed[listed.len() - 2..], ["🇺🇲", "(•̀ᴗ•́)و"]);
        assert_eq!(
            view.candidate_sources[listed.len() - 1],
            CandidateSource::Kaomoji
        );
        assert_eq!(listed.iter().filter(|word| *word == "🇺🇸").count(), 1);
        assert!(view.candidate_answers_key[1]);
        assert_eq!(view.candidates[1].pinyin, "634486");
        session.command(Command::Cancel);

        // #5667 的截图：警告 ⚠️，颜文字的关键词也有 警告，接在 emoji 后面；emoji 里的「警告」与拼音候选重复，不再出现。
        type_digits(&mut session, "5464426");
        assert_eq!(words(&session)[..3], ["警告", "⚠️", "(ﾟДﾟ≡ﾟдﾟ)!?"]);
        assert_eq!(
            words(&session)
                .iter()
                .filter(|word| *word == "警告")
                .count(),
            1
        );
        session.command(Command::Cancel);

        // #5667 的截图：54 是 里 李 鸡 🐔 几。🐔 的编码就是 ji，在 ji 开头的两百多行里按目录顺序靠后，靠编码完全相同的行先取才取得到。
        type_digits(&mut session, "54");
        assert_eq!(words(&session)[..4], ["里", "李", "鸡", "🐔"]);
        session.command(Command::Cancel);

        // 只开 emoji：颜文字不出现。
        let mut emoji_only = open_expressive(
            &fixture,
            EnglishInputOptions::default(),
            MixedExpressiveOptions {
                emoji_candidates: true,
                kaomoji_candidates: false,
            },
        );
        type_digits(&mut emoji_only, "634486");
        assert_eq!(words(&emoji_only)[..2], ["美国", "🇺🇸"]);
        assert_eq!(words(&emoji_only).last().map(String::as_str), Some("🇺🇲"));
        assert!(!sources(&emoji_only).contains(&CandidateSource::Kaomoji));
    }

    #[test]
    fn expressive_rows_follow_a_leading_english_word() {
        let fixture = expressive_fixture();
        Connection::open(fixture.paths.dictionary(assets::ENGLISH_DICTIONARY))
            .unwrap()
            .execute(
                "INSERT INTO english_words(word, display, weight) VALUES ('mei', 'Mei', 700)",
                [],
            )
            .unwrap();
        let mut session = open_expressive(&fixture, mixed(), BOTH_EXPRESSIVE);
        // 九宫格的英文首行有权重时占首选之后的位置；关键词里有这个英文词的 emoji 接在它后面，接不上的排在末尾。
        type_digits(&mut session, "634");
        let listed = words(&session);
        assert_eq!(listed[..3], ["美", "Mei", "🌸"]);
        assert_eq!(listed[listed.len() - 3..], ["🇺🇲", "🇺🇸", "(•̀ᴗ•́)و"]);
    }

    #[test]
    fn both_switches_off_leave_the_list_unchanged() {
        let fixture = expressive_fixture();
        for digits in ["634486", "5464426", "94264", "63", "6"] {
            let mut off = open_expressive(&fixture, mixed(), MixedExpressiveOptions::default());
            let mut on = open_expressive(&fixture, mixed(), BOTH_EXPRESSIVE);
            type_digits(&mut off, digits);
            type_digits(&mut on, digits);
            assert!(!has_expressive_rows(&off), "{digits}");
            // 开关打开时去掉混入的行，剩下的与关闭时逐项相同：混排只插行，不动别的候选。
            let mut without_expressive = on.snapshot().candidates;
            without_expressive.retain(|item| {
                !matches!(
                    item.source,
                    CandidateSource::Emoji | CandidateSource::Kaomoji
                )
            });
            assert_eq!(without_expressive, off.snapshot().candidates, "{digits}");
        }
    }

    #[test]
    fn expressive_rows_need_two_digits_and_pinyin() {
        let fixture = expressive_fixture();
        let mut session =
            open_expressive(&fixture, EnglishInputOptions::default(), BOTH_EXPRESSIVE);
        // 一个数字的读法 m 已经是 meiguo 的前缀，但与 26 键一样不足两个时不匹配。
        type_digits(&mut session, "6");
        assert!(!has_expressive_rows(&session));
        type_digits(&mut session, "3");
        assert!(words(&session).contains(&"🇺🇸".to_owned()));
        session.command(Command::Cancel);

        // 九键纯英文模式只拼英文词。
        session.set_english_only(true);
        type_digits(&mut session, "634486");
        assert!(!has_expressive_rows(&session));
        session.set_english_only(false);
        session.command(Command::Cancel);

        // 单字、笔画筛选针对的是汉字。
        type_digits(&mut session, "634486");
        assert!(session.set_filter(true, "").handled);
        assert!(!has_expressive_rows(&session));
        session.command(Command::Cancel);

        // 会话不允许全拼时九宫格只拼英文。
        let mut english_grid = NineKeySession::new(
            &fixture.paths,
            false,
            FrequencyAdjustmentOptions::default(),
            FuzzyPinyinOptions::default(),
            mixed(),
            false,
            false,
        );
        english_grid.set_mixed_expressive(BOTH_EXPRESSIVE);
        type_digits(&mut english_grid, "634486");
        assert!(!has_expressive_rows(&english_grid));
    }

    #[test]
    fn expressive_rows_follow_chosen_syllables_and_splits() {
        let fixture = expressive_fixture();
        let mut session =
            open_expressive(&fixture, EnglishInputOptions::default(), BOTH_EXPRESSIVE);

        // 在拼音选择条上选定 nei：只剩 nei 开头的读法，🇺🇸 只对应 mei'guo，不再出现。
        type_digits(&mut session, "634486");
        let nei = spelling_index(&session, "nei");
        assert!(session.choose_spelling(nei).handled);
        assert!(!words(&session).contains(&"🇺🇸".to_owned()));
        session.command(Command::Cancel);
        type_digits(&mut session, "634486");
        let mei = spelling_index(&session, "mei");
        assert!(session.choose_spelling(mei).handled);
        assert!(words(&session).contains(&"🇺🇸".to_owned()));
        session.command(Command::Cancel);

        // 94264 可以是 xiang，🍌（xiangjiao）出现。
        type_digits(&mut session, "94264");
        assert!(words(&session).contains(&"🍌".to_owned()));
        session.command(Command::Cancel);
        // 先选定 xian 再打 4：字母同样是 xiang，但 xian 之后的 gjiao 切不成音节，🍌 不出现。
        type_digits(&mut session, "9426");
        let xian = spelling_index(&session, "xian");
        assert!(session.choose_spelling(xian).handled);
        type_digits(&mut session, "4");
        assert!(!words(&session).contains(&"🍌".to_owned()));
        session.command(Command::Cancel);
        // 用「分词」键在 xian 后面切开也一样。
        type_keys(&mut session, "9426'4");
        assert!(!words(&session).contains(&"🍌".to_owned()));
    }

    #[test]
    fn choosing_an_expressive_row_commits_it_and_ends_the_composition() {
        let fixture = expressive_fixture();
        let mut session =
            open_expressive(&fixture, EnglishInputOptions::default(), BOTH_EXPRESSIVE);
        type_digits(&mut session, "634486");
        let chosen = session.select(index_of(&session, "🇺🇸"));
        assert_eq!(chosen.commit.as_deref(), Some("🇺🇸"));
        assert_eq!(chosen.diagnostic, None);
        assert!(!session.active());
        // 不是词库行：不能置顶、删除或固定位置，也不会被学成拼音词。
        type_digits(&mut session, "634486");
        let index = index_of(&session, "🇺🇸");
        assert!(!session.pin(index).handled);
        assert!(!session.remove(index).handled);
        assert!(!session.set_position(index, 2).handled);
        assert_eq!(
            sources(&session)
                .iter()
                .zip(words(&session))
                .filter(|(_, word)| word == "🇺🇸")
                .map(|(source, _)| *source)
                .collect::<Vec<_>>(),
            [CandidateSource::Emoji]
        );
    }

    #[test]
    fn expressive_readings_are_capped_and_led_by_the_candidates() {
        let fixture = expressive_fixture();
        let mut session =
            open_expressive(&fixture, EnglishInputOptions::default(), BOTH_EXPRESSIVE);
        type_digits(&mut session, "94264");
        let table = spelling_table();
        let mut every: Vec<String> = table
            .paths("94264")
            .into_iter()
            .map(|path| path.concat())
            .collect();
        every.sort();
        every.dedup();
        assert!(every.len() > EXPRESSIVE_READING_LIMIT, "{every:?}");

        let readings = session.expressive_readings(&table.paths("94264"), 5, &session.candidates);
        assert_eq!(readings.len(), EXPRESSIVE_READING_LIMIT);
        // 排在最前的拼音候选 香（xiang）支持的读法查在最前。
        assert_eq!(session.candidates[0].word, "香");
        assert_eq!(readings[0], "xiang");
        // 只拼了开头一个音节的路径不算读法。
        assert!(readings.iter().all(|reading| reading.len() == 5));
    }

    #[test]
    fn expressive_keys_must_split_at_the_fixed_boundaries() {
        assert!(splits_into_syllables_at("xiangjiao", &[]));
        assert!(splits_into_syllables_at("meiguo", &[3]));
        assert!(splits_into_syllables_at("meiguo", &[6]));
        assert!(splits_into_syllables_at("meiguobentuwaixiaodaoyu", &[3, 6]));
        // 编码不记音节边界，xi'ang'jiao 也是一种切法。
        assert!(splits_into_syllables_at("xiangjiao", &[2]));
        assert!(!splits_into_syllables_at("xiangjiao", &[4]));
        assert!(!splits_into_syllables_at("meiguo", &[2]));
        assert!(!splits_into_syllables_at("laugh", &[2]));
        assert!(!splits_into_syllables_at("mei", &[4]));
        assert!(letters_start_with("mei'guo", "meig"));
        assert!(!letters_start_with("mei", "meig"));
    }
}
