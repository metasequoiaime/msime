//! Romaji and kana conversion (schemes-lang.md §5.1-§5.3, `romaji_converter.cpp`).
//!
//! The table, the `n` rules, the sokuon rules and the pending tail are IME behaviour the provider's dictionary lookups depend on, so they stay hand-written; `wana_kana` has its own table (じゃ is `ja`, a lone `n` is kept as a letter) and no notion of a pending tail. The plain hiragana-to-katakana shift is `wana_kana`'s.

use std::borrow::Cow;
use std::collections::{HashMap, HashSet};
use std::sync::LazyLock;

/// `romaji_converter.cpp:11-44`, verbatim.
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
    ("cha", "ちゃ"),
    ("chu", "ちゅ"),
    ("cho", "ちょ"),
    ("cya", "ちゃ"),
    ("cyu", "ちゅ"),
    ("cyo", "ちょ"),
    ("tya", "ちゃ"),
    ("tyu", "ちゅ"),
    ("tyo", "ちょ"),
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

/// :54-131: `n` rules, sokuon, longest table match, the rest pending.
pub fn convert_romaji(input: &str) -> RomajiConversion {
    let normalized = input.to_ascii_lowercase();
    let bytes = normalized.as_bytes();
    let mut result = RomajiConversion::default();
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'n' {
            match bytes.get(index + 1).copied() {
                None => {
                    result.hiragana.push_str(MORAIC_N);
                    index += 1;
                    continue;
                }
                Some(b'\'') => {
                    result.hiragana.push_str(MORAIC_N);
                    index += 2;
                    continue;
                }
                Some(b'n') => {
                    // `nn` is one ん when the second n cannot begin a kana of its own; otherwise only the first n is consumed so `nna` still reads んな.
                    let second_n_stands_alone = match bytes.get(index + 2).copied() {
                        None | Some(b'\'') => true,
                        Some(third) => is_consonant(third) && third != b'y',
                    };
                    result.hiragana.push_str(MORAIC_N);
                    index += if second_n_stands_alone { 2 } else { 1 };
                    continue;
                }
                Some(next) if next == b'-' || (is_consonant(next) && next != b'y') => {
                    result.hiragana.push_str(MORAIC_N);
                    index += 1;
                    continue;
                }
                Some(_) => {}
            }
        }

        let current = bytes[index];
        let doubled_consonant =
            bytes.get(index + 1) == Some(&current) && is_consonant(current) && current != b'n';
        // Hepburn writes っち as `tchi`, so a t directly before `ch` is a sokuon although the consonants differ.
        let hepburn_tch = current == b't' && bytes[index + 1..].starts_with(b"ch");
        if doubled_consonant || hepburn_tch {
            result.hiragana.push_str(SOKUON);
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
            // Every consumed byte was an ASCII table key, a `n` or a sokuon letter, so `index` is on a character boundary.
            result.pending = normalized[index..].to_owned();
            break;
        };
        result.hiragana.push_str(kana);
        index += length;
    }
    result.complete = !result.hiragana.is_empty() && result.pending.is_empty();
    result
}

/// Shifts U+3041..=U+3096 by 0x60; everything else, ー included, passes through.
pub fn hiragana_to_katakana(hiragana: &str) -> String {
    wana_kana::utils::hiragana_to_katakana(hiragana)
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
            romaji.push('n');
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

    // test_engine_smoke.cpp:216-269.
    #[test]
    fn kana_and_romaji_round_trips() {
        assert_eq!(hiragana_to_katakana("かな"), "カナ");
        assert_eq!(hiragana_to_romaji("カナ"), "kana");
        // Each reading has several equally long spellings; the table's spelling tiebreak pins the choice.
        assert_eq!(hiragana_to_romaji("かんじ"), "kanji");
        assert_eq!(hiragana_to_romaji("しゃしん"), "shashin");
    }

    #[test]
    fn hiragana_view_borrows_already_normalized_input() {
        assert!(matches!(hiragana_input("かな"), Cow::Borrowed("かな")));
        assert_eq!(hiragana_input("カナ").as_ref(), "かな");
        assert_eq!(hiragana_input("かナ").as_ref(), "かな");
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
        require_conversion("nna", "んな", "", true);
        require_conversion("annai", "あんない", "", true);
        require_conversion("kanna", "かんな", "", true);
        require_conversion("sannin", "さんにん", "", true);
        require_conversion("konnichiha", "こんにちは", "", true);
        require_conversion("nnya", "んにゃ", "", true);
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
