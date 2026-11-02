//! The public `Session` (`include/metasequoia/session.h`, core-session.md §4): an input session plus the nine-key session, with the routing rules between them. One host serialises calls to its session; distinct sessions may run concurrently.
//!
//! The setters the product only sets at construction (`switch_scheme`, `set_helpcode_schema`, `set_helpcode_enabled`, `set_wubi_mixed_pinyin`, `set_personal_context_enabled`) and `candidate_key` stay public because the golden scenarios drive them. Learning undo (`set_learning_undo_enabled`, `forget_recent_commits`) is dropped.

mod candidates;
mod caret_edit;
mod chain;
mod clock;
mod commit;
mod composition;
mod conversion;
mod editing;
mod glide;
mod helpcode_entry;
mod input;
mod learning;
mod literal;
mod online;
pub mod options;
mod plugin_commands;
mod prediction;
mod raw_commit;
#[cfg(test)]
mod tests;

use std::path::PathBuf;
use std::sync::Arc;

use crate::diagnostics;
use crate::error::{EngineError, Result};
use crate::local::database::LocalDatabaseLease;
use crate::nine_key::{KeyGrid, NineKeySession};
use crate::ordering::CandidateScorer;
use crate::pinyin::glide::{GlideKeyboard, GlidePoint};
use crate::touch::{KeyGeometry, KeyTouch, TouchTuning};
use crate::types::{
    CandidateEdge, CandidateSource, Command, CommandTableEntry, CommandTranslationQuery,
    FrequencyAdjustmentOptions, KeyResult, LocalInputMode, MentionEntry, OnlineQuery,
    PluginCommand, PluginCommandQuery, QuickPhraseEntry, SchemeType, WordItem,
};
use crate::user_dictionary::journal::{release_thread_journal, JournalUse};
use crate::user_dictionary::ngram_store::{flush_journal, note_settled};

use crate::types::FirstPageRows;
pub use clock::{Clock, LocalClock};
use input::InputSession;
pub use options::{SessionOptions, SessionSnapshot};

pub struct Session {
    input: InputSession,
    nine_key: NineKeySession,
    /// 触屏的组码网格：九键的数字从 `character` 进来，14 键的字母从 `grid_key` 进来；`None` 是不用网格。
    key_grid: Option<KeyGrid>,
    shuangpin_preedit_uses_raw: bool,
    /// `SessionOptions::learn_typed_spellings_on_enter`。
    learn_typed_spellings_on_enter: bool,
    prediction: prediction::Prediction,
    /// See [`Self::candidate_revision`].
    revision: u64,
    /// `msime_user.db` of this session's paths, written once more as the session ends.
    journal: PathBuf,
    journal_use: JournalUse,
    /// The generation and resource directories the local-mode queries read; dropped after the rest of the session, so the last session to go closes their cached connections.
    _local_databases: LocalDatabaseLease,
}

// A session may move to another thread between calls (a host's worker pool, the C ABI); calls into one session are still serialised by its owner, so it does not need to be `Sync`. The journal connections it opens are cached per thread, and its end closes the idle ones on every thread (`JournalUse`), so a session used on several threads leaves none open.
const _: fn() = || {
    fn moves_between_threads<T: Send>() {}
    moves_between_threads::<Session>();
};

impl Session {
    /// Validates the paths and every option (`INVALID_SESSION_OPTIONS`, `UNKNOWN_HELPCODE_SCHEMA`), loads the helpcode table and applies the options in the reference order (session.cpp:11-30, 57-66).
    pub fn new(options: SessionOptions) -> Result<Session> {
        options.paths.validate()?;
        crate::user_dictionary::generation::check_read_only_layout(&options.paths)?;
        let input = InputSession::new(&options)?;
        let mut nine_key = NineKeySession::new(
            &options.paths,
            options.learning,
            options.frequency,
            options.fuzzy_pinyin,
            options.english,
            options.enabled_schemes.contains(SchemeType::Quanpin),
            options.single_character_only,
        );
        nine_key.set_stroke_dictionary(options.stroke_dictionary.clone());
        nine_key.set_mixed_expressive(options.expressive);
        nine_key.set_mixed_candidates_enabled(options.mixed_candidates);
        nine_key.set_inline_date_time(
            options
                .local_modes
                .date_time_inline
                .then(|| Arc::clone(&input.clock.local)),
        );
        // 九键的整句与 26 键遵守同一组句子联想设置（#6059）。
        nine_key.set_sentence_options(
            options.sentence_association.clone(),
            options.sentence_alternatives,
        );
        nine_key.set_first_page_rows(FirstPageRows::new(options.first_page_rows));
        nine_key.set_rescoring_context(&options.rescoring_context);
        // 九键选中的词记进同一个个人上下文模型，开关与 26 键相同（#6185）。
        nine_key.set_personal_context_enabled(options.personal_context);
        let journal = options.paths.user(crate::assets::USER_JOURNAL);
        Ok(Session {
            input,
            nine_key,
            key_grid: None,
            shuangpin_preedit_uses_raw: options.shuangpin_preedit_uses_raw,
            learn_typed_spellings_on_enter: options.learn_typed_spellings_on_enter,
            prediction: prediction::Prediction::new(
                options.next_word_prediction,
                options.paths.dictionary(crate::assets::MAIN_DICTIONARY),
            ),
            revision: 0,
            journal_use: JournalUse::new(journal.clone()),
            journal,
            _local_databases: LocalDatabaseLease::new([
                options.paths.dictionaries.clone(),
                options.paths.resources.clone(),
            ]),
        })
    }

    /// Changes whenever a call may have changed the candidate list, so a host that read the candidates under one revision can tell that an index it holds no longer means the same row. Read-only calls never change it; neither do the setters that only affect later keys (clock, key touch and geometry, touch tuning, punctuation options, rescoring context). Starts at 0 and only grows.
    pub fn candidate_revision(&self) -> u64 {
        self.revision
    }

    fn candidates_may_change(&mut self) {
        self.revision = self.revision.wrapping_add(1);
    }

    /// 会话建立时定下的双拼方案（内置的表，或校验过的用户表）。
    pub(crate) fn shuangpin_layout(&self) -> &'static crate::shuangpin::ShuangpinProfile {
        self.input.profile
    }

    /// 会话现在过滤候选用的辅助码表（`SessionOptions::helpcode_table`、按 `helpcode_schema` 读的表，或之后 `set_helpcode_table` 换上的表）；辅助码关着时照样留着。门面给自己合成的整句补注释时读它，不另读一份。
    pub(crate) fn helpcode_table(&self) -> Option<&crate::helpcode::HelpcodeKeymap> {
        self.input.helpcode_keymap.as_deref()
    }

    /// Replace the steady and local clocks, so tests can cross the 3 s / 8 s / 10 s personal-learning windows and the date/time mode can be pinned.
    pub fn set_clock(&mut self, clock: Clock) {
        if self.input.local_mode_options.date_time_inline {
            self.nine_key
                .set_inline_date_time(Some(Arc::clone(&clock.local)));
        }
        self.input.clock = clock;
    }

    /// One ASCII character; `shift_only` is a bare Shift+letter (local mode entry). 全拼下九键开启且没有别的组字时，数字 2-9 交给九宫格会话；九键双拼交的是方案用到的数字（可以有 1 和 0）；注音九键的数字由注音编辑器自己处理。待用的触点（[`Self::set_key_touch`]）只交给 26 键组字，九宫格的数字用不到它，一律在这一键后作废。
    pub fn character(&mut self, value: u8, shift_only: bool) -> KeyResult {
        self.candidates_may_change();
        self.prediction.clear();
        let touch = self.input.pending_touch.take();
        // English is a mode rather than a scheme, so the grid stays available in it: the same digits spell words instead of syllables. A local mode still takes the keys, and the scheme underneath must be quanpin: 拼音九宫格只认得全拼音节，注音九键走注音编辑器。九键双拼没有英文九键，专用英文里数字照常是数字。
        let grid_digit = match self.key_grid {
            Some(KeyGrid::NineKey) => (b'2'..=b'9').contains(&value),
            Some(grid @ KeyGrid::NineKeyShuangpin(_)) => {
                !self.input.dedicated_english && grid.is_code(value)
            }
            _ => false,
        };
        if grid_digit
            && self.input.scheme() == SchemeType::Quanpin
            && self.input.local_mode == LocalInputMode::None
            && self.input.preedit().is_empty()
        {
            let result = self.nine_key.character(value);
            return self.after_nine_key(result);
        }
        if self.nine_key.active() {
            // `'` splits the grid's syllables where the user is typing; any other key is still the host's to handle.
            if value == b'\'' {
                let result = self.nine_key.character(value);
                return self.after_nine_key(result);
            }
            return KeyResult::unhandled();
        }
        self.input.pending_touch = touch;
        self.input.handle_character(value, shift_only)
    }

    /// 下一次 `character` 的按下点（触屏宿主经 `msime_client_touch_character` 送来）。只有那一键是 26 键双拼的小写字母或 `;` 时才交给方案；其余会改动组字或改道按键的入口（命令、选词、以词定字、结束组字、钉住和删除候选、移光标、九宫格、网格键、滑行、标点、换方案、换网格或几何、展开首字母候选、重置上下文）一开头就丢掉它，所以改道到别处的按键不会把触点留给后面的字母。只改配置的 setter（辅助码表、云候选开关、重排上下文等）不碰它。
    pub fn set_key_touch(&mut self, touch: Option<KeyTouch>) {
        self.input.pending_touch = touch;
    }

    /// 宿主注册或清除的键位几何，见 [`crate::touch::KeyGeometry`]。
    pub fn set_key_geometry(&mut self, geometry: Option<Arc<KeyGeometry>>) {
        // 待用触点是按旧几何量出来的，交给新几何解释没有意义。
        self.input.pending_touch = None;
        self.input.engine.set_key_geometry(geometry);
    }

    /// 按触点计价的参数；只有 Rust 侧（评测示例）调用，不经 FFI。
    pub fn set_touch_tuning(&mut self, tuning: TouchTuning) {
        self.input.engine.set_touch_tuning(tuning);
    }

    /// 滑过字母键的一笔：它最可能拼出的全拼字母写到光标处，与已有组字之间用 `'` 隔开。不是全拼、在本地模式或专用英文里、九键数字正在组字，以及没有串跟得上这一笔时，不处理。
    pub fn glide(&mut self, keyboard: &GlideKeyboard, points: &[GlidePoint]) -> KeyResult {
        self.candidates_may_change();
        self.prediction.clear();
        self.input.pending_touch = None;
        if self.nine_key.active() {
            return KeyResult::unhandled();
        }
        self.input.glide(keyboard, points)
    }

    /// 上一次 [`Session::set_key_grid`]（或 [`Session::set_nine_key_enabled`]）设下的组码网格，`None` 是不用网格；新会话是 `None`。网格开着时硬件键盘的字母照样走 26 键，要知道这次组字是不是在网格里打的，看 [`Session::grid_composing`]。
    pub fn key_grid(&self) -> Option<KeyGrid> {
        self.key_grid
    }

    /// 字母网格正在组字：九宫格会话（九键、14 键、18 键）在组字，快照、命令和上屏都来自它；或者五笔 14 键在组字，组码不是确定的字母。两种情况下的原样上屏都不是用户逐个打出的字母。
    pub fn grid_composing(&self) -> bool {
        self.nine_key.active() || self.input.wubi_grid_composing()
    }

    /// `set_key_grid` 的九键开关：开是九键，关是不用网格。
    pub fn set_nine_key_enabled(&mut self, enabled: bool) {
        self.set_key_grid(enabled.then_some(KeyGrid::NineKey));
    }

    /// 换触屏的组码网格，先丢掉网格里的组字，所以在换键盘布局、没有组字时调用。英文九键和注音九键只在九键下打开（注音组字会被丢掉）：14 键的英文模式画全键盘，注音没有 14 键。
    pub fn set_key_grid(&mut self, grid: Option<KeyGrid>) {
        self.candidates_may_change();
        self.prediction.clear();
        self.input.pending_touch = None;
        self.input.helpcode_entry = None;
        self.nine_key.command(Command::Cancel);
        if let Some(grid) = grid {
            self.nine_key.set_grid(grid);
        }
        self.key_grid = grid;
        let nine_key = grid == Some(KeyGrid::NineKey);
        self.nine_key
            .set_english_only(nine_key && self.input.dedicated_english);
        self.input.set_zhuyin_nine_key(nine_key);
    }

    /// 字母网格的一键：小写字母由 `KeyGrid::code_of` 归成整组，十八键双拼另收大写 ASCII 标记精确字母。标记保留在组码里，不复用硬件键盘的 `character`。全拼和双拼分别使用自己的分组表，五笔在 14 键下按组展开码表（`wubi::grid`）；本地模式、专用英文或已有 26 键组字时不处理，状态不变。
    pub fn grid_key(&mut self, letter: u8) -> KeyResult {
        self.candidates_may_change();
        self.prediction.clear();
        self.input.pending_touch = None;
        let Some(
            grid @ (KeyGrid::FourteenKey
            | KeyGrid::EighteenKey
            | KeyGrid::FourteenKeyShuangpin(_)
            | KeyGrid::EighteenKeyShuangpin(_)),
        ) = self.key_grid
        else {
            return KeyResult::unhandled();
        };
        let Some(code) = grid.code_of(letter) else {
            return KeyResult::unhandled();
        };
        // 五笔 14 键：键面与全拼 14 键相同，组码交给五笔方案按码表展开，不经九宫格会话。
        if grid == KeyGrid::FourteenKey && self.input.scheme() == SchemeType::Wubi {
            let Some(representative) = crate::wubi::grid::representative(code) else {
                return KeyResult::unhandled();
            };
            return self.input.handle_wubi_grid_key(representative);
        }
        let supported = match grid {
            KeyGrid::FourteenKey | KeyGrid::EighteenKey => {
                self.input.scheme() == SchemeType::Quanpin
            }
            KeyGrid::FourteenKeyShuangpin(_) | KeyGrid::EighteenKeyShuangpin(_) => {
                self.input.scheme() == SchemeType::Shuangpin
            }
            _ => false,
        };
        if !supported
            || self.input.local_mode != LocalInputMode::None
            || self.input.dedicated_english
            || !(self.input.preedit().is_empty() || self.nine_key.active())
        {
            return KeyResult::unhandled();
        }
        let result = self.nine_key.character(code);
        self.after_nine_key(result)
    }

    /// 九宫格组码已经到上限时，这一键能不能先选掉首选、再接着打（`NineKeySession::limit_pick_ready`）。`grid_key` 为真时 `value` 是 `grid_key` 收到的归组字母，否则是 `character` 收到的九键或九键双拼数字；两条路由各自只认自己的网格。
    pub fn grid_limit_pick_ready(&self, value: u8, grid_key: bool) -> bool {
        let code = match self.key_grid {
            Some(
                grid @ (KeyGrid::FourteenKey
                | KeyGrid::EighteenKey
                | KeyGrid::FourteenKeyShuangpin(_)
                | KeyGrid::EighteenKeyShuangpin(_)),
            ) if grid_key => grid.code_of(value),
            Some(KeyGrid::NineKey | KeyGrid::NineKeyShuangpin(_)) if !grid_key => Some(value),
            _ => None,
        };
        code.is_some_and(|code| self.nine_key.limit_pick_ready(code))
    }

    /// 九宫格会话活跃时选它的拼写；否则在注音里钉目标音节的读音。
    pub fn choose_nine_key_spelling(&mut self, index: usize) -> KeyResult {
        self.candidates_may_change();
        self.input.pending_touch = None;
        if !self.nine_key.active() && self.input.zhuyin_rules_apply() {
            return self.input.choose_zhuyin_spelling(index);
        }
        let result = self.nine_key.choose_spelling(index);
        self.after_nine_key(result)
    }

    /// 九宫格组字时按单字、按笔画筛选候选（`NineKeySession::set_filter`）；没有九宫格组字时不处理。
    pub fn set_nine_key_filter(&mut self, single_character: bool, strokes: &str) -> KeyResult {
        self.candidates_may_change();
        self.input.pending_touch = None;
        if !self.nine_key.active() {
            return KeyResult::unhandled();
        }
        self.nine_key.set_filter(single_character, strokes)
    }

    /// 一条编辑命令。`CommitRaw` 原样上屏，专用英文里打的词学进英文词库（`SessionOptions::learn_english_on_enter`），`learn_typed_spellings_on_enter` 打开时还学临时模式与拼不成音节的拼音；`CommitRawWithoutLearning` 原样上屏、不学英文词。规则见 `session/raw_commit.rs`。
    pub fn command(&mut self, command: Command) -> KeyResult {
        match command {
            Command::CommitRawWithoutLearning => self.commit_raw_without_learning(),
            Command::CommitRaw if self.learn_typed_spellings_on_enter => {
                self.commit_raw_learning_typed_spellings()
            }
            _ => self.dispatch_command(command),
        }
    }

    /// 命令交给九宫格或组字，没有回车学词的策略。`CommitRawWithoutLearning` 不会到这里。
    fn dispatch_command(&mut self, command: Command) -> KeyResult {
        self.candidates_may_change();
        self.input.pending_touch = None;
        if !self.prediction.rows.is_empty() {
            self.prediction.clear();
            return if command == Command::Cancel {
                KeyResult::handled()
            } else {
                self.input.handle_command(command)
            };
        }
        let selected = (self.prediction.enabled && command == Command::CommitCandidate)
            .then(|| self.snapshot().candidates.first().cloned())
            .flatten();
        if self.nine_key.active() {
            // 九宫格没有整句改字，改字的光标键就是它的光标键。
            let command = match command {
                Command::ConversionLeft => Command::MoveLeft,
                Command::ConversionRight => Command::MoveRight,
                other => other,
            };
            let result = self.nine_key.command(command);
            let result = self.after_nine_key(result);
            self.predict_after_commit(selected, &result);
            return result;
        }
        let result = self.input.handle_command(command);
        self.predict_after_commit(selected, &result);
        result
    }

    /// 整句改字的左移，进入改字时从第 `index` 个候选（宿主高亮的那一行）开始；已在改字里时与 `command(ConversionLeft)` 相同。九宫格和进不了改字时按 `MoveLeft` 处理。
    pub fn conversion_left_from(&mut self, index: usize) -> KeyResult {
        self.candidates_may_change();
        self.prediction.clear();
        self.input.pending_touch = None;
        if self.nine_key.active() {
            return self.command(Command::MoveLeft);
        }
        self.input.conversion_left_from(index)
    }

    /// `1`..`9` 选择组字候选；空闲联想不接管数字键。
    pub fn candidate_key(&mut self, value: u8) -> KeyResult {
        self.candidates_may_change();
        self.prediction.clear();
        self.input.pending_touch = None;
        if self.nine_key.active() {
            if !(b'1'..=b'9').contains(&value) {
                return KeyResult::unhandled();
            }
            let result = self.nine_key.select(usize::from(value - b'1'));
            return self.after_nine_key(result);
        }
        self.input.handle_candidate_key(value)
    }

    /// Finishes the composition and appends the Chinese mark; unhandled when Chinese punctuation is off or locked to ASCII, or the key has no mark.
    pub fn punctuation(&mut self, value: u8) -> KeyResult {
        self.candidates_may_change();
        self.prediction.clear();
        self.input.pending_touch = None;
        if !self.nine_key.active() {
            return self.input.handle_punctuation(value);
        }
        let mut result = self.input.handle_punctuation(value);
        if !result.handled {
            return result;
        }
        // The grid's text goes before the mark the input session translated.
        let composition = self.nine_key.finish(0);
        if composition.commit.is_some() {
            self.input.reset_commit_context();
        }
        let mut text = composition.commit.unwrap_or_default();
        text.push_str(result.commit.as_deref().unwrap_or_default());
        result.commit = Some(text);
        result
    }

    /// Live; keeps the composition, caret and pairing state.
    pub fn set_chinese_punctuation_enabled(&mut self, enabled: bool) {
        self.input.chinese_punctuation_enabled = enabled;
    }

    /// `INVALID_PUNCTUATION_LOCK` outside 0..=2.
    pub fn set_punctuation_lock(&mut self, lock: i32) -> Result<()> {
        if !(0..=2).contains(&lock) {
            return Err(EngineError::invalid(diagnostics::INVALID_PUNCTUATION_LOCK));
        }
        self.input.punctuation_lock = lock;
        Ok(())
    }

    pub fn set_paired_punctuation_enabled(&mut self, enabled: bool) {
        self.input.punctuation.set_paired_enabled(enabled);
    }

    /// 换引号和方头括号的样式；组字、光标和交替状态都不动。
    pub fn set_punctuation_marks(&mut self, marks: crate::punctuation::PunctuationMarks) {
        self.input.punctuation.set_marks(marks);
    }

    /// The host emitted the closing half of a paired mark itself.
    pub fn balance_paired_punctuation_after_auto_close(&mut self, opening: u8) {
        self.input.punctuation.balance_after_auto_close(opening);
    }

    pub fn select(&mut self, index: usize) -> KeyResult {
        self.candidates_may_change();
        self.input.pending_touch = None;
        if !self.prediction.rows.is_empty() {
            let Some((row, word, pinyin)) = self.prediction.selected(index) else {
                return KeyResult::unhandled();
            };
            let diagnostic = self.input.record_personal_context(Some(&row), false, false);
            self.show_prediction(&word, &pinyin, &row.word, &row.word);
            return KeyResult::committed(row.word.to_string()).with_diagnostic(diagnostic);
        }
        let selected = self
            .prediction
            .enabled
            .then(|| self.snapshot().candidates.get(index).cloned())
            .flatten();
        if self.nine_key.active() {
            let result = self.nine_key.select(index);
            let result = self.after_nine_key(result);
            self.predict_after_commit(selected, &result);
            return result;
        }
        let result = self.input.select_candidate(index);
        self.predict_after_commit(selected, &result);
        result
    }

    /// Commit the first or last Han character of a candidate and drop the rest of the composition.
    pub fn select_edge(&mut self, index: usize, edge: CandidateEdge) -> KeyResult {
        self.candidates_may_change();
        self.input.pending_touch = None;
        if !self.prediction.rows.is_empty() {
            return KeyResult::unhandled();
        }
        if self.nine_key.active() {
            return KeyResult::unhandled();
        }
        self.input.select_candidate_edge(index, edge)
    }

    /// Promote a dictionary candidate without committing; unsupported rows are unhandled, persistence failures carry a diagnostic.
    pub fn pin(&mut self, index: usize) -> KeyResult {
        self.candidates_may_change();
        self.input.pending_touch = None;
        if !self.prediction.rows.is_empty() {
            return KeyResult::unhandled();
        }
        if self.nine_key.active() {
            return self.nine_key.pin(index);
        }
        self.input.pin_candidate(index)
    }

    /// Remove a dictionary phrase without committing; single-character non-English rows are protected.
    pub fn remove(&mut self, index: usize) -> KeyResult {
        self.candidates_may_change();
        self.input.pending_touch = None;
        if !self.prediction.rows.is_empty() {
            return KeyResult::unhandled();
        }
        if self.nine_key.active() {
            return self.nine_key.remove(index);
        }
        self.input.remove_candidate(index)
    }

    /// Fix a dictionary candidate to slot 1..=5 in this input context; unhandled for any other slot.
    pub fn fix_position(&mut self, index: usize, position: i32) -> KeyResult {
        self.candidates_may_change();
        self.input.pending_touch = None;
        if !self.prediction.rows.is_empty() {
            return KeyResult::unhandled();
        }
        if !(1..=5).contains(&position) {
            return KeyResult::unhandled();
        }
        if self.nine_key.active() {
            return self.nine_key.set_position(index, position);
        }
        self.input.set_candidate_position(index, position)
    }

    pub fn clear_position(&mut self, index: usize) -> KeyResult {
        self.candidates_may_change();
        self.input.pending_touch = None;
        if !self.prediction.rows.is_empty() {
            return KeyResult::unhandled();
        }
        if self.nine_key.active() {
            return self.nine_key.set_position(index, 0);
        }
        self.input.set_candidate_position(index, 0)
    }

    /// 从宿主高亮的候选开始结束整段组字（没有高亮时为 0），剩余片段使用各自首选；索引无效时提交原文。普通拼音中的大写片段由字面候选保留。
    pub fn finish(&mut self, first_index: usize) -> KeyResult {
        self.candidates_may_change();
        self.prediction.clear();
        self.input.pending_touch = None;
        if self.nine_key.active() {
            let result = self.nine_key.finish(first_index);
            return self.after_nine_key(result);
        }
        self.input.finish_composition(first_index)
    }

    /// Discards the composition. Fails, staying in the current scheme with the composition untouched, when the new scheme's dictionary cannot be opened (Cantonese without a usable `msime-cantonese.db`, Stroke without a usable `msime-stroke.db`: `LANGUAGE_DICTIONARY_UNAVAILABLE`, `LANGUAGE_DICTIONARY_VERSION_UNSUPPORTED`).
    pub fn switch_scheme(&mut self, scheme: SchemeType) -> Result<()> {
        self.candidates_may_change();
        self.input.pending_touch = None;
        self.input.switch_scheme(scheme)?;
        self.prediction.clear();
        self.nine_key.command(Command::Cancel);
        Ok(())
    }

    pub fn is_supported_helpcode_schema(schema: &str) -> bool {
        crate::helpcode::is_supported_helpcode_schema(schema)
    }

    /// False for an unsupported schema or a table that cannot be loaded.
    pub fn set_helpcode_schema(&mut self, schema: &str) -> bool {
        self.candidates_may_change();
        self.input.set_helpcode_schema(schema)
    }

    /// 换上宿主给的辅助码表，替换当前的表；全拼和双拼都用它。
    pub fn set_helpcode_table(&mut self, table: crate::helpcode::SharedKeymap) {
        self.candidates_may_change();
        self.input.set_helpcode_table(table);
    }

    /// Quanpin and shuangpin together, as `SessionOptions::helpcode` does.
    pub fn set_helpcode_enabled(&mut self, enabled: bool) {
        self.candidates_may_change();
        self.input.set_helpcode_enabled(enabled);
    }

    /// 只有模式改变时才取消组字；重复设置保留当前输入。
    pub fn set_dedicated_english(&mut self, enabled: bool) {
        self.candidates_may_change();
        self.input.pending_touch = None;
        if self.input.dedicated_english == enabled {
            return;
        }
        self.prediction.clear();
        self.nine_key.command(Command::Cancel);
        self.input.set_dedicated_english_mode(enabled);
        // 九键在英文里用数字拼单词，在中文里拼音节，模式或网格改变时都要同步。
        self.nine_key
            .set_english_only(self.key_grid == Some(KeyGrid::NineKey) && enabled);
    }

    /// 混输候选的读音能继续时，运行时应保留组字，不提前顶字上屏。
    pub fn wubi_pinyin_continues(&self, value: u8, shift: bool) -> bool {
        !shift
            && !self.input.dedicated_english
            && self.input.local_mode == LocalInputMode::None
            && self.input.engine.wubi_pinyin_continues(value)
    }

    pub fn set_wubi_mixed_pinyin(&mut self, enabled: bool) {
        self.candidates_may_change();
        self.input.set_wubi_mixed_pinyin(enabled);
    }

    /// 五笔拼音混输是否打开。
    pub fn wubi_mixed_pinyin(&self) -> bool {
        self.input.engine.wubi_input_options().mixed_pinyin
    }

    /// 宿主的清缓存动作（维护快捷键、iOS 键盘扩展的内存告警）：清掉本会话各方案的候选缓存，并放掉进程级缓存里的日文模型和没有会话在用的个人学习模型，让不再用它们的进程能交还内存；本会话若正持有模型，照常用到会话结束。
    pub fn reset_cache(&mut self) {
        self.candidates_may_change();
        self.prediction.clear();
        self.input.reset_cache();
        crate::japanese::decoder::JapaneseDictionary::release_shared();
        crate::release_idle_personal_models();
    }

    /// Replace the `/` mode's command table; rows it cannot use are dropped. A diagnostic only if the open command list could not be refreshed.
    pub fn set_command_table(&mut self, table: &[CommandTableEntry]) -> Option<String> {
        self.candidates_may_change();
        self.input.set_command_table(table)
    }

    /// 替换脚本插件的 `/` 指令；用不了的被丢弃（`local::command::usable_plugin_commands`），缓存的插件行清掉。只有打开的 `/` 列表刷新失败时才返回诊断。
    pub fn set_plugin_commands(&mut self, commands: &[PluginCommand]) -> Option<String> {
        self.candidates_may_change();
        self.input.set_plugin_commands(commands)
    }

    /// 生效的插件指令里 `trigger` 的标题，见 `CandidateQueries::plugin_command_title`。
    pub fn plugin_command_title(&self, trigger: &str) -> Option<&str> {
        self.input.queries.plugin_command_title(trigger)
    }

    /// 替换 K 模式的宿主短语表；用不了的行被丢弃。只有打开的 K 模式列表刷新失败时才返回诊断。
    pub fn set_quick_phrase_table(&mut self, table: &[QuickPhraseEntry]) -> Option<String> {
        self.candidates_may_change();
        self.input.set_quick_phrase_table(table)
    }

    /// Replace the `@` mode's list; entries it cannot use are dropped.
    pub fn set_mention_entries(&mut self, entries: &[MentionEntry]) -> Option<String> {
        self.candidates_may_change();
        self.input.set_mention_entries(entries)
    }

    /// Turn the embedded places of `@` mode on or off.
    pub fn set_mention_places(&mut self, enabled: bool) -> Option<String> {
        self.candidates_may_change();
        self.input.set_mention_places(enabled)
    }

    /// Forget the committed-word context (focus or application change, host-inserted text).
    pub fn reset_context(&mut self) {
        self.candidates_may_change();
        self.prediction.clear();
        self.input.pending_touch = None;
        self.input.reset_commit_context();
    }

    /// 宿主的即时隐私门禁；关闭时清掉建议，用户偏好仍保留。
    pub fn set_prediction_allowed(&mut self, allowed: bool) {
        self.candidates_may_change();
        self.prediction.set_allowed(allowed);
    }

    /// 宿主挂了候选打分器时打开：上屏后联想的词库续写多取到 32 条，由 [`Self::order_prediction`] 按模型排过之后仍只给 8 条。关着（默认）时联想与不打分的宿主逐行相同。只影响之后的联想，不改正在显示的行。
    pub fn set_prediction_pool(&mut self, enabled: bool) {
        self.prediction.set_pool(enabled);
    }

    /// 按模型给刚上屏之后的联想排序：词库续写按「接在上文后面」的总对数概率重排（[`crate::ordering::prediction_order`]），个人习惯行原样留在前面，最后仍只给 8 条。`before` 是这次上屏之前的文字，刚上屏的那一段由会话自己接上。每次上屏只排一次，之后的调用、没有打开候选池（[`Self::set_prediction_pool`]）或没有联想时什么都不做。返回显示的行变了没有。
    pub fn order_prediction(&mut self, scorer: &mut dyn CandidateScorer, before: &str) -> bool {
        let changed = self.prediction.order(scorer, before);
        if changed {
            self.candidates_may_change();
        }
        changed
    }

    /// Disabling also ends the current context.
    pub fn set_personal_context_enabled(&mut self, enabled: bool) {
        self.candidates_may_change();
        self.prediction.clear();
        self.input.set_personal_context_enabled(enabled);
        self.nine_key.set_personal_context_enabled(enabled);
    }

    /// 当前组字是否预留云候选席位；运行时在修改前据此判断候选身份是否会变化。
    pub fn cloud_candidates_enabled(&self) -> bool {
        self.input.cloud_candidates_enabled
    }

    /// 让宿主把云候选的固定席位交给当前会话；关闭时立即去掉尚未回答的占位行。
    pub fn set_cloud_candidates_enabled(&mut self, enabled: bool) {
        self.candidates_may_change();
        self.input.set_cloud_candidates_enabled(enabled);
    }

    pub fn english_online_candidates_enabled(&self) -> bool {
        self.input.english_online_candidates_enabled
    }

    pub fn set_english_online_candidates_enabled(&mut self, enabled: bool) {
        self.candidates_may_change();
        self.input.set_english_online_candidates_enabled(enabled);
    }

    /// The committed text the neural sentence models condition on, updated without rebuilding the session. Only the last 64 characters matter.
    pub fn set_rescoring_context(&mut self, context: &str) {
        self.input.set_rescoring_context(context);
        self.nine_key.set_rescoring_context(context);
    }

    /// 首页行数只决定词库要不要并入补全词。`Runtime` 建立、换引擎和宿主改每页候选数（`Runtime::set_page_size`）时都调它，因此引擎的首屏门槛和分页始终同源，不必等下一次因别的偏好变化重建引擎。
    pub fn set_first_page_rows(&mut self, rows: FirstPageRows) {
        self.candidates_may_change();
        self.input.set_first_page_rows(rows);
        self.nine_key.set_first_page_rows(rows);
    }

    /// 调频方式（`SessionOptions::frequency`），不重建会话就换掉；26 键和九宫格一起换。从下一次选择起按新方式学，已经学下的排序照旧生效，眼前的候选不重排。`trigger_count` 或 `linear_step` 不在 1..=10 时是 `INVALID_SESSION_OPTIONS`（建会话时同样的检查），什么也不改。
    pub fn set_frequency_adjustment(&mut self, options: FrequencyAdjustmentOptions) -> Result<()> {
        if !input::frequency_options_valid(options) {
            return Err(EngineError::invalid(diagnostics::INVALID_SESSION_OPTIONS));
        }
        self.candidates_may_change();
        self.input.frequency = options;
        self.nine_key.set_frequency(options);
        Ok(())
    }

    /// 当前的调频方式：建会话时给的，或之后 [`Self::set_frequency_adjustment`] 换上的。
    pub fn frequency_adjustment(&self) -> FrequencyAdjustmentOptions {
        self.input.frequency
    }

    /// Hand over the candidates withheld from a single-letter query; whether the list grew.
    pub fn expand_initial_candidates(&mut self) -> bool {
        self.input.pending_touch = None;
        // Nine-key rows come from the spelling session rather than a dictionary query, so nothing is withheld there.
        if self.nine_key.active() {
            return false;
        }
        let grew = self.input.expand_initial_candidates();
        if grew {
            self.candidates_may_change();
        }
        grew
    }

    /// 设置前缀解码的组字光标（overlays.md §7.6）；位置限制在可编辑范围，不能越过本地模式的入口标记。`None` 回到末尾，九键组码时无操作。
    pub fn set_caret(&mut self, caret: Option<usize>) {
        self.candidates_may_change();
        self.prediction.clear();
        self.input.pending_touch = None;
        // 五笔 14 键的组码不是逐个字母，没有可以落脚的光标位置。
        if self.nine_key.active() || self.input.wubi_grid_composing() {
            return;
        }
        self.input.set_caret(caret);
    }

    /// End of the complete pinyin units before the caret in `editing_text`; 0 while nine-key digits are active.
    pub fn prefix_end(&self) -> usize {
        if self.nine_key.active() {
            return 0;
        }
        self.input.prefix_end()
    }

    /// The raw text after `prefix_end`, case kept; empty while nine-key digits are active.
    pub fn pending_suffix(&self) -> String {
        if self.nine_key.active() {
            return String::new();
        }
        self.input.pending_suffix()
    }

    pub fn snapshot(&self) -> SessionSnapshot {
        self.snapshot_with_display_options(true, false)
    }

    /// 给宿主画在屏幕上的快照：全拼的 `preedit` 按音节分开（双拼照 `shuangpin_preedit_uses_raw`），辅助码注释照常给出。[`Session::snapshot`] 的 `preedit` 保留全拼的按键原文，参考回放按它比对；其余字段两者相同。C 接口的 `msime_engine_state` 读的是这一份。
    pub fn display_snapshot(&self) -> SessionSnapshot {
        self.snapshot_with_display_options(true, true)
    }

    pub(crate) fn snapshot_for_host(&self, show_helpcode: bool) -> SessionSnapshot {
        self.snapshot_with_display_options(show_helpcode, true)
    }

    fn snapshot_with_display_options(
        &self,
        show_helpcode: bool,
        segmented_quanpin: bool,
    ) -> SessionSnapshot {
        if !self.prediction.rows.is_empty() {
            return SessionSnapshot {
                prediction: true,
                scheme: self.input.scheme(),
                local_mode: self.input.local_mode,
                spelling_symbols: self.input.spelling_symbols(),
                shuangpin_profile: self.input.profile.kind.name().to_owned(),
                candidates: self.prediction.rows.clone(),
                candidate_sources: self.prediction.rows.iter().map(|row| row.source).collect(),
                candidate_annotations: vec![String::new(); self.prediction.rows.len()],
                candidate_answers_key: vec![false; self.prediction.rows.len()],
                ..SessionSnapshot::default()
            };
        }
        let hide_helpcode = !show_helpcode
            && self.input.helpcode_enabled()
            && !self.input.local_mode.annotates_rows();
        if self.nine_key.active() {
            let mut snapshot = self.nine_key.snapshot();
            snapshot.candidate_annotations = self.input.candidate_helpcode_annotations(
                &snapshot.candidates,
                hide_helpcode,
                snapshot.scheme,
            );
            return snapshot;
        }
        let input = &self.input;
        let mut candidates = input.candidates().to_vec();
        for item in &mut candidates {
            input.apply_english_word_case(item.source, &mut item.word);
        }
        // 光标后编辑：候选仍是光标后那段的行，首选显示时前面接上固定的那段（由 runtime 在视图里接上），这样重排、释义和选择核对都按行本身的文字进行。
        let caret_edit_suffix = input.caret_edit.active && input.conversion.is_none();
        let caret_edit_prefix = input
            .caret_edit_prefix()
            .filter(|_| caret_edit_suffix && !candidates.is_empty())
            .unwrap_or_default();
        // 宿主显示全拼分词；参考回放的底层快照仍保留原文。
        let segmented_preedit = (segmented_quanpin && input.scheme() == SchemeType::Quanpin)
            || (!self.shuangpin_preedit_uses_raw && input.scheme() == SchemeType::Shuangpin);
        let preedit = if segmented_preedit
            && input.local_mode == LocalInputMode::None
            && !input.dedicated_english
        {
            input.pinyin_segmentation_with_cases().into_owned()
        } else {
            input.preedit()
        };
        SessionSnapshot {
            prediction: false,
            scheme: input.scheme(),
            local_mode: input.local_mode,
            spelling_symbols: input.spelling_symbols(),
            preedit,
            raw_segmentation: input.raw_segmentation(),
            normalized_segmentation: input.normalized_segmentation(),
            dedicated_english: input.dedicated_english,
            editing_text: input.editing_text(),
            caret_position: input.caret_position(),
            nine_key_spellings: input.zhuyin_spellings(),
            // 五笔 14 键的读音行显示首选的编码（就是预编辑），宿主 14 键的读音行照常读这一项。
            nine_key_reading: if input.wubi_grid_composing() {
                input.preedit()
            } else {
                String::new()
            },
            nine_key_single_character: false,
            nine_key_strokes: String::new(),
            answered_by_pinyin_fallback: input.answered_by_pinyin_fallback(),
            wubi_unique_four_code: input.wubi_unique_four_code(),
            shuangpin_profile: input.profile.kind.name().to_owned(),
            candidate_sources: candidates.iter().map(|item| item.source).collect(),
            multilingual_conversion: candidates
                .iter()
                .any(|item| item.source == CandidateSource::Multilingual)
                .then(|| input.multilingual_conversion.clone())
                .flatten(),
            candidate_annotations: input.candidate_annotations(hide_helpcode),
            // 纠错候选按原始按键长度消费；普通候选仍按 `pinyin`，两者都由组字推进判断。
            candidate_answers_key: candidates
                .iter()
                .map(|item| {
                    // 占位行、网址补全和联系人不交给重排模型；联系人保留本地优先顺序。
                    !matches!(
                        item.source,
                        CandidateSource::AiPending
                            | CandidateSource::CloudPending
                            | CandidateSource::Contact
                    ) && !input.is_url_completion(item)
                        && input.selection_completes_composition(item)
                })
                .collect(),
            candidate_contest: Vec::new(),
            candidate_list_open: input.candidate_list_open(),
            conversion: input
                .conversion
                .as_ref()
                .map(|edit| edit.text())
                .unwrap_or_default(),
            conversion_focus: input
                .conversion
                .as_ref()
                .map_or((0, 0), |edit| edit.focus_span()),
            caret_edit_suffix,
            caret_edit_prefix,
            candidates,
        }
    }

    /// 返回候选词的完整五笔编码，仅供宿主显示反查结果；九宫格会话不沿用旧的普通候选反查。
    pub fn candidate_wubi_code(&self, word: &str) -> Option<&str> {
        if self.nine_key.active() {
            return None;
        }
        self.input.engine.candidate_wubi_code(word)
    }

    /// 显示候选与原始候选同座且词面一致时复用已缓存的反查码；候选身份错位时返回 `None`，由宿主回退到按词入口。
    pub(crate) fn candidate_wubi_code_at_if_item(
        &self,
        index: usize,
        item: &WordItem,
    ) -> Option<&str> {
        if self.nine_key.active() {
            return None;
        }
        self.input
            .engine
            .candidate_wubi_code_at_if_item(index, item)
    }

    pub(crate) fn candidate_wubi_code_if_item(&self, item: &WordItem) -> Option<&str> {
        if self.nine_key.active() {
            return None;
        }
        self.input.engine.candidate_wubi_code_if_item(item)
    }

    /// Byte offsets into `editing_text` for pinyin-unit editing; empty when idle, in local modes, for non-pinyin schemes and during nine-key input. Read-only.
    pub fn segment_raw_boundaries(&self) -> Vec<usize> {
        if self.nine_key.active() {
            return Vec::new();
        }
        self.input.segment_raw_boundaries()
    }

    pub fn online_query(&self) -> Option<OnlineQuery> {
        if self.nine_key.active() {
            return None;
        }
        self.input.online_query()
    }

    /// Whether an apostrophe now separates a unit from its target or one `/fy` word from the next, so a host or runtime that would finish the composition on punctuation hands it over as a character instead.
    pub fn takes_local_separator(&self) -> bool {
        !self.nine_key.active() && self.input.takes_local_separator()
    }

    /// The `/fy` translation request; see `InputSession::command_translation_query`.
    pub fn command_translation_query(&self) -> Option<CommandTranslationQuery> {
        if self.nine_key.active() {
            return None;
        }
        self.input.command_translation_query()
    }

    /// Whether the translation now shows first; false (not an error) for a stale query.
    pub fn apply_command_translation(
        &mut self,
        query: &CommandTranslationQuery,
        translation: &str,
    ) -> bool {
        if self.nine_key.active() {
            return false;
        }
        let applied = self.input.apply_command_translation(query, translation);
        if applied {
            self.candidates_may_change();
        }
        applied
    }

    /// 插件指令要宿主去问的那一次；见 `InputSession::plugin_command_query`。
    pub fn plugin_command_query(&self) -> Option<PluginCommandQuery> {
        if self.nine_key.active() {
            return None;
        }
        self.input.plugin_command_query()
    }

    /// 插件的行是否写进了会话；过期的查询返回 false（不是错误）。
    pub fn apply_plugin_command_rows(
        &mut self,
        query: &PluginCommandQuery,
        rows: &[(String, String)],
    ) -> bool {
        if self.nine_key.active() {
            return false;
        }
        let applied = self.input.apply_plugin_command_rows(query, rows);
        if applied {
            self.candidates_may_change();
        }
        applied
    }

    /// Whether the rows now show; false (not an error) for a stale query.
    pub fn apply_online_candidates(
        &mut self,
        query: &OnlineQuery,
        words: &[String],
        source: CandidateSource,
    ) -> bool {
        if self.nine_key.active() {
            return false;
        }
        let applied = self.input.apply_online_candidates(query, words, source);
        if applied {
            self.candidates_may_change();
        }
        applied
    }

    /// Remove one online provider's rows from cached and visible candidates.
    pub fn clear_online_candidates(&mut self, source: CandidateSource) {
        self.candidates_may_change();
        self.input.clear_online_candidates(source);
    }

    pub fn apply_online_candidate(
        &mut self,
        query: &OnlineQuery,
        word: &str,
        source: CandidateSource,
    ) -> bool {
        if self.nine_key.active() {
            return false;
        }
        let applied = self.input.apply_online_candidate(query, word, source);
        if applied {
            self.candidates_may_change();
        }
        applied
    }

    /// Grid commits bypass the input session, so the word after one must not read the word before it as its context (session.cpp:37-45).
    fn after_nine_key(&mut self, result: KeyResult) -> KeyResult {
        if result.commit.is_some() {
            self.input.note_external_commit();
        }
        result
    }

    fn predict_after_commit(&mut self, selected: Option<WordItem>, result: &KeyResult) {
        if !self.prediction.enabled
            || self.nine_key.active()
            || self.input.has_composition()
            || self.input.dedicated_english
            || self.input.local_mode != LocalInputMode::None
            || !matches!(
                self.input.scheme(),
                SchemeType::Quanpin | SchemeType::Shuangpin
            )
        {
            return;
        }
        let Some(selected) = selected else { return };
        if result.commit.as_deref() != Some(selected.word.as_str()) {
            return;
        }
        let previous = selected
            .sentence_words
            .last()
            .map_or(selected.word.as_str(), String::as_str);
        self.show_prediction(
            &selected.word,
            &selected.canonical_pinyin,
            &selected.word,
            previous,
        );
    }

    fn show_prediction(&mut self, word: &str, pinyin: &str, committed: &str, previous: &str) {
        let personal = self
            .input
            .personal_context_applies()
            .then_some(self.input.personal_context.as_ref());
        self.prediction
            .show(word, pinyin, committed, previous, personal);
    }
}

/// The C++ registered `PersonalNgramStore::flush_all` with `atexit` (personal_ngram_store.cpp:255), so context learned in the last ~2 s reached the journal when the host quit. Rust runs no destructors for statics and `atexit` needs unsafe, so the session writes its journal's queue as it ends, which hosts do on deactivation and shutdown. A host that can exit without dropping its sessions (macOS `[NSApp terminate:]` runs `exit()`) calls [`crate::flush_personal_learning`] (host-api's `msime_client_flush_all`) or [`crate::shutdown`] from its will-terminate hook instead.
impl Drop for Session {
    fn drop(&mut self) {
        // Nobody is left to report to: a failed write stays queued and flagged in the store, and the next record reports it and schedules another try.
        let _ = flush_journal(&self.journal);
        // A host thread whose sessions are gone (a quiesced IME, a closed window) holds no journal handle.
        release_thread_journal();
        // The last session on this journal also closes the idle connections other threads keep to it, so a session created on one thread and ended on another leaves nothing open. Before the record below: closing the last connection to a WAL journal checkpoints it, which changes the files the record describes.
        self.journal_use.end();
        // Nothing of this session writes the journal after this point. A host-api session still holds its dictionary access here (its Runtime drops first), so no dictionary maintenance can change the file between the write above and this record.
        note_settled(&self.journal);
    }
}
