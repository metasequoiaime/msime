//! T9 digit composition over quanpin syllables plus English T9 (core-session.md §8). Active while it holds digits; `Session` routes to it then.
//!
//! The digits stay the composition: a chosen spelling only rewrites its span of digits and is remembered in `locked`, and every candidate's `pinyin` is the run of digits it consumes, so selection advances the same way whichever reading produced the row.

use std::cmp::{Ordering, Reverse};
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
use crate::lattice::neural::{
    shared_sentence_model, NeuralReranker, CONTEXT_CHARACTERS, MAX_RERANK_PATHS,
};
use crate::lattice::personal::PersonalTransition;
use crate::lattice::SentencePath;
use crate::local::date_time::{inline_date_time_keyword, insert_inline_date_time};
use crate::local::emoji::{query_emoji_readings, query_kaomoji_readings, ExpressiveRow};
use crate::paths::RuntimePaths;
use crate::pinyin::segment::{cut_one_piece_min_segments, split_segments};
use crate::pinyin::syllables::intact_pinyin_list;
use crate::quanpin::QuanpinDictionary;
use crate::session::{LocalClock, SessionSnapshot};
use crate::stroke;
use crate::text::{count_utf8_chars, is_all_han, is_han_phrase, last_characters};
use crate::types::{
    CandidateSource, Command, EnglishInputOptions, FrequencyAdjustmentMode,
    FrequencyAdjustmentOptions, FuzzyPinyinOptions, KeyResult, LocalInputMode,
    MixedExpressiveOptions, PersonalDictionaryKind, SchemeType, SentenceAssociationOptions,
    WordItem,
};
use crate::user_dictionary::positions;
use crate::user_dictionary::ranking::{self, RankingRequest};
use crate::user_dictionary::removal;

pub const PATH_LIMIT: usize = 48;
pub const DIGIT_LIMIT: usize = 32;
pub const CANDIDATE_LIMIT: usize = 128;
/// 每次刷新从排在前面、覆盖全部数字的行里取几条切分当种子，见 `seeded_paths`。
const SENTENCE_SEED_LIMIT: usize = 4;
/// 上一次的种子仍对得上数字时一起留着，合计最多这么多条。只留这一次的种子时，末尾几个数字的噪声会把前面已经拼对的切分挤掉。
const SEED_RETAIN_LIMIT: usize = 8;
/// 每条种子接几条余下数字的切分。
const SEED_TAIL_LIMIT: usize = 3;
/// 有种子时，数字至少这么长才把按音节频度留的切分截到 `SEEDED_PATH_LIMIT` 条：长串的好切分主要靠种子补，频度切分截短省下的查询抵掉种子的开销；短串（词级输入）照旧留满 `PATH_LIMIT` 条。
const SEEDED_TRUNCATE_DIGITS: usize = 12;
const SEEDED_PATH_LIMIT: usize = 24;
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
/// 用户用过的简拼词可能排在按权重的前 `INITIALS_ROW_LIMIT` 行之外（出货词库里 9'7 的 隐私 前面有 284 行更重的；9'9'2'9 的 仔细查找 权重 100，前面有 246 行更重的、三百多行同样是 100）。个人上下文模型有记录时，每张首字母表各取权重最高的这么多行（`query_jianpin_codes_per_table`，九宫格的码最多分在四张表里），用过的词先留下，再截到 `INITIALS_ROW_LIMIT`（#6185）。
const INITIALS_SCAN_LIMIT: usize = 512;
/// 九键选中一个词时记进个人上下文模型的次数，与 26 键显式选词的 `session::learning::DICTIONARY_PICK_TIMES` 相同（`session/tests.rs` 核对两者一致）。简拼行按每 `PERSONAL_PICK_TIMES * trigger_count` 个计数算调频的一次触发，见 `boost_used_initials`。
pub(crate) const PERSONAL_PICK_TIMES: u32 = 2;
/// `NineKeySession::rerank_sentences` 缓存的重排结果条数。退格回到前一串数字、选词或筛选后刷新时，同一组整句和上下文会再出现；26 键按 series 槽缓存整张重排过的列表，九键只缓存模型给出的顺序，免得每次刷新都再跑一次模型。
const RERANK_CACHE_ENTRIES: usize = 16;
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
    /// 组字里的日期时间行（#5952）读的墙钟；`None` 时不出这些行。跟日期时间模式同一个开关。
    inline_date_time: Option<LocalClock>,
    /// 句子联想设置（`SessionOptions::sentence_association`），与 26 键相同：`word_lattice` 关掉时不出词网格整句行，`neural_keyboard` 打开时用键盘模型给整句重排（#6059）。
    sentence_association: SentenceAssociationOptions,
    /// `SessionOptions::sentence_alternatives`：每条切分交回全部整句读法，而不是只交回最好的一条。
    sentence_alternatives: bool,
    /// 键盘模型读的上屏上下文，只留最后 `CONTEXT_CHARACTERS` 个字；`Session::set_rescoring_context` 随时更新。
    rescoring_context: String,
    /// 键盘模型，`neural_keyboard` 打开后第一次重排时加载；内层 `None` 是模型读不到。九键不让词库在每条切分里各重排一次，见 `rerank_sentences`。
    keyboard: Option<Option<NeuralReranker>>,
    /// 键盘模型最近几次给出的整句顺序，最新的在后，见 `rerank_sentences`。换掉模型（关掉 `neural_keyboard`）时清空。
    rerank_cache: Vec<RerankEntry>,
    /// 把选中的词记进 26 键共用的个人上下文模型（`SessionOptions::personal_context`，还要 `learning`），简拼行据此把用户用过的词排到前面（#6185）。
    personal_context: bool,
    digits: String,
    locked: Vec<String>,
    /// 用户用 `'` 切开音节的数字位置，升序，都在已锁定的部分之后。和锁定的拼写不同，切分只定下一个音节在哪里结束，两边数字的各种读法都还保留：`94'26` 可以是 xi'an，也可以是 yi'an，但不会是 xian。
    splits: Vec<usize>,
    spellings: Vec<String>,
    /// `SessionSnapshot::nine_key_reading`，随候选一起重建。
    reading: String,
    candidates: Vec<WordItem>,
    /// 前几次刷新排在前面、覆盖全部数字的几行的全拼（`'` 连接）。下一次刷新把它们去掉最后一个音节，接上余下数字的切分一起查，见 `seeded_paths`。
    sentence_seeds: Vec<String>,
    /// 算下一批种子用的空缓冲，和 `sentence_seeds` 轮换，刷新时不再分配。
    seed_buffer: Vec<String>,
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
            inline_date_time: None,
            sentence_association: SentenceAssociationOptions::default(),
            sentence_alternatives: false,
            rescoring_context: String::new(),
            keyboard: None,
            rerank_cache: Vec::new(),
            personal_context: true,
            digits: String::new(),
            locked: Vec::new(),
            splits: Vec::new(),
            spellings: Vec::new(),
            reading: String::new(),
            candidates: Vec::new(),
            // 两个缓冲一开始就留够容量，按键时轮换着用，不再分配。
            sentence_seeds: Vec::with_capacity(SEED_RETAIN_LIMIT),
            seed_buffer: Vec::with_capacity(SEED_RETAIN_LIMIT),
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

    /// 数字正好拼出 `riqi`、`sj` 这类关键词时，把当前日期、时间、星期或农历接在对应的词后面，见 `local::date_time::insert_inline_date_time`；`None` 关掉。与 emoji 选项一样只在建会话时设置，测试换时钟时再设一次。
    pub fn set_inline_date_time(&mut self, clock: Option<LocalClock>) {
        self.inline_date_time = clock;
    }

    /// 和 26 键共用的句子联想设置（#6059）。与英文选项一样只在建会话时设置；词库已经打开时同步给它，组字中则按新设置重排候选。
    pub fn set_sentence_options(
        &mut self,
        association: SentenceAssociationOptions,
        alternatives: bool,
    ) {
        if self.sentence_association == association && self.sentence_alternatives == alternatives {
            return;
        }
        self.sentence_association = association;
        self.sentence_alternatives = alternatives;
        if !association.neural_keyboard {
            self.keyboard = None;
            self.rerank_cache.clear();
        }
        let dictionary_association = self.dictionary_association();
        if let Some(dictionary) = self.dictionary.as_mut() {
            dictionary.set_sentence_association(dictionary_association);
            dictionary.set_sentence_alternatives(alternatives);
        }
        if self.active() {
            self.refresh();
        }
    }

    /// 与 26 键的 `InputSession::set_personal_context_enabled` 一起设置：关掉后九键选中的词不再记进个人上下文模型。
    pub fn set_personal_context_enabled(&mut self, enabled: bool) {
        self.personal_context = enabled;
    }

    /// 键盘模型读的上屏上下文，与 26 键的 `QuanpinDictionary::set_rescoring_context` 一样只记下最后 `CONTEXT_CHARACTERS` 个字，不重排已有的候选。
    pub fn set_rescoring_context(&mut self, context: &str) {
        let trimmed = last_characters(context, CONTEXT_CHARACTERS);
        if self.rescoring_context != trimmed {
            self.rescoring_context.clear();
            self.rescoring_context.push_str(trimmed);
        }
    }

    /// 交给词库的句子联想设置。键盘模型不在词库里跑（每条切分各跑一次太贵），而由 `rerank_sentences` 跨切分统一跑一次；它要重排的是词网格整句，所以模型打开时即使用户关了词网格，词库也照样解出整句，挑完再去掉。
    fn dictionary_association(&self) -> SentenceAssociationOptions {
        let association = self.sentence_association;
        SentenceAssociationOptions {
            word_lattice: association.word_lattice || association.neural_keyboard,
            neural_keyboard: false,
            show_next_on_duplicate: association.show_next_on_duplicate,
        }
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
        // 用过的简拼行在九键里的位置只由个人上下文模型里的计数决定（`boost_used_initials`）。这一行会记进模型时不再调全局词频：两边都挪的话一次选词挪两次，全局权重还会连带改掉 26 键里这个词的位置（#6185）。
        let recorded_initials =
            self.records_personal_use(selected_source, &selected_word) && is_initials_row(selected);
        let mut diagnostic = if self.learning
            && self.frequency.mode != FrequencyAdjustmentMode::Disabled
            && index != 0
            && selected_fixed_position == 0
            && !recorded_initials
            && self.editable(index)
        {
            self.adjust_frequency(index, false)
        } else {
            None
        };
        self.consume(selected_pinyin_length);
        if self.learning {
            let (learned, stored) =
                self.learn_selection(selected_source, &selected_canonical_pinyin, &selected_word);
            diagnostic = diagnostic.or(learned);
            let recorded = self.record_personal_use(selected_source, &selected_word, stored);
            diagnostic = diagnostic.or(recorded);
        } else {
            self.reset_phrase();
        }
        self.refresh();
        KeyResult::committed(selected_word).with_diagnostic(diagnostic)
    }

    /// 选中一行之后的造词，与全拼键盘的规则相同：选掉一部分数字时记下这一段；选完全部数字时，前面有选过的段就把各段连成一个词存起来（「我滴」+「个天呐」），没有就只在选中的是整句行（词库里没有的句子）时把整句存起来，最多 `MAX_LEARNED_SENTENCE_SYLLABLES` 个音节。词库里本来就有的词不再写。返回诊断，以及这次存进（或词库里本来就有）的词。
    fn learn_selection(
        &mut self,
        selected_source: CandidateSource,
        selected_canonical_pinyin: &str,
        selected_word: &str,
    ) -> (Option<String>, Option<String>) {
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
            return (None, None);
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
        let Some((pinyin, word)) = stored else {
            return (None, None);
        };
        // 写入前由 `create_word_from_canonical_pinyin` 核对一字一个完整音节（与全拼键盘存词前的检查相同）；读不出的词（夹着英文或符号）被它拒绝，这不是写入失败，不报诊断。
        let association = self.dictionary_association();
        match self
            .dictionary
            .get_or_insert_with(|| {
                open_dictionary(&self.paths, association, self.sentence_alternatives)
            })
            .create_word_from_canonical_pinyin(&pinyin, &word)
        {
            Ok(()) => (None, Some(word)),
            Err(EngineError::InvalidArgument(_)) => (None, None),
            Err(_) if phrase => (Some(diagnostics::PHRASE_NOT_PERSISTED.to_string()), None),
            Err(_) => (Some(diagnostics::SENTENCE_NOT_PERSISTED.to_string()), None),
        }
    }

    /// 把这次选中的词记进个人上下文模型（#6185）：选中的多字汉字词库词，以及这次存进词库的词（分段连成的词组、选中的整句）。不论词库里原来有没有、选的是不是首位都记，简拼行据此把用户用过的词排到前面，26 键选的词也记在同一个模型里。九键没有 26 键那样的上屏词链，前一个词一律当作句首。
    fn record_personal_use(
        &self,
        selected_source: CandidateSource,
        selected_word: &str,
        stored: Option<String>,
    ) -> Option<String> {
        if !self.personal_context {
            return None;
        }
        let mut transitions: Vec<PersonalTransition> = Vec::with_capacity(2);
        let words = selected_source
            .is_dictionary()
            .then_some(selected_word)
            .into_iter()
            .chain(stored.as_deref());
        for word in words {
            if is_personal_word(word)
                && !transitions.iter().any(|transition| transition.word == word)
            {
                transitions.push(PersonalTransition {
                    earlier: String::new(),
                    previous: String::new(),
                    word: word.to_owned(),
                    times: PERSONAL_PICK_TIMES,
                });
            }
        }
        if transitions.is_empty() {
            return None;
        }
        // 候选都来自词库，选得到词时词库一定已经打开。
        let dictionary = self.dictionary.as_ref()?;
        dictionary
            .record_personal_use(&transitions)
            .err()
            .map(|_| diagnostics::PERSONAL_CONTEXT_NOT_PERSISTED.to_string())
    }

    /// 选中的词库词会不会被 `record_personal_use` 记进个人上下文模型：学习和个人上下文都开着，选的是两个字以上的纯汉字词。
    fn records_personal_use(&self, selected_source: CandidateSource, selected_word: &str) -> bool {
        self.learning
            && self.personal_context
            && selected_source.is_dictionary()
            && is_personal_word(selected_word)
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
            self.sentence_seeds.clear();
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
        let unlocked_digit_count = remaining.len();
        let mut split_offsets = [0usize; DIGIT_LIMIT];
        for (index, split) in self.splits.iter().enumerate() {
            split_offsets[index] = split - locked_length;
        }
        let splits = &split_offsets[..self.splits.len()];
        let initial = self.initial.map(|initial| initial.letter);
        let starts_right =
            |piece: &str| initial.is_none_or(|letter| piece.as_bytes().first() == Some(&letter));
        let mut syllables = Vec::new();
        let association = self.dictionary_association();
        let alternatives = if remaining.is_empty() {
            vec![Vec::new()]
        } else {
            let dictionary = self.dictionary.get_or_insert_with(|| {
                open_dictionary(&self.paths, association, self.sentence_alternatives)
            });
            let prior = self
                .prior
                .get_or_insert_with(|| SyllablePrior::from_dictionary(dictionary, table));
            syllables =
                table.spellings_for(remaining, locked_length, splits.first().copied(), prior);
            syllables.retain(|syllable| starts_right(syllable));
            let mut alternatives = table.split_paths(remaining, splits, prior);
            alternatives.retain(|path| path.first().is_none_or(|piece| starts_right(piece)));
            // 种子只在用户没锁定拼写、没打切分、没选首字母时用：那些情况下切分已经被用户限定，种子未必守得住这些限定。
            if self.locked.is_empty() && splits.is_empty() && initial.is_none() {
                if !self.sentence_seeds.is_empty() && remaining.len() >= SEEDED_TRUNCATE_DIGITS {
                    alternatives.truncate(SEEDED_PATH_LIMIT);
                }
                seeded_paths(
                    &mut alternatives,
                    &self.sentence_seeds,
                    remaining,
                    table,
                    prior,
                );
            }
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
            .get_or_insert_with(|| {
                open_dictionary(&self.paths, association, self.sentence_alternatives)
            })
            .row_cache_batch();
        let mut queried = (alternatives.len() > SMALL_QUERY_KEY_BATCH)
            .then(|| HashSet::with_capacity(alternatives.len()));
        // 接管已清空的会话候选向量，刷新时复用行容器容量。
        let mut candidates = std::mem::take(&mut self.candidates);
        candidates.clear();
        let mut key = String::with_capacity(locked_key.len() + self.digits.len() * 4 + 1);
        // 各条切分的前缀组彼此大量重复，一次刷新会推入上万行，见 `push_ranked`。
        let mut leading: HashMap<String, RankKey> = HashMap::new();
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
        let mut boost = None;
        if let Some(codes) = initials
            .then(|| initials_codes(remaining, INITIALS_CODE_LIMIT))
            .flatten()
        {
            // 用户用过的词（个人上下文模型里有计数，26 键选的也算）不能被按权重的截断截掉（#6185）：模型有记录时多扫一些行，用过的先留下。学习或个人上下文关掉时不读个人数据，与 26 键的 `personal_context_applies` 相同。
            let personal =
                self.learning && self.personal_context && !dictionary.personal_model().is_empty();
            let mut rows = if personal {
                dictionary.query_jianpin_codes_per_table(&codes, INITIALS_SCAN_LIMIT)
            } else {
                dictionary.query_jianpin_codes(&codes, INITIALS_ROW_LIMIT)
            };
            let mut used = HashMap::new();
            if personal {
                // 一次查询只取一次读锁，不按行各取一次。
                let model = dictionary.personal_model();
                for row in &rows {
                    let count = model.word_count(&row.word);
                    if count > 0 {
                        used.insert(row.word.clone(), count);
                    }
                }
                drop(model);
                if rows.len() > INITIALS_ROW_LIMIT {
                    // 稳定排序：用过的词在前，两边各自仍按权重。
                    rows.sort_by_key(|row| !used.contains_key(&row.word));
                    rows.truncate(INITIALS_ROW_LIMIT);
                }
            }
            // 排位沿用调频设置：调频关掉时用过的词只保证查得到，不往前挪。
            if self.frequency.mode != FrequencyAdjustmentMode::Disabled && !used.is_empty() {
                boost = Some(InitialsBoost {
                    used,
                    frequency: self.frequency,
                });
            }
            for mut candidate in rows {
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
        rank_candidates(&mut candidates, prefer_exact, initials_lead, boost.as_ref());
        let remaining_length = remaining.len();
        self.rerank_sentences(&mut candidates);
        let mut seeds = std::mem::take(&mut self.seed_buffer);
        next_seeds(
            &candidates,
            &self.digits,
            &mut self.sentence_seeds,
            &mut seeds,
        );
        self.seed_buffer = std::mem::replace(&mut self.sentence_seeds, seeds);
        // emoji、颜文字按拼音查，读法的先后要参照排好的拼音候选，所以在插入英文行之前查；插入在英文行之后，它们也可以接在英文词后面。单字、笔画筛选针对的是汉字，筛选时不混入。
        let (emoji, kaomoji) = if filtering {
            (Vec::new(), Vec::new())
        } else {
            self.expressive_candidates(&alternatives, remaining_length, &candidates)
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
        // 选了首字母或在筛选时用户是在逐字拼，不加日期时间行。
        if let Some(clock) = self
            .inline_date_time
            .as_ref()
            .filter(|_| initial.is_none() && !filtering)
        {
            let digits = self.digits.as_str();
            if let Some((anchor, kind)) =
                inline_date_time_keyword(|keyword| keyword_spells_digits(keyword, digits))
            {
                insert_inline_date_time(&mut candidates, anchor, kind, digits, || clock());
            }
        }
        positions::apply_fixed_positions(
            &self.paths.user(assets::USER_JOURNAL),
            &self.ranking_context(),
            &mut candidates,
            false,
            None,
            false,
        );
        // One digit is predictive; after that, keep a likely reading visible
        // only once its digits are complete. An unfinished longer syllable
        // must not hide readings already spelled in full.
        if let Some(front) = candidates.first() {
            let offset = if locked_key.is_empty() {
                0
            } else {
                locked_key.len() + 1
            };
            if let Some(rest) = front.canonical_pinyin.get(offset..) {
                let preferred = rest.split('\'').next().unwrap_or_default();
                if unlocked_digit_count < 2 || preferred.len() <= unlocked_digit_count {
                    if let Some(found) = self.spellings.iter().position(|s| s == preferred) {
                        self.spellings[..=found].rotate_right(1);
                    }
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

    /// `neural_keyboard` 打开时用键盘模型给整句重排，与 26 键一样多出一行模型挑的整句（`NeuralKeyboard`），排在词网格最好的整句之后（`lattice::merge::reranked_block`）。26 键在词库里对一串音节的几个最好整句重排；九键每次按键有几十条切分，各重排一次太贵，所以这里取各条切分解出的整句里排在最前的 `MAX_RERANK_PATHS` 条（已经按 `comparable_weight` 跨切分比较过，它也是交给模型的词网格分），统一重排一次。
    fn rerank_sentences(&mut self, candidates: &mut Vec<WordItem>) {
        if !self.sentence_association.neural_keyboard {
            return;
        }
        let paths = &self.paths;
        let keyboard = self.keyboard.get_or_insert_with(|| {
            shared_sentence_model(&paths.resource(assets::NEURAL_MODEL_KEYBOARD))
                .map(|model| NeuralReranker::new(CandidateSource::NeuralKeyboard, model))
        });
        let mut ranked = Vec::new();
        if let Some(keyboard) = keyboard.as_mut() {
            let scored: Vec<&WordItem> = candidates
                .iter()
                .filter(|item| is_lattice_sentence(item))
                .take(MAX_RERANK_PATHS)
                .collect();
            // 模型的顺序只取决于上屏上下文、整句和它们的词网格分，三者都相同时直接用上次的结果。
            let signature: Vec<(String, i64)> = scored
                .iter()
                .map(|item| (item.word.clone(), comparable_weight(item)))
                .collect();
            let context = &self.rescoring_context;
            if let Some(entry) = self
                .rerank_cache
                .iter()
                .find(|entry| entry.context == *context && entry.sentences == signature)
            {
                ranked.clone_from(&entry.ranked);
            } else {
                let mut sentences: Vec<SentencePath> = scored
                    .iter()
                    .map(|item| SentencePath {
                        sentence: item.word.clone(),
                        key: item.canonical_pinyin.clone(),
                        log_prob: comparable_weight(item) as f64 / 1000.0,
                        words: item.sentence_words.clone(),
                        typo_edges: 0,
                    })
                    .collect();
                if keyboard.rerank(&mut sentences, context) {
                    ranked = sentences.into_iter().map(|path| path.sentence).collect();
                }
                if self.rerank_cache.len() >= RERANK_CACHE_ENTRIES {
                    self.rerank_cache.remove(0);
                }
                self.rerank_cache.push(RerankEntry {
                    context: context.clone(),
                    sentences: signature,
                    ranked: ranked.clone(),
                });
            }
        }
        place_keyboard_pick(
            candidates,
            &ranked,
            self.sentence_association.show_next_on_duplicate,
            self.sentence_association.word_lattice,
        );
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
        let mut words = Vec::new();
        for prefix in prefixes {
            for word in english.query_prefix(&prefix, ENGLISH_LIMIT) {
                // 词条必须覆盖全部输入数字，防止展开前缀后面的字母读出不匹配的词。
                // 对照 `pinyin` 中的小写查询键；`word` 是展示形式，可以带标点或空格，
                // 例如查询键 `dont` 可以显示为 `don't`。
                if !word_matches_digits(&word.pinyin, &digits) {
                    continue;
                }
                if words.capacity() == 0 {
                    words.reserve_exact(capacity);
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

/// 键盘模型的一次重排：上屏上下文、交给模型的整句和词网格分（`comparable_weight`），以及模型给出的顺序（没有重排时为空）。
struct RerankEntry {
    context: String,
    sentences: Vec<(String, i64)>,
    ranked: Vec<String>,
}

/// 打开九键用的词库，带上 `NineKeySession::dictionary_association` 给的句子联想设置。
fn open_dictionary(
    paths: &RuntimePaths,
    association: SentenceAssociationOptions,
    alternatives: bool,
) -> QuanpinDictionary {
    let mut dictionary = QuanpinDictionary::new(paths);
    dictionary.set_sentence_association(association);
    dictionary.set_sentence_alternatives(alternatives);
    dictionary
}

/// 词网格解出的整句行（`lattice::merge::sentence_row` 的 `Generated` 行）。
fn is_lattice_sentence(item: &WordItem) -> bool {
    item.source == CandidateSource::Generated && item.sentence_association
}

/// 把键盘模型重排后的整句顺序 `ranked` 落到候选上，规则同 `lattice::merge::reranked_block`。词网格开关开着时（`include_lattice_best`），模型排第一的整句若不是词网格最好的那一行，就把它改成 `NeuralKeyboard` 行，挪到词网格最好的整句后面；它就是那一行时，`show_next_on_duplicate` 打开才接着看模型的下一句，否则不出模型行。词网格开关关着时词网格最好的整句不显示，模型排第一的整句总是改成 `NeuralKeyboard` 行、占它的位置（与它相同也一样），随后去掉其余的词网格整句行。`ranked` 为空（模型没有重排）时只做去掉整句这一步。
fn place_keyboard_pick(
    candidates: &mut Vec<WordItem>,
    ranked: &[String],
    show_next_on_duplicate: bool,
    word_lattice: bool,
) {
    let take_pick = |candidates: &mut Vec<WordItem>, sentence: &str, at_position: usize| {
        // 候选按词去过重，同一句只有一行。
        if let Some(at) = candidates
            .iter()
            .position(|item| is_lattice_sentence(item) && item.word == sentence)
        {
            let mut pick = candidates.remove(at);
            pick.source = CandidateSource::NeuralKeyboard;
            candidates.insert(at_position, pick);
        }
    };
    if let Some(best) = candidates.iter().position(is_lattice_sentence) {
        if !word_lattice {
            if let Some(sentence) = ranked.first() {
                take_pick(candidates, sentence, best);
            }
        } else {
            for sentence in ranked {
                if candidates[best].word != *sentence {
                    // 这句只可能是排在 `best` 之后的另一条词网格整句。
                    take_pick(candidates, sentence, best + 1);
                    break;
                }
                if !show_next_on_duplicate {
                    break;
                }
            }
        }
    }
    if !word_lattice {
        candidates.retain(|item| !is_lattice_sentence(item));
    }
}

/// 种子真正用到的部分：去掉最后一个音节，它可能被后面的数字续长，或者本来只拼了一半。
fn seed_prefix(seed: &str) -> &str {
    seed.rfind('\'').map_or("", |end| &seed[..end])
}

/// `prefix` 开头有几个音节按键盘依次拼得出 `digits` 的开头：(音节数, 用掉的数字数, 是否全部拼得出)。不分配。
fn fitting_syllables(prefix: &str, digits: &str) -> (usize, usize, bool) {
    let mut syllables = 0;
    let mut covered = 0;
    if prefix.is_empty() {
        return (0, 0, true);
    }
    for syllable in prefix.split('\'') {
        let end = covered + syllable.len();
        if !digits
            .get(covered..end)
            .is_some_and(|code| letters_spell_code(syllable, code))
        {
            return (syllables, covered, false);
        }
        syllables += 1;
        covered = end;
    }
    (syllables, covered, true)
}

/// 小写字母串按键盘上印的字母正好拼出同样长的 `code`。
fn letters_spell_code(letters: &str, code: &str) -> bool {
    letters.len() == code.len()
        && letters.bytes().zip(code.bytes()).all(|(letter, digit)| {
            letter.is_ascii_lowercase() && KEYPAD[usize::from(letter - b'a')] == digit
        })
}

/// 把这次刷新之后的种子写进 `seeds`（先清空），`previous` 用完清空：先取排好的候选里覆盖全部数字、不是模糊音的前 `SENTENCE_SEED_LIMIT` 条全拼（`seed_prefix` 相同的只留一条，免得几条种子只差在最后一个音节上），再接上 `previous` 里 `seed_prefix` 仍拼得出数字开头的，合计最多 `SEED_RETAIN_LIMIT` 条。末尾只按了下一个音节的头一两个数字时没有覆盖全部数字的行，这时全靠留下来的种子，否则每隔一键就丢一次。`previous` 里已有的同一条直接挪过来，不再复制字符串。
fn next_seeds(
    candidates: &[WordItem],
    digits: &str,
    previous: &mut Vec<String>,
    seeds: &mut Vec<String>,
) {
    seeds.clear();
    let is_new = |seeds: &[String], seed: &str| {
        seeds
            .iter()
            .all(|existing| seed_prefix(existing) != seed_prefix(seed))
    };
    for item in candidates {
        if seeds.len() == SENTENCE_SEED_LIMIT {
            break;
        }
        if item.pinyin.len() != digits.len()
            || item.canonical_pinyin.is_empty()
            || item.fuzzy
            || !is_new(seeds, &item.canonical_pinyin)
        {
            continue;
        }
        let seed = match previous
            .iter()
            .position(|old| *old == item.canonical_pinyin)
        {
            Some(index) => previous.remove(index),
            None => item.canonical_pinyin.clone(),
        };
        seeds.push(seed);
    }
    for seed in previous.drain(..) {
        if seeds.len() < SEED_RETAIN_LIMIT
            && fitting_syllables(seed_prefix(&seed), digits).2
            && is_new(seeds, &seed)
        {
            seeds.push(seed);
        }
    }
}

/// 按数字切分路径只看音节频度，长串里正确的切分常常排不进每个位置的 `PATH_LIMIT` 条（#6059）；而词网格看得懂词，前几次刷新排在前面的整句的切分多半就是这一次的前半截。所以取每条种子的 `seed_prefix` 里仍拼得出 `remaining` 开头的那几个音节，接上余下数字的前 `SEED_TAIL_LIMIT` 条切分，补进 `alternatives`。等于在按键之间做一次宽度为 `SEED_RETAIN_LIMIT` 的束搜索，每次按键只把束里的切分往后接一两个音节。
fn seeded_paths(
    alternatives: &mut Vec<Path>,
    seeds: &[String],
    remaining: &str,
    table: &SpellingTable,
    prior: &SyllablePrior,
) {
    for seed in seeds {
        let (kept, covered, _) = fitting_syllables(seed_prefix(seed), remaining);
        if kept == 0 {
            continue;
        }
        let tail = &remaining[covered..];
        let tails = if tail.is_empty() {
            vec![Vec::new()]
        } else {
            table.split_paths(tail, &[], prior)
        };
        for tail_path in tails.into_iter().take(SEED_TAIL_LIMIT) {
            let mut path: Path = Vec::with_capacity(kept + tail_path.len());
            path.extend(seed.split('\'').take(kept).map(str::to_owned));
            path.extend(tail_path);
            if !alternatives.contains(&path) {
                alternatives.push(path);
            }
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
    let syllables = item.canonical_pinyin.split('\'').count();
    // 每个多字词再扣一份奖励（#6059）：只按音节扣时，一个词组的分只剩它的词频，比单字的分高得多，把读法切成更多词组反而得分，`ni'ming'tian'you'long'ma`（匿名天佑龙马）就压过了 `ni'ming'tian'you'kong'ma`（你明天有空吗）。每个词扣一份，等于给每个词一份插入代价，切得越碎越吃亏；单字本来就按 `unigram_z` 扣过，不再加扣。
    let phrases = item
        .sentence_words
        .iter()
        .filter(|word| word.chars().nth(1).is_some())
        .count();
    item.weight - (PHRASE_LENGTH_BONUS * 1000.0 * (syllables + phrases) as f64) as i64
}

/// 九键记进个人上下文模型的词：两个字以上的纯汉字词，单字和夹着英文、符号的不记（#6185）。
fn is_personal_word(word: &str) -> bool {
    count_utf8_chars(word) >= 2 && is_all_han(word)
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
            if leading.is_empty() {
                leading.reserve(CANDIDATE_LIMIT);
            }
            leading.insert(candidate.word.clone(), key);
        }
    }
    candidates.push(candidate);
}

/// Stable sort by `rank_key`, dedup by word, capped (NK:283-307). 截断前再把用户用过的简拼行往前挪（`boost_used_initials`）。
fn rank_candidates(
    candidates: &mut Vec<WordItem>,
    prefer_exact: bool,
    initials_lead: bool,
    boost: Option<&InitialsBoost>,
) {
    candidates.sort_by_key(|item| rank_key(item, prefer_exact, initials_lead));
    retain_unique_words(candidates);
    if !initials_lead {
        interleave_initials(candidates);
    }
    if let Some(boost) = boost {
        boost_used_initials(candidates, initials_lead, boost);
    }
    candidates.truncate(CANDIDATE_LIMIT);
}

/// 简拼行按用户用过的次数挪位（#6185），沿用调频的模式、触发次数和步长。
struct InitialsBoost {
    /// 用过的简拼词和它在个人上下文模型里的计数。
    used: HashMap<String, u32>,
    frequency: FrequencyAdjustmentOptions,
}

/// 把用户用过的简拼行往前挪（#6185）。简拼行都在覆盖全部数字的那一段词典行里（`rank_key` 先按覆盖、再按是否合成行排，所以这一段从列表开头算起），只在这一段里挪。每个用过的词从它现在的位置起，计数每够一次触发（`PERSONAL_PICK_TIMES * trigger_count`）就按调频模式算一次新位置（`ranking::ranking_target`：置顶到头、减半、按步长、提到第五位以内再逐位上移），与 26 键调频一次次挪位的结果相同。挪不过这一段开头留给音节行的位置：没打切分时前 `SYLLABLE_ROWS_BEFORE_INITIALS` 个音节行（最常用的单字）照旧在最前；打了切分时简拼行本来就领先，可以挪到最前。几个用过的词抢同一个位置时，原来靠前（权重高）的先占。只重排去重之后的列表，保留哪一行不受影响，`push_ranked` 的跳过规则照样成立。
fn boost_used_initials(candidates: &mut Vec<WordItem>, initials_lead: bool, boost: &InitialsBoost) {
    let Some(first) = candidates.first() else {
        return;
    };
    let coverage = first.pinyin.len();
    let end = candidates
        .iter()
        .take_while(|item| item.pinyin.len() == coverage && !item.source.is_generated_or_fallback())
        .count();
    let floor = if initials_lead {
        0
    } else {
        candidates[..end]
            .iter()
            .take(SYLLABLE_ROWS_BEFORE_INITIALS)
            .take_while(|item| !is_initials_row(item))
            .count()
    };
    let trigger_count = u32::try_from(boost.frequency.trigger_count.clamp(1, 10)).unwrap_or(1);
    let per_trigger = PERSONAL_PICK_TIMES * trigger_count;
    // (目标位置, 现在的位置)
    let mut moves = Vec::new();
    for (index, item) in candidates[..end].iter().enumerate() {
        if !is_initials_row(item) {
            continue;
        }
        let Some(&count) = boost.used.get(&item.word) else {
            continue;
        };
        let mut rank = index;
        // 每次触发至少挪一位（步长为 0 的线性模式除外，它一位也不挪），所以最多算 `index` 次。
        let triggers = usize::try_from(count / per_trigger).unwrap_or(usize::MAX);
        for _ in 0..triggers.min(index) {
            if rank == 0 {
                break;
            }
            rank = ranking::ranking_target(
                rank,
                boost.frequency.mode,
                boost.frequency.linear_step,
                false,
            );
        }
        let target = rank.max(floor);
        if target < index {
            moves.push((target, index));
        }
    }
    if moves.is_empty() {
        return;
    }
    // 先从后往前取出要挪的行（前面的下标不受影响），再按目标位置从前往后放回；同一个位置先放原来靠前的，后来的顺延一位。
    let mut rows: Vec<(usize, usize, WordItem)> = moves
        .iter()
        .rev()
        .map(|&(target, index)| (target, index, candidates.remove(index)))
        .collect();
    rows.sort_by_key(|&(target, index, _)| (target, index));
    let mut next = 0;
    for (target, _, row) in rows {
        let at = target.max(next);
        candidates.insert(at, row);
        next = at + 1;
    }
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

/// `keyword`（小写字母）按键盘上的字母正好拼出 `digits`，不分配。
fn keyword_spells_digits(keyword: &str, digits: &str) -> bool {
    keyword.len() == digits.len()
        && keyword.bytes().zip(digits.bytes()).all(|(letter, digit)| {
            letter.is_ascii_lowercase() && KEYPAD[usize::from(letter - b'a')] == digit
        })
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

    /// Complete syllables the unlocked digits can start with, or that complete them, longest covered first (NK:238-250). Coverage is counted in digits; after the first digit, a reading already spelled in full comes before a longer completion within one coverage bucket.
    /// 有切分时，只有在切分处或之前结束的音节才算。
    ///
    /// 覆盖数字一样多时，已打两位以上的数字先按编码长度排（正好拼完的音节在前，要补键少的在前），编码一样长的按 `prior` 给的单字频度排，频度相同才按字典序；只打了一位数字时不比编码长度，直接按频度排。原先同组只按字典序，`74` 的左列是 pi、qi、ri、si，最常用的 qi 排不到前面，要补键的 sha、shai 也排在 shi、shu 前面（#6654）。
    fn spellings_for(
        &self,
        remaining: &str,
        locked_length: usize,
        split: Option<usize>,
        prior: &SyllablePrior,
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
            covered(b_code)
                .cmp(&covered(a_code))
                .then_with(|| {
                    if remaining.len() > 1 {
                        a_code.len().cmp(&b_code.len())
                    } else {
                        Ordering::Equal
                    }
                })
                .then_with(|| prior.syllable_score(b).total_cmp(&prior.syllable_score(a)))
                .then_with(|| a.cmp(b))
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
            let mut result = Vec::new();
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
                        if result.is_empty() {
                            result.reserve_exact(PATH_LIMIT);
                        }
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

/// 音节和音节前缀的单字频度：音节取词库里它最常用那个字的权重取对数，前缀取以它开头的音节里最高的那个。九宫格用它决定每个位置留哪 48 条切分路径和左列音节的先后，候选本身的先后仍由词库和整句解码决定。
#[derive(Default)]
struct SyllablePrior {
    scores: HashMap<String, f64>,
    /// 完整音节自己的频度，不取以它开头的更长音节的：左列里 pin 不该沾 ping 的光。
    syllables: HashMap<String, f64>,
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
        let mut own: HashMap<String, f64> = HashMap::with_capacity(syllables.len());
        for (syllable, _) in syllables {
            // 没有单字行的音节和权重为 0 的一样按 1 算。
            let weight = weights.get(syllable).copied().unwrap_or(0).max(1) as f64;
            let score = (weight / total).ln();
            own.insert(syllable.clone(), score);
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
        Self {
            scores,
            syllables: own,
        }
    }

    fn score(&self, piece: &str) -> f64 {
        self.scores.get(piece).copied().unwrap_or(0.0)
    }

    fn syllable_score(&self, syllable: &str) -> f64 {
        self.syllables.get(syllable).copied().unwrap_or(0.0)
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
    fn unreadable_paths_do_not_reserve_each_offset_page() {
        let table = SpellingTable::new(&["ni", "mi", "o"]);
        let digits = "1".repeat(32);
        let (paths, allocations) =
            crate::ime::personal_rerank::allocations::count(|| table.paths(&digits));
        assert!(paths.is_empty());
        assert!(
            allocations <= 2,
            "死路偏移不应逐偏移申请路径页：{allocations}"
        );
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

    /// #6059：每个多字词再扣一份奖励，切成更多词组的读法不再因此得分。
    #[test]
    fn each_phrase_costs_one_more_bonus_across_paths() {
        let bonus = (PHRASE_LENGTH_BONUS * 1000.0) as i64;
        let sentence = |word: &str, pinyin: &str, words: &[&str], weight: i64| {
            let mut row = item(
                word,
                "64646484268966566262",
                weight,
                CandidateSource::Generated,
            );
            row.canonical_pinyin = pinyin.into();
            row.sentence_words = words.iter().map(|word| (*word).to_owned()).collect();
            row
        };
        // 两句都是六个音节；三个词组的那句原始分高出半份奖励，两个词组加两个单字的那句扣掉多出来的一份词组奖励后反超。
        let phrases = sentence(
            "匿名天佑龙马",
            "ni'ming'tian'you'long'ma",
            &["匿名", "天佑", "龙马"],
            40_000,
        );
        let plain = sentence(
            "你明天有空吗",
            "ni'ming'tian'you'kong'ma",
            &["你", "明天", "有空", "吗"],
            30_000,
        );
        assert_eq!(comparable_weight(&phrases), 40_000 - 9 * bonus);
        assert_eq!(comparable_weight(&plain), 30_000 - 8 * bonus);
        assert_eq!(
            ranked(vec![phrases, plain], false),
            ["你明天有空吗", "匿名天佑龙马"]
        );
    }

    fn path(syllables: &[&str]) -> Path {
        syllables
            .iter()
            .map(|syllable| (*syllable).to_owned())
            .collect()
    }

    #[test]
    fn seeds_take_distinct_prefixes_then_keep_old_ones_that_still_fit() {
        let digits = encode("wo'men'yi'qi");
        let full = |word: &str, pinyin: &str| {
            let mut row = item(word, &digits, 100, CandidateSource::Generated);
            row.canonical_pinyin = pinyin.into();
            row
        };
        let mut fuzzy = full("我们一起", "wo'men'yi'qi");
        fuzzy.fuzzy = true;
        let candidates = vec![
            fuzzy,
            full("我们一起", "wo'men'yi'qi"),
            // 和上一行只差最后一个音节，前半截相同，不再当一条种子。
            full("我们一批", "wo'men'yi'pi"),
            // 只覆盖一部分数字的行不算。
            item("我们", "96636", 100, CandidateSource::Database),
            full("我们洗漆", "wo'men'xi'qi"),
        ];
        let previous = vec![
            // 和这次的第一条完全相同：直接挪过来，不复制。
            "wo'men'yi'qi".to_owned(),
            // 前半截对不上数字。
            "ni'men".to_owned(),
            "wo'men'yi'pi'a".to_owned(),
        ];
        let mut previous = previous;
        let reused = previous[0].as_ptr();
        let mut seeds = vec!["stale".to_owned()];
        next_seeds(&candidates, &digits, &mut previous, &mut seeds);
        assert_eq!(seeds, ["wo'men'yi'qi", "wo'men'xi'qi", "wo'men'yi'pi'a"]);
        assert_eq!(seeds[0].as_ptr(), reused);
        assert!(previous.is_empty());
        // 没有覆盖全部数字的行时全靠留下来的种子。
        next_seeds(&[], &digits, &mut vec!["wo'men'yi".to_owned()], &mut seeds);
        assert_eq!(seeds, ["wo'men'yi"]);
    }

    #[test]
    fn seed_prefixes_fit_digits_syllable_by_syllable() {
        assert_eq!(seed_prefix("wo'men'yi"), "wo'men");
        assert_eq!(seed_prefix("wo"), "");
        let digits = encode("wo'men'yi'qi");
        assert_eq!(fitting_syllables("wo'men", &digits), (2, 5, true));
        // ne 的 63 也是 men 的开头，接下来的 yi 就对不上了。
        assert_eq!(fitting_syllables("wo'ne'yi", &digits), (2, 4, false));
        assert_eq!(fitting_syllables("wo'mo", &digits), (1, 2, false));
        assert_eq!(fitting_syllables("", &digits), (0, 0, true));
        assert_eq!(fitting_syllables("wo'men'yi'qi'a", &digits), (4, 9, false));
    }

    #[test]
    fn seeded_paths_extend_the_fitting_prefix_with_tail_splits() {
        let table = SpellingTable::new(&["wo", "men", "yi", "qi", "xi", "pi"]);
        let prior = SyllablePrior::default();
        let remaining = encode("wo'men'yi'qi");
        let mut alternatives = vec![path(&["wo", "men", "xi", "pi"])];
        seeded_paths(
            &mut alternatives,
            // 第二条的前半截对不上数字，不加。
            &["wo'men'yi".to_owned(), "xi'men'yi".to_owned()],
            &remaining,
            &table,
            &prior,
        );
        // 种子去掉最后一个音节剩 wo'men，余下的 9474 按切分先后取前三条，已经在的不重复。
        assert_eq!(
            alternatives,
            [
                path(&["wo", "men", "xi", "pi"]),
                path(&["wo", "men", "xi", "qi"]),
                path(&["wo", "men", "yi", "pi"]),
            ]
        );
        // 前半截正好拼完全部数字时加它自己。
        let mut whole = Vec::new();
        seeded_paths(
            &mut whole,
            &["wo'men'yi'qi'a".to_owned()],
            &remaining,
            &table,
            &prior,
        );
        assert_eq!(whole, [path(&["wo", "men", "yi", "qi"])]);
    }

    /// 末尾只按了一个音节开头的数字、没有覆盖全部数字的行时，种子留着；组字结束时清空。
    #[test]
    fn seeds_survive_a_partial_syllable_and_end_with_the_composition() {
        let fixture = fixture();
        let mut session = open(&fixture.paths, false, EnglishInputOptions::default());
        type_digits(&mut session, "64426");
        assert!(session.sentence_seeds.contains(&"ni'hao".to_owned()));
        type_digits(&mut session, "3");
        assert!(session.sentence_seeds.contains(&"ni'hao".to_owned()));
        session.command(Command::Cancel);
        assert!(session.sentence_seeds.is_empty());
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
        let prior = SyllablePrior::default();
        // 覆盖三位数字的音节里，正好三键拼完的 gan、gao、han、hao 在要补第四键的 gang、hang 前面。
        assert_eq!(
            table.spellings_for("426", 2, None, &prior),
            ["gan", "gao", "han", "hao", "gang", "hang", "ga", "ha"]
        );
        // With 31 digits already locked, only a one-digit completion still fits in 32.
        assert_eq!(table.spellings_for("2", 31, None, &prior), ["a"]);
        assert!(table.spellings_for("", 0, None, &prior).is_empty());
        // 在两个数字之后切开，就排除了所有跨过这个位置的音节。
        assert_eq!(table.spellings_for("426", 0, Some(2), &prior), ["ga", "ha"]);
    }

    #[test]
    fn spellings_put_exact_syllables_first_then_frequent_ones() {
        // #6654：`74` 下正好两键的 qi、si、ri、pi 先列，按单字频度排；要补键的排在后面，补一键的 shi、pie、pin、qia 在补两键的 pian、qian、ping、piao 前面，同样长的按频度排。
        let table = SpellingTable::new(&[
            "pi", "pian", "piao", "pie", "pin", "ping", "qi", "qia", "qian", "ri", "shi", "si",
        ]);
        let weights: HashMap<String, i64> = [
            ("shi", 9_000),
            ("qi", 5_000),
            ("si", 3_000),
            ("ri", 2_000),
            ("pi", 1_000),
            ("pian", 800),
            ("qian", 700),
            ("ping", 600),
        ]
        .into_iter()
        .map(|(syllable, weight)| (syllable.to_owned(), weight))
        .collect();
        let prior = SyllablePrior::from_weights(&table.syllables, &weights);
        assert_eq!(
            table.spellings_for("74", 0, None, &prior),
            ["qi", "si", "ri", "pi", "shi", "pie", "pin", "qia", "pian", "qian", "ping", "piao"]
        );
    }

    #[test]
    fn complete_two_digit_readings_precede_longer_completions() {
        let table = SpellingTable::new(&["pi", "pian", "piao", "pie", "qi", "ri", "shi", "si"]);
        let choices = table.spellings_for("74", 0, None, &SyllablePrior::default());
        assert_eq!(&choices[..4], ["pi", "qi", "ri", "si"]);
    }

    #[test]
    fn one_digit_keeps_predictive_readings_ahead_of_single_letter_syllables() {
        let table = SpellingTable::new(&["ma", "mi", "o"]);
        assert_eq!(
            table.spellings_for("6", 0, None, &SyllablePrior::default()),
            ["ma", "mi", "o"]
        );
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
    fn push_ranked_defers_leading_capacity_until_a_candidate_exists() {
        let mut candidates = Vec::new();
        let mut leading = HashMap::new();
        assert_eq!(leading.capacity(), 0);

        push_ranked(
            &mut candidates,
            &mut leading,
            item("你", "64", 1, CandidateSource::Database),
            false,
            false,
        );

        assert_eq!(candidates.len(), 1);
        assert!(leading.capacity() >= CANDIDATE_LIMIT);
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
        rank_candidates(&mut candidates, false, false, None);
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
        rank_candidates(&mut candidates, prefer_exact, false, None);
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
            rank_candidates(&mut everything, prefer_exact, false, None);
            let mut skipped = Vec::new();
            let mut leading = HashMap::new();
            for row in rows {
                push_ranked(&mut skipped, &mut leading, row, prefer_exact, false);
            }
            rank_candidates(&mut skipped, prefer_exact, false, None);
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
    fn unfinished_top_candidate_does_not_displace_complete_readings() {
        let fixture = fixture_with("CREATE TABLE tbl_1_s(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_1_s VALUES('shi','s','是',10000);");
        let mut session = open(&fixture.paths, false, EnglishInputOptions::default());
        type_digits(&mut session, "74");
        let view = session.snapshot();
        assert_eq!(
            view.candidates
                .first()
                .map(|candidate| candidate.word.as_str()),
            Some("是")
        );
        assert_eq!(&view.nine_key_spellings[..4], ["pi", "qi", "ri", "si"]);
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
    fn refresh_reuses_candidate_vector_storage() {
        let fixture = fixture();
        let mut session = open(&fixture.paths, false, EnglishInputOptions::default());
        session.digits = "64".into();
        session.refresh();
        let pointer = session.candidates.as_ptr();
        let capacity = session.candidates.capacity();
        assert!(capacity > 0);
        session.refresh();
        assert_eq!(session.candidates.as_ptr(), pointer);
        assert_eq!(session.candidates.capacity(), capacity);
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
    fn english_t9_empty_matches_do_not_reserve_prefix_capacity() {
        let fixture = fixture();
        let mut session = open(&fixture.paths, false, EnglishInputOptions::default());
        session.set_english_only(true);
        session.digits = "999999".into();
        let words = session.english_candidates(false);
        assert!(words.is_empty());
        assert_eq!(words.capacity(), 0);
    }

    #[test]
    fn english_t9_filtered_matches_do_not_reserve_prefix_capacity() {
        let fixture = fixture();
        Connection::open(fixture.paths.dictionary(assets::ENGLISH_DICTIONARY))
            .unwrap()
            .execute(
                "INSERT INTO english_words(word, display, weight) VALUES ('wwwa', '合成展示', 100)",
                [],
            )
            .unwrap();
        let mut session = open(&fixture.paths, false, EnglishInputOptions::default());
        session.set_english_only(true);
        session.digits = "999999".into();
        assert!(!session
            .open_english()
            .unwrap()
            .query_prefix("www", ENGLISH_LIMIT)
            .is_empty());
        let words = session.english_candidates(false);
        assert!(words.is_empty());
        assert_eq!(words.capacity(), 0);
    }

    #[test]
    fn english_t9_hits_keep_the_original_prefix_capacity() {
        let fixture = fixture();
        let mut session = open(&fixture.paths, false, EnglishInputOptions::default());
        session.set_english_only(true);
        session.digits = "3668".into();
        let capacity =
            letter_prefixes(&session.digits, ENGLISH_PREFIX_BUDGET).len() * ENGLISH_LIMIT;
        Connection::open(fixture.paths.dictionary(assets::ENGLISH_DICTIONARY))
            .unwrap()
            .execute(
                "INSERT INTO english_words(word, display, weight) VALUES ('dont', '合成展示', 100)",
                [],
            )
            .unwrap();
        let words = session.english_candidates(false);
        assert!(words
            .iter()
            .any(|word| word.word == "合成展示" && word.pinyin == "dont"));
        assert_eq!(words.capacity(), capacity);
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
                "ni", "mi", "mie", "min", "miu", "nie", "nin", "niu", "mian", "miao", "ming",
                "nian", "niao", "ning", "niang", "o", "M", "N", "O", "6"
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
            ["hao", "gan", "gao", "han", "gang", "hang", "ga", "ha", "G", "H"],
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

    fn sentence_words(session: &NineKeySession) -> Vec<String> {
        session
            .snapshot()
            .candidates
            .into_iter()
            .filter(is_lattice_sentence)
            .map(|item| item.word)
            .collect()
    }

    /// #6059：九键遵守句子联想的词网格开关和整句备选开关，与 26 键一致。
    #[test]
    fn sentence_rows_follow_the_sentence_association_options() {
        let fixture = fixture_with(&format!(
            "{MAIN_FIXTURE}INSERT INTO tbl_1_h VALUES('hao','h','号',50);"
        ));
        let mut session = open(&fixture.paths, false, mixed());
        type_digits(&mut session, "64426");
        assert_eq!(sentence_words(&session), ["米好"]);

        session.set_sentence_options(
            SentenceAssociationOptions {
                word_lattice: false,
                ..SentenceAssociationOptions::default()
            },
            false,
        );
        assert!(sentence_words(&session).is_empty(), "{:?}", words(&session));
        assert_eq!(words(&session), ["你好", "你", "米", "ogham"]);

        // 每条切分交回全部整句读法：mi'hao 的 米号 和 ni'hao 的 你号 也出来了。
        session.set_sentence_options(SentenceAssociationOptions::default(), true);
        let sentences = sentence_words(&session);
        for sentence in ["米好", "米号", "你号"] {
            assert!(
                sentences.iter().any(|word| word == sentence),
                "{sentences:?}"
            );
        }
        session.command(Command::Cancel);

        // 打开之前就设好的会话，第一次打开词库时就带上设置。
        let mut fresh = open(&fixture.paths, false, mixed());
        fresh.set_sentence_options(
            SentenceAssociationOptions {
                word_lattice: false,
                ..SentenceAssociationOptions::default()
            },
            false,
        );
        type_digits(&mut fresh, "64426");
        assert!(sentence_words(&fresh).is_empty(), "{:?}", words(&fresh));
    }

    /// 键盘模型读不到时（测试目录里没有模型文件）没有模型行；词网格开关关着时，为重排解出的整句照样去掉，与 26 键里模型重排失败时一样。
    #[test]
    fn a_missing_keyboard_model_leaves_the_lattice_rows_to_their_switch() {
        let fixture = fixture();
        let mut session = open(&fixture.paths, false, mixed());
        let neural = |word_lattice| SentenceAssociationOptions {
            word_lattice,
            neural_keyboard: true,
            show_next_on_duplicate: false,
        };
        session.set_sentence_options(neural(true), false);
        type_digits(&mut session, "64426");
        assert_eq!(sentence_words(&session), ["米好"]);
        assert!(!session
            .snapshot()
            .candidates
            .iter()
            .any(|item| item.source == CandidateSource::NeuralKeyboard));
        session.set_sentence_options(neural(false), false);
        assert!(sentence_words(&session).is_empty(), "{:?}", words(&session));
    }

    #[test]
    fn the_rescoring_context_keeps_the_characters_the_model_reads() {
        let fixture = fixture();
        let mut session = open(&fixture.paths, false, mixed());
        let long: String = "甲乙".repeat(CONTEXT_CHARACTERS);
        session.set_rescoring_context(&long);
        assert_eq!(
            session.rescoring_context.chars().count(),
            CONTEXT_CHARACTERS
        );
        assert!(long.ends_with(&session.rescoring_context));
        session.set_rescoring_context("今天");
        assert_eq!(session.rescoring_context, "今天");
    }

    fn sentence(word: &str, weight: i64) -> WordItem {
        let mut item = item(word, "64426", weight, CandidateSource::Generated);
        item.sentence_association = true;
        item
    }

    fn row_sources(candidates: &[WordItem]) -> Vec<(&str, CandidateSource)> {
        candidates
            .iter()
            .map(|item| (item.word.as_str(), item.source))
            .collect()
    }

    /// 模型挑的整句改成 `NeuralKeyboard` 行，紧跟词网格最好的整句，规则同 `lattice::merge::reranked_block`。
    #[test]
    fn the_keyboard_pick_follows_the_best_lattice_sentence() {
        use CandidateSource::{Database, Generated, NeuralKeyboard};
        let list = || {
            vec![
                item("你好", "64426", 1000, Database),
                sentence("米好", -1000),
                sentence("你号", -2000),
                sentence("米号", -3000),
            ]
        };
        let ranked = |sentences: &[&str]| -> Vec<String> {
            sentences.iter().map(|word| (*word).to_owned()).collect()
        };

        let mut candidates = list();
        place_keyboard_pick(
            &mut candidates,
            &ranked(&["米号", "米好", "你号"]),
            false,
            true,
        );
        assert_eq!(
            row_sources(&candidates),
            [
                ("你好", Database),
                ("米好", Generated),
                ("米号", NeuralKeyboard),
                ("你号", Generated)
            ]
        );

        // 模型也最看好词网格那一句：默认不出模型行，`show_next_on_duplicate` 时接着取下一句。
        let mut candidates = list();
        place_keyboard_pick(
            &mut candidates,
            &ranked(&["米好", "你号", "米号"]),
            false,
            true,
        );
        assert_eq!(row_sources(&candidates), row_sources(&list()));
        let mut candidates = list();
        place_keyboard_pick(
            &mut candidates,
            &ranked(&["米好", "你号", "米号"]),
            true,
            true,
        );
        assert_eq!(candidates[2].word, "你号");
        assert_eq!(candidates[2].source, NeuralKeyboard);

        // 词网格开关关着：只留模型那一行；模型没有重排时一行整句也不留。
        let mut candidates = list();
        place_keyboard_pick(
            &mut candidates,
            &ranked(&["米号", "米好", "你号"]),
            false,
            false,
        );
        assert_eq!(
            row_sources(&candidates),
            [("你好", Database), ("米号", NeuralKeyboard)]
        );
        // 模型也最看好词网格那一句时照样出模型行：词网格那一行不显示，与 `reranked_block` 不带词网格最好的整句时相同。
        let mut candidates = list();
        place_keyboard_pick(
            &mut candidates,
            &ranked(&["米好", "你号", "米号"]),
            false,
            false,
        );
        assert_eq!(
            row_sources(&candidates),
            [("你好", Database), ("米好", NeuralKeyboard)]
        );
        let mut candidates = list();
        place_keyboard_pick(&mut candidates, &[], false, false);
        assert_eq!(row_sources(&candidates), [("你好", Database)]);
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

    /// 简拼 9'7（y's）下有两百行比 隐私 重的词，按权重只取前 `INITIALS_ROW_LIMIT` 行时 隐私 被截掉（#6185 的出货词库里它排第 285 行）。
    fn crowded_initials_fixture() -> Fixture {
        let mut main = String::from(
            "CREATE TABLE tbl_1_y(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_1_y VALUES('yin','y','因',100);\
CREATE TABLE tbl_1_s(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_1_s VALUES('si','s','四',100);\
CREATE TABLE tbl_2_y(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_2_y VALUES('yin''si','ys','隐私',10);",
        );
        for index in 0..200 {
            main.push_str(&format!(
                "INSERT INTO tbl_2_y VALUES('ya''qi','yq','压{index}',{});",
                100_000 - index
            ));
        }
        fixture_with(&main)
    }

    fn open_with_frequency(
        paths: &RuntimePaths,
        mode: FrequencyAdjustmentMode,
        trigger_count: i32,
        linear_step: i32,
    ) -> NineKeySession {
        NineKeySession::new(
            paths,
            true,
            FrequencyAdjustmentOptions {
                mode,
                trigger_count,
                linear_step,
            },
            FuzzyPinyinOptions::default(),
            EnglishInputOptions::default(),
            true,
            false,
        )
    }

    /// 别处（26 键）提交过 `word` 一次：显式选词在个人上下文模型里记 `PERSONAL_PICK_TIMES` 次。
    fn record_use(paths: &RuntimePaths, word: &str) {
        crate::user_dictionary::ngram_store::PersonalNgramStore::for_journal(
            &paths.user(assets::USER_JOURNAL),
        )
        .record(&[PersonalTransition {
            earlier: String::new(),
            previous: String::new(),
            word: word.to_owned(),
            times: PERSONAL_PICK_TIMES,
        }])
        .expect("record a use");
    }

    fn personal_count(paths: &RuntimePaths, word: &str) -> u32 {
        crate::user_dictionary::ngram_store::PersonalNgramStore::for_journal(
            &paths.user(assets::USER_JOURNAL),
        )
        .model()
        .word_count(word)
    }

    /// #6185：用过的简拼词不会被按权重的截断截掉，并按调频模式往前挪；默认的「提到前五」一次就进第五位，再用一次进第四位。
    #[test]
    fn used_initials_words_survive_the_row_limit_and_move_up() {
        let fixture = crowded_initials_fixture();
        let mut session =
            open_with_frequency(&fixture.paths, FrequencyAdjustmentMode::Promote, 1, 1);
        type_keys(&mut session, "9'7");
        assert!(!words(&session).iter().any(|word| word == "隐私"));
        session.command(Command::Cancel);

        // 用全拼读音的数字选一次（它在 94674 下排第一，不触发词频调整），简拼列表照样认它。
        type_digits(&mut session, "94674");
        assert_eq!(index_of(&session, "隐私"), 0);
        let chosen = session.select(0);
        assert_eq!(chosen.commit.as_deref(), Some("隐私"));
        assert_eq!(chosen.diagnostic, None);
        assert_eq!(personal_count(&fixture.paths, "隐私"), PERSONAL_PICK_TIMES);
        type_keys(&mut session, "9'7");
        assert_eq!(index_of(&session, "隐私"), 4);
        assert_eq!(words(&session)[..4], ["压0", "压1", "压2", "压3"]);
        session.command(Command::Cancel);

        record_use(&fixture.paths, "隐私");
        type_keys(&mut session, "9'7");
        assert_eq!(index_of(&session, "隐私"), 3);
        session.command(Command::Cancel);
        // 不打切分时 97 拼不成任何音节，同样是简拼行领头，挪法相同。
        type_digits(&mut session, "97");
        assert_eq!(index_of(&session, "隐私"), 3);
    }

    /// 在简拼列表里选中挪上来的用过的词：只记进个人上下文模型，不再另调全局词频，否则一次选词挪两次（还会连带改掉 26 键里它的位置）。提到前五、触发一次：26 键用过一次在第五位，九键再选一次进第四位，和 26 键用两次的位置相同。
    #[test]
    fn picking_a_used_initials_word_moves_it_once() {
        let fixture = crowded_initials_fixture();
        record_use(&fixture.paths, "隐私");
        let mut session =
            open_with_frequency(&fixture.paths, FrequencyAdjustmentMode::Promote, 1, 1);
        type_keys(&mut session, "9'7");
        assert_eq!(index_of(&session, "隐私"), 4);
        let chosen = session.select(4);
        assert_eq!(chosen.commit.as_deref(), Some("隐私"));
        assert_eq!(chosen.diagnostic, None);
        assert_eq!(
            personal_count(&fixture.paths, "隐私"),
            2 * PERSONAL_PICK_TIMES
        );
        type_keys(&mut session, "9'7");
        assert_eq!(index_of(&session, "隐私"), 3);
        assert_eq!(words(&session)[..3], ["压0", "压1", "压2"]);
    }

    /// 出货词库里大量词的权重同是 100：用过的词和前一张首字母表里的几百个同权重的词并列时，也不会因为合起来截断而丢掉（9'9'2'9 的 仔细查找）。
    #[test]
    fn a_used_word_tied_with_a_crowded_table_is_kept() {
        let mut main = String::from(
            "CREATE TABLE tbl_2_w(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
CREATE TABLE tbl_2_y(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_2_y VALUES('yin''si','ys','隐私',100);",
        );
        for index in 0..INITIALS_SCAN_LIMIT + 10 {
            main.push_str(&format!(
                "INSERT INTO tbl_2_w VALUES('wa''pi','wp','瓦{index}',100);"
            ));
        }
        let fixture = fixture_with(&main);
        record_use(&fixture.paths, "隐私");
        // 调频关着也查得到；同权重的行里，先留下来的用过的词排在最前。
        let mut session =
            open_with_frequency(&fixture.paths, FrequencyAdjustmentMode::Disabled, 1, 1);
        type_keys(&mut session, "9'7");
        assert_eq!(index_of(&session, "隐私"), 0);
        assert_eq!(words(&session).len(), INITIALS_ROW_LIMIT);
    }

    /// 置顶、减半、按步长三种模式和触发次数都照调频设置算。
    #[test]
    fn used_initials_words_follow_the_frequency_mode_and_trigger_count() {
        let position = |mode, trigger_count, linear_step, uses| {
            let fixture = crowded_initials_fixture();
            for _ in 0..uses {
                record_use(&fixture.paths, "隐私");
            }
            let mut session = open_with_frequency(&fixture.paths, mode, trigger_count, linear_step);
            type_keys(&mut session, "9'7");
            index_of(&session, "隐私")
        };
        // 截剩的 64 行里 隐私 原本在最后（第 64 位，下标 63）。
        assert_eq!(position(FrequencyAdjustmentMode::Pin, 1, 1, 1), 0);
        assert_eq!(position(FrequencyAdjustmentMode::Halve, 1, 1, 1), 31);
        assert_eq!(position(FrequencyAdjustmentMode::Halve, 1, 1, 2), 15);
        assert_eq!(position(FrequencyAdjustmentMode::Linear, 1, 3, 2), 57);
        // 触发次数 2：用一次还不挪，用两次才算一次触发。
        assert_eq!(position(FrequencyAdjustmentMode::Promote, 2, 1, 1), 63);
        assert_eq!(position(FrequencyAdjustmentMode::Promote, 2, 1, 2), 4);
    }

    /// 关掉调频时用过的词只保证查得到，排在它按权重该在的位置；关掉学习时不读个人数据，列表与没用过时相同。
    #[test]
    fn used_initials_words_only_move_with_frequency_adjustment() {
        let fixture = crowded_initials_fixture();
        record_use(&fixture.paths, "隐私");
        let mut unadjusted =
            open_with_frequency(&fixture.paths, FrequencyAdjustmentMode::Disabled, 1, 1);
        type_keys(&mut unadjusted, "9'7");
        assert_eq!(index_of(&unadjusted, "隐私"), INITIALS_ROW_LIMIT - 1);

        let mut quiet = open(&fixture.paths, false, EnglishInputOptions::default());
        type_keys(&mut quiet, "9'7");
        assert!(!words(&quiet).iter().any(|word| word == "隐私"));
    }

    /// 关掉个人上下文时，模型里早先记下的使用也不再读，与 26 键的 `personal_context_applies` 一样：用过的词不再多扫出来，也不挪。
    #[test]
    fn earlier_uses_are_ignored_with_personal_context_off() {
        let fixture = crowded_initials_fixture();
        record_use(&fixture.paths, "隐私");
        let mut session = open_with_frequency(&fixture.paths, FrequencyAdjustmentMode::Pin, 1, 1);
        type_keys(&mut session, "9'7");
        assert_eq!(index_of(&session, "隐私"), 0);
        session.command(Command::Cancel);

        session.set_personal_context_enabled(false);
        type_keys(&mut session, "9'7");
        assert!(!words(&session).iter().any(|word| word == "隐私"));
    }

    /// 没打切分时数字也可能是音节：用过的简拼词挪不过前面留给最常用单字的位置；打了切分才挪到最前。
    #[test]
    fn used_initials_words_stay_behind_the_leading_syllable_rows() {
        let fixture = fixture_with(INITIALS_FIXTURE);
        record_use(&fixture.paths, "那天");
        let mut session = open_with_frequency(&fixture.paths, FrequencyAdjustmentMode::Pin, 1, 1);
        type_digits(&mut session, "68");
        assert_eq!(words(&session), ["木", "欧", "那天", "每天", "明天"]);
        session.command(Command::Cancel);
        type_keys(&mut session, "6'8");
        assert_eq!(words(&session)[..3], ["那天", "每天", "明天"]);
        session.command(Command::Cancel);

        // 几个用过的词抢同一个位置时，原来靠前（权重高）的先占。
        record_use(&fixture.paths, "明天");
        type_keys(&mut session, "6'8");
        assert_eq!(words(&session)[..3], ["明天", "那天", "每天"]);
    }

    /// 九键选中的多字词库词（首位也算）、分段连成的词组都记进个人上下文模型；单字不记，关掉个人上下文时不记。
    #[test]
    fn nine_key_picks_are_recorded_for_the_initials() {
        let fixture = fixture_with(INITIALS_FIXTURE);
        let mut session = open(&fixture.paths, true, EnglishInputOptions::default());
        type_digits(&mut session, "963443842662");
        session.select(index_of(&session, "我滴"));
        let rest = session.select(index_of(&session, "个天呐"));
        assert_eq!(rest.diagnostic, None);
        for word in ["我滴", "个天呐", "我滴个天呐"] {
            assert_eq!(
                personal_count(&fixture.paths, word),
                PERSONAL_PICK_TIMES,
                "{word}"
            );
        }
        type_digits(&mut session, "68");
        session.select(index_of(&session, "木"));
        assert_eq!(personal_count(&fixture.paths, "木"), 0);

        session.set_personal_context_enabled(false);
        type_keys(&mut session, "6'8");
        session.select(index_of(&session, "每天"));
        assert_eq!(personal_count(&fixture.paths, "每天"), 0);
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

    /// #6654：出货词库上打 `74`，左列最前面除了首选候选的读音，就是正好两键拼完的 pi、qi、ri、si，pian 这些要补键的排在它们后面。没有 `MSIME_EVAL_RESOURCES` 时跳过。
    #[test]
    fn real_dictionary_lists_exact_syllables_before_completions() {
        let Some(resources) = std::env::var_os("MSIME_EVAL_RESOURCES") else {
            eprintln!("skipped: MSIME_EVAL_RESOURCES is not set to a resource directory");
            return;
        };
        let user = tempfile::tempdir().expect("user directory");
        let resources = PathBuf::from(resources);
        let paths = RuntimePaths {
            resources: resources.clone(),
            user_data: user.path().to_path_buf(),
            cache: user.path().to_path_buf(),
            dictionaries: resources,
        };
        let mut session = open(&paths, false, mixed());
        type_digits(&mut session, "74");
        let spellings = session.snapshot().nine_key_spellings;
        let preferred = &spellings[0];
        let exact: Vec<&str> = spellings[1..]
            .iter()
            .map(String::as_str)
            .filter(|spelling| spelling != preferred)
            .take_while(|spelling| encode(spelling) == "74")
            .collect();
        let mut expected: Vec<&str> = ["pi", "qi", "ri", "si"]
            .into_iter()
            .filter(|spelling| spelling != preferred)
            .collect();
        let mut sorted = exact.clone();
        sorted.sort_unstable();
        expected.sort_unstable();
        assert_eq!(sorted, expected, "{spellings:?}");
    }
}
