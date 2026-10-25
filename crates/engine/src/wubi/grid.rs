//! 五笔 14 键：触屏键面与全拼 14 键相同（`QW ER TY UI OP / AS DF GH JK L / ZX CV BN M`），一键是相邻两个字母。组码里每一位记这一组的代表字母，查码表时展开成这一组的每个字母。
//!
//! 代表字母沿用全拼 14 键的组首字母，只有 ZX 例外：五笔码没有 `z`，这一组只代表 `x`，所以记成 `x`。这样组码永远不含 `z`，`z` 通配、`z` 引导的拼音反查和「带 `z` 的码不自动上屏」这几条规则都碰不到它。
//!
//! 码长最多四位，每位最多两个字母，一次查询最多展开 16 个编码。

use crate::nine_key::KeyGrid;

/// 一次查询最多展开的编码数：四位、每位两个字母。
#[cfg(test)]
pub const MAX_EXPANSIONS: usize = 16;

/// 全拼 14 键的组码（`KeyGrid::FourteenKey::code_of` 的结果）在五笔里的代表字母。ZX 记成 `x`；不是 14 键组码时为 `None`。
pub fn representative(group_code: u8) -> Option<u8> {
    if group_code == b'z' {
        return Some(b'x');
    }
    (KeyGrid::FourteenKey.code_of(group_code) == Some(group_code)).then_some(group_code)
}

/// 字母所在键的代表字母；`z` 不是五笔码，没有代表字母。
pub fn representative_of(letter: u8) -> Option<u8> {
    if letter == b'z' {
        return None;
    }
    KeyGrid::FourteenKey
        .code_of(letter)
        .and_then(representative)
}

/// 代表字母那一组里的五笔字母，按键面顺序；ZX 只有 `x`。
pub fn letters(representative: u8) -> &'static str {
    match representative {
        b'x' => "x",
        b'z' => "",
        code => KeyGrid::FourteenKey.letters(code),
    }
}

/// `code` 的每个字母都落在 `representatives` 对应位的那一组里（只比 `representatives` 的长度）。码比组码短时不算。
pub fn code_matches(code: &str, representatives: &str) -> bool {
    code.len() >= representatives.len()
        && code
            .bytes()
            .zip(representatives.bytes())
            .all(|(letter, group)| representative_of(letter) == Some(group))
}

/// 把组码展开成全部编码，按每位组内字母的顺序做笛卡尔积，写进 `expansions`（先清空）。组码里有不认识的字节时为空。
pub fn expand_into(representatives: &str, expansions: &mut Vec<String>) {
    expansions.clear();
    if representatives.is_empty() {
        return;
    }
    expansions.push(String::with_capacity(representatives.len()));
    for group in representatives.bytes() {
        let group_letters = letters(group);
        if group_letters.is_empty() {
            expansions.clear();
            return;
        }
        let previous = expansions.len();
        for index in 0..previous {
            for (offset, letter) in group_letters.bytes().enumerate() {
                if offset + 1 == group_letters.len() {
                    expansions[index].push(char::from(letter));
                } else {
                    let mut next = expansions[index].clone();
                    next.push(char::from(letter));
                    expansions.push(next);
                }
            }
        }
    }
    expansions.sort_unstable();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_wubi_letter_has_exactly_one_group() {
        for letter in b'a'..=b'y' {
            let group = representative_of(letter).expect("五笔字母都在某个键上");
            assert!(
                letters(group).as_bytes().contains(&letter),
                "{} 应该在 {} 组里",
                char::from(letter),
                char::from(group)
            );
        }
        assert_eq!(representative_of(b'z'), None);
        assert_eq!(representative_of(b';'), None);
    }

    #[test]
    fn the_zx_key_stands_for_x_only() {
        assert_eq!(representative(b'z'), Some(b'x'));
        assert_eq!(representative_of(b'x'), Some(b'x'));
        assert_eq!(letters(b'x'), "x");
        // `x` 本身不是全拼 14 键的组码，只是五笔里 ZX 的代表字母。
        assert_eq!(representative(b'x'), None);
        assert_eq!(representative(b'w'), None);
        assert_eq!(representative(b'q'), Some(b'q'));
    }

    #[test]
    fn groups_follow_the_fourteen_key_face() {
        let groups: Vec<&str> = b"qetuoadgjlxcbm"
            .iter()
            .map(|&code| letters(code))
            .collect();
        assert_eq!(
            groups,
            ["qw", "er", "ty", "ui", "op", "as", "df", "gh", "jk", "l", "x", "cv", "bn", "m"]
        );
    }

    #[test]
    fn expansion_is_the_sorted_cartesian_product() {
        let mut expansions = Vec::new();
        expand_into("qq", &mut expansions);
        assert_eq!(expansions, ["qq", "qw", "wq", "ww"]);
        expand_into("lxm", &mut expansions);
        assert_eq!(expansions, ["lxm"]);
        expand_into("qetu", &mut expansions);
        assert_eq!(expansions.len(), MAX_EXPANSIONS);
        assert!(expansions.windows(2).all(|pair| pair[0] < pair[1]));
        expand_into("qz", &mut expansions);
        assert!(expansions.is_empty());
        expand_into("", &mut expansions);
        assert!(expansions.is_empty());
    }

    #[test]
    fn a_code_matches_the_groups_it_was_typed_on() {
        assert!(code_matches("wqiy", "qq"));
        assert!(code_matches("wq", "qq"));
        assert!(!code_matches("w", "qq"));
        assert!(!code_matches("vb", "qq"));
        assert!(code_matches("xx", "xx"));
    }
}
