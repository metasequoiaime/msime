//! 光标后编辑（`SessionOptions::caret_edit_suffix`，默认关）。
//!
//! 全拼或双拼组字时，光标停在组字中间：光标前那段原文固定为它当前的最佳转换（只显示，不上屏），候选换成光标后那段原文的候选，与把它当成整段组字时一样从同一条候选管线出来；首选前面接上固定的部分，所以首选就是整句。选中第 N 条候选时，先把固定的那段按一次部分选择交出去，再照常选中光标后那段的这一行：它只消耗自己的音节，剩下的接着在本模式里转换，首选仍是「已交出去的文字 + 剩余部分的首选」。宿主打开 `phrase_preedit` 时（三个桌面宿主都打开）这些交出去的文字由 runtime 暂存在组字区里，整段组字完成时一起上屏。
//!
//! 光标在开头或末尾、方案不是全拼或双拼、在本地模式或专用英文里时，行为与关着时完全相同（光标前缀解码，见 `editing.rs` 的 `refresh_prefix_candidates`）。光标前那段找不到一行覆盖它全部音节的转换时同样退回光标前缀解码。
//!
//! 本模式里不混入英文、emoji、颜文字、符号、日文、连续多语言、快捷短语、行内日期时间、联系人和网址补全这些行，也不发云候选和 AI 查询：选中它们会结束整段组字，扔掉固定的那段或光标后剩下的拼写。学习沿用部分选择的规则：光标后那段的选择照常调频、记个人上下文，固定的那段只按默认转换记个人上下文和造词进度、不调词频，整段完成时已选的各段按造词规则存成个人词条。

use super::input::InputSession;
use crate::types::{CandidateSource, KeyResult, LocalInputMode, SchemeType, WordItem};

/// 光标后编辑的状态。
#[derive(Debug, Clone, Default, PartialEq)]
pub(super) struct CaretEdit {
    /// 这段组字里，本模式已经交出去（宿主暂存着、还没真正上屏）的文字。非空时即使光标回到末尾，模式也继续生效，首选前面接上它。
    pub held: String,
    /// 最近一次刷新后模式是否生效。
    pub active: bool,
    /// 光标前那段原文的最佳转换，还没交出去；光标不在组字中间时为 `None`。
    pub prefix_row: Option<WordItem>,
    /// `prefix_row` 是用哪段原文查出来的（带大小写）；同一段原文不重复查询。
    pub prefix_input: String,
}

/// 两行是不是同一个候选：选中前后列表会重新生成，只能按内容找回来。
fn same_row(left: &WordItem, right: &WordItem) -> bool {
    left.word == right.word
        && left.source == right.source
        && left.pinyin == right.pinyin
        && left.canonical_pinyin == right.canonical_pinyin
}

impl InputSession {
    /// 选项开着，且是全拼或双拼的普通组字（不是本地模式、专用英文、字面原文）。不看光标。
    fn caret_edit_allowed(&self) -> bool {
        self.caret_edit_suffix
            && !self.dedicated_english
            && self.local_mode == LocalInputMode::None
            && self.temporary_original_scheme.is_none()
            && matches!(
                self.engine.current_scheme_type(),
                SchemeType::Quanpin | SchemeType::Shuangpin
            )
            && !self.has_literal_input()
            && self.has_composition()
    }

    /// 刷新候选前调用：模式生效时把光标后那段的候选放进 `prefix_candidates`（光标在中间时）或让列表照常取引擎的整段候选（只剩已交出去的文字时），返回真；不生效时返回假，由调用方照旧做光标前缀解码。
    pub(super) fn refresh_caret_edit(&mut self) -> bool {
        self.caret_edit.active = false;
        if !self.has_composition() {
            self.caret_edit.held.clear();
        }
        if !self.caret_edit_allowed() {
            self.caret_edit.prefix_row = None;
            return false;
        }
        let raw_with_cases = self.raw_with_cases().to_owned();
        let end = if self.caret.is_some() {
            self.prefix_end()
        } else {
            raw_with_cases.len()
        };
        let inside = end > 0 && end < raw_with_cases.len();
        let prefix_row = if inside {
            self.caret_prefix_conversion(&raw_with_cases[..end])
        } else {
            None
        };
        if prefix_row.is_none() && self.caret_edit.held.is_empty() {
            // 光标不在中间，或光标前那段转不完整：与关着时一样。
            self.caret_edit.prefix_row = None;
            return false;
        }
        match prefix_row {
            Some(row) => {
                self.caret_edit.prefix_row = Some(row);
                self.query_caret_suffix(&raw_with_cases[end..]);
                self.prefix_active = true;
            }
            None => {
                // 已经交出去一部分、光标不在中间：整段组字都是「光标后」，候选就是引擎的整段候选。
                self.caret_edit.prefix_row = None;
                self.prefix_candidates.clear();
                self.prefix_query_input.clear();
            }
        }
        self.caret_edit.active = true;
        true
    }

    /// 光标前那段原文的最佳转换：临时查询里第一行覆盖了这段全部音节的拼音行。找不到时为 `None`。
    fn caret_prefix_conversion(&mut self, prefix_with_cases: &str) -> Option<WordItem> {
        if self.caret_edit.prefix_input == prefix_with_cases {
            return self.caret_edit.prefix_row.clone();
        }
        let prefix = prefix_with_cases.to_ascii_lowercase();
        let (rows, request) = self
            .engine
            .query_raw_candidates_with_request(&prefix, prefix_with_cases);
        let row = rows.into_iter().find(|row| {
            !row.pinyin.is_empty()
                && (row.source.is_dictionary()
                    || (row.source == CandidateSource::Generated
                        && !row.canonical_pinyin.is_empty()))
                && self.selection_completes_request(&request, row)
        });
        self.caret_edit.prefix_input = prefix_with_cases.to_owned();
        self.caret_edit.prefix_row = row.clone();
        row
    }

    /// 光标后那段原文的候选，与光标前缀解码共用 `prefix_candidates` 和它的缓存键。
    fn query_caret_suffix(&mut self, suffix_with_cases: &str) {
        let lowercase = suffix_with_cases
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte == b'\'');
        let suffix = if lowercase {
            suffix_with_cases.to_owned()
        } else {
            suffix_with_cases.to_ascii_lowercase()
        };
        if self.prefix_query_input != suffix {
            self.prefix_candidates = self.engine.query_raw_candidates(&suffix, suffix_with_cases);
            self.prefix_query_input = suffix;
        }
    }

    /// 首选前面接上的固定文字：已经交出去的部分加上光标前那段的最佳转换。模式不生效时为 `None`。
    pub(super) fn caret_edit_prefix(&self) -> Option<String> {
        if !self.caret_edit.active {
            return None;
        }
        let mut text = self.caret_edit.held.clone();
        if let Some(row) = &self.caret_edit.prefix_row {
            text.push_str(&row.word);
        }
        Some(text)
    }

    /// 光标后那段在原文里从哪里开始；光标不在中间时是 0。
    pub(super) fn caret_edit_suffix_start(&self) -> usize {
        if self.caret_edit.prefix_row.is_some() {
            self.prefix_end()
        } else {
            0
        }
    }

    /// 本模式里选中第 `index` 条候选：先交出光标前固定的那段，再按部分选择的规则选中光标后那段的这一行。`None` 是这一条不归本模式处理（下标越界），调用方照常处理。
    pub(super) fn commit_caret_edit(&mut self, index: usize) -> Option<KeyResult> {
        let row = self.candidates().get(index)?.clone();
        if matches!(
            row.source,
            CandidateSource::AiPending | CandidateSource::CloudPending
        ) {
            return Some(KeyResult::handled());
        }
        self.helpcode_entry = None;
        let mut text = String::new();
        let mut diagnostic = None;
        if let Some(prefix) = self.caret_edit.prefix_row.clone() {
            diagnostic = self.consume_caret_prefix(&prefix);
            text.push_str(&prefix.word);
        }
        self.caret = None;
        // 学习要在固定的那段交出去之后做：这时引擎的组字正是光标后那段，调频按它的上下文记，与把它当作整段组字时一样。
        let learn_index = self
            .ranking_list()
            .iter()
            .position(|item| same_row(item, &row));
        if let Some(learned) = learn_index.and_then(|index| self.learn_candidate(index)) {
            diagnostic.get_or_insert(learned);
        }
        let selected =
            super::commit::clone_selected_candidate(&row, self.personal_context_applies());
        let word = row.word.to_string();
        text.push_str(&word);
        let mut result = self.commit_selection(Some(selected), Some(word.clone()), diagnostic);
        if result.commit.is_some() {
            result.commit = Some(text);
        }
        if self.has_composition() {
            self.caret_edit.held.push_str(&word);
        }
        Some(result)
    }

    /// 以词定字：固定的那段照样交出去，接上光标后那一行的首字或尾字，其余拼写丢掉，不学习。
    pub(super) fn caret_edit_edge_text(&self, character: &str) -> String {
        let mut text = String::new();
        if let Some(prefix) = &self.caret_edit.prefix_row {
            text.push_str(&prefix.word);
        }
        text.push_str(character);
        text
    }

    /// 把光标前固定的那段按一次部分选择交出去：记个人上下文和造词进度，但不调词频，也不算一次显式选择（不参与选词对学习）。组字随之只剩光标后那段。
    fn consume_caret_prefix(&mut self, prefix: &WordItem) -> Option<String> {
        let raw_with_cases = self.raw_with_cases().to_owned();
        let end = self.prefix_end().min(raw_with_cases.len());
        let rest_with_cases = raw_with_cases[end..].trim_start_matches('\'').to_owned();
        let rest = rest_with_cases.to_ascii_lowercase();
        let mut diagnostic = None;
        self.record_context_into(&mut diagnostic, Some(prefix), false, true);
        self.chain.last_pick = None;
        self.engine
            .replace_active_raw_input(&rest, &rest_with_cases);
        self.caret = None;
        self.online_requests.invalidate();
        let transition = super::composition::SelectionTransition {
            continues_composition: true,
            selected_canonical_pinyin: prefix.canonical_pinyin.to_string(),
            current_segmentation_with_cases: self.pinyin_segmentation_with_cases().into_owned(),
            ..Default::default()
        };
        self.phrase_progress = Self::update_creating_word_progress(
            &self.phrase_progress.pinyin,
            &self.phrase_progress.word,
            &prefix.word,
            &transition,
        );
        self.caret_edit.prefix_row = None;
        self.caret_edit.held.push_str(&prefix.word);
        self.update_mixed_candidates();
        diagnostic
    }
}
