//! The bridge's `Session` wrapper over the engine session (api-contract §1a, §2, §5): ASCII checks, the host command numbering, the flattened snapshot and the online query snapshot. The raw-commit learning policy and `CommitRawWithoutLearning` are core options and commands (`SessionOptions::learn_typed_spellings_on_enter`, `Command::CommitRawWithoutLearning`); the facade turns the policy on through `session_options` and maps the command by name.

use std::marker::PhantomData;
use std::path::Path;
use std::rc::Rc;
use std::sync::Arc;

use super::options::{session_options, EngineOptions};
use crate::diagnostics;
use crate::error::{EngineError, Result};
use crate::helpcode::{
    compute_helpcodes, load_helpcode_keymap_with_fallback, HelpcodeKeymap, SharedKeymap,
};
use crate::japanese::romaji::romaji_display_reading_into;
use crate::nine_key::KeyGrid;
use crate::pinyin::glide::{GlideKeyboard, GlidePoint};
use crate::touch::{KeyGeometry, KeyTouch, TouchTuning};
use crate::types::{
    CandidateEdge, CandidateSource, CommandTableEntry, CommandTranslationQuery, FirstPageRows,
    KeyResult, LocalInputMode, MentionEntry, OnlineQuery, PluginCommand, PluginCommandQuery,
    QuickPhraseEntry, SchemeType,
};

/// The host's command numbering. `CommitRawWithoutLearning` took 11 here before the engine had it, so the engine numbers it 14, and the engine's `ConvertHanja` (11) is 12 here; those are mapped by name rather than by ordinal. 整句改字的两条命令同样按名字映射，宿主编号比引擎多一。
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(u8)]
pub enum Command {
    Backspace = 0,
    CommitCandidate = 1,
    CommitRaw = 2,
    Cancel = 3,
    MoveLeft = 4,
    MoveRight = 5,
    MoveHome = 6,
    MoveEnd = 7,
    DeleteForward = 8,
    CycleKanaVariant = 9,
    CommitReading = 10,
    /// Commit the letters as typed without learning them as an English word.
    CommitRawWithoutLearning = 11,
    /// Open or close the active scheme's candidate list (the Korean Hanja list, the Zhuyin conversion list); unhandled in a scheme without one. Hosts may call it `MSIME_OPEN_CANDIDATE_LIST`.
    ConvertHanja = 12,
    /// 整句改字的光标左移一个字（`crate::types::Command::ConversionLeft`）。
    ConversionLeft = 13,
    /// 整句改字的光标右移一个字（`crate::types::Command::ConversionRight`）。
    ConversionRight = 14,
}

/// 当前唯一混合候选的输入身份及语言分段，不随候选下标建立平行向量。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MultilingualComposition {
    pub raw_input: String,
    pub conversion: crate::multilingual::Conversion,
}

/// 除可选的静态评分证据外，所有 `candidate_*` 向量必须与 `candidates` 等长，运行时才能同步重排。
#[derive(Debug, Clone, PartialEq, Default)]
pub struct EngineSnapshot {
    /// 上屏后的空闲联想，不属于组字。
    pub prediction: bool,
    pub local_mode: String,
    /// The non-letter characters `character` takes in this state (`SessionSnapshot::spelling_symbols`): send one of these as a character, never as punctuation.
    pub spelling_symbols: String,
    pub dedicated_english: bool,
    /// 九键组码网格开着（`set_key_grid` 设成九键，或 `set_nine_key_enabled(true)`）；14 键下为假。引擎快照里没有这个标志，由这里转述。
    pub nine_key: bool,
    /// `set_key_grid` 设下的组码网格，`None` 是不用网格。九键和 14 键共用 `nine_key_*` 那几项。
    pub key_grid: Option<KeyGrid>,
    pub nine_key_spellings: Vec<String>,
    /// `SessionSnapshot::nine_key_reading`.
    pub nine_key_reading: String,
    /// `SessionSnapshot::nine_key_single_character`.
    pub nine_key_single_character: bool,
    /// `SessionSnapshot::nine_key_strokes`.
    pub nine_key_strokes: String,
    /// 这次组字是在九宫格（九键或 14 键）里打的，见 [`crate::Session::grid_composing`]。`key_grid` 开着时硬件键盘的字母照样走全拼 26 键，这一项分得清两者。
    pub grid_composing: bool,
    /// 当前方案是双拼，且韵母或零声母编码用 `;` 作第二键：宿主把 `;` 当作字母键送给 Engine。字段名沿用微软双拼，是宿主已经在读的契约。
    pub microsoft_shuangpin: bool,
    /// 会话跑的双拼方案名（`xiaohe`、`custom` 等），任何时候都给出，九宫格组字时也一样：宿主拿它画键面、算键盘外观的缓存键。核心快照在九宫格组字时这一项留空（金样例按此冻结），门面不照抄它。
    pub shuangpin_profile: String,
    pub preedit: String,
    /// The kana reading in Japanese, the composed Hangul in Korean, the converted text plus the pending bopomofo in Zhuyin, the stroke glyphs (一丨丿丶乛＊) in Stroke, else empty. In Stroke each glyph stands for one ASCII letter of `editing_text`, so `caret_position` also counts glyphs.
    pub reading: String,
    /// 宿主在行内画的读音：日文罗马字组字（非本地模式、非英文）时末尾单个 `n` 还没定成ん，按微软、Google 日文输入法的样子留作字母（`kan` 画成 `かn`，`kann` 才是 `かん`）；其余情况与 `reading` 相同。`reading` 仍是确定（`CommitReading`）上屏的读音，宿主不经引擎结束组字区、且编辑器仍报告着组字区时，应把它换成 `reading` 再结束；确认不了组字区还在时只结束组字（规则见 `msime_client.h` 的 `View.display_reading`）。
    pub display_reading: String,
    pub editing_text: String,
    pub caret_position: usize,
    pub segment_raw_boundaries: Vec<u64>,
    pub candidates: Vec<String>,
    pub candidate_codes: Vec<String>,
    pub scheme: u8,
    pub answered_by_pinyin_fallback: bool,
    pub wubi_unique_four_code: bool,
    pub candidate_annotations: Vec<String>,
    pub candidate_sources: Vec<u8>,
    /// 必须按来源、原始输入和完整词面匹配后使用；没有混合行时为空。
    pub multilingual_conversion: Option<MultilingualComposition>,
    pub candidate_positions: Vec<u8>,
    pub candidate_corrected: Vec<bool>,
    pub candidate_answers_key: Vec<bool>,
    /// 与候选同步排列的词网格逐字评分证据；非词网格行为空，证据缺失或长度不符时沿用原模型决策。
    pub candidate_static_evidence: Vec<Option<crate::ordering::StaticEvidence>>,
    /// 与候选同步排列：九宫格标出的窄裁决点（`ordering::CONTEST_*`，见 `SessionSnapshot::candidate_contest`）。一行都没有标记时为空。
    pub candidate_contest: Vec<u8>,
    /// 与候选同步排列：这一行是 26 键双拼按触点计过价、落点支持留在引擎座位上的纠错整句（`WordItem::touch_supported`），runtime 不把它当作次选读法挪到表尾（`ordering::runner_up_order_with`）。一行都没有时为空，没有触点的请求（桌面、实体键盘）永远为空；按行而不是按整张表，因为同一张表里别的纠错行（模糊音替代读法按平面表查出的）没过落点的闸。
    pub candidate_touch_supported: Vec<bool>,
    /// 与候选同步排列：这一行是按首选规则坐上第一位的双拼触点纠错（`WordItem::confident_correction`），视图据此给它标 `Candidate::confident_correction`，宿主画强调色、读屏念「已纠正」。一行都没有时为空，没有触点的请求永远为空；本地模式和专用英文下也为空。
    pub candidate_confident_correction: Vec<bool>,
    /// The scheme's openable candidate list is showing (the Korean Hanja list, the Zhuyin conversion list); candidates are its rows while it is.
    pub candidate_list_open: bool,
    /// 整句改字时改好的整句，否则为空（`SessionSnapshot::conversion`）。
    pub conversion: String,
    /// 整句改字的光标在第几个字之前，按 `conversion` 的 Unicode 标量计。
    pub conversion_focus_start: usize,
    /// 光标处那一段（候选所替换的字）的结尾，同样按标量计；光标在句末时等于 `conversion_focus_start`。
    pub conversion_focus_end: usize,
    /// 光标后编辑正在生效（`SessionSnapshot::caret_edit_suffix`）：宿主显示模式提示，runtime 不重排候选。
    pub caret_edit_suffix: bool,
    /// 首选前面固定的那段文字（`SessionSnapshot::caret_edit_prefix`），首选显示为它接上 `candidates[0]`；模式不生效时为空。
    pub caret_edit_prefix: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct EngineResult {
    pub handled: bool,
    pub has_commit: bool,
    pub commit: String,
    pub diagnostic: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct OnlineQuerySnapshot {
    pub available: bool,
    pub mode: crate::OnlineInputMode,
    pub scheme: u8,
    pub generation: u64,
    pub identity: String,
    pub query_text: String,
    pub cache_key: String,
    pub pinyin_segments: Vec<String>,
    pub cloud_eligible: bool,
    pub ai_eligible: bool,
    pub session_id: u64,
}

/// Product glue over the core [`crate::Session`]: the core owns the candidate revision, teardown, the key grid, the helpcode table and the shuangpin profile, and this wrapper keeps only the product's options (the raw-commit learning switch, whether to show helpcodes, where to reload a helpcode table from). Thread-confined like the bridge session it replaces, which is host-api's threading model; the core session itself may move between threads.
pub struct Session {
    inner: crate::session::Session,
    options: EngineOptions,
    _thread_confined: PhantomData<Rc<()>>,
}

impl Session {
    /// 显示候选仍与引擎原始候选同座时直接取反查码；身份变化时扫描已有候选缓存。
    pub(crate) fn candidate_wubi_code_for_display_item(
        &self,
        index: usize,
        item: &crate::types::WordItem,
    ) -> Option<&str> {
        self.inner
            .candidate_wubi_code_at_if_item(index, item)
            .or_else(|| self.inner.candidate_wubi_code_if_item(item))
    }

    /// 供已有宿主测试和按词入口使用的查码；不使用可能与显示列表错位的座位。
    #[cfg(test)]
    pub(crate) fn candidate_wubi_code_for_display(
        &self,
        _index: usize,
        word: &str,
    ) -> Option<&str> {
        self.inner.candidate_wubi_code(word)
    }
}

impl Session {
    /// 隐私输入立即关闭联想，不等待会话按偏好重建。
    pub fn set_prediction_allowed(&mut self, allowed: bool) {
        self.inner.set_prediction_allowed(allowed);
    }

    /// 联想的词库续写多取一池留给模型排序，同 [`crate::Session::set_prediction_pool`]。
    pub fn set_prediction_pool(&mut self, enabled: bool) {
        self.inner.set_prediction_pool(enabled);
    }

    /// 按模型给刚上屏之后的联想排序，同 [`crate::Session::order_prediction`]。
    pub fn order_prediction(
        &mut self,
        scorer: &mut dyn crate::ordering::CandidateScorer,
        before: &str,
    ) -> bool {
        self.inner.order_prediction(scorer, before)
    }

    /// 换掉会话读的单调时钟和本地墙钟，同 [`crate::Session::set_clock`]。只给评测和测试用，宿主不调：`typing_sim` 按模拟的打字节奏推进时钟，个人学习的 3 s / 8 s / 10 s 窗口和行内日期时间行才不随机器快慢、负载高低和运行的日子变化。
    pub fn set_clock(&mut self, clock: crate::Clock) {
        self.inner.set_clock(clock);
    }

    pub fn new(options: &EngineOptions) -> Result<Session> {
        // The C++ ran `options_for` a second time only to read the profile name back (bridge.cpp:430); that rerun recopied the same sidecar, so one mapping is enough. The bridge also loaded a second copy of the helpcode table only to annotate (bridge.cpp:431-435); the annotation now reads the table the core session filters with.
        let inner = crate::session::Session::new(session_options(options)?)?;
        Ok(Session {
            inner,
            options: options.clone(),
            _thread_confined: PhantomData,
        })
    }

    /// The scheme is shuangpin and its profile takes `;` as a second key (`EngineSnapshot::microsoft_shuangpin`). The facade never switches scheme, so the scheme it was built with is the one running.
    fn microsoft_shuangpin(&self) -> bool {
        self.options.scheme == SchemeType::Shuangpin as u8
            && self.inner.shuangpin_layout().uses_semicolon_key()
    }

    /// With the annotation rule of bridge.cpp:914-929.
    pub fn snapshot(&self) -> Result<EngineSnapshot> {
        let value = self.inner.snapshot_for_host(self.options.show_helpcode);
        let helpcode_scheme = value.scheme.helpcode();
        let uppercase_all = value.scheme == SchemeType::Quanpin;
        // Rows the engine generated in the expression, command and mention modes are not spelled by pinyin; their annotations are the engine's own. 拆字行的注释是字的读音，没有读音的字也不补辅助码。
        let keymap = self.inner.helpcode_table().filter(|_| {
            self.options.helpcode && helpcode_scheme && !value.local_mode.annotates_rows()
        });
        let count = value.candidates.len();
        // 一行都没有落点支持时整列留空，桌面每次快照不多一次分配。
        let touch_supported = value.candidates.iter().any(|row| row.touch_supported);
        // 首选规则 (f)：本地模式、专用英文不算，那里的行不是双拼的回答。
        let confident = value.local_mode == LocalInputMode::None
            && !value.dedicated_english
            && value.candidates.iter().any(|row| row.confident_correction);
        let display_reading = if !value.scheme.draws_reading() {
            String::new()
        } else if value.scheme == SchemeType::JapaneseRomaji
            && value.local_mode == LocalInputMode::None
            && !value.dedicated_english
        {
            let mut reading = String::new();
            romaji_display_reading_into(&value.raw_segmentation, &mut reading);
            reading
        } else {
            value.normalized_segmentation.clone()
        };
        let multilingual_conversion = value.multilingual_conversion.and_then(|conversion| {
            value
                .candidates
                .iter()
                .find(|row| row.source == CandidateSource::Multilingual)
                .map(|row| MultilingualComposition {
                    raw_input: row.pinyin.to_string(),
                    conversion,
                })
        });
        let mut output = EngineSnapshot {
            prediction: value.prediction,
            local_mode: value.local_mode.name().to_owned(),
            spelling_symbols: value.spelling_symbols,
            dedicated_english: value.dedicated_english,
            nine_key: self.inner.key_grid() == Some(KeyGrid::NineKey),
            key_grid: self.inner.key_grid(),
            nine_key_spellings: value.nine_key_spellings,
            nine_key_reading: value.nine_key_reading,
            nine_key_single_character: value.nine_key_single_character,
            nine_key_strokes: value.nine_key_strokes,
            grid_composing: self.inner.grid_composing(),
            microsoft_shuangpin: self.microsoft_shuangpin(),
            shuangpin_profile: self.inner.shuangpin_layout().kind.name().to_owned(),
            preedit: value.preedit,
            display_reading,
            reading: if value.scheme.draws_reading() {
                value.normalized_segmentation
            } else {
                String::new()
            },
            editing_text: value.editing_text,
            caret_position: value.caret_position,
            segment_raw_boundaries: self
                .inner
                .segment_raw_boundaries()
                .into_iter()
                .map(|boundary| boundary as u64)
                .collect(),
            candidates: Vec::with_capacity(count),
            candidate_codes: Vec::with_capacity(count),
            scheme: value.scheme as u8,
            answered_by_pinyin_fallback: value.answered_by_pinyin_fallback,
            wubi_unique_four_code: value.wubi_unique_four_code,
            candidate_annotations: Vec::with_capacity(count),
            candidate_sources: Vec::with_capacity(count),
            multilingual_conversion,
            candidate_positions: Vec::with_capacity(count),
            candidate_corrected: Vec::with_capacity(count),
            candidate_answers_key: Vec::with_capacity(count),
            candidate_static_evidence: Vec::with_capacity(count),
            candidate_contest: if value.candidate_contest.len() == count {
                value.candidate_contest
            } else {
                Vec::new()
            },
            candidate_touch_supported: if touch_supported {
                Vec::with_capacity(count)
            } else {
                Vec::new()
            },
            candidate_confident_correction: if confident {
                Vec::with_capacity(count)
            } else {
                Vec::new()
            },
            candidate_list_open: value.candidate_list_open,
            conversion: value.conversion,
            conversion_focus_start: value.conversion_focus.0,
            conversion_focus_end: value.conversion_focus.1,
            caret_edit_suffix: value.caret_edit_suffix,
            caret_edit_prefix: value.caret_edit_prefix,
        };
        let mut annotations = value.candidate_annotations.into_iter();
        for (index, candidate) in value.candidates.into_iter().enumerate() {
            let mut annotation = annotations
                .next()
                .unwrap_or_else(|| candidate.corrected_from.to_string());
            if let Some(keymap) = keymap {
                if self.options.show_helpcode
                    && annotation.is_empty()
                    && candidate.source == CandidateSource::Generated
                {
                    // The engine annotates dictionary rows itself; a sentence it synthesised carries no helpcode until the host adds one.
                    annotation = compute_helpcodes(&candidate.word, uppercase_all, keymap);
                }
            }
            // 五笔反查只服务五笔方案：混输拼音的候选虽然来自全拼，也一样附上五笔码；全拼和双拼方案本身不显示，那里的注释位留给辅助码和纠错提示。
            if value.local_mode == LocalInputMode::None
                && !value.dedicated_english
                && value.scheme == SchemeType::Wubi
                && matches!(
                    candidate.scheme,
                    SchemeType::Quanpin | SchemeType::Shuangpin | SchemeType::Wubi
                )
            {
                let code = self.candidate_wubi_code_for_display_item(index, &candidate);
                if let Some(code) = code {
                    if !annotation.contains(code) {
                        if !annotation.is_empty() {
                            annotation.push(' ');
                        }
                        annotation.push_str(code);
                    }
                }
            }
            output.candidate_annotations.push(annotation);
            output.candidate_sources.push(candidate.source as u8);
            output
                .candidate_positions
                .push(candidate.fixed_position as u8);
            output
                .candidate_corrected
                .push(!candidate.corrected_from.is_empty());
            // A short vector would be an engine bug. False is the safe reading of one: a consumer that sees nothing answering the key declines to reorder (bridge.cpp:933-937).
            output.candidate_answers_key.push(
                value
                    .candidate_answers_key
                    .get(index)
                    .copied()
                    .unwrap_or(false),
            );
            output
                .candidate_static_evidence
                .push(candidate.static_evidence());
            if touch_supported {
                output
                    .candidate_touch_supported
                    .push(candidate.touch_supported);
            }
            if confident {
                output
                    .candidate_confident_correction
                    .push(candidate.confident_correction);
            }
            output.candidate_codes.push(candidate.pinyin.into_string());
            output.candidates.push(candidate.word.into_string());
        }
        Ok(output)
    }

    /// `available = false` and defaults when there is no query.
    pub fn online_query(&self) -> Result<OnlineQuerySnapshot> {
        let Some(query) = self.inner.online_query() else {
            return Ok(OnlineQuerySnapshot::default());
        };
        Ok(OnlineQuerySnapshot {
            available: true,
            mode: query.mode,
            scheme: query.scheme as u8,
            generation: query.generation,
            identity: query.identity,
            query_text: query.query_text,
            cache_key: query.cache_key,
            pinyin_segments: query.pinyin_segments,
            cloud_eligible: query.cloud_eligible,
            ai_eligible: query.ai_eligible,
            session_id: query.session_id,
        })
    }

    pub fn reset_cache(&mut self) {
        self.inner.reset_cache();
    }

    /// Remove cached rows for one online provider (host source 0 = cloud, 1 = AI).
    pub fn clear_online_candidates(&mut self, source: u8) -> Result<()> {
        let source = match source {
            0 => CandidateSource::CloudSuggestion,
            1 => CandidateSource::AiSuggestion,
            _ => return Err(EngineError::invalid("invalid online candidate source")),
        };
        self.inner.clear_online_candidates(source);
        Ok(())
    }

    /// 读取当前云席位开关，供运行时在修改候选前预留新代次。
    pub fn cloud_candidates_enabled(&self) -> bool {
        self.inner.cloud_candidates_enabled()
    }

    pub fn set_cloud_candidates_enabled(&mut self, enabled: bool) {
        self.inner.set_cloud_candidates_enabled(enabled);
    }

    pub fn english_online_candidates_enabled(&self) -> bool {
        self.inner.english_online_candidates_enabled()
    }

    pub fn set_english_online_candidates_enabled(&mut self, enabled: bool) {
        self.inner.set_english_online_candidates_enabled(enabled);
    }

    pub fn set_caret(&mut self, caret: Option<usize>) {
        self.inner.set_caret(caret);
    }

    pub fn prefix_end(&self) -> usize {
        self.inner.prefix_end()
    }

    pub fn pending_suffix(&self) -> String {
        self.inner.pending_suffix()
    }

    pub fn reset_context(&mut self) {
        self.inner.reset_context();
    }

    /// Replace the `/` mode's command table live; `EngineOptions::command_table` is what a rebuilt session starts with.
    pub fn set_command_table(&mut self, table: &[CommandTableEntry]) -> Result<()> {
        match self.inner.set_command_table(table) {
            Some(diagnostic) => Err(EngineError::failed(&diagnostic)),
            None => Ok(()),
        }
    }

    /// 实时替换脚本插件的 `/` 指令，清掉缓存的插件行；`EngineOptions::plugin_commands` 是重建的会话的起点。
    pub fn set_plugin_commands(&mut self, commands: &[PluginCommand]) -> Result<()> {
        match self.inner.set_plugin_commands(commands) {
            Some(diagnostic) => Err(EngineError::failed(&diagnostic)),
            None => Ok(()),
        }
    }

    /// 生效的插件指令里 `trigger` 的标题：引擎丢掉用不了的、重复的插件指令之后留下的那一条。插件指令行（来源 14）的 `pinyin` 就是它的触发词。
    pub fn plugin_command_title(&self, trigger: &str) -> Option<&str> {
        self.inner.plugin_command_title(trigger)
    }

    /// 实时替换宿主给的辅助码表（辅助码表插件）；`None` 回到 `helpcode_schema` 对应的表。重建的会话从 `EngineOptions::helpcode_table` 开始。
    ///
    /// 从不失败：回退的表读不出来（典型是 `custom/<stem>` 的文件已被删掉）时装上空表并记一条日志。宿主在获得焦点时调用它，这里报错会让每一次聚焦都失败。
    pub fn set_helpcode_table(&mut self, table: Option<SharedKeymap>) -> Result<()> {
        let keymap = match &table {
            Some(table) => table.clone(),
            None => Arc::new(
                load_helpcode_keymap_with_fallback(
                    Path::new(&self.options.resources),
                    &self.options.helpcode_schema,
                    self.options
                        .helpcode_fallback_resources
                        .as_deref()
                        .map(Path::new),
                )
                .unwrap_or_else(|error| {
                    crate::logging::warn(format_args!(
                        "helpcode schema {} unavailable, using an empty table: {error}",
                        self.options.helpcode_schema
                    ));
                    HelpcodeKeymap::default()
                }),
            ),
        };
        self.inner.set_helpcode_table(keymap);
        self.options.helpcode_table = table;
        Ok(())
    }

    /// 实时替换 K 模式的宿主短语表；重建的会话从 `EngineOptions::quick_phrase_table` 开始。
    pub fn set_quick_phrase_table(&mut self, table: &[QuickPhraseEntry]) -> Result<()> {
        match self.inner.set_quick_phrase_table(table) {
            Some(diagnostic) => Err(EngineError::failed(&diagnostic)),
            None => Ok(()),
        }
    }

    /// Replace the `@` mode's list live; `EngineOptions::mention_entries` is what a rebuilt session starts with.
    pub fn set_mention_entries(&mut self, entries: &[MentionEntry]) -> Result<()> {
        match self.inner.set_mention_entries(entries) {
            Some(diagnostic) => Err(EngineError::failed(&diagnostic)),
            None => Ok(()),
        }
    }

    /// Offer the Chinese administrative divisions in `@` mode after the user's list. Off in a new session, so a host that has the switch on sets it on every session it builds, as it does nine-key mode.
    pub fn set_mention_places(&mut self, enabled: bool) -> Result<()> {
        match self.inner.set_mention_places(enabled) {
            Some(diagnostic) => Err(EngineError::failed(&diagnostic)),
            None => Ok(()),
        }
    }

    /// Live update of the neural rescoring context without a rebuild.
    pub fn set_rescoring_context(&mut self, context: &str) {
        self.inner.set_rescoring_context(context);
    }

    /// Live update of the first page size without a rebuild; it only moves the longer-phrase gate.
    pub fn set_first_page_rows(&mut self, rows: usize) {
        self.inner.set_first_page_rows(FirstPageRows::new(rows));
    }

    /// Whether an apostrophe now is input (`3jin'g`, `/fyhello'world`) rather than punctuation that ends the composition.
    pub fn takes_local_separator(&self) -> bool {
        self.inner.takes_local_separator()
    }

    /// The `/fy` request for the user's translation service, the only one a local mode makes; `None` whenever nothing asks for one.
    pub fn command_translation_query(&self) -> Option<CommandTranslationQuery> {
        self.inner.command_translation_query()
    }

    /// Puts the translation first in the `/fy` list; false for a stale query or text a row cannot show.
    pub fn apply_command_translation(
        &mut self,
        query: &CommandTranslationQuery,
        translation: &str,
    ) -> bool {
        self.inner.apply_command_translation(query, translation)
    }

    /// 插件指令要宿主去问的那一次；没有要问的时候是 `None`（不在 `/` 模式、输入不属于插件指令、这份预编辑串已经问过）。
    pub fn plugin_command_query(&self) -> Option<PluginCommandQuery> {
        self.inner.plugin_command_query()
    }

    /// 把插件给的行（文本，插件注释）放到 `/` 列表前面；过期的查询返回 false。
    pub fn apply_plugin_command_rows(
        &mut self,
        query: &PluginCommandQuery,
        rows: &[(String, String)],
    ) -> bool {
        self.inner.apply_plugin_command_rows(query, rows)
    }

    /// False for an unavailable query or a source other than 0 (cloud) and 1 (AI).
    pub fn apply_online_candidate(
        &mut self,
        query: &OnlineQuerySnapshot,
        candidate: &str,
        source: u8,
    ) -> Result<bool> {
        let Some((request, source)) = online_request(query, source) else {
            return Ok(false);
        };
        Ok(self
            .inner
            .apply_online_candidate(&request, candidate, source))
    }

    pub fn apply_online_candidates(
        &mut self,
        query: &OnlineQuerySnapshot,
        candidates: &[String],
        source: u8,
    ) -> Result<bool> {
        let Some((request, source)) = online_request(query, source) else {
            return Ok(false);
        };
        Ok(self
            .inner
            .apply_online_candidates(&request, candidates, source))
    }

    /// 九宫格组码到上限时，这一键能不能先选掉首选再接着打；见 `Session::grid_limit_pick_ready`。
    pub fn grid_limit_pick_ready(&self, value: u8, grid_key: bool) -> bool {
        self.inner.grid_limit_pick_ready(value, grid_key)
    }

    /// 由引擎判断第五码是否继续混输拼音，宿主不复制读音规则。
    pub fn wubi_pinyin_continues(&self, value: u8, shift: bool) -> bool {
        self.inner.wubi_pinyin_continues(value, shift)
    }

    /// 五笔拼音混输开着时，顶字关掉才有地方容纳第五个字母。
    pub fn wubi_mixed_pinyin(&self) -> bool {
        self.inner.wubi_mixed_pinyin()
    }

    /// `CHARACTER_MUST_BE_ASCII` above 127.
    pub fn character(&mut self, value: u8, shift: bool) -> Result<EngineResult> {
        require_ascii(value, diagnostics::CHARACTER_MUST_BE_ASCII)?;
        Ok(result_for(self.inner.character(value, shift)))
    }

    /// 下一次 `character` 的按下点，见 [`crate::Session::set_key_touch`]。
    pub fn set_key_touch(&mut self, touch: Option<KeyTouch>) {
        self.inner.set_key_touch(touch);
    }

    /// 宿主注册或清除的键位几何，见 [`crate::Session::set_key_geometry`]。
    pub fn set_key_geometry(&mut self, geometry: Option<Arc<KeyGeometry>>) {
        self.inner.set_key_geometry(geometry);
    }

    /// 按触点计价的参数，只给 Rust 侧的评测调参用，不经 FFI；见 [`crate::touch::TouchTuning`]。
    pub fn set_touch_tuning(&mut self, tuning: TouchTuning) {
        self.inner.set_touch_tuning(tuning);
    }

    pub fn expand_initial_candidates(&mut self) -> Result<bool> {
        Ok(self.inner.expand_initial_candidates())
    }

    pub fn set_nine_key_enabled(&mut self, enabled: bool) -> Result<()> {
        self.set_key_grid(enabled.then_some(KeyGrid::NineKey))
    }

    /// 见 [`crate::Session::set_key_grid`]。快照的 `nine_key` 只在九键下为真。
    pub fn set_key_grid(&mut self, grid: Option<KeyGrid>) -> Result<()> {
        self.inner.set_key_grid(grid);
        Ok(())
    }

    /// 14 键的一键，见 [`crate::Session::grid_key`]；不是当前网格的字母时不处理，状态不变。
    pub fn grid_key(&mut self, letter: u8) -> Result<EngineResult> {
        Ok(result_for(self.inner.grid_key(letter)))
    }

    /// 滑行的一笔，见 [`crate::Session::glide`]。
    pub fn glide(
        &mut self,
        keyboard: &GlideKeyboard,
        points: &[GlidePoint],
    ) -> Result<EngineResult> {
        Ok(result_for(self.inner.glide(keyboard, points)))
    }

    /// Out of range is unhandled.
    pub fn choose_nine_key_spelling(&mut self, index: usize) -> Result<EngineResult> {
        Ok(result_for(self.inner.choose_nine_key_spelling(index)))
    }

    /// 见 [`crate::Session::set_nine_key_filter`]。
    pub fn set_nine_key_filter(
        &mut self,
        single_character: bool,
        strokes: &str,
    ) -> Result<EngineResult> {
        Ok(result_for(
            self.inner.set_nine_key_filter(single_character, strokes),
        ))
    }

    /// `CommitRaw` 与 `CommitRawWithoutLearning` 交给核心同名的命令：回车学英文词的策略在核心里（`SessionOptions::learn_typed_spellings_on_enter`，产品打开），门面只换编号。
    pub fn command(&mut self, command: Command) -> Result<EngineResult> {
        match command {
            Command::CommitRawWithoutLearning => Ok(result_for(
                self.inner
                    .command(crate::types::Command::CommitRawWithoutLearning),
            )),
            Command::ConvertHanja => Ok(result_for(
                self.inner.command(crate::types::Command::ConvertHanja),
            )),
            Command::ConversionLeft => Ok(result_for(
                self.inner.command(crate::types::Command::ConversionLeft),
            )),
            Command::ConversionRight => Ok(result_for(
                self.inner.command(crate::types::Command::ConversionRight),
            )),
            _ => {
                // The other host codes are the engine's ordinals one for one (bridge.cpp:1302-1319).
                let engine = crate::types::Command::from_u8(command as u8)
                    .ok_or_else(|| EngineError::invalid(diagnostics::UNSUPPORTED_INPUT_COMMAND))?;
                Ok(result_for(self.inner.command(engine)))
            }
        }
    }

    /// 整句改字的左移，进入改字时从第 `index` 个候选开始（`crate::session::Session::conversion_left_from`）。
    pub fn conversion_left_from(&mut self, index: usize) -> Result<EngineResult> {
        Ok(result_for(self.inner.conversion_left_from(index)))
    }

    pub fn select(&mut self, index: usize) -> Result<EngineResult> {
        Ok(result_for(self.inner.select(index)))
    }

    pub fn pin_candidate(&mut self, index: usize) -> Result<EngineResult> {
        Ok(result_for(self.inner.pin(index)))
    }

    pub fn remove_candidate(&mut self, index: usize) -> Result<EngineResult> {
        Ok(result_for(self.inner.remove(index)))
    }

    /// `INVALID_CANDIDATE_POSITION` unless 1..=5.
    pub fn fix_candidate_position(&mut self, index: usize, position: u8) -> Result<EngineResult> {
        if !(1..=5).contains(&position) {
            return Err(EngineError::invalid(
                diagnostics::INVALID_CANDIDATE_POSITION,
            ));
        }
        Ok(result_for(
            self.inner.fix_position(index, i32::from(position)),
        ))
    }

    pub fn clear_candidate_position(&mut self, index: usize) -> Result<EngineResult> {
        Ok(result_for(self.inner.clear_position(index)))
    }

    pub fn select_edge(&mut self, index: usize, edge: CandidateEdge) -> Result<EngineResult> {
        Ok(result_for(self.inner.select_edge(index, edge)))
    }

    pub fn finish(&mut self, index: usize) -> Result<EngineResult> {
        Ok(result_for(self.inner.finish(index)))
    }

    /// `PUNCTUATION_MUST_BE_ASCII` above 127.
    pub fn punctuation(&mut self, value: u8) -> Result<EngineResult> {
        require_ascii(value, diagnostics::PUNCTUATION_MUST_BE_ASCII)?;
        Ok(result_for(self.inner.punctuation(value)))
    }

    pub fn balance_paired_punctuation_after_auto_close(&mut self, opening: u8) -> Result<()> {
        require_ascii(opening, diagnostics::PAIRED_OPENING_MUST_BE_ASCII)?;
        self.inner
            .balance_paired_punctuation_after_auto_close(opening);
        Ok(())
    }

    pub fn set_chinese_punctuation_enabled(&mut self, enabled: bool) -> Result<()> {
        self.inner.set_chinese_punctuation_enabled(enabled);
        Ok(())
    }

    pub fn set_punctuation_lock(&mut self, lock: u8) -> Result<()> {
        self.inner.set_punctuation_lock(i32::from(lock))
    }

    pub fn set_paired_punctuation_enabled(&mut self, enabled: bool) -> Result<()> {
        self.inner.set_paired_punctuation_enabled(enabled);
        Ok(())
    }

    pub fn set_dedicated_english(&mut self, enabled: bool) -> Result<()> {
        self.inner.set_dedicated_english(enabled);
        Ok(())
    }
}

/// Whether a commit made in `local_mode` (a `local_mode` name, as `EngineSnapshot` carries it) counts as typing. Text the expression, command and mention modes generated does not; an unknown name does, as every mode did before those.
pub fn local_mode_counts_as_typing(local_mode: &str) -> bool {
    LocalInputMode::from_name(local_mode).is_none_or(|mode| !mode.generates_text())
}

/// `result_for` (bridge.cpp:408-410): an empty commit with `has_commit = true` stays representable.
fn result_for(value: KeyResult) -> EngineResult {
    EngineResult {
        handled: value.handled,
        has_commit: value.commit.is_some(),
        commit: value.commit.unwrap_or_default(),
        diagnostic: value.diagnostic.unwrap_or_default(),
    }
}

fn require_ascii(value: u8, message: &str) -> Result<()> {
    if value > 127 {
        return Err(EngineError::invalid(message));
    }
    Ok(())
}

/// The query handed back field by field (bridge.cpp:977-1017). `None` for an unavailable query, a source other than cloud or AI, or a scheme ordinal no session could have issued, which is a stale query rather than an error (api-contract §5.3).
fn online_request(
    query: &OnlineQuerySnapshot,
    source: u8,
) -> Option<(OnlineQuery, CandidateSource)> {
    if !query.available {
        return None;
    }
    let source = match source {
        0 => CandidateSource::CloudSuggestion,
        1 => CandidateSource::AiSuggestion,
        _ => return None,
    };
    let request = OnlineQuery {
        mode: query.mode,
        scheme: SchemeType::from_u8(query.scheme)?,
        generation: query.generation,
        identity: query.identity.clone(),
        query_text: query.query_text.clone(),
        cache_key: query.cache_key.clone(),
        pinyin_segments: query.pinyin_segments.clone(),
        cloud_eligible: query.cloud_eligible,
        ai_eligible: query.ai_eligible,
        session_id: query.session_id,
    };
    Some((request, source))
}

#[cfg(test)]
#[path = "session/annotation_move_tests.rs"]
mod annotation_move_tests;

#[cfg(test)]
#[path = "session/grid_annotation_tests.rs"]
mod grid_annotation_tests;
