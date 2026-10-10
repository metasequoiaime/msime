//! 用户自定义双拼方案：校验 [`ShuangpinCustomTable`]，把合法的表变成与内置方案同样的 `&'static ShuangpinProfile`，之后的切分、转换、辅助码和 `;` 键都走内置方案那一套，不另写一份解码。
//!
//! 合法的表按内容只留一份：同一张表反复校验、反复应用（每次偏好变更都会校验并重建 Engine）拿到的是同一个引用，只有第一次逐个检查编码；进程里只为用户实际用过的每一张不同的表各留一份（几 KB）。不合法的表不留任何东西。

use std::collections::HashSet;
use std::fmt;
use std::sync::{Mutex, OnceLock, PoisonError};

use super::profile::{accepted_syllables, default_profile, profile};
use super::ShuangpinProfile;
use crate::diagnostics;
use crate::error::{EngineError, Result};
use crate::types::{ShuangpinCustomTable, ShuangpinProfileKind};

/// 自定义方案要给出键位的多字母声母；单字母声母固定在自己的字母键上。
pub const MULTI_LETTER_INITIALS: [&str; 3] = ["zh", "ch", "sh"];
/// 单字母声母。多字母声母不能放在这些键上：一个单键片段先按多字母声母读，放上去以后这个单字母声母就打不出来了。
const SINGLE_LETTER_INITIALS: &[u8] = b"bpmfdtnlgkhjqxrzcsyw";
/// 每个方案都要给出键位的 33 个韵母，与内置方案的韵母表相同（`builtin_profiles_use_the_same_units` 核对）。
pub const FINALS: [&str; 33] = [
    "a", "o", "e", "i", "u", "v", "ai", "ei", "ui", "ao", "ou", "iu", "ie", "ve", "ue", "an", "en",
    "in", "un", "ang", "eng", "ing", "ong", "ia", "ua", "uo", "uai", "ian", "uan", "iao", "iang",
    "uang", "iong",
];
/// 每个方案都要给出两键编码的 12 个零声母音节。
pub const ZERO_INITIALS: [&str; 12] = [
    "a", "ai", "an", "ang", "ao", "e", "ei", "en", "eng", "er", "o", "ou",
];

/// 表里的哪一部分。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CustomTablePart {
    Initials,
    Finals,
    ZeroInitials,
}

impl CustomTablePart {
    pub fn name(self) -> &'static str {
        match self {
            Self::Initials => "initials",
            Self::Finals => "finals",
            Self::ZeroInitials => "zero_initials",
        }
    }
}

/// 表为什么不能用。宿主可以按种类给出自己的提示文字，`Display` 是给日志看的英文。
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CustomProfileError {
    /// 这一部分不认识的单位，例如 `initials` 里写了 `b`，`finals` 里写了 `iou`。
    UnknownUnit { part: CustomTablePart, unit: String },
    /// 同一个单位写了两次。
    DuplicateUnit { part: CustomTablePart, unit: String },
    /// 缺了这个单位的键位。
    MissingUnit {
        part: CustomTablePart,
        unit: &'static str,
    },
    /// 键不是这里允许的：声母和韵母是一个键，零声母是两个键；键是字母或 `;`，`;` 只能做韵母或零声母编码的第二个键（它只能接在奇数长度的片段之后）。
    InvalidKey {
        part: CustomTablePart,
        unit: String,
        key: String,
    },
    /// 多字母声母放在了单字母声母的键上，或两个多字母声母放在了同一个键上。
    InitialKeyTaken { unit: &'static str, key: String },
    /// 两个不同的音节解出了同一个两键编码。`code` 是这个编码，`first` 和 `second` 是这两个音节。
    AmbiguousCode {
        code: String,
        first: String,
        second: String,
    },
}

impl fmt::Display for CustomProfileError {
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
            Self::InvalidKey { part, unit, key } => {
                write!(
                    formatter,
                    "{} unit {unit:?} has an invalid key {key:?}",
                    part.name()
                )
            }
            Self::InitialKeyTaken { unit, key } => {
                write!(formatter, "initial {unit:?} cannot take key {key:?}")
            }
            Self::AmbiguousCode {
                code,
                first,
                second,
            } => write!(
                formatter,
                "code {code:?} reads as both {first:?} and {second:?}"
            ),
        }
    }
}

impl std::error::Error for CustomProfileError {}

/// 规范化后的一张表：单位换成常量里的那个 `&'static str`，键转成小写，按常量的顺序排好。
struct Canonical {
    initials: Vec<(&'static str, String)>,
    finals: Vec<(&'static str, String)>,
    zero_initials: Vec<(&'static str, String)>,
}

/// 校验 `table`，合法时返回它对应的方案。同一张表（单位和键相同，顺序和键的大小写不论）总是返回同一个引用。
pub fn custom_profile(
    table: &ShuangpinCustomTable,
) -> Result<&'static ShuangpinProfile, CustomProfileError> {
    let canonical = canonicalize(table)?;
    static INTERNED: OnceLock<Mutex<Vec<&'static ShuangpinProfile>>> = OnceLock::new();
    let mut interned = INTERNED
        .get_or_init(Mutex::default)
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    if let Some(found) = interned.iter().find(|profile| canonical.matches(profile)) {
        return Ok(found);
    }
    check_codes(&canonical)?;
    let profile: &'static ShuangpinProfile = Box::leak(Box::new(ShuangpinProfile {
        kind: ShuangpinProfileKind::Custom,
        initials: leak_pairs(canonical.initials),
        zero_initials: leak_pairs(canonical.zero_initials),
        finals: leak_pairs(canonical.finals),
    }));
    interned.push(profile);
    Ok(profile)
}

/// 会话用的双拼方案（`SessionOptions::shuangpin_profile` 与 `shuangpin_custom_profile`）。内置方案取内置的表；`Custom` 校验用户的表，没有表或表不合法时报 `INVALID_CUSTOM_SHUANGPIN_PROFILE`。`shuangpin_enabled` 为假（会话里没有双拼）时这份设置用不上，出错也按小鹤处理，免得一份只对双拼有意义的偏好让没有双拼的会话建不起来，与 `host::options::shuangpin_profile` 处理不合法的 ABI 值相同。
pub fn session_profile(
    kind: ShuangpinProfileKind,
    table: Option<&ShuangpinCustomTable>,
    shuangpin_enabled: bool,
) -> Result<&'static ShuangpinProfile> {
    if let Some(builtin) = profile(kind) {
        return Ok(builtin);
    }
    let resolved = match table {
        Some(table) => custom_profile(table).map_err(|error| error.to_string()),
        None => Err("no table".to_owned()),
    };
    match resolved {
        Ok(custom) => Ok(custom),
        Err(_) if !shuangpin_enabled => Ok(default_profile()),
        Err(reason) => Err(EngineError::invalid(format!(
            "{}: {reason}",
            diagnostics::INVALID_CUSTOM_SHUANGPIN_PROFILE
        ))),
    }
}

/// 只回答表能不能用。合法的表同样按内容留下一份，随后用它建会话（每次偏好变更都会先校验再重建）时直接复用，不再逐个编码检查一遍。
pub fn validate_custom_profile(table: &ShuangpinCustomTable) -> Result<(), CustomProfileError> {
    custom_profile(table).map(|_| ())
}

fn leak_pairs(pairs: Vec<(&'static str, String)>) -> &'static [(&'static str, &'static str)] {
    let leaked: Vec<(&'static str, &'static str)> = pairs
        .into_iter()
        .map(|(unit, key)| (unit, &*Box::leak(key.into_boxed_str())))
        .collect();
    Box::leak(leaked.into_boxed_slice())
}

impl Canonical {
    fn matches(&self, profile: &ShuangpinProfile) -> bool {
        fn same(left: &[(&'static str, String)], right: &[(&str, &str)]) -> bool {
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right)
                    .all(|((unit, key), (other_unit, other_key))| {
                        unit == other_unit && key == other_key
                    })
        }
        same(&self.initials, profile.initials)
            && same(&self.finals, profile.finals)
            && same(&self.zero_initials, profile.zero_initials)
    }

    /// 单键片段读作的声母：多字母声母的键读作那个声母，其余就是这个字母本身（与 `utils::initial_for_key` 相同）。
    fn initial_for_key(&self, key: u8) -> String {
        self.initials
            .iter()
            .find(|(_, mapped)| mapped.as_bytes() == [key])
            .map_or_else(
                || char::from(key).to_string(),
                |(unit, _)| (*unit).to_owned(),
            )
    }

    /// 声母键加韵母键解出的音节，与 `utils::cvt_single_sp_to_pinyin` 跳过零声母表之后的那一步相同；同一个编码解出两个不同的音节时报错。
    /// `initial` 是 `first` 读作的声母（`initial_for_key`），由调用方算好；`buffer` 是拼音节用的临时存储，逐个编码复用。
    fn decode_pair(
        &self,
        initial: &str,
        first: u8,
        second: u8,
        buffer: &mut String,
    ) -> Result<Option<String>, CustomProfileError> {
        let accepted = accepted_syllables();
        let mut found: Option<String> = None;
        for (unit, _) in self
            .finals
            .iter()
            .filter(|(_, key)| key.as_bytes() == [second])
        {
            let normalized = if *unit == "v" && matches!(initial, "j" | "q" | "x" | "y") {
                "u"
            } else {
                unit
            };
            buffer.clear();
            buffer.push_str(initial);
            buffer.push_str(normalized);
            if !accepted.contains(buffer.as_str()) {
                continue;
            }
            match &found {
                Some(previous) if previous != buffer => {
                    return Err(CustomProfileError::AmbiguousCode {
                        code: code_text(first, second),
                        first: previous.clone(),
                        second: buffer.clone(),
                    });
                }
                Some(_) => {}
                None => found = Some(buffer.clone()),
            }
        }
        Ok(found)
    }
}

fn code_text(first: u8, second: u8) -> String {
    [char::from(first), char::from(second)].iter().collect()
}

fn canonicalize(table: &ShuangpinCustomTable) -> Result<Canonical, CustomProfileError> {
    Ok(Canonical {
        initials: canonical_part(
            CustomTablePart::Initials,
            &table.initials,
            &MULTI_LETTER_INITIALS,
        )?,
        finals: canonical_part(CustomTablePart::Finals, &table.finals, &FINALS)?,
        zero_initials: canonical_part(
            CustomTablePart::ZeroInitials,
            &table.zero_initials,
            &ZERO_INITIALS,
        )?,
    })
}

fn canonical_part(
    part: CustomTablePart,
    entries: &[(String, String)],
    units: &[&'static str],
) -> Result<Vec<(&'static str, String)>, CustomProfileError> {
    let mut keys: Vec<Option<String>> = vec![None; units.len()];
    for (unit, key) in entries {
        let Some(index) = units.iter().position(|known| known == unit) else {
            return Err(CustomProfileError::UnknownUnit {
                part,
                unit: unit.clone(),
            });
        };
        if keys[index].is_some() {
            return Err(CustomProfileError::DuplicateUnit {
                part,
                unit: unit.clone(),
            });
        }
        let key = key.to_ascii_lowercase();
        if !key_is_valid(part, key.as_bytes()) {
            return Err(CustomProfileError::InvalidKey {
                part,
                unit: unit.clone(),
                key,
            });
        }
        keys[index] = Some(key);
    }
    units
        .iter()
        .zip(keys)
        .map(|(unit, key)| {
            key.map(|key| (*unit, key))
                .ok_or(CustomProfileError::MissingUnit { part, unit })
        })
        .collect()
}

/// `;` 只能做一个音节的第二个键：方案只在奇数长度的片段之后接受它（`ShuangpinScheme::accepts_ing_key`）。
fn key_is_valid(part: CustomTablePart, key: &[u8]) -> bool {
    match (part, key) {
        (CustomTablePart::Initials, [key]) => key.is_ascii_lowercase(),
        (CustomTablePart::Finals, [key]) => key.is_ascii_lowercase() || *key == b';',
        (CustomTablePart::ZeroInitials, [first, second]) => {
            first.is_ascii_lowercase() && (second.is_ascii_lowercase() || *second == b';')
        }
        _ => false,
    }
}

fn check_codes(canonical: &Canonical) -> Result<(), CustomProfileError> {
    let mut initial_keys: HashSet<u8> = HashSet::new();
    for (unit, key) in &canonical.initials {
        let byte = key.as_bytes()[0];
        if SINGLE_LETTER_INITIALS.contains(&byte) || !initial_keys.insert(byte) {
            return Err(CustomProfileError::InitialKeyTaken {
                unit,
                key: key.clone(),
            });
        }
    }
    let mut final_keys: Vec<u8> = canonical
        .finals
        .iter()
        .map(|(_, key)| key.as_bytes()[0])
        .collect();
    final_keys.sort_unstable();
    final_keys.dedup();
    let mut buffer = String::with_capacity(8);
    for first in b'a'..=b'z' {
        let initial = canonical.initial_for_key(first);
        for &second in &final_keys {
            canonical.decode_pair(&initial, first, second, &mut buffer)?;
        }
    }
    let mut zero_codes: Vec<(&str, &str)> = Vec::with_capacity(canonical.zero_initials.len());
    for (syllable, code) in &canonical.zero_initials {
        // 零声母表先于声母加韵母查（`cvt_single_sp_to_pinyin`），编码一旦被别的音节用掉，那个音节就再也打不出来。
        if let Some((other, _)) = zero_codes.iter().find(|(_, used)| used == code) {
            return Err(CustomProfileError::AmbiguousCode {
                code: code.clone(),
                first: (*other).to_owned(),
                second: (*syllable).to_owned(),
            });
        }
        let bytes = code.as_bytes();
        let initial = canonical.initial_for_key(bytes[0]);
        if let Some(decoded) = canonical.decode_pair(&initial, bytes[0], bytes[1], &mut buffer)? {
            if decoded != *syllable {
                return Err(CustomProfileError::AmbiguousCode {
                    code: code.clone(),
                    first: decoded,
                    second: (*syllable).to_owned(),
                });
            }
        }
        zero_codes.push((syllable, code));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::shuangpin::profile::profile;
    use crate::shuangpin::scheme::ShuangpinScheme;
    use crate::shuangpin::utils::cvt_single_sp_to_pinyin;
    use crate::types::SchemeKey;

    const BUILTIN: [ShuangpinProfileKind; 4] = [
        ShuangpinProfileKind::Xiaohe,
        ShuangpinProfileKind::Ziranma,
        ShuangpinProfileKind::Shoudao,
        ShuangpinProfileKind::Microsoft,
    ];

    fn owned(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(unit, key)| ((*unit).to_owned(), (*key).to_owned()))
            .collect()
    }

    fn table_of(kind: ShuangpinProfileKind) -> ShuangpinCustomTable {
        let source = profile(kind).unwrap();
        ShuangpinCustomTable {
            initials: owned(source.initials),
            finals: owned(source.finals),
            zero_initials: owned(source.zero_initials),
        }
    }

    fn set(units: impl IntoIterator<Item = &'static str>) -> Vec<&'static str> {
        let mut units: Vec<&'static str> = units.into_iter().collect();
        units.sort_unstable();
        units
    }

    fn replace(entries: &mut [(String, String)], unit: &str, key: &str) {
        entries
            .iter_mut()
            .find(|(known, _)| known == unit)
            .unwrap()
            .1 = key.to_owned();
    }

    #[test]
    fn builtin_profiles_use_the_same_units() {
        for kind in BUILTIN {
            let source = profile(kind).unwrap();
            assert_eq!(
                set(source.initials.iter().map(|(unit, _)| *unit)),
                set(MULTI_LETTER_INITIALS),
                "{kind:?}"
            );
            assert_eq!(
                set(source.finals.iter().map(|(unit, _)| *unit)),
                set(FINALS),
                "{kind:?}"
            );
            assert_eq!(
                set(source.zero_initials.iter().map(|(unit, _)| *unit)),
                set(ZERO_INITIALS),
                "{kind:?}"
            );
        }
    }

    /// 每个内置方案写成自定义表都合法，解出的每个编码都和内置方案一样。
    #[test]
    fn builtin_tables_pass_and_decode_as_the_builtin_profile() {
        for kind in BUILTIN {
            let builtin = profile(kind).unwrap();
            let custom = custom_profile(&table_of(kind)).unwrap();
            assert_eq!(custom.kind, ShuangpinProfileKind::Custom);
            assert_eq!(custom.uses_semicolon_key(), builtin.uses_semicolon_key());
            for first in b'a'..=b'z' {
                for second in (b'a'..=b'z').chain(*b";") {
                    let code = code_text(first, second);
                    assert_eq!(
                        cvt_single_sp_to_pinyin(&code, custom),
                        cvt_single_sp_to_pinyin(&code, builtin),
                        "{kind:?} {code}"
                    );
                }
            }
        }
    }

    #[test]
    fn the_same_table_is_interned_once() {
        let first = custom_profile(&table_of(ShuangpinProfileKind::Ziranma)).unwrap();
        let mut shuffled = table_of(ShuangpinProfileKind::Ziranma);
        shuffled.finals.reverse();
        for (_, key) in &mut shuffled.zero_initials {
            *key = key.to_ascii_uppercase();
        }
        let second = custom_profile(&shuffled).unwrap();
        assert!(std::ptr::eq(first, second));
        let other = custom_profile(&table_of(ShuangpinProfileKind::Xiaohe)).unwrap();
        assert!(!std::ptr::eq(first, other));
    }

    #[test]
    fn a_custom_profile_moves_finals_and_initials() {
        // 小鹤的表，把 zh 换到 a 键，零声母改用 o 引导（像微软那样），ing 换到 `;`。
        let mut table = table_of(ShuangpinProfileKind::Xiaohe);
        replace(&mut table.initials, "zh", "a");
        table.zero_initials = owned(&[
            ("a", "oa"),
            ("ai", "od"),
            ("an", "oj"),
            ("ang", "oh"),
            ("ao", "oc"),
            ("e", "oe"),
            ("ei", "ow"),
            ("en", "of"),
            ("eng", "og"),
            ("er", "or"),
            ("o", "oo"),
            ("ou", "oz"),
        ]);
        replace(&mut table.finals, "ing", ";");
        let custom = custom_profile(&table).unwrap();
        assert!(custom.uses_semicolon_key());
        assert_eq!(cvt_single_sp_to_pinyin("ah", custom), "zhang");
        assert_eq!(cvt_single_sp_to_pinyin("oa", custom), "a");
        assert_eq!(cvt_single_sp_to_pinyin("x;", custom), "xing");
        assert_eq!(cvt_single_sp_to_pinyin("xk", custom), "");
        assert_eq!(cvt_single_sp_to_pinyin("vh", custom), "");
    }

    #[test]
    fn a_zero_initial_can_use_semicolon_without_a_final_on_semicolon() {
        let mut table = table_of(ShuangpinProfileKind::Xiaohe);
        replace(&mut table.zero_initials, "o", "o;");
        let custom = custom_profile(&table).unwrap();
        assert_eq!(cvt_single_sp_to_pinyin("o;", custom), "o");
        assert!(custom.uses_semicolon_key());

        let mut scheme = ShuangpinScheme::new(custom);
        assert!(scheme.handle_key(SchemeKey::Letter(b'o')));
        assert!(scheme.handle_key(SchemeKey::Semicolon));
        assert_eq!(scheme.preedit(), "o;");
    }

    #[test]
    fn incomplete_or_unknown_units_are_refused() {
        let mut table = table_of(ShuangpinProfileKind::Xiaohe);
        table.finals.retain(|(unit, _)| unit != "iong");
        assert_eq!(
            validate_custom_profile(&table),
            Err(CustomProfileError::MissingUnit {
                part: CustomTablePart::Finals,
                unit: "iong",
            })
        );
        let mut table = table_of(ShuangpinProfileKind::Xiaohe);
        table.initials.push(("b".to_owned(), "b".to_owned()));
        assert_eq!(
            validate_custom_profile(&table),
            Err(CustomProfileError::UnknownUnit {
                part: CustomTablePart::Initials,
                unit: "b".to_owned(),
            })
        );
        let mut table = table_of(ShuangpinProfileKind::Xiaohe);
        table.finals.push(("iu".to_owned(), "q".to_owned()));
        assert_eq!(
            validate_custom_profile(&table),
            Err(CustomProfileError::DuplicateUnit {
                part: CustomTablePart::Finals,
                unit: "iu".to_owned(),
            })
        );
        assert!(matches!(
            validate_custom_profile(&ShuangpinCustomTable::default()),
            Err(CustomProfileError::MissingUnit { .. })
        ));
    }

    #[test]
    fn keys_must_fit_their_place() {
        for (part, unit, key) in [
            (CustomTablePart::Finals, "iu", "qq"),
            (CustomTablePart::Finals, "iu", "1"),
            (CustomTablePart::Finals, "iu", ""),
            (CustomTablePart::Initials, "zh", ";"),
            (CustomTablePart::ZeroInitials, "a", "a"),
            (CustomTablePart::ZeroInitials, "a", ";a"),
            (CustomTablePart::ZeroInitials, "a", "aé"),
        ] {
            let mut table = table_of(ShuangpinProfileKind::Xiaohe);
            let entries = match part {
                CustomTablePart::Initials => &mut table.initials,
                CustomTablePart::Finals => &mut table.finals,
                CustomTablePart::ZeroInitials => &mut table.zero_initials,
            };
            replace(entries, unit, key);
            assert_eq!(
                validate_custom_profile(&table),
                Err(CustomProfileError::InvalidKey {
                    part,
                    unit: unit.to_owned(),
                    key: key.to_ascii_lowercase(),
                }),
                "{unit} {key:?}"
            );
        }
    }

    #[test]
    fn initials_cannot_take_a_single_letter_initial_or_share_a_key() {
        let mut table = table_of(ShuangpinProfileKind::Xiaohe);
        replace(&mut table.initials, "sh", "s");
        assert_eq!(
            validate_custom_profile(&table),
            Err(CustomProfileError::InitialKeyTaken {
                unit: "sh",
                key: "s".to_owned(),
            })
        );
        let mut table = table_of(ShuangpinProfileKind::Xiaohe);
        replace(&mut table.initials, "ch", "u");
        assert!(matches!(
            validate_custom_profile(&table),
            Err(CustomProfileError::InitialKeyTaken { key, .. }) if key == "u"
        ));
    }

    #[test]
    fn colliding_codes_are_refused() {
        // 小鹤里 ing 和 uai 同在 k 上；把 ang 也放上去，`hk` 就既是 hang 又是 huai。
        let mut table = table_of(ShuangpinProfileKind::Xiaohe);
        replace(&mut table.finals, "ang", "k");
        assert!(matches!(
            validate_custom_profile(&table),
            Err(CustomProfileError::AmbiguousCode { .. })
        ));
        // 零声母编码占了一个声母加韵母的音节：小鹤的 sh 在 u 上，`ua` 就是 sha。
        let mut table = table_of(ShuangpinProfileKind::Xiaohe);
        replace(&mut table.zero_initials, "a", "ua");
        assert_eq!(
            validate_custom_profile(&table),
            Err(CustomProfileError::AmbiguousCode {
                code: "ua".to_owned(),
                first: "sha".to_owned(),
                second: "a".to_owned(),
            })
        );
        // 两个零声母音节用了同一个编码。
        let mut table = table_of(ShuangpinProfileKind::Xiaohe);
        replace(&mut table.zero_initials, "o", "aa");
        assert_eq!(
            validate_custom_profile(&table),
            Err(CustomProfileError::AmbiguousCode {
                code: "aa".to_owned(),
                first: "a".to_owned(),
                second: "o".to_owned(),
            })
        );
    }

    #[test]
    fn an_invalid_table_is_not_interned() {
        let mut table = table_of(ShuangpinProfileKind::Shoudao);
        replace(&mut table.finals, "ang", "k");
        assert!(custom_profile(&table).is_err());
        assert!(custom_profile(&table).is_err());
    }
}
