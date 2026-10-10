//! Keyboard-face hints read out of the profile tables (api-contract §1b, bridge.cpp:822-877), so a keyboard never keeps its own copy of the keymap.

use super::profile::profile;
use crate::types::ShuangpinProfileKind;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShuangpinKeyHint {
    pub key: String,
    pub hint: String,
}

const KEY_ORDER: [&str; 27] = [
    "Q", "W", "E", "R", "T", "Y", "U", "I", "O", "P", "A", "S", "D", "F", "G", "H", "J", "K", "L",
    "Z", "X", "C", "V", "B", "N", "M", ";",
];

/// The units a mapping puts on `key`, in byte order and joined with a space. The ü finals are spelled with a leading `v` because that is what the keys type; a person reads the hint, so it shows the vowel.
fn units_on_key(mapping: &[(&str, &str)], key: &str) -> String {
    let mut units: Vec<String> = mapping
        .iter()
        .filter(|(_, mapped)| mapped.eq_ignore_ascii_case(key))
        .map(|(unit, _)| match unit.strip_prefix('v') {
            Some(rest) => format!("ü{rest}"),
            None => unit.to_string(),
        })
        .collect();
    units.sort();
    units.join(" ")
}

/// 按 `QWERTYUIOPASDFGHJKLZXCVBNM;` 的顺序列出每个键，写成 `"声母 / 韵母"`，只有一边时只写那一边；单位排好序，开头的 `v` 显示成 `ü`。只认 `xiaohe`、`ziranma`、`shoudao` 和 `microsoft`，其他名字都不给提示，`custom` 也一样：光有名字拿不到用户的表。
pub fn shuangpin_key_hints(profile_name: &str) -> Vec<ShuangpinKeyHint> {
    // An unknown name yields nothing rather than the default profile's face: labelling the keys with a scheme the session is not running is worse than labelling nothing.
    let Some(source) = ShuangpinProfileKind::from_name(profile_name).and_then(profile) else {
        return Vec::new();
    };
    let mut hints = Vec::with_capacity(KEY_ORDER.len());
    for &key in &KEY_ORDER {
        let initials = units_on_key(source.initials, key);
        let finals = units_on_key(source.finals, key);
        let hint = match (initials.is_empty(), finals.is_empty()) {
            (true, true) => continue,
            (true, false) => finals,
            (false, true) => initials,
            (false, false) => format!("{initials} / {finals}"),
        };
        hints.push(ShuangpinKeyHint {
            key: key.to_string(),
            hint,
        });
    }
    hints
}

/// 方案里每个整个的零声母音节和它的两键编码，按表里的顺序。不认识的名字（以及 `custom`）什么也不给，与 `shuangpin_key_hints` 相同。
pub fn shuangpin_zero_initials(profile_name: &str) -> Vec<(&'static str, &'static str)> {
    ShuangpinProfileKind::from_name(profile_name)
        .and_then(profile)
        .map(|source| source.zero_initials.to_vec())
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    /// engine-bridge tests.rs:602-647.
    #[test]
    fn hints_describe_each_profile() {
        let mut faces: Vec<Vec<ShuangpinKeyHint>> = Vec::new();
        for name in ["xiaohe", "ziranma", "shoudao", "microsoft"] {
            let hints = shuangpin_key_hints(name);
            assert_eq!(hints.capacity(), KEY_ORDER.len());
            assert!(
                hints.len() >= 26,
                "{name} labelled only {} keys",
                hints.len()
            );
            for hint in &hints {
                assert!(
                    hint.key.len() == 1
                        && (hint.key.as_bytes()[0].is_ascii_uppercase() || hint.key == ";"),
                    "{name} produced a hint for {:?}",
                    hint.key
                );
                assert!(!hint.hint.is_empty());
                assert!(hint.hint.matches(" / ").count() <= 1);
            }
            faces.push(hints);
        }
        for (index, face) in faces.iter().enumerate() {
            for other in faces.iter().skip(index + 1) {
                assert_ne!(face, other);
            }
        }
        assert_eq!(
            shuangpin_key_hints("microsoft")
                .last()
                .map(|hint| hint.key.as_str()),
            Some(";")
        );
    }

    /// engine-bridge tests.rs:649-660.
    #[test]
    fn hints_keep_every_unit_a_key_carries() {
        let hints: HashMap<String, String> = shuangpin_key_hints("xiaohe")
            .into_iter()
            .map(|entry| (entry.key, entry.hint))
            .collect();
        assert_eq!(hints.get("K").map(String::as_str), Some("ing uai"));
        assert_eq!(hints.get("V").map(String::as_str), Some("zh / ui ü"));
        assert_eq!(hints.get("T").map(String::as_str), Some("ue üe"));
        assert!(!hints.contains_key(";"));
    }

    /// engine-bridge tests.rs:662-667.
    #[test]
    fn hints_reject_an_unknown_profile() {
        assert!(shuangpin_key_hints("").is_empty());
        assert!(shuangpin_key_hints("xiaohe-v2").is_empty());
        assert!(shuangpin_key_hints("quanpin").is_empty());
        assert!(shuangpin_key_hints("custom").is_empty());
        assert!(shuangpin_zero_initials("custom").is_empty());
    }
}
