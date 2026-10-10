//! Profile tables, verbatim from `shuangpin_profile.cpp` (schemes-lang.md §1.2), and the accepted syllable set (§1.3): the intact set plus `eng`, minus the historical exclusions; `yo` is accepted in every profile.

use std::collections::HashSet;
use std::sync::OnceLock;

use crate::pinyin::syllables::intact_pinyin_set;
use crate::types::ShuangpinProfileKind;

#[derive(Debug, PartialEq, Eq)]
pub struct ShuangpinProfile {
    pub kind: ShuangpinProfileKind,
    /// Multi-letter initial to key.
    pub initials: &'static [(&'static str, &'static str)],
    /// Whole zero-initial syllable to its two-key code.
    pub zero_initials: &'static [(&'static str, &'static str)],
    /// Final to key.
    pub finals: &'static [(&'static str, &'static str)],
}

static XIAOHE: ShuangpinProfile = ShuangpinProfile {
    kind: ShuangpinProfileKind::Xiaohe,
    initials: &[("sh", "u"), ("ch", "i"), ("zh", "v")],
    zero_initials: &[
        ("a", "aa"),
        ("ai", "ai"),
        ("an", "an"),
        ("ao", "ao"),
        ("ang", "ah"),
        ("e", "ee"),
        ("ei", "ei"),
        ("en", "en"),
        ("eng", "eg"),
        ("er", "er"),
        ("o", "oo"),
        ("ou", "ou"),
    ],
    finals: &[
        ("iu", "q"),
        ("ei", "w"),
        ("e", "e"),
        ("uan", "r"),
        ("ue", "t"),
        ("ve", "t"),
        ("un", "y"),
        ("u", "u"),
        ("i", "i"),
        ("uo", "o"),
        ("o", "o"),
        ("ie", "p"),
        ("a", "a"),
        ("ong", "s"),
        ("iong", "s"),
        ("ai", "d"),
        ("en", "f"),
        ("eng", "g"),
        ("ang", "h"),
        ("an", "j"),
        ("uai", "k"),
        ("ing", "k"),
        ("uang", "l"),
        ("iang", "l"),
        ("ou", "z"),
        ("ua", "x"),
        ("ia", "x"),
        ("ao", "c"),
        ("ui", "v"),
        ("v", "v"),
        ("in", "b"),
        ("iao", "n"),
        ("ian", "m"),
    ],
};

static ZIRANMA: ShuangpinProfile = ShuangpinProfile {
    kind: ShuangpinProfileKind::Ziranma,
    initials: &[("sh", "u"), ("ch", "i"), ("zh", "v")],
    zero_initials: &[
        ("a", "aa"),
        ("ai", "ai"),
        ("an", "an"),
        ("ao", "ao"),
        ("ang", "ah"),
        ("e", "ee"),
        ("ei", "ei"),
        ("en", "en"),
        ("eng", "eg"),
        ("er", "er"),
        ("o", "oo"),
        ("ou", "ou"),
    ],
    finals: &[
        ("iu", "q"),
        ("ia", "w"),
        ("ua", "w"),
        ("e", "e"),
        ("uan", "r"),
        ("ue", "t"),
        ("ve", "t"),
        ("ing", "y"),
        ("uai", "y"),
        ("u", "u"),
        ("i", "i"),
        ("o", "o"),
        ("uo", "o"),
        ("un", "p"),
        ("a", "a"),
        ("iong", "s"),
        ("ong", "s"),
        ("iang", "d"),
        ("uang", "d"),
        ("en", "f"),
        ("eng", "g"),
        ("ang", "h"),
        ("an", "j"),
        ("ao", "k"),
        ("ai", "l"),
        ("ei", "z"),
        ("ie", "x"),
        ("iao", "c"),
        ("ui", "v"),
        ("v", "v"),
        ("ou", "b"),
        ("in", "n"),
        ("ian", "m"),
    ],
};

static SHOUDAO: ShuangpinProfile = ShuangpinProfile {
    kind: ShuangpinProfileKind::Shoudao,
    initials: &[("sh", "e"), ("ch", "i"), ("zh", "v")],
    zero_initials: &[
        ("a", "aa"),
        ("ai", "ai"),
        ("an", "an"),
        ("ao", "ao"),
        ("ang", "ay"),
        // sh sits on "e", so "ee"/"ei"/"ef" would collide with she/shi/sheng.
        ("e", "ue"),
        ("ei", "ui"),
        ("en", "en"),
        ("eng", "uf"),
        ("er", "er"),
        ("o", "oo"),
        ("ou", "ou"),
    ],
    finals: &[
        ("iu", "q"),
        ("ua", "w"),
        ("e", "e"),
        ("ie", "r"),
        ("uan", "t"),
        ("ang", "y"),
        ("u", "u"),
        ("i", "i"),
        ("o", "o"),
        ("uo", "o"),
        ("iao", "p"),
        ("a", "a"),
        ("ou", "s"),
        ("ao", "d"),
        ("eng", "f"),
        ("uai", "g"),
        ("ing", "g"),
        ("ong", "h"),
        ("iong", "h"),
        ("an", "j"),
        ("en", "k"),
        ("ia", "k"),
        ("ai", "l"),
        ("ue", "l"),
        ("un", "z"),
        ("iang", "x"),
        ("uang", "x"),
        ("in", "c"),
        ("v", "v"),
        ("ui", "v"),
        ("ve", "b"),
        ("ian", "n"),
        ("ei", "m"),
    ],
};

static MICROSOFT: ShuangpinProfile = ShuangpinProfile {
    kind: ShuangpinProfileKind::Microsoft,
    initials: &[("sh", "u"), ("ch", "i"), ("zh", "v")],
    zero_initials: &[
        ("a", "oa"),
        ("ai", "ol"),
        ("an", "oj"),
        ("ang", "oh"),
        ("ao", "ok"),
        ("e", "oe"),
        ("ei", "oz"),
        ("en", "of"),
        ("eng", "og"),
        ("er", "or"),
        ("o", "oo"),
        ("ou", "ob"),
    ],
    finals: &[
        ("iu", "q"),
        ("ia", "w"),
        ("ua", "w"),
        ("e", "e"),
        ("uan", "r"),
        ("ue", "t"),
        ("ve", "v"),
        ("uai", "y"),
        ("v", "y"),
        ("u", "u"),
        ("i", "i"),
        ("o", "o"),
        ("uo", "o"),
        ("un", "p"),
        ("a", "a"),
        ("iong", "s"),
        ("ong", "s"),
        ("iang", "d"),
        ("uang", "d"),
        ("en", "f"),
        ("eng", "g"),
        ("ang", "h"),
        ("an", "j"),
        ("ao", "k"),
        ("ai", "l"),
        // The `ing` final is the `;` key; the scheme accepts `;` only as the second key of a chunk.
        ("ing", ";"),
        ("ei", "z"),
        ("ie", "x"),
        ("iao", "c"),
        ("ui", "v"),
        ("ou", "b"),
        ("in", "n"),
        ("ian", "m"),
    ],
};

impl ShuangpinProfile {
    /// 有韵母放在 `;` 上，或零声母编码以 `;` 结尾。这时方案在奇数长度的片段之后接受 `;`，宿主把 `;` 当字母键送进来（`EngineSnapshot::microsoft_shuangpin`）。
    pub fn uses_semicolon_key(&self) -> bool {
        self.finals.iter().any(|(_, key)| *key == ";")
            || self
                .zero_initials
                .iter()
                .any(|(_, code)| code.ends_with(';'))
    }
}

/// 小鹤，即 `ShuangpinProfileKind` 的缺省值。只有名字、拿不到用户的表的地方（后端请求）用它代替 `custom`，与不认识的名字一样。
pub fn default_profile() -> &'static ShuangpinProfile {
    &XIAOHE
}

/// 内置方案的表。`Custom` 没有内置的表，为 `None`：自定义方案的表由 `custom::custom_profile` 从用户的表建出来。
pub fn profile(kind: ShuangpinProfileKind) -> Option<&'static ShuangpinProfile> {
    match kind {
        ShuangpinProfileKind::Xiaohe => Some(&XIAOHE),
        ShuangpinProfileKind::Ziranma => Some(&ZIRANMA),
        ShuangpinProfileKind::Shoudao => Some(&SHOUDAO),
        ShuangpinProfileKind::Microsoft => Some(&MICROSOFT),
        ShuangpinProfileKind::Custom => None,
    }
}

/// Spellings the quanpin table accepts but the historical shuangpin `pinyin.txt` did not; keeping them out preserves the ambiguous-segmentation results users are used to (shuangpin_utils.cpp:40-47). `yo` was removed from this list by the shuangpin_yo overlay.
const EXCLUDED_SYLLABLES: [&str; 13] = [
    "chua", "den", "fiao", "jve", "lo", "lue", "nou", "nue", "nun", "qve", "xve", "yve", "zhei",
];

pub fn accepted_syllables() -> &'static HashSet<&'static str> {
    static SET: OnceLock<HashSet<&'static str>> = OnceLock::new();
    SET.get_or_init(|| {
        let mut set = intact_pinyin_set().clone();
        set.insert("eng");
        for syllable in EXCLUDED_SYLLABLES {
            set.remove(syllable);
        }
        set
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key_of(table: &'static [(&'static str, &'static str)], unit: &str) -> Option<&'static str> {
        table
            .iter()
            .find(|(source, _)| *source == unit)
            .map(|(_, key)| *key)
    }

    #[test]
    fn profiles_select_by_kind() {
        for kind in [
            ShuangpinProfileKind::Xiaohe,
            ShuangpinProfileKind::Ziranma,
            ShuangpinProfileKind::Shoudao,
            ShuangpinProfileKind::Microsoft,
        ] {
            let selected = profile(kind).unwrap();
            assert_eq!(selected.kind, kind);
            assert_eq!(selected.zero_initials.len(), 12);
            assert_eq!(selected.finals.len(), 33);
            assert_eq!(
                selected.uses_semicolon_key(),
                kind == ShuangpinProfileKind::Microsoft
            );
        }
        assert!(profile(ShuangpinProfileKind::Custom).is_none());
    }

    #[test]
    fn tables_keep_the_profile_specific_keys() {
        let shoudao = profile(ShuangpinProfileKind::Shoudao).unwrap();
        assert_eq!(key_of(shoudao.initials, "sh"), Some("e"));
        assert_eq!(key_of(shoudao.zero_initials, "ang"), Some("ay"));
        assert_eq!(key_of(shoudao.zero_initials, "e"), Some("ue"));
        assert_eq!(key_of(shoudao.zero_initials, "ei"), Some("ui"));
        assert_eq!(key_of(shoudao.zero_initials, "eng"), Some("uf"));
        let microsoft = profile(ShuangpinProfileKind::Microsoft).unwrap();
        assert_eq!(key_of(microsoft.finals, "ing"), Some(";"));
        assert_eq!(key_of(microsoft.finals, "v"), Some("y"));
        assert_eq!(key_of(microsoft.finals, "ve"), Some("v"));
        assert_eq!(key_of(microsoft.zero_initials, "a"), Some("oa"));
        let xiaohe = profile(ShuangpinProfileKind::Xiaohe).unwrap();
        assert_eq!(key_of(xiaohe.initials, "zh"), Some("v"));
        assert_eq!(key_of(xiaohe.finals, "ing"), Some("k"));
    }

    #[test]
    fn accepted_set_keeps_yo_and_drops_the_historical_exclusions() {
        let set = accepted_syllables();
        assert!(set.contains("yo"));
        assert!(set.contains("eng"));
        for excluded in EXCLUDED_SYLLABLES {
            assert!(!set.contains(excluded), "{excluded} is accepted");
        }
        assert!(set.contains("shi"));
        assert!(set.contains("lve"));
    }
}
