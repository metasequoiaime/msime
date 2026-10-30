//! 回车原样上屏的两条命令在会话层的那一半：`Command::CommitRawWithoutLearning`，以及 `SessionOptions::learn_typed_spellings_on_enter` 打开时 `Command::CommitRaw` 之后学英文词的策略。专用英文里打的词由组字自己的 `commit_raw` 学（`learn_english_on_enter`），这里只管它之外的那些。两者以前在 host 门面里（bridge.cpp:1320-1359），搬进核心之后门面只传产品取值。

use super::input::ENTERED_ENGLISH_WORD_WEIGHT;
use super::Session;
use crate::assets;
use crate::diagnostics;
use crate::pinyin::segment::is_complete_pinyin_input;
use crate::types::{Command, KeyResult, LocalInputMode};
use crate::user_dictionary::removal::learn_entered_english_word;

impl Session {
    /// 原样上屏、不学词。专用英文里组字自己的原样上屏会学这个词，所以那里取预编辑原文、结束组字，自己交出上屏（bridge.cpp:1320-1331）；上屏的就是预编辑原文，本地模式的入口字母也在内。其余情况就是不带学词策略的 `CommitRaw`。
    pub(super) fn commit_raw_without_learning(&mut self) -> KeyResult {
        let before = self.snapshot();
        if !before.dedicated_english {
            return self.dispatch_command(Command::CommitRaw);
        }
        let raw = before.preedit;
        self.dispatch_command(Command::Cancel);
        KeyResult::committed(raw)
    }

    /// `learn_typed_spellings_on_enter` 打开时的 `CommitRaw`：专用英文、本地模式里原样上屏的字母，以及中文方案里拼不成完整音节的拼音，上屏之后学成英文词（bridge.cpp:1332-1359）。九宫格（九键、14 键）组字时上屏的不是用户逐个打出的字母：九键是数字，14 键是结束组字后的首选，都不学。专用英文里的词组字自己已经学过，这里再学一次是空操作（已有的词不再插入）。
    pub(super) fn commit_raw_learning_typed_spellings(&mut self) -> KeyResult {
        let grid_composing = self.grid_composing();
        let before = self.snapshot();
        let chinese_scheme = before.scheme.learns_english_words();
        let complete_pure_pinyin = chinese_scheme && {
            let segmentation = if before.normalized_segmentation.is_empty() {
                &before.raw_segmentation
            } else {
                &before.normalized_segmentation
            };
            !segmentation.is_empty() && is_complete_pinyin_input(segmentation)
        };
        // 计算、指令和名单模式里是算式、触发词或键，网址也不是英文单词，都不是用户拼出的词，不进英文词库。
        // 整句改字时回车上屏的是改好的汉字，不是拼出来的字母。
        let should_learn = self.input.learn_english_on_enter
            && before.conversion.is_empty()
            && !grid_composing
            && !before.local_mode.generates_text()
            && before.local_mode != LocalInputMode::Url
            && (before.dedicated_english
                || before.local_mode != LocalInputMode::None
                || (chinese_scheme && !complete_pure_pinyin));
        let mut result = self.dispatch_command(Command::CommitRaw);
        if !should_learn || !self.input.can_learn_english_words() {
            return result;
        }
        if let Some(commit) = result.commit.as_deref() {
            // 进词库的词去掉临时模式的入口字母：回车上屏的原文带它，词库里不该有（#7616）。只有入口字母时正文为空，那就没有词可学。
            let text = before.local_mode.without_entry_marker(commit);
            if !text.is_empty() {
                let word = if before.local_mode == LocalInputMode::TemporaryJapanese {
                    temporary_japanese_word(text)
                } else {
                    text.to_owned()
                };
                let paths = &self.input.paths;
                // 上屏已经发生；学不进去的词在结果旁边报告，不撤回上屏。
                if learn_entered_english_word(
                    &paths.dictionary(assets::ENGLISH_DICTIONARY),
                    &paths.user(assets::USER_JOURNAL),
                    &word,
                    ENTERED_ENGLISH_WORD_WEIGHT,
                )
                .is_err()
                {
                    result.diagnostic = Some(diagnostics::ENGLISH_WORD_NOT_LEARNED.to_owned());
                }
            }
        }
        result
    }
}

/// 临时日文回车上屏的词按 `R` 加上屏文字去学，与入口字母的写法一致。
fn temporary_japanese_word(commit: &str) -> String {
    let mut word = String::with_capacity(1 + commit.len());
    word.push('R');
    word.push_str(commit);
    word
}

#[cfg(test)]
mod tests {
    use super::temporary_japanese_word;

    #[test]
    fn temporary_japanese_word_allocates_only_result_bytes() {
        let word = temporary_japanese_word("かな");
        assert_eq!(word, "Rかな");
        assert_eq!(word.capacity(), word.len());
    }
}
