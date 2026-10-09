//! 罗马字与假名转换（schemes-lang.md §5.1-§5.3，`romaji_converter.cpp`）。
//!
//! 罗马字表、`n`、促音与待定尾部规则供 provider 的词库查询共用，保留输入法专用实现。
//! 平假名转片假名直接按 Unicode 位移写入；测试用固定版本 `wana_kana` 对照原行为。

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// 以 `romaji_converter.cpp:11-44` 为底，补上微软和 Google 日文输入法都认、C++ 表里缺的拼法：促音 `xtu`/`ltu`，拗音 `zya`/`zyu`/`zyo`（じゃ行）和 `dya`/`dyu`/`dyo`（ぢゃ行）。缺 `xtu` 时它整段停在待定字母里，用户打不出っ。反查（[`hiragana_to_romaji`]）先取最长拼法、再按字母序，っ 仍是 `xtsu`，じゃ 仍是 `jya`。
const ROMAJI_TABLE: &[(&str, &str)] = &[
    ("a", "あ"),
    ("i", "い"),
    ("u", "う"),
    ("e", "え"),
    ("o", "お"),
    ("ka", "か"),
    ("ki", "き"),
    ("ku", "く"),
    ("ke", "け"),
    ("ko", "こ"),
    ("ga", "が"),
    ("gi", "ぎ"),
    ("gu", "ぐ"),
    ("ge", "げ"),
    ("go", "ご"),
    ("sa", "さ"),
    ("shi", "し"),
    ("si", "し"),
    ("su", "す"),
    ("se", "せ"),
    ("so", "そ"),
    ("za", "ざ"),
    ("ji", "じ"),
    ("zi", "じ"),
    ("zu", "ず"),
    ("ze", "ぜ"),
    ("zo", "ぞ"),
    ("ta", "た"),
    ("chi", "ち"),
    ("ti", "ち"),
    ("tsu", "つ"),
    ("tu", "つ"),
    ("te", "て"),
    ("to", "と"),
    ("da", "だ"),
    ("di", "ぢ"),
    ("du", "づ"),
    ("de", "で"),
    ("do", "ど"),
    ("na", "な"),
    ("ni", "に"),
    ("nu", "ぬ"),
    ("ne", "ね"),
    ("no", "の"),
    ("ha", "は"),
    ("hi", "ひ"),
    ("fu", "ふ"),
    ("hu", "ふ"),
    ("he", "へ"),
    ("ho", "ほ"),
    ("ba", "ば"),
    ("bi", "び"),
    ("bu", "ぶ"),
    ("be", "べ"),
    ("bo", "ぼ"),
    ("pa", "ぱ"),
    ("pi", "ぴ"),
    ("pu", "ぷ"),
    ("pe", "ぺ"),
    ("po", "ぽ"),
    ("ma", "ま"),
    ("mi", "み"),
    ("mu", "む"),
    ("me", "め"),
    ("mo", "も"),
    ("ya", "や"),
    ("yu", "ゆ"),
    ("yo", "よ"),
    ("ra", "ら"),
    ("ri", "り"),
    ("ru", "る"),
    ("re", "れ"),
    ("ro", "ろ"),
    ("wa", "わ"),
    ("wo", "を"),
    ("nn", "ん"),
    ("kya", "きゃ"),
    ("kyu", "きゅ"),
    ("kyo", "きょ"),
    ("gya", "ぎゃ"),
    ("gyu", "ぎゅ"),
    ("gyo", "ぎょ"),
    ("sha", "しゃ"),
    ("shu", "しゅ"),
    ("sho", "しょ"),
    ("sya", "しゃ"),
    ("syu", "しゅ"),
    ("syo", "しょ"),
    ("ja", "じゃ"),
    ("ju", "じゅ"),
    ("jo", "じょ"),
    ("jya", "じゃ"),
    ("jyu", "じゅ"),
    ("jyo", "じょ"),
    ("zya", "じゃ"),
    ("zyu", "じゅ"),
    ("zyo", "じょ"),
    ("cha", "ちゃ"),
    ("chu", "ちゅ"),
    ("cho", "ちょ"),
    ("cya", "ちゃ"),
    ("cyu", "ちゅ"),
    ("cyo", "ちょ"),
    ("tya", "ちゃ"),
    ("tyu", "ちゅ"),
    ("tyo", "ちょ"),
    ("dya", "ぢゃ"),
    ("dyu", "ぢゅ"),
    ("dyo", "ぢょ"),
    ("nya", "にゃ"),
    ("nyu", "にゅ"),
    ("nyo", "にょ"),
    ("hya", "ひゃ"),
    ("hyu", "ひゅ"),
    ("hyo", "ひょ"),
    ("bya", "びゃ"),
    ("byu", "びゅ"),
    ("byo", "びょ"),
    ("pya", "ぴゃ"),
    ("pyu", "ぴゅ"),
    ("pyo", "ぴょ"),
    ("mya", "みゃ"),
    ("myu", "みゅ"),
    ("myo", "みょ"),
    ("rya", "りゃ"),
    ("ryu", "りゅ"),
    ("ryo", "りょ"),
    ("fa", "ふぁ"),
    ("fi", "ふぃ"),
    ("fe", "ふぇ"),
    ("fo", "ふぉ"),
    ("va", "ゔぁ"),
    ("vi", "ゔぃ"),
    ("vu", "ゔ"),
    ("ve", "ゔぇ"),
    ("vo", "ゔぉ"),
    ("wi", "うぃ"),
    ("we", "うぇ"),
    ("she", "しぇ"),
    ("je", "じぇ"),
    ("che", "ちぇ"),
    ("tsa", "つぁ"),
    ("tsi", "つぃ"),
    ("tse", "つぇ"),
    ("tso", "つぉ"),
    ("thi", "てぃ"),
    ("dhi", "でぃ"),
    ("twu", "とぅ"),
    ("dwu", "どぅ"),
    ("kwa", "くぁ"),
    ("gwa", "ぐぁ"),
    ("xa", "ぁ"),
    ("xi", "ぃ"),
    ("xu", "ぅ"),
    ("xe", "ぇ"),
    ("xo", "ぉ"),
    ("la", "ぁ"),
    ("li", "ぃ"),
    ("lu", "ぅ"),
    ("le", "ぇ"),
    ("lo", "ぉ"),
    ("xya", "ゃ"),
    ("xyu", "ゅ"),
    ("xyo", "ょ"),
    ("lya", "ゃ"),
    ("lyu", "ゅ"),
    ("lyo", "ょ"),
    ("xtsu", "っ"),
    ("ltsu", "っ"),
    ("xtu", "っ"),
    ("ltu", "っ"),
    ("xwa", "ゎ"),
    ("-", "ー"),
];

/// Every key is ASCII, so byte-slice lookups never split a character of the input.
static ROMAJI_LOOKUP: LazyLock<HashMap<&'static [u8], &'static str>> = LazyLock::new(|| {
    ROMAJI_TABLE
        .iter()
        .map(|&(romaji, kana)| (romaji.as_bytes(), kana))
        .collect()
});

/// The inverted table `(kana, romaji)`: longest kana first so きゃ beats き, then the longest spelling, then the spelling itself, one entry per kana (:154-180). The spelling tiebreak is what made the C++ choice independent of `unordered_map` bucket order: じ is `ji`, じゃ is `jya`, し is `shi`.
static KANA_TO_ROMAJI: LazyLock<Vec<(&'static str, &'static str)>> = LazyLock::new(|| {
    let mut entries: Vec<(&'static str, &'static str)> = ROMAJI_TABLE
        .iter()
        .map(|&(romaji, kana)| (kana, romaji))
        .collect();
    entries.sort_by(|a, b| {
        b.0.len()
            .cmp(&a.0.len())
            .then(b.1.len().cmp(&a.1.len()))
            .then(a.1.cmp(b.1))
    });
    let mut unique: Vec<(&'static str, &'static str)> = Vec::with_capacity(entries.len());
    let mut seen = HashSet::with_capacity(entries.len());
    for entry in entries {
        if seen.insert(entry.0) {
            unique.push(entry);
        }
    }
    unique
});

/// Each kana advances through its ring in the order a Japanese keyboard prints 小゛゜: the small form where one exists, then the voiced and semi-voiced ones, then back to the plain kana (:269-276).
const KANA_VARIANT_CYCLES: &[&[&str]] = &[
    &["あ", "ぁ"],
    &["い", "ぃ"],
    &["う", "ぅ", "ゔ"],
    &["え", "ぇ"],
    &["お", "ぉ"],
    &["か", "が"],
    &["き", "ぎ"],
    &["く", "ぐ"],
    &["け", "げ"],
    &["こ", "ご"],
    &["さ", "ざ"],
    &["し", "じ"],
    &["す", "ず"],
    &["せ", "ぜ"],
    &["そ", "ぞ"],
    &["た", "だ"],
    &["ち", "ぢ"],
    &["つ", "っ", "づ"],
    &["て", "で"],
    &["と", "ど"],
    &["は", "ば", "ぱ"],
    &["ひ", "び", "ぴ"],
    &["ふ", "ぶ", "ぷ"],
    &["へ", "べ", "ぺ"],
    &["ほ", "ぼ", "ぽ"],
    &["や", "ゃ"],
    &["ゆ", "ゅ"],
    &["よ", "ょ"],
    &["わ", "ゎ"],
];

const HIRAGANA_FIRST: char = '\u{3041}';
const HIRAGANA_LAST: char = '\u{3096}';
const KATAKANA_TO_HIRAGANA_FIRST: u32 = 0x30A1;
const KATAKANA_TO_HIRAGANA_LAST: u32 = 0x30F6;
const KATAKANA_OFFSET: u32 = 0x60;
const SOKUON: &str = "っ";
const MORAIC_N: &str = "ん";

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct RomajiConversion {
    pub hiragana: String,
    /// Letters that do not form kana yet.
    pub pending: String,
    /// Non-empty hiragana and nothing pending.
    pub complete: bool,
}

/// `a..=z` other than the five vowels; `y` counts as a consonant here and the `n` rules exclude it themselves.
fn is_consonant(byte: u8) -> bool {
    byte.is_ascii_lowercase() && !matches!(byte, b'a' | b'i' | b'u' | b'e' | b'o')
}

fn normalized_romaji(input: &str) -> Cow<'_, str> {
    if input.bytes().any(|byte| byte.is_ascii_uppercase()) {
        Cow::Owned(input.to_ascii_lowercase())
    } else {
        Cow::Borrowed(input)
    }
}

/// 按原 `n`、促音与最长表匹配规则转换，剩余输入作为待定尾部。
pub fn convert_romaji(input: &str) -> RomajiConversion {
    let normalized = normalized_romaji(input);
    let mut result = RomajiConversion::default();
    let pending = scan_romaji(&normalized, |kana| result.hiragana.push_str(kana));
    result.pending = pending.to_owned();
    result.complete = !result.hiragana.is_empty() && result.pending.is_empty();
    result
}

/// 按原扫描规则写回转换结果，复用平假名和待定尾部的容量。
pub(crate) fn convert_romaji_into(input: &str, destination: &mut RomajiConversion) {
    destination.hiragana.clear();
    destination.pending.clear();
    let normalized = normalized_romaji(input);
    let pending = scan_romaji(&normalized, |kana| destination.hiragana.push_str(kana));
    destination.pending.push_str(pending);
    destination.complete = !destination.hiragana.is_empty() && destination.pending.is_empty();
}

/// 直接写入假名及待定尾部，复用目标容量；规范化的大写副本仅在本次调用中存活。
pub(crate) fn romaji_reading_into(input: &str, destination: &mut String) {
    destination.clear();
    let normalized = normalized_romaji(input);
    let pending = scan_romaji(&normalized, |kana| destination.push_str(kana));
    destination.push_str(pending);
}

/// 只判断是否有假名且没有待定尾部，不物化转换字符串。
pub(crate) fn is_romaji_complete(input: &str) -> bool {
    let normalized = normalized_romaji(input);
    let mut has_kana = false;
    let pending = scan_romaji(&normalized, |_| has_kana = true);
    has_kana && pending.is_empty()
}

/// 逐个发出假名，返回尚未消费的尾部；调用方持有规范化输入的存储。
fn scan_romaji(normalized: &str, mut emit: impl FnMut(&'static str)) -> &str {
    let bytes = normalized.as_bytes();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'n' {
            match bytes.get(index + 1).copied() {
                None => {
                    emit(MORAIC_N);
                    index += 1;
                    continue;
                }
                Some(b'\'') => {
                    emit(MORAIC_N);
                    index += 2;
                    continue;
                }
                Some(b'n') => {
                    // `nn` 一律是一个ん，和微软、Google 日文输入法及 Rime 一致：习惯这些输入法的人每个ん都打 `nn`，`sinnyou` 要得到しんよう而不是しんにょう。代价是んな要打 `nnna`、こんにちは要打 `konnnichiha`，这也是那些输入法的写法。
                    emit(MORAIC_N);
                    index += 2;
                    continue;
                }
                Some(next) if next == b'-' || (is_consonant(next) && next != b'y') => {
                    emit(MORAIC_N);
                    index += 1;
                    continue;
                }
                Some(_) => {}
            }
        }

        let current = bytes[index];
        let doubled_consonant =
            bytes.get(index + 1) == Some(&current) && is_consonant(current) && current != b'n';
        // Hepburn 把っち写成 `tchi`，所以 `ch` 前面的 `t` 也是促音。
        let hepburn_tch = current == b't' && bytes[index + 1..].starts_with(b"ch");
        if doubled_consonant || hepburn_tch {
            emit(SOKUON);
            index += 1;
            continue;
        }

        let remaining = bytes.len() - index;
        let matched = (1..=remaining.min(4)).rev().find_map(|length| {
            ROMAJI_LOOKUP
                .get(&bytes[index..index + length])
                .map(|&kana| (kana, length))
        });
        let Some((kana, length)) = matched else {
            // 已消费的字节只可能是 ASCII 表键、`n` 或促音字母，`index` 始终落在字符边界。
            return &normalized[index..];
        };
        emit(kana);
        index += length;
    }
    &normalized[index..]
}

/// 将 `U+3041..=U+3096` 加 `0x60`；其余字符（包括ー）原样保留。
pub fn hiragana_to_katakana(hiragana: &str) -> String {
    let mut katakana = String::with_capacity(hiragana.len());
    hiragana_to_katakana_into(hiragana, &mut katakana);
    katakana
}

/// 直接写入片假名，输入输出字节数相同，已有容量不足时只按实际长度扩容。
pub(crate) fn hiragana_to_katakana_into(hiragana: &str, destination: &mut String) {
    destination.clear();
    destination.reserve(hiragana.len());
    for character in hiragana.chars() {
        let katakana = if (HIRAGANA_FIRST..=HIRAGANA_LAST).contains(&character) {
            char::from_u32(u32::from(character) + KATAKANA_OFFSET).unwrap_or(character)
        } else {
            character
        };
        destination.push(katakana);
    }
}

/// Complete, one code point, in U+3041..=U+3096.
pub fn is_single_kana_conversion(conversion: &RomajiConversion) -> bool {
    if !conversion.complete {
        return false;
    }
    let mut chars = conversion.hiragana.chars();
    matches!(
        (chars.next(), chars.next()),
        (Some(kana), None) if (HIRAGANA_FIRST..=HIRAGANA_LAST).contains(&kana)
    )
}

/// Every table kana whose romaji starts with `pending`, sorted and deduplicated (:242-261).
pub fn kana_for_romaji_prefix(pending: &str) -> Vec<&'static str> {
    if pending.is_empty() {
        return Vec::new();
    }
    let prefix = pending.to_ascii_lowercase();
    let mut kana: Vec<&'static str> = ROMAJI_TABLE
        .iter()
        .filter(|(romaji, _)| romaji.starts_with(prefix.as_str()))
        .map(|&(_, kana)| kana)
        .collect();
    kana.sort_unstable();
    kana.dedup();
    kana
}

/// The first inverted-table entry whose kana starts `text`.
fn romaji_at(text: &str) -> Option<(&'static str, &'static str)> {
    KANA_TO_ROMAJI
        .iter()
        .find(|(kana, _)| text.starts_with(kana))
        .copied()
}

fn hiragana_input(kana: &str) -> Cow<'_, str> {
    let has_katakana = kana.chars().any(|character| {
        let code_point = u32::from(character);
        (KATAKANA_TO_HIRAGANA_FIRST..=KATAKANA_TO_HIRAGANA_LAST).contains(&code_point)
    });
    if !has_katakana {
        return Cow::Borrowed(kana);
    }
    kana.chars()
        .map(|character| {
            let code_point = u32::from(character);
            if (KATAKANA_TO_HIRAGANA_FIRST..=KATAKANA_TO_HIRAGANA_LAST).contains(&code_point) {
                char::from_u32(code_point - KATAKANA_OFFSET).unwrap_or(character)
            } else {
                character
            }
        })
        .collect::<String>()
        .into()
}

/// :182-240, using the inverted table's preferred spellings. Katakana is folded to hiragana first by the plain code-point shift; `wana_kana`'s own katakana conversion rewrites an inner ー as a vowel, which would lose the long-vowel mark this has to keep.
pub fn hiragana_to_romaji(kana: &str) -> String {
    let hiragana = hiragana_input(kana);
    let mut romaji = String::with_capacity(kana.len());
    let mut rest = hiragana.as_ref();
    while let Some(character) = rest.chars().next() {
        if let Some(after) = rest.strip_prefix(SOKUON) {
            // A sokuon doubles the next kana's consonant; before a vowel or at the end it has to be spelled out.
            match romaji_at(after).and_then(|(_, next)| next.bytes().next()) {
                Some(first) if is_consonant(first) => romaji.push(char::from(first)),
                _ => romaji.push_str("xtsu"),
            }
            rest = after;
            continue;
        }
        if let Some(after) = rest.strip_prefix(MORAIC_N) {
            // 后面是元音、や行或な行时单个 n 会和它拼成一个假名，要写成 `nn` 才转得回ん（`convert_romaji` 里 `nn` 一律是ん）。
            let joins_next = romaji_at(after)
                .and_then(|(_, next)| next.bytes().next())
                .is_some_and(|first| !is_consonant(first) || first == b'y' || first == b'n');
            romaji.push_str(if joins_next { "nn" } else { "n" });
            rest = after;
            continue;
        }
        match romaji_at(rest) {
            Some((matched, spelling)) => {
                romaji.push_str(spelling);
                rest = &rest[matched.len()..];
            }
            None => rest = &rest[character.len_utf8()..],
        }
    }
    romaji
}

/// The next kana in its variant ring (か→が→か, つ→っ→づ), unchanged without a ring (:269-293).
pub fn next_kana_variant(kana: &str) -> &str {
    for cycle in KANA_VARIANT_CYCLES {
        if let Some(position) = cycle.iter().position(|&entry| entry == kana) {
            return cycle[(position + 1) % cycle.len()];
        }
    }
    kana
}

#[cfg(test)]
mod tests {
    use std::borrow::Cow;

    use super::*;

    #[track_caller]
    fn require_conversion(romaji: &str, hiragana: &str, pending: &str, complete: bool) {
        let conversion = convert_romaji(romaji);
        assert_eq!(
            conversion,
            RomajiConversion {
                hiragana: hiragana.to_owned(),
                pending: pending.to_owned(),
                complete,
            },
            "{romaji}"
        );
    }

    #[test]
    fn lowercase_romaji_conversion_does_not_clone_the_input() {
        let (conversion, allocations) =
            crate::ime::personal_rerank::allocations::count(|| convert_romaji("nihongo"));

        assert_eq!(conversion.hiragana, "にほんご");
        assert!(conversion.pending.is_empty());
        assert!(conversion.complete);
        assert!(
            allocations <= 3,
            "lowercase conversion allocations: {allocations}"
        );
    }

    #[test]
    fn katakana_conversion_allocates_only_the_returned_string() {
        for input in ["か", "かな", "ゔぁゕゖー", "a漢😀"] {
            let expected = wana_kana::utils::hiragana_to_katakana(input);
            let (actual, allocations) =
                crate::ime::personal_rerank::allocations::count(|| hiragana_to_katakana(input));
            assert_eq!(actual, expected);
            eprintln!("片假名拥有型转换分配：{allocations}");
            assert_eq!(allocations, 1, "非空转换只需分配返回字符串");
        }
    }

    #[test]
    fn katakana_conversion_matches_old_library_for_every_unicode_scalar() {
        let input: String = (0..=0x10ffff).filter_map(char::from_u32).collect();
        let expected = wana_kana::utils::hiragana_to_katakana(&input);
        assert_eq!(hiragana_to_katakana(&input), expected);
        assert_eq!(expected.len(), input.len());
    }

    #[test]
    fn conversion_buffers_reuse_storage_across_shrinking_and_pending_edits() {
        let long = "ka".repeat(32);
        let pending = "漢".repeat(32);
        let mut conversion = RomajiConversion::default();
        convert_romaji_into(&long, &mut conversion);
        convert_romaji_into(&pending, &mut conversion);
        let hiragana_pointer = conversion.hiragana.as_ptr();
        let pending_pointer = conversion.pending.as_ptr();
        for input in [
            long.as_str(),
            "nihong",
            "ka漢",
            "n'a",
            "k",
            "",
            pending.as_str(),
            long.as_str(),
        ] {
            let expected = convert_romaji(input);
            let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
                convert_romaji_into(input, &mut conversion);
            });
            assert_eq!(conversion, expected, "{input}");
            assert_eq!(conversion.hiragana.as_ptr(), hiragana_pointer);
            assert_eq!(conversion.pending.as_ptr(), pending_pointer);
            assert_eq!(allocations, 0, "转换编辑复用：{input}");
        }
        let long_kana = "か".repeat(32);
        let mut katakana = String::new();
        hiragana_to_katakana_into(&long_kana, &mut katakana);
        let pointer = katakana.as_ptr();
        for input in [
            long_kana.as_str(),
            "ゔぁゕゖー・ｰ",
            "a漢😀",
            "か",
            "",
            long_kana.as_str(),
        ] {
            let expected = wana_kana::utils::hiragana_to_katakana(input);
            let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
                hiragana_to_katakana_into(input, &mut katakana);
            });
            assert_eq!(katakana, expected);
            assert_eq!(katakana.as_ptr(), pointer);
            assert_eq!(allocations, 0, "片假名编辑复用");
        }
        let (empty, allocations) =
            crate::ime::personal_rerank::allocations::count(|| hiragana_to_katakana(""));
        assert!(empty.is_empty());
        assert_eq!(allocations, 0);
    }

    fn assert_streamed_conversion_matches_owned(input: &str) {
        let expected = convert_romaji(input);
        let expected_reading = format!("{}{}", expected.hiragana, expected.pending);
        let mut conversion = RomajiConversion::default();
        convert_romaji_into(input, &mut conversion);
        assert_eq!(conversion, expected, "{input}");
        let hiragana_pointer = conversion.hiragana.as_ptr();
        let pending_pointer = conversion.pending.as_ptr();
        let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
            convert_romaji_into(input, &mut conversion);
        });
        assert_eq!(conversion, expected, "{input}");
        assert_eq!(conversion.hiragana.as_ptr(), hiragana_pointer);
        assert_eq!(conversion.pending.as_ptr(), pending_pointer);
        assert_eq!(
            allocations,
            usize::from(input.bytes().any(|byte| byte.is_ascii_uppercase())),
            "转换缓冲：{input}"
        );
        let mut reading = String::new();
        romaji_reading_into(input, &mut reading);
        assert_eq!(reading, expected_reading, "{input}");
        let pointer = reading.as_ptr();
        let expected_allocations = usize::from(input.bytes().any(|byte| byte.is_ascii_uppercase()));
        let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
            romaji_reading_into(input, &mut reading);
        });
        assert_eq!(reading, expected_reading, "{input}");
        assert_eq!(reading.as_ptr(), pointer, "{input}");
        assert_eq!(allocations, expected_allocations, "直接写入：{input}");
        let (complete, allocations) =
            crate::ime::personal_rerank::allocations::count(|| is_romaji_complete(input));
        assert_eq!(complete, expected.complete, "{input}");
        assert_eq!(allocations, expected_allocations, "完整性扫描：{input}");
    }

    #[test]
    fn streamed_conversion_matches_every_table_spelling_and_prefix() {
        for &(romaji, _) in ROMAJI_TABLE {
            for end in 0..=romaji.len() {
                assert_streamed_conversion_matches_owned(&romaji[..end]);
                assert_streamed_conversion_matches_owned(&romaji[..end].to_ascii_uppercase());
            }
        }
        for input in [
            "sinnyou",
            "konnnichiha",
            "nnna",
            "n'a",
            "nn",
            "n-",
            "nk",
            "ny",
            "kka",
            "tchi",
            "matcha",
            "ka漢字",
            "ka😀Tail",
            "漢字",
            "😀",
            "KA漢字",
            "n'漢",
            "x?",
            "a[",
            "\0",
        ] {
            assert_streamed_conversion_matches_owned(input);
        }
    }

    #[test]
    fn streamed_reading_reuses_storage_across_shrinking_and_pending_edits() {
        let long = "ka".repeat(32);
        let mut reading = String::new();
        romaji_reading_into(&long, &mut reading);
        let pointer = reading.as_ptr();
        for input in [
            long.as_str(),
            "nihong",
            "ka漢",
            "n'a",
            "k",
            "",
            long.as_str(),
        ] {
            let expected = convert_romaji(input);
            let expected_reading = format!("{}{}", expected.hiragana, expected.pending);
            let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
                romaji_reading_into(input, &mut reading);
            });
            assert_eq!(reading, expected_reading, "{input}");
            assert_eq!(reading.as_ptr(), pointer, "{input}");
            assert_eq!(allocations, 0, "编辑复用：{input}");
        }
    }

    // test_engine_smoke.cpp:216-269.
    #[test]
    fn kana_and_romaji_round_trips() {
        assert_eq!(hiragana_to_katakana("かな"), "カナ");
        assert_eq!(hiragana_to_romaji("カナ"), "kana");
        // Each reading has several equally long spellings; the table's spelling tiebreak pins the choice.
        assert_eq!(hiragana_to_romaji("かんじ"), "kanji");
        assert_eq!(hiragana_to_romaji("しゃしん"), "shashin");
        // ん后面是元音、や行、な行时写成 `nn`，再转换回去仍是同一个读音。
        for reading in [
            "かんな",
            "しんよう",
            "きんえん",
            "こんにちは",
            "んにゃ",
            "ほん",
            "かんじ",
        ] {
            let romaji = hiragana_to_romaji(reading);
            assert_eq!(
                convert_romaji(&romaji).hiragana,
                reading,
                "{reading} -> {romaji}"
            );
        }
        assert_eq!(hiragana_to_romaji("かんな"), "kannna");
    }

    #[test]
    fn hiragana_view_borrows_already_normalized_input() {
        assert!(matches!(hiragana_input("かな"), Cow::Borrowed("かな")));
        assert_eq!(hiragana_input("カナ").as_ref(), "かな");
        assert_eq!(hiragana_input("かナ").as_ref(), "かな");
    }

    /// 用户反馈的两种拼法（`xtu` 打不出っ、`tyou` 打不出ちょう）和同一批补上的拗音；反查仍取原来的拼法。
    #[test]
    fn microsoft_and_google_spellings() {
        require_conversion("xtu", "っ", "", true);
        require_conversion("ltu", "っ", "", true);
        require_conversion("taxtuta", "たった", "", true);
        require_conversion("tyou", "ちょう", "", true);
        require_conversion("zyouzu", "じょうず", "", true);
        require_conversion("dyo", "ぢょ", "", true);
        assert!(is_single_kana_conversion(&convert_romaji("xtu")));
        assert_eq!(hiragana_to_romaji("っ"), "xtsu");
        assert_eq!(hiragana_to_romaji("じゃ"), "jya");
        assert_eq!(hiragana_to_romaji("じょうず"), "jyouzu");
    }

    #[test]
    fn doubled_n_rules() {
        require_conversion("nihonn", "にほん", "", true);
        require_conversion("kann", "かん", "", true);
        require_conversion("nn", "ん", "", true);
        require_conversion("nnn", "んん", "", true);
        require_conversion("gennki", "げんき", "", true);
        assert!(is_single_kana_conversion(&convert_romaji("nn")));
        require_conversion("n", "ん", "", true);
        require_conversion("kanji", "かんじ", "", true);
        // `nn` 一律是ん（微软、Google、Rime 的规则）：后面的元音、や行自成一个假名，な行要再打一个 n。
        require_conversion("nna", "んあ", "", true);
        require_conversion("nnna", "んな", "", true);
        require_conversion("annai", "あんあい", "", true);
        require_conversion("annnai", "あんない", "", true);
        require_conversion("kannna", "かんな", "", true);
        require_conversion("sinnyou", "しんよう", "", true);
        require_conversion("kinnenn", "きんえん", "", true);
        require_conversion("konnnichiha", "こんにちは", "", true);
        require_conversion("konnichiha", "こんいちは", "", true);
        require_conversion("nnya", "んや", "", true);
        require_conversion("nnnya", "んにゃ", "", true);
        require_conversion("n'a", "んあ", "", true);
        require_conversion("ko-hi-", "こーひー", "", true);
        require_conversion("n-", "んー", "", true);
    }

    #[test]
    fn sokuon_rules() {
        require_conversion("matcha", "まっちゃ", "", true);
        require_conversion("kotchi", "こっち", "", true);
        require_conversion("itchi", "いっち", "", true);
        require_conversion("tchi", "っち", "", true);
        require_conversion("match", "まっ", "ch", false);
        require_conversion("matc", "ま", "tc", false);
        require_conversion("mat", "ま", "t", false);
        require_conversion("kitte", "きって", "", true);
        require_conversion("kitto", "きっと", "", true);
        require_conversion("maccha", "まっちゃ", "", true);
        require_conversion("mattya", "まっちゃ", "", true);
        require_conversion("ecchi", "えっち", "", true);
    }

    #[test]
    fn pending_tails() {
        require_conversion("k", "", "k", false);
        require_conversion("kanj", "かん", "j", false);
        require_conversion("KA", "か", "", true);
        require_conversion("sis", "し", "s", false);
        // A lone apostrophe not after n, and an n before a non-letter, stay pending.
        require_conversion("a'", "あ", "'", false);
        require_conversion("n1", "", "n1", false);
        require_conversion("", "", "", false);
    }

    #[test]
    fn single_kana_conversion() {
        assert!(is_single_kana_conversion(&convert_romaji("ka")));
        assert!(is_single_kana_conversion(&convert_romaji("xtsu")));
        assert!(!is_single_kana_conversion(&convert_romaji("kya")));
        assert!(!is_single_kana_conversion(&convert_romaji("-")));
        assert!(!is_single_kana_conversion(&convert_romaji("k")));
        assert!(!is_single_kana_conversion(&convert_romaji("kak")));
    }

    #[test]
    fn inverted_table_preferences() {
        for (kana, romaji) in [
            ("し", "shi"),
            ("じ", "ji"),
            ("ち", "chi"),
            ("つ", "tsu"),
            ("ふ", "fu"),
            ("ぁ", "la"),
            ("ゃ", "lya"),
            ("しゃ", "sha"),
            ("じゃ", "jya"),
            ("ちゃ", "cha"),
            ("ー", "-"),
        ] {
            assert_eq!(hiragana_to_romaji(kana), romaji, "{kana}");
        }
        assert_eq!(hiragana_to_romaji("きって"), "kitte");
        assert_eq!(hiragana_to_romaji("っ"), "xtsu");
        assert_eq!(hiragana_to_romaji("っあ"), "xtsua");
        assert_eq!(hiragana_to_romaji("こーひー"), "ko-hi-");
        assert_eq!(hiragana_to_romaji("コーヒー"), "ko-hi-");
        assert_eq!(hiragana_to_romaji("か漢じ"), "kaji");
    }

    #[test]
    fn katakana_shift_covers_the_small_and_voiced_range() {
        assert_eq!(hiragana_to_katakana("ゔぁゕゖー"), "ヴァヵヶー");
        assert_eq!(hiragana_to_katakana("a漢"), "a漢");
    }

    #[test]
    fn kana_for_prefix_is_sorted_and_unique() {
        let kana = kana_for_romaji_prefix("K");
        for expected in ["か", "き", "く", "け", "こ", "きゃ", "きゅ", "きょ", "くぁ"]
        {
            assert!(kana.contains(&expected), "{expected}");
        }
        assert!(kana.windows(2).all(|pair| pair[0] < pair[1]));
        assert_eq!(
            kana_for_romaji_prefix("sh"),
            vec!["し", "しぇ", "しゃ", "しゅ", "しょ"]
        );
        assert!(kana_for_romaji_prefix("").is_empty());
        assert!(kana_for_romaji_prefix("q").is_empty());
    }

    #[test]
    fn kana_variant_rings() {
        assert_eq!(next_kana_variant("か"), "が");
        assert_eq!(next_kana_variant("が"), "か");
        assert_eq!(next_kana_variant("は"), "ば");
        assert_eq!(next_kana_variant("ば"), "ぱ");
        assert_eq!(next_kana_variant("ぱ"), "は");
        assert_eq!(next_kana_variant("つ"), "っ");
        assert_eq!(next_kana_variant("っ"), "づ");
        assert_eq!(next_kana_variant("づ"), "つ");
        assert_eq!(next_kana_variant("ん"), "ん");
    }
}
