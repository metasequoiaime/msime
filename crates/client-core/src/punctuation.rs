//! Host-context punctuation decisions shared by native clients.

use crate::preferences::{BracketStyle, Preferences, PunctuationLock, QuoteStyle};

/// 解析后的引号样式：`corner_when_traditional` 已按繁体开关定成两者之一。
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, serde::Serialize)]
#[serde(rename_all = "snake_case")]
pub enum QuoteMarks {
    #[default]
    Curly,
    Corner,
}

/// 中文标点下引号键和方头括号键实际出哪一组字、`\` 和 `/` 谁出顿号。引擎、View 和 Windows Server 都只认解析好的值；View 里序列化成 `{"quotes": "curly"|"corner", "brackets": "solid"|"hollow", "enumeration_comma_backslash": bool, "enumeration_comma_slash": bool}`。
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize)]
pub struct PunctuationMarks {
    pub quotes: QuoteMarks,
    pub brackets: BracketStyle,
    /// `\` 键在中文标点下出顿号「、」。
    pub enumeration_comma_backslash: bool,
    /// `/` 键在中文标点下出顿号「、」。
    pub enumeration_comma_slash: bool,
}

impl Default for PunctuationMarks {
    /// 与偏好的缺省一致：`\` 出顿号、`/` 是命令模式触发键，不出顿号。
    fn default() -> Self {
        Self {
            quotes: QuoteMarks::Curly,
            brackets: BracketStyle::Solid,
            enumeration_comma_backslash: true,
            enumeration_comma_slash: false,
        }
    }
}

/// 从偏好算出生效的标点样式与顿号键，这是唯一的解析处：`corner_when_traditional` 在繁体输出打开时是直角引号，否则是弯引号；「至少留一个」在这里归一，两个顿号键都关掉时按「`\` 开」读，采用 Windows IPC 无效值的反斜杠回退。Windows Server 读偏好 JSON 时照抄样式规则（`server_main.cpp` 的 `tsf_local_config`）。
pub fn punctuation_marks(preferences: &Preferences) -> PunctuationMarks {
    let corner = match preferences.quote_style {
        QuoteStyle::Curly => false,
        QuoteStyle::Corner => true,
        QuoteStyle::CornerWhenTraditional => preferences.traditional_chinese_output,
    };
    let slash = preferences.enumeration_comma_slash;
    PunctuationMarks {
        quotes: if corner {
            QuoteMarks::Corner
        } else {
            QuoteMarks::Curly
        },
        brackets: preferences.bracket_style,
        enumeration_comma_backslash: preferences.enumeration_comma_backslash || !slash,
        enumeration_comma_slash: slash,
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PunctuationRoute {
    Engine,
    Ascii,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PunctuationContext {
    pub character: u8,
    pub preceding: Option<char>,
    pub host_context_available: bool,
    pub has_composition: bool,
    pub chinese_punctuation: bool,
    pub smart_punctuation: bool,
    /// 数字后、字母后的 `,` `.` `:` 保留 ASCII。拆成两个开关，是因为版本号和英文句子要的答案可能不同；两者都跟着智能标点默认开，用户可以单独关掉一半，宿主不能无视它们。
    pub direct_digit: bool,
    pub direct_letter: bool,
    pub lock: PunctuationLock,
    /// 大写锁定打开、且用户打开了 `caps_lock_ascii_punctuation`。为真时没有组字的标点和英文模式一样走 ASCII，`lock` 为 `Chinese` 时仍归 Engine。
    pub caps_lock_ascii: bool,
}

/// 只在宿主能安全按文档上下文决定时才选显式 ASCII 路线。组字中、本地/日文/英文模式、韩文（标点总是半角 ASCII），以及共享智能标点约定和大写锁定开关都不管的标点，仍由 Engine 决定。
pub fn route(context: PunctuationContext) -> PunctuationRoute {
    if context.has_composition || !context.host_context_available {
        return PunctuationRoute::Engine;
    }
    match context.lock {
        PunctuationLock::Chinese => return PunctuationRoute::Engine,
        PunctuationLock::English => return PunctuationRoute::Ascii,
        PunctuationLock::Follow => {}
    }
    if context.caps_lock_ascii {
        return PunctuationRoute::Ascii;
    }
    let direct = context.preceding.is_some_and(|value| {
        (value.is_ascii_digit() && context.direct_digit)
            || (value.is_ascii_alphabetic() && context.direct_letter)
    });
    if context.chinese_punctuation
        && context.smart_punctuation
        && matches!(context.character, b',' | b'.' | b':')
        && direct
    {
        PunctuationRoute::Ascii
    } else {
        PunctuationRoute::Engine
    }
}

/// The Chinese mark an ASCII one converts to, for the three the repeat gesture covers.
fn chinese_mark(ascii: u8) -> Option<char> {
    match ascii {
        b',' => Some('，'),
        b'.' => Some('。'),
        b':' => Some('：'),
        _ => None,
    }
}

/// The full-width twin of an ASCII mark, for hosts that commit one instead.
fn full_width_mark(ascii: u8) -> char {
    match ascii {
        b',' => '\u{ff0c}',
        b'.' => '\u{ff0e}',
        b':' => '\u{ff1a}',
        _ => ascii as char,
    }
}

/// What the previous press of a smart-punctuation key left behind.
///
/// Armed only when the press actually committed the ASCII mark that was asked for - or its
/// full-width twin, which is what a full-width host commits. `editor_generation` is the host's
/// own counter for "the caret is still where it was and the document is still the same one".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RepeatSnapshot {
    pub ascii: u8,
    /// The scalar the first press committed.
    pub committed: char,
    pub timestamp_ms: u64,
    pub editor_generation: u64,
}

/// Arm the repeat gesture after a commit, or decline to.
pub fn arm_repeat(
    ascii: u8,
    commit: &str,
    timestamp_ms: u64,
    editor_generation: u64,
) -> Option<RepeatSnapshot> {
    chinese_mark(ascii)?;
    let committed = commit.chars().next_back()?;
    if committed != ascii as char && committed != full_width_mark(ascii) {
        return None;
    }
    Some(RepeatSnapshot {
        ascii,
        committed,
        timestamp_ms,
        editor_generation,
    })
}

/// How long after the first press the second one still counts as a correction.
pub const REPEAT_WINDOW_MS: u64 = 2000;

/// What the host knows when the same punctuation key is pressed again.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RepeatContext {
    pub ascii: u8,
    pub preceding: Option<char>,
    pub timestamp_ms: u64,
    pub editor_generation: u64,
    pub smart_punctuation: bool,
    pub repeat_enabled: bool,
    pub has_composition: bool,
    pub candidate_count: usize,
}

/// Pressing the same mark again within the window replaces the ASCII one with Chinese.
///
/// The user asked for ASCII by context and is now saying they meant the Chinese mark after all,
/// so this only fires when the document still holds exactly what the first press committed: a
/// composition, any candidate on screen, a moved caret or a different editor all mean the gesture
/// is about something else.
pub fn should_replace_repeat(
    snapshot: Option<RepeatSnapshot>,
    context: RepeatContext,
) -> Option<char> {
    if !context.smart_punctuation
        || !context.repeat_enabled
        || context.has_composition
        || context.candidate_count != 0
    {
        return None;
    }
    let snapshot = snapshot?;
    if snapshot.ascii != context.ascii
        || snapshot.editor_generation != context.editor_generation
        || context.preceding != Some(snapshot.committed)
    {
        return None;
    }
    if context.timestamp_ms.checked_sub(snapshot.timestamp_ms)? > REPEAT_WINDOW_MS {
        return None;
    }
    chinese_mark(context.ascii)
}

/// `、` 的 ASCII 孪生按当前开着的顿号键判（契约 `enumerationComma` 段）：`source` 是产生它的键，开着时原样转回；不是顿号键（候选上屏）时按契约序取开着的那个——`\` 优先、`/` 其次；两个都关按归一的 `\`。
pub fn enumeration_comma_ascii(source: u8, marks: &PunctuationMarks) -> u8 {
    match source {
        b'\\' if marks.enumeration_comma_backslash => b'\\',
        b'/' if marks.enumeration_comma_slash => b'/',
        _ if marks.enumeration_comma_backslash => b'\\',
        _ if marks.enumeration_comma_slash => b'/',
        _ => b'\\',
    }
}

/// The ASCII twin of a Chinese punctuation mark, or `None` where there is none.
///
/// Both quote directions map to the same ASCII quote: a straight quote has no handedness. 直角引号和空心方头括号是同一个键的另一组字（`quote_style`、`bracket_style`），同样转回那个键。顿号按 `source`（产生它的键，没有时传 0）和 `marks` 里开着的顿号键定。
pub fn ascii_for_chinese_mark(chinese: char, source: u8, marks: &PunctuationMarks) -> Option<u8> {
    Some(match chinese {
        '。' => b'.',
        '\u{ff0c}' => b',',
        '！' => b'!',
        '？' => b'?',
        '；' => b';',
        '\u{ff1a}' => b':',
        '、' => enumeration_comma_ascii(source, marks),
        '\u{201c}' | '\u{201d}' => b'"',
        '\u{2018}' | '\u{2019}' => b'\'',
        '『' | '』' => b'"',
        '「' | '」' => b'\'',
        '【' | '〖' => b'[',
        '】' | '〗' => b']',
        '《' => b'<',
        '》' => b'>',
        '（' => b'(',
        '）' => b')',
        _ => return None,
    })
}

/// A Chinese mark that a following space would rewrite as ASCII.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SpaceConvertSnapshot {
    pub chinese: char,
    /// Resolved when arming, so the conversion cannot disagree with what armed it.
    pub ascii: u8,
    pub editor_generation: u64,
}

/// Arm the space conversion after a commit, or decline to.
///
/// An auto-closed pair is refused: the caret sits between the two marks, so the character before
/// it is the opening one and rewriting that would break the pair. A commit of more than one scalar
/// is refused too - the mark has to be the last thing typed for the space to be about it.
pub fn arm_space_convert(
    committed: &str,
    auto_closed_pair: bool,
    smart_punctuation: bool,
    space_convert: bool,
    editor_generation: u64,
    source: u8,
    marks: &PunctuationMarks,
) -> Option<SpaceConvertSnapshot> {
    if !smart_punctuation || !space_convert || auto_closed_pair {
        return None;
    }
    let mut characters = committed.chars();
    let chinese = characters.next()?;
    if characters.next().is_some() {
        return None;
    }
    Some(SpaceConvertSnapshot {
        chinese,
        ascii: ascii_for_chinese_mark(chinese, source, marks)?,
        editor_generation,
    })
}

/// A space right after a committed Chinese mark rewrites it as ASCII and is swallowed.
///
/// Swallowing the space is the whole gesture: the user is correcting the mark they just typed,
/// not typing a mark and then a space. Nothing here is time-limited - the arming survives as long
/// as the caret has not moved and the editor has not changed - because the decision re-reads what
/// is actually before the caret and declines when it disagrees.
pub fn decide_space_convert(
    snapshot: Option<SpaceConvertSnapshot>,
    character: u8,
    preceding: Option<char>,
    has_composition: bool,
    editor_generation: u64,
) -> Option<u8> {
    let snapshot = snapshot?;
    if character != b' ' || has_composition || snapshot.editor_generation != editor_generation {
        return None;
    }
    if preceding != Some(snapshot.chinese) {
        return None;
    }
    Some(snapshot.ascii)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn context(character: u8, preceding: Option<char>) -> PunctuationContext {
        PunctuationContext {
            character,
            preceding,
            host_context_available: true,
            has_composition: false,
            chinese_punctuation: true,
            smart_punctuation: true,
            direct_digit: true,
            direct_letter: true,
            lock: PunctuationLock::Follow,
            caps_lock_ascii: false,
        }
    }

    #[test]
    fn caps_lock_switch_sends_every_idle_mark_to_ascii() {
        for character in *b",.:;?!\\'\"()[]<>^_$~`/@{}" {
            for preceding in [None, Some('中'), Some('A')] {
                let mut value = context(character, preceding);
                value.smart_punctuation = false;
                value.caps_lock_ascii = true;
                assert_eq!(
                    route(value),
                    PunctuationRoute::Ascii,
                    "{}",
                    character as char
                );
            }
        }
    }

    #[test]
    fn caps_lock_switch_leaves_composition_chinese_lock_and_other_modes_alone() {
        let mut value = context(b',', None);
        value.caps_lock_ascii = true;
        value.has_composition = true;
        assert_eq!(route(value), PunctuationRoute::Engine);
        value.has_composition = false;
        value.host_context_available = false;
        assert_eq!(route(value), PunctuationRoute::Engine);
        value.host_context_available = true;
        value.lock = PunctuationLock::Chinese;
        assert_eq!(route(value), PunctuationRoute::Engine);
        // 开关关着时，大写锁定不影响原来的判断：汉字后的逗号仍交给 Engine。
        let mut off = context(b',', Some('中'));
        off.caps_lock_ascii = false;
        assert_eq!(route(off), PunctuationRoute::Engine);
    }

    #[test]
    fn ascii_alphanumeric_context_uses_ascii_for_supported_marks() {
        for preceding in ['0', '9', 'a', 'z', 'A', 'Z'] {
            for character in *b",.:" {
                assert_eq!(
                    route(context(character, Some(preceding))),
                    PunctuationRoute::Ascii
                );
            }
        }
    }

    #[test]
    fn the_shipped_defaults_do_what_the_switch_says_they_do() {
        // 智能标点在所有平台上默认开：只看默认偏好（不用上面把三个开关都设好的夹具），数字后的 `,` `.` `:` 都走 ASCII，`0.10.1` 不会变成 `0。10。1`；字母后同样；中文、空格或没有前文时仍归 Engine 出中文标点。
        let defaults = crate::preferences::Preferences::default();
        let shipped = |character: u8, preceding: Option<char>| {
            let mut value = context(character, preceding);
            value.chinese_punctuation = defaults.chinese_punctuation;
            value.smart_punctuation = defaults.smart_punctuation;
            value.direct_digit = defaults.smart_punctuation_direct_digit;
            value.direct_letter = defaults.smart_punctuation_direct_letter;
            value.lock = defaults.punctuation_lock;
            value.caps_lock_ascii = false;
            value
        };
        for character in *b",.:" {
            assert_eq!(
                route(shipped(character, Some('1'))),
                PunctuationRoute::Ascii
            );
            assert_eq!(
                route(shipped(character, Some('a'))),
                PunctuationRoute::Ascii
            );
            for preceding in [Some('计'), Some(' '), Some('。'), None] {
                assert_eq!(
                    route(shipped(character, preceding)),
                    PunctuationRoute::Engine,
                    "{} after {preceding:?}",
                    character as char
                );
            }
        }
        // 重复标点转中文同样默认开：数字后上屏了 ASCII 句点，两秒内再按一次就换回「。」。
        let armed = arm_repeat(b'.', ".", 1_000, 7);
        let again = RepeatContext {
            ascii: b'.',
            preceding: Some('.'),
            timestamp_ms: 1_500,
            editor_generation: 7,
            smart_punctuation: defaults.smart_punctuation,
            repeat_enabled: defaults.smart_punctuation_repeat,
            has_composition: false,
            candidate_count: 0,
        };
        assert_eq!(should_replace_repeat(armed, again), Some('。'));
        // Turning the parent off still silences both halves, whatever they say.
        let mut parent_off = shipped(b',', Some('1'));
        parent_off.smart_punctuation = false;
        parent_off.direct_digit = true;
        parent_off.direct_letter = true;
        assert_eq!(route(parent_off), PunctuationRoute::Engine);
    }

    #[test]
    fn unsupported_or_non_ascii_context_stays_with_engine() {
        for preceding in [None, Some('中'), Some(' '), Some('_')] {
            assert_eq!(route(context(b',', preceding)), PunctuationRoute::Engine);
        }
        assert_eq!(route(context(b'?', Some('a'))), PunctuationRoute::Engine);
    }

    #[test]
    fn lock_and_composition_take_precedence() {
        let mut value = context(b',', Some('a'));
        value.lock = PunctuationLock::Chinese;
        assert_eq!(route(value), PunctuationRoute::Engine);
        value.lock = PunctuationLock::English;
        assert_eq!(route(value), PunctuationRoute::Ascii);
        value.has_composition = true;
        assert_eq!(route(value), PunctuationRoute::Engine);
    }

    #[test]
    fn each_direct_switch_only_answers_for_its_own_kind_of_neighbour() {
        // Off is the Windows baseline and the shipped default, so a host that reads neither switch was
        // converting punctuation for every user who had asked for none of it.
        let mut value = context(b'.', Some('7'));
        value.direct_digit = false;
        value.direct_letter = true;
        assert_eq!(route(value), PunctuationRoute::Engine);

        let mut value = context(b'.', Some('a'));
        value.direct_digit = true;
        value.direct_letter = false;
        assert_eq!(route(value), PunctuationRoute::Engine);

        // A version number and an English sentence are the two cases the pair exists to separate.
        let mut value = context(b'.', Some('7'));
        value.direct_letter = false;
        assert_eq!(route(value), PunctuationRoute::Ascii);
        let mut value = context(b',', Some('z'));
        value.direct_digit = false;
        assert_eq!(route(value), PunctuationRoute::Ascii);

        for preceding in ['7', 'a'] {
            let mut value = context(b':', Some(preceding));
            value.direct_digit = false;
            value.direct_letter = false;
            assert_eq!(route(value), PunctuationRoute::Engine);
        }
    }

    #[test]
    fn disabled_or_unavailable_host_context_stays_with_engine() {
        let mut value = context(b'.', Some('7'));
        value.smart_punctuation = false;
        assert_eq!(route(value), PunctuationRoute::Engine);
        value.smart_punctuation = true;
        value.chinese_punctuation = false;
        assert_eq!(route(value), PunctuationRoute::Engine);
        value.chinese_punctuation = true;
        value.host_context_available = false;
        assert_eq!(route(value), PunctuationRoute::Engine);
    }

    #[test]
    fn repeat_arms_only_on_the_mark_the_press_actually_committed() {
        // 。 is the only one of the three whose Chinese form differs from its full-width form:
        // ，and ：are U+FF0C and U+FF1A either way. So this is the case that can tell "the host
        // committed the Chinese mark" apart from "the host commits full-width".
        assert!(
            arm_repeat(b'.', "。", 0, 1).is_none(),
            "committed the Chinese mark, so the ASCII press never landed"
        );
        assert!(
            arm_repeat(b'?', "?", 0, 1).is_none(),
            "not one of the three marks"
        );
        assert!(arm_repeat(b',', "", 0, 1).is_none());
        assert_eq!(arm_repeat(b',', ",", 5, 1).unwrap().committed, ',');
        // A full-width host commits the full-width twin, and the gesture still belongs to it.
        assert_eq!(
            arm_repeat(b'.', "\u{ff0e}", 5, 1).unwrap().committed,
            '\u{ff0e}'
        );
        // Only the last scalar matters: the commit may carry the word the mark ended.
        assert_eq!(arm_repeat(b':', "ok:", 5, 1).unwrap().committed, ':');
    }

    #[test]
    fn repeat_replaces_inside_the_window_and_only_then() {
        let armed = arm_repeat(b',', ",", 1_000, 7);
        let replace = |timestamp_ms, editor_generation, preceding| {
            should_replace_repeat(
                armed,
                RepeatContext {
                    ascii: b',',
                    preceding,
                    timestamp_ms,
                    editor_generation,
                    smart_punctuation: true,
                    repeat_enabled: true,
                    has_composition: false,
                    candidate_count: 0,
                },
            )
        };
        assert_eq!(replace(1_500, 7, Some(',')), Some('，'));
        assert_eq!(replace(1_000 + REPEAT_WINDOW_MS, 7, Some(',')), Some('，'));
        assert_eq!(
            replace(1_001 + REPEAT_WINDOW_MS, 7, Some(',')),
            None,
            "past the window"
        );
        assert_eq!(replace(900, 7, Some(',')), None, "clock went backwards");
        assert_eq!(replace(1_500, 8, Some(',')), None, "another editor");
        assert_eq!(
            replace(1_500, 7, Some('a')),
            None,
            "the mark is not what is there"
        );
        assert_eq!(replace(1_500, 7, None), None, "nothing before the caret");
    }

    #[test]
    fn repeat_declines_while_the_engine_owns_the_gesture() {
        let armed = arm_repeat(b'.', ".", 0, 1);
        let repeat_context =
            |ascii, smart_punctuation, repeat_enabled, has_composition, candidate_count| {
                RepeatContext {
                    ascii,
                    preceding: Some('.'),
                    timestamp_ms: 10,
                    editor_generation: 1,
                    smart_punctuation,
                    repeat_enabled,
                    has_composition,
                    candidate_count,
                }
            };
        let replace = |smart, repeat, composing, candidates| {
            should_replace_repeat(
                armed,
                repeat_context(b'.', smart, repeat, composing, candidates),
            )
        };
        assert_eq!(replace(true, true, false, 0), Some('。'));
        assert_eq!(
            replace(false, true, false, 0),
            None,
            "smart punctuation off"
        );
        assert_eq!(
            replace(true, false, false, 0),
            None,
            "the switch itself is off"
        );
        assert_eq!(replace(true, true, true, 0), None, "mid composition");
        assert_eq!(replace(true, true, false, 3), None, "candidates on screen");
        // A different key than the one that armed it is not this gesture.
        assert_eq!(
            should_replace_repeat(armed, repeat_context(b',', true, true, false, 0)),
            None
        );
    }

    #[test]
    fn space_conversion_covers_the_marks_the_source_maps_and_no_others() {
        let marks = PunctuationMarks::default();
        for (chinese, ascii) in [
            ('。', b'.'),
            ('\u{ff0c}', b','),
            ('！', b'!'),
            ('？', b'?'),
            ('；', b';'),
            ('\u{ff1a}', b':'),
            ('【', b'['),
            ('】', b']'),
            ('《', b'<'),
            ('》', b'>'),
            ('（', b'('),
            ('）', b')'),
        ] {
            assert_eq!(
                ascii_for_chinese_mark(chinese, 0, &marks),
                Some(ascii),
                "{chinese}"
            );
        }
        // Both directions of a quote map to the one straight quote, which has no handedness.
        assert_eq!(ascii_for_chinese_mark('\u{201c}', 0, &marks), Some(b'"'));
        assert_eq!(ascii_for_chinese_mark('\u{201d}', 0, &marks), Some(b'"'));
        assert_eq!(ascii_for_chinese_mark('\u{2018}', 0, &marks), Some(b'\''));
        assert_eq!(ascii_for_chinese_mark('\u{2019}', 0, &marks), Some(b'\''));
        // 直角引号与空心括号转回同一个键。
        for (chinese, ascii) in [
            ('「', b'\''),
            ('」', b'\''),
            ('『', b'"'),
            ('』', b'"'),
            ('〖', b'['),
            ('〗', b']'),
        ] {
            assert_eq!(
                ascii_for_chinese_mark(chinese, 0, &marks),
                Some(ascii),
                "{chinese}"
            );
        }
        for other in ['中', 'a', '.', '…', '—'] {
            assert_eq!(ascii_for_chinese_mark(other, 0, &marks), None, "{other}");
        }
    }

    /// 顿号空格反查随当前开着的键判：按下的键开着时转回它，否则 `\` 优先、`/` 其次；关掉 `/` 后 `/` 不再被当成顿号来源。
    #[test]
    fn enumeration_comma_space_reverse_follows_the_enabled_keys() {
        let marks = |backslash, slash| PunctuationMarks {
            enumeration_comma_backslash: backslash,
            enumeration_comma_slash: slash,
            ..PunctuationMarks::default()
        };
        // 缺省「只有 `\`」：按 `\` 出的顿号转回 `\`，候选上屏的顿号也转回 `\`，`/` 不再是来源。
        let backslash_only = marks(true, false);
        for source in [b'\\', b'/', 0] {
            assert_eq!(
                ascii_for_chinese_mark('、', source, &backslash_only),
                Some(b'\\'),
                "source {source}"
            );
            assert_eq!(
                enumeration_comma_ascii(source, &backslash_only),
                b'\\',
                "source {source}"
            );
        }
        // 只有 `/`：转回 `/`，按 `\` 出的顿号也归到开着的那个键。
        let slash_only = marks(false, true);
        for source in [b'\\', b'/', 0] {
            assert_eq!(
                enumeration_comma_ascii(source, &slash_only),
                b'/',
                "source {source}"
            );
        }
        // 两个都开：按下的键开着时转回它，候选上屏按契约序归 `\`。
        let both = marks(true, true);
        assert_eq!(enumeration_comma_ascii(b'\\', &both), b'\\');
        assert_eq!(enumeration_comma_ascii(b'/', &both), b'/');
        assert_eq!(enumeration_comma_ascii(0, &both), b'\\');
        // 两个都关（未经归一的值）：按归一的 `\`。
        assert_eq!(enumeration_comma_ascii(b'/', &marks(false, false)), b'\\');
        // 反查落到快照里：关掉 `/` 后，`、` 的空格转换不再把 `/` 转回。
        let armed = arm_space_convert("、", false, true, true, 7, b'\\', &backslash_only);
        assert_eq!(
            decide_space_convert(armed, b' ', Some('、'), false, 7),
            Some(b'\\')
        );
        let armed = arm_space_convert("、", false, true, true, 7, b'/', &slash_only);
        assert_eq!(
            decide_space_convert(armed, b' ', Some('、'), false, 7),
            Some(b'/')
        );
    }

    #[test]
    fn space_conversion_arms_only_a_lone_mark_the_user_finished_typing() {
        let marks = PunctuationMarks::default();
        assert!(arm_space_convert("。", false, true, true, 1, 0, &marks).is_some());
        assert!(
            arm_space_convert("。", true, true, true, 1, 0, &marks).is_none(),
            "auto-closed pair"
        );
        assert!(
            arm_space_convert("好。", false, true, true, 1, 0, &marks).is_none(),
            "not the only scalar"
        );
        assert!(
            arm_space_convert("中", false, true, true, 1, 0, &marks).is_none(),
            "not a mark"
        );
        assert!(arm_space_convert("", false, true, true, 1, 0, &marks).is_none());
        assert!(
            arm_space_convert("。", false, false, true, 1, 0, &marks).is_none(),
            "smart punctuation off"
        );
        assert!(
            arm_space_convert("。", false, true, false, 1, 0, &marks).is_none(),
            "the switch is off"
        );
    }

    #[test]
    fn space_converts_only_when_the_mark_is_still_there() {
        let armed = arm_space_convert("，", false, true, true, 4, 0, &PunctuationMarks::default());
        assert_eq!(
            decide_space_convert(armed, b' ', Some('\u{ff0c}'), false, 4),
            Some(b',')
        );
        assert_eq!(
            decide_space_convert(armed, b'a', Some('\u{ff0c}'), false, 4),
            None,
            "not a space"
        );
        assert_eq!(
            decide_space_convert(armed, b' ', Some('\u{ff0c}'), true, 4),
            None,
            "mid composition"
        );
        assert_eq!(
            decide_space_convert(armed, b' ', Some('\u{ff0c}'), false, 5),
            None,
            "another editor"
        );
        // The arming says what was committed, not what is still there; anything may have happened.
        assert_eq!(
            decide_space_convert(armed, b' ', Some('好'), false, 4),
            None
        );
        assert_eq!(decide_space_convert(armed, b' ', None, false, 4), None);
        assert_eq!(
            decide_space_convert(None, b' ', Some('\u{ff0c}'), false, 4),
            None
        );
    }

    #[test]
    fn punctuation_marks_resolve_corner_when_traditional() {
        let marks = |quote_style, bracket_style, traditional| {
            punctuation_marks(&Preferences {
                quote_style,
                bracket_style,
                traditional_chinese_output: traditional,
                ..Preferences::default()
            })
        };
        let defaults = punctuation_marks(&Preferences::default());
        assert_eq!(defaults, PunctuationMarks::default());
        for traditional in [false, true] {
            assert_eq!(
                marks(QuoteStyle::Curly, BracketStyle::Solid, traditional).quotes,
                QuoteMarks::Curly
            );
            assert_eq!(
                marks(QuoteStyle::Corner, BracketStyle::Solid, traditional).quotes,
                QuoteMarks::Corner
            );
            assert_eq!(
                marks(QuoteStyle::Curly, BracketStyle::Hollow, traditional).brackets,
                BracketStyle::Hollow
            );
        }
        assert_eq!(
            marks(
                QuoteStyle::CornerWhenTraditional,
                BracketStyle::Solid,
                false
            )
            .quotes,
            QuoteMarks::Curly
        );
        assert_eq!(
            marks(
                QuoteStyle::CornerWhenTraditional,
                BracketStyle::Hollow,
                true
            ),
            PunctuationMarks {
                quotes: QuoteMarks::Corner,
                brackets: BracketStyle::Hollow,
                ..PunctuationMarks::default()
            }
        );
    }

    /// 顿号自定义（契约 `enumerationComma` 段）：`\` 默认开、`/` 默认关；手改文档把两个都关掉时归一成「`\` 开」，采用 Windows IPC 无效值的反斜杠回退。
    #[test]
    fn enumeration_comma_keys_keep_at_least_one() {
        let marks = |backslash, slash| {
            punctuation_marks(&Preferences {
                enumeration_comma_backslash: backslash,
                enumeration_comma_slash: slash,
                ..Preferences::default()
            })
        };
        let defaults = marks(true, false);
        assert!(defaults.enumeration_comma_backslash);
        assert!(!defaults.enumeration_comma_slash);
        // 各自独立开关。
        assert!(marks(false, true).enumeration_comma_slash);
        assert!(!marks(false, true).enumeration_comma_backslash);
        assert!(marks(true, true).enumeration_comma_backslash);
        assert!(marks(true, true).enumeration_comma_slash);
        // 两个都关按「`\` 开」读。
        let normalized = marks(false, false);
        assert!(normalized.enumeration_comma_backslash);
        assert!(!normalized.enumeration_comma_slash);
        assert_eq!(normalized, marks(true, false));
    }
}
