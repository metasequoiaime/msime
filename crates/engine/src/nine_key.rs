//! T9 digit composition over quanpin syllables plus English T9 (core-session.md §8). Active while it holds digits; `Session` routes to it then.
//!
//! The digits stay the composition: a chosen spelling only rewrites its span of digits and is remembered in `locked`, and every candidate's `pinyin` is the run of digits it consumes, so selection advances the same way whichever reading produced the row.

use std::borrow::Cow;
use std::cmp::{Ordering, Reverse};
use std::collections::{HashMap, HashSet};
use std::hash::{BuildHasherDefault, Hasher};
use std::path::PathBuf;
use std::sync::OnceLock;

use crate::assets;
use crate::diagnostics;
use crate::dictionary::english::{append_keypad_ranked, EnglishDictionary, KeypadRow};
use crate::error::EngineError;
use crate::ime::queries::{
    insert_expressive_rows, keep_expressive_rows_off_the_lead, MIXED_EXPRESSIVE_MINIMUM_INPUT,
};
use crate::language_dictionary::{self, LanguageDictionary};
use crate::lattice::decode::PHRASE_LENGTH_BONUS;
use crate::lattice::neural::{
    shared_sentence_model, NeuralReranker, CONTEXT_CHARACTERS, MAX_RERANK_PATHS,
};
use crate::lattice::personal::PersonalTransition;
use crate::lattice::SentencePath;
use crate::local::date_time::{inline_date_time_keyword, insert_inline_date_time};
use crate::local::emoji::{query_emoji_readings, query_kaomoji_readings, ExpressiveRow};
use crate::local::symbol_candidates::{query_symbol_readings, SymbolRow, MIXED_SYMBOL_LIMIT};
use crate::nine_key_shuangpin::NineKeyShuangpin;
use crate::paths::RuntimePaths;
use crate::pinyin::segment::{is_complete_pinyin_input, split_segments};
use crate::pinyin::syllables::intact_pinyin_list;
use crate::quanpin::QuanpinDictionary;
use crate::session::{LocalClock, SessionSnapshot};
use crate::stroke;
use crate::text::{count_utf8_chars, is_all_han, is_han_phrase, last_characters};
use crate::types::{
    CandidateSource, Command, EnglishInputOptions, FirstPageRows, FrequencyAdjustmentMode,
    FrequencyAdjustmentOptions, FuzzyPinyinOptions, KeyResult, LocalInputMode,
    MixedExpressiveOptions, PersonalDictionaryKind, SchemeType, SentenceAssociationOptions,
    SharedString, WordItem,
};
use crate::user_dictionary::positions;
use crate::user_dictionary::ranking::{self, RankingRequest};
use crate::user_dictionary::removal;

pub const PATH_LIMIT: usize = 48;
pub const DIGIT_LIMIT: usize = 48;
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
/// 每次刷新真正拿去查词库的切分条数。切分搜索本身每个位置仍留 `PATH_LIMIT` 条，这里只截最后物化的路径：每条路径都要查一遍候选、合并整句，一次刷新的耗时大致随条数增长。从 48 降到 24，`ninekey_latency` 整轮执行的指令数少约 31%；`convert_eval` 九键常用词首选 57.5% → 57.1%、前五 85.2% → 84.7%，九键长句首选不变、前五 65.4% → 64.2%，14 键常用词首选 66.0% → 65.8%、长句不变；先锁定首音节再打余下数字时，1500 个多音节词里正确词进前九的从 1448 个变为 1446 个。搜索宽度一起降到 24 时这几组数字完全相同，指令数也一样，所以只截查询这一步。有种子的长串原本截到的 `SEEDED_PATH_LIMIT` 与它相等，那一步现在不再多截。
const QUERY_PATH_LIMIT: usize = 24;
/// 音节频度取这个音节权重最高的几个单字之和，见 `SyllablePrior`。
const PRIOR_CHAR_LIMIT: usize = 3;
// 少量查询键直接扫描已有切分路径，避免刷新时为临时哈希表分配堆内存。
const SMALL_QUERY_KEY_BATCH: usize = 64;
// 少量九宫格候选直接扫描已保留词，避免排序后为一次去重分配哈希表。
const SMALL_CANDIDATE_DEDUP: usize = 64;
// 短列表保留直接排序，避免额外创建缓存键数组。
const SMALL_CANDIDATE_SORT: usize = 32;

/// The digit printed beside each of `a..=z` (NK:37).
const KEYPAD: &[u8; 26] = b"22233344455566677778889999";
/// The letters printed on each digit key (NK:74).
const DIGIT_LETTERS: [&str; 10] = [
    "", "", "abc", "def", "ghi", "jkl", "mno", "pqrs", "tuv", "wxyz",
];
/// 全拼 14 键 `a..=z` 各自的组码：QWERTY 上相邻两个字母一个键，组码取这一组首字母的小写。
const FOURTEEN_KEYPAD: &[u8; 26] = b"abcdedggujjlmbooqeatucqztz";
/// 全拼 14 键的十四个键，按键面从左到右、从上到下。组码是每组的首字母，与左列的大写字母项和 `'` 都不冲突。
const FOURTEEN_KEY_GROUPS: [&str; 14] = [
    "qw", "er", "ty", "ui", "op", "as", "df", "gh", "jk", "l", "zx", "cv", "bn", "m",
];
/// 全拼十八键的字母归组，组码取键面首字母。
const EIGHTEEN_KEYPAD: &[u8; 26] = b"abxswffhijjlmbipqrsruvwxyz";
const EIGHTEEN_KEY_GROUPS: [&str; 18] = [
    "q", "we", "rt", "y", "u", "io", "p", "a", "sd", "fg", "h", "jk", "l", "z", "xc", "v", "bn",
    "m",
];

/// 十八键双拼沿用全拼字母分组，并把分号归入 L。
pub(crate) fn eighteen_key_code(letter: u8) -> Option<u8> {
    if letter == b';' {
        Some(b'l')
    } else {
        KeyGrid::EighteenKey.code_of(letter)
    }
}

/// T9 expansion multiplies by three or four per digit, so only the leading digits become letter prefixes and the dictionary's own prefix search carries the rest (NK:189).
const ENGLISH_PREFIX_BUDGET: usize = 64;
const ENGLISH_LIMIT: usize = 5;
const ENGLISH_CANDIDATE_CAPACITY: usize = ENGLISH_PREFIX_BUDGET * ENGLISH_LIMIT;
/// 一个字母前缀最多缓存这么多行；词库里最大的三字母前缀 `con` 有 1034 个词，超过上限的前缀组照旧逐次查询。
const ENGLISH_CACHED_PREFIX_ROWS: usize = 2048;
/// 九宫格简拼一次最多展开的码数：每个数字取它键上能作音节首字母的字母，五个数字最多 4^5 = 1024 个，五字以内的词都查得到；更长的输入展开太多，不查简拼。
const INITIALS_CODE_LIMIT: usize = 1024;
/// 简拼一次最多取的行数，按权重从高到低。
const INITIALS_ROW_LIMIT: usize = 64;
/// 九键双拼按声母那几位找词（见 `refresh`）从几个音节起查：两个音节的切分不超过 `PATH_LIMIT` 条，不用查。
const SHUANGPIN_WORD_MIN_SYLLABLES: usize = 3;
/// 九键双拼按声母那几位找词时每张首字母表取的行数，按权重；取出来还要按编码筛，所以比简拼行多取。
const SHUANGPIN_WORD_SCAN_LIMIT: usize = 1024;
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

/// 九宫格会话把字母归成组码用的键盘。一个组码键上有几个字母，解码只认组码：路径搜索、锁定、切分、整句、简拼和英文混入都与「字母怎么变成码」无关，只经由这里的表。
///
/// 九键双拼不按字母归组，而是一个音节两位数字（声母一位、韵母一位），表由用户选的方案给出，所以它带着那张表（进程里按内容只留一份，见 `nine_key_shuangpin`）；相等按表的引用比较。
#[derive(Clone, Copy, Debug)]
pub enum KeyGrid {
    /// 电话键盘：组码是 `2`–`9`。
    NineKey,
    /// 全拼 14 键：组码是每组首字母的小写 `q e t u o a d g j l z c b m`。
    FourteenKey,
    /// 全拼十八键：八组双字母键与十组单字母键。
    EighteenKey,
    /// 九键双拼：组码是方案用到的数字，可以有 `1` 和 `0`。
    NineKeyShuangpin(&'static NineKeyShuangpin),
    /// 14 键双拼：当前 26 键方案投到相邻字母组，每个音节仍占两键，分号与 L 共键。
    FourteenKeyShuangpin(&'static NineKeyShuangpin),
    /// 双拼 18 键：按 Q WE RT Y U IO P／A SD FG H JK L／Z XC V BN M 分组。
    EighteenKeyShuangpin(&'static NineKeyShuangpin),
}

impl PartialEq for KeyGrid {
    fn eq(&self, other: &Self) -> bool {
        match (self, other) {
            (Self::NineKey, Self::NineKey)
            | (Self::FourteenKey, Self::FourteenKey)
            | (Self::EighteenKey, Self::EighteenKey) => true,
            (Self::NineKeyShuangpin(left), Self::NineKeyShuangpin(right))
            | (Self::FourteenKeyShuangpin(left), Self::FourteenKeyShuangpin(right))
            | (Self::EighteenKeyShuangpin(left), Self::EighteenKeyShuangpin(right)) => {
                std::ptr::eq(*left, *right)
            }
            _ => false,
        }
    }
}

impl Eq for KeyGrid {}

impl std::hash::Hash for KeyGrid {
    fn hash<H: std::hash::Hasher>(&self, state: &mut H) {
        std::mem::discriminant(self).hash(state);
        if let Some(table) = self.shuangpin_table() {
            std::ptr::hash(table, state);
        }
    }
}

impl KeyGrid {
    fn shuangpin_table(self) -> Option<&'static NineKeyShuangpin> {
        match self {
            Self::NineKeyShuangpin(table)
            | Self::FourteenKeyShuangpin(table)
            | Self::EighteenKeyShuangpin(table) => Some(table),
            _ => None,
        }
    }

    /// `a..=z` 各自的组码；双拼网格使用音节编码表，不使用这张字母归组表。
    fn keypad(self) -> Option<&'static [u8; 26]> {
        match self {
            Self::NineKey => Some(KEYPAD),
            Self::FourteenKey => Some(FOURTEEN_KEYPAD),
            Self::EighteenKey => Some(EIGHTEEN_KEYPAD),
            Self::NineKeyShuangpin(_)
            | Self::FourteenKeyShuangpin(_)
            | Self::EighteenKeyShuangpin(_) => None,
        }
    }

    /// 字母所在键的组码；字母双拼网格的分号归入 L，九键双拼不按字母归组。
    pub fn code_of(self, letter: u8) -> Option<u8> {
        if matches!(self, Self::EighteenKeyShuangpin(_)) {
            return eighteen_key_code(letter);
        }
        if matches!(self, Self::FourteenKeyShuangpin(_)) {
            return if letter == b';' {
                Some(b'l')
            } else {
                Self::FourteenKey.code_of(letter)
            };
        }
        let keypad = self.keypad()?;
        letter
            .is_ascii_lowercase()
            .then(|| keypad[usize::from(letter - b'a')])
    }

    /// 组码键上的字母；不是这种键盘的组码时为空。九键双拼是这个键上能起头一个音节的字母（声母的首字母和从这个键开始的零声母音节的首字母）。
    pub fn letters(self, code: u8) -> &'static str {
        match self {
            Self::NineKey if (b'2'..=b'9').contains(&code) => {
                DIGIT_LETTERS[usize::from(code - b'0')]
            }
            Self::NineKey => "",
            Self::FourteenKey => FOURTEEN_KEY_GROUPS
                .iter()
                .find(|group| group.as_bytes().first() == Some(&code))
                .copied()
                .unwrap_or(""),
            Self::EighteenKey => EIGHTEEN_KEY_GROUPS
                .iter()
                .find(|group| group.as_bytes().first() == Some(&code))
                .copied()
                .unwrap_or(""),
            Self::NineKeyShuangpin(table)
            | Self::FourteenKeyShuangpin(table)
            | Self::EighteenKeyShuangpin(table) => table.letters(code),
        }
    }

    /// `byte` 是这种键盘的一个组码键。九键双拼是方案用到的数字。
    pub fn is_code(self, byte: u8) -> bool {
        match self {
            Self::NineKeyShuangpin(table)
            | Self::FourteenKeyShuangpin(table)
            | Self::EighteenKeyShuangpin(table) => table.uses_key(byte),
            _ => !self.letters(byte).is_empty(),
        }
    }

    /// 拼音（或任何字母串）的组码；`a..=z` 以外的字节（`'`）跳过。九键双拼按 `'` 分开的音节编码，完整音节两位、单独的声母一位，有一段两样都不是时为空。
    pub fn encode(self, pinyin: &str) -> String {
        let Some(keypad) = self.keypad() else {
            return match self {
                Self::NineKeyShuangpin(table)
                | Self::FourteenKeyShuangpin(table)
                | Self::EighteenKeyShuangpin(table) => table.encode(pinyin),
                _ => String::new(),
            };
        };
        pinyin
            .bytes()
            .filter(u8::is_ascii_lowercase)
            .map(|letter| char::from(keypad[usize::from(letter - b'a')]))
            .collect()
    }

    /// 返回相容组码覆盖的输入长度；按字母分组时只比较共同前缀，不拥有中间编码。
    fn matching_code_prefix_len(self, pinyin: &str, digits: &str) -> Option<usize> {
        let Some(keypad) = self.keypad() else {
            return self.shuangpin_table()?.matching_prefix_len(pinyin, digits);
        };
        let digits = digits.as_bytes();
        let mut covered = None;
        for (index, letter) in pinyin.bytes().filter(u8::is_ascii_lowercase).enumerate() {
            let Some(&digit) = digits.get(index) else {
                return Some(index);
            };
            if keypad[usize::from(letter - b'a')] != digit {
                return None;
            }
            covered = Some(index + 1);
        }
        covered
    }

    /// 一个完整音节（锁定的拼写、左列的音节、整句种子里的音节）占几个组码：全拼按字母计数，双拼总是两个。
    pub(crate) fn syllable_code_len(self, syllable: &str) -> usize {
        match self {
            Self::NineKeyShuangpin(_)
            | Self::FourteenKeyShuangpin(_)
            | Self::EighteenKeyShuangpin(_) => 2,
            _ => syllable.len(),
        }
    }

    /// 一行候选的全拼（`'` 连接的完整音节）一共占几个组码，不分配。
    fn reading_code_len(self, canonical_pinyin: &str) -> usize {
        match self {
            Self::NineKeyShuangpin(_)
            | Self::FourteenKeyShuangpin(_)
            | Self::EighteenKeyShuangpin(_) => 2 * reading_syllable_count(canonical_pinyin),
            _ => canonical_pinyin
                .bytes()
                .filter(u8::is_ascii_lowercase)
                .count(),
        }
    }

    /// 组码本身是不是可上屏的文字。九键的数字是：左列在没有锁定时给出「原样上屏这一键」，`CommitRaw` 上屏数字。十四键或十八键的组码只是这一组的首字母，原样上屏没有意义：左列没有这一项，`CommitRaw` 改为结束组字。九键双拼的数字和九键一样是数字本身。
    pub fn offers_raw_key(self) -> bool {
        match self {
            Self::NineKey | Self::NineKeyShuangpin(_) => true,
            Self::FourteenKey
            | Self::EighteenKey
            | Self::FourteenKeyShuangpin(_)
            | Self::EighteenKeyShuangpin(_) => false,
        }
    }

    /// 组码是数字，从 `character` 进来：九键和九键双拼。数字在这两种网格下是组字的键，不是选词键。
    pub fn uses_digits(self) -> bool {
        matches!(self, Self::NineKey | Self::NineKeyShuangpin(_))
    }

    /// 能拼英文词（英文九键、英文混入）。双拼网格按声母和韵母编码，不按字母分组拼英文。
    fn spells_english(self) -> bool {
        self.shuangpin_table().is_none()
    }

    /// 这种键盘的拼写表，每种键盘（九键双拼是每张表）在进程里建一次。
    fn spelling_table(self) -> &'static SpellingTable {
        match self {
            Self::NineKey => {
                static TABLE: OnceLock<SpellingTable> = OnceLock::new();
                TABLE.get_or_init(|| SpellingTable::build(self, intact_pinyin_list()))
            }
            Self::FourteenKey => {
                static TABLE: OnceLock<SpellingTable> = OnceLock::new();
                TABLE.get_or_init(|| SpellingTable::build(self, intact_pinyin_list()))
            }
            Self::EighteenKey => {
                static TABLE: OnceLock<SpellingTable> = OnceLock::new();
                TABLE.get_or_init(|| SpellingTable::build(self, intact_pinyin_list()))
            }
            Self::NineKeyShuangpin(table)
            | Self::FourteenKeyShuangpin(table)
            | Self::EighteenKeyShuangpin(table) => table
                .spelling
                .get_or_init(|| SpellingTable::build_shuangpin(self, table)),
        }
    }
}

type Path = Vec<String>;

pub struct NineKeySession {
    paths: RuntimePaths,
    learning: bool,
    frequency: FrequencyAdjustmentOptions,
    fuzzy: FuzzyPinyinOptions,
    english_options: EnglishInputOptions,
    /// 候选里混入 emoji、颜文字和符号（共享偏好 `mixed_input.emoji` / `mixed_input.kaomoji` / `mixed_input.symbols`）。引擎层默认都关，宿主按偏好打开。
    expressive: MixedExpressiveOptions,
    /// 候选混输总开关（`SessionOptions::mixed_candidates`）：关着时英文、emoji、颜文字、符号和行内日期时间都不混进候选。
    mixed_candidates_enabled: bool,
    /// 组字里的日期时间行（#5952）读的墙钟；`None` 时不出这些行。跟日期时间模式同一个开关。
    inline_date_time: Option<LocalClock>,
    /// 句子联想设置（`SessionOptions::sentence_association`），与 26 键相同：`word_lattice` 关掉时不出词网格整句行，`neural_keyboard` 打开时用键盘模型给整句重排（#6059）。
    sentence_association: SentenceAssociationOptions,
    /// `SessionOptions::sentence_alternatives`：每条切分交回全部整句读法，而不是只交回最好的一条。
    sentence_alternatives: bool,
    /// `SessionOptions::first_page_rows`：首页放得下几行，决定全长组的补全词是按权重并进来还是接在整组末尾。
    first_page_rows: FirstPageRows,
    /// 键盘模型读的上屏上下文，只留最后 `CONTEXT_CHARACTERS` 个字；`Session::set_rescoring_context` 随时更新。
    rescoring_context: String,
    /// 键盘模型，`neural_keyboard` 打开后第一次重排时加载；内层 `None` 是模型读不到。九键不让词库在每条切分里各重排一次，见 `rerank_sentences`。
    keyboard: Option<Option<NeuralReranker>>,
    /// 键盘模型最近几次给出的整句顺序，最新的在后，见 `rerank_sentences`。换掉模型（关掉 `neural_keyboard`）时清空。
    rerank_cache: Vec<RerankEntry>,
    /// 把选中的词记进 26 键共用的个人上下文模型（`SessionOptions::personal_context`，还要 `learning`），简拼行据此把用户用过的词排到前面（#6185）。
    personal_context: bool,
    /// `finish` 逐段代选时不把默认选择写成最近候选位置。
    remember_recent: bool,
    digits: String,
    locked: Vec<String>,
    /// 用户用 `'` 切开音节的数字位置，升序，都在已锁定的部分之后。和锁定的拼写不同，切分只定下一个音节在哪里结束，两边数字的各种读法都还保留：`94'26` 可以是 xi'an，也可以是 yi'an，但不会是 xian。
    splits: Vec<usize>,
    spellings: Vec<String>,
    /// 拼写匹配的排序索引，刷新时复用，避免每次为借用的音节引用重新申请临时向量。
    spelling_match_indices: Vec<usize>,
    /// `SessionSnapshot::nine_key_reading`，随候选一起重建。
    reading: String,
    candidates: Vec<WordItem>,
    /// 前几次刷新排在前面、覆盖全部数字的几行的全拼（`'` 连接）。下一次刷新把它们去掉最后一个音节，接上余下数字的切分一起查，见 `seeded_paths`。
    sentence_seeds: Vec<String>,
    /// 算下一批种子用的空缓冲，和 `sentence_seeds` 轮换，刷新时不再分配。
    seed_buffer: Vec<String>,
    /// 九键切分路径的输出缓冲；相邻刷新复用路径及其音节字符串。
    path_buffer: Vec<Path>,
    /// 活动拼音组字复用搜索页；组字结束时释放页容器。
    path_scratch: PathScratch<'static>,
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
    prior: Option<SyllablePrior<'static>>,
    english: Option<EnglishDictionary>,
    /// 完整分组码的 SQLite GLOB 模式；热刷新复用容量。
    english_pattern: String,
    /// 上一次英文查询的数字和字母前缀下的全部匹配行，见 `EnglishKeypadCache`。
    english_rows: Option<EnglishKeypadCache>,
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
    /// 把字母归成组码的键盘，`digits` 里存的是它的组码。
    grid: KeyGrid,
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
            mixed_candidates_enabled: true,
            inline_date_time: None,
            sentence_association: SentenceAssociationOptions::default(),
            sentence_alternatives: false,
            first_page_rows: FirstPageRows::default(),
            rescoring_context: String::new(),
            keyboard: None,
            rerank_cache: Vec::new(),
            personal_context: true,
            remember_recent: true,
            digits: String::new(),
            locked: Vec::new(),
            splits: Vec::new(),
            spellings: Vec::new(),
            spelling_match_indices: Vec::new(),
            reading: String::new(),
            candidates: Vec::new(),
            // 两个缓冲一开始就留够容量，按键时轮换着用，不再分配。
            sentence_seeds: Vec::with_capacity(SEED_RETAIN_LIMIT),
            seed_buffer: Vec::with_capacity(SEED_RETAIN_LIMIT),
            path_buffer: Vec::new(),
            path_scratch: PathScratch::default(),
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
            english_pattern: String::new(),
            english_rows: None,
            lock_undo: Vec::new(),
            initial: None,
            single_character: false,
            strokes: String::new(),
            stroke_dictionary: PathBuf::new(),
            stroke: None,
            stroke_texts: HashSet::new(),
            grid: KeyGrid::NineKey,
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

    /// 候选混输总开关（`SessionOptions::mixed_candidates`）；与英文选项一样只在建会话时设置。
    pub fn set_mixed_candidates_enabled(&mut self, enabled: bool) {
        self.mixed_candidates_enabled = enabled;
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
        self.sentence_association = association.clone();
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

    /// 首页行数只影响词库要不要并入补全词；词库已经打开时同步给它。`rows` 用 `FirstPageRows`，0 在这里没有入口，全拼和九键的归一只有 [`FirstPageRows::new`] 一处。
    pub fn set_first_page_rows(&mut self, rows: FirstPageRows) {
        if self.first_page_rows == rows {
            return;
        }
        self.first_page_rows = rows;
        if let Some(dictionary) = self.dictionary.as_mut() {
            dictionary.set_first_page_rows(rows);
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
        let association = self.sentence_association.clone();
        SentenceAssociationOptions {
            word_lattice: association.word_lattice || association.neural_keyboard,
            neural_keyboard: false,
            show_next_on_duplicate: association.show_next_on_duplicate,
            ..association
        }
    }

    /// 打开词库要用的选项。在这里先算出来，是因为 `dictionary_association` 要读整个 `self`，
    /// 不能在 `get_or_insert_with` 的闭包里调用；三个取用点共用这一处。
    fn dictionary_options(&self) -> DictionaryOptions {
        DictionaryOptions {
            association: self.dictionary_association(),
            alternatives: self.sentence_alternatives,
            first_page_rows: self.first_page_rows,
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

    /// 换一种网格。已打的组码和整句种子都是按原来的网格编的，所以先结束组字，再丢掉种子。
    pub fn set_grid(&mut self, grid: KeyGrid) {
        self.command(Command::Cancel);
        self.sentence_seeds.clear();
        self.grid = grid;
    }

    /// `2`..=`9`；到 `DIGIT_LIMIT` 个数字时按 `NINE_KEY_DIGIT_LIMIT` 处理（运行时会先用 `limit_pick_ready` 判断能不能替用户选掉首选再接着打）。组字中按 `'` 在光标处切开音节；在同一处再切一次，或者紧跟在锁定的拼写之后切，都没有作用。光标在数字中间时数字插在光标处，光标落在锁定的拼写里时从那个音节起解除锁定。
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
        if !self.grid.is_code(digit) {
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

    /// 组码已经到 `DIGIT_LIMIT`、光标在末尾，再按一个组码 `digit` 时，先选掉首选能不能腾出位置：首选要占用至少一位组码。光标在中间（用户在改前面的码）、首选不占组码或者没有候选时不行，那一键照旧按上限拦下。
    pub fn limit_pick_ready(&self, digit: u8) -> bool {
        self.active()
            && self.grid.is_code(digit)
            && self.digits.len() >= DIGIT_LIMIT
            && self.caret_position() == self.digits.len()
            && self
                .candidates
                .first()
                .is_some_and(|candidate| !candidate.pinyin.is_empty())
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
                end += self.grid.syllable_code_len(spelling);
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
                let end = offset
                    + self
                        .grid
                        .syllable_code_len(&choice)
                        .min(self.digits.len() - offset);
                let replaced = self.digits[offset..end].to_string();
                let before = self.digits.len();
                self.digits
                    .replace_range(offset..end, &self.grid.encode(&choice));
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
        let end = (offset + self.grid.syllable_code_len(&spelling)).min(self.digits.len());
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
        let selected_is_initials = is_initials_row(selected);
        let selected_canonical_pinyin = if self.learning
            && (selected_source.is_dictionary() || selected_source.is_sentence_learning())
        {
            selected.canonical_pinyin.to_string()
        } else {
            String::new()
        };
        // 用过的简拼行在九键里的位置只由个人上下文模型里的计数决定（`boost_used_initials`）。这一行会记进模型时不再调全局词频：两边都挪的话一次选词挪两次，全局权重还会连带改掉 26 键里这个词的位置（#6185）。
        let recorded_initials =
            self.records_personal_use(selected_source, &selected_word) && selected_is_initials;
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
        // 最近选中的候选要在下一次相同数字输入中直接显示在首位。选定读法后，首候选也是显式选择，另外记到未锁定的上下文；不把单字写进个人模型，也不改主词库权重。
        if self.learning
            && selected_source.is_dictionary()
            && !selected_canonical_pinyin.is_empty()
            && !selected_is_initials
            && selected_pinyin_length == self.digits.len()
            && self.remember_recent
        {
            if index != 0 && selected_fixed_position == 0 {
                diagnostic = diagnostic.or(self.remember_recent_candidate(
                    &self.ranking_context(),
                    &selected_canonical_pinyin,
                    &selected_word,
                ));
            }
            if !self.locked.is_empty() {
                let unlocked_context = format!("nine-key:{}:", self.digits);
                // 两个上下文的固定位置互相独立，不能因为读法内未固定就覆盖未锁定上下文里同一候选的手动槽位。
                if !positions::is_fixed(
                    &self.paths.user(assets::USER_JOURNAL),
                    &unlocked_context,
                    &selected_canonical_pinyin,
                    &selected_word,
                ) {
                    diagnostic = diagnostic.or(self.remember_recent_candidate(
                        &unlocked_context,
                        &selected_canonical_pinyin,
                        &selected_word,
                    ));
                }
            }
        }
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
        let options = self.dictionary_options();
        match self
            .dictionary
            .get_or_insert_with(|| open_dictionary(&self.paths, options))
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

    /// `first_index` 越界时与 `CommitRaw` 相同：九键上屏数字，14 键因此取首选。与全拼键盘的 `finish_composition` 相同，余下各段逐个按 `select` 选首选，造词也一样：用户先选掉的段和替他选的余下各段连成一个词存起来（选了「我滴」再打标点，存的是「我滴个天呐」），首选是整句行时存整句。
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
            let previous_remember_recent = self.remember_recent;
            self.remember_recent = false;
            let result = self.select(index);
            self.remember_recent = previous_remember_recent;
            if diagnostic.is_none() {
                diagnostic = result.diagnostic;
            }
            if let Some(text) = result.commit {
                if commit.is_empty() {
                    commit = text;
                } else {
                    commit.push_str(&text);
                }
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
                // 14 键的组码只是每组的首字母（按「你好」的键是 `bugao`），不是用户想写的字母：原样上屏改为结束组字、逐段取首选，与 `finish(0)` 相同。没有候选时只剩组码可上屏。
                if !self.grid.offers_raw_key() && !self.candidates.is_empty() {
                    return self.finish(0);
                }
                let raw = std::mem::take(&mut self.digits);
                self.clear_composition();
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
        self.path_scratch.pages.clear();
        // 选词学习可能刚改了英文权重。
        self.english_rows = None;
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
        let selected = self.candidates[index].clone();
        let replaces_recent = selected.fixed_position == 0
            && self
                .candidates
                .iter()
                .any(|candidate| candidate.fixed_position == 1);
        let diagnostic = self.adjust_frequency(index, true);
        let position_diagnostic = replaces_recent.then(|| {
            positions::set_fixed_position(
                &self.paths.user(assets::USER_JOURNAL),
                &self.ranking_context(),
                &selected.canonical_pinyin,
                &selected.word,
                1,
            )
            .err()
            .map(|_| diagnostics::NINE_KEY_POSITION_NOT_PERSISTED.to_string())
        });
        self.refresh();
        KeyResult::handled().with_diagnostic(diagnostic.or(position_diagnostic.flatten()))
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
        let locked_preedit_length = self
            .locked
            .iter()
            .map(String::len)
            .sum::<usize>()
            .saturating_add(self.locked.len().saturating_sub(1));
        let mut preedit = String::with_capacity(
            locked_preedit_length
                .saturating_add(self.digits.len().saturating_sub(locked_length))
                .saturating_add(self.splits.len())
                .saturating_add(usize::from(
                    self.active() && locked_length < self.digits.len() && !self.locked.is_empty(),
                )),
        );
        for (index, spelling) in self.locked.iter().enumerate() {
            if index != 0 {
                preedit.push('\'');
            }
            preedit.push_str(spelling);
        }
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
                let at = locked_preedit_length + usize::from(!self.locked.is_empty());
                let mut initial_text = [0; 4];
                let initial_text = char::from(initial.letter).encode_utf8(&mut initial_text);
                preedit.replace_range(at..=at, initial_text);
            }
        }
        SessionSnapshot {
            prediction: false,
            scheme: if matches!(
                self.grid,
                KeyGrid::FourteenKeyShuangpin(_) | KeyGrid::EighteenKeyShuangpin(_)
            ) {
                SchemeType::Shuangpin
            } else {
                SchemeType::Quanpin
            },
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
                .map(|item| item.corrected_from.to_string())
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
        self.reading.clear();
        if !self.active() {
            self.path_scratch.pages.clear();
            self.spellings.clear();
            // 筛选和撤销记录只属于这一次组字。
            self.sentence_seeds.clear();
            self.lock_undo.clear();
            self.initial = None;
            self.single_character = false;
            self.strokes.clear();
            return;
        }
        if self.english_only || !self.pinyin {
            self.path_scratch.pages.clear();
            self.spellings.clear();
            // No syllables to offer and no pinyin to look up: the digits stand for letters only.
            self.candidates = self.english_candidates(false);
            return;
        }
        // 每个锁定都有一条撤销记录，哪怕是作废的。
        self.lock_undo.resize_with(self.locked.len(), || None);
        let grid = self.grid;
        let table = grid.spelling_table();
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
        let dictionary_options = self.dictionary_options();
        let mut alternatives = std::mem::take(&mut self.path_buffer);
        let alternatives = if remaining.is_empty() {
            self.spellings.clear();
            alternatives.clear();
            alternatives.push(Vec::new());
            alternatives
        } else {
            let dictionary = self
                .dictionary
                .get_or_insert_with(|| open_dictionary(&self.paths, dictionary_options.clone()));
            let prior = self
                .prior
                .get_or_insert_with(|| SyllablePrior::from_dictionary(dictionary, table));
            table.spellings_for_into(
                remaining,
                locked_length,
                splits.first().copied(),
                prior,
                &mut self.spellings,
                &mut self.spelling_match_indices,
            );
            self.spellings.retain(|syllable| starts_right(syllable));
            table.split_paths_with_scratch(
                remaining,
                splits,
                prior,
                QUERY_PATH_LIMIT,
                &mut self.path_scratch,
                &mut alternatives,
            );
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
                self.spellings
                    .iter()
                    .filter(|spelling| grid.syllable_code_len(spelling) <= remaining.len())
                    .map(|spelling| vec![spelling.clone()]),
            );
            alternatives
        };

        let mut locked_key = String::new();
        append_path_key(&mut locked_key, "", &self.locked);
        let mut dictionary = self
            .dictionary
            .get_or_insert_with(|| open_dictionary(&self.paths, dictionary_options.clone()))
            .row_cache_batch();
        let mut queried = (alternatives.len() > SMALL_QUERY_KEY_BATCH)
            .then(|| HashSet::with_capacity(alternatives.len()));
        // 接管已清空的会话候选向量，刷新时复用行容器容量。
        let mut candidates = std::mem::take(&mut self.candidates);
        candidates.clear();
        let mut key = String::with_capacity(locked_key.len() + self.digits.len() * 4 + 1);
        // 各条切分的前缀组彼此大量重复，一次刷新会推入上万行，见 `push_ranked`。
        let mut leading = LeadingMap::default();
        // Only a split the user typed says where a syllable ends; without one, `3` must keep 的 (a completion of d) ahead of the rarer 额 (e).
        let prefer_exact = !self.splits.is_empty();
        // 每个数字都能当一个音节的首字母时也按简拼查（`68` 是 m't：明天、每天）。用户在每个数字之间都打了切分（`6'8`），说的就是简拼，简拼行排在前面；没打切分时数字也可能是完整音节（`68` 是 mu），简拼行排在同样覆盖的音节行之后。
        // 在展开面板里选定了首字母时用户是在逐个拼音节，简拼行不经过 `starts_right` 的首字母过滤，这时不查简拼。
        let initials =
            self.locked.is_empty() && initial.is_none() && initials_apply(remaining.len(), splits);
        let initials_lead = initials && !splits.is_empty();
        // 模糊页已拥有结果；九键双拼的覆盖核对会另建编码，保留原单次核对路径。
        let filter_dominated = self.fuzzy.rules == 0
            && matches!(
                grid,
                KeyGrid::NineKey | KeyGrid::FourteenKey | KeyGrid::EighteenKey
            );
        for (index, path) in alternatives.iter().enumerate() {
            append_path_key(&mut key, &locked_key, path);
            if key.is_empty() || !query_key_is_new(&alternatives, index, &key, queried.as_ref()) {
                continue;
            }
            let full_len = self.locked.len() + path.len();
            let accepts_reading = |candidate: &WordItem, cached: bool| {
                let word = candidate.word.as_str();
                let pinyin = candidate.pinyin.as_str();
                let canonical = candidate.canonical_pinyin.as_str();
                let fuzzy = candidate.fuzzy;
                if self.single_character_only && is_han_phrase(word) {
                    return false;
                }
                let canonical = if canonical.is_empty() {
                    pinyin
                } else {
                    canonical
                };
                // 多出音节的行是越过本条路径的补全，不参与这条读法。
                if reading_syllable_count(canonical) > full_len {
                    return false;
                }
                let matched = if fuzzy { pinyin } else { canonical };
                if !agrees_with_locked(matched, &locked_key) {
                    return false;
                }
                if let Some(letter) = initial {
                    // 模糊音按实际匹配的拼写核对已选声母。
                    let next = matched
                        .split('\'')
                        .nth(self.locked.len())
                        .and_then(|syllable| syllable.as_bytes().first());
                    if next != Some(&letter) {
                        return false;
                    }
                }
                if cached && filter_dominated {
                    if let Some(best) = leading_rank(&leading, &candidates, word) {
                        let Some(covered) = grid.matching_code_prefix_len(matched, &self.digits)
                        else {
                            return false;
                        };
                        // 只查已接纳的前页结果；本页内的同词仍由 `push_ranked` 处理。
                        if best
                            <= rank_key_for_reading(
                                candidate,
                                covered,
                                canonical,
                                prefer_exact,
                                initials_lead,
                                grid,
                            )
                        {
                            return false;
                        }
                    }
                }
                true
            };
            for mut candidate in
                dictionary.query_filtered_rows(&key, &key, self.fuzzy, accepts_reading)
            {
                let canonical = take_candidate_canonical_pinyin(&mut candidate);
                let matched = if candidate.fuzzy {
                    candidate.pinyin.as_str()
                } else {
                    canonical.as_str()
                };
                // 组码覆盖只算一次；九键双拼的既有实现会申请临时编码。
                let Some(covered) = grid.matching_code_prefix_len(matched, &self.digits) else {
                    continue;
                };
                // 词库行与缓存共享，整段换新比在共享的那份上清空再写少一次复制。
                candidate.pinyin = SharedString::from(&self.digits[..covered]);
                candidate.canonical_pinyin = canonical;
                push_ranked(
                    &mut candidates,
                    &mut leading,
                    candidate,
                    prefer_exact,
                    initials_lead,
                    grid,
                );
            }
            if let Some(seen) = queried.as_mut() {
                seen.insert(key.clone());
            }
        }
        // 九键双拼一个两位编码对应五六个音节，三个音节以上时，按音节频度留下的切分（每个位置 `PATH_LIMIT` 条）常常没有词的那一条：`22942288` 有一千多种读法，百依百顺 的 bai'yi'bai'shun 排不进去。词可以不靠切分找：按每个音节的声母那一位（第 1、3、5… 位）查简拼，只留编码正好以这串数字开头、音节数也对得上的词。全拼九宫格一个音节的码长不定，没有这一步。
        if let Some(table) = grid.shuangpin_table() {
            let codes = (self.locked.is_empty()
                && initial.is_none()
                && splits.is_empty()
                && !self.single_character_only)
                .then(|| shuangpin_word_codes(grid, remaining))
                .flatten();
            if let Some(codes) = codes {
                for mut candidate in
                    dictionary.query_jianpin_codes_per_table(&codes, SHUANGPIN_WORD_SCAN_LIMIT)
                {
                    if !shuangpin_word_spells(table, &candidate.canonical_pinyin, remaining) {
                        continue;
                    }
                    candidate.pinyin.clear();
                    candidate.pinyin.push_str(&self.digits);
                    push_ranked(
                        &mut candidates,
                        &mut leading,
                        candidate,
                        prefer_exact,
                        initials_lead,
                        grid,
                    );
                }
            }
        }
        let mut boost = None;
        if let Some(codes) = initials
            .then(|| initials_codes(grid, remaining, INITIALS_CODE_LIMIT))
            .flatten()
        {
            // 用户用过的词（个人上下文模型里有计数，26 键选的也算）不能被按权重的截断截掉（#6185）：模型有记录时多扫一些行，用过的先留下。学习或个人上下文关掉时不读个人数据，与 26 键的 `personal_context_applies` 相同。
            let personal =
                self.learning && self.personal_context && !dictionary.personal_model().is_empty();
            // 九键双拼的简拼按首字母查出来的行还要逐个核对声母所在的键：`z` 同时查出 z 和 zh 开头的词，而它们常在不同的键上。核对会筛掉一部分，所以先多取一些行，核对之后再截。
            let shuangpin = match grid {
                KeyGrid::NineKeyShuangpin(table)
                | KeyGrid::FourteenKeyShuangpin(table)
                | KeyGrid::EighteenKeyShuangpin(table) => Some(table),
                _ => None,
            };
            let mut rows = if personal {
                dictionary.query_jianpin_codes_per_table(&codes, INITIALS_SCAN_LIMIT)
            } else if shuangpin.is_some() {
                dictionary.query_jianpin_codes(&codes, INITIALS_SCAN_LIMIT)
            } else {
                dictionary.query_jianpin_codes(&codes, INITIALS_ROW_LIMIT)
            };
            if let Some(table) = shuangpin {
                rows.retain(|row| table.spells_initials(&row.canonical_pinyin, remaining));
                if !personal {
                    rows.truncate(INITIALS_ROW_LIMIT);
                }
            }
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
                candidate.pinyin.clear();
                candidate.pinyin.push_str(&self.digits);
                push_ranked(
                    &mut candidates,
                    &mut leading,
                    candidate,
                    prefer_exact,
                    initials_lead,
                    grid,
                );
            }
        }
        drop(dictionary);
        let filtering = self.single_character || !self.strokes.is_empty();
        if filtering {
            let strokes = (!self.strokes.is_empty()).then_some(&self.stroke_texts);
            candidates.retain(|item| passes_filter(&item.word, self.single_character, strokes));
        }
        rank_candidates(
            &mut candidates,
            prefer_exact,
            initials_lead,
            boost.as_ref(),
            grid,
        );
        self.rerank_sentences(&mut candidates);
        let mut seeds = std::mem::take(&mut self.seed_buffer);
        next_seeds(
            grid,
            &candidates,
            &self.digits,
            &mut self.sentence_seeds,
            &mut seeds,
        );
        self.seed_buffer = std::mem::replace(&mut self.sentence_seeds, seeds);
        // emoji、颜文字和符号按拼音查，读法的先后要参照排好的拼音候选，所以在插入英文行之前查；插入在英文行之后，它们也可以接在英文词后面。单字、笔画筛选针对的是汉字，筛选时不混入。
        let (emoji, kaomoji, symbols) = if filtering {
            (Vec::new(), Vec::new(), Vec::new())
        } else {
            self.expressive_candidates(&alternatives, &candidates)
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
            // 有权重的英文保留第二位，零权重词仍接在拼音末尾；额外英文只占首页剩余空间。
            let first = english.remove(0);
            let slot = if first.weight > 0 {
                candidates.len().min(1)
            } else {
                candidates.len()
            };
            candidates.insert(slot, first);
            let extra = self
                .english_options
                .first_page_count
                .saturating_sub(1)
                .min(self.first_page_rows.get().saturating_sub(slot + 1))
                .min(english.len());
            if extra > 0 {
                candidates.splice(slot + 1..slot + 1, english.drain(..extra));
            }
            candidates.extend(english);
        }
        let mut candidates = insert_expressive_rows(candidates, emoji, kaomoji, symbols);
        // 选了首字母或在筛选时用户是在逐字拼，不加日期时间行。
        if let Some(clock) = self
            .inline_date_time
            .as_ref()
            .filter(|_| self.mixed_candidates_enabled && initial.is_none() && !filtering)
        {
            let digits = self.digits.as_str();
            if let Some((_keyword, anchor, kind)) =
                inline_date_time_keyword(|keyword| keyword_spells_digits(grid, keyword, digits))
            {
                // 九键不给「📅日期」入口：换页交互只在 26 键会话实现。
                insert_inline_date_time(&mut candidates, anchor, kind, digits, || clock(), None);
            }
        }
        positions::apply_fixed_positions(
            &self.paths.user(assets::USER_JOURNAL),
            &format!("nine-key:{}:{locked_key}", self.digits),
            &mut candidates,
            false,
            None,
            false,
        );
        keep_expressive_rows_off_the_lead(&mut candidates);
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
        let mut reading = std::mem::take(&mut self.reading);
        self.write_reading(candidates.first(), &mut reading);
        self.reading = reading;
        self.candidates = candidates;
        self.path_buffer = alternatives;
        let remaining = remaining_digits(&self.digits, locked_length);
        if remaining.is_empty() {
            // 数字全部锁定时，左列还是最后一次锁定时的选项，可以换选。
            if let Some(Some(undo)) = self.lock_undo.last() {
                self.spellings = undo.choices.clone();
            }
        } else {
            self.append_key_choices(locked_length);
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
        let ranked: &[String] = if let Some(keyboard) = keyboard.as_mut() {
            let (scored, scored_len) = collect_lattice_sentences(candidates);
            let scored = &scored[..scored_len];
            let context = &self.rescoring_context;
            let ranked = if let Some(index) = self
                .rerank_cache
                .iter()
                .position(|entry| rerank_cache_matches(entry, context, scored))
            {
                // 缓存命中直接借用已拥有的顺序；不再复制签名或排名字符串。
                self.rerank_cache[index].ranked.as_slice()
            } else {
                // 缓存未命中时才拥有签名；命中路径只借用当前候选行比较。
                let signature: Vec<(String, i64)> = scored
                    .iter()
                    .flatten()
                    .map(|item| (item.word.to_string(), comparable_weight(item)))
                    .collect();
                let mut sentences: Vec<SentencePath> = scored
                    .iter()
                    .flatten()
                    .map(|item| SentencePath {
                        sentence: item.word.to_string(),
                        // 重排器只读取词面和静态分；规范读法和词段不会参与模型评分，
                        // 也不会在重排后回到候选行，因此不必为临时路径复制它们。
                        key: String::new(),
                        log_prob: comparable_weight(item) as f64 / 1000.0,
                        words: Vec::new(),
                        typo_edges: 0,
                    })
                    .collect();
                let ranked = if keyboard.rerank(&mut sentences, context) {
                    sentences.into_iter().map(|path| path.sentence).collect()
                } else {
                    Vec::new()
                };
                if self.rerank_cache.len() >= RERANK_CACHE_ENTRIES {
                    self.rerank_cache.remove(0);
                }
                self.rerank_cache.push(RerankEntry {
                    context: context.clone(),
                    sentences: signature,
                    ranked,
                });
                self.rerank_cache.last().unwrap().ranked.as_slice()
            };
            ranked
        } else {
            &[]
        };
        place_keyboard_pick(
            candidates,
            ranked,
            self.sentence_association.show_next_on_duplicate,
            self.sentence_association.word_lattice,
        );
    }

    /// 按数字串可能的读法查要混入的 emoji、颜文字和符号，开关都关、数字不足 `MIXED_EXPRESSIVE_MINIMUM_INPUT` 个时不查。符号要读法与名称的全拼完全相等（`query_symbol_readings`），不认首字母。
    ///
    /// 读法是锁定的拼写加上一条拼满其余数字的切分路径（`alternatives` 已经按切分和左列选的首字母筛过），只保留各个字母；只拼了开头一个音节、供部分选择用的路径不算，它对应的不是整串数字。读法按支持它的拼音候选在列表里的位置排先后（候选的全拼以这个读法开头），没有候选支持的按路径原来的先后排在后面，只查前 `EXPRESSIVE_READING_LIMIT` 种。命中的编码还要能在已确定的音节边界（锁定拼写的结尾、用户打的切分）处切成完整音节，见 `splits_into_syllables_at`。
    fn expressive_candidates(
        &self,
        alternatives: &[Path],
        candidates: &[WordItem],
    ) -> (Vec<ExpressiveRow>, Vec<ExpressiveRow>, Vec<SymbolRow>) {
        let enabled = self.mixed_candidates_enabled
            && (self.expressive.emoji_candidates
                || self.expressive.kaomoji_candidates
                || self.expressive.symbol_candidates);
        if !enabled || self.digits.len() < MIXED_EXPRESSIVE_MINIMUM_INPUT {
            return (Vec::new(), Vec::new(), Vec::new());
        }
        let mut boundaries: Vec<usize> = self
            .locked
            .iter()
            .scan(0, |end, spelling| {
                *end += self.grid.syllable_code_len(spelling);
                Some(*end)
            })
            .chain(self.splits.iter().copied())
            .collect();
        boundaries.sort_unstable();
        boundaries.dedup();
        // 边界是数字位置，读法是字母：按字母归组的键盘一个数字一个字母，所有读法共用一组边界；九键双拼的边界落在每条读法的不同字母位置上，按读法换算，边界相同的读法一起查。
        let mut groups: Vec<(Vec<usize>, Vec<String>)> = Vec::new();
        for (reading, letters) in self.expressive_readings(alternatives, &boundaries, candidates) {
            match groups
                .iter_mut()
                .find(|(known, _)| known.as_slice() == letters.as_ref())
            {
                Some((_, readings)) => readings.push(reading),
                None => groups.push((letters.into_owned(), vec![reading])),
            }
        }
        let others = (self.expressive.emoji_candidates || self.expressive.kaomoji_candidates)
            .then(|| self.paths.resource(assets::OTHER_DICTIONARY));
        let mut emoji = Vec::new();
        let mut kaomoji = Vec::new();
        let mut symbols: Vec<SymbolRow> = Vec::new();
        for (letters, readings) in &groups {
            let accept = |key: &str| splits_into_syllables_at(key, letters);
            // 与 26 键相同，资源打不开或查询失败时只是没有这些行，不报诊断。
            if self.expressive.emoji_candidates {
                if let Some(others) = others.as_deref() {
                    append_unique_rows(&mut emoji, query_emoji_readings(readings, others, &accept));
                }
            }
            if self.expressive.kaomoji_candidates {
                if let Some(others) = others.as_deref() {
                    append_unique_rows(
                        &mut kaomoji,
                        query_kaomoji_readings(readings, others, &accept),
                    );
                }
            }
            if self.expressive.symbol_candidates {
                for row in query_symbol_readings(readings, &accept) {
                    if symbols.len() < MIXED_SYMBOL_LIMIT
                        && !symbols.iter().any(|known| known.item.word == row.item.word)
                    {
                        symbols.push(row);
                    }
                }
            }
        }
        // 选中后吃掉全部数字，与覆盖整串数字的拼音候选一样结束组字。
        for item in emoji
            .iter_mut()
            .chain(kaomoji.iter_mut())
            .map(|row| &mut row.item)
            .chain(symbols.iter_mut().map(|row| &mut row.item))
        {
            item.pinyin = SharedString::from(&self.digits);
        }
        (emoji, kaomoji, symbols)
    }

    /// 见 `expressive_candidates`。每种读法带着 `boundaries`（数字位置）在它里面的字母位置。
    fn expressive_readings<'a>(
        &self,
        alternatives: &[Path],
        boundaries: &'a [usize],
        candidates: &[WordItem],
    ) -> Vec<(String, Cow<'a, [usize]>)> {
        let mut readings: Vec<(String, usize)> = Vec::new();
        for (path_index, path) in alternatives.iter().enumerate() {
            if !self.reading_fits(path) {
                continue;
            }
            let mut reading = String::with_capacity(self.digits.len());
            for spelling in &self.locked {
                reading.push_str(spelling);
            }
            path.iter().for_each(|piece| reading.push_str(piece));
            if !readings.iter().any(|(known, _)| *known == reading) {
                readings.push((reading, path_index));
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
        readings.sort_by_cached_key(|(reading, _)| support(reading));
        readings.truncate(EXPRESSIVE_READING_LIMIT);
        readings
            .into_iter()
            .filter_map(|(reading, path_index)| {
                self.reading_boundaries(&alternatives[path_index], boundaries)
                    .map(|letters| (reading, letters))
            })
            .collect()
    }

    /// 只验证切分路径是否拼满输入，延后双拼边界向量的物化，避免为排序后丢弃的读法分配存储。
    fn reading_fits(&self, path: &Path) -> bool {
        let remaining = remaining_digits(&self.digits, self.locked_length());
        let Some(table) = self.grid.shuangpin_table() else {
            return path.iter().map(String::len).sum::<usize>() == remaining.len();
        };
        let mut offset = 0;
        for piece in path {
            let Some(length) = table.piece_code_len(piece, &remaining[offset..]) else {
                return false;
            };
            offset += length;
        }
        offset == remaining.len()
    }

    /// 切分路径正好拼满未锁定的数字时，`boundaries`（数字位置，含锁定拼写的结尾）在「锁定拼写加这条路径」的字母里落在哪些位置；没拼满（只拼了开头一个音节、供部分选择用的路径）时为 `None`。按字母归组的键盘一个数字一个字母，位置不变。
    fn reading_boundaries<'a>(
        &self,
        path: &Path,
        boundaries: &'a [usize],
    ) -> Option<Cow<'a, [usize]>> {
        let remaining = remaining_digits(&self.digits, self.locked_length());
        let Some(table) = self.grid.shuangpin_table() else {
            return (path.iter().map(String::len).sum::<usize>() == remaining.len())
                .then_some(Cow::Borrowed(boundaries));
        };
        // 边界按数字位置升序；走到每个音节末尾时直接记录对应的字母位置。
        let mut letters_at_boundaries = Vec::with_capacity(boundaries.len());
        let mut next_boundary = 0;
        let (mut digit, mut letter) = (0, 0);
        for spelling in &self.locked {
            digit += self.grid.syllable_code_len(spelling);
            letter += spelling.len();
            while next_boundary < boundaries.len() && boundaries[next_boundary] <= digit {
                if boundaries[next_boundary] == digit {
                    letters_at_boundaries.push(letter);
                }
                next_boundary += 1;
            }
        }
        let mut offset = 0;
        for piece in path {
            let length = table.piece_code_len(piece, &remaining[offset..])?;
            offset += length;
            digit += length;
            letter += piece.len();
            while next_boundary < boundaries.len() && boundaries[next_boundary] <= digit {
                if boundaries[next_boundary] == digit {
                    letters_at_boundaries.push(letter);
                }
                next_boundary += 1;
            }
        }
        (offset == remaining.len()).then_some(Cow::Owned(letters_at_boundaries))
    }

    /// 左列末尾的按键选项：下一个数字键上能起头一个音节的字母（大写，和同形的音节 `o`、`a`、`e` 区分开），再是这个数字本身。数字只在没有锁定时给：前面锁定的音节还没上屏。
    fn append_key_choices(&mut self, locked_length: usize) {
        let remaining = remaining_digits(&self.digits, locked_length);
        let Some(&digit) = remaining.as_bytes().first() else {
            return;
        };
        let table = self.grid.spelling_table();
        let letters = self.grid.letters(digit);
        let raw_key = self.locked.is_empty() && self.grid.offers_raw_key();
        self.spellings.reserve(letters.len() + usize::from(raw_key));
        for letter in letters
            .bytes()
            .filter(|&letter| table.starts_syllable(letter))
        {
            self.spellings
                .push(char::from(letter.to_ascii_uppercase()).to_string());
        }
        if raw_key {
            self.spellings.push(char::from(digit).to_string());
        }
    }

    #[cfg(test)]
    fn key_choices(&self, remaining: &str) -> Vec<String> {
        let Some(&digit) = remaining.as_bytes().first() else {
            return Vec::new();
        };
        let table = self.grid.spelling_table();
        let mut choices: Vec<String> = self
            .grid
            .letters(digit)
            .bytes()
            .filter(|&letter| table.starts_syllable(letter))
            .map(|letter| char::from(letter.to_ascii_uppercase()).to_string())
            .collect();
        if self.locked.is_empty() && self.grid.offers_raw_key() {
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

    /// 读音行：首行的拼音截到它覆盖的那几个键，后面接上没覆盖的键码和切分，例如 `94'26` 上的 西安 是 `xi'an`，`942` 上的 遗产 是 `yi'c`。首行不是从拼音读出来的（英文词、emoji、日期时间行）或者没有首行时为空；14 键的首行是英文词时是这个词覆盖已打键数的那段小写字母，一键一个字母，没有歧义。
    #[cfg(test)]
    fn reading_for(&self, front: Option<&WordItem>) -> String {
        let mut reading = String::new();
        self.write_reading(front, &mut reading);
        reading
    }

    fn write_reading(&self, front: Option<&WordItem>, reading: &mut String) {
        reading.clear();
        let Some(front) = front else {
            return;
        };
        // A fuzzy row was matched through another spelling (知 under `94` as zi), so its canonical letters do not line up one per typed digit.
        if front.fuzzy {
            return;
        }
        if front.source == CandidateSource::EnglishDictionary
            && matches!(self.grid, KeyGrid::FourteenKey | KeyGrid::EighteenKey)
        {
            // 英文行的 `pinyin` 是小写的查询键（完整组码查询保证全是字母）；九键不给英文读音，与按数字判断时相同。
            let typed = front.pinyin.len().min(self.digits.len());
            for byte in front.pinyin[..typed].bytes() {
                reading.push(char::from(byte.to_ascii_lowercase()));
            }
            return;
        }
        // 只有从拼音读出来的行（词库行、整句）才有可显示的读音：按来源正向判定，不看 `pinyin` 的字节形状，因为 14 键的组码和英文词一样都是小写字母。
        if front.canonical_pinyin.is_empty() || !reads_pinyin(front.source) {
            return;
        }
        // 简拼行一个数字一个音节，读音行显示每个音节的首字母（`m't`），而不是全拼的前几个字母（`me`）。
        if is_initials_row(front) {
            let mut first = true;
            for syllable in front.canonical_pinyin.split('\'') {
                let Some(letter) = syllable.get(..1) else {
                    continue;
                };
                if !first {
                    reading.push('\'');
                }
                reading.push_str(letter);
                first = false;
            }
            return;
        }
        let covered = front.pinyin.len();
        if let Some(table) = self.grid.shuangpin_table() {
            // 一个音节两位；只覆盖了一位的那个音节显示它的声母（零声母音节显示首字母）：`8146` 上没打完的 中国 是 `zhong'g`。
            let mut digits = covered;
            for syllable in front.canonical_pinyin.split('\'') {
                if digits == 0 {
                    break;
                }
                if !reading.is_empty() {
                    reading.push('\'');
                }
                if digits >= 2 {
                    reading.push_str(syllable);
                    digits -= 2;
                } else {
                    reading.push_str(table.initial_part(syllable));
                    digits = 0;
                }
            }
        } else {
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
        }
        let trimmed_len = reading.trim_end_matches('\'').len();
        reading.truncate(trimmed_len);
        if reading.is_empty() {
            return;
        }
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
    }

    /// `unanswered`：拼音一行候选都没有，英文词就是整个答案，和英文模式一样不受混输开关和最短前缀限制；锁定了拼音时仍不给。
    fn english_candidates(&mut self, unanswered: bool) -> Vec<WordItem> {
        // 九键双拼的数字不是字母的分组，拼不出英文词。
        if !self.grid.spells_english() {
            return Vec::new();
        }
        // In English-only mode the words are the whole answer, so neither the mixed-candidate setting nor the prefix length that keeps a mixed list readable applies: one digit already narrows the alphabet enough (NK:169-178).
        if self.english_only {
            if self.digits.is_empty() {
                return Vec::new();
            }
        } else if unanswered {
            if self.digits.is_empty() || !self.locked.is_empty() {
                return Vec::new();
            }
        } else if !self.mixed_candidates_enabled
            || !self.english_options.mixed_candidates
            || !self.locked.is_empty()
            || self.digits.len() < self.english_options.minimum_prefix
        {
            return Vec::new();
        }
        let grid = self.grid;
        if self.open_english().is_none() {
            return Vec::new();
        }
        let Some(english) = self.english.as_ref() else {
            return Vec::new();
        };
        let digits = self.digits.as_str();
        self.english_pattern.clear();
        for digit in digits.bytes() {
            let letters = grid.letters(digit);
            if letters.is_empty() {
                return Vec::new();
            }
            self.english_pattern.push('[');
            self.english_pattern.push_str(letters);
            for letter in letters.bytes() {
                self.english_pattern
                    .push(char::from(letter.to_ascii_uppercase()));
            }
            self.english_pattern.push(']');
        }
        self.english_pattern.push('*');
        let depth = letter_prefix_depth(grid, digits, ENGLISH_PREFIX_BUDGET);
        let reusable = self.english_rows.as_mut().filter(|cache| {
            cache.grid == grid && cache.depth == depth && digits.starts_with(&cache.digits)
        });
        match reusable {
            Some(cache) => {
                if let Some(rows) = &mut cache.rows {
                    let from = cache.digits.len();
                    for prefix_rows in rows.iter_mut() {
                        prefix_rows.retain(|row| word_spells_from(grid, &row.word, digits, from));
                    }
                } else {
                    // 前缀组第二次出现时才取全部行：九键前三位每打一位前缀组都变，取了也用不上。
                    cache.rows = letter_prefixes(grid, digits, ENGLISH_PREFIX_BUDGET)
                        .iter()
                        .map(|prefix| {
                            english.keypad_prefix_rows(
                                prefix,
                                &self.english_pattern,
                                ENGLISH_CACHED_PREFIX_ROWS,
                            )
                        })
                        .collect();
                }
                cache.digits.clear();
                cache.digits.push_str(digits);
            }
            None => {
                let cache = self.english_rows.get_or_insert_with(|| EnglishKeypadCache {
                    grid,
                    depth,
                    digits: String::new(),
                    rows: None,
                });
                cache.grid = grid;
                cache.depth = depth;
                cache.rows = None;
                cache.digits.clear();
                cache.digits.push_str(digits);
            }
        }
        let mut words = Vec::new();
        if let Some(rows) = self
            .english_rows
            .as_ref()
            .and_then(|cache| cache.rows.as_ref())
        {
            let capacity = rows.len().saturating_mul(ENGLISH_LIMIT);
            for prefix_rows in rows {
                append_keypad_ranked(
                    prefix_rows,
                    digits.len(),
                    ENGLISH_LIMIT,
                    &mut words,
                    capacity,
                );
            }
        } else {
            let prefixes = letter_prefixes(grid, digits, ENGLISH_PREFIX_BUDGET);
            let capacity = prefixes.len().saturating_mul(ENGLISH_LIMIT);
            for prefix in prefixes {
                english.append_keypad_prefix(
                    &prefix,
                    &self.english_pattern,
                    digits.len(),
                    ENGLISH_LIMIT,
                    &mut words,
                    capacity,
                );
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
        self.locked
            .iter()
            .map(|spelling| self.grid.syllable_code_len(spelling))
            .sum()
    }

    /// Drop the digits a selection consumed and every locked syllable they fully cover (NK:355-361). An English row's `pinyin` is the word, which can be longer than what was typed.
    fn consume(&mut self, count: usize) {
        let count = count.min(self.digits.len());
        self.digits.drain(..count);
        // 选词学习可能刚改了英文权重。
        self.english_rows = None;
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
            let length = self.grid.syllable_code_len(front);
            if consumed < length {
                break;
            }
            consumed -= length;
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

    /// 把最近一次九键选择固定到指定上下文的首位；后一次选择会替换第 1 槽。
    fn remember_recent_candidate(
        &self,
        context: &str,
        canonical_pinyin: &str,
        word: &str,
    ) -> Option<String> {
        positions::set_fixed_position(
            &self.paths.user(assets::USER_JOURNAL),
            context,
            canonical_pinyin,
            word,
            1,
        )
        .err()
        .map(|_| diagnostics::NINE_KEY_POSITION_NOT_PERSISTED.to_string())
    }
}

/// 键盘模型的一次重排：上屏上下文、交给模型的整句和词网格分（`comparable_weight`），以及模型给出的顺序（没有重排时为空）。
struct RerankEntry {
    context: String,
    sentences: Vec<(String, i64)>,
    ranked: Vec<String>,
}

/// 缓存命中只比较当前候选行，不为签名复制词面；签名仍在未命中时拥有并存入缓存。
fn rerank_cache_matches(entry: &RerankEntry, context: &str, scored: &[Option<&WordItem>]) -> bool {
    entry.context == context
        && entry.sentences.len() == scored.len()
        && entry
            .sentences
            .iter()
            .zip(scored)
            .all(|((word, weight), item)| {
                item.is_some_and(|item| word == &item.word && *weight == comparable_weight(item))
            })
}

/// 打开词库时要交给它的选项。取用词库的地方有三处，选项只在这一处攒齐（`dictionary_options`），
/// 所以以后加选项不会再漏掉某一条路径，让它悄悄用上默认值。
#[derive(Clone)]
struct DictionaryOptions {
    /// `NineKeySession::dictionary_association` 给的句子联想设置。
    association: SentenceAssociationOptions,
    alternatives: bool,
    first_page_rows: FirstPageRows,
}

/// 打开九键用的词库，带上 `NineKeySession::dictionary_association` 给的句子联想设置。
fn open_dictionary(paths: &RuntimePaths, options: DictionaryOptions) -> QuanpinDictionary {
    let mut dictionary = QuanpinDictionary::new(paths);
    dictionary.set_sentence_association(options.association);
    dictionary.set_sentence_alternatives(options.alternatives);
    dictionary.set_first_page_rows(options.first_page_rows);
    dictionary
}

/// 词网格解出的整句行（`lattice::merge::sentence_row` 的 `Generated` 行）。
fn is_lattice_sentence(item: &WordItem) -> bool {
    matches!(
        item.source,
        CandidateSource::Generated | CandidateSource::Collocation
    ) && item.sentence_association
}

/// 取候选列表中最多 `MAX_RERANK_PATHS` 条词网格整句，借用行本身而不建立临时索引向量。
fn collect_lattice_sentences(
    candidates: &[WordItem],
) -> ([Option<&WordItem>; MAX_RERANK_PATHS], usize) {
    let mut slots = [None; MAX_RERANK_PATHS];
    let mut len = 0;
    for item in candidates {
        if !is_lattice_sentence(item) {
            continue;
        }
        slots[len] = Some(item);
        len += 1;
        if len == MAX_RERANK_PATHS {
            break;
        }
    }
    (slots, len)
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
fn fitting_syllables(grid: KeyGrid, prefix: &str, digits: &str) -> (usize, usize, bool) {
    let mut syllables = 0;
    let mut covered = 0;
    if prefix.is_empty() {
        return (0, 0, true);
    }
    for syllable in prefix.split('\'') {
        let end = covered + grid.syllable_code_len(syllable);
        if !digits
            .get(covered..end)
            .is_some_and(|code| letters_spell_code(grid, syllable, code))
        {
            return (syllables, covered, false);
        }
        syllables += 1;
        covered = end;
    }
    (syllables, covered, true)
}

/// 小写字母串按键盘上印的字母正好拼出同样长的 `code`。九键双拼按完整音节的两位码比。
fn letters_spell_code(grid: KeyGrid, letters: &str, code: &str) -> bool {
    let Some(keypad) = grid.keypad() else {
        return match grid {
            KeyGrid::NineKeyShuangpin(table)
            | KeyGrid::FourteenKeyShuangpin(table)
            | KeyGrid::EighteenKeyShuangpin(table) => table
                .syllable_codes(letters)
                .any(|pair| pair.as_slice() == code.as_bytes()),
            _ => false,
        };
    };
    letters.len() == code.len()
        && letters.bytes().zip(code.bytes()).all(|(letter, digit)| {
            letter.is_ascii_lowercase() && keypad[usize::from(letter - b'a')] == digit
        })
}

/// 把这次刷新之后的种子写进 `seeds`（先清空），`previous` 用完清空：先取排好的候选里覆盖全部数字、不是模糊音的前 `SENTENCE_SEED_LIMIT` 条全拼（`seed_prefix` 相同的只留一条，免得几条种子只差在最后一个音节上），再接上 `previous` 里 `seed_prefix` 仍拼得出数字开头的，合计最多 `SEED_RETAIN_LIMIT` 条。末尾只按了下一个音节的头一两个数字时没有覆盖全部数字的行，这时全靠留下来的种子，否则每隔一键就丢一次。`previous` 里已有的同一条直接挪过来，不再复制字符串。
fn next_seeds(
    grid: KeyGrid,
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
            None => item.canonical_pinyin.to_string(),
        };
        seeds.push(seed);
    }
    for seed in previous.drain(..) {
        if seeds.len() < SEED_RETAIN_LIMIT
            && fitting_syllables(grid, seed_prefix(&seed), digits).2
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
        let (kept, covered, _) = fitting_syllables(table.grid, seed_prefix(seed), remaining);
        if kept == 0 {
            continue;
        }
        // 实际会话最多 32 位；常见前缀只切分一次，超长纯函数调用仍走完整迭代。
        let mut pieces = [""; DIGIT_LIMIT];
        if kept <= pieces.len() {
            for (slot, piece) in pieces.iter_mut().zip(seed.split('\'').take(kept)) {
                *slot = piece;
            }
        }
        let mut append = |tail_path| {
            if kept <= pieces.len() {
                append_seeded_path(
                    alternatives,
                    pieces[..kept].iter().copied(),
                    kept,
                    tail_path,
                );
            } else {
                append_seeded_path(alternatives, seed.split('\'').take(kept), kept, tail_path);
            }
        };
        let tail = &remaining[covered..];
        if tail.is_empty() {
            append(Vec::new());
        } else {
            for tail_path in table
                .split_paths_limited(tail, &[], prior, SEED_TAIL_LIMIT)
                .into_iter()
                .take(SEED_TAIL_LIMIT)
            {
                append(tail_path);
            }
        }
    }
}

/// 先用借用音节检查重复，只为新增组合建立路径和复制种子前缀。
fn append_seeded_path<'a>(
    alternatives: &mut Vec<Path>,
    prefix: impl Iterator<Item = &'a str> + Clone,
    kept: usize,
    tail_path: Path,
) {
    if alternatives.iter().any(|path| {
        path.len() == kept + tail_path.len()
            && path[..kept].iter().map(String::as_str).eq(prefix.clone())
            && path[kept..] == tail_path
    }) {
        return;
    }
    let mut path = Vec::with_capacity(kept + tail_path.len());
    path.extend(prefix.map(str::to_owned));
    path.extend(tail_path);
    alternatives.push(path);
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

/// 把另一组读法查到的 emoji、颜文字接在后面，按文字去重。
fn append_unique_rows(rows: &mut Vec<ExpressiveRow>, more: Vec<ExpressiveRow>) {
    for row in more {
        if !rows.iter().any(|known| known.item.word == row.item.word) {
            rows.push(row);
        }
    }
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
        // 单片编码不接受撇号；完整性接口接受撇号分段，因此在这里保留旧限制。
        if !piece.is_empty() && (piece.contains('\'') || !is_complete_pinyin_input(piece)) {
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

const LEADING_FNV_OFFSET: u64 = 0xcbf2_9ce4_8422_2325;
const LEADING_FNV_PRIME: u64 = 0x0000_0100_0000_01b3;

#[derive(Default)]
struct LeadingHasher(u64);

impl Hasher for LeadingHasher {
    fn finish(&self) -> u64 {
        self.0
    }

    fn write(&mut self, bytes: &[u8]) {
        self.0 = bytes.iter().fold(LEADING_FNV_OFFSET, |state, &byte| {
            (state ^ u64::from(byte)).wrapping_mul(LEADING_FNV_PRIME)
        });
    }

    fn write_u64(&mut self, value: u64) {
        self.0 = value;
    }
}

#[derive(Clone, Copy)]
struct LeadingEntry {
    candidate: usize,
    key: RankKey,
}

enum LeadingSlot {
    One(LeadingEntry),
    Many(Vec<LeadingEntry>),
}

impl LeadingSlot {
    fn find(&self, candidates: &[WordItem], word: &str) -> Option<(usize, RankKey)> {
        let entries: &[LeadingEntry] = match self {
            Self::One(entry) => std::slice::from_ref(entry),
            Self::Many(entries) => entries,
        };
        entries.iter().enumerate().find_map(|(slot, entry)| {
            (candidates[entry.candidate].word == word).then_some((slot, entry.key))
        })
    }

    fn update(&mut self, slot: usize, candidate: usize, key: RankKey) {
        let entries: &mut [LeadingEntry] = match self {
            Self::One(entry) => std::slice::from_mut(entry),
            Self::Many(entries) => entries,
        };
        entries[slot] = LeadingEntry { candidate, key };
    }

    fn insert(&mut self, entry: LeadingEntry) {
        match self {
            Self::One(previous) => {
                *self = Self::Many(vec![*previous, entry]);
            }
            Self::Many(entries) => entries.push(entry),
        }
    }
}

type LeadingMap = HashMap<u64, LeadingSlot, BuildHasherDefault<LeadingHasher>>;

fn leading_hash(word: &str) -> u64 {
    word.bytes().fold(LEADING_FNV_OFFSET, |state, byte| {
        (state ^ u64::from(byte)).wrapping_mul(LEADING_FNV_PRIME)
    })
}

fn leading_rank(leading: &LeadingMap, candidates: &[WordItem], word: &str) -> Option<RankKey> {
    leading
        .get(&leading_hash(word))
        .and_then(|slot| slot.find(candidates, word).map(|(_, key)| key))
}

/// More digits covered first. Synthesised rows (whole-sentence Generated, Fallback) score on a different scale from dictionary weights, so within one coverage bucket dictionary rows lead; then exact before fuzzy, then weight.
/// With `prefer_exact` (the user typed a split), a row the typed digits spell to its end then leads one that has to be completed past them: over `94'26` 西安 (xi'an) comes before 自从 (zi'cong), however common the longer word. Without a split the digits do not say where a syllable ends, so `3` keeps 的 (de) ahead of the rarer 额 (e) by weight.
/// 简拼行（`is_initials_row`）在同样覆盖的词典行里的位置由 `initials_lead` 决定：用户在每个数字之间都打了切分时排在前面，否则先排在音节行之后、整句行之前，去重后再由 `interleave_initials` 按权重插进音节行里。
/// `grid` 只用来算全拼占几个组码：九键双拼一个音节两位，不是一个字母一位。
fn rank_key(item: &WordItem, prefer_exact: bool, initials_lead: bool, grid: KeyGrid) -> RankKey {
    rank_key_for_reading(
        item,
        item.pinyin.len(),
        &item.canonical_pinyin,
        prefer_exact,
        initials_lead,
        grid,
    )
}

/// 复制前用将来的数字覆盖长度和规范读法求同一个排序键。
fn rank_key_for_reading(
    item: &WordItem,
    covered: usize,
    canonical: &str,
    prefer_exact: bool,
    initials_lead: bool,
    grid: KeyGrid,
) -> RankKey {
    #[cfg(all(test, debug_assertions))]
    cached_rank_tests::KEY_EVALUATIONS.with(|count| count.set(count.get() + 1));
    let completion = prefer_exact && grid.reading_code_len(canonical) > covered;
    let initials = is_initials_reading(item, covered, canonical);
    (
        Reverse(covered),
        item.source.is_generated_or_fallback(),
        initials != initials_lead,
        completion,
        item.fuzzy,
        Reverse(comparable_weight_for_reading(item, canonical)),
    )
}

/// 整句行的分是 `log_prob * 1000`，词组边每覆盖一个音节就带一份 `PHRASE_LENGTH_BONUS`。同一串音节里比较时这无关紧要，九宫格却要比较不同音节串各自解出的整句：音节越多、越能拼成词组的读法分越高，`9436364782662` 的 xi'e'meng'suan'ma（洗噩梦算吗）就会压过 zhe'neng'suan'ma（这能算吗）。这里按路径的音节数每个扣一份奖励：同一条路径内的先后不变，多切出来的音节要靠词本身的分量才能赢。只扣词组实际拿到的那部分反而更差（整句集 top-1 降了一截），见 `.agents/notes/implemented/bug-fix/2026-10-08-nine-key-path-ranking.md`。
fn comparable_weight(item: &WordItem) -> i64 {
    comparable_weight_for_reading(item, &item.canonical_pinyin)
}

/// 保留 `split` 的空段语义；ASCII 撇号不会出现在 UTF-8 多字节编码内部。
fn reading_syllable_count(canonical: &str) -> usize {
    canonical.bytes().filter(|&byte| byte == b'\'').count() + 1
}

fn comparable_weight_for_reading(item: &WordItem, canonical: &str) -> i64 {
    if item.source != CandidateSource::Generated || canonical.is_empty() {
        return item.weight;
    }
    let syllables = reading_syllable_count(canonical);
    // 每词插入代价已在共享词网格解码中扣除；这里只消去跨切分比较的音节奖励。
    item.weight - (PHRASE_LENGTH_BONUS * 1000.0 * syllables as f64) as i64
}

/// 九键记进个人上下文模型的词：两个字以上的纯汉字词，单字和夹着英文、符号的不记（#6185）。
fn is_personal_word(word: &str) -> bool {
    count_utf8_chars(word) >= 2 && is_all_han(word)
}

/// 这一来源的行是查拼音词库得来的（词库行、词网格和键盘模型的整句、回退解码），`canonical_pinyin` 是它的全拼。
fn reads_pinyin(source: CandidateSource) -> bool {
    source.is_dictionary()
        || source.is_generated_or_fallback()
        || source == CandidateSource::NeuralKeyboard
}

/// 一个数字一个音节、全拼比数字长的词典行：按简拼查出来的行（`68` 的 明天 mei'tian）。只有两个及以上数字时才算，一个数字本来就按首字母补全。
fn is_initials_row(item: &WordItem) -> bool {
    is_initials_reading(item, item.pinyin.len(), &item.canonical_pinyin)
}

fn is_initials_reading(item: &WordItem, digits: usize, canonical: &str) -> bool {
    digits >= 2
        && !item.source.is_generated_or_fallback()
        && reading_syllable_count(canonical) == digits
        && canonical.bytes().filter(u8::is_ascii_lowercase).count() > digits
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

/// 九键双拼按声母那几位找词时查的简拼（见 `refresh`）：每两位数字一个音节，取第 1、3、5… 位当声母展开。不到 `SHUANGPIN_WORD_MIN_SYLLABLES` 个音节，或展开超过 `INITIALS_CODE_LIMIT` 个码时为 `None`。
fn shuangpin_word_codes(grid: KeyGrid, digits: &str) -> Option<Vec<String>> {
    if digits.len().div_ceil(2) < SHUANGPIN_WORD_MIN_SYLLABLES {
        return None;
    }
    let initials: String = digits.bytes().step_by(2).map(char::from).collect();
    initials_codes(grid, &initials, INITIALS_CODE_LIMIT)
}

/// 词的全拼在九键双拼上的编码以 `digits` 开头，而且音节数正好是 `digits` 拼得出的那么多（末尾单独一位是下一个音节的声母）。
fn shuangpin_word_spells(table: &NineKeyShuangpin, canonical_pinyin: &str, digits: &str) -> bool {
    reading_syllable_count(canonical_pinyin) == digits.len().div_ceil(2)
        && table.matching_prefix_len(canonical_pinyin, digits) == Some(digits.len())
}

/// 数字键上能作音节首字母的字母：i、u、v 不起头任何音节，词库也没有以它们命名的表，所以 4 只有 g、h，8 只有 t。
fn initials_for_digit(grid: KeyGrid, digit: u8) -> impl Iterator<Item = u8> {
    grid.letters(digit)
        .bytes()
        .filter(|letter| !matches!(letter, b'i' | b'u' | b'v'))
}

/// 每个数字当一个音节的首字母时能拼出的全部简拼（`68` → mt、nt、ot）；多于 `limit` 个，或有数字不是 2-9 时为 `None`。
fn initials_codes(grid: KeyGrid, digits: &str, limit: usize) -> Option<Vec<String>> {
    // 先检查完整展开的预算，拒绝时不创建任何中间前缀。
    let mut total = 1usize;
    for digit in digits.bytes() {
        let count = initials_for_digit(grid, digit).count();
        total = total.saturating_mul(count);
        if count == 0 || total > limit {
            return None;
        }
    }
    let mut codes = vec![String::with_capacity(digits.len())];
    for digit in digits.bytes() {
        codes = codes
            .iter()
            .flat_map(|code| {
                initials_for_digit(grid, digit).map(move |letter| {
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

fn take_candidate_canonical_pinyin(candidate: &mut WordItem) -> SharedString {
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
    leading: &mut LeadingMap,
    candidate: WordItem,
    prefer_exact: bool,
    initials_lead: bool,
    grid: KeyGrid,
) {
    let key = rank_key(&candidate, prefer_exact, initials_lead, grid);
    let hash = leading_hash(&candidate.word);
    let existing = leading
        .get(&hash)
        .and_then(|slot| slot.find(candidates, &candidate.word));
    if let Some((slot, best)) = existing {
        if best <= key {
            return;
        }
        let candidate_index = candidates.len();
        candidates.push(candidate);
        leading
            .get_mut(&hash)
            .expect("leading hash entry remains present")
            .update(slot, candidate_index, key);
        return;
    }
    if leading.is_empty() {
        leading.reserve(CANDIDATE_LIMIT);
    }
    let candidate_index = candidates.len();
    candidates.push(candidate);
    let entry = LeadingEntry {
        candidate: candidate_index,
        key,
    };
    if let Some(slot) = leading.get_mut(&hash) {
        slot.insert(entry);
    } else {
        leading.insert(hash, LeadingSlot::One(entry));
    }
}

/// 按 `rank_key` 稳定排序、按词去重，简拼交错和个人调频后再截断（NK:283-307）。
fn rank_candidates(
    candidates: &mut Vec<WordItem>,
    prefer_exact: bool,
    initials_lead: bool,
    boost: Option<&InitialsBoost>,
    grid: KeyGrid,
) {
    if candidates.len() <= SMALL_CANDIDATE_SORT {
        candidates.sort_by_key(|item| rank_key(item, prefer_exact, initials_lead, grid));
    } else {
        candidates.sort_by_cached_key(|item| rank_key(item, prefer_exact, initials_lead, grid));
    }
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
    used: HashMap<SharedString, u32>,
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

/// The digit code of an English word, ignoring case; a word with any non-letter has none.
#[cfg(test)]
fn digits_for_word(word: &str) -> String {
    let lowered = word.to_ascii_lowercase();
    if !lowered.bytes().all(|byte| byte.is_ascii_lowercase()) {
        return String::new();
    }
    KeyGrid::NineKey.encode(&lowered)
}

#[cfg(test)]
fn word_matches_digits(grid: KeyGrid, word: &str, digits: &str) -> bool {
    let Some(keypad) = grid.keypad() else {
        return false;
    };
    let mut matched = 0;
    for byte in word.bytes() {
        let letter = byte.to_ascii_lowercase();
        if !letter.is_ascii_lowercase() {
            return false;
        }
        if let Some(&digit) = digits.as_bytes().get(matched) {
            if keypad[usize::from(letter - b'a')] != digit {
                return false;
            }
            matched += 1;
        }
    }
    matched == digits.len()
}

/// `keyword`（小写字母）按键盘上的字母正好拼出 `digits`，不分配。
fn keyword_spells_digits(grid: KeyGrid, keyword: &str, digits: &str) -> bool {
    let Some(keypad) = grid.keypad() else {
        return match grid {
            KeyGrid::NineKeyShuangpin(table)
            | KeyGrid::FourteenKeyShuangpin(table)
            | KeyGrid::EighteenKeyShuangpin(table) => table.spells_keyword(keyword, digits),
            _ => false,
        };
    };
    keyword.len() == digits.len()
        && keyword.bytes().zip(digits.bytes()).all(|(letter, digit)| {
            letter.is_ascii_lowercase() && keypad[usize::from(letter - b'a')] == digit
        })
}

/// `letter_prefixes` 展开到第几位数字、共多少个前缀。
fn letter_prefix_span(grid: KeyGrid, digits: &str, budget: usize) -> (usize, usize) {
    let mut count = 1;
    let mut depth = 0;
    for digit in digits.bytes() {
        let letters = grid.letters(digit);
        let width = letters.chars().count();
        if width == 0 || count > budget / width {
            break;
        }
        count *= width;
        depth += 1;
    }
    (depth, count)
}

fn letter_prefix_depth(grid: KeyGrid, digits: &str, budget: usize) -> usize {
    letter_prefix_span(grid, digits, budget).0
}

/// `word` 从第 `from` 位起逐位落在 `digits` 对应键的字母里，即 `english_pattern` 在已知前 `from` 位匹配时的剩余条件；大小写都算。
fn word_spells_from(grid: KeyGrid, word: &str, digits: &str, from: usize) -> bool {
    word.len() >= digits.len()
        && word.as_bytes()[from..digits.len()]
            .iter()
            .zip(&digits.as_bytes()[from..])
            .all(|(&letter, &digit)| {
                grid.letters(digit)
                    .as_bytes()
                    .contains(&letter.to_ascii_lowercase())
            })
}

/// Every letter prefix of the leading digits, expanded while the count stays within `budget`.
fn letter_prefixes(grid: KeyGrid, digits: &str, budget: usize) -> Vec<String> {
    let (depth, count) = letter_prefix_span(grid, digits, budget);
    if depth == 0 {
        Vec::new()
    } else {
        fn visit(grid: KeyGrid, digits: &[u8], prefix: &mut String, result: &mut Vec<String>) {
            let Some((&digit, rest)) = digits.split_first() else {
                result.push(prefix.clone());
                return;
            };
            for letter in grid.letters(digit).chars() {
                prefix.push(letter);
                visit(grid, rest, prefix, result);
                prefix.pop();
            }
        }

        let mut prefixes = Vec::with_capacity(count);
        let mut prefix = String::with_capacity(depth);
        visit(
            grid,
            &digits.as_bytes()[..depth],
            &mut prefix,
            &mut prefixes,
        );
        prefixes
    }
}

/// The syllables and spelling pieces digits can stand for, built once from the intact syllable list.
pub(crate) struct SpellingTable {
    /// 码按哪种键盘算。
    grid: KeyGrid,
    /// Every complete syllable with its code, in list order.
    syllables: Vec<(String, String)>,
    /// Every prefix of every syllable, grouped by its code, with whether that piece is a complete syllable under this code. Partial syllables are included, but never letter combinations pinyin does not have (NK:52-68). 按字母归组的键盘上一个片段完不完整只看它是不是完整音节；九键双拼的 `a` 既是两位的完整音节，又是只打了一位的零声母片段，所以按码记。
    by_code: HashMap<String, Vec<(String, bool)>>,
    longest_code: usize,
}

/// 九键英文的逐前缀查询在第四位数字以后占一次刷新的约两成（每位最多 64 条带排序的 SQL）。前缀组只由前几位数字决定，之后数字变长只是让匹配的词变少，所以把每个前缀下匹配的全部行留在内存里：数字只在末尾变长时按新增的位筛掉不再匹配的行，再按 `append_keypad_ranked` 取前五，不查 SQLite。行按数字内容复用，只有词库变了才会过时，而组字中改英文权重的只有选词学习，`consume` 和 `clear_composition` 因此清掉它。
struct EnglishKeypadCache {
    grid: KeyGrid,
    /// `letter_prefix_depth`：前缀组由前这么多位数字决定。
    depth: usize,
    /// `rows` 匹配的数字串。
    digits: String,
    /// 按 `letter_prefixes` 的顺序每个前缀一组；前缀组只出现过一次或某个前缀超过 `ENGLISH_CACHED_PREFIX_ROWS` 行时为 `None`。
    rows: Option<Vec<Vec<KeypadRow>>>,
}

/// 搜索页只借用拼写表音节；每次切分从后向前完整覆写旧节点。
#[derive(Default)]
struct PathScratch<'a> {
    pages: Vec<Vec<SpellingPath<'a>>>,
}

/// 中间路径只借用音节；后继索引指向已经定序、不会再改变的后页。
#[derive(Clone, Copy)]
struct SpellingPath<'a> {
    score: f64,
    complete: bool,
    piece: &'a str,
    next: (usize, usize),
    length: usize,
}

impl<'a> SpellingPath<'a> {
    fn pieces<'s>(self, suffix: &'s [Vec<Self>]) -> impl Iterator<Item = &'a str> + 's
    where
        'a: 's,
    {
        let mut first = (self.length != 0).then_some(self.piece);
        let mut next = (self.length > 1).then_some(self.next);
        std::iter::from_fn(move || {
            if let Some(piece) = first.take() {
                return Some(piece);
            }
            let (column, row) = next?;
            let node = &suffix[column][row];
            next = (node.length > 1).then_some(node.next);
            Some(node.piece)
        })
    }
}

impl SpellingTable {
    fn build(grid: KeyGrid, syllables: &[&str]) -> Self {
        let prefix_capacity = syllables.iter().map(|syllable| syllable.len()).sum();
        let intact: HashSet<&str> = syllables.iter().copied().collect();
        let mut seen = HashSet::with_capacity(prefix_capacity);
        let mut by_code: HashMap<String, Vec<(String, bool)>> =
            HashMap::with_capacity(prefix_capacity);
        let mut longest_code = 0;
        for syllable in syllables {
            for end in 1..=syllable.len() {
                let Some(prefix) = syllable.get(..end) else {
                    continue;
                };
                if seen.insert(prefix.to_string()) {
                    let code = grid.encode(prefix);
                    longest_code = longest_code.max(code.len());
                    by_code
                        .entry(code)
                        .or_default()
                        .push((prefix.to_string(), intact.contains(prefix)));
                }
            }
        }
        Self {
            grid,
            syllables: syllables
                .iter()
                .map(|syllable| (syllable.to_string(), grid.encode(syllable)))
                .collect(),
            by_code,
            longest_code,
        }
    }

    /// 九键双拼的拼写表：完整音节是方案给的两位码，只打了一位的片段是声母（`zh`、`b`）和零声母音节的首字母（`a`、`e`、`o`），见 `NineKeyShuangpin::one_key_pieces`。
    fn build_shuangpin(grid: KeyGrid, table: &NineKeyShuangpin) -> Self {
        let mut syllables = Vec::new();
        let mut by_code: HashMap<String, Vec<(String, bool)>> = HashMap::new();
        for (syllable, code) in table.coded_syllables() {
            let code: String = code.iter().map(|&digit| char::from(digit)).collect();
            by_code
                .entry(code.clone())
                .or_default()
                .push((syllable.to_owned(), true));
            syllables.push((syllable.to_owned(), code));
        }
        for (piece, key) in table.one_key_pieces() {
            by_code
                .entry(char::from(key).to_string())
                .or_default()
                .push((piece.to_owned(), false));
        }
        Self {
            grid,
            syllables,
            by_code,
            longest_code: 2,
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
    #[cfg(test)]
    fn spellings_for(
        &self,
        remaining: &str,
        locked_length: usize,
        split: Option<usize>,
        prior: &SyllablePrior,
    ) -> Vec<String> {
        let mut spellings = Vec::new();
        let mut matches = Vec::new();
        self.spellings_for_into(
            remaining,
            locked_length,
            split,
            prior,
            &mut spellings,
            &mut matches,
        );
        spellings
    }

    fn spellings_for_into(
        &self,
        remaining: &str,
        locked_length: usize,
        split: Option<usize>,
        prior: &SyllablePrior,
        destination: &mut Vec<String>,
        matches: &mut Vec<usize>,
    ) {
        matches.clear();
        if remaining.is_empty() {
            destination.clear();
            return;
        }
        matches.extend(
            self.syllables
                .iter()
                .enumerate()
                .filter_map(|(index, (_, code))| {
                    let matches = match split {
                        Some(split) => code.len() <= split && remaining.starts_with(code.as_str()),
                        None => {
                            locked_length + remaining.len().max(code.len()) <= DIGIT_LIMIT
                                && (remaining.starts_with(code.as_str())
                                    || code.starts_with(remaining))
                        }
                    };
                    matches.then_some(index)
                }),
        );
        let covered = |code: &str| code.len().min(remaining.len());
        matches.sort_by(|&a, &b| {
            let (a, a_code) = &self.syllables[a];
            let (b, b_code) = &self.syllables[b];
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
        destination.truncate(matches.len());
        for (position, &index) in matches.iter().enumerate() {
            if let Some(spelling) = destination.get_mut(position) {
                spelling.clear();
                spelling.push_str(&self.syllables[index].0);
            } else {
                destination.push(self.syllables[index].0.clone());
            }
        }
    }

    /// Syllable paths spelling `digits`, built from the end. A piece may end a path early only if it is a complete syllable; each offset keeps the 48 best by fewer syllables, complete last syllable, then lexicographic (NK:122-156). 不带频度，测试用。
    #[cfg(test)]
    fn paths(&self, digits: &str) -> Vec<Path> {
        self.split_paths(digits, &[], &SyllablePrior::default())
    }

    /// 和 `paths` 一样，但每个 `splits` 位置（`digits` 里的下标）都必须有一个音节在那里结束，而且这个音节必须完整；同样音节数、同样收尾的路径按 `prior` 给的频度之和排，频度相同才按字典序。
    ///
    /// 每个位置只留 48 条，留哪些就决定了哪些读法还能查到。原先在字典序上截断，而 9 键是 wxyz、7 键是 pqrs，于是长串里 xi 开头的路径把 yi、zhe 开头的全部挤掉，`943426943426` 只剩「洗点一点」查不到「一点一点」，`7264685487` 只剩「盘后」查不到「然后」。
    #[cfg(test)]
    fn split_paths(&self, digits: &str, splits: &[usize], prior: &SyllablePrior) -> Vec<Path> {
        self.split_paths_limited(digits, splits, prior, PATH_LIMIT)
    }

    /// 所有中间页仍保留完整搜索宽度，只按消费者上限物化最终路径。
    fn split_paths_limited(
        &self,
        digits: &str,
        splits: &[usize],
        prior: &SyllablePrior,
        output_limit: usize,
    ) -> Vec<Path> {
        let mut paths = Vec::new();
        self.split_paths_into(digits, splits, prior, output_limit, &mut paths);
        paths
    }

    /// 将最终路径写入已有缓冲，保留路径和音节字符串的容量。
    fn split_paths_into(
        &self,
        digits: &str,
        splits: &[usize],
        prior: &SyllablePrior,
        output_limit: usize,
        destination: &mut Vec<Path>,
    ) {
        self.split_paths_with_scratch(
            digits,
            splits,
            prior,
            output_limit,
            &mut PathScratch::default(),
            destination,
        );
    }

    fn split_paths_with_scratch<'a>(
        &'a self,
        digits: &str,
        splits: &[usize],
        prior: &SyllablePrior,
        output_limit: usize,
        scratch: &mut PathScratch<'a>,
        destination: &mut Vec<Path>,
    ) {
        let length = digits.len();
        if scratch.pages.is_empty() {
            scratch.pages = vec![Vec::new(); length + 1];
        } else {
            scratch.pages.resize_with(length + 1, Vec::new);
        }
        let suffix = &mut scratch.pages;
        if destination.is_empty() {
            // 冷输出的根页从空容器建立，保持消费式 collect 的原分配与容量。
            suffix[0] = Vec::new();
        }
        suffix[length].clear();
        suffix[length].push(SpellingPath {
            score: 0.0,
            complete: false,
            piece: "",
            next: (length, 0),
            length: 0,
        });
        for offset in (0..length).rev() {
            let mut result = std::mem::take(&mut suffix[offset]);
            result.clear();
            for end in offset + 1..=length.min(offset + self.longest_code) {
                if splits.iter().any(|&split| offset < split && split < end) {
                    break;
                }
                let Some(pieces) = self.by_code.get(&digits[offset..end]) else {
                    continue;
                };
                let must_complete = end != length || splits.contains(&end);
                for (piece, complete) in pieces {
                    if must_complete && !complete {
                        continue;
                    }
                    let score = prior.score(piece);
                    for (row, tail) in suffix[end].iter().enumerate() {
                        if result.is_empty() {
                            result.reserve_exact(PATH_LIMIT);
                        }
                        result.push(SpellingPath {
                            score: score + tail.score,
                            complete: if tail.length == 0 {
                                *complete
                            } else {
                                tail.complete
                            },
                            piece,
                            next: (end, row),
                            length: tail.length + 1,
                        });
                    }
                }
            }
            // 按码分组访问音节；比较项与原拥有路径相同，只在需要字典序时回溯音节。
            let compare = |a: &SpellingPath<'_>, b: &SpellingPath<'_>| {
                a.length
                    .cmp(&b.length)
                    .then_with(|| b.complete.cmp(&a.complete))
                    .then_with(|| b.score.total_cmp(&a.score))
                    .then_with(|| a.pieces(suffix).cmp(b.pieces(suffix)))
            };
            let keep = if offset == 0 {
                output_limit.min(PATH_LIMIT)
            } else {
                PATH_LIMIT
            };
            // 根页没有前驱消费者；同分同读法的路径输出相同，只选择最终采用的前几条。
            if keep < PATH_LIMIT && result.len() > keep {
                result.select_nth_unstable_by(keep, compare);
                result.truncate(keep);
            }
            result.sort_by(compare);
            result.truncate(keep);
            suffix[offset] = result;
        }
        // 冷输出仍消费根页，让 collect 复用原分配；已有输出时才保留根页容量。
        if destination.is_empty() {
            let roots = std::mem::take(&mut suffix[0]);
            *destination = roots
                .into_iter()
                .take(output_limit)
                .map(|root| {
                    let mut path = Vec::with_capacity(root.length);
                    path.extend(root.pieces(suffix).map(str::to_owned));
                    path
                })
                .collect();
            return;
        }
        let roots = &suffix[0];
        let output_len = roots.len().min(output_limit);
        destination.reserve(output_len.saturating_sub(destination.len()));
        destination.truncate(output_len);
        for (index, root) in roots.iter().copied().take(output_limit).enumerate() {
            let path = if let Some(path) = destination.get_mut(index) {
                path
            } else {
                destination.push(Vec::with_capacity(root.length));
                destination.last_mut().expect("new path")
            };
            path.truncate(root.length);
            path.reserve(root.length.saturating_sub(path.len()));
            for (piece_index, piece) in root.pieces(suffix).enumerate() {
                if piece_index == path.len() {
                    path.push(String::new());
                }
                path[piece_index].clear();
                path[piece_index].push_str(piece);
            }
            path.truncate(root.length);
        }
    }
}

/// 音节和音节前缀的单字频度：音节取词库里它权重最高的 `PRIOR_CHAR_LIMIT` 个字的权重之和取对数，前缀取以它开头的音节里最高的那个。九宫格用它决定每个位置留哪 48 条切分路径、种子接哪几条尾段和左列音节的先后，候选本身的先后仍由词库和整句解码决定。
///
/// 原先只取最常用的那一个字。dict-v2.0.30 只调了单字权重，na 最常用的「那」从 9929703 降到 5580163，ma 的「吗」从 9028487 降到 5664547，两个音节的先后就翻了过来；种子 wo'men'jin'tian'qu'na 接 6254 的三条尾段变成 ma'ji、na'ji、ma'li，na'li 排第四被丢掉，`966365468426786254` 的首选从「我们今天去哪里」变成「我们今天去马力」，「哪里」不在 128 行里。取前三个字之和时 na 有 那、哪、拿 三个常用字，比只有一个常用字「吗」的 ma 稳。
#[derive(Default)]
struct SyllablePrior<'a> {
    scores: HashMap<&'a str, f64>,
    /// 完整音节自己的频度，不取以它开头的更长音节的：左列里 pin 不该沾 ping 的光。
    syllables: HashMap<&'a str, f64>,
}

impl<'a> SyllablePrior<'a> {
    fn from_dictionary(
        dictionary: &QuanpinDictionary,
        table: &'a SpellingTable,
    ) -> SyllablePrior<'a> {
        let keys: Vec<String> = table
            .syllables
            .iter()
            .map(|(syllable, _)| syllable.clone())
            .collect();
        let weights = dictionary.top_weight_sums(&keys, PRIOR_CHAR_LIMIT);
        Self::from_weights(&table.syllables, &weights)
    }

    fn from_weights(syllables: &'a [(String, String)], weights: &HashMap<String, i64>) -> Self {
        let total: f64 = syllables
            .iter()
            .map(|(syllable, _)| weights.get(syllable).copied().unwrap_or(0).max(1) as f64)
            .sum();
        let prefix_capacity = syllables.iter().map(|(syllable, _)| syllable.len()).sum();
        let mut scores: HashMap<&str, f64> = HashMap::with_capacity(prefix_capacity);
        let mut own: HashMap<&str, f64> = HashMap::with_capacity(syllables.len());
        for (syllable, _) in syllables {
            // 没有单字行的音节和权重为 0 的一样按 1 算。
            let weight = weights.get(syllable).copied().unwrap_or(0).max(1) as f64;
            let score = (weight / total).ln();
            own.insert(syllable.as_str(), score);
            for end in 1..=syllable.len() {
                let Some(prefix) = syllable.get(..end) else {
                    continue;
                };
                let slot = scores.entry(prefix).or_insert(score);
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
mod dominated_projection_tests;
#[cfg(test)]
mod filter_projection_tests;
#[cfg(test)]
mod isolated_test;
#[cfg(test)]
mod leading_entry_refresh_tests;
#[cfg(test)]
mod leading_entry_tests;

#[cfg(test)]
mod cached_rank_tests;
#[cfg(test)]
mod english_cache_tests;
#[cfg(test)]
mod syllable_count_tests;

#[cfg(test)]
mod initials_code_tests;

#[cfg(test)]
mod path_chain_tests;

#[cfg(test)]
mod path_buffer_tests;

#[cfg(test)]
mod path_scratch_tests;

#[cfg(test)]
mod seed_tail_tests;

#[cfg(test)]
mod seed_dedup_tests;

#[cfg(test)]
mod code_prefix_tests;

#[cfg(test)]
mod spelling_buffer_tests;

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{CandidateSource, NineKeyShuangpinKind};
    use rusqlite::Connection;

    // 九键的单测按电话键盘写，这几个函数固定取九键。
    fn encode(pinyin: &str) -> String {
        KeyGrid::NineKey.encode(pinyin)
    }

    fn word_matches_digits(word: &str, digits: &str) -> bool {
        super::word_matches_digits(KeyGrid::NineKey, word, digits)
    }

    fn initials_codes(digits: &str, limit: usize) -> Option<Vec<String>> {
        super::initials_codes(KeyGrid::NineKey, digits, limit)
    }

    fn letter_prefixes(digits: &str, budget: usize) -> Vec<String> {
        super::letter_prefixes(KeyGrid::NineKey, digits, budget)
    }

    fn spelling_table() -> &'static SpellingTable {
        KeyGrid::NineKey.spelling_table()
    }

    #[test]
    fn spelling_match_indices_reuse_storage() {
        let table = spelling_table();
        let prior = SyllablePrior::default();
        let mut destination = Vec::with_capacity(128);
        let mut matches = Vec::new();
        table.spellings_for_into("64", 0, None, &prior, &mut destination, &mut matches);
        let (_, allocations) = crate::ime::personal_rerank::allocations::count(|| {
            table.spellings_for_into("64", 0, None, &prior, &mut destination, &mut matches);
        });
        assert!(
            allocations <= destination.len() + 1,
            "拼写匹配刷新不应重新申请索引工作区：{allocations} 次分配，{} 个读音",
            destination.len()
        );
    }

    #[test]
    fn syllable_prior_does_not_clone_static_spelling_keys() {
        let table = spelling_table();
        let weights = HashMap::new();
        let (_, allocations) = crate::ime::personal_rerank::allocations::count(|| {
            SyllablePrior::from_weights(&table.syllables, &weights)
        });
        assert!(
            allocations <= 4,
            "静态九键音节表的频度索引不应复制音节和前缀键：{allocations} 次分配"
        );
    }

    fn fitting_syllables(prefix: &str, digits: &str) -> (usize, usize, bool) {
        super::fitting_syllables(KeyGrid::NineKey, prefix, digits)
    }

    fn next_seeds(
        candidates: &[WordItem],
        digits: &str,
        previous: &mut Vec<String>,
        seeds: &mut Vec<String>,
    ) {
        super::next_seeds(KeyGrid::NineKey, candidates, digits, previous, seeds);
    }

    impl SpellingTable {
        fn new(syllables: &[&str]) -> Self {
            Self::build(KeyGrid::NineKey, syllables)
        }
    }

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
    fn letter_prefixes_do_not_allocate_discarded_intermediate_levels() {
        let (_, allocations) =
            crate::ime::personal_rerank::allocations::count(|| letter_prefixes("64426", 64));
        assert!(
            allocations <= 30,
            "letter prefix expansion allocated {allocations} buffers"
        );
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

    /// 每词插入代价由词网格计入，跨切分比较不能再扣一次。
    #[test]
    fn phrase_insertion_cost_is_not_deducted_again_across_paths() {
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
        // 两句都是六个音节；输入分已含三份和两份词组插入代价，九键只扣相同的六份音节奖励。
        let phrases = sentence(
            "匿名天佑龙马",
            "ni'ming'tian'you'long'ma",
            &["匿名", "天佑", "龙马"],
            40_000 - 3 * bonus,
        );
        let plain = sentence(
            "你明天有空吗",
            "ni'ming'tian'you'kong'ma",
            &["你", "明天", "有空", "吗"],
            30_000 - 2 * bonus,
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

    /// 种子接尾段时只取频度最高的 `SEED_TAIL_LIMIT` 条，两个音节各有一对读法（ma/na、ji/li）时四种组合留三种；只按最常用的一个字估频度，na 和 ma 的单字权重差几个百分点就决定 na'li 在不在。权重取自 dict-v2.0.32。
    #[test]
    fn seed_tails_keep_a_reading_whose_best_character_barely_loses() {
        let table = SpellingTable::new(&[
            "wo", "men", "jin", "tian", "qu", "na", "ma", "li", "ji", "jiu",
        ]);
        let remaining = encode("wo'men'jin'tian'qu'na'li");
        let seeds = ["wo'men'jin'tian'qu'na".to_owned()];
        let wanted = path(&["wo", "men", "jin", "tian", "qu", "na", "li"]);
        // 每个音节权重最高的三个字；末尾的 ji 按前缀取以它开头的 jiu 的频度，jiu 的权重是编的。
        let characters: [(&str, [i64; 3]); 10] = [
            ("wo", [3_000_000, 50_000, 40_000]),
            ("men", [1_788_123, 544_382, 24_690]),
            ("jin", [2_000_000, 900_000, 500_000]),
            ("tian", [2_000_000, 400_000, 100_000]),
            ("qu", [3_000_000, 800_000, 300_000]),
            ("na", [5_580_163, 1_372_701, 1_345_242]),
            ("ma", [5_664_547, 279_925, 112_776]),
            ("li", [7_995_093, 999_387, 380_201]),
            ("ji", [2_726_986, 1_102_969, 159_650]),
            ("jiu", [8_500_000, 600_000, 200_000]),
        ];
        let tails = |weights: HashMap<String, i64>| {
            let prior = SyllablePrior::from_weights(&table.syllables, &weights);
            let mut alternatives = Vec::new();
            seeded_paths(&mut alternatives, &seeds, &remaining, &table, &prior);
            alternatives
        };
        let best: HashMap<String, i64> = characters
            .iter()
            .map(|(syllable, weights)| (syllable.to_string(), weights[0]))
            .collect();
        let summed: HashMap<String, i64> = characters
            .iter()
            .map(|(syllable, weights)| {
                (
                    syllable.to_string(),
                    weights[..PRIOR_CHAR_LIMIT].iter().sum(),
                )
            })
            .collect();
        // 只看最常用的字：吗 比 那 高 1.5%，尾段取 ma'ji、na'ji、ma'li，na'li 排第四。
        assert!(!tails(best).contains(&wanted));
        assert!(tails(summed).contains(&wanted));
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
        // 已经锁定了 `DIGIT_LIMIT - 1` 位时，只有一位就拼完的音节还放得下。
        assert_eq!(
            table.spellings_for("2", DIGIT_LIMIT - 1, None, &prior),
            ["a"]
        );
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
        let mut leading = LeadingMap::default();
        assert_eq!(leading.capacity(), 0);

        push_ranked(
            &mut candidates,
            &mut leading,
            item("你", "64", 1, CandidateSource::Database),
            false,
            false,
            KeyGrid::NineKey,
        );

        assert_eq!(candidates.len(), 1);
        assert!(leading.capacity() >= CANDIDATE_LIMIT);
    }

    #[test]
    fn push_ranked_leading_map_allocation_profile() {
        let rows: Vec<_> = (0..256)
            .map(|index| {
                item(
                    &format!("合成词{index}"),
                    "64426",
                    index,
                    CandidateSource::Database,
                )
            })
            .collect();
        let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
            let mut candidates = Vec::new();
            let mut leading = LeadingMap::default();
            for row in rows.iter().cloned() {
                push_ranked(
                    &mut candidates,
                    &mut leading,
                    row,
                    false,
                    false,
                    KeyGrid::NineKey,
                );
            }
        });
        eprintln!("push-ranked-leading allocations: {allocations}");
        assert!(allocations < 700, "leading map allocations: {allocations}");
    }

    #[test]
    fn leading_slot_keeps_distinct_words_when_hashes_collide() {
        let candidates = vec![
            item("甲", "64", 1, CandidateSource::Database),
            item("乙", "64", 2, CandidateSource::Database),
        ];
        let first = LeadingEntry {
            candidate: 0,
            key: rank_key(&candidates[0], false, false, KeyGrid::NineKey),
        };
        let second = LeadingEntry {
            candidate: 1,
            key: rank_key(&candidates[1], false, false, KeyGrid::NineKey),
        };
        let mut slot = LeadingSlot::One(first);
        slot.insert(second);
        assert!(slot.find(&candidates, "甲").is_some());
        assert!(slot.find(&candidates, "乙").is_some());
        assert!(slot.find(&candidates, "丙").is_none());
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
        rank_candidates(&mut candidates, false, false, None, KeyGrid::NineKey);
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
        row.canonical_pinyin = canonical.into();
        row
    }

    fn ranked(mut candidates: Vec<WordItem>, prefer_exact: bool) -> Vec<String> {
        rank_candidates(&mut candidates, prefer_exact, false, None, KeyGrid::NineKey);
        candidates
            .into_iter()
            .map(|item| item.word.into_string())
            .collect()
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
                    ["e", "de", "xi'an", "zi'cong", "ni'hao'ma"][next(5) as usize].into();
                rows.push(row);
            }
            let prefer_exact = round % 2 == 0;
            let mut everything = rows.clone();
            rank_candidates(&mut everything, prefer_exact, false, None, KeyGrid::NineKey);
            let mut skipped = Vec::new();
            let mut leading = LeadingMap::default();
            for row in rows {
                push_ranked(
                    &mut skipped,
                    &mut leading,
                    row,
                    prefer_exact,
                    false,
                    KeyGrid::NineKey,
                );
            }
            rank_candidates(&mut skipped, prefer_exact, false, None, KeyGrid::NineKey);
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

    #[test]
    fn snapshot_reuses_locked_prefix_length_for_an_initial() {
        let mut session = detached();
        session.digits = "644269".into();
        session.locked = vec!["ni".into(), "hao".into()];
        session.initial = Some(Initial {
            letter: b'n',
            typed: 6,
            splits: 0,
        });
        let (_, allocations) =
            crate::ime::personal_rerank::allocations::count(|| session.snapshot());
        assert!(
            allocations <= 2,
            "九键快照预编辑缓冲扩容产生了 {allocations} 次分配"
        );
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
            .map(|item| item.word.into_string())
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
    fn refresh_appends_key_choices_without_a_temporary_vector() {
        let fixture = fixture();
        let mut session = open(&fixture.paths, false, EnglishInputOptions::default());
        session.digits = "64".into();
        session.refresh();
        let (_, allocations) =
            crate::ime::personal_rerank::allocations::count(|| session.refresh());
        let spellings = session.snapshot().nine_key_spellings;
        assert!(spellings.ends_with(
            &["o", "M", "N", "O", "6"]
                .into_iter()
                .map(String::from)
                .collect::<Vec<_>>()
        ));
        assert!(
            allocations <= 110,
            "key choices refresh allocations: {allocations}"
        );
    }

    #[test]
    fn refresh_reuses_reading_storage() {
        let fixture = fixture();
        let mut session = open(&fixture.paths, false, EnglishInputOptions::default());
        session.digits = "64426".into();
        session.refresh();

        let (_, allocations) =
            crate::ime::personal_rerank::allocations::count(|| session.refresh());
        assert!(allocations < 601, "九键读音热刷新分配了 {allocations} 次");
    }

    #[test]
    fn refresh_reuses_search_pages_and_releases_them_outside_pinyin_composition() {
        let fixture = fixture();
        let mut session = open(&fixture.paths, false, EnglishInputOptions::default());
        session.digits = "64426".into();
        session.refresh();
        session.refresh();
        let pages: Vec<_> = session
            .path_scratch
            .pages
            .iter()
            .map(|page| (page.as_ptr(), page.capacity()))
            .collect();
        assert!(pages.iter().any(|(_, capacity)| *capacity != 0));
        session.refresh();
        assert_eq!(
            pages,
            session
                .path_scratch
                .pages
                .iter()
                .map(|page| (page.as_ptr(), page.capacity()))
                .collect::<Vec<_>>()
        );
        session.english_only = true;
        session.refresh();
        assert!(session.path_scratch.pages.is_empty());
        session.english_only = false;
        session.refresh();
        assert!(!session.path_scratch.pages.is_empty());
        session.digits.clear();
        session.refresh();
        assert!(session.path_scratch.pages.is_empty());
    }

    #[test]
    fn commit_raw_releases_search_pages_before_returning() {
        let fixture = fixture();
        let mut session = open(&fixture.paths, false, EnglishInputOptions::default());
        session.digits = "64426".into();
        session.refresh();
        assert!(!session.path_scratch.pages.is_empty());
        let result = session.command(Command::CommitRaw);
        assert_eq!(result.commit.as_deref(), Some("64426"));
        assert!(session.path_scratch.pages.is_empty());
    }

    #[test]
    fn refresh_reuses_split_path_storage() {
        let fixture = fixture();
        let mut session = open(&fixture.paths, false, EnglishInputOptions::default());
        session.digits = "64426".into();
        session.refresh();
        let pointer = session.path_buffer[0][0].as_ptr();
        let capacity = session.path_buffer[0][0].capacity();
        session.refresh();
        assert_eq!(session.path_buffer[0][0].as_ptr(), pointer);
        assert_eq!(session.path_buffer[0][0].capacity(), capacity);
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
    fn refresh_reuses_spelling_vector_storage() {
        let fixture = fixture();
        let mut session = open(&fixture.paths, false, EnglishInputOptions::default());
        session.digits = "64426".into();
        session.refresh();
        let pointer = session.spellings.as_ptr();
        let capacity = session.spellings.capacity();
        assert!(capacity > 0);
        session.refresh();
        assert_eq!(session.spellings.as_ptr(), pointer);
        assert_eq!(session.spellings.capacity(), capacity);
    }

    #[test]
    fn refresh_keeps_spelling_string_buffers() {
        let fixture = fixture();
        let mut session = open(&fixture.paths, false, EnglishInputOptions::default());
        session.digits = "64".into();
        session.refresh();
        let expected = session.snapshot();
        // 排名会移动行，给每个音节留下足够容量后，按指针集合检查复用。
        let syllables = session.spellings.len() - session.key_choices("64").len();
        assert!(syllables > 0);
        for value in &mut session.spellings[..syllables] {
            value.reserve(64);
        }
        let mut before: Vec<_> = session.spellings[..syllables]
            .iter()
            .map(|value| value.as_ptr())
            .collect();
        before.sort_unstable();
        session.refresh();
        assert_eq!(session.snapshot(), expected);
        let mut after: Vec<_> = session.spellings[..syllables]
            .iter()
            .map(|value| value.as_ptr())
            .collect();
        after.sort_unstable();
        assert_eq!(after, before, "刷新不应清空已有音节字符串");
        assert!(session.spellings[..syllables]
            .iter()
            .all(|value| value.capacity() >= 64));
    }

    #[test]
    fn refresh_discards_spellings_when_there_is_no_pinyin_target() {
        let fixture = fixture();
        let mut session = open(&fixture.paths, false, EnglishInputOptions::default());
        session.digits = "64".into();
        session.refresh();
        assert!(!session.spellings.is_empty());
        session.set_english_only(true);
        assert!(session.spellings.is_empty());
        session.set_english_only(false);
        assert!(!session.spellings.is_empty());
        session.pinyin = false;
        session.refresh();
        assert!(session.spellings.is_empty());
        session.pinyin = true;
        session.refresh();
        assert!(!session.spellings.is_empty());
        // 没有撤销选项的全锁定状态也不能带出上一次的读音。
        session.locked = vec!["ni".to_owned()];
        session.refresh();
        assert!(session.spellings.is_empty());
        session.locked.clear();
        session.refresh();
        assert!(!session.spellings.is_empty());
        session.command(Command::Cancel);
        assert!(session.spellings.is_empty());
    }

    #[test]
    fn locked_refresh_reuses_the_existing_ranking_context_prefix() {
        let fixture = fixture();
        let mut session = open(&fixture.paths, false, EnglishInputOptions::default());
        type_digits(&mut session, "64426");
        assert!(
            session
                .choose_spelling(spelling_index(&session, "ni"))
                .handled
        );
        let (_, allocations) =
            crate::ime::personal_rerank::allocations::count(|| session.refresh());
        assert!(
            allocations <= 106,
            "locked refresh rebuilt the ranking context prefix: {allocations}"
        );
    }

    #[test]
    fn finish_reuses_the_first_commit_buffer() {
        let fixture = fixture_with(INITIALS_FIXTURE);
        let mut session = open(&fixture.paths, true, EnglishInputOptions::default());
        type_digits(&mut session, "963443842662");
        assert!(session.select(index_of(&session, "我滴")).handled);

        let (_, allocations) =
            crate::ime::personal_rerank::allocations::count(|| session.finish(0));
        assert!(
            allocations <= 32,
            "九键结束组字复制了首段提交文本：{allocations} 次分配"
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
    fn english_keypad_filters_full_code_before_prefix_limit() {
        for grid in [KeyGrid::NineKey, KeyGrid::FourteenKey] {
            for english_only in [false, true] {
                let fixture = fixture();
                Connection::open(fixture.paths.dictionary(assets::ENGLISH_DICTIONARY))
                    .unwrap()
                    .execute_batch("INSERT INTO english_words VALUES('previewing','previewing',1),('previewsa','previewsa',100),('previewsb','previewsb',100),('previewsc','previewsc',100),('previewsd','previewsd',100),('previewse','previewse',100);")
                    .unwrap();
                let mut session = open(&fixture.paths, false, mixed());
                session.set_grid(grid);
                session.set_english_only(english_only);
                type_digits(&mut session, &grid.encode("previewing"));
                assert!(
                    words(&session).iter().any(|word| word == "previewing"),
                    "完整组码不能被不匹配的高权重前缀词挤掉：{grid:?}，专用英文={english_only}"
                );
                let result = session.select(index_of(&session, "previewing"));
                assert_eq!(result.commit.as_deref(), Some("previewing"));
            }
        }
    }

    #[test]
    fn english_keypad_prefix_limit_uses_final_ranking() {
        let fixture = fixture();
        Connection::open(fixture.paths.dictionary(assets::ENGLISH_DICTIONARY))
            .unwrap()
            .execute_batch("INSERT INTO english_words VALUES('commit','COMMIT',1),('commita','commita',100),('commitb','commitb',100),('commitc','commitc',100),('commitd','commitd',100),('commite','commite',100);")
            .unwrap();
        let mut session = open(&fixture.paths, false, mixed());
        session.set_english_only(true);
        type_digits(&mut session, "266648");
        assert_eq!(words(&session).first().map(String::as_str), Some("COMMIT"));
        assert_eq!(words(&session).len(), ENGLISH_LIMIT);
    }

    #[test]
    fn english_keypad_invalid_rows_do_not_consume_prefix_limit() {
        let fixture = fixture();
        let db = Connection::open(fixture.paths.dictionary(assets::ENGLISH_DICTIONARY)).unwrap();
        for lookup in [
            "commit!",
            "commit ",
            "commit9",
            "commit-é",
            "commit\0suffix",
            "commit",
        ] {
            let display = (lookup != "commit").then_some("合成无效词");
            db.execute(
                "INSERT INTO english_words VALUES(?1,?2,1000)",
                rusqlite::params![lookup, display],
            )
            .unwrap();
        }
        db.execute_batch("INSERT INTO english_words VALUES('commiT','don''t',1);")
            .unwrap();
        let mut session = open(&fixture.paths, false, mixed());
        session.set_english_only(true);
        type_digits(&mut session, "266648");
        assert_eq!(words(&session), ["don't"]);
    }

    #[test]
    #[ignore = "需要通过 MSIME_TEST_ENGLISH_DICTIONARY 指定已校验的发布英文词库"]
    fn english_keypad_shipped_dictionary_reported_words() {
        let database =
            std::env::var_os("MSIME_TEST_ENGLISH_DICTIONARY").expect("指定已校验的发布英文词库");
        let fixture = fixture();
        std::fs::copy(
            database,
            fixture.paths.dictionary(assets::ENGLISH_DICTIONARY),
        )
        .unwrap();
        for grid in [KeyGrid::NineKey, KeyGrid::FourteenKey] {
            for english_only in [false, true] {
                for (lookup, display) in [
                    ("github", "GitHub"),
                    ("preview", "preview"),
                    ("commit", "commit"),
                    ("clash", "clash"),
                ] {
                    let mut session = open(&fixture.paths, false, mixed());
                    session.set_grid(grid);
                    session.set_english_only(english_only);
                    type_digits(&mut session, &grid.encode(lookup));
                    assert!(
                        words(&session).iter().any(|word| word == display),
                        "发布词库缺少候选：{display}，{grid:?}，专用英文={english_only}"
                    );
                    assert_eq!(
                        session
                            .select(index_of(&session, display))
                            .commit
                            .as_deref(),
                        Some(display)
                    );
                }
            }
        }
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
    fn english_refresh_does_not_allocate_a_copy_of_typed_digits() {
        let fixture = fixture();
        let mut session = open(&fixture.paths, false, EnglishInputOptions::default());
        session.set_english_only(true);
        session.digits = "3668".into();
        session.english_candidates(false);

        let (_, allocations) =
            crate::ime::personal_rerank::allocations::count(|| session.english_candidates(false));
        assert!(
            allocations < 157,
            "英文候选热刷新不应复制已输入的数字：{allocations} 次分配"
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
        // 到上限但没有候选可选：运行时不能替用户选首选，这一键照旧拦下。
        assert!(!session.limit_pick_ready(b'7'));
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

        // 到上限、首选占用组码、光标在末尾时，运行时可以先选掉首选再接着打；光标移到中间时不行。不是组码的键不算。
        let long = "64426".repeat(DIGIT_LIMIT / 5 + 1);
        type_digits(&mut session, &long[..DIGIT_LIMIT]);
        assert!(!session.snapshot().candidates.is_empty());
        assert!(session.limit_pick_ready(b'6'));
        assert!(!session.limit_pick_ready(b'1'));
        session.command(Command::MoveLeft);
        assert!(!session.limit_pick_ready(b'6'));
        session.command(Command::MoveEnd);
        assert!(session.limit_pick_ready(b'6'));
        // 提示里写的上限和常量一致。
        assert!(diagnostics::NINE_KEY_DIGIT_LIMIT.contains(&DIGIT_LIMIT.to_string()));
        session.command(Command::Cancel);
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
    fn mixed_english_first_page_count_respects_the_page_budget() {
        let fixture = fixture();
        Connection::open(fixture.paths.dictionary(assets::ENGLISH_DICTIONARY))
            .unwrap()
            .execute_batch("INSERT INTO english_words VALUES('night','night',900),('nights','nights',800),('nightly','nightly',700);")
            .unwrap();
        for (count, page_rows, expected) in [
            (
                1,
                5,
                vec!["你", "night", "米", "你好", "nights", "nightly", "ogham"],
            ),
            (
                2,
                5,
                vec!["你", "night", "nights", "米", "你好", "nightly", "ogham"],
            ),
            (
                3,
                5,
                vec!["你", "night", "nights", "nightly", "米", "你好", "ogham"],
            ),
            (
                3,
                2,
                vec!["你", "night", "米", "你好", "nights", "nightly", "ogham"],
            ),
        ] {
            let mut session = open(
                &fixture.paths,
                false,
                EnglishInputOptions {
                    mixed_candidates: true,
                    minimum_prefix: 2,
                    first_page_count: count,
                },
            );
            session.set_first_page_rows(FirstPageRows::new(page_rows));
            type_digits(&mut session, "64");
            assert_eq!(
                words(&session),
                expected,
                "首页英文数={count}，首页行数={page_rows}"
            );
            assert_eq!(
                session
                    .select(index_of(&session, "nights"))
                    .commit
                    .as_deref(),
                Some("nights")
            );
        }
    }

    #[test]
    fn english_only_spells_words_from_the_first_digit() {
        let fixture = fixture();
        let options = EnglishInputOptions {
            mixed_candidates: false,
            minimum_prefix: 2,
            first_page_count: 1,
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
            .map(|item| item.word.into_string())
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
            ..Default::default()
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

    #[test]
    fn lattice_sentence_slots_filter_in_order_without_allocations() {
        let mut candidates = Vec::new();
        candidates.push(item("跳过来源", "64426", 0, CandidateSource::Database));
        candidates.push(sentence("整句 0", 0));
        candidates.push(sentence("整句 1", 0));
        let mut non_sentence = sentence("跳过标记", 0);
        non_sentence.sentence_association = false;
        candidates.push(non_sentence);
        for index in 2..14 {
            candidates.push(sentence(&format!("整句 {index}"), 0));
        }

        let ((slots, len), allocations) = crate::ime::personal_rerank::allocations::count(|| {
            collect_lattice_sentences(&candidates)
        });

        assert_eq!(len, MAX_RERANK_PATHS);
        assert_eq!(slots[0].map(|item| item.word.as_str()), Some("整句 0"));
        assert_eq!(slots[1].map(|item| item.word.as_str()), Some("整句 1"));
        assert_eq!(
            slots[MAX_RERANK_PATHS - 1].map(|item| item.word.as_str()),
            Some("整句 11")
        );
        assert_eq!(allocations, 0);
    }

    #[test]
    fn rerank_cache_match_uses_borrowed_rows_without_allocating() {
        let rows = [sentence("整句 0", 17), sentence("整句 1", 23)];
        let scored = [Some(&rows[0]), Some(&rows[1])];
        let entry = RerankEntry {
            context: "上文".to_owned(),
            sentences: rows
                .iter()
                .map(|row| (row.word.to_string(), comparable_weight(row)))
                .collect(),
            ranked: vec!["整句 1".to_owned(), "整句 0".to_owned()],
        };
        let (matches, allocations) = crate::ime::personal_rerank::allocations::count(|| {
            rerank_cache_matches(&entry, "上文", &scored)
        });
        assert!(matches);
        assert_eq!(allocations, 0);
        assert!(!rerank_cache_matches(&entry, "别的上下文", &scored));
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

    /// 选中过的单字也应在相同九键输入的下一次候选刷新中直接位于首位，而不依赖个人上下文模型。
    #[test]
    fn recently_selected_candidate_leads_the_same_nine_key_context() {
        let fixture = fixture_with(
            "CREATE TABLE tbl_1_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_n VALUES('neng','n','能',1),('neng','n','恁',100),('neng','n','嫩',90),('neng','n','嗯',80),('neng','n','𠮾',70),('neng','n','莻',60);",
        );
        let mut first = open(&fixture.paths, true, EnglishInputOptions::default());
        type_digits(&mut first, "6364");
        let selected = index_of(&first, "能");
        assert!(selected > 0, "fixture must require a non-first selection");
        assert_eq!(first.select(selected).commit.as_deref(), Some("能"));

        let mut repeated = open(&fixture.paths, true, EnglishInputOptions::default());
        type_digits(&mut repeated, "6364");
        assert_eq!(words(&repeated)[0], "能");
        assert_eq!(repeated.snapshot().candidates[0].fixed_position, 1);
    }

    const SPELLING_RECENT_FIXTURE: &str =
        "CREATE TABLE tbl_1_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_n VALUES('ni','n','你',1000);\
CREATE TABLE tbl_1_m(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_m VALUES('mi','m','米',100),('mi','m','谜',10),('mo','m','墨',100);\
CREATE TABLE tbl_1_h(key TEXT,jp TEXT,value TEXT,weight INTEGER);\
INSERT INTO tbl_1_h VALUES('hao','h','好',100);";

    fn lock_spelling(session: &mut NineKeySession, spelling: &str) {
        let index = session
            .snapshot()
            .nine_key_spellings
            .iter()
            .position(|choice| choice == spelling)
            .expect("synthetic spelling offered");
        assert!(session.choose_spelling(index).handled);
    }

    /// 显式选读法再确认候选，也要让下一次未锁定的数字输入记住这次选择。
    #[test]
    fn spelling_selection_remembers_first_and_later_candidates_without_a_lock() {
        for (word, selected_index) in [("米", 0), ("谜", 1)] {
            let fixture = fixture_with(SPELLING_RECENT_FIXTURE);
            let mut session =
                open_with_frequency(&fixture.paths, FrequencyAdjustmentMode::Disabled, 1, 1);
            type_digits(&mut session, "64");
            assert_eq!(words(&session)[0], "你");
            lock_spelling(&mut session, "mi");
            assert_eq!(index_of(&session, word), selected_index);
            let selected = session.select(selected_index);
            assert_eq!(selected.commit.as_deref(), Some(word));
            assert_eq!(selected.diagnostic, None);
            drop(session);

            let mut repeated = open(&fixture.paths, false, EnglishInputOptions::default());
            type_digits(&mut repeated, "64");
            assert_eq!(words(&repeated)[0], word);
            assert_eq!(repeated.snapshot().candidates[0].fixed_position, 1);
            lock_spelling(&mut repeated, "ni");
            assert_eq!(words(&repeated)[0], "你");
        }
    }

    /// 未显式完成整串选择时，不把读法筛选本身当作最近选词。
    #[test]
    fn spelling_recent_requires_learning_and_an_explicit_whole_selection() {
        for case in ["learning_off", "finish", "partial", "unlocked_first"] {
            let fixture = fixture_with(SPELLING_RECENT_FIXTURE);
            let mut session =
                open_with_frequency(&fixture.paths, FrequencyAdjustmentMode::Disabled, 1, 1);
            session.learning = case != "learning_off";
            let digits = if case == "partial" { "64426" } else { "64" };
            type_digits(&mut session, digits);
            if case != "unlocked_first" {
                lock_spelling(&mut session, "mi");
            }
            let word = if case == "unlocked_first" {
                "你"
            } else {
                "米"
            };
            let index = index_of(&session, word);
            let result = if case == "finish" {
                session.finish(index)
            } else {
                session.select(index)
            };
            assert_eq!(result.commit.as_deref(), Some(word), "{case}");
            assert_eq!(result.diagnostic, None, "{case}");
            if case == "partial" {
                assert!(session.active());
            }
            let journal = fixture.paths.user(assets::USER_JOURNAL);
            for (key, spelling, value) in
                [("64", "mi", "米"), (digits, "mi", "米"), ("64", "ni", "你")]
            {
                assert!(
                    !positions::is_fixed(&journal, &format!("nine-key:{key}:"), spelling, value),
                    "{case}"
                );
                assert!(
                    !positions::is_fixed(
                        &journal,
                        &format!("nine-key:{key}:{spelling}"),
                        spelling,
                        value
                    ),
                    "{case}"
                );
            }
        }
    }

    /// 未锁定的最近候选变化后，再选已有锁定首位也能更新；不同数字与读法仍隔离。
    #[test]
    fn spelling_recent_replaces_the_previous_unlocked_pick() {
        let fixture = fixture_with(SPELLING_RECENT_FIXTURE);
        let journal = fixture.paths.user(assets::USER_JOURNAL);
        positions::set_fixed_position(&journal, "nine-key:64:mi", "mi", "米", 1).unwrap();
        for word in ["米", "你", "米"] {
            let mut session =
                open_with_frequency(&fixture.paths, FrequencyAdjustmentMode::Disabled, 1, 1);
            type_digits(&mut session, "64");
            if word == "米" {
                lock_spelling(&mut session, "mi");
                assert_eq!(session.snapshot().candidates[0].fixed_position, 1);
            }
            let index = index_of(&session, word);
            assert_eq!(session.select(index).commit.as_deref(), Some(word));
            let mut repeated = open(&fixture.paths, false, EnglishInputOptions::default());
            type_digits(&mut repeated, "64");
            assert_eq!(words(&repeated)[0], word);
            lock_spelling(&mut repeated, "ni");
            assert_eq!(words(&repeated), ["你"]);
            repeated.command(Command::Cancel);
            type_digits(&mut repeated, "66");
            assert_eq!(words(&repeated)[0], "墨");
            assert_eq!(repeated.snapshot().candidates[0].fixed_position, 0);
        }
    }

    /// 读法内的位置不能覆盖未锁定上下文里同一词或其它词的手动槽位。
    #[test]
    fn spelling_recent_preserves_unlocked_manual_slots() {
        for selected_is_fixed in [false, true] {
            let fixture = fixture_with(SPELLING_RECENT_FIXTURE);
            let journal = fixture.paths.user(assets::USER_JOURNAL);
            positions::set_fixed_position(&journal, "nine-key:64:", "mi", "谜", 2).unwrap();
            if selected_is_fixed {
                positions::set_fixed_position(&journal, "nine-key:64:", "mi", "米", 3).unwrap();
            }
            let mut session =
                open_with_frequency(&fixture.paths, FrequencyAdjustmentMode::Disabled, 1, 1);
            type_digits(&mut session, "64");
            lock_spelling(&mut session, "mi");
            assert_eq!(session.select(0).commit.as_deref(), Some("米"));
            let mut repeated = open(&fixture.paths, false, EnglishInputOptions::default());
            type_digits(&mut repeated, "64");
            let snapshot = repeated.snapshot();
            let target = &snapshot.candidates[index_of(&repeated, "米")];
            assert_eq!(target.fixed_position, if selected_is_fixed { 3 } else { 1 });
            assert_eq!(snapshot.candidates[1].word, "谜");
            assert_eq!(snapshot.candidates[1].fixed_position, 2);
        }
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
            word: word.into(),
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
        symbol_candidates: false,
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
                symbol_candidates: false,
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

        let readings: Vec<String> = session
            .expressive_readings(&table.paths("94264"), &[], &session.candidates)
            .into_iter()
            .map(|(reading, _)| reading)
            .collect();
        assert_eq!(readings.len(), EXPRESSIVE_READING_LIMIT);
        // 排在最前的拼音候选 香（xiang）支持的读法查在最前。
        assert_eq!(session.candidates[0].word, "香");
        assert_eq!(readings[0], "xiang");
        // 只拼了开头一个音节的路径不算读法。
        assert!(readings.iter().all(|reading| reading.len() == 5));
    }

    #[test]
    fn shuangpin_readings_do_not_allocate_an_intermediate_boundary_table() {
        let fixture = fixture_with(SHUANGPIN_FIXTURE);
        let mut session = shuangpin_session(&fixture.paths, NineKeyShuangpinKind::Migala3);
        session.digits = "6447".into();
        let alternatives = session.grid.spelling_table().paths("6447");
        let candidates = session.candidates.clone();
        let boundaries = [2, 4];
        let (_, allocations) = crate::ime::personal_rerank::allocations::count(|| {
            session.expressive_readings(&alternatives, &boundaries, &candidates)
        });
        assert!(
            allocations <= 100,
            "shuangpin readings allocated an intermediate boundary table: {allocations}"
        );
    }

    #[test]
    fn shuangpin_readings_materialize_boundaries_only_for_retained_readings() {
        let fixture = fixture_with(SHUANGPIN_FIXTURE);
        let mut session = shuangpin_session(&fixture.paths, NineKeyShuangpinKind::Migala3);
        session.digits = "94264".into();
        let alternatives = session.grid.spelling_table().paths("94264");
        let candidates = session.candidates.clone();
        let boundaries = [2, 4];
        let (_, allocations) = crate::ime::personal_rerank::allocations::count(|| {
            session.expressive_readings(&alternatives, &boundaries, &candidates)
        });
        assert!(
            allocations <= 100,
            "discarded shuangpin readings still materialized boundaries: {allocations}"
        );
    }

    #[test]
    fn ordinary_nine_key_readings_borrow_shared_boundaries() {
        let fixture = expressive_fixture();
        let mut session =
            open_expressive(&fixture, EnglishInputOptions::default(), BOTH_EXPRESSIVE);
        session.digits = "634486".into();
        let alternatives = spelling_table().paths("634486");
        let candidates = session.candidates.clone();
        let boundaries = [3, 6];
        let (_, allocations) = crate::ime::personal_rerank::allocations::count(|| {
            session.expressive_readings(&alternatives, &boundaries, &candidates)
        });
        assert!(
            allocations <= 84,
            "ordinary readings copied shared boundaries: {allocations}"
        );
    }

    #[test]
    fn expressive_readings_do_not_allocate_a_separate_locked_prefix() {
        let fixture = expressive_fixture();
        let mut session =
            open_expressive(&fixture, EnglishInputOptions::default(), BOTH_EXPRESSIVE);
        type_digits(&mut session, "634486");
        assert!(
            session
                .choose_spelling(spelling_index(&session, "mei"))
                .handled
        );
        let alternatives = session.grid.spelling_table().paths("486");
        let candidates = session.candidates.clone();
        let (_, allocations) = crate::ime::personal_rerank::allocations::count(|| {
            session.expressive_readings(&alternatives, &[], &candidates)
        });
        assert!(
            allocations <= 15,
            "locked expressive readings allocated {allocations} buffers"
        );
    }

    // ---- 打拼音出符号 ----

    /// 正负（zheng'fu）排在 政府 后面；派（pai）是单音节名称，前面另有三个 pai 的字。
    const SYMBOL_MAIN_FIXTURE: &str = "CREATE TABLE tbl_1_z(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_1_z VALUES('zheng','z','正',300),('zheng','z','政',200);CREATE TABLE tbl_1_f(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_1_f VALUES('fu','f','负',300),('fu','f','府',200);CREATE TABLE tbl_2_z(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_2_z VALUES('zheng''fu','zf','政府',1000),('zheng''fu','zf','正负',500);CREATE TABLE tbl_1_p(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_1_p VALUES('pai','p','派',900),('pai','p','排',800),('pai','p','牌',700),('pai','p','拍',600);";

    const SYMBOLS_ONLY: MixedExpressiveOptions = MixedExpressiveOptions {
        emoji_candidates: false,
        kaomoji_candidates: false,
        symbol_candidates: true,
    };

    fn has_symbol_row(session: &NineKeySession, symbol: &str) -> bool {
        sources(session)
            .iter()
            .zip(words(session))
            .any(|(source, word)| *source == CandidateSource::Emoji && word == symbol)
    }

    #[test]
    fn digits_spelling_a_whole_symbol_name_add_its_symbol_after_the_name() {
        let fixture = fixture_with(SYMBOL_MAIN_FIXTURE);
        let mut session = open_expressive(&fixture, EnglishInputOptions::default(), SYMBOLS_ONLY);
        // 9436438 读作 zhengfu：± 紧跟在 正负 后面，不占首位。
        type_digits(&mut session, "9436438");
        let listed = words(&session);
        assert_eq!(listed[..3], ["政府", "正负", "±"]);
        assert_eq!(sources(&session)[2], CandidateSource::Emoji);
        assert_eq!(session.snapshot().candidates[2].pinyin, "9436438");
        session.command(Command::Cancel);

        // 没打完整的名称不出。
        type_digits(&mut session, "943643");
        assert!(!has_symbol_row(&session, "±"));
        session.command(Command::Cancel);

        // 单音节名称最早排第 4 位（下标 3）。
        type_digits(&mut session, "724");
        let listed = words(&session);
        assert_eq!(listed[0], "派");
        assert_eq!(listed[3], "π");
        session.command(Command::Cancel);

        // 单字筛选时不出。
        type_digits(&mut session, "9436438");
        assert!(session.set_filter(true, "").handled);
        assert!(!has_symbol_row(&session, "±"));
        session.command(Command::Cancel);

        // 开关关着时不出，其余候选逐项相同。
        let mut off = open_expressive(
            &fixture,
            EnglishInputOptions::default(),
            MixedExpressiveOptions::default(),
        );
        let mut on = open_expressive(&fixture, EnglishInputOptions::default(), SYMBOLS_ONLY);
        type_digits(&mut off, "9436438");
        type_digits(&mut on, "9436438");
        assert!(!has_symbol_row(&off, "±"));
        let mut without_symbols = on.snapshot().candidates;
        without_symbols.retain(|item| item.source != CandidateSource::Emoji);
        assert_eq!(without_symbols, off.snapshot().candidates);
    }

    #[test]
    fn choosing_a_symbol_row_commits_it_and_eats_every_digit() {
        let fixture = fixture_with(SYMBOL_MAIN_FIXTURE);
        let mut session = open_expressive(&fixture, EnglishInputOptions::default(), SYMBOLS_ONLY);
        type_digits(&mut session, "9436438");
        let chosen = session.select(index_of(&session, "±"));
        assert_eq!(chosen.commit.as_deref(), Some("±"));
        assert!(!session.active());
        // 不能置顶、删除或固定位置。
        type_digits(&mut session, "9436438");
        let index = index_of(&session, "±");
        assert!(!session.pin(index).handled);
        assert!(!session.remove(index).handled);
        assert!(!session.set_position(index, 2).handled);
    }

    /// 名称词被固定到后面的位置时，接在它后面的符号不能顶到首位。
    #[test]
    fn a_symbol_row_never_leads_after_its_name_is_fixed_further_down() {
        let fixture = fixture_with("CREATE TABLE tbl_1_d(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_1_d VALUES('da','d','大',300);CREATE TABLE tbl_1_y(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_1_y VALUES('yu','y','于',300);CREATE TABLE tbl_2_d(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_2_d VALUES('da''yu','dy','大于',900);");
        let mut session = open_expressive(&fixture, EnglishInputOptions::default(), SYMBOLS_ONLY);
        type_digits(&mut session, "3298");
        assert_eq!(words(&session)[..3], ["大于", ">", "＞"]);
        assert!(session.set_position(0, 3).handled);
        // 名称词留在固定的第 3 位。
        let listed = words(&session);
        assert_ne!(sources(&session)[0], CandidateSource::Emoji, "{listed:?}");
        assert_eq!(listed[2], "大于", "{listed:?}");
        assert!(listed.contains(&">".to_owned()) && listed.contains(&"＞".to_owned()));
    }

    #[test]
    fn symbols_only_does_not_allocate_an_unused_other_dictionary_path() {
        let fixture = fixture_with(SYMBOL_MAIN_FIXTURE);
        let mut session = open_expressive(&fixture, EnglishInputOptions::default(), SYMBOLS_ONLY);
        session.digits = "9436438".into();
        let alternatives = spelling_table().paths("9436438");
        assert!(!session
            .expressive_readings(&alternatives, &[], &[])
            .is_empty());
        let candidates = session.candidates.clone();
        let (_, allocations) = crate::ime::personal_rerank::allocations::count(|| {
            session.expressive_candidates(&alternatives, &candidates)
        });
        assert!(
            allocations <= 1150,
            "symbols-only expressive path allocated an unused resource path: {allocations}"
        );
    }

    fn frozen_splits_into_syllables_at(key: &str, boundaries: &[usize]) -> bool {
        let mut start = 0;
        for &end in boundaries.iter().chain(std::iter::once(&key.len())) {
            let Some(piece) = key.get(start..end) else {
                return false;
            };
            if !piece.is_empty()
                && crate::pinyin::segment::cut_one_piece_min_segments(piece, true).is_empty()
            {
                return false;
            }
            start = end;
        }
        true
    }

    #[test]
    fn expressive_boundary_check_matches_owned_cuts() {
        let mut inputs = vec![
            String::new(),
            "nihao".into(),
            "meiguo".into(),
            "xiangjiao".into(),
            "xi'an".into(),
            "ni''hao".into(),
            "'ni".into(),
            "ni'".into(),
            "你好".into(),
            "ni好".into(),
            "NIHAO".into(),
            "gamma".into(),
            "ni".repeat(32),
            "ni".repeat(33),
            "ni".repeat(33) + "h",
        ];
        for syllable in intact_pinyin_list() {
            for suffix in ["", "ni", "h", "'an", "好", "sahng", "zhonguo"] {
                inputs.push(format!("{syllable}{suffix}"));
            }
        }
        for key in inputs {
            let mut boundaries = vec![
                vec![],
                vec![0],
                vec![key.len()],
                vec![0, key.len()],
                vec![key.len(), key.len()],
                vec![key.len() + 1],
                vec![usize::MAX],
                vec![key.len(), 0],
            ];
            boundaries.extend((0..=key.len()).map(|at| vec![at, at]));
            for boundaries in boundaries {
                assert_eq!(
                    splits_into_syllables_at(&key, &boundaries),
                    frozen_splits_into_syllables_at(&key, &boundaries),
                    "编码 {key:?}，边界 {boundaries:?}"
                );
            }
        }
    }

    #[test]
    fn expressive_boundary_check_does_not_materialize_syllables() {
        use crate::ime::personal_rerank::allocations::count;
        assert!(splits_into_syllables_at("nihao", &[]));
        for (key, boundaries, expected_allocations) in [
            ("nihao".to_owned(), vec![], 0),
            ("meiguo".to_owned(), vec![3], 0),
            ("xiangjiao".to_owned(), vec![2, 5], 0),
            ("ni".repeat(32), vec![], 0),
            ("ni".repeat(33), vec![], 1),
        ] {
            let (expected, old) = count(|| frozen_splits_into_syllables_at(&key, &boundaries));
            let (actual, new) = count(|| splits_into_syllables_at(&key, &boundaries));
            assert!(expected);
            assert_eq!(actual, expected);
            eprintln!("表情边界：{} 字节 {boundaries:?}，{old} → {new}", key.len());
            assert_eq!(new, expected_allocations, "只应保留超长片段的 DP 表分配");
            assert!(new < old);
        }
    }

    #[test]
    fn expressive_symbol_query_preserves_rows_without_cut_results() {
        use crate::ime::personal_rerank::allocations::count;
        let readings = vec!["zhengfu".to_owned()];
        let boundaries = [5];
        let current = |key: &str| splits_into_syllables_at(key, &boundaries);
        let frozen = |key: &str| frozen_splits_into_syllables_at(key, &boundaries);
        assert!(!query_symbol_readings(&readings, &current).is_empty());
        let (expected, old) = count(|| query_symbol_readings(&readings, &frozen));
        let (actual, new) = count(|| query_symbol_readings(&readings, &current));
        assert_eq!(actual, expected);
        eprintln!("符号查询：{old} → {new}");
        assert!(new < old, "符号查询仍然物化了边界切分结果");
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

    // ---- 全拼归组键盘：同一套解码替换码表，与键盘有关的判断按三种网格分别检查 ----

    const GRIDS: [KeyGrid; 3] = [KeyGrid::NineKey, KeyGrid::FourteenKey, KeyGrid::EighteenKey];

    /// 14 键的词库：nihao 和 buhao、xi'an 和 xian 在 14 键上是同一串组码。
    const FOURTEEN_FIXTURE: &str = "CREATE TABLE tbl_1_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_1_n VALUES('ni','n','你',100);CREATE TABLE tbl_1_b(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_1_b VALUES('bu','b','不',90);CREATE TABLE tbl_1_h(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_1_h VALUES('hao','h','好',100);CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_2_n VALUES('ni''hao','nh','你好',1000);CREATE TABLE tbl_2_b(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_2_b VALUES('bu''hao','bh','不好',500);CREATE TABLE tbl_1_x(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_1_x VALUES('xi','x','西',100),('xian','x','先',300);CREATE TABLE tbl_1_a(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_1_a VALUES('an','a','安',100);CREATE TABLE tbl_2_x(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_2_x VALUES('xi''an','xa','西安',800);CREATE TABLE tbl_1_s(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_1_s VALUES('shi','s','是',1000),('shu','s','书',500);";

    fn fourteen_key_session(paths: &RuntimePaths, english: EnglishInputOptions) -> NineKeySession {
        let mut session = open(paths, false, english);
        session.set_grid(KeyGrid::FourteenKey);
        session
    }

    #[test]
    fn grid_codes_come_from_the_printed_keys() {
        for grid in GRIDS {
            // 每个字母恰好印在一个键上，它的组码就是那个键。
            let mut derived = [0u8; 26];
            let codes: Vec<u8> = match grid {
                KeyGrid::NineKey => (b'2'..=b'9').collect(),
                KeyGrid::FourteenKey => FOURTEEN_KEY_GROUPS
                    .iter()
                    .map(|group| group.as_bytes()[0])
                    .collect(),
                KeyGrid::EighteenKey => EIGHTEEN_KEY_GROUPS
                    .iter()
                    .map(|group| group.as_bytes()[0])
                    .collect(),
                KeyGrid::NineKeyShuangpin(_)
                | KeyGrid::FourteenKeyShuangpin(_)
                | KeyGrid::EighteenKeyShuangpin(_) => {
                    unreachable!("GRIDS 只有按字母归组的键盘")
                }
            };
            for &code in &codes {
                assert!(grid.is_code(code));
                for letter in grid.letters(code).bytes() {
                    let slot = &mut derived[usize::from(letter - b'a')];
                    assert_eq!(*slot, 0, "{grid:?}: {} on two keys", char::from(letter));
                    *slot = code;
                }
            }
            assert_eq!(Some(&derived), grid.keypad(), "{grid:?}");
            for letter in b'a'..=b'z' {
                let code = grid.code_of(letter).unwrap();
                assert!(grid.letters(code).as_bytes().contains(&letter));
            }
            assert_eq!(grid.code_of(b'A'), None);
            assert_eq!(grid.code_of(b'\''), None);
            assert!(!grid.is_code(b'\'') && !grid.is_code(b'A') && !grid.is_code(b'1'));
        }
        assert_eq!(FOURTEEN_KEYPAD, b"abcdedggujjlmbooqeatucqztz");
        assert_eq!(KeyGrid::FourteenKey.encode("ni'hao"), "bugao");
        assert_eq!(KeyGrid::FourteenKey.letters(b'l'), "l");
        assert_eq!(KeyGrid::FourteenKey.letters(b'w'), "", "w is not a code");
        assert!(KeyGrid::NineKey.offers_raw_key());
        assert!(!KeyGrid::FourteenKey.offers_raw_key());
    }

    #[test]
    fn syllable_codes_per_grid() {
        for (grid, expected) in [(KeyGrid::NineKey, 218), (KeyGrid::FourteenKey, 314)] {
            let codes: HashSet<String> = intact_pinyin_list()
                .iter()
                .map(|syllable| grid.encode(syllable))
                .collect();
            assert_eq!(intact_pinyin_list().len(), 421);
            assert_eq!(codes.len(), expected, "{grid:?}");
        }
    }

    #[test]
    fn fourteen_key_shares_codes_between_syllables() {
        let grid = KeyGrid::FourteenKey;
        for (code, syllables) in [
            ("bu", ["bi", "bu", "ni", "nu"]),
            ("zu", ["xi", "xu", "zi", "zu"]),
        ] {
            for syllable in syllables {
                assert_eq!(grid.encode(syllable), code, "{syllable}");
            }
        }
        assert_eq!(grid.encode("shi"), grid.encode("shu"));
        // 跨音节的歧义：sha 和 a'ga 是同一串组码。
        assert_eq!(grid.encode("sha"), grid.encode("a'ga"));
        let paths = grid
            .spelling_table()
            .split_paths("aga", &[], &SyllablePrior::default());
        assert!(paths.contains(&path(&["sha"])), "{paths:?}");
        assert!(paths.contains(&path(&["a", "ga"])), "{paths:?}");
        let table = grid.spelling_table();
        let spellings = table.spellings_for("agu", 0, None, &SyllablePrior::default());
        assert!(spellings.contains(&"shi".to_owned()) && spellings.contains(&"shu".to_owned()));
    }

    #[test]
    fn grid_initials_expand_each_key_and_stop_at_keys_without_initials() {
        assert_eq!(
            super::initials_codes(KeyGrid::FourteenKey, "bg", 1024).unwrap(),
            ["bg", "bh", "ng", "nh"]
        );
        // UI 键上的 u、i 都不起头音节，含它的串没有简拼。
        assert!(super::initials_codes(KeyGrid::FourteenKey, "bug", 1024).is_none());
        // 两个字母一键，十个键正好 1024 个码。
        assert_eq!(
            super::initials_codes(KeyGrid::FourteenKey, "qqqqqqqqqq", 1024)
                .unwrap()
                .len(),
            1024
        );
        assert!(super::initials_codes(KeyGrid::FourteenKey, "qqqqqqqqqqq", 1024).is_none());
        // 九键的 1 键同样没有字母。
        assert!(super::initials_codes(KeyGrid::NineKey, "61", 1024).is_none());
    }

    #[test]
    fn grid_letter_prefixes_and_english_matches() {
        assert_eq!(
            super::letter_prefixes(KeyGrid::FourteenKey, "oj", 64),
            ["oj", "ok", "pj", "pk"]
        );
        assert_eq!(super::letter_prefixes(KeyGrid::FourteenKey, "l", 64), ["l"]);
        assert!(super::word_matches_digits(KeyGrid::FourteenKey, "OK", "oj"));
        assert!(super::word_matches_digits(KeyGrid::FourteenKey, "ok", "o"));
        assert!(!super::word_matches_digits(
            KeyGrid::FourteenKey,
            "ok",
            "ob"
        ));
        assert!(!super::word_matches_digits(KeyGrid::NineKey, "ok", "oj"));
        assert!(super::keyword_spells_digits(
            KeyGrid::FourteenKey,
            "riqi",
            "euqu"
        ));
        assert!(super::letters_spell_code(KeyGrid::FourteenKey, "ni", "bu"));
        assert!(!super::letters_spell_code(KeyGrid::FourteenKey, "ni", "64"));
    }

    #[test]
    fn fourteen_key_left_column_has_letters_but_no_raw_key() {
        let fixture = fixture_with(FOURTEEN_FIXTURE);
        let mut nine = open(&fixture.paths, false, EnglishInputOptions::default());
        let mut fourteen = fourteen_key_session(&fixture.paths, EnglishInputOptions::default());
        assert!(
            !fourteen.character(b'6').handled,
            "digits are not 14-key codes"
        );
        assert!(
            !fourteen.character(b'w').handled,
            "only the first letter is a code"
        );
        type_digits(&mut nine, "7");
        type_digits(&mut fourteen, "q");
        let spellings = fourteen.snapshot().nine_key_spellings;
        assert_eq!(spellings[spellings.len() - 2..], ["Q", "W"]);
        assert!(!spellings.contains(&"q".to_owned()), "{spellings:?}");
        assert_eq!(
            nine.snapshot()
                .nine_key_spellings
                .last()
                .map(String::as_str),
            Some("7")
        );
        fourteen.command(Command::Cancel);
        // UI 键上的字母都不起头音节：没有字母项，也没有原样上屏项。
        type_digits(&mut fourteen, "u");
        assert!(fourteen.snapshot().nine_key_spellings.is_empty());
        fourteen.command(Command::Cancel);
        // AS GH UI：选择条里有 shi 和 shu。
        type_digits(&mut fourteen, "agu");
        let spellings = fourteen.snapshot().nine_key_spellings;
        assert!(
            spellings.starts_with(&["shi".to_owned(), "shu".to_owned()]),
            "{spellings:?}"
        );
        assert_eq!(words(&fourteen)[..2], ["是", "书"]);
        assert_eq!(fourteen.snapshot().nine_key_reading, "shi");
    }

    #[test]
    fn fourteen_key_types_words_sentences_and_splits() {
        let fixture = fixture_with(FOURTEEN_FIXTURE);
        let mut session = fourteen_key_session(&fixture.paths, EnglishInputOptions::default());
        // 你好：BN UI GH AS OP，与 不好 是同一串组码。
        type_digits(&mut session, "bugao");
        let view = session.snapshot();
        assert_eq!(view.editing_text, "bugao");
        assert_eq!(view.preedit, "bugao");
        assert_eq!(words(&session)[..2], ["你好", "不好"]);
        assert_eq!(view.nine_key_reading, "ni'hao");
        assert_eq!(view.candidates[0].pinyin, "bugao");
        let result = session.select(0);
        assert_eq!(result.commit.as_deref(), Some("你好"));
        assert!(!session.active());

        // 不打切分时 zuab 也是 xian（先）；用分词键切开就是 西安。
        type_digits(&mut session, "zu");
        assert!(session.character(b'\'').handled);
        type_digits(&mut session, "ab");
        assert_eq!(session.snapshot().preedit, "zu'ab");
        assert_eq!(words(&session).first().map(String::as_str), Some("西安"));
        assert_eq!(session.snapshot().nine_key_reading, "xi'an");
        assert!(!words(&session).contains(&"先".to_owned()));
        session.command(Command::Cancel);
        type_digits(&mut session, "zuab");
        assert!(words(&session).contains(&"先".to_owned()));
    }

    #[test]
    fn fourteen_key_locks_undoes_and_backspaces() {
        let fixture = fixture_with(FOURTEEN_FIXTURE);
        let mut session = fourteen_key_session(&fixture.paths, EnglishInputOptions::default());
        type_digits(&mut session, "bu");
        let spellings = session.snapshot().nine_key_spellings;
        let ni = spellings
            .iter()
            .position(|spelling| spelling == "ni")
            .unwrap();
        assert!(session.choose_spelling(ni).handled);
        assert_eq!(session.snapshot().preedit, "ni");
        assert_eq!(words(&session).first().map(String::as_str), Some("你"));
        // 组码全部锁定时退格先撤销锁定。
        assert!(session.command(Command::Backspace).handled);
        assert_eq!(session.snapshot().preedit, "bu");
        assert!(session.locked.is_empty());
        assert!(session.command(Command::Backspace).handled);
        assert_eq!(session.snapshot().preedit, "b");
        // 锁定之后还有没锁定的组码：退格删的是组码。
        type_digits(&mut session, "ugao");
        let spellings = session.snapshot().nine_key_spellings;
        let bu = spellings
            .iter()
            .position(|spelling| spelling == "bu")
            .unwrap();
        assert!(session.choose_spelling(bu).handled);
        assert_eq!(session.snapshot().preedit, "bu'gao");
        assert_eq!(words(&session).first().map(String::as_str), Some("不好"));
        assert!(session.command(Command::Backspace).handled);
        assert_eq!(session.snapshot().preedit, "bu'ga");
        assert_eq!(session.locked, ["bu"]);
        // 选首字母：W 限定下一个音节以 w 开头，退格撤销它。
        session.command(Command::Cancel);
        type_digits(&mut session, "q");
        let spellings = session.snapshot().nine_key_spellings;
        let w = spellings
            .iter()
            .position(|spelling| spelling == "W")
            .unwrap();
        assert!(session.choose_spelling(w).handled);
        assert_eq!(session.snapshot().preedit, "w");
        assert!(session.command(Command::Backspace).handled);
        assert_eq!(session.snapshot().preedit, "q");
    }

    #[test]
    fn fourteen_key_mixes_english_words_and_reads_them() {
        let fixture = fixture_with(FOURTEEN_FIXTURE);
        let mut session = fourteen_key_session(&fixture.paths, mixed());
        // OP JK：拼音读不通，出 ok，读音是这个词覆盖已打键数的字母。
        type_digits(&mut session, "oj");
        assert_eq!(words(&session).first().map(String::as_str), Some("ok"));
        assert_eq!(session.snapshot().nine_key_reading, "ok");
        session.command(Command::Cancel);
        // 英文词比已打的键长时只读已打的那几个：OP LL → ol，首行是 old。
        type_digits(&mut session, "ol");
        assert_eq!(words(&session).first().map(String::as_str), Some("old"));
        assert_eq!(session.snapshot().nine_key_reading, "ol");
        let result = session.select(0);
        assert_eq!(result.commit.as_deref(), Some("old"));
        assert!(!session.active());
    }

    /// 读音行按首行的来源判定：九键下与按 `pinyin` 是否全是数字判定的结果相同，14 键下英文首行也有读音。
    #[test]
    fn reading_is_judged_by_where_the_leading_row_came_from() {
        let mut nine = detached();
        nine.digits = "94".into();
        let mut fourteen = detached();
        fourteen.grid = KeyGrid::FourteenKey;
        fourteen.digits = "zu".into();
        let pinyin_sources = [
            CandidateSource::Database,
            CandidateSource::UserDatabase,
            CandidateSource::Generated,
            CandidateSource::Fallback,
            CandidateSource::NeuralKeyboard,
        ];
        for source in pinyin_sources {
            assert_eq!(
                nine.reading_for(Some(&spelled("西", "94", "xi", 1, source))),
                "xi"
            );
            assert_eq!(
                fourteen.reading_for(Some(&spelled("西", "zu", "xi", 1, source))),
                "xi"
            );
            // 日期时间行是 `Generated`，没有规范读音。
            assert_eq!(nine.reading_for(Some(&item("2026年", "94", 1, source))), "");
        }
        for source in [CandidateSource::Emoji, CandidateSource::Kaomoji] {
            // emoji、颜文字行的 `pinyin` 是全部组码，没有规范读音。
            assert_eq!(nine.reading_for(Some(&item("😀", "94", 1, source))), "");
            assert_eq!(fourteen.reading_for(Some(&item("😀", "zu", 1, source))), "");
        }
        let english = item("xi", "xi", 1, CandidateSource::EnglishDictionary);
        // 英文行的 `pinyin` 是查询键：九键下不全是数字，没有读音；14 键下是覆盖已打键数的字母。
        assert_eq!(nine.reading_for(Some(&english)), "");
        assert_eq!(fourteen.reading_for(Some(&english)), "xi");
        let longer = item("Xian", "xian", 1, CandidateSource::EnglishDictionary);
        assert_eq!(fourteen.reading_for(Some(&longer)), "xi");
        assert_eq!(nine.reading_for(None), "");
    }

    #[test]
    fn commit_raw_moves_the_digit_buffer_into_the_result() {
        let fixture = fixture();
        let mut session = open(&fixture.paths, false, EnglishInputOptions::default());
        type_digits(&mut session, "64426");

        let (result, allocations) =
            crate::ime::personal_rerank::allocations::count(|| session.command(Command::CommitRaw));
        assert_eq!(result.commit.as_deref(), Some("64426"));
        assert!(!session.active());
        assert_eq!(
            allocations, 0,
            "九键原样上屏复制了数字缓冲：{allocations} 次分配"
        );
    }

    #[test]
    fn switching_grids_ends_the_composition() {
        let fixture = fixture_with(FOURTEEN_FIXTURE);
        let mut session = open(&fixture.paths, false, EnglishInputOptions::default());
        type_digits(&mut session, "64426");
        assert!(!session.sentence_seeds.is_empty());
        session.set_grid(KeyGrid::FourteenKey);
        assert!(!session.active());
        assert!(session.sentence_seeds.is_empty());
        assert!(!session.character(b'6').handled);
        type_digits(&mut session, "bugao");
        assert_eq!(words(&session).first().map(String::as_str), Some("你好"));
    }

    /// 原样上屏：九键上屏键入的数字；14 键的组码不是用户想写的字母（按「你好」的键是 `bugao`），改为结束组字取首选，锁定的音节照样算数；没有候选时只剩组码。
    #[test]
    fn fourteen_key_commit_raw_finishes_instead_of_writing_the_codes() {
        let fixture = fixture_with(FOURTEEN_FIXTURE);
        let mut nine = open(&fixture.paths, false, EnglishInputOptions::default());
        type_digits(&mut nine, "64426");
        assert_eq!(
            nine.command(Command::CommitRaw).commit.as_deref(),
            Some("64426")
        );

        let mut session = fourteen_key_session(&fixture.paths, EnglishInputOptions::default());
        type_digits(&mut session, "bugao");
        assert_eq!(
            session.command(Command::CommitRaw).commit.as_deref(),
            Some("你好")
        );
        assert!(!session.active());
        type_digits(&mut session, "bugao");
        assert_eq!(session.finish(999).commit.as_deref(), Some("你好"));
        assert!(!session.active());

        type_digits(&mut session, "bu");
        let bu = session
            .snapshot()
            .nine_key_spellings
            .iter()
            .position(|spelling| spelling == "bu")
            .unwrap();
        assert!(session.choose_spelling(bu).handled);
        type_digits(&mut session, "gao");
        assert_eq!(
            session.command(Command::CommitRaw).commit.as_deref(),
            Some("不好")
        );

        type_digits(&mut session, "qqq");
        assert!(session.snapshot().candidates.is_empty());
        assert_eq!(
            session.command(Command::CommitRaw).commit.as_deref(),
            Some("qqq")
        );
        assert!(!session.active());
    }

    /// 九键双拼的测试词库：米旮旯 3 下 你好 是 64 47，中国 是 81 46，安 和 班 都是 25，路 是 50（u 在 0 键），走过 的 z 在 9 键、中国 的 zh 在 8 键。
    const SHUANGPIN_FIXTURE: &str = "CREATE TABLE tbl_1_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_1_n VALUES('ni','n','你',100);CREATE TABLE tbl_1_h(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_1_h VALUES('hao','h','好',100);CREATE TABLE tbl_1_z(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_1_z VALUES('zhong','z','中',300);CREATE TABLE tbl_1_g(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_1_g VALUES('guo','g','国',200);CREATE TABLE tbl_1_a(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_1_a VALUES('an','a','安',150);CREATE TABLE tbl_1_b(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_1_b VALUES('ban','b','班',120);CREATE TABLE tbl_1_l(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_1_l VALUES('lu','l','路',90);CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_2_n VALUES('ni''hao','nh','你好',1000);CREATE TABLE tbl_2_z(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_2_z VALUES('zhong''guo','zg','中国',900),('zou''guo','zg','走过',950);";

    #[test]
    fn eighteen_key_shuangpin_locks_readings_and_finishes_real_pinyin() {
        let fixture = fixture_with(SHUANGPIN_FIXTURE);
        let mut session = open(&fixture.paths, false, mixed());
        let table = crate::nine_key_shuangpin::resolve_eighteen_key_shuangpin(
            crate::ShuangpinProfileKind::Xiaohe,
            None,
        )
        .unwrap();
        session.set_grid(KeyGrid::EighteenKeyShuangpin(table));
        type_digits(&mut session, "bi");
        let choices = session.snapshot().nine_key_spellings;
        assert!(choices.iter().any(|value| value == "ni"), "{choices:?}");
        assert!(choices.iter().any(|value| value == "bi"), "{choices:?}");
        assert!(
            session
                .choose_spelling(spelling_index(&session, "ni"))
                .handled
        );
        assert!(session.command(Command::Backspace).handled);
        assert_eq!(session.snapshot().editing_text, "bi");
        assert!(
            session
                .choose_spelling(spelling_index(&session, "ni"))
                .handled
        );
        type_digits(&mut session, "hx");
        assert!(words(&session).contains(&"你好".to_owned()));
        assert_eq!(session.snapshot().nine_key_reading, "ni'hao");
        assert_eq!(session.snapshot().scheme, SchemeType::Shuangpin);
        assert_eq!(
            session.command(Command::CommitRaw).commit.as_deref(),
            Some("你好")
        );
        assert!(!session.active());
    }

    #[test]
    fn fourteen_key_shuangpin_locks_readings_and_finishes_real_pinyin() {
        let fixture = fixture_with(SHUANGPIN_FIXTURE);
        let mut session = open(&fixture.paths, false, mixed());
        let table = crate::nine_key_shuangpin::resolve_fourteen_key_shuangpin(
            crate::ShuangpinProfileKind::Xiaohe,
            None,
        )
        .unwrap();
        session.set_grid(KeyGrid::FourteenKeyShuangpin(table));
        type_digits(&mut session, "bu");
        let choices = session.snapshot().nine_key_spellings;
        assert!(choices.iter().any(|value| value == "ni"), "{choices:?}");
        assert!(choices.iter().any(|value| value == "bu"), "{choices:?}");
        let selected = spelling_index(&session, "ni");
        assert!(session.choose_spelling(selected).handled);
        assert!(session.command(Command::Backspace).handled);
        assert_eq!(session.snapshot().editing_text, "bu");
        assert!(
            session
                .choose_spelling(spelling_index(&session, "ni"))
                .handled
        );
        type_digits(&mut session, "gc");
        assert!(words(&session).contains(&"你好".to_owned()));
        assert_eq!(session.snapshot().nine_key_reading, "ni'hao");
        assert_eq!(
            session.command(Command::CommitRaw).commit.as_deref(),
            Some("你好")
        );
        assert!(!session.active());
    }

    #[test]
    fn fourteen_key_shuangpin_queries_semicolon_and_alternate_codes() {
        let fixture = fixture_with("CREATE TABLE tbl_1_q(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_1_q VALUES('qing','q','青',100);CREATE TABLE tbl_1_j(key TEXT,jp TEXT,value TEXT,weight INTEGER);INSERT INTO tbl_1_j VALUES('ju','j','居',100);");
        let mut session = open(&fixture.paths, false, mixed());
        let table = crate::nine_key_shuangpin::resolve_fourteen_key_shuangpin(
            crate::ShuangpinProfileKind::Microsoft,
            None,
        )
        .unwrap();
        session.set_grid(KeyGrid::FourteenKeyShuangpin(table));
        for (code, word) in [("ql", "青"), ("ju", "居"), ("jt", "居")] {
            type_digits(&mut session, code);
            assert!(
                words(&session).contains(&word.to_owned()),
                "{code}: {:?}",
                words(&session)
            );
            assert_eq!(
                session.command(Command::CommitRaw).commit.as_deref(),
                Some(word)
            );
        }
    }

    fn shuangpin_session(paths: &RuntimePaths, kind: NineKeyShuangpinKind) -> NineKeySession {
        let mut session = open(paths, false, mixed());
        let table = crate::nine_key_shuangpin::resolve(kind, None).unwrap();
        session.set_grid(KeyGrid::NineKeyShuangpin(table));
        session
    }

    #[test]
    fn nine_key_shuangpin_types_two_digits_per_syllable() {
        let fixture = fixture_with(SHUANGPIN_FIXTURE);
        let mut session = shuangpin_session(&fixture.paths, NineKeyShuangpinKind::Migala3);
        type_digits(&mut session, "6447");
        let view = session.snapshot();
        assert_eq!(view.editing_text, "6447");
        assert_eq!(words(&session).first().map(String::as_str), Some("你好"));
        assert_eq!(view.nine_key_reading, "ni'hao");
        assert_eq!(view.candidates[0].pinyin, "6447");
        // 混入英文开着也没有英文行：九键双拼的数字不是字母的分组。
        assert!(view
            .candidate_sources
            .iter()
            .all(|source| *source != CandidateSource::EnglishDictionary));
        assert_eq!(session.select(0).commit.as_deref(), Some("你好"));
        assert!(!session.active());

        // 只打了下一个音节的声母那一位：中国 是补全，读音行的 g 是声母。
        type_digits(&mut session, "814");
        assert_eq!(words(&session).first().map(String::as_str), Some("中国"));
        assert_eq!(session.snapshot().nine_key_reading, "zhong'g");
        session.command(Command::Cancel);

        // u 在 0 键：0 也是组码。
        assert!(session.character(b'5').handled);
        assert!(session.character(b'0').handled);
        assert_eq!(words(&session).first().map(String::as_str), Some("路"));
        session.command(Command::Cancel);
        // 米旮旯 2 不用 0 键，0 不是组码。
        let mut migala2 = shuangpin_session(&fixture.paths, NineKeyShuangpinKind::Migala2);
        assert!(!migala2.character(b'0').handled);
        assert!(!migala2.active());
    }

    #[test]
    fn nine_key_shuangpin_shares_codes_and_locks_one_reading() {
        let fixture = fixture_with(SHUANGPIN_FIXTURE);
        let mut session = shuangpin_session(&fixture.paths, NineKeyShuangpinKind::Migala3);
        // 零声母的 安 和 声母加韵母的 班 都是 25。
        type_digits(&mut session, "25");
        let found = words(&session);
        assert!(found.contains(&"安".to_owned()), "{found:?}");
        assert!(found.contains(&"班".to_owned()), "{found:?}");
        let spellings = session.snapshot().nine_key_spellings;
        for spelling in ["an", "ban", "ang", "can"] {
            assert!(spellings.contains(&spelling.to_owned()), "{spellings:?}");
        }
        // 锁定 ban：数字不变，读音只剩 班。
        assert!(
            session
                .choose_spelling(spelling_index(&session, "ban"))
                .handled
        );
        assert_eq!(session.snapshot().preedit, "ban");
        assert_eq!(session.snapshot().editing_text, "25");
        assert_eq!(words(&session), ["班"]);
        // 数字全部锁定时退格撤销锁定。
        assert!(session.command(Command::Backspace).handled);
        assert_eq!(session.snapshot().preedit, "25");
        assert!(session.locked.is_empty());
        assert!(session.command(Command::Backspace).handled);
        assert_eq!(session.snapshot().preedit, "2");

        // 锁定的音节后面还有数字：锁定只占两位，后面的数字照常解。
        session.command(Command::Cancel);
        type_digits(&mut session, "8146");
        assert!(
            session
                .choose_spelling(spelling_index(&session, "zhong"))
                .handled
        );
        assert_eq!(session.snapshot().preedit, "zhong'46");
        assert_eq!(words(&session).first().map(String::as_str), Some("中国"));
        assert!(session.command(Command::Backspace).handled);
        assert_eq!(session.snapshot().preedit, "zhong'4");
        assert_eq!(session.locked, ["zhong"]);
        // 部分上屏：选 中 吃掉两位，剩下的 4 接着组字。
        session.command(Command::Cancel);
        type_digits(&mut session, "8146");
        let zhong = index_of(&session, "中");
        assert_eq!(session.select(zhong).commit.as_deref(), Some("中"));
        assert_eq!(session.snapshot().editing_text, "46");
        assert_eq!(words(&session).first().map(String::as_str), Some("国"));
    }

    #[test]
    fn nine_key_shuangpin_initials_check_the_key_of_each_initial() {
        let fixture = fixture_with(SHUANGPIN_FIXTURE);
        let mut session = shuangpin_session(&fixture.paths, NineKeyShuangpinKind::Migala3);
        // 8 4 当两个声母：zh 在 8，z 在 9，所以 中国 是简拼，走过 不是，哪怕它更重。
        type_digits(&mut session, "84");
        let found = words(&session);
        assert!(found.contains(&"中国".to_owned()), "{found:?}");
        assert!(!found.contains(&"走过".to_owned()), "{found:?}");
        assert_eq!(session.snapshot().nine_key_reading, "z'g");
        session.command(Command::Cancel);
        type_digits(&mut session, "94");
        let found = words(&session);
        assert!(found.contains(&"走过".to_owned()), "{found:?}");
        assert!(!found.contains(&"中国".to_owned()), "{found:?}");
    }

    #[test]
    fn nine_key_shuangpin_splits_and_letters_follow_the_table() {
        let fixture = fixture_with(SHUANGPIN_FIXTURE);
        let mut session = shuangpin_session(&fixture.paths, NineKeyShuangpinKind::Migala3);
        // 切开 8'4：两个声母，不是一个音节。
        assert!(session.character(b'8').handled);
        assert!(session.character(b'\'').handled);
        assert!(session.character(b'4').handled);
        assert_eq!(session.snapshot().preedit, "8'4");
        assert_eq!(words(&session).first().map(String::as_str), Some("中国"));
        session.command(Command::Cancel);
        // 左列的字母是这一位上的声母和零声母音节的首字母；数字本身仍可原样上屏。
        type_digits(&mut session, "2");
        let spellings = session.snapshot().nine_key_spellings;
        for letter in ["A", "B", "C", "E", "O", "2"] {
            assert!(spellings.contains(&letter.to_owned()), "{spellings:?}");
        }
        assert!(
            session
                .choose_spelling(spelling_index(&session, "A"))
                .handled
        );
        assert_eq!(session.snapshot().preedit, "a");
        assert_eq!(words(&session), ["安"]);
        // 原样上屏写的是数字。
        assert_eq!(
            session.command(Command::CommitRaw).commit.as_deref(),
            Some("2")
        );
    }

    #[test]
    fn nine_key_shuangpin_does_not_change_the_nine_key_grid() {
        let fixture = fixture_with(SHUANGPIN_FIXTURE);
        let mut session = open(&fixture.paths, false, mixed());
        type_digits(&mut session, "64426");
        assert_eq!(words(&session).first().map(String::as_str), Some("你好"));
        // 米旮旯 2 的 ao 在 2 键：你好 是 64 42。
        let mut shuangpin = shuangpin_session(&fixture.paths, NineKeyShuangpinKind::Migala2);
        type_digits(&mut shuangpin, "6442");
        assert_eq!(words(&shuangpin).first().map(String::as_str), Some("你好"));
        assert!(KeyGrid::NineKey.spelling_table().longest_code > 2);
        assert_ne!(KeyGrid::NineKey, shuangpin.grid);
    }

    #[test]
    fn nine_key_shuangpin_words_are_found_by_their_initial_keys() {
        let table =
            crate::nine_key_shuangpin::resolve(NineKeyShuangpinKind::Migala3, None).unwrap();
        let grid = KeyGrid::NineKeyShuangpin(table);
        // 两个音节不查；三个音节按 8、4、4 三位展开：zh 和 t、s 都在 8 键上，g、h、r 在 4 键上。
        assert_eq!(shuangpin_word_codes(grid, "8146"), None);
        let codes = shuangpin_word_codes(grid, "814643").unwrap();
        assert!(codes.contains(&"zgr".to_owned()));
        assert!(codes.contains(&"tgh".to_owned()));
        assert!(!codes.iter().any(|code| code.starts_with('x')));
        // 中国人 是 81 46 43；张国荣 的 zhang 是 85，声母那几位相同也不算。
        assert!(shuangpin_word_spells(table, "zhong'guo'ren", "814643"));
        assert!(!shuangpin_word_spells(table, "zhang'guo'rong", "814643"));
        // 末尾只打了声母那一位：三个音节的词照样算，四个音节的不算。
        assert!(shuangpin_word_spells(table, "zhong'guo'ren", "81464"));
        assert!(!shuangpin_word_spells(table, "zhong'guo'ren'min", "81464"));
        assert!(!shuangpin_word_spells(table, "zhong'guo", "81464"));
    }
}

#[cfg(test)]
#[path = "nine_key/english_page_append_tests.rs"]
mod english_page_append_tests;
