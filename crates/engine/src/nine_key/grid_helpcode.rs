//! 十四／十八键的网格辅助码：拼音组码之后放下分隔符，再输入最多两槽辅助码，只用来筛排候选。
//!
//! 状态只在组码会话里，不写进组码、预编辑、上屏文字或句子种子；分隔符和辅助码由宿主按快照另画。九键与九键双拼没有接入，进入时被拒绝。决策见 `.agents/notes/implemented/feature/2026-11-04-grid-helpcode.md`。

use super::*;
use crate::helpcode::{
    match_single_helpcode, matches_double_helpcodes, SharedKeymap, SingleHelpcodeMatch,
};

/// 最多两槽，与单字的两码、词语的首尾码对应。
const SLOTS: usize = 2;

/// [`crate::Session::grid_helpcode`] 的操作。放下分隔符之后辅助码只从这里进，`grid_key` 一律不收；整串解释从拼音选择区的 `choose_nine_key_spelling` 选。字母只收小写：26 键双拼一码的大写是「尾字优先」，网格辅助码的大写留给与它一致的含义。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum GridHelpcodeOp {
    /// 在组码后放下分隔符；已经放下时不变。
    Begin,
    /// 按下的这个键：追加它上面的整组字母，`letter` 是组里任一个小写字母。
    Group(u8),
    /// 追加一个确定的小写字母，给已有的精确字母手势（如十八键双拼的横滑）用。
    Exact(u8),
    /// 清掉分隔符和全部辅助码。
    Clear,
}

/// 一槽：键面上的整组字母，确定了其中一个时只认它。
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct HelpcodeSlot {
    letters: &'static str,
    exact: Option<u8>,
}

impl HelpcodeSlot {
    fn allowed(&self) -> &[u8] {
        self.exact
            .as_ref()
            .map_or(self.letters.as_bytes(), std::slice::from_ref)
    }
}

/// 码表和严格偏好在放下分隔符时取定，组字中途改设置从下一次组字起生效。
pub(super) struct GridHelpcode {
    table: SharedKeymap,
    strict: bool,
    slots: Vec<HelpcodeSlot>,
}

impl GridHelpcode {
    /// 至少有一槽才筛选；只有分隔符时候选与普通组字相同。
    pub(super) fn constrains(&self) -> bool {
        !self.slots.is_empty()
    }

    /// 第 `index` 个整串解释：按键面顺序展开各槽允许的字母，第一槽在外层。各槽都已确定时没有可选的解释。
    fn combination(&self, index: usize) -> Option<[Option<u8>; SLOTS]> {
        let first = self.slots.first()?.allowed();
        let second = self.slots.get(1).map(HelpcodeSlot::allowed);
        let inner = second.map_or(1, <[u8]>::len);
        if first.len() * inner <= 1 || index >= first.len() * inner {
            return None;
        }
        Some([
            Some(first[index / inner]),
            second.map(|second| second[index % inner]),
        ])
    }

    fn combinations(&self) -> impl Iterator<Item = [Option<u8>; SLOTS]> + '_ {
        (0..).map_while(|index| self.combination(index))
    }

    /// 选一个整串解释，各槽一起锁定。
    fn choose(&mut self, index: usize) -> bool {
        let Some(letters) = self.combination(index) else {
            return false;
        };
        for (slot, letter) in self.slots.iter_mut().zip(letters) {
            slot.exact = letter;
        }
        true
    }

    /// 宿主画在读音后面的样子：`|` 接各槽，多义的槽写成方括号里的整组，如 `|Q[ER]`。
    fn display(&self, shuangpin: bool) -> String {
        let mut text = String::from("|");
        for (index, slot) in self.slots.iter().enumerate() {
            let ambiguous = slot.allowed().len() > 1;
            if ambiguous {
                text.push('[');
            }
            text.extend(
                slot.allowed()
                    .iter()
                    .map(|&letter| shown(letter, index, shuangpin)),
            );
            if ambiguous {
                text.push(']');
            }
        }
        text
    }

    /// 留下匹配的行，按覆盖、匹配等级稳定排序，同一等级保留原来的先后；每行只求一次等级。
    fn keep_matches(&self, candidates: &mut Vec<WordItem>, shuangpin: bool) {
        let mut graded: Vec<(WordItem, u8)> = candidates
            .drain(..)
            .filter_map(|item| self.grade(&item.word, shuangpin).map(|grade| (item, grade)))
            .collect();
        graded.sort_by_key(|(item, grade)| (Reverse(item.pinyin.len()), *grade));
        candidates.extend(graded.into_iter().map(|(item, _)| item));
    }

    /// 越小越靠前，`None` 是筛掉。槽里多个字母按并集算，任一个命中即可。两码按码序严格匹配。一码不严格时首码（或首尾都）命中、只有尾码命中、都不中依次排；严格时都不中的筛掉，全拼只筛不排（与 26 键相同），双拼仍让首码命中的在前。
    fn grade(&self, word: &str, shuangpin: bool) -> Option<u8> {
        let first = self.slots.first()?.allowed();
        if let Some(second) = self.slots.get(1) {
            let second = second.allowed();
            let matched = first.iter().any(|&a| {
                second
                    .iter()
                    .any(|&b| matches_double_helpcodes(word, ascii(&[a, b]), &self.table))
            });
            return matched.then_some(0);
        }
        let (mut leading, mut trailing) = (false, false);
        for &letter in first {
            match match_single_helpcode(word, ascii(&[letter]), &self.table) {
                SingleHelpcodeMatch::First => leading = true,
                SingleHelpcodeMatch::Last => trailing = true,
                SingleHelpcodeMatch::Both => (leading, trailing) = (true, true),
                SingleHelpcodeMatch::None => {}
            }
        }
        match (leading, trailing) {
            (false, false) if self.strict => None,
            (false, false) => Some(2),
            _ if self.strict && !shuangpin => Some(0),
            (true, _) => Some(0),
            (false, true) => Some(1),
        }
    }
}

/// 全拼两位都大写；双拼首码小写、次码大写。只是显示，不影响匹配。
fn shown(letter: u8, index: usize, shuangpin: bool) -> char {
    char::from(if shuangpin && index == 0 {
        letter
    } else {
        letter.to_ascii_uppercase()
    })
}

fn ascii(letters: &[u8]) -> &str {
    std::str::from_utf8(letters).unwrap_or_default()
}

impl NineKeySession {
    /// 辅助码字母所在的键面：十四／十八键全拼和双拼的字母键排法相同。双拼网格的 `letters` 是能起头音节的字母，不能当键面用。
    fn helpcode_keyboard(&self) -> Option<KeyGrid> {
        match self.grid {
            KeyGrid::FourteenKey | KeyGrid::FourteenKeyShuangpin(_) => Some(KeyGrid::FourteenKey),
            KeyGrid::EighteenKey | KeyGrid::EighteenKeyShuangpin(_) => Some(KeyGrid::EighteenKey),
            KeyGrid::NineKey | KeyGrid::NineKeyShuangpin(_) => None,
        }
    }

    fn helpcode_shuangpin(&self) -> bool {
        self.grid.shuangpin_table().is_some()
    }

    /// 这次组字能不能放下分隔符：十四／十八键在拼组码、光标在末尾。
    pub(crate) fn grid_helpcode_ready(&self) -> bool {
        self.active()
            && !self.english_only
            && self.caret.is_none()
            && self.helpcode_keyboard().is_some()
    }

    pub(super) fn grid_helpcode_constrains(&self) -> bool {
        self.helpcode.as_ref().is_some_and(GridHelpcode::constrains)
    }

    /// 有辅助码而筛空：结束组字的键（空格、标点、数字、首选上屏）一律吞掉，不能让它们先上屏、组字留在后面。
    pub(crate) fn grid_helpcode_blocks(&self) -> bool {
        self.candidates.is_empty() && self.grid_helpcode_constrains()
    }

    /// `None` 是不接受，`Some(false)` 是接受了但没有变化（重复放下分隔符）。放下分隔符时清掉笔画筛选，单字筛选保留。
    pub(crate) fn begin_grid_helpcode(
        &mut self,
        table: SharedKeymap,
        strict: bool,
    ) -> Option<bool> {
        if self.helpcode.is_some() {
            return Some(false);
        }
        if !self.grid_helpcode_ready() {
            return None;
        }
        self.helpcode = Some(GridHelpcode {
            table,
            strict,
            slots: Vec::with_capacity(SLOTS),
        });
        if !self.strokes.is_empty() {
            self.strokes.clear();
            self.stroke_texts.clear();
            self.refresh();
        }
        Some(true)
    }

    /// `Group`、`Exact` 在有分隔符时往下一槽放整组或确定的字母；两槽已满时接受但不变。只收小写（见 [`GridHelpcodeOp`]）。
    pub(crate) fn append_grid_helpcode(&mut self, letter: u8, exact: bool) -> Option<bool> {
        if self.helpcode.is_none() || !letter.is_ascii_lowercase() {
            return None;
        }
        Some(self.push_helpcode_slot(letter, exact))
    }

    fn push_helpcode_slot(&mut self, letter: u8, exact: bool) -> bool {
        let Some(keyboard) = self.helpcode_keyboard() else {
            return false;
        };
        let Some(letters) = keyboard.code_of(letter).map(|code| keyboard.letters(code)) else {
            return false;
        };
        let Some(helpcode) = self
            .helpcode
            .as_mut()
            .filter(|helpcode| helpcode.slots.len() < SLOTS)
        else {
            return false;
        };
        helpcode.slots.push(HelpcodeSlot {
            letters,
            exact: (exact || letters.len() == 1).then_some(letter),
        });
        self.refresh();
        true
    }

    /// 有分隔符时拼音选择区里是整串解释，选中一项锁定各槽，不上屏。
    pub(super) fn choose_grid_helpcode(&mut self, index: usize) -> KeyResult {
        if !self
            .helpcode
            .as_mut()
            .is_some_and(|helpcode| helpcode.choose(index))
        {
            return KeyResult::unhandled();
        }
        self.refresh();
        KeyResult::handled()
    }

    /// 去掉分隔符和辅助码，返回原来是否在筛选；要不要刷新由调用方决定。
    pub(super) fn drop_grid_helpcode(&mut self) -> bool {
        self.helpcode
            .take()
            .is_some_and(|helpcode| helpcode.constrains())
    }

    /// `None` 是本来就没有分隔符。
    pub(crate) fn clear_grid_helpcode(&mut self) -> Option<bool> {
        self.helpcode.as_ref()?;
        if self.drop_grid_helpcode() {
            self.refresh();
        }
        Some(true)
    }

    /// 有分隔符时先处理的命令：退格先删槽、再去掉分隔符，这两次都不删组码；筛空时首选上屏吞掉，原样上屏（宿主在插入别的内容之前用它结束组字）丢掉这次组字、什么也不上屏，免得只剩组码字母的组码被当原文上屏；移光标和向后删除走进主码编辑，先去掉辅助码。返回 `None` 时照常处理。
    pub(super) fn grid_helpcode_command(&mut self, command: Command) -> Option<KeyResult> {
        let constrained = self.helpcode.as_ref()?.constrains();
        match command {
            Command::Backspace => {
                if !constrained {
                    self.helpcode = None;
                } else if let Some(helpcode) = self.helpcode.as_mut() {
                    helpcode.slots.pop();
                }
                if constrained {
                    self.refresh();
                }
                Some(KeyResult::handled())
            }
            Command::CommitCandidate if self.grid_helpcode_blocks() => Some(KeyResult::handled()),
            Command::CommitRaw if self.grid_helpcode_blocks() => {
                Some(self.command(Command::Cancel))
            }
            Command::MoveLeft
            | Command::MoveRight
            | Command::MoveHome
            | Command::MoveEnd
            | Command::DeleteForward => {
                self.helpcode = None;
                if constrained {
                    self.refresh();
                }
                None
            }
            _ => None,
        }
    }

    /// 有辅助码条件时的刷新收尾，接在基础排序和键盘模型重排之后：在整张已查到的列表上筛排、截出候选池，再照粗俗词政策挪出首屏。固定位置、首页补全折算、裁决标记、句子种子和混入行都不做。
    pub(super) fn settle_grid_helpcode(
        &mut self,
        mut candidates: Vec<WordItem>,
        alternatives: Vec<Path>,
    ) {
        if let Some(helpcode) = &self.helpcode {
            helpcode.keep_matches(&mut candidates, self.helpcode_shuangpin());
        }
        candidates.truncate(CANDIDATE_LIMIT);
        let vulgar = self.offensive_words();
        keep_off_first_page(&mut candidates, self.first_page_rows.get(), |item| {
            grid_demotion(item, &vulgar)
        });
        let mut reading = std::mem::take(&mut self.reading);
        self.write_reading(candidates.first(), &mut reading);
        self.reading = reading;
        self.candidates = candidates;
        self.path_buffer = alternatives;
    }

    pub(super) fn grid_helpcode_display(&self) -> String {
        let shuangpin = self.helpcode_shuangpin();
        self.helpcode
            .as_ref()
            .map_or_else(String::new, |helpcode| helpcode.display(shuangpin))
    }

    /// 快照里的拼音选择区：有分隔符时是整串解释（只有分隔符或各槽都已确定时为空），拼音选项留着，去掉分隔符后照旧显示。
    pub(super) fn grid_helpcode_choices(&self) -> Option<Vec<String>> {
        let shuangpin = self.helpcode_shuangpin();
        let helpcode = self.helpcode.as_ref()?;
        Some(
            helpcode
                .combinations()
                .map(|letters| {
                    letters
                        .into_iter()
                        .flatten()
                        .enumerate()
                        .map(|(index, letter)| shown(letter, index, shuangpin))
                        .collect()
                })
                .collect(),
        )
    }
}
