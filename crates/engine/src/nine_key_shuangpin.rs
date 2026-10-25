//! 九键双拼（#6572）：每个音节固定两个数字，第一位是声母所在的键，第二位是韵母所在的键；零声母音节按方案给的两位编码；只打了声母的一位单独算一位，后面的韵母还没打。一个两位编码可以对应好几个音节，但每个音节只有一个编码。
//!
//! 这里只管方案本身：内置方案的表、用户表的校验、音节的编码和键面提示。解码复用九宫格：方案经 [`crate::KeyGrid::NineKeyShuangpin`] 交给 `nine_key::NineKeySession`，它照这里给的编码建拼写表，路径搜索、锁定、切分、整句、学习和候选排序与全拼九宫格是同一套。
//!
//! 合法的表按内容只留一份，与自定义 26 键双拼（`shuangpin::custom`）相同：同一张表反复校验、反复应用拿到的是同一个引用，进程里只为实际用过的每一张不同的表各留一份（含它的拼写表，几十 KB）；不合法的表不留任何东西。

use std::collections::HashMap;
use std::fmt;
use std::sync::{Mutex, MutexGuard, OnceLock, PoisonError};

use crate::nine_key::SpellingTable;
use crate::pinyin::syllables::{canonical_lattice_syllable, intact_pinyin_list};
use crate::shuangpin::custom::{CustomTablePart, FINALS, ZERO_INITIALS};
use crate::types::{NineKeyShuangpinKind, NineKeyShuangpinTable};

/// 九键双拼的表要给出数字的 23 个声母：九键上每个声母都要放到某个数字上，不像 26 键那样单字母声母固定在自己的字母键上。
pub const INITIALS: [&str; 23] = [
    "b", "p", "m", "f", "d", "t", "n", "l", "g", "k", "h", "j", "q", "x", "r", "z", "c", "s", "y",
    "w", "zh", "ch", "sh",
];

/// 九键数字在键面上的顺序：电话键盘的 1 到 9，再是 0。
pub const DIGIT_ORDER: &[u8; 10] = b"1234567890";

// 内置方案的表。数字按公开描述独立整理，来源和取舍见 `.agents/notes/implemented/feature/2026-10-10-nine-key-shuangpin.md`；单韵母 a e i o u ü 没有单独列出的，按这几个方案共同的约定留在电话键盘上自己字母所在的键（a 2、e 3、i 4、o 6、u 和 ü 8）。

/// 米旮旯 2：ch 在 5，sh、zh 在 8，零声母在 6；其余声母在电话键盘上自己字母所在的键。
const MIGALA_2: BuiltinTable = BuiltinTable {
    initials: [
        ("b", "2"),
        ("p", "7"),
        ("m", "6"),
        ("f", "3"),
        ("d", "3"),
        ("t", "8"),
        ("n", "6"),
        ("l", "5"),
        ("g", "4"),
        ("k", "5"),
        ("h", "4"),
        ("j", "5"),
        ("q", "7"),
        ("x", "9"),
        ("r", "7"),
        ("z", "9"),
        ("c", "2"),
        ("s", "7"),
        ("y", "9"),
        ("w", "9"),
        ("zh", "8"),
        ("ch", "5"),
        ("sh", "8"),
    ],
    finals: [
        ("a", "2"),
        ("o", "6"),
        ("e", "3"),
        ("i", "4"),
        ("u", "8"),
        ("v", "8"),
        ("ai", "2"),
        ("ei", "3"),
        ("ui", "9"),
        ("ao", "2"),
        ("ou", "6"),
        ("iu", "1"),
        ("ie", "1"),
        ("ve", "9"),
        ("ue", "9"),
        ("an", "2"),
        ("en", "3"),
        ("in", "1"),
        ("un", "9"),
        ("ang", "5"),
        ("eng", "5"),
        ("ing", "4"),
        ("ong", "5"),
        ("ia", "1"),
        ("ua", "9"),
        ("uo", "9"),
        ("uai", "8"),
        ("ian", "4"),
        ("uan", "8"),
        ("iao", "4"),
        ("iang", "7"),
        ("uang", "7"),
        ("iong", "7"),
    ],
    zero_initial_key: b'6',
    er_key: b'3',
};

/// 米旮旯 3：ch 在 3，r 在 4，sh、zh 在 8，零声母在 2，u、ü 和 ei 在 0 键（点讯版，也是现在搜狗的版本；百度早年不支持 0 键时把 ei 挪到 8、u 和 ü 留在 8，那一版不收录）。
const MIGALA_3: BuiltinTable = BuiltinTable {
    initials: [
        ("b", "2"),
        ("p", "7"),
        ("m", "6"),
        ("f", "3"),
        ("d", "3"),
        ("t", "8"),
        ("n", "6"),
        ("l", "5"),
        ("g", "4"),
        ("k", "5"),
        ("h", "4"),
        ("j", "5"),
        ("q", "7"),
        ("x", "9"),
        ("r", "4"),
        ("z", "9"),
        ("c", "2"),
        ("s", "7"),
        ("y", "9"),
        ("w", "9"),
        ("zh", "8"),
        ("ch", "3"),
        ("sh", "8"),
    ],
    finals: [
        ("a", "2"),
        ("o", "6"),
        ("e", "3"),
        ("i", "4"),
        ("u", "0"),
        ("v", "0"),
        ("ai", "2"),
        ("ei", "0"),
        ("ui", "6"),
        ("ao", "7"),
        ("ou", "1"),
        ("iu", "8"),
        ("ie", "4"),
        ("ve", "3"),
        ("ue", "3"),
        ("an", "5"),
        ("en", "3"),
        ("in", "6"),
        ("un", "8"),
        ("ang", "5"),
        ("eng", "8"),
        ("ing", "1"),
        ("ong", "1"),
        ("ia", "4"),
        ("ua", "9"),
        ("uo", "6"),
        ("uai", "9"),
        ("ian", "9"),
        ("uan", "7"),
        ("iao", "9"),
        ("iang", "2"),
        ("uang", "4"),
        ("iong", "6"),
    ],
    zero_initial_key: b'2',
    er_key: b'3',
};

/// 辜氏 2 低重码：ch 在 3，sh、zh 在 8，零声母在 2，in、ing、eng 在 0 键，1 键放 ou、ong、iong。
const GUSHI_2_LOW: BuiltinTable = BuiltinTable {
    initials: [
        ("b", "2"),
        ("p", "7"),
        ("m", "6"),
        ("f", "3"),
        ("d", "3"),
        ("t", "8"),
        ("n", "6"),
        ("l", "5"),
        ("g", "4"),
        ("k", "5"),
        ("h", "4"),
        ("j", "5"),
        ("q", "7"),
        ("x", "9"),
        ("r", "7"),
        ("z", "9"),
        ("c", "2"),
        ("s", "7"),
        ("y", "9"),
        ("w", "9"),
        ("zh", "8"),
        ("ch", "3"),
        ("sh", "8"),
    ],
    finals: [
        ("a", "2"),
        ("o", "6"),
        ("e", "3"),
        ("i", "4"),
        ("u", "8"),
        ("v", "8"),
        ("ai", "2"),
        ("ei", "3"),
        ("ui", "8"),
        ("ao", "7"),
        ("ou", "1"),
        ("iu", "4"),
        ("ie", "4"),
        ("ve", "8"),
        ("ue", "8"),
        ("an", "5"),
        ("en", "3"),
        ("in", "0"),
        ("un", "6"),
        ("ang", "5"),
        ("eng", "0"),
        ("ing", "0"),
        ("ong", "1"),
        ("ia", "4"),
        ("ua", "9"),
        ("uo", "6"),
        ("uai", "9"),
        ("ian", "9"),
        ("uan", "7"),
        ("iao", "9"),
        ("iang", "9"),
        ("uang", "7"),
        ("iong", "1"),
    ],
    zero_initial_key: b'2',
    er_key: b'3',
};

/// 一个内置方案：声母和韵母各一个数字，零声母音节都是「零声母键 + 韵母键」。
struct BuiltinTable {
    initials: [(&'static str, &'static str); 23],
    finals: [(&'static str, &'static str); 33],
    /// 零声母所在的键，零声母音节的第一位。
    zero_initial_key: u8,
    /// `er` 所在的键。`er` 只作零声母音节出现，不在韵母表里，三个方案的描述都把它和别的韵母一起列在某个键上。
    er_key: u8,
}

impl BuiltinTable {
    /// 零声母音节的两位编码：零声母键，再是它的韵母所在的键（`er` 是 `er_key`）。
    fn zero_initial_code(&self, syllable: &str) -> String {
        let final_key = if syllable == "er" {
            char::from(self.er_key)
        } else {
            self.finals
                .iter()
                .find(|(final_unit, _)| *final_unit == syllable)
                .and_then(|(_, key)| key.chars().next())
                .unwrap_or('?')
        };
        format!("{}{final_key}", char::from(self.zero_initial_key))
    }

    fn table(&self) -> NineKeyShuangpinTable {
        let owned = |pairs: &[(&str, &str)]| -> Vec<(String, String)> {
            pairs
                .iter()
                .map(|(unit, key)| ((*unit).to_owned(), (*key).to_owned()))
                .collect()
        };
        NineKeyShuangpinTable {
            initials: owned(&self.initials),
            finals: owned(&self.finals),
            zero_initials: ZERO_INITIALS
                .iter()
                .map(|syllable| ((*syllable).to_owned(), self.zero_initial_code(syllable)))
                .collect(),
        }
    }
}

fn builtin_source(kind: NineKeyShuangpinKind) -> Option<&'static BuiltinTable> {
    match kind {
        NineKeyShuangpinKind::Migala2 => Some(&MIGALA_2),
        NineKeyShuangpinKind::Migala3 => Some(&MIGALA_3),
        NineKeyShuangpinKind::Gushi2Low => Some(&GUSHI_2_LOW),
        NineKeyShuangpinKind::Custom => None,
    }
}

/// 内置方案写成用户表的样子，作为编辑自定义九键双拼的起点；`Custom` 没有内置的表，返回 `None`。
pub fn builtin_table(kind: NineKeyShuangpinKind) -> Option<NineKeyShuangpinTable> {
    builtin_source(kind).map(BuiltinTable::table)
}

/// 表为什么不能用，带出错的位置。宿主可以按种类给出自己的提示文字，`Display` 是给日志和代理看的英文。
///
/// 没有「编码冲突」这一类：九键上一个两位编码本来就对应好几个音节，零声母音节和声母加韵母的音节共用编码（米旮旯 3 的「安」和「班」都是 25）也是方案的一部分，不会让哪个音节打不出来；每个音节恰好一个编码由「每个单位恰好一个键」保证。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NineKeyShuangpinError {
    /// 这一部分不认识的单位，例如 `finals` 里写了 `iou`。
    UnknownUnit { part: CustomTablePart, unit: String },
    /// 同一个单位写了两次。
    DuplicateUnit { part: CustomTablePart, unit: String },
    /// 缺了这个单位的键位。
    MissingUnit {
        part: CustomTablePart,
        unit: &'static str,
    },
    /// 键不是这里允许的：声母和韵母是一个数字，零声母是两个数字，数字是 `0`–`9`。
    InvalidKey {
        part: CustomTablePart,
        unit: String,
        key: String,
    },
}

impl fmt::Display for NineKeyShuangpinError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::UnknownUnit { part, unit } => {
                write!(formatter, "unknown {} unit {unit:?}", part.name())
            }
            Self::DuplicateUnit { part, unit } => {
                write!(formatter, "{} unit {unit:?} appears twice", part.name())
            }
            Self::MissingUnit { part, unit } => {
                write!(formatter, "{} unit {unit:?} has no key", part.name())
            }
            Self::InvalidKey { part, unit, key } => write!(
                formatter,
                "{} unit {unit:?} has an invalid key {key:?}",
                part.name()
            ),
        }
    }
}

impl std::error::Error for NineKeyShuangpinError {}

/// 一个音节怎么拆：声母加韵母，或者零声母。
enum Parts {
    Consonant {
        initial: &'static str,
        final_unit: &'static str,
    },
    Zero,
}

/// 按拼写拆音节：先认 `zh` `ch` `sh`，再认单字母声母，都不是的是零声母音节。`ju` 拆成 j 加 `u`、`lve` 拆成 l 加 `ve`，与拼写一致：一个音节只有一种拆法，所以只有一个编码。
fn parts(syllable: &'static str) -> Parts {
    for initial in ["zh", "ch", "sh"] {
        if let Some(final_unit) = syllable.strip_prefix(initial) {
            return Parts::Consonant {
                initial,
                final_unit,
            };
        }
    }
    let initial = INITIALS
        .iter()
        .take(20)
        .find(|initial| syllable.as_bytes().first() == initial.as_bytes().first());
    match initial {
        Some(initial) => Parts::Consonant {
            initial,
            final_unit: &syllable[1..],
        },
        None => Parts::Zero,
    }
}

/// 九键双拼能打的音节：九宫格的完整音节表去掉 `jv`、`lue` 这类只是别的拼写的别名（词库只存 `ju`、`lve`），再加上零声母表里的 `eng`。按音节表的顺序。
pub(crate) fn syllables() -> &'static [&'static str] {
    static SYLLABLES: OnceLock<Vec<&'static str>> = OnceLock::new();
    SYLLABLES.get_or_init(|| {
        let mut syllables: Vec<&'static str> = intact_pinyin_list()
            .iter()
            .copied()
            .filter(|syllable| canonical_lattice_syllable(syllable) == *syllable)
            .collect();
        for syllable in ZERO_INITIALS {
            if !syllables.contains(&syllable) {
                syllables.push(syllable);
            }
        }
        syllables
    })
}

/// 规范化后的一张表：单位换成常量里的那个 `&'static str`，键换成数字字节，按常量的顺序排好。
#[derive(PartialEq, Eq)]
struct Canonical {
    initials: Vec<(&'static str, u8)>,
    finals: Vec<(&'static str, u8)>,
    zero_initials: Vec<(&'static str, [u8; 2])>,
}

fn canonicalize(table: &NineKeyShuangpinTable) -> Result<Canonical, NineKeyShuangpinError> {
    let single = |key: &[u8]| match key {
        [digit] if digit.is_ascii_digit() => Some(*digit),
        _ => None,
    };
    let pair = |key: &[u8]| match key {
        [first, second] if first.is_ascii_digit() && second.is_ascii_digit() => {
            Some([*first, *second])
        }
        _ => None,
    };
    Ok(Canonical {
        initials: canonical_part(
            CustomTablePart::Initials,
            &table.initials,
            &INITIALS,
            single,
        )?,
        finals: canonical_part(CustomTablePart::Finals, &table.finals, &FINALS, single)?,
        zero_initials: canonical_part(
            CustomTablePart::ZeroInitials,
            &table.zero_initials,
            &ZERO_INITIALS,
            pair,
        )?,
    })
}

fn canonical_part<K: Copy>(
    part: CustomTablePart,
    entries: &[(String, String)],
    units: &[&'static str],
    parse: impl Fn(&[u8]) -> Option<K>,
) -> Result<Vec<(&'static str, K)>, NineKeyShuangpinError> {
    let mut keys: Vec<Option<K>> = vec![None; units.len()];
    for (unit, key) in entries {
        let Some(index) = units.iter().position(|known| known == unit) else {
            return Err(NineKeyShuangpinError::UnknownUnit {
                part,
                unit: unit.clone(),
            });
        };
        if keys[index].is_some() {
            return Err(NineKeyShuangpinError::DuplicateUnit {
                part,
                unit: unit.clone(),
            });
        }
        let Some(parsed) = parse(key.as_bytes()) else {
            return Err(NineKeyShuangpinError::InvalidKey {
                part,
                unit: unit.clone(),
                key: key.clone(),
            });
        };
        keys[index] = Some(parsed);
    }
    units
        .iter()
        .zip(keys)
        .map(|(unit, key)| {
            key.map(|key| (*unit, key))
                .ok_or(NineKeyShuangpinError::MissingUnit { part, unit })
        })
        .collect()
}

/// 一张两键双拼表，进程里按内容只留一份，以 `&'static` 交给九宫格；九键用户表使用数字，14 键表从已解析的字母方案派生。
pub struct NineKeyShuangpin {
    canonical: Canonical,
    /// 每个能打的音节的两位编码。
    codes: HashMap<&'static str, [u8; 2]>,
    /// 14 键保留原方案接受的其他编码，例如微软双拼把 j+ü 也解作 ju。九键用户表每个音节只有一种编码，此表为空。
    aliases: HashMap<&'static str, Vec<[u8; 2]>>,
    /// 每个组码键上能起头一个音节的字母，按字母序：声母的首字母（`zh` 算 `z`），以及编码从这个键开始的零声母音节的首字母。槽位由 `key_slot` 确定。
    letters: [String; 62],
    /// 九宫格的拼写表，第一次在九宫格里用这张表时建（`KeyGrid::spelling_table`）。
    pub(crate) spelling: OnceLock<SpellingTable>,
}

impl fmt::Debug for NineKeyShuangpin {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("NineKeyShuangpin")
            .field("initials", &self.canonical.initials)
            .field("finals", &self.canonical.finals)
            .field("zero_initials", &self.canonical.zero_initials)
            .finish_non_exhaustive()
    }
}

fn key_slot(key: u8) -> Option<usize> {
    match key {
        b'0'..=b'9' => Some(usize::from(key - b'0')),
        b'a'..=b'z' => Some(10 + usize::from(key - b'a')),
        b'A'..=b'Z' => Some(36 + usize::from(key - b'A')),
        _ => None,
    }
}

impl NineKeyShuangpin {
    fn build(canonical: Canonical) -> Self {
        let initial_key = |unit: &str| {
            canonical
                .initials
                .iter()
                .find(|(known, _)| *known == unit)
                .map(|(_, key)| *key)
        };
        let final_key = |unit: &str| {
            canonical
                .finals
                .iter()
                .find(|(known, _)| *known == unit)
                .map(|(_, key)| *key)
        };
        let mut codes = HashMap::with_capacity(syllables().len());
        for &syllable in syllables() {
            let code = match parts(syllable) {
                Parts::Consonant {
                    initial,
                    final_unit,
                } => match (initial_key(initial), final_key(final_unit)) {
                    (Some(first), Some(second)) => [first, second],
                    // 每个音节都拆得开（`every_syllable_has_one_code` 核对），走不到这里。
                    _ => continue,
                },
                Parts::Zero => match canonical
                    .zero_initials
                    .iter()
                    .find(|(known, _)| *known == syllable)
                {
                    Some((_, code)) => *code,
                    None => continue,
                },
            };
            codes.insert(syllable, code);
        }
        let mut letters: [Vec<u8>; 62] = std::array::from_fn(|_| Vec::new());
        let mut add = |digit: u8, letter: u8| {
            let Some(index) = key_slot(digit) else { return };
            let slot = &mut letters[index];
            if !slot.contains(&letter) {
                slot.push(letter);
            }
        };
        for (initial, key) in &canonical.initials {
            add(*key, initial.as_bytes()[0]);
        }
        for (syllable, code) in &canonical.zero_initials {
            add(code[0], syllable.as_bytes()[0]);
        }
        let letters = letters.map(|mut slot| {
            slot.sort_unstable();
            String::from_utf8(slot).unwrap_or_default()
        });
        Self {
            canonical,
            codes,
            aliases: HashMap::new(),
            letters,
            spelling: OnceLock::new(),
        }
    }

    /// 完整音节的两位编码；不是能打的音节时为 `None`。
    pub fn syllable_code(&self, syllable: &str) -> Option<&[u8; 2]> {
        self.codes.get(syllable)
    }

    pub(crate) fn syllable_codes(&self, syllable: &str) -> impl Iterator<Item = &[u8; 2]> {
        self.codes
            .get(syllable)
            .into_iter()
            .chain(self.aliases.get(syllable).into_iter().flatten())
    }

    /// 声母所在的键。
    pub fn initial_key(&self, initial: &str) -> Option<u8> {
        self.canonical
            .initials
            .iter()
            .find(|(unit, _)| *unit == initial)
            .map(|(_, key)| *key)
    }

    /// 能打的音节和它的编码，按音节表的顺序。
    pub(crate) fn coded_syllables(&self) -> impl Iterator<Item = (&'static str, &[u8; 2])> {
        syllables().iter().flat_map(|&syllable| {
            self.syllable_codes(syllable)
                .map(move |code| (syllable, code))
        })
    }

    /// 只打了一位时，这一位能代表的拼写片段：声母（`zh`、`b`），以及零声母音节的首字母（编码从这个键开始的 `a`、`e`、`o`）。片段都不完整，后面还要接韵母。
    pub(crate) fn one_key_pieces(&self) -> Vec<(&'static str, u8)> {
        let mut pieces: Vec<(&'static str, u8)> = self.canonical.initials.clone();
        for (syllable, code) in &self.canonical.zero_initials {
            let letter = &syllable[..1];
            if !pieces
                .iter()
                .any(|(piece, key)| *piece == letter && *key == code[0])
            {
                pieces.push((letter, code[0]));
            }
        }
        pieces
    }

    /// 拼写片段在 `digits` 开头占几位：是完整音节、编码正好是开头两位时两位，是这一位上的单键片段（`one_key_pieces`）时一位，都不是时为 `None`。`a` 这样的片段两样都可能，按数字定。
    pub(crate) fn piece_code_len(&self, piece: &str, digits: &str) -> Option<usize> {
        let digits = digits.as_bytes();
        if digits.len() >= 2
            && self
                .syllable_codes(piece)
                .any(|code| code[..] == digits[..2])
        {
            return Some(2);
        }
        let first = *digits.first()?;
        let one_key = self
            .canonical
            .initials
            .iter()
            .any(|(unit, key)| *unit == piece && *key == first)
            || self
                .canonical
                .zero_initials
                .iter()
                .any(|(syllable, code)| code[0] == first && syllable.get(..1) == Some(piece));
        one_key.then_some(1)
    }

    /// 只打了声母那一位时读音行显示的部分：声母（`zh`、`b`），零声母音节是首字母。
    pub fn initial_part<'a>(&self, syllable: &'a str) -> &'a str {
        let length = if ["zh", "ch", "sh"]
            .iter()
            .any(|initial| syllable.starts_with(initial))
        {
            2
        } else {
            1
        };
        syllable.get(..length).unwrap_or(syllable)
    }

    /// 组码键上能起头一个音节的字母，见 `letters` 字段；不是组码或这个键上没有时为空。
    pub fn letters(&self, digit: u8) -> &str {
        key_slot(digit).map_or("", |index| self.letters[index].as_str())
    }

    /// 这张表用到了这个组码键：有声母、韵母或零声母编码放在它上面。
    pub fn uses_key(&self, digit: u8) -> bool {
        key_slot(digit).is_some()
            && (self.canonical.initials.iter().any(|(_, key)| *key == digit)
                || self.canonical.finals.iter().any(|(_, key)| *key == digit)
                || self
                    .canonical
                    .zero_initials
                    .iter()
                    .any(|(_, code)| code.contains(&digit)))
    }

    /// `'` 分开的拼写的数字码：完整音节两位，单独的声母一位；有一段两样都不是时整个返回空串。
    pub fn encode(&self, pinyin: &str) -> String {
        let mut code = String::with_capacity(pinyin.len());
        for piece in pinyin.split('\'') {
            if let Some(pair) = self.codes.get(piece) {
                code.push(char::from(pair[0]));
                code.push(char::from(pair[1]));
            } else if let Some(key) = self.initial_key(piece) {
                code.push(char::from(key));
            } else {
                return String::new();
            }
        }
        code
    }

    /// 核对全拼的两键编码与输入共同覆盖的前缀；逐音节核对，保留别名而不展开所有编码的笛卡尔积。
    pub(crate) fn matching_prefix_len(&self, pinyin: &str, input: &str) -> Option<usize> {
        let mut offset = 0;
        for piece in pinyin.split('\'') {
            let remaining = input.as_bytes().get(offset..).unwrap_or_default();
            if self.codes.contains_key(piece) {
                let covered = remaining.len().min(2);
                if !self
                    .syllable_codes(piece)
                    .any(|code| code[..covered] == remaining[..covered])
                {
                    return None;
                }
                offset += 2;
            } else {
                if !self.canonical.initials.iter().any(|(unit, key)| {
                    *unit == piece && remaining.first().is_none_or(|value| value == key)
                }) {
                    return None;
                }
                offset += 1;
            }
        }
        (offset > 0).then_some(offset.min(input.len()))
    }

    /// 关键词（日期时间的 `riqi`、`sj` 这类小写字母）正好拼出 `digits`：能切成完整音节时按音节的编码比，否则每个字母当一个音节的首字母按声母的键比（简拼）。首字母 `z` `c` `s` 也可以是 `zh` `ch` `sh`（`sj` 是 shi'jian），与简拼按首字母查词相同。
    pub fn spells_keyword(&self, keyword: &str, digits: &str) -> bool {
        let pieces = crate::pinyin::segment::cut_one_piece_min_segments(keyword, true);
        if !pieces.is_empty()
            && pieces.len() * 2 == digits.len()
            && self.matching_prefix_len(&pieces.join("'"), digits) == Some(digits.len())
        {
            return true;
        }
        keyword.len() == digits.len()
            && keyword.bytes().zip(digits.bytes()).all(|(letter, digit)| {
                self.canonical
                    .initials
                    .iter()
                    .any(|(initial, key)| *key == digit && initial.as_bytes()[0] == letter)
            })
    }

    /// 简拼行（一位数字一个音节）的每个音节都以对应那一位上的声母开头。九宫格的简拼按首字母查词库，`z` 同时查出 `z` 和 `zh` 开头的词，而九键双拼里它们常在不同的键上。
    pub fn spells_initials(&self, canonical_pinyin: &str, digits: &str) -> bool {
        let mut count = 0;
        for (syllable, &digit) in canonical_pinyin.split('\'').zip(digits.as_bytes()) {
            count += 1;
            if !self.syllable_codes(syllable).any(|code| code[0] == digit) {
                return false;
            }
        }
        count == digits.len()
    }

    /// 键面提示：按电话键盘的 1 到 9、0 列出每个用到的键上的声母和韵母，以及零声母编码是否从这个键开始。声母按电话键盘的习惯排：单字母声母按字母序在前，`ch` `sh` `zh` 在后（3 键是 d f ch）；韵母按韵母表的顺序。`ü` 开头的韵母写成 `ü`，与 26 键的提示相同。
    pub fn key_hints(&self) -> Vec<NineKeyShuangpinKeyHint> {
        let umlaut = |unit: &str| match unit.strip_prefix('v') {
            Some(rest) => format!("ü{rest}"),
            None => unit.to_owned(),
        };
        DIGIT_ORDER
            .iter()
            .filter(|digit| self.uses_key(**digit))
            .map(|&digit| NineKeyShuangpinKeyHint {
                key: char::from(digit),
                initials: {
                    let mut initials: Vec<&str> = self
                        .canonical
                        .initials
                        .iter()
                        .filter(|(_, key)| *key == digit)
                        .map(|(unit, _)| *unit)
                        .collect();
                    initials.sort_by_key(|unit| (unit.len(), *unit));
                    initials.into_iter().map(str::to_owned).collect()
                },
                finals: self
                    .canonical
                    .finals
                    .iter()
                    .filter(|(_, key)| *key == digit)
                    .map(|(unit, _)| umlaut(unit))
                    .collect(),
                zero_initial: self
                    .canonical
                    .zero_initials
                    .iter()
                    .any(|(_, code)| code[0] == digit),
            })
            .collect()
    }
}

/// 一个数字键的键面提示，见 [`NineKeyShuangpin::key_hints`]。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NineKeyShuangpinKeyHint {
    pub key: char,
    pub initials: Vec<String>,
    pub finals: Vec<String>,
    pub zero_initial: bool,
}

/// 进程里按内容只留一份的合法表。
fn interned() -> MutexGuard<'static, Vec<&'static NineKeyShuangpin>> {
    static INTERNED: OnceLock<Mutex<Vec<&'static NineKeyShuangpin>>> = OnceLock::new();
    INTERNED
        .get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
}

/// 校验 `table`，合法时返回它对应的方案。同一张表（单位和键相同，顺序不论）总是返回同一个引用。
pub fn nine_key_shuangpin(
    table: &NineKeyShuangpinTable,
) -> Result<&'static NineKeyShuangpin, NineKeyShuangpinError> {
    let canonical = canonicalize(table)?;
    let mut interned = interned();
    if let Some(found) = interned.iter().find(|found| found.canonical == canonical) {
        return Ok(found);
    }
    let leaked: &'static NineKeyShuangpin = Box::leak(Box::new(NineKeyShuangpin::build(canonical)));
    interned.push(leaked);
    Ok(leaked)
}

/// 将已解析的 26 键双拼方案投到字母组；分号与 L 共键，缓存同时区分方案和布局。九键用户表的数字校验不变。
fn letter_key_shuangpin(
    profile: &'static crate::shuangpin::ShuangpinProfile,
    eighteen_key: bool,
) -> Option<&'static NineKeyShuangpin> {
    type DerivedTable = (
        bool,
        &'static crate::shuangpin::ShuangpinProfile,
        &'static NineKeyShuangpin,
    );
    static DERIVED: OnceLock<Mutex<Vec<DerivedTable>>> = OnceLock::new();
    let mut derived = DERIVED
        .get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    if let Some((_, _, table)) = derived
        .iter()
        .find(|(layout, source, _)| *layout == eighteen_key && std::ptr::eq(*source, profile))
    {
        return Some(table);
    }
    let key = |value: u8| {
        if eighteen_key {
            crate::nine_key::eighteen_key_code(value)
        } else if value == b';' {
            Some(b'l')
        } else {
            crate::KeyGrid::FourteenKey.code_of(value)
        }
    };
    let lookup = |pairs: &[(&str, &str)], unit: &str| {
        pairs
            .iter()
            .find(|(name, _)| *name == unit)
            .and_then(|(_, value)| match value.as_bytes() {
                [letter] => key(*letter),
                _ => None,
            })
    };
    let mut canonical = Canonical {
        initials: INITIALS
            .iter()
            .map(|&unit| {
                let value = if unit.len() == 1 {
                    key(unit.as_bytes()[0])
                } else {
                    lookup(profile.initials, unit)
                }?;
                Some((unit, value))
            })
            .collect::<Option<Vec<_>>>()?,
        finals: FINALS
            .iter()
            .map(|&unit| Some((unit, lookup(profile.finals, unit)?)))
            .collect::<Option<Vec<_>>>()?,
        zero_initials: ZERO_INITIALS
            .iter()
            .map(|&unit| {
                let code = profile
                    .zero_initials
                    .iter()
                    .find(|(name, _)| *name == unit)?
                    .1;
                let [first, second] = code.as_bytes() else {
                    return None;
                };
                Some((unit, [key(*first)?, key(*second)?]))
            })
            .collect::<Option<Vec<_>>>()?,
    };
    if eighteen_key {
        // 原方案的单字母键用大写标记精确输入；整组编码仍排在前面，默认编码不变。
        for &unit in &INITIALS {
            let raw = if unit.len() == 1 {
                unit.as_bytes()[0]
            } else {
                profile
                    .initials
                    .iter()
                    .find(|(name, _)| *name == unit)?
                    .1
                    .as_bytes()[0]
            };
            canonical.initials.push((unit, raw.to_ascii_uppercase()));
        }
        for &unit in &FINALS {
            let raw = profile
                .finals
                .iter()
                .find(|(name, _)| *name == unit)?
                .1
                .as_bytes()[0];
            if raw.is_ascii_lowercase() {
                canonical.finals.push((unit, raw.to_ascii_uppercase()));
            }
        }
        for &(unit, raw) in profile.zero_initials {
            let [first, second] = *raw.as_bytes() else {
                return None;
            };
            let group = [key(first)?, key(second)?];
            for pair in precise_pair_variants([first, second], group) {
                if !canonical.zero_initials.contains(&(unit, pair)) {
                    canonical.zero_initials.push((unit, pair));
                }
            }
        }
    }
    let mut table = NineKeyShuangpin::build(canonical);
    table
        .codes
        .retain(|syllable, _| crate::shuangpin::profile::accepted_syllables().contains(syllable));
    for first in b'a'..=b'z' {
        for second in (b'a'..=b'z').chain(std::iter::once(b';')) {
            let raw = String::from_utf8(vec![first, second]).ok()?;
            let decoded = crate::shuangpin::utils::cvt_single_sp_to_pinyin(&raw, profile);
            let Some(&syllable) = syllables().iter().find(|&&value| value == decoded) else {
                continue;
            };
            let group = [key(first)?, key(second)?];
            let variants = if eighteen_key {
                precise_pair_variants([first, second], group).to_vec()
            } else {
                vec![group]
            };
            for code in variants {
                if table.codes.get(syllable) != Some(&code) {
                    let aliases = table.aliases.entry(syllable).or_default();
                    if !aliases.contains(&code) {
                        aliases.push(code);
                    }
                }
            }
        }
    }
    let leaked: &'static NineKeyShuangpin = Box::leak(Box::new(table));
    derived.push((eighteen_key, profile, leaked));
    Some(leaked)
}

/// 两位编码的每一位都可保留整组或限定字母；分号继续走 L 整组。
fn precise_pair_variants(raw: [u8; 2], group: [u8; 2]) -> [[u8; 2]; 4] {
    let precise = std::array::from_fn::<_, 2, _>(|index| {
        if raw[index].is_ascii_lowercase() {
            raw[index].to_ascii_uppercase()
        } else {
            group[index]
        }
    });
    [
        group,
        [precise[0], group[1]],
        [group[0], precise[1]],
        precise,
    ]
}

/// 按当前内置或自定义双拼方案派生 14 键表；自定义表沿用 26 键校验，不接受另一套字母用户表。
pub fn resolve_fourteen_key_shuangpin(
    kind: crate::ShuangpinProfileKind,
    custom: Option<&crate::ShuangpinCustomTable>,
) -> crate::Result<&'static NineKeyShuangpin> {
    let profile = crate::shuangpin::custom::session_profile(kind, custom, true)?;
    letter_key_shuangpin(profile, false)
        .ok_or_else(|| crate::EngineError::invalid("invalid fourteen-key shuangpin profile"))
}

/// 按当前内置或自定义双拼方案派生 18 键表，沿用 26 键方案校验。
pub fn resolve_eighteen_key_shuangpin(
    kind: crate::ShuangpinProfileKind,
    custom: Option<&crate::ShuangpinCustomTable>,
) -> crate::Result<&'static NineKeyShuangpin> {
    let profile = crate::shuangpin::custom::session_profile(kind, custom, true)?;
    letter_key_shuangpin(profile, true)
        .ok_or_else(|| crate::EngineError::invalid("invalid eighteen-key shuangpin profile"))
}

/// 与 [`nine_key_shuangpin`] 的结论相同，但什么也不留下，给编辑中的表用（设置页每改一个键就问一次）。
pub fn check_table(table: &NineKeyShuangpinTable) -> Result<(), NineKeyShuangpinError> {
    canonicalize(table).map(|_| ())
}

/// 一张表的键面提示，不留下任何东西（设置页预览编辑中的表时用）；表不合法时返回原因。
pub fn table_key_hints(
    table: &NineKeyShuangpinTable,
) -> Result<Vec<NineKeyShuangpinKeyHint>, NineKeyShuangpinError> {
    Ok(NineKeyShuangpin::build(canonicalize(table)?).key_hints())
}

/// 方案 `kind` 的表：内置方案取内置的表，`Custom` 校验 `custom`（没有表时报缺了第一个声母）。
pub fn resolve(
    kind: NineKeyShuangpinKind,
    custom: Option<&NineKeyShuangpinTable>,
) -> Result<&'static NineKeyShuangpin, NineKeyShuangpinError> {
    match builtin_table(kind) {
        Some(table) => nine_key_shuangpin(&table),
        None => nine_key_shuangpin(custom.unwrap_or(&NineKeyShuangpinTable::default())),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn eighteen_key_preserves_every_accepted_builtin_and_custom_encoding() {
        use crate::types::ShuangpinProfileKind;
        let groups = [
            "q", "we", "rt", "y", "u", "io", "p", "a", "sd", "fg", "h", "jk", "l;", "z", "xc", "v",
            "bn", "m",
        ];
        let expected_key = |letter| {
            groups
                .iter()
                .find(|group| group.as_bytes().contains(&letter))
                .unwrap()
                .as_bytes()[0]
        };
        for kind in [
            ShuangpinProfileKind::Xiaohe,
            ShuangpinProfileKind::Ziranma,
            ShuangpinProfileKind::Microsoft,
            ShuangpinProfileKind::Shoudao,
            ShuangpinProfileKind::SmartAbc,
        ] {
            let profile = crate::shuangpin::profile::profile(kind).unwrap();
            let custom_source = crate::shuangpin::custom::builtin_table(kind).unwrap();
            let table = resolve_eighteen_key_shuangpin(kind, None).unwrap();
            let fourteen = resolve_fourteen_key_shuangpin(kind, None).unwrap();
            assert!(
                !std::ptr::eq(table, fourteen),
                "布局必须拥有独立的派生表和拼写缓存"
            );
            assert!(std::ptr::eq(
                table,
                resolve_eighteen_key_shuangpin(kind, None).unwrap()
            ));
            let custom =
                resolve_eighteen_key_shuangpin(ShuangpinProfileKind::Custom, Some(&custom_source))
                    .unwrap();
            let grid = crate::KeyGrid::EighteenKeyShuangpin(table);
            for letter in (b'a'..=b'z').chain(std::iter::once(b';')) {
                assert_eq!(grid.code_of(letter), Some(expected_key(letter)));
            }
            for first in b'a'..=b'z' {
                for second in (b'a'..=b'z').chain(std::iter::once(b';')) {
                    let raw = String::from_utf8(vec![first, second]).unwrap();
                    let syllable = crate::shuangpin::utils::cvt_single_sp_to_pinyin(&raw, profile);
                    if syllable.is_empty() {
                        continue;
                    }
                    let projected = [expected_key(first), expected_key(second)];
                    for actual in [table, custom] {
                        assert!(
                            actual
                                .syllable_codes(&syllable)
                                .any(|code| *code == projected),
                            "{kind:?} {raw} {syllable} {projected:?}"
                        );
                        let input = String::from_utf8(projected.to_vec()).unwrap();
                        assert_eq!(actual.matching_prefix_len(&syllable, &input), Some(2));
                    }
                }
            }
        }
        assert!(resolve_eighteen_key_shuangpin(ShuangpinProfileKind::Custom, None).is_err());
        assert!(resolve_eighteen_key_shuangpin(
            ShuangpinProfileKind::Custom,
            Some(&crate::ShuangpinCustomTable::default())
        )
        .is_err());
    }

    #[test]
    fn fourteen_key_preserves_every_accepted_builtin_and_custom_encoding() {
        use crate::types::ShuangpinProfileKind;
        for kind in [
            ShuangpinProfileKind::Xiaohe,
            ShuangpinProfileKind::Ziranma,
            ShuangpinProfileKind::Microsoft,
            ShuangpinProfileKind::Shoudao,
            ShuangpinProfileKind::SmartAbc,
        ] {
            let profile = crate::shuangpin::profile::profile(kind).unwrap();
            let custom_source = crate::shuangpin::custom::builtin_table(kind).unwrap();
            let table = resolve_fourteen_key_shuangpin(kind, None).unwrap();
            assert!(std::ptr::eq(
                table,
                resolve_fourteen_key_shuangpin(kind, None).unwrap()
            ));
            let custom =
                resolve_fourteen_key_shuangpin(ShuangpinProfileKind::Custom, Some(&custom_source))
                    .unwrap();
            let grid = crate::KeyGrid::FourteenKey;
            for first in b'a'..=b'z' {
                for second in (b'a'..=b'z').chain(std::iter::once(b';')) {
                    let raw = String::from_utf8(vec![first, second]).unwrap();
                    let syllable = crate::shuangpin::utils::cvt_single_sp_to_pinyin(&raw, profile);
                    if syllable.is_empty() {
                        continue;
                    }
                    let projected = [
                        grid.code_of(first).unwrap(),
                        if second == b';' {
                            b'l'
                        } else {
                            grid.code_of(second).unwrap()
                        },
                    ];
                    for actual in [table, custom] {
                        assert!(
                            actual
                                .syllable_codes(&syllable)
                                .any(|code| *code == projected),
                            "{kind:?} {raw} {syllable} {projected:?}"
                        );
                        let input = String::from_utf8(projected.to_vec()).unwrap();
                        assert_eq!(
                            actual.matching_prefix_len(&syllable, &input),
                            Some(2),
                            "{kind:?} {raw}"
                        );
                    }
                }
            }
        }
    }

    #[test]
    fn eighteen_key_exact_pairs_match_only_the_source_profile() {
        use crate::types::ShuangpinProfileKind;
        for kind in [
            ShuangpinProfileKind::Xiaohe,
            ShuangpinProfileKind::Ziranma,
            ShuangpinProfileKind::Microsoft,
            ShuangpinProfileKind::Shoudao,
            ShuangpinProfileKind::SmartAbc,
        ] {
            let profile = crate::shuangpin::profile::profile(kind).unwrap();
            let custom = crate::shuangpin::custom::builtin_table(kind).unwrap();
            for table in [
                resolve_eighteen_key_shuangpin(kind, None).unwrap(),
                resolve_eighteen_key_shuangpin(ShuangpinProfileKind::Custom, Some(&custom))
                    .unwrap(),
            ] {
                for first in b'a'..=b'z' {
                    for second in b'a'..=b'z' {
                        let raw = String::from_utf8(vec![first, second]).unwrap();
                        let expected =
                            crate::shuangpin::utils::cvt_single_sp_to_pinyin(&raw, profile);
                        let exact = [first.to_ascii_uppercase(), second.to_ascii_uppercase()];
                        let found: Vec<_> = table
                            .coded_syllables()
                            .filter(|(_, code)| **code == exact)
                            .map(|(syllable, _)| syllable)
                            .collect();
                        if expected.is_empty() {
                            assert!(found.is_empty(), "{kind:?} {raw}: {found:?}");
                        } else {
                            assert_eq!(found, [expected.as_str()], "{kind:?} {raw}");
                        }
                    }
                }
            }
        }
    }

    #[test]
    fn fourteen_key_keeps_semicolon_aliases_and_rejects_invalid_custom_tables() {
        use crate::types::ShuangpinProfileKind;
        let table = resolve_fourteen_key_shuangpin(ShuangpinProfileKind::Microsoft, None).unwrap();
        assert_eq!(table.encode("qing"), "ql");
        for code in ["ju", "jt"] {
            assert_eq!(table.matching_prefix_len("ju", code), Some(2));
            assert_eq!(table.piece_code_len("ju", code), Some(2));
            assert_eq!(
                table.matching_prefix_len("ju'ni", &format!("{code}bu")),
                Some(4)
            );
        }
        assert_eq!(table.matching_prefix_len("ju", "ja"), None);
        assert_eq!(table.matching_prefix_len("ju'unknown", "ju"), None);
        assert!(resolve_fourteen_key_shuangpin(ShuangpinProfileKind::Custom, None).is_err());
        assert!(resolve_fourteen_key_shuangpin(
            ShuangpinProfileKind::Custom,
            Some(&crate::ShuangpinCustomTable::default())
        )
        .is_err());
        let mut digits = builtin_table(NineKeyShuangpinKind::Migala3).unwrap();
        digits.initials[0].1 = "q".to_owned();
        assert!(check_table(&digits).is_err(), "九键用户表仍只接受数字");
    }

    fn table_of(kind: NineKeyShuangpinKind) -> NineKeyShuangpinTable {
        builtin_table(kind).unwrap()
    }

    fn code(profile: &NineKeyShuangpin, syllable: &str) -> String {
        profile.encode(syllable)
    }

    fn replace(entries: &mut [(String, String)], unit: &str, key: &str) {
        entries
            .iter_mut()
            .find(|(known, _)| known == unit)
            .unwrap()
            .1 = key.to_owned();
    }

    #[test]
    fn every_syllable_has_one_code() {
        // 每个能打的音节都拆得成声母加韵母表里的韵母，或者是零声母表里的音节，所以每张合法的表都给每个音节恰好一个编码。
        for &syllable in syllables() {
            match parts(syllable) {
                Parts::Consonant {
                    initial,
                    final_unit,
                } => {
                    assert!(INITIALS.contains(&initial), "{syllable}");
                    assert!(FINALS.contains(&final_unit), "{syllable}");
                }
                Parts::Zero => assert!(ZERO_INITIALS.contains(&syllable), "{syllable}"),
            }
        }
        for kind in NineKeyShuangpinKind::BUILTIN {
            let profile = resolve(kind, None).unwrap();
            assert_eq!(profile.codes.len(), syllables().len(), "{kind:?}");
        }
        assert!(!syllables().contains(&"jv"));
        assert!(!syllables().contains(&"lue"));
        assert!(syllables().contains(&"lve"));
        assert!(syllables().contains(&"eng"));
    }

    /// 内置表写成用户表再读回来，是同一份方案；打乱顺序也一样。
    #[test]
    fn builtin_tables_round_trip() {
        for kind in NineKeyShuangpinKind::BUILTIN {
            let table = table_of(kind);
            assert_eq!(table.initials.len(), 23);
            assert_eq!(table.finals.len(), 33);
            assert_eq!(table.zero_initials.len(), 12);
            let builtin = resolve(kind, None).unwrap();
            let custom = resolve(NineKeyShuangpinKind::Custom, Some(&table)).unwrap();
            assert!(std::ptr::eq(builtin, custom), "{kind:?}");
            let mut shuffled = table.clone();
            shuffled.initials.reverse();
            shuffled.finals.rotate_left(7);
            assert!(std::ptr::eq(
                builtin,
                nine_key_shuangpin(&shuffled).unwrap()
            ));
            assert_eq!(check_table(&table), Ok(()));
        }
        assert_eq!(builtin_table(NineKeyShuangpinKind::Custom), None);
        let migala2 = resolve(NineKeyShuangpinKind::Migala2, None).unwrap();
        let migala3 = resolve(NineKeyShuangpinKind::Migala3, None).unwrap();
        assert!(!std::ptr::eq(migala2, migala3));
    }

    /// 米旮旯 3 按公开描述的几处核对：声母加韵母，零声母在 2 键，u、ü、ei 在 0 键。
    #[test]
    fn migala_3_follows_its_published_rules() {
        let profile = resolve(NineKeyShuangpinKind::Migala3, None).unwrap();
        // 助记诗：「相（iang）爱（ai）在 2 键上，铅（ian）华（ua）在 9 键上」。
        assert_eq!(code(profile, "xiang"), "92");
        assert_eq!(code(profile, "ai"), "22");
        assert_eq!(code(profile, "qian"), "79");
        assert_eq!(code(profile, "hua"), "49");
        assert_eq!(code(profile, "zhong"), "81");
        assert_eq!(code(profile, "guo"), "46");
        assert_eq!(code(profile, "chu"), "30");
        assert_eq!(code(profile, "lv"), "50");
        assert_eq!(code(profile, "ju"), "50");
        assert_eq!(code(profile, "ren"), "43");
        assert_eq!(code(profile, "ei"), "20");
        assert_eq!(code(profile, "er"), "23");
        assert_eq!(code(profile, "zhong'g"), "814");
        assert_eq!(code(profile, "zhong'xyz"), "");
    }

    /// 辜氏 2 低重码的零声母按来源列出的例子：爱 22、熬 27、诶 23、耳 23、嗯 23、安 25、昂 25。
    #[test]
    fn gushi_2_low_zero_initials_match_the_published_examples() {
        let profile = resolve(NineKeyShuangpinKind::Gushi2Low, None).unwrap();
        for (syllable, expected) in [
            ("ai", "22"),
            ("ao", "27"),
            ("ei", "23"),
            ("er", "23"),
            ("en", "23"),
            ("an", "25"),
            ("ang", "25"),
        ] {
            assert_eq!(code(profile, syllable), expected, "{syllable}");
        }
        assert_eq!(code(profile, "xing"), "90");
        assert_eq!(code(profile, "xiong"), "91");
        assert_eq!(code(profile, "lve"), "58");
    }

    #[test]
    fn migala_2_puts_the_zero_initial_on_six() {
        let profile = resolve(NineKeyShuangpinKind::Migala2, None).unwrap();
        assert_eq!(code(profile, "ou"), "66");
        assert_eq!(code(profile, "ang"), "65");
        assert_eq!(code(profile, "chang"), "55");
        assert_eq!(code(profile, "shuai"), "88");
        assert_eq!(code(profile, "lia"), "51");
    }

    /// 一个编码可以对应好几个音节，零声母音节和声母加韵母的音节也可以共用编码。
    #[test]
    fn codes_are_shared_between_syllables() {
        let profile = resolve(NineKeyShuangpinKind::Migala3, None).unwrap();
        let mut on_25: Vec<&str> = profile
            .coded_syllables()
            .filter(|(_, code)| **code == *b"25")
            .map(|(syllable, _)| syllable)
            .collect();
        on_25.sort_unstable();
        assert_eq!(on_25, ["an", "ang", "ban", "bang", "can", "cang"]);
    }

    #[test]
    fn invalid_tables_name_the_place() {
        let mut table = table_of(NineKeyShuangpinKind::Migala3);
        table.finals.retain(|(unit, _)| unit != "iong");
        assert_eq!(
            check_table(&table),
            Err(NineKeyShuangpinError::MissingUnit {
                part: CustomTablePart::Finals,
                unit: "iong",
            })
        );
        let mut table = table_of(NineKeyShuangpinKind::Migala3);
        table.finals.push(("iou".to_owned(), "1".to_owned()));
        assert_eq!(
            check_table(&table),
            Err(NineKeyShuangpinError::UnknownUnit {
                part: CustomTablePart::Finals,
                unit: "iou".to_owned(),
            })
        );
        let mut table = table_of(NineKeyShuangpinKind::Migala3);
        table.initials.push(("zh".to_owned(), "8".to_owned()));
        assert_eq!(
            check_table(&table),
            Err(NineKeyShuangpinError::DuplicateUnit {
                part: CustomTablePart::Initials,
                unit: "zh".to_owned(),
            })
        );
        for (part, unit, key) in [
            (CustomTablePart::Initials, "b", "a"),
            (CustomTablePart::Initials, "b", "22"),
            (CustomTablePart::Finals, "ang", ""),
            (CustomTablePart::Finals, "ang", "*"),
            (CustomTablePart::ZeroInitials, "a", "2"),
            (CustomTablePart::ZeroInitials, "a", "2a"),
            (CustomTablePart::ZeroInitials, "a", "222"),
        ] {
            let mut table = table_of(NineKeyShuangpinKind::Migala3);
            let entries = match part {
                CustomTablePart::Initials => &mut table.initials,
                CustomTablePart::Finals => &mut table.finals,
                CustomTablePart::ZeroInitials => &mut table.zero_initials,
            };
            replace(entries, unit, key);
            assert_eq!(
                check_table(&table),
                Err(NineKeyShuangpinError::InvalidKey {
                    part,
                    unit: unit.to_owned(),
                    key: key.to_owned(),
                }),
                "{unit} {key:?}"
            );
            assert_eq!(nine_key_shuangpin(&table).map(|_| ()), check_table(&table));
        }
        assert!(matches!(
            resolve(NineKeyShuangpinKind::Custom, None),
            Err(NineKeyShuangpinError::MissingUnit { .. })
        ));
    }

    /// 1 和 0 都可以用；一张把零声母放在 1 键、谁也不和谁共用编码的表照样合法。
    #[test]
    fn a_custom_table_may_use_one_and_zero() {
        let mut table = table_of(NineKeyShuangpinKind::Migala3);
        for (syllable, key) in table.zero_initials.iter_mut() {
            let final_key = &key[1..];
            *key = format!("1{final_key}");
            assert_eq!(key.len(), 2, "{syllable}");
        }
        let profile = nine_key_shuangpin(&table).unwrap();
        assert_eq!(code(profile, "an"), "15");
        assert!(profile.uses_key(b'1'));
        assert!(profile.uses_key(b'0'));
        assert_eq!(profile.letters(b'1'), "aeo");
    }

    #[test]
    fn one_key_pieces_and_letters_follow_the_table() {
        let profile = resolve(NineKeyShuangpinKind::Migala3, None).unwrap();
        assert_eq!(profile.letters(b'2'), "abceo");
        assert_eq!(profile.letters(b'8'), "stz");
        assert_eq!(profile.letters(b'1'), "");
        let pieces = profile.one_key_pieces();
        assert!(pieces.contains(&("zh", b'8')));
        assert!(pieces.contains(&("a", b'2')));
        assert!(pieces.contains(&("o", b'2')));
        assert_eq!(pieces.iter().filter(|(piece, _)| *piece == "a").count(), 1);
    }

    #[test]
    fn keywords_and_initials_spell_digits() {
        let profile = resolve(NineKeyShuangpinKind::Migala3, None).unwrap();
        // ri'qi：r 4 + i 4，q 7 + i 4。
        assert!(profile.spells_keyword("riqi", "4474"));
        assert!(profile.spells_keyword("rq", "47"));
        assert!(!profile.spells_keyword("rq", "4474"));
        // shi'jian：sh 8 + i 4，j 5 + ian 9。简拼 sj 的 s 可以是 s（7）也可以是 sh（8）。
        assert!(profile.spells_keyword("shijian", "8459"));
        assert!(profile.spells_keyword("sj", "75"));
        assert!(profile.spells_keyword("sj", "85"));
        assert!(!profile.spells_keyword("sj", "95"));
        assert!(profile.spells_initials("zhong'guo", "84"));
        assert!(!profile.spells_initials("zou'guo", "84"));
        assert!(!profile.spells_initials("zhong", "84"));
    }

    #[test]
    fn key_hints_list_each_used_key() {
        let profile = resolve(NineKeyShuangpinKind::Migala3, None).unwrap();
        let hints = profile.key_hints();
        let keys: String = hints.iter().map(|hint| hint.key).collect();
        assert_eq!(keys, "1234567890");
        let two = &hints[1];
        assert_eq!(two.initials, ["b", "c"]);
        assert_eq!(two.finals, ["a", "ai", "iang"]);
        assert!(two.zero_initial);
        let zero = &hints[9];
        assert!(zero.initials.is_empty());
        assert_eq!(zero.finals, ["u", "ü", "ei"]);
        assert_eq!(hints[2].initials, ["d", "f", "ch"]);
        assert_eq!(hints[7].initials, ["t", "sh", "zh"]);
        assert!(!zero.zero_initial);
        let migala2 = resolve(NineKeyShuangpinKind::Migala2, None).unwrap();
        assert!(migala2.key_hints().iter().all(|hint| hint.key != '0'));
    }
}
