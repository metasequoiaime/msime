//! 14／18 键全拼的横滑精确字母。组码串里的大写 ASCII 字母表示同一组里的一个精确小写字母（`KeyGrid::takes_exact_letters`）。全拼一个组码就是一个字母，所以精确字母只约束读法里同一位置的那个字母，音节在哪里分开与它无关。
//!
//! 标记留在组码串里，插入、删除、切分、锁定、部分选词和恢复都照旧搬动同一串字节。匹配只经过这里的几个小函数：`letters` 对精确码只给那一个字母（简拼、英文混入、左列的首字母随之收窄），`fits` 是逐字母的比对，拼写表按组码查找前先用 `group_codes` 把精确码换回整组码，再用 `marks_agree` 去掉精确位上字母不对的片段。没有精确码时这些都不改变原来的结果。决策见 `.agents/notes/implemented/feature/2026-11-08-grid-exact-swipe.md`。

use std::borrow::Cow;

use super::{KeyGrid, DIGIT_LIMIT};

const LETTERS: &str = "abcdefghijklmnopqrstuvwxyz";

impl KeyGrid {
    /// 收精确码的全拼网格：14 键和十八键。五笔 14 键共用 `FourteenKey`，由 `wubi::grid::representative` 拒收大写。
    fn quanpin_exact(self) -> bool {
        matches!(self, Self::FourteenKey | Self::EighteenKey)
    }

    /// 全拼网格上大写精确码代表的那一个小写字母；其他网格或整组码为 `None`。
    pub(super) fn exact_letter(self, code: u8) -> Option<&'static str> {
        if !self.quanpin_exact() || !code.is_ascii_uppercase() {
            return None;
        }
        let index = usize::from(code - b'A');
        LETTERS.get(index..=index)
    }

    /// 把 `typed` 里的精确码换回它那一组的整组码，给按组码建的拼写表查找用。没有精确码或不是全拼网格时为 `None`，调用方直接用 `typed`；有时写进栈上的 `buffer`，只有超过 `DIGIT_LIMIT` 的串才分配。
    pub(super) fn group_codes<'a>(
        self,
        typed: &str,
        buffer: &'a mut [u8; DIGIT_LIMIT],
    ) -> Option<Cow<'a, str>> {
        let keypad = self.keypad().filter(|_| self.quanpin_exact())?;
        if !typed.bytes().any(|byte| byte.is_ascii_uppercase()) {
            return None;
        }
        let group = |byte: u8| {
            if byte.is_ascii_uppercase() {
                keypad[usize::from(byte.to_ascii_lowercase() - b'a')]
            } else {
                byte
            }
        };
        if typed.len() > buffer.len() {
            return Some(Cow::Owned(
                typed.bytes().map(group).map(char::from).collect(),
            ));
        }
        for (slot, byte) in buffer.iter_mut().zip(typed.bytes()) {
            *slot = group(byte);
        }
        // 只把 ASCII 大写换成 ASCII 小写，其余字节原样，仍是合法 UTF-8。
        std::str::from_utf8(&buffer[..typed.len()])
            .ok()
            .map(Cow::Borrowed)
    }

    /// 锁定拼写时写回组码串的编码：拼写的整组码，已打的那几位上保留精确码，锁定后在拼写中间改字也不会把限定放宽回整组。双拼网格的编码由方案表给，照旧整段换成方案的编码。
    pub(super) fn locked_code(self, spelling: &str, typed: &str) -> String {
        let code = self.encode(spelling);
        if !self.quanpin_exact() {
            return code;
        }
        let mut bytes = code.into_bytes();
        for (byte, &mark) in bytes.iter_mut().zip(typed.as_bytes()) {
            if mark.is_ascii_uppercase() {
                *byte = mark;
            }
        }
        String::from_utf8(bytes).unwrap_or_default()
    }
}

/// 小写字母 `letter` 能不能由组码 `code` 打出：精确码只收它那一个字母，整组码收印在这一键上的字母。
pub(super) fn fits(keypad: &[u8; 26], letter: u8, code: u8) -> bool {
    if !letter.is_ascii_lowercase() {
        return false;
    }
    if code.is_ascii_uppercase() {
        letter == code.to_ascii_lowercase()
    } else {
        keypad[usize::from(letter - b'a')] == code
    }
}

/// 拼写片段在精确码那几位上是不是对应的字母；`typed` 是片段对应的那段原始组码（全拼一码一字母，比到较短的一方为止）。整组码那几位由按组码查表或比较保证。
pub(super) fn marks_agree(piece: &str, typed: &[u8]) -> bool {
    piece
        .bytes()
        .zip(typed)
        .all(|(letter, &code)| !code.is_ascii_uppercase() || letter == code.to_ascii_lowercase())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::nine_key::{SpellingTable, SyllablePrior};
    use std::collections::BTreeSet;

    const GRIDS: [KeyGrid; 2] = [KeyGrid::FourteenKey, KeyGrid::EighteenKey];

    #[test]
    fn exact_codes_stand_for_one_letter_only_on_quanpin_grids() {
        for grid in GRIDS {
            assert_eq!(grid.code_of(b'N'), Some(b'N'), "{grid:?}");
            assert_eq!(grid.letters(b'N'), "n", "{grid:?}");
            assert!(grid.is_code(b'N'), "{grid:?}");
            let keypad = grid.keypad().unwrap();
            assert!(fits(keypad, b'n', b'N') && !fits(keypad, b'b', b'N'));
            assert!(fits(keypad, b'n', b'b') && fits(keypad, b'b', b'b'));
            assert!(!fits(keypad, b'\'', b'N'));
        }
        assert_eq!(KeyGrid::NineKey.code_of(b'N'), None);
        assert_eq!(KeyGrid::NineKey.letters(b'N'), "");
        assert_eq!(KeyGrid::NineKey.exact_letter(b'N'), None);
    }

    #[test]
    fn group_codes_borrow_unmarked_input_and_map_marks_on_the_stack() {
        let mut buffer = [0; DIGIT_LIMIT];
        assert!(KeyGrid::FourteenKey
            .group_codes("bugao", &mut buffer)
            .is_none());
        let mut buffer = [0; DIGIT_LIMIT];
        let mapped = KeyGrid::FourteenKey
            .group_codes("NuHao", &mut buffer)
            .unwrap();
        assert!(matches!(mapped, Cow::Borrowed(_)));
        assert_eq!(mapped, "bugao");
        let mut buffer = [0; DIGIT_LIMIT];
        assert_eq!(
            KeyGrid::EighteenKey
                .group_codes("NihaI", &mut buffer)
                .unwrap(),
            "bihai"
        );
        // 双拼网格的大写是方案表里的编码，不换。
        let table = crate::nine_key_shuangpin::resolve_eighteen_key_shuangpin(
            crate::types::ShuangpinProfileKind::Xiaohe,
            None,
        )
        .unwrap();
        let mut buffer = [0; DIGIT_LIMIT];
        assert!(KeyGrid::EighteenKeyShuangpin(table)
            .group_codes("Ni", &mut buffer)
            .is_none());
        let long = "N".repeat(DIGIT_LIMIT + 1);
        let mut buffer = [0; DIGIT_LIMIT];
        let owned = KeyGrid::FourteenKey
            .group_codes(&long, &mut buffer)
            .unwrap();
        assert_eq!(owned, "b".repeat(DIGIT_LIMIT + 1));
    }

    #[test]
    fn locked_codes_keep_the_typed_marks() {
        assert_eq!(KeyGrid::FourteenKey.locked_code("ni", "Nu"), "Nu");
        assert_eq!(KeyGrid::FourteenKey.locked_code("ni", "N"), "Nu");
        assert_eq!(
            KeyGrid::EighteenKey.locked_code("zhuang", "zhuaN"),
            "zhuaNf"
        );
        assert_eq!(KeyGrid::FourteenKey.locked_code("ni", "bu"), "bu");
    }

    /// 带精确码的切分只有精确位上字母对得上的路径，而且只按整组切分时留下的相容路径一条不少（不相容的路径让出位置后，每个位置的 `PATH_LIMIT` 条里可能多留一些）。每个音节在两字母键上的位置，逐个子集标成它自己的字母，再全部标成同键的另一个字母，各核对一遍。
    #[test]
    fn marked_paths_are_the_unmarked_paths_whose_letters_agree() {
        for grid in GRIDS {
            let table = grid.spelling_table();
            let prior = SyllablePrior::shipped(table);
            for (syllable, code) in &table.syllables {
                let doubled: Vec<usize> = (0..code.len())
                    .filter(|&index| grid.letters(code.as_bytes()[index]).len() == 2)
                    .collect();
                let other = |index: usize| {
                    let letters = grid.letters(code.as_bytes()[index]).as_bytes();
                    letters[usize::from(letters[0] == syllable.as_bytes()[index])]
                };
                let mut variants: Vec<(String, bool)> = (1..(1u32 << doubled.len()))
                    .map(|mask| {
                        let mut typed = code.clone().into_bytes();
                        for (bit, &index) in doubled.iter().enumerate() {
                            if mask & (1 << bit) != 0 {
                                typed[index] = syllable.as_bytes()[index].to_ascii_uppercase();
                            }
                        }
                        (String::from_utf8(typed).unwrap(), true)
                    })
                    .collect();
                if !doubled.is_empty() {
                    let mut typed = code.clone().into_bytes();
                    for &index in &doubled {
                        typed[index] = other(index).to_ascii_uppercase();
                    }
                    variants.push((String::from_utf8(typed).unwrap(), false));
                }
                let unmarked = table.split_paths_limited(code, &[], &prior, usize::MAX);
                for (typed, own) in variants {
                    let marked: BTreeSet<Vec<String>> = table
                        .split_paths_limited(&typed, &[], &prior, usize::MAX)
                        .into_iter()
                        .collect();
                    assert!(
                        marked
                            .iter()
                            .all(|path| marks_agree(&path.concat(), typed.as_bytes())),
                        "{grid:?} {typed}"
                    );
                    let compatible = unmarked
                        .iter()
                        .filter(|path| marks_agree(&path.concat(), typed.as_bytes()));
                    for path in compatible {
                        assert!(marked.contains(path), "{grid:?} {typed} {path:?}");
                    }
                    if own {
                        assert!(marked.contains(&vec![syllable.clone()]), "{grid:?} {typed}");
                    }
                }
            }
        }
    }

    #[test]
    fn spellings_keep_only_syllables_whose_marked_letters_agree() {
        let prior = SyllablePrior::default();
        for (grid, ambiguous, exact) in [
            (KeyGrid::FourteenKey, "bu", "Nu"),
            (KeyGrid::EighteenKey, "bi", "Ni"),
        ] {
            let table: &SpellingTable = grid.spelling_table();
            let all = table.spellings_for(ambiguous, 0, None, &prior);
            assert!(all.contains(&"bi".to_owned()) && all.contains(&"ni".to_owned()));
            let narrowed = table.spellings_for(exact, 0, None, &prior);
            assert!(narrowed.contains(&"ni".to_owned()), "{grid:?} {narrowed:?}");
            assert!(
                narrowed
                    .iter()
                    .all(|spelling| marks_agree(spelling, exact.as_bytes())),
                "{grid:?} {narrowed:?}"
            );
            assert_eq!(
                narrowed,
                all.iter()
                    .filter(|spelling| marks_agree(spelling, exact.as_bytes()))
                    .cloned()
                    .collect::<Vec<_>>(),
                "{grid:?}"
            );
            assert!(grid.matching_code_prefix_len("ni", exact).is_some());
            assert!(grid.matching_code_prefix_len("bi", exact).is_none());
        }
    }
}
