//! Vietnamese input: Telex or VNI keystrokes transformed into Vietnamese letters with tone marks. The composition is the text itself; there are no candidates and nothing is learned.

pub mod scheme;

pub use scheme::VietnameseScheme;

/// How keystrokes spell diacritics and tones. The ordinal is the host ABI value (`EngineOptions::vietnamese_input_method`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum InputMethod {
    /// Letters spell the marks (`aa` â, `w` ư, `f` grave).
    #[default]
    Telex = 0,
    /// Digits spell the marks (`a6` â, `u7` ư, `a2` à).
    Vni = 1,
}

impl InputMethod {
    pub fn from_u8(value: u8) -> Option<Self> {
        Some(match value {
            0 => Self::Telex,
            1 => Self::Vni,
            _ => return None,
        })
    }
}

/// Where the tone mark goes on an `oa`, `oe` or `uy` syllable. The ordinal is the host ABI value (`EngineOptions::vietnamese_tone_style`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum ToneStyle {
    /// On the second vowel: hoà.
    #[default]
    Modern = 0,
    /// On the first vowel: hòa.
    Classic = 1,
}

impl ToneStyle {
    pub fn from_u8(value: u8) -> Option<Self> {
        Some(match value {
            0 => Self::Modern,
            1 => Self::Classic,
            _ => return None,
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ordinals_round_trip_and_out_of_range_values_are_refused() {
        for method in [InputMethod::Telex, InputMethod::Vni] {
            assert_eq!(InputMethod::from_u8(method as u8), Some(method));
        }
        for style in [ToneStyle::Modern, ToneStyle::Classic] {
            assert_eq!(ToneStyle::from_u8(style as u8), Some(style));
        }
        assert_eq!(InputMethod::from_u8(2), None);
        assert_eq!(ToneStyle::from_u8(2), None);
        assert_eq!(InputMethod::default(), InputMethod::Telex);
        assert_eq!(ToneStyle::default(), ToneStyle::Modern);
    }
}
