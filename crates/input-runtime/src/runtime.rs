//! `Runtime`: the orchestration between keystrokes, the engine, and everything the
//! providers return asynchronously.

use super::*;
pub(crate) use msime_engine::ordering::apply_order;
use msime_engine::ordering::{
    ensure_engine_order, rerank_pick, rotate_to_front, runner_up_order, ParallelOrderRows,
};
// 排序决策搬进了 `msime_engine::ordering`；`tests.rs` 仍按原来的 crate 内名字引用这几项，这里为它们重新导出。
#[cfg(test)]
pub(crate) use msime_engine::ordering::{
    reorders_candidates as runtime_reorders_candidates, LATTICE_SOURCE,
};
use msime_engine::SchemeType;

pub enum Action {
    ResetCache,
    Character {
        value: u8,
        shift: bool,
    },
    Punctuation(u8),
    /// Finish the highlighted composition and append the literal ASCII mark.
    /// Linux uses this when IBus surrounding text says smart punctuation
    /// should stay ASCII; the Engine's normal punctuation table remains
    /// authoritative for every other punctuation action.
    PunctuationAscii(u8),
    Command(Command),
    SegmentBackspace,
    SegmentMoveLeft,
    SegmentMoveRight,
    Select(CandidateId),
    /// Select any candidate in the current Engine generation. This is reserved
    /// for hosts that explicitly requested [`Runtime::all_candidates`].
    SelectAnyCandidate(CandidateId),
    SelectEdge(CandidateId, CandidateEdge),
    PinCandidate(CandidateId),
    RemoveCandidate(CandidateId),
    FixCandidatePosition(CandidateId, u8),
    ClearCandidatePosition(CandidateId),
    ChooseNineKeySpelling(NineKeySpellingId),
    /// 滑过字母键的一笔，用宿主自己的坐标。
    Glide {
        keyboard: Box<GlideKeyboard>,
        points: Vec<GlidePoint>,
    },
    SelectHighlighted,
    Finish,
    NextPage,
    PreviousPage,
    NextCandidate,
    PreviousCandidate,
    FirstCandidate,
    LastCandidate,
}

/// What one selection of a phrase-in-progress can be taken back to.
///
/// `word_before` is the whole held phrase as it stood before the selection, not just the piece it
/// added: the reference restores its accumulated word wholesale for the same reason - a selection
/// that recorded nothing sits between two that did, and only the whole word puts them all back.
pub(crate) struct PhraseSelection {
    pub(crate) word_before: String,
    pub(crate) reading: String,
}

pub struct Runtime<E: InputEngine = Session> {
    pub(crate) engine: E,
    pub(crate) session: u64,
    pub(crate) generation: u64,
    pub(crate) focused: bool,
    pub(crate) page_size: usize,
    pub(crate) highlighted: usize,
    pub(crate) translations: HashMap<String, String>,
    pub(crate) cached: EngineSnapshot,
    /// The Engine's own index for each seat of `cached`.
    ///
    /// `rerank`, `demote_runner_up_readings` and `normalize_online_slots` reorder the cached list, but the Engine knows nothing of that and selects by its own order. Every call that names a candidate to the Engine goes through [`Self::engine_index`]; without it, an AI candidate seated in slot 1 committed whatever the Engine held at 1.
    pub(crate) engine_order: Vec<usize>,
    pub(crate) snapshot_valid: bool,
    pub(crate) character_width: CharacterWidth,
    pub(crate) touch_keyboard_layout: TouchKeyboardLayout,
    /// Whether the host draws a half-composed phrase itself instead of having it committed.
    ///
    /// Picking a candidate that consumes only part of the input leaves the Engine composing the
    /// rest, and it hands back the piece that was chosen. The reference keeps that piece inside its
    /// composition - `word_for_creating_word` is prepended to the reading and the caret is shifted
    /// past it - and commits the phrase as one piece when the composition ends. This runtime sent
    /// it to the document immediately, so half a phrase landed in the application while the user
    /// was still typing the rest of it.
    ///
    /// Off by default because a host that does not draw [`View::phrase_prefix`] would show nothing
    /// at all for that piece. Each host turns it on as it learns to draw it.
    pub(crate) phrase_preedit: bool,
    /// The piece already chosen for the phrase being composed, held back from the document.
    ///
    /// Non-empty while the Engine is still composing, and in one case after its reading is gone: a segment Backspace (Ctrl+Backspace) that empties a reading whose phrase still has a selection to take back leaves the phrase in the composition, as the reference's `keep_creating_word_after_empty_raw` does. A host must therefore count a non-empty [`View::phrase_prefix`] as a composition, alongside the reading and the candidates.
    pub(crate) phrase_prefix: String,
    /// One entry per selection that grew the held phrase, newest last.
    ///
    /// This is what lets the user go back: Backspace on the last of the reading puts the selection
    /// that consumed it back the way it was, instead of deleting a letter and ending the
    /// composition. The reference keeps the same stack in its Server
    /// (`CompositionState::selection_history`) and its two rules read exactly these fields.
    ///
    /// A selection that consumed no reading is not recorded, because there is nothing for it to
    /// restore - the same reason the reference refuses an empty `consumed_raw_input_with_cases`.
    pub(crate) phrase_selections: Vec<PhraseSelection>,
    /// Recently committed text, sent to the AI provider as context and pushed to the Engine as its rescoring context.
    ///
    /// The reference sends what the user has just written so a suggestion fits the sentence in progress. Every host but Linux left this empty, which made AI suggestions guess from the pinyin alone. The Engine's neural sentence association conditions on the same text, so every change here is mirrored with [`InputEngine::set_rescoring_context`]; without it the engine-side models ranked every sentence as if nothing had been written before it.
    pub(crate) ai_context: String,
    /// Reorders candidates the pinyin decoder assembled, when a host supplied a model.
    ///
    /// Absent unless a host calls [`Runtime::set_reranker`], and absent is the only state the
    /// hosts that ship no model ever see.
    pub(crate) reranker: Option<Reranker>,
    /// A second, larger model run once the user stops typing, when one is attached.
    ///
    /// Capacity is the most effective lever the model has — the 24M preset beats the 6.8M one by
    /// 49 points of top-1 on the harvested failure set — and it is also the one the keystroke path
    /// cannot afford: the same model measures p95 153ms against a 16ms frame, with the slowest
    /// keystroke at 342ms. Both numbers are real and they do not have to be reconciled, because
    /// they are answers to different questions. While the user is typing, the first row has to be
    /// plausible now; when the user stops to read the candidates, it has to be right. The fast
    /// model owns the first job and this one owns the second.
    pub(crate) settled_reranker: Option<Reranker>,
    /// Whether [`Runtime::rerank_settled`] runs the settled model: the product's desktop sentence model switch. On unless a host turns it off, so a caller that attaches a model and says nothing else gets it.
    pub(crate) settled_rerank_enabled: bool,
}

/// `SchemeType::Korean`: Hangul syllables that compose in the preedit, with no Chinese punctuation; the only candidates are the composing syllable's Hanja, in the Engine's table order, once the host asks for them.
pub const KOREAN_SCHEME: u8 = SchemeType::Korean as u8;

/// The traits of the scheme behind `scheme`, which the Engine reports as its `SchemeType` ordinal. Only the placeholder snapshot of a failed refresh carries an ordinal no scheme has; each caller decides what that placeholder means, the way the ordinal comparisons this replaces did.
fn scheme_type(scheme: u8) -> Option<SchemeType> {
    SchemeType::from_u8(scheme)
}

/// [`View::script_conversion`] for a scheme ordinal and local mode name.
pub(crate) fn script_conversion(scheme: u8, local_mode: &str) -> bool {
    scheme_type(scheme).is_some_and(SchemeType::script_conversion_applies)
        && !matches!(local_mode, "unicode" | "temporary_japanese")
}

/// Move the flagged elements to the end, keeping both groups in their existing order.
#[cfg(test)]
pub(crate) fn move_to_back<T>(items: &mut [T], moved: &[bool]) {
    // Stable-partition in place. A rotation moves the next unflagged item ahead of the flagged
    // run without allocating a second vector; candidate arrays are kept in lockstep by calling
    // this once for each array below, and their usual size makes the bounded O(n²) movement cheap.
    let mut head_len = 0;
    for index in 0..items.len() {
        if moved.get(index).copied().unwrap_or(false) {
            continue;
        }
        if head_len != index {
            items[head_len..=index].rotate_right(1);
        }
        head_len += 1;
    }
}

/// 排序决策读取的候选行，借用快照里的并行数组。调用方先确认各数组等长。
fn order_rows(snapshot: &EngineSnapshot) -> ParallelOrderRows<'_> {
    ParallelOrderRows::new(
        &snapshot.candidates,
        &snapshot.candidate_sources,
        &snapshot.candidate_answers_key,
        &snapshot.candidate_corrected,
    )
}

impl Runtime<Session> {
    /// A live host mode changes neither composition nor candidate identity.
    pub fn set_chinese_punctuation_enabled(&mut self, enabled: bool) -> Result<(), RuntimeError> {
        self.engine
            .set_chinese_punctuation_enabled(enabled)
            .map_err(|error| RuntimeError::Engine(error.to_string()))?;
        self.refresh_idle_spelling_symbols()
    }

    pub fn online_query(&self) -> Result<Option<OnlineQuery>, RuntimeError> {
        let query = self
            .engine
            .online_query()
            .map_err(|error| RuntimeError::Engine(error.to_string()))?;
        if !query.available {
            return Ok(None);
        }
        Ok(Some(OnlineQuery {
            scheme: query.scheme,
            generation: query.generation,
            identity: query.identity,
            query_text: query.query_text,
            cache_key: query.cache_key,
            pinyin_segments: query.pinyin_segments,
            cloud_eligible: query.cloud_eligible,
            ai_eligible: query.ai_eligible,
            cloud_candidates: true,
            session_id: query.session_id,
            ai_context: if query.ai_eligible {
                self.ai_context.clone()
            } else {
                String::new()
            },
            ai_assistant: None,
            ai_cache_only: false,
        }))
    }

    pub fn apply_online_candidate(
        &mut self,
        query: &OnlineQuery,
        candidate: &str,
        source: u8,
    ) -> Result<bool, RuntimeError> {
        let cloud_candidates = query.cloud_candidates;
        let query = OnlineQuerySnapshot {
            available: true,
            scheme: query.scheme,
            generation: query.generation,
            identity: query.identity.clone(),
            query_text: query.query_text.clone(),
            cache_key: query.cache_key.clone(),
            pinyin_segments: query.pinyin_segments.clone(),
            cloud_eligible: query.cloud_eligible,
            ai_eligible: query.ai_eligible,
            session_id: query.session_id,
        };
        self.apply_online_candidate_snapshot(query, candidate, source, cloud_candidates)
    }

    /// Apply a provider result while transferring its query into the Engine call.
    ///
    /// Hosts that own the deserialized query do not need it after this call. Moving its strings
    /// and pinyin segments avoids rebuilding the same bounded query just before every asynchronous
    /// candidate is merged.
    pub fn apply_online_candidate_owned(
        &mut self,
        query: OnlineQuery,
        candidate: &str,
        source: u8,
    ) -> Result<bool, RuntimeError> {
        let cloud_candidates = query.cloud_candidates;
        let OnlineQuery {
            scheme,
            generation,
            identity,
            query_text,
            cache_key,
            pinyin_segments,
            cloud_eligible,
            ai_eligible,
            session_id,
            ..
        } = query;
        let query = OnlineQuerySnapshot {
            available: true,
            scheme,
            generation,
            identity,
            query_text,
            cache_key,
            pinyin_segments,
            cloud_eligible,
            ai_eligible,
            session_id,
        };
        self.apply_online_candidate_snapshot(query, candidate, source, cloud_candidates)
    }

    fn apply_online_candidate_snapshot(
        &mut self,
        query: OnlineQuerySnapshot,
        candidate: &str,
        source: u8,
        cloud_candidates: bool,
    ) -> Result<bool, RuntimeError> {
        // Provider callbacks are asynchronous and can be malformed even when
        // their query identity is still current. Keep the single-item path
        // subject to the same bounds as the batch path before handing text to
        // Engine; the Windows source rejects empty callback results as well.
        if candidate.is_empty()
            || !msime_client_core::is_bounded_text(candidate, 4096)
            || source > 1
            // Windows only merges a cloud suggestion into an existing
            // candidate page.  A callback arriving after the local page was
            // cleared must not manufacture a new page from stale provider
            // state.  AI suggestions intentionally do not use this guard:
            // Windows accepts them for an otherwise eligible pinyin query
            // even when the local dictionary returned no rows.
            || (source == 0 && self.cached.candidates.is_empty())
            || (source == 0 && (!cloud_candidates || !query.cloud_eligible))
            || (source == 1 && !query.ai_eligible)
        {
            return Ok(false);
        }
        let applied = self
            .engine
            .apply_online_candidate(&query, candidate, source)
            .map_err(|error| RuntimeError::Engine(error.to_string()))?;
        if applied {
            // An asynchronous provider replaces the visible Engine candidate
            // set without going through dispatch(). Advance the host-owned
            // identity just as an input action does, so stale candidate IDs
            // cannot select the pre-provider page and Windows UI mailboxes can
            // recognize the replacement as a new rendered generation.
            self.advance()?;
            self.refresh()
                .map_err(|error| RuntimeError::Engine(error.to_string()))?;
        }
        Ok(applied)
    }
    pub fn apply_online_candidates(
        &mut self,
        query: &OnlineQuery,
        candidates: &[String],
        source: u8,
    ) -> Result<bool, RuntimeError> {
        let cloud_candidates = query.cloud_candidates;
        let limit = if source == 0 {
            1
        } else {
            query.ai_candidate_limit()
        };
        let query = OnlineQuerySnapshot {
            available: true,
            scheme: query.scheme,
            generation: query.generation,
            identity: query.identity.clone(),
            query_text: query.query_text.clone(),
            cache_key: query.cache_key.clone(),
            pinyin_segments: query.pinyin_segments.clone(),
            cloud_eligible: query.cloud_eligible,
            ai_eligible: query.ai_eligible,
            session_id: query.session_id,
        };
        self.apply_online_candidates_snapshot(query, candidates, source, cloud_candidates, limit)
    }

    /// Apply an ordered provider batch while transferring its query into the Engine call.
    pub fn apply_online_candidates_owned(
        &mut self,
        query: OnlineQuery,
        candidates: &[String],
        source: u8,
    ) -> Result<bool, RuntimeError> {
        let cloud_candidates = query.cloud_candidates;
        let limit = if source == 0 {
            1
        } else {
            query.ai_candidate_limit()
        };
        let OnlineQuery {
            scheme,
            generation,
            identity,
            query_text,
            cache_key,
            pinyin_segments,
            cloud_eligible,
            ai_eligible,
            session_id,
            ..
        } = query;
        let query = OnlineQuerySnapshot {
            available: true,
            scheme,
            generation,
            identity,
            query_text,
            cache_key,
            pinyin_segments,
            cloud_eligible,
            ai_eligible,
            session_id,
        };
        self.apply_online_candidates_snapshot(query, candidates, source, cloud_candidates, limit)
    }

    fn apply_online_candidates_snapshot(
        &mut self,
        query: OnlineQuerySnapshot,
        candidates: &[String],
        source: u8,
        cloud_candidates: bool,
        limit: usize,
    ) -> Result<bool, RuntimeError> {
        if candidates.is_empty()
            || candidates.len() > limit
            || candidates
                .iter()
                .any(|text| text.is_empty() || !msime_client_core::is_bounded_text(text, 4096))
            || source > 1
            || (source == 0 && self.cached.candidates.is_empty())
            || (source == 0 && (!cloud_candidates || !query.cloud_eligible))
            || (source == 1 && !query.ai_eligible)
        {
            return Ok(false);
        }
        let applied = self
            .engine
            .apply_online_candidates(&query, candidates, source)
            .map_err(|error| RuntimeError::Engine(error.to_string()))?;
        if applied {
            self.advance()?;
            self.refresh()
                .map_err(|error| RuntimeError::Engine(error.to_string()))?;
        }
        Ok(applied)
    }
}

impl<E: InputEngine> Runtime<E> {
    pub fn set_paired_punctuation_enabled(&mut self, enabled: bool) -> Result<(), RuntimeError> {
        self.engine.set_paired_punctuation_enabled(enabled)
    }

    pub fn balance_paired_punctuation_after_auto_close(
        &mut self,
        opening: u8,
    ) -> Result<(), RuntimeError> {
        if opening != b'<' {
            return Err(RuntimeError::InvalidPunctuation);
        }
        self.engine
            .balance_paired_punctuation_after_auto_close(opening)
    }

    pub fn set_punctuation_lock(&mut self, lock: u8) -> Result<(), RuntimeError> {
        self.engine.set_punctuation_lock(lock)?;
        self.refresh_idle_spelling_symbols()
    }

    /// With nothing composed, whether `/` and `@` open their modes follows the punctuation mode and lock, so the cached `spelling_symbols` that `punctuation` and the host read is taken again. A composition keeps its view: its symbols do not depend on either.
    fn refresh_idle_spelling_symbols(&mut self) -> Result<(), RuntimeError> {
        if self.cached.editing_text.is_empty() {
            self.refresh()?;
        }
        Ok(())
    }

    pub fn set_dedicated_english(&mut self, enabled: bool) -> Result<(), RuntimeError> {
        self.advance()?;
        self.engine.set_dedicated_english(enabled)?;
        self.refresh()
    }

    /// Hand the Engine a new `/` command table. An open command list is rebuilt from it, so the view is refreshed.
    pub fn set_command_table(&mut self, table: &[CommandTableEntry]) -> Result<(), RuntimeError> {
        self.advance()?;
        self.engine.set_command_table(table)?;
        self.refresh()
    }

    /// 把宿主给的辅助码表交给引擎（`None` 回到方案自己的表），并刷新视图里的候选和辅助码注释。
    pub fn set_helpcode_table(&mut self, table: Option<SharedKeymap>) -> Result<(), RuntimeError> {
        self.advance()?;
        self.engine.set_helpcode_table(table)?;
        self.refresh()
    }

    /// 把新的 K 模式宿主短语表交给引擎；打开的 K 模式列表据此重建，所以视图要刷新。
    pub fn set_quick_phrase_table(
        &mut self,
        table: &[QuickPhraseEntry],
    ) -> Result<(), RuntimeError> {
        self.advance()?;
        self.engine.set_quick_phrase_table(table)?;
        self.refresh()
    }

    /// Hand the Engine a new `@` name list, refreshing the view as `set_command_table` does.
    pub fn set_mention_entries(&mut self, entries: &[MentionEntry]) -> Result<(), RuntimeError> {
        self.advance()?;
        self.engine.set_mention_entries(entries)?;
        self.refresh()
    }

    /// Turn the places of `@` mode on or off, refreshing the view as `set_mention_entries` does. The engine starts with them off, so a new or replaced engine needs this again.
    pub fn set_mention_places(&mut self, enabled: bool) -> Result<(), RuntimeError> {
        self.advance()?;
        self.engine.set_mention_places(enabled)?;
        self.refresh()
    }

    pub fn new(engine: E, page_size: u8) -> Result<Self, RuntimeError> {
        Self::new_with_touch_layout(engine, page_size, TouchKeyboardLayout::default())
    }

    pub fn new_with_touch_layout(
        engine: E,
        page_size: u8,
        touch_keyboard_layout: TouchKeyboardLayout,
    ) -> Result<Self, RuntimeError> {
        if !(1..=9).contains(&page_size) {
            return Err(RuntimeError::InvalidPageSize);
        }
        let session = NEXT_SESSION
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |id| id.checked_add(1))
            .map_err(|_| RuntimeError::IdentityExhausted)?;
        let cached = engine.snapshot()?;
        Ok(Self {
            engine,
            session,
            generation: 0,
            ai_context: String::new(),
            reranker: None,
            settled_reranker: None,
            settled_rerank_enabled: true,
            focused: false,
            page_size: page_size.into(),
            highlighted: 0,
            translations: HashMap::new(),
            // The initial snapshot is already in Engine order. Keep the mapping empty until a
            // presentation reorder actually needs it; engine_index falls back to the seat while
            // the list remains untouched.
            engine_order: Vec::new(),
            cached,
            snapshot_valid: true,
            character_width: CharacterWidth::Halfwidth,
            touch_keyboard_layout,
            phrase_preedit: false,
            phrase_prefix: String::new(),
            phrase_selections: Vec::new(),
        })
    }

    /// Hold a half-composed phrase in the composition instead of committing its parts.
    ///
    /// The host promises to draw [`View::phrase_prefix`] ahead of the editing text; see the field
    /// for why this is the host's call. Turning it off while a phrase is held commits what is held,
    /// because the alternative is dropping text the user already chose.
    pub fn set_phrase_preedit(&mut self, enabled: bool) -> Option<String> {
        self.phrase_preedit = enabled;
        if enabled || self.phrase_prefix.is_empty() {
            self.phrase_selections.clear();
            return None;
        }
        self.phrase_selections.clear();
        Some(std::mem::take(&mut self.phrase_prefix))
    }

    /// Attach a candidate reranker. Hosts load the model themselves, because where a model file
    /// lives is a packaging question that differs per platform and the runtime has no business
    /// guessing at it.
    pub fn set_reranker(&mut self, reranker: Option<Reranker>) {
        self.reranker = reranker;
    }

    /// Whether a per-keystroke reranker is attached, so a host can attach or drop it when the preference that governs it changes.
    pub fn has_reranker(&self) -> bool {
        self.reranker.is_some()
    }

    /// Attach the model that runs after typing settles. Absent leaves the behaviour unchanged.
    pub fn set_settled_reranker(&mut self, reranker: Option<Reranker>) {
        self.settled_reranker = reranker;
    }

    /// Turn the settled rerank on or off without dropping the attached model, so switching it back on needs no reload. Hosts drive this from the desktop sentence model preference.
    pub fn set_settled_rerank_enabled(&mut self, enabled: bool) {
        self.settled_rerank_enabled = enabled;
    }

    pub fn settled_rerank_enabled(&self) -> bool {
        self.settled_rerank_enabled
    }

    /// Re-rank the current candidates with the settled model, reporting whether the order moved. Nothing happens without an attached model or while the settled rerank is switched off.
    ///
    /// The host decides when this is: it owns the clock and already runs a settle timer for cloud candidates. The runtime has no timer of its own and should not grow one — a keystroke that arrives while this is deciding makes the whole answer stale, and only the host knows that a keystroke arrived.
    ///
    /// Returns false when nothing changed, so a host can skip redrawing the candidate window. A window that repaints identically on every pause is a flicker the user cannot explain.
    pub fn rerank_settled(&mut self) -> bool {
        // A reorder has to advance the generation (old IDs would otherwise select by the new seats), so an exhausted generation cannot reorder at all.
        if !self.settled_rerank_enabled
            || self.settled_reranker.is_none()
            || self.is_idle()
            || self.generation.checked_add(1).is_none()
        {
            return false;
        }
        std::mem::swap(&mut self.reranker, &mut self.settled_reranker);
        let mut moved = self.rerank();
        std::mem::swap(&mut self.reranker, &mut self.settled_reranker);
        // The same passes the fast path runs after its rerank, so the seats they fix stay fixed.
        moved |= self.demote_runner_up_readings();
        moved |= self.normalize_online_slots();
        if moved {
            self.snapshot_valid = true;
            self.highlighted = 0;
            let _ = self.advance();
        }
        moved
    }

    pub fn set_character_width(&mut self, width: CharacterWidth) {
        self.character_width = width;
    }

    /// Put text into the committed context without typing it.
    ///
    /// The context a candidate is ranked against is whatever the user just committed, and it is
    /// what lets the model tell 会议 from 回忆. An evaluation harness has to be able to establish
    /// that context: replaying it as keystrokes would make each case depend on how well the
    /// *previous* sentence converted, which is precisely the confound a per-case measurement is
    /// supposed to remove. Bounded and focus-gated exactly as a real commit is, so a seeded
    /// session is indistinguishable from one that typed its way there.
    pub fn seed_context(&mut self, text: &str) {
        self.remember_commit(text);
    }

    /// Forget the committed text that candidates are ranked against, which [`Runtime::seed_context`] fills.
    ///
    /// [`Runtime::seed_context`] appends, exactly as a commit does, and cancelling a composition keeps what was committed before it. A harness that seeds a different context per case therefore has to clear it first, or every case is ranked against the tail of all the cases before it. The Engine's rescoring context is cleared with it, so both rankers start from the same empty sentence. The Engine's committed-word context is left alone: seeding never adds to it, so there is nothing of the seed's there to forget, unlike [`Runtime::focus`], which also ends real commits.
    pub fn clear_context(&mut self) {
        self.ai_context.clear();
        self.engine.set_rescoring_context("");
    }

    /// Switch the Engine's digit interpretation only after the host finishes composition.
    /// Move the caret used by Engine prefix decoding. `None` restores end-of-composition behavior.
    pub fn set_caret(&mut self, caret: Option<usize>) -> Result<(), RuntimeError> {
        self.engine.set_caret(caret);
        self.refresh()
            .map_err(|error| RuntimeError::Engine(error.to_string()))
    }

    /// Raw offset consumed by the candidate decoder, floored to a complete pinyin unit.
    pub fn prefix_end(&self) -> usize {
        self.engine.prefix_end()
    }

    /// Original-cased raw input after the decoded prefix.
    pub fn pending_suffix(&self) -> String {
        self.engine.pending_suffix()
    }

    pub fn set_nine_key_enabled(&mut self, enabled: bool) -> Result<(), RuntimeError> {
        if enabled && !scheme_type(self.cached.scheme).is_some_and(SchemeType::nine_key) {
            return Err(RuntimeError::InvalidNineKeyScheme);
        }
        if !self.is_idle() {
            return Err(RuntimeError::CompositionActive);
        }
        if self.cached.nine_key == enabled {
            return Ok(());
        }
        self.advance()?;
        self.engine.set_nine_key_enabled(enabled)?;
        self.refresh()
    }

    pub fn view(&self) -> View {
        let page = self.highlighted / self.page_size;
        let start = page * self.page_size;
        let page_len = self
            .cached
            .candidates
            .len()
            .saturating_sub(start)
            .min(self.page_size);
        let mut candidates = Vec::with_capacity(page_len);
        candidates.extend(
            self.cached
                .candidates
                .iter()
                .enumerate()
                .skip(start)
                .take(page_len)
                .map(|(index, text)| self.candidate(index, text)),
        );
        View {
            scheme: self.cached.scheme,
            chinese_text: scheme_type(self.cached.scheme).is_some_and(SchemeType::is_chinese),
            script_conversion: script_conversion(self.cached.scheme, &self.cached.local_mode),
            nine_key: self.cached.nine_key,
            nine_key_spellings: self.cached.nine_key_spellings.clone(),
            nine_key_reading: self.cached.nine_key_reading.clone(),
            touch_keyboard_layout: self.touch_keyboard_layout,
            character_width: self.character_width,
            microsoft_shuangpin: self.cached.microsoft_shuangpin,
            shuangpin_profile: self.cached.shuangpin_profile.clone(),
            answered_by_pinyin_fallback: self.cached.answered_by_pinyin_fallback,
            local_mode: self.cached.local_mode.clone(),
            // A held phrase piece is a composition too: no key opens a mode behind it, and a host that reads the symbols (Harmony) must not compose or pick with them.
            spelling_symbols: if self.phrase_prefix.is_empty() || self.cached.local_mode != "none" {
                self.cached.spelling_symbols.clone()
            } else {
                String::new()
            },
            dedicated_english: self.cached.dedicated_english,
            session: self.session,
            generation: self.generation,
            focused: self.focused,
            preedit: self.cached.preedit.clone(),
            phrase_prefix: self.phrase_prefix.clone(),
            reading: self.cached.reading.clone(),
            editing_text: self.cached.editing_text.clone(),
            caret_position: self.cached.caret_position,
            page,
            page_size: self.page_size,
            page_count: self.cached.candidates.len().div_ceil(self.page_size),
            candidate_list_open: self.cached.candidate_list_open,
            candidates,
        }
    }

    /// Number of candidates on the currently published page, without building candidate rows.
    /// Hosts that only need the count for a punctuation decision can avoid materializing a full
    /// [`View`].
    pub fn candidate_page_len(&self) -> usize {
        let start = (self.highlighted / self.page_size) * self.page_size;
        self.cached
            .candidates
            .len()
            .saturating_sub(start)
            .min(self.page_size)
    }

    /// The Engine scheme ordinal of the applied state, without materializing a [`View`].
    pub fn scheme(&self) -> u8 {
        self.cached.scheme
    }

    /// Whether punctuation may use the Engine's Chinese route for this applied state.
    /// This mirrors the host-facing mode checks without materializing a [`View`].
    pub fn punctuation_host_context_available(&self, english_mode: bool) -> bool {
        !english_mode
            && !self.cached.dedicated_english
            && self.cached.local_mode == "none"
            && scheme_type(self.cached.scheme).is_some_and(SchemeType::host_smart_punctuation)
    }

    /// Copy only the state and candidate fields needed to plan translation requests. This avoids
    /// constructing display-only codes, annotations, IDs and highlight flags on every key.
    pub fn translation_candidates(&self) -> Option<TranslationCandidates> {
        let start = (self.highlighted / self.page_size) * self.page_size;
        let page_len = self
            .cached
            .candidates
            .len()
            .saturating_sub(start)
            .min(self.page_size);
        let mut candidates = Vec::with_capacity(page_len);
        candidates.extend(
            self.cached
                .candidates
                .iter()
                .enumerate()
                .skip(start)
                .take(page_len)
                .map(|(index, text)| TranslationCandidate {
                    text: text.clone(),
                    source: self
                        .cached
                        .candidate_sources
                        .get(index)
                        .copied()
                        .unwrap_or_default(),
                }),
        );
        (!candidates.is_empty()).then_some(TranslationCandidates {
            generation: self.generation,
            scheme: self.cached.scheme,
            local_mode: self.cached.local_mode.clone(),
            candidates,
        })
    }

    /// Copy the complete candidate generation for an explicitly opened panel.
    ///
    /// The engine holds candidates back behind the initial answer and only releases them when asked
    /// (`expand_initial_candidates`), which until now happened solely on the way into the last page.
    /// A host that pages reaches them; a host that opens the whole list instead -- which the touch
    /// keyboards do, having dropped paging -- never did, so the panel that promises everything was
    /// quietly showing the first tranche. Release them here too: this call is the request for all of
    /// them. A refusal is not fatal; the caller still gets whatever the generation already holds.
    pub fn all_candidates(&mut self) -> CandidateSnapshot {
        // Candidate IDs are tied to the generation.  Once that identity space is
        // exhausted we cannot publish a reordered snapshot safely: advancing would
        // fail and retaining the old generation would let an ID from the previous
        // seat select a different candidate.  Keep the currently published view
        // stable; callers can still inspect the candidates already released.
        if self.generation == u64::MAX {
            return self.all_candidates_cached();
        }
        // The released tail reorders the list, so the page's IDs must not keep selecting by seat.
        if self.expand_cached_candidates().unwrap_or(false) {
            // `generation == u64::MAX` was handled above, so this cannot fail.
            debug_assert!(self.advance().is_ok());
        }
        self.all_candidates_cached()
    }

    /// The generation as it stands, without asking the engine for more.
    fn all_candidates_cached(&self) -> CandidateSnapshot {
        let mut candidates = Vec::with_capacity(self.cached.candidates.len());
        candidates.extend(
            self.cached
                .candidates
                .iter()
                .enumerate()
                .map(|(index, text)| self.candidate(index, text)),
        );
        CandidateSnapshot {
            session: self.session,
            generation: self.generation,
            preedit: self.cached.preedit.clone(),
            reading: self.cached.reading.clone(),
            candidates,
        }
    }

    fn candidate(&self, index: usize, text: &str) -> Candidate {
        Candidate {
            id: CandidateId {
                session: self.session,
                generation: self.generation,
                index,
            },
            text: text.to_owned(),
            code: self
                .cached
                .candidate_codes
                .get(index)
                .cloned()
                .unwrap_or_default(),
            annotation: self
                .cached
                .candidate_annotations
                .get(index)
                .cloned()
                .unwrap_or_default(),
            source: self
                .cached
                .candidate_sources
                .get(index)
                .copied()
                .unwrap_or_default(),
            corrected: self
                .cached
                .candidate_corrected
                .get(index)
                .copied()
                .unwrap_or(false),
            fixed_position: self
                .cached
                .candidate_positions
                .get(index)
                .copied()
                .unwrap_or_default(),
            highlighted: index == self.highlighted,
            translation: (!self.translations.is_empty())
                .then(|| self.translations.get(text).cloned())
                .flatten(),
        }
    }

    /// A held phrase counts as a composition even when its reading is empty.
    pub fn is_idle(&self) -> bool {
        self.snapshot_valid
            && self.phrase_prefix.is_empty()
            && self.cached.preedit.is_empty()
            && self.cached.editing_text.is_empty()
            && self.cached.candidates.is_empty()
    }

    /// The host uses the generation to detect whether a deferred preference
    /// update changed the view after an input action.
    pub fn generation(&self) -> u64 {
        self.generation
    }

    /// The live `/fy` request, if the composition is one; see [`CommandTranslation`].
    pub fn command_translation(&self) -> Option<CommandTranslation> {
        let query = self.engine.command_translation_query()?;
        Some(CommandTranslation {
            generation: self.generation,
            session_id: query.session_id,
            text: query.text,
        })
    }

    /// Show a translation for a `/fy` request as the first row, which commits the translation. False for an answer to an older generation, another session or text no longer typed, and for text a row cannot show.
    pub fn apply_command_translation(
        &mut self,
        query: &CommandTranslation,
        translation: &str,
    ) -> Result<bool, RuntimeError> {
        if query.generation != self.generation
            || !msime_client_core::is_bounded_text(translation, 4096)
        {
            return Ok(false);
        }
        let request = CommandTranslationQuery {
            session_id: query.session_id,
            text: query.text.clone(),
        };
        if !self.engine.apply_command_translation(&request, translation) {
            return Ok(false);
        }
        // The row arrives outside dispatch(), so the identity advances as it does for an online candidate and an ID from the page before cannot select the new first row.
        self.advance()?;
        self.refresh()?;
        Ok(true)
    }

    /// Apply translations to the current candidate generation. Stale async
    /// responses are ignored so a newer candidate window cannot be polluted.
    pub fn apply_translations(
        &mut self,
        generation: u64,
        translations: impl IntoIterator<Item = (String, String)>,
    ) -> bool {
        if generation != self.generation {
            return false;
        }
        let translations: HashMap<String, String> = translations.into_iter().collect();
        // The answer to a `/fy` request arrives through the same host path as candidate glosses, so a host forwarding `translation_query` needs nothing else: it becomes the row that commits the translation, not a gloss under the English.
        if let Some(query) = self.command_translation() {
            if let Some(translation) = translations.get(&query.text) {
                return self
                    .apply_command_translation(&query, translation)
                    .unwrap_or(false);
            }
        }
        self.translations = translations;
        true
    }

    /// Presentation-only resize; a live composition keeps its numeric key map.
    pub fn set_page_size(&mut self, page_size: u8) -> Result<(), RuntimeError> {
        if !(1..=9).contains(&page_size) {
            return Err(RuntimeError::InvalidPageSize);
        }
        if self.page_size == usize::from(page_size) {
            return Ok(());
        }
        if !self.is_idle() {
            return Err(RuntimeError::CompositionActive);
        }
        self.advance()?;
        self.page_size = page_size.into();
        Ok(())
    }

    /// Preserve the host handle/focus while invalidating every old candidate ID.
    /// Validate the replacement before changing any live state.
    pub fn replace_engine(&mut self, engine: E, page_size: u8) -> Result<(), RuntimeError> {
        self.replace_engine_with_touch_layout(engine, page_size, self.touch_keyboard_layout)
    }

    pub fn replace_engine_with_touch_layout(
        &mut self,
        engine: E,
        page_size: u8,
        touch_keyboard_layout: TouchKeyboardLayout,
    ) -> Result<(), RuntimeError> {
        if !(1..=9).contains(&page_size) {
            return Err(RuntimeError::InvalidPageSize);
        }
        if !self.is_idle() {
            return Err(RuntimeError::CompositionActive);
        }
        let cached = engine.snapshot()?;
        self.advance()?;
        self.engine = engine;
        // The committed text belongs to the client, not the engine, so the replacement ranks against it as its predecessor did.
        self.engine.set_rescoring_context(&self.ai_context);
        self.load_snapshot(cached);
        self.snapshot_valid = true;
        self.page_size = page_size.into();
        self.highlighted = 0;
        self.touch_keyboard_layout = touch_keyboard_layout;
        Ok(())
    }

    /// The Engine caps a single-letter query at twenty-four candidates so the first page is cheap,
    /// and hands over the rest only when asked. Without this, paging stops at that cap and the rest
    /// of the dictionary is unreachable for those queries.
    ///
    /// Expanding when the next page would be the partial last one keeps that page full the first
    /// time it is shown, rather than showing a short page that silently grows.
    ///
    /// Answers whether the arrivals filled the page the caller is already on, in which case paging
    /// has to stay put: advancing would step over the candidates that just showed up.
    fn expand_for_next_page(&mut self) -> Result<bool, RuntimeError> {
        let len = self.cached.candidates.len();
        if len == 0 {
            return Ok(false);
        }
        let page = self.highlighted / self.page_size;
        let last_page = (len - 1) / self.page_size;
        let next_is_partial_last = page + 1 == last_page && !len.is_multiple_of(self.page_size);
        if page != last_page && !next_is_partial_last {
            return Ok(false);
        }
        let page_was_full = (page + 1) * self.page_size <= len;
        if !self.expand_cached_candidates()? {
            return Ok(false);
        }
        Ok(page == last_page && !page_was_full)
    }

    /// Moving the highlight off the end of the loaded list has to release the withheld candidates
    /// too, not only paging.
    ///
    /// The same cap sits behind both. A host that walks the list one candidate at a time - which is
    /// every arrow key and every mouse wheel notch - would otherwise stop at the twenty-fourth
    /// candidate and be unable to reach the rest of the dictionary, while pressing page-down on the
    /// same query walks straight past it. The second condition mirrors the paging one: stepping into
    /// the partial last page fills it first, so it is never shown short and then grown.
    fn expand_for_next_candidate(&mut self) -> Result<(), RuntimeError> {
        let len = self.cached.candidates.len();
        if len == 0 {
            return Ok(());
        }
        let page = self.highlighted / self.page_size;
        let last_page = (len - 1) / self.page_size;
        let at_last_candidate = self.highlighted + 1 == len;
        let at_page_end = (self.highlighted + 1).is_multiple_of(self.page_size);
        let next_is_partial_last = page + 1 == last_page && !len.is_multiple_of(self.page_size);
        if !at_last_candidate && !(at_page_end && next_is_partial_last) {
            return Ok(());
        }
        self.expand_cached_candidates()?;
        Ok(())
    }

    /// Ask the Engine for what it held back, and re-apply the orderings the cached page carries:
    /// the arrivals are ranked against the candidates already on screen, not appended raw.
    fn expand_cached_candidates(&mut self) -> Result<bool, RuntimeError> {
        if !self.engine.expand_initial_candidates()? {
            return Ok(false);
        }
        let snapshot = self.engine.snapshot()?;
        self.load_snapshot(snapshot);
        self.rerank();
        self.demote_runner_up_readings();
        self.normalize_online_slots();
        Ok(true)
    }

    /// Take a snapshot straight from the Engine, whose seats are still in the Engine's order.
    fn load_snapshot(&mut self, snapshot: EngineSnapshot) {
        // An empty mapping means the cached order is the Engine's order; build it lazily only if
        // one of the presentation reorderings below actually moves a candidate.
        self.engine_order.clear();
        self.cached = snapshot;
    }

    /// The Engine's index for the candidate sitting at `seat` of the cached list. A seat past the end is passed through unchanged, so the Engine keeps answering for an empty page exactly as it did.
    fn engine_index(&self, seat: usize) -> usize {
        self.engine_order.get(seat).copied().unwrap_or(seat)
    }

    fn advance(&mut self) -> Result<(), RuntimeError> {
        self.generation = self
            .generation
            .checked_add(1)
            .ok_or(RuntimeError::IdentityExhausted)?;
        Ok(())
    }

    /// Keep the tail of what was committed, cut on a character boundary, and hand it to the Engine.
    ///
    /// Bounded at 1024 bytes because `query_candidates` refuses anything longer outright - an over-long context would silently disable the whole query rather than being trimmed for us.
    pub(crate) fn remember_commit(&mut self, text: &str) {
        if !self.focused {
            self.ai_context.clear();
        } else {
            self.ai_context.push_str(text);
            if self.ai_context.len() > 1024 {
                let mut cut = self.ai_context.len() - 1024;
                while cut < self.ai_context.len() && !self.ai_context.is_char_boundary(cut) {
                    cut += 1;
                }
                self.ai_context.drain(..cut);
            }
        }
        self.engine.set_rescoring_context(&self.ai_context);
    }

    /// Take the last selection of a phrase-in-progress back, when the key asks for it.
    ///
    /// Two rules, both the reference's (`ShouldRetreatCreatingWordSelection` and
    /// `ShouldDropCreatingWordSegment` in its `input_key_policy.h`), and both about the same
    /// situation: the user has picked a candidate that covered part of the input, is looking at the
    /// piece it produced, and wants it back.
    ///
    /// - Backspace with at most one character of reading left, the caret at its end: the key would
    ///   otherwise delete that character and end the composition, taking the chosen piece with it.
    ///   Instead the newest selection is undone - its reading comes back and the held phrase
    ///   returns to what it was before it - so the user can pick again.
    /// - Segment Backspace (Ctrl+Backspace) with nothing before the caret: the reading this key
    ///   deletes by units has already been emptied, so it deletes the selection itself. Its reading
    ///   is *not* restored: the user asked to remove the segment, not to edit its spelling.
    ///
    /// Returns `None` for every other key, which then runs as usual.
    ///
    /// The reference also requires a client that negotiated `CompositionRestore` and a host that is
    /// not UILess, because its TSF side has to rebuild the composition from a reply it may not
    /// understand. Here that condition is `phrase_preedit`: a host only turns it on once it draws
    /// the held phrase from the view, and the view is how every host here learns the composition
    /// changed.
    fn retreat_phrase_selection(
        &mut self,
        action: &Action,
    ) -> Result<Option<Transition>, RuntimeError> {
        if !self.phrase_preedit || self.phrase_selections.is_empty() {
            return Ok(None);
        }
        let reading = self.cached.editing_text.as_str();
        let caret = self.cached.caret_position;
        let restore = match action {
            Action::Command(Command::Backspace) => {
                if reading.chars().count() > 1 || caret != reading.len() {
                    return Ok(None);
                }
                true
            }
            Action::SegmentBackspace => {
                if caret != 0 {
                    return Ok(None);
                }
                false
            }
            _ => return Ok(None),
        };
        let selection = self
            .phrase_selections
            .pop()
            .expect("the stack was checked above");
        self.phrase_prefix = selection.word_before;
        if !restore {
            // The segment is gone and its spelling with it. The reading is already empty, so the
            // Engine has nothing to say; the view still changes, because the held phrase is
            // shorter - or gone, which ends the composition with nothing committed.
            self.refresh()?;
            return Ok(Some(self.transition(empty_result(true))));
        }
        // Put the reading back. The reference hands its Engine the spelling directly
        // (`set_pinyin_sequence` then `recompute_candidates`); this Engine is only reachable
        // through the keys that built the composition, so the composition is thrown away and the
        // spelling typed again. The user sees the same thing either way: the pinyin that selection
        // consumed, with its candidates, and the caret at its end.
        self.engine.command(Command::Cancel)?;
        for byte in selection.reading.bytes() {
            self.engine.character(byte, byte.is_ascii_uppercase())?;
        }
        self.refresh()?;
        Ok(Some(self.transition(empty_result(true))))
    }

    /// Keep a chosen piece of a phrase out of the document until the phrase is done.
    ///
    /// Three things can happen to what the Engine hands back:
    ///
    /// - it picked a candidate and is still composing the rest, so the piece is held;
    /// - something ended the composition and committed, so the held pieces lead that commit - the
    ///   reference does the same on Enter, which commits `word_for_creating_word` together with the
    ///   remaining raw input;
    /// - the reading is gone with nothing committed. A cancel means the user threw the whole thing away, so the held pieces go with it. A segment Backspace (`keep_empty`) that emptied the reading while a selection can still be taken back keeps the phrase in the composition, as the reference's `keep_creating_word_after_empty_raw` does: the next Backspace puts the last reading back and the next Ctrl+Backspace deletes the last chosen piece, both in [`Runtime::retreat_phrase_selection`]. Anything else commits what is held rather than dropping letters the user chose. A plain Backspace only empties the reading here with nothing to go back to - a selection that can be taken back takes that key first - and the reference ends the word in that case too.
    ///
    /// When the held phrase is all there is to send or throw away, the key acted on the composition, so it counts as handled: an Enter or Space the Engine does not want with an empty reading must not also reach the application.
    fn hold_phrase_progress(
        &mut self,
        picked: bool,
        discard: bool,
        keep_empty: bool,
        consumed: &str,
        result: &mut EngineResult,
    ) {
        // A Korean syllable that the next key finished is already final text, not a chosen piece of a phrase: it goes to the document even while the next syllable composes. So does anything a scheme that never holds phrase progress commits.
        if !self.phrase_preedit
            || scheme_type(self.cached.scheme).is_some_and(|scheme| !scheme.holds_phrase_progress())
        {
            return;
        }
        let composing = !self.cached.editing_text.is_empty();
        if picked && result.has_commit && composing {
            if !consumed.is_empty() {
                self.phrase_selections.push(PhraseSelection {
                    word_before: self.phrase_prefix.clone(),
                    reading: consumed.to_owned(),
                });
            }
            self.phrase_prefix.push_str(&result.commit);
            result.has_commit = false;
            result.commit = String::new();
            return;
        }
        if self.phrase_prefix.is_empty() || composing {
            return;
        }
        if keep_empty && !discard && !result.has_commit && !self.phrase_selections.is_empty() {
            result.handled = true;
            return;
        }
        let held = std::mem::take(&mut self.phrase_prefix);
        self.phrase_selections.clear();
        if result.has_commit {
            if !discard {
                result.commit = held + &result.commit;
            }
            return;
        }
        result.handled = true;
        if !discard {
            result.has_commit = true;
            result.commit = held;
        }
    }

    /// The context of a commit made in the applied state.
    fn output_context(&self) -> OutputContext {
        OutputContext {
            scheme: self.cached.scheme,
            local_mode: self.cached.local_mode.clone(),
            script_conversion: script_conversion(self.cached.scheme, &self.cached.local_mode),
            typing_statistics: local_mode_counts_as_typing(&self.cached.local_mode),
        }
    }

    fn transition(&mut self, result: EngineResult) -> Transition {
        // Every commit passes through here, so this is the one place the AI
        // context has to be fed from.
        if result.has_commit {
            self.remember_commit(&result.commit);
        }
        Transition {
            commit_context: result.has_commit.then(|| self.output_context()),
            handled: result.handled,
            commit: result.has_commit.then_some(result.commit),
            diagnostic: (!result.diagnostic.is_empty()).then_some(result.diagnostic),
            view: self.view(),
        }
    }

    /// Let the model promote a candidate the pinyin decoder assembled, if a host attached one.
    ///
    /// Reordering happens here because this is the one place a candidate list enters the runtime,
    /// so everything downstream — the view, `all_candidates`, the evaluation harness — sees the
    /// same order the user does.
    ///
    /// The candidate arrays run in parallel and every one of them has to move together. Rotating
    /// only the texts would leave each candidate wearing another's code, annotation and source.
    /// Seat the online candidates the way the reference does.
    ///
    /// `candidate_selection_policy.h` writes the arrangement out:
    ///
    /// ```text
    /// no cloud:    Chinese, English, AI, emoji, kaomoji
    /// cloud:       Chinese, cloud, AI, English, emoji, kaomoji
    /// cloud only:  Chinese, cloud, English, emoji, kaomoji
    /// base:        Chinese, English, emoji, kaomoji
    /// ```
    ///
    /// The reference applies it in its Server, on top of what the Engine returned. This client
    /// replaced that Server with this runtime and the step did not come across, so the Engine's own
    /// placement was what the user saw - and the two agree until an online candidate arrives.
    /// Injecting an AI suggestion moved the English candidate from the second seat to the fourth
    /// and put a second Chinese candidate in front of it, which is the last line of the table read
    /// backwards.
    ///
    /// Only the online case is touched: with neither a cloud nor an AI candidate present the
    /// Engine already produces the fourth line, so there is nothing to rearrange and nothing to
    /// risk.
    fn normalize_online_slots(&mut self) -> bool {
        const CLOUD: u8 = 2;
        const AI: u8 = 3;
        const ENGLISH: u8 = 4;
        const EMOJI: u8 = 6;
        const KAOMOJI: u8 = 7;

        let snapshot = &self.cached;
        let count = snapshot.candidates.len();
        if count < 2
            || snapshot.candidate_sources.len() != count
            || snapshot.candidate_codes.len() != count
            || snapshot.candidate_annotations.len() != count
            || snapshot.candidate_positions.len() != count
            || snapshot.candidate_corrected.len() != count
            || snapshot.candidate_answers_key.len() != count
        {
            return false;
        }
        if !snapshot
            .candidate_sources
            .iter()
            .any(|source| *source == CLOUD || *source == AI)
        {
            return false;
        }

        // A provider may answer with several candidates - the AI limit reaches ten - and they take
        // their seat as a group. The reference has only one of each to place and silently drops the
        // rest; dropping a candidate the user was offered is not an option here.
        //
        // The Engine never puts English first while a Chinese candidate exists, whatever its weight or pin, except for a word the user fixed at position 1 (`apply_candidate_positions`). An English candidate at index zero with locals present is that word. It keeps the first seat and the leading English seat is not filled a second time.
        let is_local = |source: u8| !matches!(source, CLOUD | AI | ENGLISH | EMOJI | KAOMOJI);
        let first_source = |source: u8| {
            snapshot
                .candidate_sources
                .iter()
                .position(|candidate_source| *candidate_source == source)
        };
        let local_count = snapshot
            .candidate_sources
            .iter()
            .filter(|source| is_local(**source))
            .count();
        let first_english = first_source(ENGLISH);
        let promoted_english = first_english == Some(0) && local_count != 0;
        let has_cloud = snapshot.candidate_sources.contains(&CLOUD);
        let first_emoji = first_source(EMOJI);
        let first_kaomoji = first_source(KAOMOJI);
        let mut order = Vec::with_capacity(count);
        let append_group =
            |order: &mut Vec<usize>, source: u8, skip: usize, limit: Option<usize>| {
                let end = limit.map_or(usize::MAX, |count| skip.saturating_add(count));
                let mut matched = 0;
                for (index, candidate_source) in snapshot.candidate_sources.iter().enumerate() {
                    if *candidate_source != source {
                        continue;
                    }
                    if matched < skip {
                        matched += 1;
                        continue;
                    }
                    if matched >= end {
                        break;
                    }
                    order.push(index);
                    matched += 1;
                }
            };
        let append_local = |order: &mut Vec<usize>, skip: usize, limit: Option<usize>| {
            let end = limit.map_or(usize::MAX, |count| skip.saturating_add(count));
            let mut matched = 0;
            for (index, candidate_source) in snapshot.candidate_sources.iter().enumerate() {
                if !is_local(*candidate_source) {
                    continue;
                }
                if matched < skip {
                    matched += 1;
                    continue;
                }
                if matched >= end {
                    break;
                }
                order.push(index);
                matched += 1;
            }
        };
        if promoted_english {
            order.push(0);
        }
        // The hiragana/katakana pair of a single complete kana keeps seats 1 and 2 ahead of every online candidate, as the reference's `preserve_single_kana_pair` does (server/src/ipc/event_listener.cpp); the reading is the converted kana, so one character in U+3041..U+3096 is its `IsSingleKanaConversion`.
        let mut reading = snapshot.reading.chars();
        let single_kana = scheme_type(snapshot.scheme) == Some(SchemeType::JapaneseRomaji)
            && matches!((reading.next(), reading.next()), (Some(kana), None) if ('\u{3041}'..='\u{3096}').contains(&kana));
        let local_prefix = if single_kana { 2 } else { 1 };
        append_local(&mut order, 0, Some(local_prefix));
        if has_cloud {
            append_group(&mut order, CLOUD, 0, None);
            append_group(&mut order, AI, 0, None);
        }
        if !promoted_english {
            if let Some(index) = first_english {
                order.push(index);
            }
        }
        if !has_cloud {
            append_group(&mut order, AI, 0, None);
        }
        if let Some(index) = first_emoji {
            order.push(index);
        }
        if let Some(index) = first_kaomoji {
            order.push(index);
        }
        append_local(&mut order, local_prefix, None);
        append_group(&mut order, ENGLISH, 1, None);
        append_group(&mut order, EMOJI, 1, None);
        append_group(&mut order, KAOMOJI, 1, None);
        // An English candidate the user fixed to a seat goes back to that seat after the seating, so a cloud or AI reply does not push it behind the online candidates (reference: server/src/ipc/candidate_selection_policy.h, the fixed-English pass at the end of NormalizeMixedCandidateOrder). Seats are 1-based and 0 means unfixed; a seat past the end clamps to the end, as the reference's `insert_at` does.
        let mut fixed_english = Vec::with_capacity(count);
        order.retain(|index| {
            let fixed = snapshot.candidate_sources[*index] == ENGLISH
                && snapshot.candidate_positions[*index] > 0;
            if fixed {
                fixed_english.push(*index);
            }
            !fixed
        });
        fixed_english.sort_by_key(|index| snapshot.candidate_positions[*index]);
        for index in fixed_english {
            let seat = usize::from(snapshot.candidate_positions[index] - 1).min(order.len());
            order.insert(seat, index);
        }
        // A permutation or nothing: a missing or repeated index would silently drop a candidate.
        debug_assert_eq!(order.len(), count);
        if order.len() != count {
            return false;
        }
        if order.iter().enumerate().all(|(seat, index)| seat == *index) {
            return false;
        }

        ensure_engine_order(&mut self.engine_order, count);
        let snapshot = &mut self.cached;
        apply_order(&mut snapshot.candidates, &order);
        apply_order(&mut snapshot.candidate_codes, &order);
        apply_order(&mut snapshot.candidate_annotations, &order);
        apply_order(&mut snapshot.candidate_sources, &order);
        apply_order(&mut snapshot.candidate_positions, &order);
        apply_order(&mut snapshot.candidate_corrected, &order);
        apply_order(&mut snapshot.candidate_answers_key, &order);
        apply_order(&mut self.engine_order, &order);
        true
    }

    fn rerank(&mut self) -> bool {
        let Some(reranker) = self.reranker.as_mut() else {
            return false;
        };
        let snapshot = &self.cached;
        let count = snapshot.candidates.len();
        if count < 2
            || snapshot.candidate_sources.len() != count
            || snapshot.candidate_codes.len() != count
            || snapshot.candidate_annotations.len() != count
            || snapshot.candidate_positions.len() != count
            || snapshot.candidate_corrected.len() != count
            || snapshot.candidate_answers_key.len() != count
        {
            return false;
        }
        // 方案、五笔表码和上文窗口的判断都在 `rerank_pick` 里，这里只负责把八个并行数组同步旋转。
        let rows = order_rows(snapshot);
        let Some(promote) = rerank_pick(
            reranker,
            &self.ai_context,
            snapshot.scheme,
            snapshot.answered_by_pinyin_fallback,
            &rows,
        ) else {
            return false;
        };
        ensure_engine_order(&mut self.engine_order, count);
        let snapshot = &mut self.cached;
        rotate_to_front(&mut snapshot.candidates, promote);
        rotate_to_front(&mut snapshot.candidate_codes, promote);
        rotate_to_front(&mut snapshot.candidate_annotations, promote);
        rotate_to_front(&mut snapshot.candidate_sources, promote);
        rotate_to_front(&mut snapshot.candidate_positions, promote);
        rotate_to_front(&mut snapshot.candidate_corrected, promote);
        rotate_to_front(&mut snapshot.candidate_answers_key, promote);
        rotate_to_front(&mut self.engine_order, promote);
        true
    }

    /// Keep the leading sentence readings together near the top and move the rest of them behind the list.
    ///
    /// 规则本身（保留几条整句读法、哪些算整句、为什么挪而不删）见 `msime_engine::ordering::runner_up_order`；这里只把它给出的排列同步应用到八个并行数组上。
    pub(crate) fn demote_runner_up_readings(&mut self) -> bool {
        let snapshot = &self.cached;
        let count = snapshot.candidates.len();
        if count < 2
            || snapshot.candidate_codes.len() != count
            || snapshot.candidate_annotations.len() != count
            || snapshot.candidate_sources.len() != count
            || snapshot.candidate_positions.len() != count
            || snapshot.candidate_corrected.len() != count
            || snapshot.candidate_answers_key.len() != count
        {
            return false;
        }
        let Some(order) = runner_up_order(snapshot.scheme, &order_rows(snapshot)) else {
            return false;
        };
        ensure_engine_order(&mut self.engine_order, count);
        let snapshot = &mut self.cached;
        apply_order(&mut snapshot.candidates, &order);
        apply_order(&mut snapshot.candidate_codes, &order);
        apply_order(&mut snapshot.candidate_annotations, &order);
        apply_order(&mut snapshot.candidate_sources, &order);
        apply_order(&mut snapshot.candidate_positions, &order);
        apply_order(&mut snapshot.candidate_corrected, &order);
        apply_order(&mut snapshot.candidate_answers_key, &order);
        apply_order(&mut self.engine_order, &order);
        true
    }

    pub(crate) fn refresh(&mut self) -> Result<(), RuntimeError> {
        self.snapshot_valid = false;
        self.translations.clear();
        self.engine_order.clear();
        let previous_highlight = self.highlighted;
        self.highlighted = 0;
        // Drop cached candidate identities even if fetching the replacement fails.
        let snapshot = match self.engine.snapshot() {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.cached = EngineSnapshot {
                    scheme: 255,
                    nine_key: false,
                    nine_key_spellings: Vec::new(),
                    nine_key_reading: String::new(),
                    candidate_annotations: Vec::new(),
                    candidate_codes: Vec::new(),
                    candidate_sources: Vec::new(),
                    candidate_positions: Vec::new(),
                    candidate_corrected: Vec::new(),
                    candidate_answers_key: Vec::new(),
                    candidate_list_open: false,
                    microsoft_shuangpin: false,
                    shuangpin_profile: String::new(),
                    answered_by_pinyin_fallback: true,
                    wubi_unique_four_code: false,
                    local_mode: "unknown".into(),
                    spelling_symbols: String::new(),
                    dedicated_english: false,
                    preedit: String::new(),
                    reading: String::new(),
                    editing_text: String::new(),
                    caret_position: 0,
                    segment_raw_boundaries: vec![],
                    candidates: Vec::new(),
                };
                return Err(error);
            }
        };
        let previous = std::mem::replace(&mut self.cached, snapshot);
        self.rerank();
        self.demote_runner_up_readings();
        self.normalize_online_slots();
        self.snapshot_valid = true;
        if self.cached.editing_text == previous.editing_text
            && self.cached.scheme == previous.scheme
            && self.cached.local_mode == previous.local_mode
            && self.cached.reading == previous.reading
            && self.cached.dedicated_english == previous.dedicated_english
            && self.cached.candidates == previous.candidates
            && self.cached.candidate_codes == previous.candidate_codes
            && self.cached.candidate_annotations == previous.candidate_annotations
            && self.cached.candidate_sources == previous.candidate_sources
            && self.cached.candidate_positions == previous.candidate_positions
            && self.cached.candidate_corrected == previous.candidate_corrected
            && self.cached.candidate_answers_key == previous.candidate_answers_key
        {
            self.highlighted =
                previous_highlight.min(self.cached.candidates.len().saturating_sub(1));
        }
        Ok(())
    }

    pub fn focus(&mut self, focused: bool) -> Result<Transition, RuntimeError> {
        self.advance()?;
        // Invalidate the client before cancellation, including on engine failure.
        self.focused = false;
        // A Korean syllable is text the user already wrote, not a reading still to be converted, so leaving the client commits it, as it does for every scheme that commits on blur; every other composition is cancelled. Attaching a client (`focused`) stays a pure reset: a syllable typed in the previous client must never be written into the new one.
        let commits_on_blur = !focused
            && scheme_type(self.cached.scheme).is_some_and(SchemeType::commits_on_blur)
            && !self.cached.dedicated_english
            && self.cached.local_mode == "none"
            && !self.cached.editing_text.is_empty();
        let result = if commits_on_blur {
            self.engine.finish(0)
        } else {
            self.discard_composition()
        };
        self.refresh()?;
        let mut result = result?;
        // Leaving the client cancels the composition, but a phrase piece being held back is text
        // the user chose and, before it was held back, would already be in the document. Send it.
        self.hold_phrase_progress(false, false, false, "", &mut result);
        self.focused = focused;
        // A different client is a different sentence, so context never leaks
        // from one application into another. The Engine's committed-word context follows the same rule: the next word
        // no longer follows the last one, and no later pick may take back what a commit in the other client taught.
        self.ai_context.clear();
        self.engine.set_rescoring_context("");
        self.engine.reset_context();
        Ok(self.transition(result))
    }

    /// 丢弃组字。Cancel 对应用户按 Esc，有些方案里第一次 Cancel 会保留组字：开着可打开的候选列表（韩文汉字列表）时只关闭列表，越南文单词和藏文音节则退回原始按键。这时再发一次 Cancel 才把组字也丢掉。
    fn discard_composition(&mut self) -> Result<EngineResult, RuntimeError> {
        let result = self.engine.command(Command::Cancel)?;
        if scheme_type(self.cached.scheme).is_some_and(SchemeType::cancel_keeps_composition)
            && !self.engine.snapshot()?.editing_text.is_empty()
        {
            return self.engine.command(Command::Cancel);
        }
        Ok(result)
    }

    fn punctuation(&mut self, value: u8) -> Result<EngineResult, RuntimeError> {
        // A symbol the Engine spells with (an operator in the expression mode) or opens a mode with (`/` with nothing composed) is input, whichever route the host chose for the key: finishing first would commit the half-typed spelling. A phrase still being held is a composition too, and the mark has to end it rather than open a mode after it.
        if self.cached.spelling_symbols.as_bytes().contains(&value)
            && (self.cached.local_mode != "none" || self.phrase_prefix.is_empty())
        {
            return self.engine.character(value, false);
        }
        // The apostrophe is not a spelling symbol, but right after a unit (`3jin'g`) or a `/fy` word it separates rather than ends.
        if value == b'\'' && self.engine.takes_local_separator() {
            return self.engine.character(value, false);
        }
        // A mark on a bare `/` or `@` ends the mode as punctuation; finishing first would commit the first row.
        if self.bare_mode_prefix() {
            return self.engine.punctuation(value);
        }
        // Finish through Engine with the host highlight BEFORE asking it to translate.
        // Calling Engine punctuation on an active composition would choose candidate zero.
        let mut finished = self.engine.finish(self.engine_index(self.highlighted))?;
        let punctuation = match self.engine.punctuation(value) {
            Ok(result) => result,
            Err(error) if finished.has_commit => {
                // Completion already changed Engine state: never discard that commit.
                finished.commit.push(char::from(value));
                finished.handled = true;
                finished.diagnostic =
                    format!("{} Punctuation failed: {error}", finished.diagnostic)
                        .trim()
                        .to_owned();
                return Ok(finished);
            }
            Err(error) => return Err(error),
        };
        if !finished.has_commit {
            if !punctuation.handled && !self.phrase_prefix.is_empty() {
                // The held phrase piece goes out ahead of the mark (`hold_phrase_progress`), which marks the key handled, so a mark with no Chinese form has to go out with it rather than be left to the host.
                return Ok(literal_mark(value, punctuation.diagnostic));
            }
            return Ok(punctuation);
        }
        finished.handled = true;
        if punctuation.has_commit {
            finished.commit.push_str(&punctuation.commit);
        } else if !punctuation.handled {
            // ASCII mode/unsupported symbols still terminate composition in one commit.
            finished.commit.push(char::from(value));
        }
        if !punctuation.diagnostic.is_empty() {
            finished.diagnostic = format!("{} {}", finished.diagnostic, punctuation.diagnostic)
                .trim()
                .to_owned();
        }
        Ok(finished)
    }

    /// A `/` or `@` mode holding nothing but its prefix.
    fn bare_mode_prefix(&self) -> bool {
        matches!(self.cached.local_mode.as_str(), "command" | "mention")
            && self.cached.editing_text.len() == 1
    }

    fn punctuation_ascii(&mut self, value: u8) -> Result<EngineResult, RuntimeError> {
        // A spelling symbol is input, as on the punctuation route: it extends the local mode in progress, and a scheme that spells with marks (Zhuyin's bopomofo keys) takes them whenever it lists them. The symbols that open a mode with nothing composed are the exception: there the host asked for the literal mark after weighing the surrounding text (a `/` after a digit), so it never opens a mode.
        let spells = self.cached.local_mode != "none"
            || (self.phrase_prefix.is_empty()
                && scheme_type(self.cached.scheme)
                    .is_some_and(|scheme| !scheme.opens_local_modes()));
        // 字面标点路由刻意不进入网址模式：组字 `www` 时引擎在 `spelling_symbols` 里列出 `.`，但宿主在这条路由上要的是字面符号，所以这里不收，照常结束组字再接上 `.`（列出但不接受的例外）。
        if spells && self.cached.spelling_symbols.as_bytes().contains(&value) {
            return self.engine.character(value, false);
        }
        if value == b'\'' && self.engine.takes_local_separator() {
            return self.engine.character(value, false);
        }
        // Keep the same highlighted-candidate completion semantics as normal
        // punctuation, but do not ask Engine to translate the trailing mark.
        // The Linux host has already applied its surrounding-text policy.
        // A bare `/` or `@` commits as the literal prefix rather than its first row.
        let mut finished = if self.bare_mode_prefix() {
            self.engine.command(Command::CommitRaw)?
        } else {
            self.engine.finish(self.engine_index(self.highlighted))?
        };
        if !finished.has_commit {
            // As in `punctuation`: a held phrase piece takes the key, so the mark goes out with it.
            if !self.phrase_prefix.is_empty() {
                return Ok(literal_mark(value, finished.diagnostic));
            }
            return Ok(finished);
        }
        finished.handled = true;
        finished.commit.push(char::from(value));
        Ok(finished)
    }

    pub fn dispatch(&mut self, action: Action) -> Result<Transition, RuntimeError> {
        if matches!(&action, Action::Punctuation(value) | Action::PunctuationAscii(value) if !value.is_ascii_punctuation())
        {
            return Err(RuntimeError::InvalidPunctuation);
        }
        // Cache maintenance belongs to the session, including while its host
        // has no focus. Ordinary input must still pass through unchanged.
        if !self.focused && !matches!(&action, Action::ResetCache) {
            return Ok(self.transition(empty_result(false)));
        }
        if let Action::SelectAnyCandidate(id) = &action {
            if id.session != self.session
                || id.generation != self.generation
                || id.index >= self.cached.candidates.len()
            {
                return Err(RuntimeError::StaleCandidate);
            }
        }
        if let Action::Select(id)
        | Action::SelectEdge(id, _)
        | Action::PinCandidate(id)
        | Action::RemoveCandidate(id)
        | Action::FixCandidatePosition(id, _)
        | Action::ClearCandidatePosition(id) = &action
        {
            let start = (self.highlighted / self.page_size) * self.page_size;
            if id.session != self.session
                || id.generation != self.generation
                || id.index < start
                || id.index >= (start + self.page_size).min(self.cached.candidates.len())
            {
                return Err(RuntimeError::StaleCandidate);
            }
        }
        if let Action::ChooseNineKeySpelling(id) = &action {
            if id.session != self.session
                || id.generation != self.generation
                || !self.cached.nine_key
                || id.index >= self.cached.nine_key_spellings.len()
            {
                return Err(RuntimeError::StaleNineKeySpelling);
            }
        }
        self.advance()?;
        let filled_current_page =
            matches!(action, Action::NextPage) && self.expand_for_next_page()?;
        if matches!(action, Action::NextCandidate) {
            self.expand_for_next_candidate()?;
        }
        // The last candidate means the last one there is. The Engine caps what it returns to a
        // single-letter query and hands the rest over on request, so without this End would stop at
        // the end of what happened to be cached and move again the next time it was pressed.
        if matches!(action, Action::LastCandidate) {
            self.expand_cached_candidates()?;
        }
        let len = self.cached.candidates.len();
        let next_highlight = match &action {
            // Staying keeps the highlight exactly where it was: the page did not change, it only
            // stopped being short.
            Action::NextPage if filled_current_page => Some(self.highlighted),
            Action::NextPage if len > 0 => Some(
                (self.highlighted / self.page_size + 1).min((len - 1) / self.page_size)
                    * self.page_size,
            ),
            Action::PreviousPage if len > 0 => {
                Some((self.highlighted / self.page_size).saturating_sub(1) * self.page_size)
            }
            Action::NextCandidate if len > 0 => Some((self.highlighted + 1).min(len - 1)),
            Action::PreviousCandidate if len > 0 => Some(self.highlighted.saturating_sub(1)),
            // The ends of the list, not the ends of the page. The reference's Home and End are
            // FUNCTION_MOVE_PAGE_TOP and FUNCTION_MOVE_PAGE_BOTTOM, and its presenter answers both
            // with SetSelection - index 0, or -1 read as Count() - 1 - which then pulls the page
            // along to wherever that candidate sits. Every host here routes its own Home and End to
            // this action, so all four used to stop at the edges of the page the user was already
            // looking at, which is a keystroke that does almost nothing.
            Action::FirstCandidate if len > 0 => Some(0),
            Action::LastCandidate if len > 0 => Some(len - 1),
            _ => None,
        };
        if let Some(index) = next_highlight {
            self.highlighted = index;
            return Ok(self.transition(empty_result(true)));
        }
        // Going back into the phrase, before the Engine sees the key: both rules replace what the
        // key would otherwise do.
        if let Some(transition) = self.retreat_phrase_selection(&action)? {
            return Ok(transition);
        }
        // What the reading held before the Engine saw this key. A selection that consumes part of
        // it has to record the piece it took, and only the difference says what that was. Digits
        // may become a selection after the Engine sees them, so all Character actions stay in the
        // set; commands, punctuation and navigation never consume a candidate reading.
        let selection_action = matches!(
            &action,
            Action::Character { .. }
                | Action::Select(_)
                | Action::SelectAnyCandidate(_)
                | Action::SelectEdge(..)
                | Action::SelectHighlighted
        );
        let reading_before =
            (self.phrase_preedit && selection_action).then(|| self.cached.editing_text.clone());
        // A digit on the candidate page picks a candidate; the Engine is asked the same question as
        // for Select, so it can begin a phrase the same way.
        let mut selected_by_digit = false;
        let character_action = matches!(action, Action::Character { .. });
        // Wubi top-commit (顶字): a letter typed after a complete four-letter code the Wubi table
        // answered, unique or not, commits the first candidate and starts the next composition
        // with that letter. The Engine caps a native Wubi code at four letters and would drop the
        // fifth, so without this the user loses the key they typed. It is judged on the reading
        // before the key, with the caret at its end: a caret moved back into the code is an edit
        // of the code, not the start of the next character. A held phrase stays open, matching
        // the reference's creating-word guard.
        let wubi_top_commit = matches!(action, Action::Character { value, .. } if value.is_ascii_alphabetic())
            && self.snapshot_valid
            && self.phrase_prefix.is_empty()
            && wubi_four_code_is_complete(&self.cached);
        let result = match action {
            Action::ResetCache => {
                self.engine.reset_cache()?;
                Ok(EngineResult {
                    handled: true,
                    has_commit: false,
                    commit: String::new(),
                    diagnostic: String::new(),
                })
            }
            Action::Punctuation(value) => self.punctuation(value),
            Action::PunctuationAscii(value) => self.punctuation_ascii(value),
            // A bare `/` or `@` flushes as the literal prefix, as on the punctuation routes; finishing would commit the list's first row.
            Action::Finish if self.bare_mode_prefix() => self.engine.command(Command::CommitRaw),
            Action::Finish => self.engine.finish(self.engine_index(self.highlighted)),
            Action::Character { value, shift } if wubi_top_commit => self
                .engine
                .select(self.engine_index(0))
                .and_then(|committed| {
                    self.engine.character(value, shift)?;
                    Ok(committed)
                }),
            // A symbol that would open a mode behind a held phrase piece ends the phrase as punctuation instead, as on the punctuation route.
            Action::Character { value, .. }
                if !self.phrase_prefix.is_empty()
                    && self.cached.local_mode == "none"
                    && self.cached.spelling_symbols.as_bytes().contains(&value) =>
            {
                self.punctuation(value)
            }
            Action::Character { value, shift } => {
                self.engine.character(value, shift).and_then(|result| {
                    // The nine-key separator is a layout action, not Chinese quote punctuation.
                    if !result.handled && self.cached.nine_key && value == b'\'' {
                        return Ok(result);
                    }
                    if !result.handled && value.is_ascii_punctuation() {
                        return self.punctuation(value);
                    }
                    // Space the Engine let go over an open list, with nothing committed, is a pick of the highlighted row, as the Space command is (Zhuyin leaves it to the runtime; Korean commits its syllable first and so is not a pick).
                    if !result.handled
                        && !result.has_commit
                        && value == b' '
                        && self.cached.candidate_list_open
                        && len > 0
                    {
                        return self.engine.select(self.engine_index(self.highlighted));
                    }
                    // Let Engine consume numeric input (Unicode mode, nine-key, etc.) first. A result that already committed (a Korean syllable the digit ended) is final: selecting now would replace that commit and lose the text. A digit the scheme spells with (a Zhuyin tone or phonetic key) is never a pick, even one the Engine let go: Zhuyin leaves 1-9 to selection only while its list is open, when they are not spelling symbols, so `0` there stays ㄢ.
                    if result.handled
                        || result.has_commit
                        || (self.cached.local_mode == "none"
                            && self.cached.spelling_symbols.as_bytes().contains(&value))
                        || self.cached.nine_key
                        || !(b'1'..=b'9').contains(&value)
                        || len == 0
                    {
                        return Ok(result);
                    }
                    let page_start = (self.highlighted / self.page_size) * self.page_size;
                    let slot = usize::from(value - b'1');
                    if slot >= self.page_size || page_start + slot >= len {
                        return Ok(empty_result(true));
                    }
                    selected_by_digit = true;
                    self.engine.select(self.engine_index(page_start + slot))
                })
            }
            Action::Command(command) => self.engine.command(command),
            Action::SegmentBackspace => self.engine.segment_command(SegmentCommand::Backspace),
            Action::SegmentMoveLeft => self.engine.segment_command(SegmentCommand::MoveLeft),
            Action::SegmentMoveRight => self.engine.segment_command(SegmentCommand::MoveRight),
            Action::Select(id) => self.engine.select(self.engine_index(id.index)),
            Action::SelectAnyCandidate(id) => self.engine.select(self.engine_index(id.index)),
            Action::SelectEdge(id, edge) => {
                self.engine.select_edge(self.engine_index(id.index), edge)
            }
            Action::PinCandidate(id) => self.engine.pin_candidate(self.engine_index(id.index)),
            Action::RemoveCandidate(id) => {
                self.engine.remove_candidate(self.engine_index(id.index))
            }
            Action::FixCandidatePosition(id, position) => {
                if !(1..=5).contains(&position) {
                    return Err(RuntimeError::Engine(
                        "Candidate position must be between 1 and 5".into(),
                    ));
                }
                self.engine
                    .fix_candidate_position(self.engine_index(id.index), position)
            }
            Action::ClearCandidatePosition(id) => self
                .engine
                .clear_candidate_position(self.engine_index(id.index)),
            Action::ChooseNineKeySpelling(id) => self.engine.choose_nine_key_spelling(id.index),
            Action::Glide {
                ref keyboard,
                ref points,
            } => self.engine.glide(keyboard, points),
            // A scheme that spells with Space lists it among its spelling symbols (Zhuyin's first tone, or opening its list with no syllable pending), and then the Space command is that key rather than a pick of the highlighted row.
            Action::SelectHighlighted
                if self.cached.spelling_symbols.as_bytes().contains(&b' ') =>
            {
                self.engine.character(b' ', false)
            }
            Action::SelectHighlighted if len > 0 => {
                self.engine.select(self.engine_index(self.highlighted))
            }
            Action::SelectHighlighted => self.engine.command(Command::CommitCandidate),
            _ => return Ok(self.transition(empty_result(false))),
        };
        // Keep the pre-refresh mode only when this action can produce a commit. Most keystrokes
        // leave the composition open, so copying local_mode for them is wasted work. A held phrase
        // and the automatic Wubi top-commit can produce a commit after the Engine result itself
        // says otherwise.
        let needs_commit_context = result.as_ref().is_ok_and(|result| result.has_commit)
            || !self.phrase_prefix.is_empty()
            || (character_action
                && self.snapshot_valid
                && self.cached.wubi_unique_four_code
                && self.phrase_prefix.is_empty());
        let commit_context = needs_commit_context.then(|| self.output_context());
        let refresh = self.refresh();
        let mut result = result?;
        if let Err(error) = refresh {
            // A successful engine commit must survive a presentation refresh failure.
            result.diagnostic = format!("Candidate refresh failed: {error}");
        }
        // The Engine owns the definition of a complete, native, unique Wubi code. Every host gets
        // the same fourth-key behavior here; platform adapters only decide how that commit crosses
        // their native composition boundary. A held phrase is still being assembled and must stay
        // open, matching the reference's creating-word guard.
        if character_action
            && self.snapshot_valid
            && self.cached.wubi_unique_four_code
            && self.phrase_prefix.is_empty()
        {
            result = self.engine.select(self.engine_index(0))?;
            if let Err(error) = self.refresh() {
                result.diagnostic = format!("Candidate refresh failed: {error}");
            }
        }
        // The Engine takes what it used off the front of the reading, so what is gone from the
        // front is what the selection consumed. A reading that did not simply shrink - a special
        // mode rewriting it, a fallback replacing it - leaves nothing to restore, and that
        // selection is recorded as unretractable rather than guessed at.
        let consumed = reading_before
            .as_deref()
            .and_then(|reading| reading.strip_suffix(self.cached.editing_text.as_str()))
            .unwrap_or("");
        let picked = selected_by_digit || selection_action;
        // Escape throws the whole composition away, the chosen pieces with it - the reference's
        // _HandleCancel clears `word_for_creating_word` in the same breath.
        let discarded = matches!(action, Action::Command(Command::Cancel));
        // Segment editing on an emptied reading leaves the held phrase for the next segment key or Backspace, instead of sending it to the document (the reference's `keep_creating_word_after_empty_raw`).
        let keep_empty = matches!(
            action,
            Action::SegmentBackspace | Action::SegmentMoveLeft | Action::SegmentMoveRight
        );
        self.hold_phrase_progress(picked, discarded, keep_empty, consumed, &mut result);
        let mut transition = self.transition(result);
        if let Some(commit_context) = commit_context {
            if transition.commit.is_some() {
                transition.commit_context = Some(commit_context);
            }
        }
        Ok(transition)
    }
}

/// `value` committed as it is, for a mark that has to leave together with a held phrase piece.
fn literal_mark(value: u8, diagnostic: String) -> EngineResult {
    EngineResult {
        handled: true,
        has_commit: true,
        commit: char::from(value).to_string(),
        diagnostic,
    }
}

pub(crate) fn empty_result(handled: bool) -> EngineResult {
    EngineResult {
        handled,
        has_commit: false,
        commit: String::new(),
        diagnostic: String::new(),
    }
}

/// The reference Engine's `wubi_four_code_is_complete`, read off the snapshot this Engine already
/// publishes: the guards of `wubi_unique_four_code` without the candidate count. The code is a
/// native Wubi one (not dedicated English, no local mode, not answered by the pinyin fallback), it
/// is exactly the four letters the Wubi scheme caps a table-answered code at, the caret is at its
/// end, and there is a candidate to commit: a four-letter spelling no row matched was not answered
/// by the table, and committing nothing would still drop the key.
pub(crate) fn wubi_four_code_is_complete(snapshot: &EngineSnapshot) -> bool {
    const WUBI_COMPLETE_CODE_LENGTH: usize = 4;
    scheme_type(snapshot.scheme) == Some(SchemeType::Wubi)
        && !snapshot.dedicated_english
        && snapshot.local_mode == "none"
        && !snapshot.nine_key
        && !snapshot.answered_by_pinyin_fallback
        && snapshot.editing_text.len() == WUBI_COMPLETE_CODE_LENGTH
        && msime_client_core::is_ascii_alphabetic(&snapshot.editing_text)
        && snapshot.caret_position == WUBI_COMPLETE_CODE_LENGTH
        && !snapshot.candidates.is_empty()
}
