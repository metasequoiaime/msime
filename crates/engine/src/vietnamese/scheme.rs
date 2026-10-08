//! Vietnamese key handling: the composition is the raw keystrokes of one word, and the display is those keystrokes run through the `vi` crate's Telex or VNI transform. Backspace removes one keystroke, never one displayed letter, so marks come off the way they went on. The first Esc locks the display to the raw keys (for English words the transform would mangle); the session cancels on the second.

use super::{InputMethod, ToneStyle};
use crate::types::{QueryRequest, SchemeKey, SchemeType};
use vi::processor::AccentStyle;

/// VNI's mark keys; they only spell while a word is composing, so an idle digit still types the digit.
const VNI_DIGITS: &str = "0123456789";

#[derive(Debug, Clone, Default)]
pub struct VietnameseScheme {
    method: InputMethod,
    style: ToneStyle,
    /// Keystrokes of the composing word, case kept.
    raw: String,
    /// Esc asked for the raw keys: the display is `raw` verbatim and later keys append verbatim.
    raw_locked: bool,
}

impl VietnameseScheme {
    pub fn new(method: InputMethod, style: ToneStyle) -> Self {
        Self {
            method,
            style,
            ..Self::default()
        }
    }

    pub fn reset(&mut self) {
        self.raw.clear();
        self.raw_locked = false;
    }

    pub fn is_composing(&self) -> bool {
        !self.raw.is_empty()
    }

    /// The keystrokes as typed, case kept.
    pub fn raw(&self) -> &str {
        &self.raw
    }

    pub fn raw_locked(&self) -> bool {
        self.raw_locked
    }

    /// Letters always spell; VNI digits spell only while composing; Backspace removes the last keystroke. Every other key is ignored, which leaves it to the session to finish the word.
    pub fn handle_key(&mut self, key: SchemeKey) {
        match key {
            SchemeKey::Letter(letter) if letter.is_ascii_alphabetic() => {
                self.raw.push(char::from(letter));
            }
            SchemeKey::Symbol(digit) if self.claims_digit(digit) => {
                self.raw.push(char::from(digit));
            }
            SchemeKey::Backspace => {
                self.raw.pop();
                if self.raw.is_empty() {
                    self.raw_locked = false;
                }
            }
            SchemeKey::Letter(_)
            | SchemeKey::Symbol(_)
            | SchemeKey::Apostrophe
            | SchemeKey::Semicolon
            | SchemeKey::Minus
            | SchemeKey::Requery => {}
        }
    }

    /// Esc: the first one switches the display to the raw keys and returns true; with nothing composing or the raw keys already showing it returns false, and the session cancels.
    pub fn restore_raw(&mut self) -> bool {
        if self.raw.is_empty() || self.raw_locked {
            return false;
        }
        self.raw_locked = true;
        true
    }

    /// The non-letter keys `handle_key` takes in this state (`SessionSnapshot::spelling_symbols`): VNI's digits while a word is composing, nothing otherwise, so the runtime never selects with a digit that should spell a mark.
    pub fn spelling_symbols(&self) -> &'static str {
        if self.method == InputMethod::Vni && self.is_composing() {
            VNI_DIGITS
        } else {
            ""
        }
    }

    /// The text the word commits as: the transformed keystrokes, or the keystrokes themselves once Esc locked them.
    pub fn preedit(&self) -> String {
        let mut display = String::new();
        self.preedit_into(&mut display);
        display
    }

    pub fn preedit_into(&self, display: &mut String) {
        display.clear();
        if self.raw_locked {
            display.push_str(&self.raw);
            return;
        }
        let lower = self.transform(&self.raw.to_lowercase());
        let cased = self.transform(&self.raw);
        // vi 0.8.0 places tones wrongly on uppercase input, so the letters come from the lowercase run and only the case per position from the cased one. When the two runs disagree on length the positions cannot be matched, and the cased run is the only one that kept the case.
        if lower.chars().count() != cased.chars().count() {
            display.push_str(&cased);
            return;
        }
        display.reserve(lower.len());
        for (letter, case) in lower.chars().zip(cased.chars()) {
            if case.is_uppercase() {
                display.extend(letter.to_uppercase());
            } else {
                display.push(letter);
            }
        }
    }

    /// The keystrokes as the raw input (case kept in `raw_input_with_cases`, so host editing and scratch schemes rebuild the same word) and the display as the segmentation. Valid while a word is composing; no provider answers it.
    pub fn build_request(&self) -> QueryRequest {
        let mut request = QueryRequest::default();
        self.build_request_into(&mut request);
        request
    }

    pub fn build_request_into(&self, request: &mut QueryRequest) {
        request.scheme = SchemeType::Vietnamese;
        request.raw_input.clear();
        request.raw_input.extend(
            self.raw
                .bytes()
                .map(|byte| char::from(byte.to_ascii_lowercase())),
        );
        request.raw_input_with_cases.clone_from(&self.raw);
        request.normalized_input.clone_from(&request.raw_input);
        request.raw_segmentation.clone_from(&self.raw);
        self.preedit_into(&mut request.normalized_segmentation);
        request
            .segmentation
            .clone_from(&request.normalized_segmentation);
        request.valid = self.is_composing();
    }

    /// Keeps the keys `handle_key` would take, preferring the cased spelling when the host sent one; the raw lock is dropped because the keys changed.
    pub fn set_raw_input(&mut self, raw: &str, raw_with_cases: &str) {
        let source = if raw_with_cases.is_empty() {
            raw
        } else {
            raw_with_cases
        };
        self.reset();
        self.raw.reserve(source.len());
        for key in source.bytes() {
            if key.is_ascii_alphabetic() || self.claims_digit(key) {
                self.raw.push(char::from(key));
            }
        }
    }

    fn claims_digit(&self, key: u8) -> bool {
        self.method == InputMethod::Vni && key.is_ascii_digit() && self.is_composing()
    }

    fn transform(&self, keys: &str) -> String {
        let definition = match self.method {
            InputMethod::Telex => &vi::TELEX,
            InputMethod::Vni => &vi::VNI,
        };
        let style = match self.style {
            ToneStyle::Modern => AccentStyle::New,
            ToneStyle::Classic => AccentStyle::Old,
        };
        let mut output = String::new();
        vi::transform_buffer_with_style(definition, style, keys.chars(), &mut output);
        output
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Types `keys` the way the session sends them: letters as `Letter`, everything else as `Symbol`.
    fn typed(method: InputMethod, style: ToneStyle, keys: &str) -> VietnameseScheme {
        let mut scheme = VietnameseScheme::new(method, style);
        for key in keys.bytes() {
            scheme.handle_key(if key.is_ascii_alphabetic() {
                SchemeKey::Letter(key)
            } else {
                SchemeKey::Symbol(key)
            });
        }
        scheme
    }

    fn telex(keys: &str) -> String {
        typed(InputMethod::Telex, ToneStyle::Modern, keys).preedit()
    }

    fn vni(keys: &str) -> String {
        typed(InputMethod::Vni, ToneStyle::Modern, keys).preedit()
    }

    #[test]
    fn set_raw_input_reserves_source_capacity() {
        let source: String = (0..100)
            .map(|index| if index % 5 == 3 { '_' } else { 'a' })
            .collect();
        let mut scheme = VietnameseScheme::new(InputMethod::Telex, ToneStyle::Modern);
        scheme.set_raw_input(&source, "");
        assert_eq!(scheme.raw.len(), 80);
        assert_eq!(scheme.raw.capacity(), source.len());
    }

    #[test]
    fn telex_and_vni_transform_table() {
        let cases = [
            ("hoaf", "hoà"),
            ("thuyr", "thuỷ"),
            ("khoer", "khoẻ"),
            ("tieengs", "tiếng"),
            ("nguoiwf", "người"),
            ("dduwowngf", "đường"),
            ("NGUOWIF", "NGƯỜI"),
            ("w", "ư"),
            ("ww", "w"),
            ("aaa", "aa"),
            ("ddd", "dd"),
        ];
        for (keys, expected) in cases {
            assert_eq!(telex(keys), expected, "{keys}");
        }
        assert_eq!(vni("viet65"), "việt");
        assert_eq!(vni("nguo72i"), "người");
    }

    #[test]
    fn classic_style_puts_the_tone_on_the_first_vowel() {
        let cases = [
            ("hoaf", "hoà", "hòa"),
            ("thuyr", "thuỷ", "thủy"),
            ("khoer", "khoẻ", "khỏe"),
            ("tieengs", "tiếng", "tiếng"),
        ];
        for (keys, modern, classic) in cases {
            assert_eq!(
                typed(InputMethod::Telex, ToneStyle::Modern, keys).preedit(),
                modern,
                "{keys}"
            );
            assert_eq!(
                typed(InputMethod::Telex, ToneStyle::Classic, keys).preedit(),
                classic,
                "{keys}"
            );
        }
        assert_eq!(
            typed(InputMethod::Vni, ToneStyle::Classic, "hoa2").preedit(),
            "hòa"
        );
        assert_eq!(
            typed(InputMethod::Vni, ToneStyle::Modern, "hoa2").preedit(),
            "hoà"
        );
    }

    #[test]
    fn uppercase_keeps_the_lowercase_tone_position() {
        assert_eq!(telex("NGUOWIF"), "NGƯỜI");
        assert_eq!(
            typed(InputMethod::Telex, ToneStyle::Modern, "HOAF").preedit(),
            "HOÀ"
        );
        assert_eq!(
            typed(InputMethod::Telex, ToneStyle::Classic, "HOAF").preedit(),
            "HÒA"
        );
        assert_eq!(telex("Vieejt"), "Việt");
        assert_eq!(telex("DDuwowngf"), "Đường");
        assert_eq!(vni("VIET65"), "VIỆT");
    }

    #[test]
    fn backspace_removes_one_raw_keystroke() {
        let mut scheme = typed(InputMethod::Telex, ToneStyle::Modern, "vieejt");
        let mut shown = vec![scheme.preedit()];
        while scheme.is_composing() {
            scheme.handle_key(SchemeKey::Backspace);
            shown.push(scheme.preedit());
        }
        assert_eq!(shown, ["việt", "việ", "viê", "vie", "vi", "v", ""]);

        let mut scheme = typed(InputMethod::Vni, ToneStyle::Modern, "viet65");
        scheme.handle_key(SchemeKey::Backspace);
        assert_eq!(scheme.preedit(), "viêt");
        assert_eq!(scheme.raw(), "viet6");
    }

    #[test]
    fn esc_locks_the_raw_keys_and_a_second_esc_is_left_to_the_session() {
        let mut scheme = typed(InputMethod::Telex, ToneStyle::Modern, "coffee");
        assert_ne!(scheme.preedit(), "coffee");
        assert!(scheme.restore_raw());
        assert!(scheme.raw_locked());
        assert_eq!(scheme.preedit(), "coffee");
        // Later keys append verbatim.
        scheme.handle_key(SchemeKey::Letter(b's'));
        assert_eq!(scheme.preedit(), "coffees");
        assert!(!scheme.restore_raw());

        // Backspace keeps the lock until the word is gone.
        scheme.handle_key(SchemeKey::Backspace);
        assert_eq!(scheme.preedit(), "coffee");
        for _ in 0..6 {
            scheme.handle_key(SchemeKey::Backspace);
        }
        assert!(!scheme.raw_locked());
        scheme.handle_key(SchemeKey::Letter(b'w'));
        assert_eq!(scheme.preedit(), "ư");

        assert!(!VietnameseScheme::new(InputMethod::Telex, ToneStyle::Modern).restore_raw());
    }

    #[test]
    fn vni_digits_spell_only_while_composing() {
        let mut scheme = VietnameseScheme::new(InputMethod::Vni, ToneStyle::Modern);
        assert_eq!(scheme.spelling_symbols(), "");
        scheme.handle_key(SchemeKey::Symbol(b'6'));
        assert!(!scheme.is_composing());
        scheme.handle_key(SchemeKey::Letter(b'a'));
        assert_eq!(scheme.spelling_symbols(), "0123456789");
        scheme.handle_key(SchemeKey::Symbol(b'6'));
        assert_eq!(scheme.preedit(), "â");

        // Telex never claims a digit.
        let mut scheme = typed(InputMethod::Telex, ToneStyle::Modern, "a");
        assert_eq!(scheme.spelling_symbols(), "");
        scheme.handle_key(SchemeKey::Symbol(b'6'));
        assert_eq!(scheme.raw(), "a");
    }

    #[test]
    fn other_keys_are_ignored() {
        let mut scheme = typed(InputMethod::Vni, ToneStyle::Modern, "a");
        for key in [
            SchemeKey::Apostrophe,
            SchemeKey::Semicolon,
            SchemeKey::Minus,
            SchemeKey::Requery,
            SchemeKey::Letter(b'1'),
            SchemeKey::Symbol(b' '),
            SchemeKey::Symbol(b';'),
        ] {
            scheme.handle_key(key);
        }
        assert_eq!(scheme.raw(), "a");
    }

    #[test]
    fn set_raw_input_prefers_the_cased_spelling_and_drops_the_lock() {
        let mut scheme = typed(InputMethod::Telex, ToneStyle::Modern, "abc");
        assert!(scheme.restore_raw());
        scheme.set_raw_input("vieejt", "Vieejt 1'");
        assert!(!scheme.raw_locked());
        assert_eq!(scheme.raw(), "Vieejt");
        assert_eq!(scheme.preedit(), "Việt");
        scheme.set_raw_input("hoaf", "");
        assert_eq!(scheme.preedit(), "hoà");

        // A VNI digit is kept only after a letter, as typing it would.
        let mut scheme = VietnameseScheme::new(InputMethod::Vni, ToneStyle::Modern);
        scheme.set_raw_input("6viet65", "");
        assert_eq!(scheme.raw(), "viet65");
        assert_eq!(scheme.preedit(), "việt");
    }

    #[test]
    fn reset_drops_the_word_and_the_lock() {
        let mut scheme = typed(InputMethod::Telex, ToneStyle::Modern, "abc");
        scheme.restore_raw();
        scheme.reset();
        assert!(!scheme.is_composing());
        assert!(!scheme.raw_locked());
        assert_eq!(scheme.preedit(), "");
    }

    #[test]
    fn request_carries_the_keys_as_raw_input_and_the_display_as_segmentation() {
        let request = typed(InputMethod::Telex, ToneStyle::Modern, "Vieejt").build_request();
        assert_eq!(request.scheme, SchemeType::Vietnamese);
        assert_eq!(request.raw_input, "vieejt");
        assert_eq!(request.raw_input_with_cases, "Vieejt");
        assert_eq!(request.normalized_input, "vieejt");
        assert_eq!(request.raw_segmentation, "Vieejt");
        assert_eq!(request.segmentation, "Việt");
        assert_eq!(request.normalized_segmentation, "Việt");
        assert!(request.valid);
        assert!(!VietnameseScheme::default().build_request().valid);
    }
}
