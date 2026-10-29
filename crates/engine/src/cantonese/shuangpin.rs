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

/// 末尾单独一个双拼键只定下了声母：返回库存里首键为 `key` 的音节在粤拼里的开头拼写，供按前缀补全。
///
/// 声母键取对应的粤拼声母（`i` 是 `gw`、`v` 是 `ng`、`y` 是 `j`），零声母键 `o` 按各音节的首字母分组，`m`、`ng` 两个鼻音音节以整个音节作前缀。`g`、`n` 这样的前缀也会补出 `gw`、`ng` 开头的音节，调用方须先用 `key_spans` 收窄。结果去重并按字节序排序；库存里没有以该键开头的音节时为空。
pub fn initial_prefixes(inventory: &Inventory, key: u8) -> Vec<String> {
    let mut prefixes = inventory
        .syllables()
        .filter(|syllable| encode(syllable).is_some_and(|code| code[0] == key))
        .map(|syllable| {
            let length = if matches!(syllable, "m" | "ng") {
                syllable.len()
            } else if matches!(key, b'i' | b'u' | b'v') {
                2
            } else {
                1
            };
            syllable[..length].to_owned()
        })
        .collect::<Vec<_>>();
    prefixes.sort_unstable();
    prefixes.dedup();
    prefixes
}

/// 把 `initial_prefixes` 给出的一个前缀收窄成只落在 `key` 上的若干段，每段是 `(拼写, 是否整音节)`：整音节段只查这个音节本身，其余段按前缀补全，段下库存里没有首键是别的键的音节。
///
/// 前缀下没有别的键的音节时原样返回这一个前缀；`g` 在 `gw` 处分开成 `ga`、`ge`、`go`……，`n` 绕开 `ng`，零声母键的 `m` 只剩音节 `m` 自己。`encode` 不认识的音节（库存里的 `deu`、`gep` 一类口语音）不在任何键上，也不拆段，调用方查询后仍要用 `encode` 筛掉它们。各段互不重叠，按字节序排序，免得别的键上的重行占满查询上限。
pub fn key_spans(inventory: &Inventory, prefix: &str, key: u8) -> Vec<(String, bool)> {
    let on_key = |syllable: &str| encode(syllable).is_some_and(|code| code[0] == key);
    let on_other_key = |syllable: &str| encode(syllable).is_some_and(|code| code[0] != key);
    let under_prefix = inventory
        .syllables()
        .filter(|syllable| syllable.starts_with(prefix))
        .collect::<Vec<_>>();
    let mut spans = Vec::new();
    let mut pending = vec![prefix.to_owned()];
    while let Some(span) = pending.pop() {
        let under = under_prefix
            .iter()
            .copied()
            .filter(|syllable| syllable.starts_with(span.as_str()))
            .collect::<Vec<_>>();
        if !under.iter().any(|syllable| on_key(syllable)) {
            continue;
        }
        if !under.iter().any(|syllable| on_other_key(syllable)) {
            spans.push((span, false));
            continue;
        }
        if under.contains(&span.as_str()) && on_key(&span) {
            spans.push((span.clone(), true));
        }
        let mut next = under
            .iter()
            .filter(|syllable| syllable.len() > span.len())
            .map(|syllable| syllable.as_bytes()[span.len()])
            .collect::<Vec<_>>();
        next.sort_unstable();
        next.dedup();
        for byte in next {
            let mut longer = span.clone();
            longer.push(char::from(byte));
            pending.push(longer);
        }
    }
    spans.sort_unstable();
    spans
}

#[cfg(test)]
mod tests {
    use super::{decode, encode, initial_prefixes, key_spans};
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

    #[test]
    fn a_lone_initial_key_lists_the_jyutping_spellings_it_starts() {
        let inventory = Inventory::new([
            "gwong", "gaa", "ngo", "nei", "m", "ng", "oi", "aa", "maa", "jan",
        ]);
        assert_eq!(initial_prefixes(&inventory, b'i'), ["gw"]);
        assert_eq!(initial_prefixes(&inventory, b'g'), ["g"]);
        assert_eq!(initial_prefixes(&inventory, b'v'), ["ng"]);
        assert_eq!(initial_prefixes(&inventory, b'n'), ["n"]);
        assert_eq!(initial_prefixes(&inventory, b'y'), ["j"]);
        assert_eq!(initial_prefixes(&inventory, b'm'), ["m"]);
        // 零声母按首字母分组，两个鼻音音节自成前缀。
        assert_eq!(initial_prefixes(&inventory, b'o'), ["a", "m", "ng", "o"]);
        assert!(initial_prefixes(&inventory, b'x').is_empty());
    }

    #[test]
    fn key_spans_split_a_prefix_around_syllables_of_other_keys() {
        let inventory = Inventory::new([
            "gaa", "gei", "go", "gwong", "gwaa", "nei", "naa", "ngo", "ng", "m", "maa", "mui",
            "daa", "daai", "deu",
        ]);
        // 前缀下没有别的键的音节时不拆；`encode` 不认识的 `deu` 不算别的键。
        assert_eq!(key_spans(&inventory, "d", b'd'), [("d".to_owned(), false)]);
        // `g` 绕开 `gw`，`n` 绕开 `ng`。
        assert_eq!(
            key_spans(&inventory, "g", b'g'),
            [
                ("ga".to_owned(), false),
                ("ge".to_owned(), false),
                ("go".to_owned(), false)
            ]
        );
        assert_eq!(
            key_spans(&inventory, "n", b'n'),
            [("na".to_owned(), false), ("ne".to_owned(), false)]
        );
        // 零声母键的 `m`、`ng` 只剩整音节，`m` 键的 `m` 前缀不含音节 `m`。
        assert_eq!(key_spans(&inventory, "m", b'o'), [("m".to_owned(), true)]);
        assert_eq!(key_spans(&inventory, "ng", b'o'), [("ng".to_owned(), true)]);
        assert_eq!(
            key_spans(&inventory, "m", b'm'),
            [("ma".to_owned(), false), ("mu".to_owned(), false)]
        );
        assert!(key_spans(&inventory, "g", b'x').is_empty());
    }
}
