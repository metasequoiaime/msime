//! The Dachen keyboard: which ASCII key types which bopomofo symbol, the tone keys, and the non-letter keys the scheme claims in each state. The table follows libchewing's `standard.rs`.

/// The slot a bopomofo symbol fills in a syllable. A key of a kind already present replaces that slot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum Kind {
    Initial,
    Medial,
    Rime,
}

/// The 37 phonetic keys of the Dachen layout.
pub const DACHEN: [(u8, char, Kind); 37] = [
    (b'1', 'ㄅ', Kind::Initial),
    (b'q', 'ㄆ', Kind::Initial),
    (b'a', 'ㄇ', Kind::Initial),
    (b'z', 'ㄈ', Kind::Initial),
    (b'2', 'ㄉ', Kind::Initial),
    (b'w', 'ㄊ', Kind::Initial),
    (b's', 'ㄋ', Kind::Initial),
    (b'x', 'ㄌ', Kind::Initial),
    (b'e', 'ㄍ', Kind::Initial),
    (b'd', 'ㄎ', Kind::Initial),
    (b'c', 'ㄏ', Kind::Initial),
    (b'r', 'ㄐ', Kind::Initial),
    (b'f', 'ㄑ', Kind::Initial),
    (b'v', 'ㄒ', Kind::Initial),
    (b'5', 'ㄓ', Kind::Initial),
    (b't', 'ㄔ', Kind::Initial),
    (b'g', 'ㄕ', Kind::Initial),
    (b'b', 'ㄖ', Kind::Initial),
    (b'y', 'ㄗ', Kind::Initial),
    (b'h', 'ㄘ', Kind::Initial),
    (b'n', 'ㄙ', Kind::Initial),
    (b'u', 'ㄧ', Kind::Medial),
    (b'j', 'ㄨ', Kind::Medial),
    (b'm', 'ㄩ', Kind::Medial),
    (b'8', 'ㄚ', Kind::Rime),
    (b'i', 'ㄛ', Kind::Rime),
    (b'k', 'ㄜ', Kind::Rime),
    (b',', 'ㄝ', Kind::Rime),
    (b'9', 'ㄞ', Kind::Rime),
    (b'o', 'ㄟ', Kind::Rime),
    (b'l', 'ㄠ', Kind::Rime),
    (b'.', 'ㄡ', Kind::Rime),
    (b'0', 'ㄢ', Kind::Rime),
    (b'p', 'ㄣ', Kind::Rime),
    (b';', 'ㄤ', Kind::Rime),
    (b'/', 'ㄥ', Kind::Rime),
    (b'-', 'ㄦ', Kind::Rime),
];

/// Every non-letter key the scheme can claim, phonetic and tone keys together, in the canonical order `spelling_symbols` lists them in.
pub const DACHEN_SYMBOLS: &str = "1234567890,./;- ";

/// Space is tone 1, then `6 3 4 7` are tones 2, 3, 4 and the neutral tone.
pub const TONE_KEYS: [u8; 5] = *b" 6347";

/// The mark each of `TONE_KEYS` appends to a syllable; tone 1 is unmarked.
pub const TONE_MARKS: [&str; 5] = ["", "ˊ", "ˇ", "ˋ", "˙"];

/// The non-letter keys that start a composition from idle: every phonetic non-letter key. The tone digits and space are left out, so they type themselves when nothing is composed.
pub const IDLE_SYMBOLS: &str = "125890,./;-";

/// Shifted keys that commit the conversion and then insert a full-width mark.
pub const SHIFT_PUNCTUATION: [(u8, char); 8] = [
    (b'<', '，'),
    (b'>', '。'),
    (b'?', '？'),
    (b':', '：'),
    (b'[', '「'),
    (b']', '」'),
    (b'{', '『'),
    (b'}', '』'),
];

/// The bopomofo symbol and slot of a phonetic key.
pub fn symbol(key: u8) -> Option<(char, Kind)> {
    DACHEN
        .iter()
        .find(|(dachen_key, _, _)| *dachen_key == key)
        .map(|(_, symbol, kind)| (*symbol, *kind))
}

/// The mark a tone key appends, `None` for any other key.
pub fn tone_mark(key: u8) -> Option<&'static str> {
    TONE_KEYS
        .iter()
        .position(|tone_key| *tone_key == key)
        .map(|index| TONE_MARKS[index])
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;

    #[test]
    fn dachen_has_37_unique_keys_and_symbols() {
        let keys: HashSet<u8> = DACHEN.iter().map(|(key, _, _)| *key).collect();
        let symbols: HashSet<char> = DACHEN.iter().map(|(_, symbol, _)| *symbol).collect();
        assert_eq!(keys.len(), 37);
        assert_eq!(symbols.len(), 37);
        let count = |kind| DACHEN.iter().filter(|entry| entry.2 == kind).count();
        assert_eq!(
            (count(Kind::Initial), count(Kind::Medial), count(Kind::Rime)),
            (21, 3, 13)
        );
        // The bopomofo block runs ㄅ (U+3105) to ㄩ (U+3129); the layout covers all of it.
        let block: HashSet<char> = ('\u{3105}'..='\u{3129}').collect();
        assert_eq!(symbols, block);
    }

    #[test]
    fn tone_keys_are_not_phonetic_keys() {
        for key in TONE_KEYS {
            assert_eq!(symbol(key), None, "{}", key as char);
        }
        assert_eq!(tone_mark(b' '), Some(""));
        assert_eq!(tone_mark(b'6'), Some("ˊ"));
        assert_eq!(tone_mark(b'7'), Some("˙"));
        assert_eq!(tone_mark(b'1'), None);
        assert_eq!(symbol(b'1'), Some(('ㄅ', Kind::Initial)));
        assert_eq!(symbol(b'-'), Some(('ㄦ', Kind::Rime)));
    }

    #[test]
    fn symbol_strings_follow_the_layout() {
        // Every non-letter phonetic key and every tone key, nothing else, in canonical order.
        let non_letters: HashSet<u8> = DACHEN
            .iter()
            .map(|(key, _, _)| *key)
            .filter(|key| !key.is_ascii_alphabetic())
            .chain(TONE_KEYS)
            .collect();
        assert_eq!(DACHEN_SYMBOLS.len(), non_letters.len());
        assert!(DACHEN_SYMBOLS.bytes().all(|key| non_letters.contains(&key)));

        let idle: String = DACHEN_SYMBOLS
            .chars()
            .filter(|key| !TONE_KEYS.contains(&(*key as u8)))
            .collect();
        assert_eq!(IDLE_SYMBOLS, idle);
        for excluded in *b"3467 " {
            assert!(!IDLE_SYMBOLS.as_bytes().contains(&excluded));
        }
        assert!(IDLE_SYMBOLS.bytes().all(|key| symbol(key).is_some()));
    }

    #[test]
    fn shift_punctuation_keys_are_not_phonetic() {
        for (key, _) in SHIFT_PUNCTUATION {
            assert_eq!(symbol(key), None);
            assert_eq!(tone_mark(key), None);
        }
    }
}
