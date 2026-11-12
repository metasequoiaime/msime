//! 九键读音的容错切分：按键切不成音节时读音行仍然是字母，不退回键码。
//!
//! 手机全拼九键打长句时多按或按错一键，整串就切不成音节。原先首行覆盖不到的尾段这时原样写键码（`jin'84268426774936639264`），没有首行时读音为空、宿主只能画整串数字。这里守住一条不变量：全拼九键在拼音组字期间读音非空、不含数字，去掉 `'` 后一位对一个已按的键，每个字母都印在那个键上，宿主靠这一点把读音和按键逐位对上（点读音移光标、放大编辑条）。
//!
//! 做法是在严格切分失败之后再切一次，多一种转移：切不开的那一键单独成段，显示这个键上频度最高的声母。能成音节的照常成音节，单键段越少越好。这不是纠错：不出键外的字母，不丢键，候选也不因此改变（整串切不开时候选退到错键之前的前缀是另一件事，见 `typo_prefix`）。三种双拼网格一个音节两位，单独跳过一位会让后面的配对整体错位，不纳入；英文专用模式的读音照旧为空。取舍见 `.agents/notes/implemented/bug-fix/2026-11-12-nine-key-typo-reading-and-prefix-candidates.md`，尾段原来的做法见同目录的 `2026-11-02-nine-key-uncovered-tail-reading.md`。

use super::{
    fits, marks_agree, KeyGrid, NineKeySession, SpellingTable, SyllablePrior, DIGIT_LIMIT,
};
use crate::types::{CandidateSource, WordItem};
use std::cmp::Ordering;

/// 从某个键位到末尾的最好一条切分的第一段，`end` 指向下一段开始的键位。每个键位只留一条，整张表放在栈上。
#[derive(Clone, Copy)]
struct Segment<'a> {
    piece: &'a str,
    end: usize,
    /// 这条切分里单独成段的键数。
    strays: usize,
    /// 这条切分的段数，末尾的哨兵是 0。
    pieces: usize,
    /// 最后一段是不是完整音节。
    complete: bool,
    /// 各音节段的频度之和；单键段不计分。
    score: f64,
}

impl<'a> Segment<'a> {
    /// 从这一段起依次的各段。
    fn spelled<'n>(self, nodes: &'n [Segment<'a>]) -> impl Iterator<Item = &'a str> + 'n
    where
        'a: 'n,
    {
        let mut next = (self.pieces != 0).then_some(self);
        std::iter::from_fn(move || {
            let segment = next?;
            next = nodes
                .get(segment.end)
                .filter(|node| node.pieces != 0)
                .copied();
            Some(segment.piece)
        })
    }

    /// 排在前面的更好：单键段少、段数少、末段完整、频度和高，最后按字典序。后四项与严格切分（`split_paths_with_scratch`）的排序相同，所以整串切得开时得到的就是严格切分的第一条。
    fn order(self, other: Self, nodes: &[Segment<'a>]) -> Ordering {
        self.strays
            .cmp(&other.strays)
            .then_with(|| self.pieces.cmp(&other.pieces))
            .then_with(|| other.complete.cmp(&self.complete))
            .then_with(|| other.score.total_cmp(&self.score))
            .then_with(|| self.spelled(nodes).cmp(other.spelled(nodes)))
    }
}

impl SpellingTable {
    /// 把 `digits` 切成读音接到 `reading` 后面，段与段之间是 `'`。音节段的规则与严格切分相同：按整组码查表、精确码的位置字母必须对上、不跨 `splits`（`digits` 里的下标）、中间的段和切分处结束的段必须是完整音节，只有最后一段可以没打完。另外任何一键都可以单独成段（`fallback_letter`），所以全拼网格上总有结果。`initial` 是用户选定的首字母，第一段以它开头。
    ///
    /// 单键段不向 `prior` 要分数：表外的片段查出来是 0.0，比任何真实音节的负分都高，计分的话三个单键段会压过一个音节。`prior` 为 `None`（只有不经 `refresh` 的单测会话）时频度都按 0 算，仍然出字母。
    ///
    /// 只对按字母归组的全拼网格有效；双拼网格、空串、超过 `DIGIT_LIMIT` 的串返回 `false`，不动 `reading`。从末尾往前每个键位只留最好的一条，最多 48 个键位各查 `longest_code` 次表，不做堆分配。
    pub(super) fn lenient_split<'a>(
        &'a self,
        digits: &'a str,
        splits: &[usize],
        prior: Option<&SyllablePrior<'_>>,
        initial: Option<u8>,
        reading: &mut String,
    ) -> bool {
        let length = digits.len();
        if length == 0 || length > DIGIT_LIMIT || !digits.is_ascii() || self.grid.keypad().is_none()
        {
            return false;
        }
        let mut buffer = [0; DIGIT_LIMIT];
        let groups = self.grid.group_codes(digits, &mut buffer);
        let codes = groups.as_deref().unwrap_or(digits);
        let typed = digits.as_bytes();
        let mut nodes = [Segment {
            piece: "",
            end: length,
            strays: 0,
            pieces: 0,
            complete: false,
            score: 0.0,
        }; DIGIT_LIMIT + 1];
        for offset in (0..length).rev() {
            let chosen = initial.filter(|_| offset == 0);
            // 单独成段总是可行的，先拿它垫底。键上没有字母（不是这种键盘的组码）时保留原键码。
            let tail = nodes[offset + 1];
            let letters = self.grid.letters(typed[offset]);
            let letter = chosen
                .and_then(|letter| letters.bytes().position(|printed| printed == letter))
                .and_then(|index| letters.get(index..=index))
                .or_else(|| self.fallback_letter(typed[offset], prior))
                .unwrap_or(&digits[offset..=offset]);
            let mut best = Segment {
                piece: letter,
                end: offset + 1,
                strays: tail.strays + 1,
                pieces: tail.pieces + 1,
                complete: tail.pieces != 0 && tail.complete,
                score: tail.score,
            };
            for end in offset + 1..=length.min(offset + self.longest_code) {
                if splits.iter().any(|&split| offset < split && split < end) {
                    break;
                }
                let Some(pieces) = self.by_code.get(&codes[offset..end]) else {
                    continue;
                };
                let must_complete = end != length || splits.contains(&end);
                let tail = nodes[end];
                for (piece, complete) in pieces {
                    if must_complete && !complete {
                        continue;
                    }
                    if groups.is_some() && !marks_agree(piece, &typed[offset..end]) {
                        continue;
                    }
                    if chosen.is_some_and(|letter| piece.as_bytes().first() != Some(&letter)) {
                        continue;
                    }
                    let candidate = Segment {
                        piece,
                        end,
                        strays: tail.strays,
                        pieces: tail.pieces + 1,
                        complete: if tail.pieces == 0 {
                            *complete
                        } else {
                            tail.complete
                        },
                        score: prior.map_or(0.0, |prior| prior.score(piece)) + tail.score,
                    };
                    if candidate.order(best, &nodes) == Ordering::Less {
                        best = candidate;
                    }
                }
            }
            nodes[offset] = best;
        }
        for (index, piece) in nodes[0].spelled(&nodes).enumerate() {
            if index != 0 {
                reading.push('\'');
            }
            reading.push_str(piece);
        }
        true
    }

    /// 一键单独成段时显示的字母：这个键上能起头音节的字母里 `prior` 频度最高的那个，频度相同取键面上靠前的（7 键是 s，8 键只有 t）。键上的字母都不起头音节（14 键的 UI 键、十八键的 U、V 键）时取键面首字母；精确码只有它指定的那一个字母。不是这种键盘的组码时为 `None`。
    pub(super) fn fallback_letter(
        &self,
        code: u8,
        prior: Option<&SyllablePrior<'_>>,
    ) -> Option<&'static str> {
        let letters = self.grid.letters(code);
        let keypad = self.grid.keypad()?;
        let first = letters.as_bytes().first()?;
        // 精确码换回它那一组的整组码，拼写表是按整组码建的。
        let group = if code.is_ascii_uppercase() {
            keypad[usize::from(first - b'a')]
        } else {
            code
        };
        let key = [group];
        let pieces = std::str::from_utf8(&key)
            .ok()
            .and_then(|key| self.by_code.get(key));
        let mut best: Option<(&'static str, f64)> = None;
        for index in 0..letters.len() {
            let Some(letter) = letters.get(index..=index) else {
                continue;
            };
            if !pieces.is_some_and(|pieces| pieces.iter().any(|(piece, _)| piece == letter)) {
                continue;
            }
            let score = prior.map_or(0.0, |prior| prior.score(letter));
            if best.is_none_or(|(_, top)| score > top) {
                best = Some((letter, score));
            }
        }
        best.map(|(letter, _)| letter).or_else(|| letters.get(..1))
    }
}

impl NineKeySession {
    /// 读音行。先按首行写（`write_front_reading`）；全拼九键上首行给不出读音时（没有首行、模糊音首行、emoji 和日期时间行、英文词）改写成已按的键的拼写，不让宿主退回去画数字。首行是英文词时是这个词覆盖已按键数的小写字母，与 14 键一致；其余是锁定的拼写接上未锁定数字的容错切分，数字全部锁定时就是锁定的拼写。14／18 键读音为空时宿主画的是组码字母，三种双拼网格不做单键兜底，都保持原样。
    pub(super) fn write_reading(&self, front: Option<&WordItem>, reading: &mut String) {
        self.write_front_reading(front, reading);
        if !reading.is_empty()
            || !matches!(self.grid, KeyGrid::NineKey)
            || self.english_only
            || !self.pinyin
        {
            return;
        }
        let locked_length = self.locked_length();
        if locked_length >= self.digits.len() {
            // 数字全部锁定：没有要切的键，读音就是锁定的拼写。
            for (index, spelling) in self.locked.iter().enumerate() {
                if index != 0 {
                    reading.push('\'');
                }
                reading.push_str(spelling);
            }
            return;
        }
        if front.is_some_and(|front| {
            front.source == CandidateSource::EnglishDictionary
                && self.write_english_letters(front.word.as_str(), reading)
        }) {
            return;
        }
        for spelling in &self.locked {
            reading.push_str(spelling);
            reading.push('\'');
        }
        if !self.write_lenient_tail(locked_length, reading) {
            reading.clear();
        }
    }

    /// 英文词的前几个字母逐位落在已按的键上时，把它们的小写写进 `reading`。行的 `pinyin` 在九键上是什么形状没有保证，所以取词本身逐位核对；词比已按的键短或有一位对不上时返回 `false`，不动 `reading`。
    fn write_english_letters(&self, word: &str, reading: &mut String) -> bool {
        let Some(keypad) = self.grid.keypad() else {
            return false;
        };
        let typed = self.digits.as_bytes();
        let Some(letters) = word.as_bytes().get(..typed.len()) else {
            return false;
        };
        if !letters
            .iter()
            .zip(typed)
            .all(|(letter, &code)| fits(keypad, letter.to_ascii_lowercase(), code))
        {
            return false;
        }
        reading.extend(
            letters
                .iter()
                .map(|letter| char::from(letter.to_ascii_lowercase())),
        );
        true
    }

    /// 从第 `start` 位起的数字按容错切分接到 `reading` 后面，用户打的切分照样守住（换算成这一段里的位置，末尾的切分不算）；这一段从第一个未锁定的数字开始时还守住选定的首字母。只对全拼网格有效，双拼网格返回 `false`、不动 `reading`，由调用方原样写键码。
    pub(super) fn write_lenient_tail(&self, start: usize, reading: &mut String) -> bool {
        if self.grid.keypad().is_none() {
            return false;
        }
        let Some(tail) = self.digits.get(start..) else {
            return false;
        };
        let mut splits = [0usize; DIGIT_LIMIT];
        let mut split_count = 0;
        for &split in &self.splits {
            if split > start && split < self.digits.len() && split_count < splits.len() {
                splits[split_count] = split - start;
                split_count += 1;
            }
        }
        let initial = self
            .initial
            .filter(|_| start == self.locked_length())
            .map(|initial| initial.letter);
        self.grid.spelling_table().lenient_split(
            tail,
            &splits[..split_count],
            self.prior.as_ref(),
            initial,
            reading,
        )
    }
}
