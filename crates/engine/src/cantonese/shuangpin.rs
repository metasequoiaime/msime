//! 粤拼双拼的独立键位规则。
//!
//! 规则只描述粤拼音节到双拼码的可复现映射，不携带词表或上游方案文件。候选查找仍使用
//! 已加载的 `msime-cantonese.db` 音节库存，因此键位规则与数据许可彼此独立。

use super::syllable::Inventory;

/// 把一个无声调粤拼音节编码成两键粤语双拼。
///
/// 声母优先取最长匹配；零声母使用 `o`。`m`、`ng` 两个独立鼻音按方案约定编码为
/// `om`、`ov`。未知声母或韵母返回 `None`。
pub fn encode(syllable: &str) -> Option<[u8; 2]> {
    let syllable = syllable.as_bytes();
    if syllable.is_empty() || syllable.iter().any(|byte| !byte.is_ascii_lowercase()) {
        return None;
    }
    if syllable == b"m" {
        return Some(*b"om");
    }
    if syllable == b"ng" {
        return Some(*b"ov");
    }
    let (initial, rest) = if syllable.starts_with(b"gw") {
        (b'i', &syllable[2..])
    } else if syllable.starts_with(b"kw") {
        (b'u', &syllable[2..])
    } else if syllable.starts_with(b"ng") {
        (b'v', &syllable[2..])
    } else if syllable.starts_with(b"j") {
        (b'y', &syllable[1..])
    } else if b"bpmfdtnlgkhzcsw".contains(&syllable[0]) {
        (syllable[0], &syllable[1..])
    } else {
        (b'o', syllable)
    };
    let final_key = match rest {
        b"a" | b"aa" => b'a',
        b"ou" => b'b',
        b"aam" | b"aap" => b'c',
        b"am" | b"ap" => b'd',
        b"e" => b'e',
        b"an" | b"at" => b'f',
        b"ang" | b"ak" | b"eng" => b'g',
        b"aang" | b"aak" => b'h',
        b"i" => b'i',
        b"aan" | b"aat" => b'j',
        b"aau" | b"au" => b'k',
        b"aai" | b"ai" => b'l',
        b"im" | b"ip" => b'm',
        b"in" => b'n',
        b"o" | b"oi" => b'o',
        b"un" | b"ut" | b"ung" | b"eon" => b'p',
        b"iu" => b'q',
        b"yun" | b"yut" | b"uk" => b'r',
        b"on" | b"ong" | b"ok" | b"ot" => b's',
        b"it" | b"ek" => b't',
        b"u" => b'u',
        b"yu" | b"ui" => b'v',
        b"oe" => b'w',
        b"oeng" | b"oek" => b'x',
        b"eoi" | b"eot" => b'y',
        b"ei" => b'z',
        b"ing" | b"ik" => b';',
        _ => return None,
    };
    Some([initial, final_key])
}

/// 从现有音节库存中找出与双拼码相符的粤拼音节。
///
/// 规则允许同一键代表多个长短韵母，因此反向查找必须以实际库存为边界；结果按字节序
/// 排序，让候选顺序不受 `HashSet` 的迭代顺序影响。
pub fn decode(inventory: &Inventory, code: [u8; 2]) -> Vec<String> {
    let mut values = inventory
        .syllables()
        .filter(|syllable| encode(syllable) == Some(code))
        .map(str::to_owned)
        .collect::<Vec<_>>();
    values.sort_unstable();
    values
}

#[cfg(test)]
mod tests {
    use super::{decode, encode};
    use crate::cantonese::Inventory;

    #[test]
    fn encodes_examples_without_copying_a_dictionary() {
        for (syllable, code) in [
            ("ngo", "vo"),
            ("nei", "nz"),
            ("hou", "hb"),
            ("keoi", "ky"),
            ("leng", "lg"),
            ("gwong", "is"),
            ("gwaan", "ij"),
            ("oi", "oo"),
            ("sek", "st"),
            ("m", "om"),
            ("ng", "ov"),
        ] {
            let bytes = code.as_bytes();
            assert_eq!(encode(syllable), Some([bytes[0], bytes[1]]), "{syllable}");
        }
    }

    #[test]
    fn rejects_unknown_or_malformed_syllables() {
        assert_eq!(encode(""), None);
        assert_eq!(encode("NG"), None);
        assert_eq!(encode("gx"), None);
        assert_eq!(encode("gwong2"), None);
    }

    #[test]
    fn reverse_lookup_is_bounded_by_the_loaded_inventory() {
        let inventory = Inventory::new(["ngo", "ngoi", "nei", "gaa", "gai"]);
        assert_eq!(decode(&inventory, *b"vo"), vec!["ngo", "ngoi"]);
        assert_eq!(decode(&inventory, *b"nz"), vec!["nei"]);
        assert_eq!(decode(&inventory, *b"ga"), vec!["gaa"]);
        assert!(decode(&inventory, *b"zz").is_empty());
    }
}
