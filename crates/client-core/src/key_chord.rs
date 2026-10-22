//! 可重绑切换热键的共享 chord 契约：修饰键集合加一个主键。
//!
//! 字符串写法沿用旧版 Windows v0.11.1（`engine/contracts/key_chord.h` 与
//! `input_state_shortcuts.h`）：`"Ctrl+Shift+KeyE"`、`"Ctrl+Shift+Space"`、
//! `"Ctrl+Period"`，主键用 `KeyA`–`KeyZ`、`Digit0`–`Digit9`、`F1`–`F12` 和十二个
//! 具名键，与旧版文档和设置页录入的写法互读。解析比旧版宽：大小写不敏感、
//! 修饰键按集合比较、`Ctrl`/`Control`、`Alt`/`Option`、`Meta`/`Win`/`Cmd` 互为别名；
//! 格式化一律输出下面的规范形，同一个 chord 只有一种写法，冲突判定收敛成相等比较。

use std::fmt;
use std::str::FromStr;

/// chord 的主键：可以按下的那个非修饰键。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum ChordKey {
    /// 字母键，规范形 `KeyA`–`KeyZ`；按 Windows 虚拟键的惯例只存大写。
    Letter(char),
    /// 主键盘数字键，规范形 `Digit0`–`Digit9`。
    Digit(char),
    /// 功能键，规范形 `F1`–`F12`。
    Function(u8),
    Space,
    Semicolon,
    Equal,
    Comma,
    Minus,
    Period,
    Slash,
    Backquote,
    BracketLeft,
    Backslash,
    BracketRight,
    Quote,
}

/// 十二个具名键的规范名，与旧版 `NamedKeys[]` 一一对应。
const NAMED_KEYS: [(&str, ChordKey); 12] = [
    ("Space", ChordKey::Space),
    ("Semicolon", ChordKey::Semicolon),
    ("Equal", ChordKey::Equal),
    ("Comma", ChordKey::Comma),
    ("Minus", ChordKey::Minus),
    ("Period", ChordKey::Period),
    ("Slash", ChordKey::Slash),
    ("Backquote", ChordKey::Backquote),
    ("BracketLeft", ChordKey::BracketLeft),
    ("Backslash", ChordKey::Backslash),
    ("BracketRight", ChordKey::BracketRight),
    ("Quote", ChordKey::Quote),
];

impl ChordKey {
    /// 大小写不敏感地认一个键名；不认得的串返回 `None`，是不是修饰键名由调用方先排除。
    fn parse_name(name: &str) -> Option<Self> {
        let lower = name.to_ascii_lowercase();
        if lower.len() == 4 && lower.starts_with("key") {
            let letter = lower.as_bytes()[3];
            if letter.is_ascii_lowercase() {
                return Some(Self::Letter((letter - b'a' + b'A') as char));
            }
        }
        if lower.len() == 6 && lower.starts_with("digit") {
            let digit = lower.as_bytes()[5];
            if digit.is_ascii_digit() {
                return Some(Self::Digit(digit as char));
            }
        }
        if let Some(number) = lower.strip_prefix('f') {
            if let Ok(number) = number.parse::<u8>() {
                if (1..=12).contains(&number) {
                    return Some(Self::Function(number));
                }
            }
        }
        NAMED_KEYS
            .iter()
            .find(|(canonical, _)| canonical.eq_ignore_ascii_case(name))
            .map(|(_, key)| *key)
    }
}

/// chord 的修饰键集合：成员没有左右手之分，`Ctrl` 同时认左右 Ctrl。
///
/// `meta` 是各平台「徽标」键的中立名：macOS 的 Command、Windows 的 Win、Linux 的 Super。
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct ChordModifiers {
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
    pub meta: bool,
}

impl ChordModifiers {
    /// 一个修饰键名对应哪个成员；别名在这里归一，重复按同一成员判。
    fn slot(name: &str) -> Option<fn(&mut Self) -> &mut bool> {
        match name.to_ascii_lowercase().as_str() {
            "ctrl" | "control" => Some(|m| &mut m.ctrl),
            "alt" | "option" | "opt" => Some(|m| &mut m.alt),
            "shift" => Some(|m| &mut m.shift),
            "meta" | "win" | "cmd" | "command" | "super" => Some(|m| &mut m.meta),
            _ => None,
        }
    }
}

/// 一个完整的快捷键组合：若干修饰键加一个主键。
///
/// 相等即冲突判定的全部语义：解析时修饰键已归一成集合、键名归一成规范形，
/// 两个写法不同但指向同一按键的串解析出同一个值。
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct KeyChord {
    pub key: ChordKey,
    pub modifiers: ChordModifiers,
}

/// `KeyChord` 解析失败的原因。
#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum KeyChordError {
    #[error("key chord is empty")]
    Empty,
    #[error("key chord has no main key")]
    MissingKey,
    #[error("unknown key name `{0}`")]
    UnknownKey(String),
    #[error("unknown modifier `{0}`")]
    UnknownModifier(String),
    #[error("modifier `{0}` is repeated")]
    DuplicateModifier(String),
}

impl KeyChord {
    /// 两个 chord 是否冲突：相等就是冲突。解析已把别名、大小写和修饰键顺序归一，这里是给「冲突表」调用的命名入口。
    pub fn conflicts_with(&self, other: &Self) -> bool {
        self == other
    }
}

impl FromStr for KeyChord {
    type Err = KeyChordError;

    /// `修饰键+…+主键`，段间用 `+` 分隔，前后空白忽略。空串不是 chord：偏好里「不绑定」由调用方在解析前判空。
    fn from_str(text: &str) -> Result<Self, Self::Err> {
        let text = text.trim();
        if text.is_empty() {
            return Err(KeyChordError::Empty);
        }
        let mut segments = text.split('+').peekable();
        let mut modifier_texts = Vec::new();
        while let Some(segment) = segments.next() {
            if segments.peek().is_some() {
                modifier_texts.push(segment.trim());
            } else {
                let key_text = segment.trim();
                if key_text.is_empty() {
                    return Err(KeyChordError::MissingKey);
                }
                let mut modifiers = ChordModifiers::default();
                for name in &modifier_texts {
                    if name.is_empty() {
                        return Err(KeyChordError::MissingKey);
                    }
                    let Some(slot) = ChordModifiers::slot(name) else {
                        return Err(KeyChordError::UnknownModifier((*name).into()));
                    };
                    let flag = slot(&mut modifiers);
                    if *flag {
                        return Err(KeyChordError::DuplicateModifier((*name).into()));
                    }
                    *flag = true;
                }
                if ChordModifiers::slot(key_text).is_some() {
                    return Err(KeyChordError::MissingKey);
                }
                let key = ChordKey::parse_name(key_text)
                    .ok_or_else(|| KeyChordError::UnknownKey(key_text.into()))?;
                return Ok(Self { key, modifiers });
            }
        }
        unreachable!("split always yields at least one segment")
    }
}

impl fmt::Display for KeyChord {
    /// 规范形：修饰键固定 `Ctrl`、`Alt`、`Shift`、`Meta` 的顺序，键名取 [`ChordKey`] 的规范名。
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if self.modifiers.ctrl {
            f.write_str("Ctrl+")?;
        }
        if self.modifiers.alt {
            f.write_str("Alt+")?;
        }
        if self.modifiers.shift {
            f.write_str("Shift+")?;
        }
        if self.modifiers.meta {
            f.write_str("Meta+")?;
        }
        write!(f, "{}", self.key)
    }
}

impl fmt::Display for ChordKey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Letter(letter) => write!(f, "Key{letter}"),
            Self::Digit(digit) => write!(f, "Digit{digit}"),
            Self::Function(number) => write!(f, "F{number}"),
            named => {
                let (name, _) = NAMED_KEYS.iter().find(|(_, key)| key == named).unwrap();
                f.write_str(name)
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn chord(text: &str) -> KeyChord {
        text.parse().unwrap()
    }

    #[test]
    fn parses_the_three_factory_bindings() {
        assert_eq!(
            chord("Ctrl+Shift+KeyE"),
            KeyChord {
                key: ChordKey::Letter('E'),
                modifiers: ChordModifiers {
                    ctrl: true,
                    shift: true,
                    ..ChordModifiers::default()
                },
            }
        );
        assert_eq!(
            chord("Ctrl+Shift+Space"),
            KeyChord {
                key: ChordKey::Space,
                modifiers: ChordModifiers {
                    ctrl: true,
                    shift: true,
                    ..ChordModifiers::default()
                },
            }
        );
        assert_eq!(
            chord("Ctrl+Period"),
            KeyChord {
                key: ChordKey::Period,
                modifiers: ChordModifiers {
                    ctrl: true,
                    ..ChordModifiers::default()
                },
            }
        );
    }

    #[test]
    fn formats_back_to_the_canonical_spelling() {
        for text in [
            "Ctrl+Shift+KeyE",
            "Ctrl+Shift+Space",
            "Ctrl+Period",
            "Ctrl+Shift+KeyF",
            "Ctrl+F9",
            "Alt+Shift+Digit3",
            "Meta+Quote",
            "Ctrl+Alt+Shift+Meta+Backquote",
            "F12",
        ] {
            assert_eq!(chord(text).to_string(), text);
        }
    }

    #[test]
    fn round_trips_through_parse_and_format() {
        for text in [
            "shift+ctrl+keye",
            "CONTROL+PERIOD",
            "Ctrl+Shift+Space",
            "option+command+semicolon",
            "Win+Slash",
            "super+bracketleft",
        ] {
            let parsed = chord(text);
            assert_eq!(parsed.to_string().parse::<KeyChord>().unwrap(), parsed);
        }
    }

    #[test]
    fn rejects_inputs_that_are_not_chords() {
        for text in [
            "",
            "   ",
            "Ctrl+",
            "Ctrl+Shift",
            "Ctrl++Space",
            "+Space",
            "KeyE+Ctrl",
            "Ctrl+KeyEE",
            "Ctrl+Bogus",
            "Ctrl+Control+Space",
            "Shift+Shift+KeyE",
            "Key13",
            "Digit",
            "F13",
            "key e",
        ] {
            assert!(text.parse::<KeyChord>().is_err(), "{text:?} must fail");
        }
    }

    #[test]
    fn ctrl_and_control_are_the_same_modifier() {
        assert_eq!(chord("Ctrl+Period"), chord("Control+Period"));
        assert_eq!(chord("ctrl+period"), chord("CONTROL+PERIOD"));
    }

    #[test]
    fn modifier_order_does_not_matter() {
        assert_eq!(chord("Ctrl+Shift+Space"), chord("Shift+Ctrl+Space"));
        assert_eq!(
            chord("Ctrl+Alt+Shift+Meta+KeyC"),
            chord("Meta+Shift+Alt+Ctrl+KeyC")
        );
    }

    #[test]
    fn modifier_aliases_unify() {
        assert_eq!(chord("Alt+Space"), chord("Option+Space"));
        assert_eq!(chord("Opt+Space"), chord("Option+Space"));
        assert_eq!(chord("Meta+Space"), chord("Win+Space"));
        assert_eq!(chord("Meta+Space"), chord("Cmd+Space"));
        assert_eq!(chord("Meta+Space"), chord("Command+Space"));
        assert_eq!(chord("Meta+Space"), chord("Super+Space"));
    }

    #[test]
    fn key_names_cover_letters_digits_function_and_named_keys() {
        assert_eq!(chord("KeyA").key, ChordKey::Letter('A'));
        assert_eq!(chord("keyz").key, ChordKey::Letter('Z'));
        assert_eq!(chord("Digit0").key, ChordKey::Digit('0'));
        assert_eq!(chord("digit9").key, ChordKey::Digit('9'));
        assert_eq!(chord("F1").key, ChordKey::Function(1));
        assert_eq!(chord("f12").key, ChordKey::Function(12));
        for (name, key) in [
            ("Space", ChordKey::Space),
            ("Semicolon", ChordKey::Semicolon),
            ("Equal", ChordKey::Equal),
            ("Comma", ChordKey::Comma),
            ("Minus", ChordKey::Minus),
            ("Period", ChordKey::Period),
            ("Slash", ChordKey::Slash),
            ("Backquote", ChordKey::Backquote),
            ("BracketLeft", ChordKey::BracketLeft),
            ("Backslash", ChordKey::Backslash),
            ("BracketRight", ChordKey::BracketRight),
            ("Quote", ChordKey::Quote),
        ] {
            assert_eq!(chord(name).key, key);
            assert_eq!(chord(&name.to_uppercase()).key, key);
        }
    }

    #[test]
    fn distinct_chords_do_not_conflict() {
        assert!(!chord("Ctrl+Period").conflicts_with(&chord("Ctrl+Comma")));
        assert!(!chord("Ctrl+Space").conflicts_with(&chord("Ctrl+Shift+Space")));
        assert!(chord("Ctrl+Period").conflicts_with(&chord("Control+Period")));
        assert!(chord("Ctrl+Shift+KeyE").conflicts_with(&chord("shift+ctrl+keye")));
    }

    #[test]
    fn a_bare_key_is_a_chord_without_modifiers() {
        assert_eq!(chord("Space").modifiers, ChordModifiers::default());
        assert_eq!(chord("Period").to_string(), "Period");
    }

    #[test]
    fn whitespace_around_the_value_is_ignored() {
        assert_eq!(chord("  Ctrl+Period  "), chord("Ctrl+Period"));
    }
}
