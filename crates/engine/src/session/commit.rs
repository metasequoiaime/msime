//! Selection and commit (core-session.md §5.7).

use super::chain::Pick;
use super::input::InputSession;
use crate::diagnostics;
use crate::error::Result;
use crate::pinyin::segment::{join_segments, split_segments};
use crate::pinyin::syllables::normalize_umlaut_aliases;
use crate::quanpin::QuanpinEngine;
use crate::text::{count_utf8_chars, first_han_char, last_han_char};
use crate::types::{
    CandidateEdge, CandidateSource, KeyResult, LocalInputMode, PersonalDictionaryKind, SchemeType,
    WordItem,
};
use crate::user_dictionary::journal::is_user_inserted;

/// 提交只会读取这些候选字段；纠错来源用于计算原始按键的消费长度。
fn clone_selected_candidate(item: &WordItem, copy_sentence_words: bool) -> WordItem {
    WordItem {
        pinyin: item.pinyin.clone(),
        canonical_pinyin: item.canonical_pinyin.clone(),
        word: item.word.clone(),
        source: item.source,
        scheme: item.scheme,
        corrected_from: item.corrected_from.clone(),
        sentence_words: if copy_sentence_words && item.source.is_generated_or_fallback() {
            item.sentence_words.clone()
        } else {
            Vec::new()
        },
        ..WordItem::default()
    }
}

impl InputSession {
    pub(super) fn select_candidate(&mut self, index: usize) -> KeyResult {
        if self.conversion.is_some() {
            return self.select_conversion_row(index);
        }
        if let Some(result) = self.select_in_open_list(index) {
            return result;
        }
        if !self.has_composition() || index >= self.candidates().len() {
            return KeyResult::unhandled();
        }
        // AI 占位行不是候选：点它或按它的数字键什么都不做，但按键算处理过了，数字不会落进文档（#6888）。
        if matches!(
            self.candidates()[index].source,
            CandidateSource::AiPending | CandidateSource::CloudPending
        ) {
            return KeyResult::handled();
        }
        // 网址补全（组字里的 `https://`、网址模式里补全了顶级域名的行）不上屏：以它为预编辑进入网址模式，接着打网址。
        if let Some(text) = self.url_continuation(index) {
            self.enter_url_mode(text);
            return KeyResult::handled();
        }
        self.commit(index)
    }

    pub(super) fn select_candidate_edge(&mut self, index: usize, edge: CandidateEdge) -> KeyResult {
        // 改字的候选是光标处一段的替换，不是可以单独上屏的词。
        if self.conversion.is_some() {
            return KeyResult::unhandled();
        }
        // A row of an editor-owned list is a choice for the conversion, not text to commit, so its edge characters are not either.
        if let Some(result) = self.select_in_open_list(index) {
            return result;
        }
        if !self.has_composition() {
            return KeyResult::unhandled();
        }
        // 「📅日期」入口的边缘选择和选中一样打开换页；日期页里的行没有可拆的字。
        if let Some(keyword) = self.date_time_menu_keyword_at(index) {
            self.enter_date_time_menu(&keyword);
            return KeyResult::handled();
        }
        if self.date_time_menu_page.is_some() {
            return KeyResult::unhandled();
        }
        let Some(candidate) = self.candidates().get(index) else {
            return KeyResult::unhandled();
        };
        let character = match edge {
            CandidateEdge::FirstHan => first_han_char(&candidate.word),
            CandidateEdge::LastHan => last_han_char(&candidate.word),
        }
        .to_owned();
        if character.is_empty() {
            return KeyResult::unhandled();
        }
        // One character of a candidate is not what the composition spelled, so nothing is learned and the rest is dropped.
        self.reset_composition();
        self.chain.reset();
        KeyResult::committed(character)
    }

    /// Choosing a row of an openable list whose rows stay in the editor (Zhuyin): the row's text replaces its span of the conversion, the list closes and nothing is committed. Unhandled when there is no such row. `None` when no such list is open; the Korean Hanja list commits its row through `commit`.
    fn select_in_open_list(&mut self, index: usize) -> Option<KeyResult> {
        if !self.candidate_list_open() || !self.zhuyin_rules_apply() {
            return None;
        }
        let selected = self.engine.select_zhuyin(index);
        self.update_mixed_candidates();
        Some(match selected {
            Ok(true) => {
                self.online_requests.invalidate();
                KeyResult::handled()
            }
            Ok(false) => KeyResult::unhandled(),
            Err(error) => KeyResult::handled().with_diagnostic(Some(error.to_string())),
        })
    }

    /// Commit segment by segment until nothing is composing. An open Korean Hanja list is closed first, so finishing (punctuation, leaving the client, a host's finish key) commits the Hangul: only an explicit choice commits a Hanja.
    pub(super) fn finish_composition(&mut self, first_index: usize) -> KeyResult {
        // 上屏/结束组字同样结束大写串：宿主结束键（`Action::Finish`、失焦、macOS 的 Cmd+A）与字面标点路由都从这里过，不经过 `handle_command`（#7616）。
        self.end_uppercase_run();
        self.close_korean_hanja();
        let mut result = KeyResult::unhandled();
        // 高亮停在 AI 占位行上时，按没有高亮处理：上屏首选，不上屏占位文字。
        let mut index = if self.candidates().get(first_index).is_some_and(|item| {
            matches!(
                item.source,
                CandidateSource::AiPending | CandidateSource::CloudPending
            )
        }) {
            0
        } else {
            first_index
        };
        // 组字里的网址补全只在明确选中时进入网址模式；结束组字（标点、失焦）时高亮停在补全上，上屏的是补全之外的第一行，没有这样的行就上屏原文。
        if self.local_mode == LocalInputMode::None
            && self
                .candidates()
                .get(index)
                .is_some_and(|item| self.is_url_completion(item))
        {
            index = self
                .candidates()
                .iter()
                .position(|item| !self.is_url_completion(item))
                .unwrap_or(usize::MAX);
        }
        // 改字时上屏的是改好的整句，不是光标处的候选。
        if self.conversion.is_some() {
            result = self.commit_conversion();
            index = 0;
        }
        // 高亮停在「📅日期」入口上时上屏打开的是换页，不是这个字：入口不上屏，组字原样留着。
        if let Some(keyword) = self.date_time_menu_keyword_at(index) {
            self.enter_date_time_menu(&keyword);
            result.handled = true;
            return result;
        }
        while self.has_composition() {
            let part = self.commit(index);
            index = 0;
            result.handled = true;
            if let Some(text) = part.commit {
                result
                    .commit
                    .get_or_insert_with(String::new)
                    .push_str(&text);
            }
            if part.diagnostic.is_some() {
                result.diagnostic = part.diagnostic;
            }
        }
        // Hosts flush a composition when focus moves or they insert text themselves, so what follows is rarely the next word of the same phrase.
        self.chain.last_pick = None;
        result
    }

    /// input_session.cpp:828-956.
    pub(super) fn commit(&mut self, index: usize) -> KeyResult {
        // 「📅日期」入口不上屏：候选窗换成这一组的全部格式，组字原样留着（`EnterDateTimeCandidatePage`）。
        if let Some(keyword) = self.date_time_menu_keyword_at(index) {
            self.enter_date_time_menu(&keyword);
            return KeyResult::handled();
        }
        self.helpcode_entry = None;
        if let Some(result) = self.commit_literal_candidate(index) {
            return result;
        }
        // A selection made while a caret prefix is decoded leaves prefix mode: the rest of the composition decodes whole again.
        self.caret = None;
        // A Zhuyin commit is the converted text, whatever the list shows: its rows are choices inside the conversion, made through `select_in_open_list`. The pending syllable is dropped, nothing is learned, and a composition that converted nothing yet commits nothing.
        if self.zhuyin_rules_apply() {
            let text = self.engine.take_zhuyin_text();
            self.reset_composition();
            self.chain.reset();
            if text.is_empty() {
                return KeyResult::handled();
            }
            return KeyResult::committed(text);
        }
        let committed = |text: Option<String>, diagnostic: Option<String>| KeyResult {
            handled: true,
            commit: text,
            diagnostic,
        };
        if self.local_mode != LocalInputMode::None {
            let text = self.candidates().get(index).map(|item| item.word.clone());
            let diagnostic = self
                .ranking_index(index)
                .and_then(|learn_index| self.learn_candidate(learn_index));
            self.chain.reset();
            self.reset_composition();
            return committed(text, diagnostic);
        }
        if self.stroke_rules_apply() {
            let text = self.candidates().get(index).map_or_else(
                || self.engine.request().raw_input.clone(),
                |item| item.word.clone(),
            );
            self.reset_composition();
            self.chain.reset();
            return KeyResult::committed(text);
        }
        if self.cantonese_rules_apply() {
            let Some(candidate) = self.candidates().get(index) else {
                let text = self.preedit();
                self.reset_composition();
                self.chain.reset();
                return KeyResult::committed(text);
            };
            let text = candidate.word.clone();
            let end = candidate.pinyin.len();
            if self.engine.select_cantonese_end(end) {
                self.update_mixed_candidates();
            } else {
                self.reset_composition();
            }
            self.chain.reset();
            return KeyResult::committed(text);
        }
        if self.korean_rules_apply() || self.vietnamese_rules_apply() || self.tibetan_rules_apply()
        {
            let text = self
                .candidates()
                .get(index)
                .map_or_else(|| self.preedit(), |item| item.word.clone());
            self.reset_composition();
            self.chain.reset();
            return KeyResult::committed(text);
        }
        let copy_sentence_words = self.personal_context_applies();
        let selected = self
            .candidates()
            .get(index)
            .map(|item| clone_selected_candidate(item, copy_sentence_words));
        let text = match &selected {
            Some(item) => {
                let mut text = item.word.clone();
                self.apply_english_word_case(item.source, &mut text);
                Some(text)
            }
            // The bare prefix letter of a temporary mode is a marker, not text.
            None if matches!(
                self.local_mode,
                LocalInputMode::TemporaryEnglish | LocalInputMode::TemporaryJapanese
            ) && self.local_preedit.len() == 1 =>
            {
                None
            }
            // 五笔临时拼音反查没有候选时上屏键入的拼音，引导键 `z` 同样只是标记。
            None if self.engine.wubi_pinyin_lookup_active() => {
                Some(self.preedit().split_off(1)).filter(|text| !text.is_empty())
            }
            None => Some(self.preedit()),
        };
        let diagnostic = self
            .ranking_index(index)
            .and_then(|learn_index| self.learn_candidate(learn_index));
        self.commit_selection(selected, text, diagnostic)
    }

    /// 拼音组字里上屏选中的行（`None` 是上屏 `text` 本身）之后的事：记个人上下文、推进组字或者结束组字、造词。`diagnostic` 是已经做过的学习留下的诊断。整句改字上屏改好的整句时也走这里。
    pub(super) fn commit_selection(
        &mut self,
        selected: Option<WordItem>,
        text: Option<String>,
        mut diagnostic: Option<String>,
    ) -> KeyResult {
        let committed = |text: Option<String>, diagnostic: Option<String>| KeyResult {
            handled: true,
            commit: text,
            diagnostic,
        };
        let Some(selected) = selected else {
            self.record_context_into(&mut diagnostic, None, false, false);
            self.chain.last_pick = None;
            self.reset_composition();
            return committed(text, diagnostic);
        };
        let has_dictionary_reading = selected.source.is_dictionary();
        // Whole sentences from the lattice carry a canonical reading and join the phrase being composed like dictionary rows do.
        let has_generated_reading =
            selected.source.is_generated_or_fallback() && !selected.canonical_pinyin.is_empty();
        let follows_pinyin =
            self.candidates_follow_pinyin() || selected.scheme == SchemeType::Quanpin;
        if !((has_dictionary_reading || has_generated_reading)
            && self.local_mode == LocalInputMode::None
            && !self.dedicated_english
            && follows_pinyin)
        {
            self.record_context_into(&mut diagnostic, Some(&selected), false, false);
            self.chain.last_pick = None;
            self.reset_composition();
            return committed(text, diagnostic);
        }

        // A user row of three or more characters answering the whole composition is almost always a sentence stored by sentence learning; counting it as one word would teach the model a sentence as a word. User rows come back as Database like shipped ones, so the journal's insert record tells them apart; checked last because it reads a file.
        let learned_sentence_word = has_dictionary_reading
            && self.personal_context_applies()
            && !self.chain.same_composition
            && !selected.canonical_pinyin.is_empty()
            && count_utf8_chars(&selected.word) >= 3
            && self.selection_completes_composition(&selected)
            && {
                let mut segments = split_segments(&selected.canonical_pinyin);
                normalize_umlaut_aliases(&mut segments);
                is_user_inserted(
                    self.journal_path(),
                    PersonalDictionaryKind::Pinyin,
                    &join_segments(&segments),
                    &selected.word,
                )
            };
        let opens_composition = self.phrase_progress.word.is_empty();
        let transition = self.advance_composition_after_selection(&selected);
        self.record_context_into(
            &mut diagnostic,
            Some(&selected),
            learned_sentence_word,
            transition.continues_composition,
        );
        let pick = (has_dictionary_reading
            && !learned_sentence_word
            && !self.is_wubi()
            && !self.is_japanese()
            && self.learning_enabled
            && self.personal_context_enabled)
            .then(|| Pick {
                canonical_pinyin: transition.selected_canonical_pinyin.clone(),
                word: selected.word.clone(),
                committed_at: self.steady_now(),
            });
        // Recording the context has already ended the chain for a commit that does not follow the previous one.
        if opens_composition {
            if let (Some(current), Some(previous)) = (&pick, self.chain.last_pick.clone()) {
                if let Some(pair_diagnostic) = self.learn_pick_pair(&previous, current) {
                    diagnostic.get_or_insert(pair_diagnostic);
                }
            }
        }
        self.chain.last_pick = pick;
        // The rest of the input was re-queried before the chain moved on; order it against the word just committed.
        if transition.continues_composition && self.chain.previous.is_some() {
            self.update_mixed_candidates();
        }
        let progress = Self::update_creating_word_progress(
            &self.phrase_progress.pinyin,
            &self.phrase_progress.word,
            &selected.word,
            &transition,
        );
        if transition.continues_composition {
            self.phrase_progress = progress;
            self.discard_abandoned_phrase_progress();
            return committed(text, diagnostic);
        }
        // 反查是查编码用的，不造词：它的词条不该因为一次查询进到用户词库。
        if !self.phrase_progress.word.is_empty()
            && progress.can_store
            && self.learning_enabled
            && !self.engine.wubi_pinyin_lookup_active()
            && self
                .store_user_phrase_from_canonical_pinyin(&progress.pinyin, &progress.word)
                .is_err()
        {
            diagnostic = Some(diagnostics::PHRASE_NOT_PERSISTED.to_owned());
        }
        self.reset_composition();
        committed(text, diagnostic)
    }

    /// Chain bookkeeping for a nine-key commit: the grid bypasses this session, so the word after it must not read the word before it as its context. The committed text is not needed, since it only fed the dropped learning-undo ledger.
    pub(super) fn note_external_commit(&mut self) {
        self.reset_commit_context();
    }

    /// Store a phrase under a complete quanpin key. Quanpin and shuangpin share the canonical dictionary, so the key never goes back through a shuangpin profile (input_session_composition.cpp:522-536).
    pub(super) fn store_user_phrase_from_canonical_pinyin(
        &mut self,
        pinyin: &str,
        word: &str,
    ) -> Result<()> {
        self.canonical_phrase_engine()
            .create_word_from_canonical_pinyin(pinyin, word)?;
        if crate::user_dictionary::wubi_learning::learn(&self.paths, word)? {
            self.engine.reset_cache();
        }
        Ok(())
    }

    pub(super) fn canonical_phrase_engine(&mut self) -> &mut QuanpinEngine {
        let paths = &self.paths;
        self.canonical_phrase_engine
            .get_or_insert_with(|| QuanpinEngine::new(paths))
    }

    /// `record_personal_context`, with its diagnostic kept only when nothing earlier failed.
    fn record_context_into(
        &mut self,
        diagnostic: &mut Option<String>,
        selected: Option<&WordItem>,
        learned_sentence_word: bool,
        composition_left: bool,
    ) {
        if let Some(context_diagnostic) =
            self.record_personal_context(selected, learned_sentence_word, composition_left)
        {
            diagnostic.get_or_insert(context_diagnostic);
        }
    }
}
