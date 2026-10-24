//! The public `Session` (`include/metasequoia/session.h`, core-session.md §4): an input session plus the nine-key session, with the routing rules between them. One host serialises calls to its session; distinct sessions may run concurrently.
//!
//! The setters the product only sets at construction (`switch_scheme`, `set_helpcode_schema`, `set_helpcode_enabled`, `set_wubi_mixed_pinyin`, `set_personal_context_enabled`) and `candidate_key` stay public because the golden scenarios drive them. Learning undo (`set_learning_undo_enabled`, `forget_recent_commits`) is dropped.

mod candidates;
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
#[cfg(test)]
mod tests;

use std::sync::Arc;

use crate::diagnostics;
use crate::error::{EngineError, Result};
use crate::nine_key::{KeyGrid, NineKeySession};
use crate::pinyin::glide::{GlideKeyboard, GlidePoint};
use crate::types::{
    CandidateEdge, CandidateSource, Command, CommandTableEntry, CommandTranslationQuery, KeyResult,
    LocalInputMode, MentionEntry, OnlineQuery, PluginCommand, PluginCommandQuery, QuickPhraseEntry,
    SchemeType, WordItem,
};

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
    prediction: prediction::Prediction,
}

impl Session {
    /// Validates the paths and every option (`INVALID_SESSION_OPTIONS`, `UNKNOWN_HELPCODE_SCHEMA`), loads the helpcode table and applies the options in the reference order (session.cpp:11-30, 57-66).
    pub fn new(options: SessionOptions) -> Result<Session> {
        options.paths.validate()?;
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
        Ok(Session {
            input,
            nine_key,
            key_grid: None,
            shuangpin_preedit_uses_raw: options.shuangpin_preedit_uses_raw,
            prediction: prediction::Prediction::new(
                options.next_word_prediction,
                options.paths.dictionary(crate::assets::MAIN_DICTIONARY),
            ),
        })
    }

    /// 会话建立时定下的双拼方案（内置的表，或校验过的用户表）。
    pub(crate) fn shuangpin_layout(&self) -> &'static crate::shuangpin::ShuangpinProfile {
        self.input.profile
    }

    /// Replace the steady and local clocks, so tests can cross the 3 s / 8 s / 10 s personal-learning windows and the date/time mode can be pinned.
    pub fn set_clock(&mut self, clock: Clock) {
        if self.input.local_mode_options.date_time_inline {
            self.nine_key
                .set_inline_date_time(Some(Arc::clone(&clock.local)));
        }
        self.input.clock = clock;
    }

    /// One ASCII character; `shift_only` is a bare Shift+letter (local mode entry). 全拼下九键开启且没有别的组字时，数字 2-9 交给九宫格会话；九键双拼交的是方案用到的数字（可以有 1 和 0）；注音九键的数字由注音编辑器自己处理。
    pub fn character(&mut self, value: u8, shift_only: bool) -> KeyResult {
        self.prediction.clear();
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
        self.input.handle_character(value, shift_only)
    }

    /// 滑过字母键的一笔：它最可能拼出的全拼字母写到光标处，与已有组字之间用 `'` 隔开。不是全拼、在本地模式或专用英文里、九键数字正在组字，以及没有串跟得上这一笔时，不处理。
    pub fn glide(&mut self, keyboard: &GlideKeyboard, points: &[GlidePoint]) -> KeyResult {
        self.prediction.clear();
        if self.nine_key.active() {
            return KeyResult::unhandled();
        }
        self.input.glide(keyboard, points)
    }

    /// 九宫格（九键或 14 键）正在组字：这时的快照、命令和上屏都来自九宫格会话。
    pub fn grid_composing(&self) -> bool {
        self.nine_key.active()
    }

    /// `set_key_grid` 的九键开关：开是九键，关是不用网格。
    pub fn set_nine_key_enabled(&mut self, enabled: bool) {
        self.set_key_grid(enabled.then_some(KeyGrid::NineKey));
    }

    /// 换触屏的组码网格，先丢掉网格里的组字，所以在换键盘布局、没有组字时调用。英文九键和注音九键只在九键下打开（注音组字会被丢掉）：14 键的英文模式画全键盘，注音没有 14 键。
    pub fn set_key_grid(&mut self, grid: Option<KeyGrid>) {
        self.prediction.clear();
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

    /// 字母网格的一键：`letter` 是这一组里的任一字母，由 `KeyGrid::code_of` 归成组码交给九宫格会话，宿主约定送首字母。不复用硬件键盘的 `character`。全拼和双拼分别使用自己的分组表；本地模式、专用英文或已有 26 键组字时不处理，状态不变。
    pub fn grid_key(&mut self, letter: u8) -> KeyResult {
        self.prediction.clear();
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
        if !self.nine_key.active() && self.input.zhuyin_rules_apply() {
            return self.input.choose_zhuyin_spelling(index);
        }
        let result = self.nine_key.choose_spelling(index);
        self.after_nine_key(result)
    }

    /// 九宫格组字时按单字、按笔画筛选候选（`NineKeySession::set_filter`）；没有九宫格组字时不处理。
    pub fn set_nine_key_filter(&mut self, single_character: bool, strokes: &str) -> KeyResult {
        if !self.nine_key.active() {
            return KeyResult::unhandled();
        }
        self.nine_key.set_filter(single_character, strokes)
    }

    pub fn command(&mut self, command: Command) -> KeyResult {
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
        self.prediction.clear();
        if self.nine_key.active() {
            return self.command(Command::MoveLeft);
        }
        self.input.conversion_left_from(index)
    }

    /// `1`..`9` 选择组字候选；空闲联想不接管数字键。
    pub fn candidate_key(&mut self, value: u8) -> KeyResult {
        self.prediction.clear();
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
        self.prediction.clear();
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
        if !self.prediction.rows.is_empty() {
            let Some((row, word, pinyin)) = self.prediction.selected(index) else {
                return KeyResult::unhandled();
            };
            let diagnostic = self.input.record_personal_context(Some(&row), false, false);
            self.show_prediction(&word, &pinyin, &row.word);
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
        self.prediction.clear();
        if self.nine_key.active() {
            let result = self.nine_key.finish(first_index);
            return self.after_nine_key(result);
        }
        self.input.finish_composition(first_index)
    }

    /// Discards the composition. Fails, staying in the current scheme with the composition untouched, when the new scheme's dictionary cannot be opened (Cantonese without a usable `msime-cantonese.db`, Stroke without a usable `msime-stroke.db`: `LANGUAGE_DICTIONARY_UNAVAILABLE`, `LANGUAGE_DICTIONARY_VERSION_UNSUPPORTED`).
    pub fn switch_scheme(&mut self, scheme: SchemeType) -> Result<()> {
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
        self.input.set_helpcode_schema(schema)
    }

    /// 换上宿主给的辅助码表，替换当前的表；全拼和双拼都用它。
    pub fn set_helpcode_table(&mut self, table: crate::helpcode::SharedKeymap) {
        self.input.set_helpcode_table(table);
    }

    /// Quanpin and shuangpin together, as `SessionOptions::helpcode` does.
    pub fn set_helpcode_enabled(&mut self, enabled: bool) {
        self.input.set_helpcode_enabled(enabled);
    }

    /// 只有模式改变时才取消组字；重复设置保留当前输入。
    pub fn set_dedicated_english(&mut self, enabled: bool) {
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
        self.input.set_wubi_mixed_pinyin(enabled);
    }

    /// 五笔拼音混输是否打开。
    pub fn wubi_mixed_pinyin(&self) -> bool {
        self.input.engine.wubi_input_options().mixed_pinyin
    }

    /// 宿主的清缓存动作（维护快捷键、iOS 键盘扩展的内存告警）：清掉本会话各方案的候选缓存，并放掉进程级缓存里的日文模型，让不再用它的进程能交还前缀索引和映射；本会话若正持有模型，照常用到会话结束。
    pub fn reset_cache(&mut self) {
        self.prediction.clear();
        self.input.reset_cache();
        crate::japanese::decoder::JapaneseDictionary::release_shared();
    }

    /// Replace the `/` mode's command table; rows it cannot use are dropped. A diagnostic only if the open command list could not be refreshed.
    pub fn set_command_table(&mut self, table: &[CommandTableEntry]) -> Option<String> {
        self.input.set_command_table(table)
    }

    /// 替换脚本插件的 `/` 指令；用不了的被丢弃（`local::command::usable_plugin_commands`），缓存的插件行清掉。只有打开的 `/` 列表刷新失败时才返回诊断。
    pub fn set_plugin_commands(&mut self, commands: &[PluginCommand]) -> Option<String> {
        self.input.set_plugin_commands(commands)
    }

    /// 生效的插件指令里 `trigger` 的标题，见 `CandidateQueries::plugin_command_title`。
    pub fn plugin_command_title(&self, trigger: &str) -> Option<&str> {
        self.input.queries.plugin_command_title(trigger)
    }

    /// 替换 K 模式的宿主短语表；用不了的行被丢弃。只有打开的 K 模式列表刷新失败时才返回诊断。
    pub fn set_quick_phrase_table(&mut self, table: &[QuickPhraseEntry]) -> Option<String> {
        self.input.set_quick_phrase_table(table)
    }

    /// Replace the `@` mode's list; entries it cannot use are dropped.
    pub fn set_mention_entries(&mut self, entries: &[MentionEntry]) -> Option<String> {
        self.input.set_mention_entries(entries)
    }

    /// Turn the embedded places of `@` mode on or off.
    pub fn set_mention_places(&mut self, enabled: bool) -> Option<String> {
        self.input.set_mention_places(enabled)
    }

    /// Forget the committed-word context (focus or application change, host-inserted text).
    pub fn reset_context(&mut self) {
        self.prediction.clear();
        self.input.reset_commit_context();
    }

    /// 宿主的即时隐私门禁；关闭时清掉建议，用户偏好仍保留。
    pub fn set_prediction_allowed(&mut self, allowed: bool) {
        self.prediction.set_allowed(allowed);
    }

    /// Disabling also ends the current context.
    pub fn set_personal_context_enabled(&mut self, enabled: bool) {
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
        self.input.set_cloud_candidates_enabled(enabled);
    }

    pub fn english_online_candidates_enabled(&self) -> bool {
        self.input.english_online_candidates_enabled
    }

    pub fn set_english_online_candidates_enabled(&mut self, enabled: bool) {
        self.input.set_english_online_candidates_enabled(enabled);
    }

    /// The committed text the neural sentence models condition on, updated without rebuilding the session. Only the last 64 characters matter.
    pub fn set_rescoring_context(&mut self, context: &str) {
        self.input.set_rescoring_context(context);
        self.nine_key.set_rescoring_context(context);
    }

    /// 首页行数只决定词库要不要并入补全词。`Runtime::set_page_size` 在宿主改每页候选数时调它，
    /// 因此引擎的首屏门槛和分页始终同源，不必等下一次因别的偏好变化重建引擎。
    pub fn set_first_page_rows(&mut self, rows: FirstPageRows) {
        self.input.set_first_page_rows(rows);
        self.nine_key.set_first_page_rows(rows);
    }

    /// Hand over the candidates withheld from a single-letter query; whether the list grew.
    pub fn expand_initial_candidates(&mut self) -> bool {
        // Nine-key rows come from the spelling session rather than a dictionary query, so nothing is withheld there.
        if self.nine_key.active() {
            return false;
        }
        self.input.expand_initial_candidates()
    }

    /// 设置前缀解码的组字光标（overlays.md §7.6）；位置限制在可编辑范围，不能越过本地模式的入口标记。`None` 回到末尾，九键组码时无操作。
    pub fn set_caret(&mut self, caret: Option<usize>) {
        self.prediction.clear();
        if self.nine_key.active() {
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
        if self.nine_key.active() {
            return self.nine_key.snapshot();
        }
        let hide_helpcode = !show_helpcode
            && self.input.helpcode_enabled()
            && !self.input.local_mode.annotates_rows();
        let input = &self.input;
        let mut candidates = input.candidates().to_vec();
        for item in &mut candidates {
            input.apply_english_word_case(item.source, &mut item.word);
        }
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
            nine_key_reading: String::new(),
            nine_key_single_character: false,
            nine_key_strokes: String::new(),
            answered_by_pinyin_fallback: input.answered_by_pinyin_fallback(),
            wubi_unique_four_code: input.wubi_unique_four_code(),
            shuangpin_profile: input.profile.kind.name().to_owned(),
            candidate_sources: candidates.iter().map(|item| item.source).collect(),
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
        self.input.apply_command_translation(query, translation)
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
        self.input.apply_plugin_command_rows(query, rows)
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
        self.input.apply_online_candidates(query, words, source)
    }

    /// Remove one online provider's rows from cached and visible candidates.
    pub fn clear_online_candidates(&mut self, source: CandidateSource) {
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
        self.input.apply_online_candidate(query, word, source)
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
        self.show_prediction(&selected.word, &selected.canonical_pinyin, previous);
    }

    fn show_prediction(&mut self, word: &str, pinyin: &str, previous: &str) {
        let personal = self
            .input
            .personal_context_applies()
            .then_some(self.input.personal_context.as_ref());
        self.prediction.show(word, pinyin, previous, personal);
    }
}
