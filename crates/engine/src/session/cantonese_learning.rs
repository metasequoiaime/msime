//! 粤语上屏时的学习（#10544），开关与拼音相同：`learning_enabled`（宿主的「学习」，隐私会话里宿主把它关掉）关着时什么也不写；开着时选中词库里的非首选行按调频方式调频（`frequency.mode` 为「关闭」时不调），一次组字分几次选完、或选了组出来的整句时把整段记成一个词。写法见 `cantonese::learning`。
//!
//! 粤语选一个候选就立刻上屏、剩下的接着组字（`CantoneseScheme::select_end`），没有拼音那样留到组字结束才上屏的 `phrase_progress`，所以这里另记已选的几段，组字结束时才决定造不造词。选首选一次选完整段（最常见的上屏）不碰日志也不分配（`selecting_a_cantonese_candidate_does_not_clone_the_full_row`）：首选不调频，词库行和组句行靠候选自己的标记（`CantoneseCandidate::composed`）分开。

use super::input::InputSession;
use crate::cantonese::learning::{adjust_frequency, create_word, RankedRow};
use crate::diagnostics;
use crate::types::{CandidateSource, FrequencyAdjustmentMode, SchemeType, WordItem};

/// 一次粤语组字里已经上屏、后面还有字母在组字的几段：粤拼键以空格连接，文字直接相接。
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub(super) struct CantonesePhraseProgress {
    key: String,
    word: String,
    /// 某一段不是粤语方案自己的行，这次组字不造词。
    broken: bool,
}

impl CantonesePhraseProgress {
    fn push(&mut self, key: &str, word: &str) {
        if !self.key.is_empty() {
            self.key.push(' ');
        }
        self.key.push_str(key);
        self.word.push_str(word);
    }
}

/// 选中的一段粤语候选，在把它从组字里拿走之前记下。
pub(super) struct CantonesePick<'a> {
    pub key: &'a str,
    pub word: &'a str,
    /// 粤语方案自己出的行（词库行或组句行），带粤拼键。
    pub cantonese_row: bool,
    /// 组出来的整句，不是词库里的一行。
    pub composed: bool,
}

/// 粤语方案自己出的候选行：来自粤语词库或组出来的整句，带粤拼键。
pub(super) fn is_cantonese_row(row: &WordItem) -> bool {
    row.source == CandidateSource::Database
        && row.scheme == SchemeType::Cantonese
        && !row.canonical_pinyin.is_empty()
}

impl InputSession {
    /// 选中粤语候选 `index`、还没把它从组字里拿走之前调用：词库里的非首选行按调频方式调频，比的是与它覆盖同一段输入的那组行（整段的词、只覆盖前几个音节的词各成一组，组内按权重比名次）。组句行没有可调的权重，不调。首选是默认的选择，选它不调频，同拼音（`learn_ranked_candidate`）。返回写日志失败时的诊断。
    pub(super) fn learn_cantonese_frequency(
        &mut self,
        index: usize,
        pick: &CantonesePick<'_>,
    ) -> Option<String> {
        if !self.learning_enabled
            || index == 0
            || !pick.cantonese_row
            || pick.composed
            || self.frequency.mode == FrequencyAdjustmentMode::Disabled
        {
            return None;
        }
        let rows: Vec<RankedRow> = {
            let candidates = self.candidates();
            let span = &candidates.get(index)?.pinyin;
            candidates
                .iter()
                .filter(|row| is_cantonese_row(row) && row.pinyin == *span)
                .map(|row| RankedRow {
                    text: self
                        .engine
                        .cantonese_original_word(&row.canonical_pinyin, &row.word)
                        .unwrap_or(&row.word)
                        .to_owned(),
                    weight: row.weight,
                })
                .collect()
        };
        adjust_frequency(
            self.journal_path(),
            &rows,
            pick.key,
            pick.word,
            self.frequency,
        )
        .err()
        .map(|_| diagnostics::FREQUENCY_NOT_PERSISTED.to_owned())
    }

    /// 选中的一段已经从组字里拿走之后调用。`composing` 为真时后面还有字母，记下这一段；为假时组字到此结束（调用方随后 `reset_composition`），把这次分几段选出来的整段、或一次选中的组句行在粤语词库里还没有时记成一个词。返回写日志失败时的诊断。
    pub(super) fn note_cantonese_piece(
        &mut self,
        pick: &CantonesePick<'_>,
        composing: bool,
    ) -> Option<String> {
        if !self.learning_enabled {
            return None;
        }
        if composing {
            if pick.cantonese_row {
                self.cantonese_progress.push(pick.key, pick.word);
            } else {
                self.cantonese_progress.broken = true;
            }
            return None;
        }
        let mut progress = std::mem::take(&mut self.cantonese_progress);
        if !pick.cantonese_row || progress.broken {
            return None;
        }
        // 一次选完：只有组句行才是新词。
        if progress.key.is_empty() && !pick.composed {
            return None;
        }
        progress.push(pick.key, pick.word);
        if self
            .engine
            .cantonese_has_entry(&progress.key, &progress.word)
        {
            return None;
        }
        create_word(self.journal_path(), &progress.key, &progress.word)
            .err()
            .map(|_| diagnostics::PHRASE_NOT_PERSISTED.to_owned())
    }
}
