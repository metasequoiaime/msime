//! 网格辅筛的显式编辑协议；不复用拼音的大写精确标记或竖线字符。

use crate::helpcode::{self, HelpcodeKeymap, SingleHelpcodeMatch};
use crate::nine_key::KeyGrid;
use crate::types::WordItem;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum GridHelpcodeAction {
    Begin,
    ToggleCase,
    /// 九键传数字键，十四／十八键传该组首个小写字母。
    AppendGroup(u8),
    /// 横滑得到的小写字母；大小写由编辑状态决定。
    AppendExact(u8),
    Choose(String),
    End,
    Clear,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum GridHelpcodeAvailability {
    Available,
    Disabled,
    NotComposing,
    Unsupported,
    MissingTable,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GridHelpcodeSlot {
    /// 键面顺序的小写字母，锁定后仍保留来源。
    pub letters: String,
    pub exact: Option<u8>,
    pub uppercase: bool,
}

impl GridHelpcodeSlot {
    fn effective(&self) -> &[u8] {
        self.exact
            .as_ref()
            .map_or(self.letters.as_bytes(), std::slice::from_ref)
    }

    fn display(&self, letter: u8) -> char {
        char::from(if self.uppercase {
            letter.to_ascii_uppercase()
        } else {
            letter
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct GridHelpcodeSnapshot {
    pub availability: GridHelpcodeAvailability,
    /// 所有专用操作均核对此代次，编辑状态变化也使旧选择失效。
    pub generation: u64,
    /// 候选刷新代次；纯编辑状态变化不递增，供后续运行时保持翻页位置。
    pub candidate_generation: u64,
    pub editing: bool,
    pub uppercase: bool,
    pub slots: Vec<GridHelpcodeSlot>,
    pub choices: Vec<String>,
    /// 仅 H，不含拼音或竖线，不可写入应用 composing text。
    pub display: String,
}

#[derive(Default)]
pub(crate) struct GridHelpcodeState {
    pub generation: u64,
    pub candidate_generation: u64,
    pub editing: bool,
    pub uppercase: bool,
    pub slots: Vec<GridHelpcodeSlot>,
}

impl GridHelpcodeState {
    pub fn changed(&mut self) {
        self.generation = self.generation.wrapping_add(1);
    }

    pub fn active(&self) -> bool {
        !self.slots.is_empty()
    }

    pub fn clear(&mut self) {
        if self.editing || self.active() || self.uppercase {
            self.slots.clear();
            self.editing = false;
            self.uppercase = false;
            self.changed();
        }
    }

    pub fn combinations(&self) -> Vec<String> {
        if !self.active() {
            return Vec::new();
        }
        let mut values = vec![String::new()];
        for slot in &self.slots {
            values = values
                .into_iter()
                .flat_map(|prefix| {
                    slot.effective().iter().map(move |&letter| {
                        let mut value = prefix.clone();
                        value.push(slot.display(letter));
                        value
                    })
                })
                .collect();
        }
        values
    }

    pub fn snapshot(&self, availability: GridHelpcodeAvailability) -> GridHelpcodeSnapshot {
        let mut display = String::new();
        for slot in &self.slots {
            let ambiguous = slot.effective().len() > 1;
            if ambiguous {
                display.push('[');
            }
            display.extend(slot.effective().iter().map(|&letter| slot.display(letter)));
            if ambiguous {
                display.push(']');
            }
        }
        let choices = if self.editing && self.slots.iter().any(|slot| slot.effective().len() > 1) {
            self.combinations()
        } else {
            Vec::new()
        };
        GridHelpcodeSnapshot {
            availability,
            generation: self.generation,
            candidate_generation: self.candidate_generation,
            editing: self.editing,
            uppercase: self.uppercase,
            slots: self.slots.clone(),
            choices,
            display,
        }
    }

    /// 直接复用共享匹配器，最多枚举 16 个解释；取码规则不另写一份。
    fn grade(&self, word: &str, map: &HelpcodeKeymap) -> Option<u8> {
        let first = self.slots.first()?;
        if self.slots.len() == 1 {
            let mut grade = 2;
            for &letter in first.effective() {
                let code = [letter];
                let matched =
                    helpcode::match_single_helpcode(word, std::str::from_utf8(&code).unwrap(), map);
                let value = match matched {
                    SingleHelpcodeMatch::Both => 0,
                    SingleHelpcodeMatch::First => u8::from(first.uppercase),
                    SingleHelpcodeMatch::Last => u8::from(!first.uppercase),
                    SingleHelpcodeMatch::None => 2,
                };
                grade = grade.min(value);
            }
            Some(grade)
        } else {
            for &a in first.effective() {
                for &b in self.slots[1].effective() {
                    let code = if first.uppercase { [b, a] } else { [a, b] };
                    if helpcode::matches_double_helpcodes(
                        word,
                        std::str::from_utf8(&code).unwrap(),
                        map,
                    ) {
                        return Some(0);
                    }
                }
            }
            None
        }
    }

    pub fn rank(&self, rows: &mut Vec<WordItem>, map: &HelpcodeKeymap) {
        rows.retain(|row| self.grade(&row.word, map).is_some());
        rows.sort_by_cached_key(|row| {
            (
                std::cmp::Reverse(row.pinyin.len()),
                self.grade(&row.word, map),
            )
        });
    }
}

/// 形码键面始终是拉丁字母分组，不使用双拼声韵母映射。
pub(crate) fn group_letters(grid: KeyGrid, code: u8) -> &'static str {
    let grid = match grid {
        KeyGrid::NineKey | KeyGrid::NineKeyShuangpin(_) => KeyGrid::NineKey,
        KeyGrid::FourteenKey | KeyGrid::FourteenKeyShuangpin(_) => KeyGrid::FourteenKey,
        KeyGrid::EighteenKey | KeyGrid::EighteenKeyShuangpin(_) => KeyGrid::EighteenKey,
    };
    grid.letters(code)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn grid_helpcode_matching_inherits_shared_single_mixed_missing_and_overwrite_rules() {
        let map =
            HelpcodeKeymap::from_table_bytes("你=zz\n你=ab\n好=bc\n同=aa\nA你=ba\n".as_bytes())
                .unwrap();
        for word in [
            "你", "好", "同", "你好", "好你", "A你", "你A", "A你好B", "缺", "", "abc",
        ] {
            for uppercase in [false, true] {
                for a in b'a'..=b'c' {
                    let mut state = GridHelpcodeState {
                        slots: vec![GridHelpcodeSlot {
                            letters: char::from(a).to_string(),
                            exact: None,
                            uppercase,
                        }],
                        ..Default::default()
                    };
                    let shared =
                        helpcode::match_single_helpcode(word, &char::from(a).to_string(), &map);
                    let expected = match shared {
                        SingleHelpcodeMatch::Both => 0,
                        SingleHelpcodeMatch::First => u8::from(uppercase),
                        SingleHelpcodeMatch::Last => u8::from(!uppercase),
                        SingleHelpcodeMatch::None => 2,
                    };
                    assert_eq!(state.grade(word, &map), Some(expected));
                    for b in b'a'..=b'c' {
                        for second_uppercase in [false, true] {
                            state.slots.truncate(1);
                            state.slots.push(GridHelpcodeSlot {
                                letters: char::from(b).to_string(),
                                exact: None,
                                uppercase: second_uppercase,
                            });
                            let pair = if uppercase { [b, a] } else { [a, b] };
                            let expected = helpcode::matches_double_helpcodes(
                                word,
                                std::str::from_utf8(&pair).unwrap(),
                                &map,
                            );
                            assert_eq!(
                                state.grade(word, &map).is_some(),
                                expected,
                                "{word} {uppercase} {second_uppercase}"
                            );
                        }
                    }
                }
            }
        }
        // 历史函数名误导：实际数全部字符，混合文字走首尾汉字分支。
        assert!(helpcode::matches_double_helpcodes("A你", "aa", &map));
        assert!(helpcode::matches_double_helpcodes("你A", "aa", &map));
        assert!(!helpcode::matches_double_helpcodes("你A", "ab", &map));
    }

    #[test]
    fn grid_helpcode_set_union_stable_order_and_coverage_override_positions() {
        let map = HelpcodeKeymap::from_table_bytes("你=ab\n好=ba\n同=aa".as_bytes()).unwrap();
        let state = GridHelpcodeState {
            slots: vec![GridHelpcodeSlot {
                letters: "ab".into(),
                exact: None,
                uppercase: false,
            }],
            ..Default::default()
        };
        let mut rows: Vec<_> = [
            ("缺", "64"),
            ("好", "64"),
            ("你", "64"),
            ("同", "64"),
            ("你好", "64426"),
        ]
        .into_iter()
        .map(|(word, pinyin)| WordItem {
            word: word.into(),
            pinyin: pinyin.into(),
            fixed_position: 1,
            ..Default::default()
        })
        .collect();
        state.rank(&mut rows, &map);
        assert_eq!(
            rows.iter()
                .map(|row| row.word.as_ref())
                .collect::<Vec<&str>>(),
            ["你好", "好", "你", "同", "缺"]
        );
        assert_eq!(rows.len(), 5, "多个解释命中不复制候选");
    }
}
