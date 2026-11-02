//! `Runtime`: the orchestration between keystrokes, the engine, and everything the
//! providers return asynchronously.

use super::*;
use msime_engine::ordering::{
    apply_order_with, ensure_engine_order, rerank_needs_model, rerank_pick, rotate_to_front,
    runner_up_order_with, CandidateScorer, ParallelOrderRows, StaticEvidence,
};
use msime_engine::touch::{KeyGeometry, KeyTouch};
// 排序决策搬进了 `msime_engine::ordering`；`tests.rs` 仍按原来的 crate 内名字引用这几项，这里为它们重新导出。
#[cfg(test)]
pub(crate) use msime_engine::ordering::{
    reorders_candidates as runtime_reorders_candidates, LATTICE_SOURCE,
};
use msime_engine::SchemeType;
use std::collections::HashSet;
use std::sync::Arc;

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
    /// 14 键的一键，`a`–`z` 里这一组的任一字母（宿主约定送首字母）。不在 14 键下时不处理。
    GridKey(u8),
    /// 九宫格候选的筛选：只留单字，以及首字笔顺的前缀（`hspnz`，空表示不按笔画）。没有九宫格组字时不处理。
    SetNineKeyFilter {
        single_character: bool,
        strokes: String,
    },
    /// 滑过字母键的一笔，用宿主自己的坐标。
    Glide {
        keyboard: Box<GlideKeyboard>,
        points: Vec<GlidePoint>,
    },
    SelectHighlighted,
    Finish,
    /// 焦点即将离开，宿主替用户结束组字（命令 20）。同 [`Action::Finish`]，只有一处不同：高亮行是插件指令行（来源 14）时原样上屏预编辑串（`/uuid`），因为这一次用户没在看候选窗。按键结束（`Finish`、标点）照旧上屏高亮行，和 `/rq` 一致。
    FinishOnFocusLoss,
    NextPage,
    PreviousPage,
    NextCandidate,
    PreviousCandidate,
    FirstCandidate,
    LastCandidate,
    /// Home 键（命令 106）：高亮不在首选时回到首选，已经在首选、输入串可以编辑（没有打开的候选列表、不在整句改字）时把光标移到开头。由 [`Runtime::home_key_action`] 换成 [`Action::FirstCandidate`] 或 `MoveHome`，不会走到别处。
    FirstCandidateOrHome,
}

/// What one selection of a phrase-in-progress can be taken back to.
///
/// `word_before` is the whole held phrase as it stood before the selection, not just the piece it
/// added: the reference restores its accumulated word wholesale for the same reason - a selection
/// that recorded nothing sits between two that did, and only the whole word puts them all back.
pub(crate) struct PhraseSelection {
    pub(crate) word_before: String,
    pub(crate) reading: String,
    /// 选词时正在组字的网格；`None` 是 26 键组字。退格收回这次选词时按它把读音打回去：14 键的读音是组码字母，要从 `grid_key` 进，从 `character` 进会变成全拼 26 键的字母。
    pub(crate) grid: Option<KeyGrid>,
}

pub struct Runtime<E: InputEngine = Session> {
    pub(crate) engine: E,
    pub(crate) session: u64,
    pub(crate) generation: u64,
    pub(crate) focused: bool,
    pub(crate) page_size: usize,
    pub(crate) highlighted: usize,
    /// 画在候选下面的释义，按候选文字记。「仅当前高亮」生效时这里只放不受范围限制的离线释义（宿主标了 `gloss` 的回答，以及不是高亮那一行的回答），候选词翻译放在 [`Self::highlight_translations`]。
    pub(crate) translations: HashMap<String, String>,
    /// 「仅当前高亮」生效时，高亮那一行收到的候选词翻译，按候选文字记，只画在高亮的那一行。高亮移开时保留，同一次组字里移回来不用再等回答；组字变化时和 [`Self::translations`] 一起清掉。
    pub(crate) highlight_translations: HashMap<String, String>,
    /// 候选词翻译开着且范围是「仅当前高亮」，由 host-api 按偏好设置，见 [`Self::set_translation_scope_highlighted`]。
    pub(crate) translation_scope_highlighted: bool,
    /// 宿主实际显示的一组候选（按宽度自适应排版的宿主，一行未必等于固定的一页）：设置时那一代和各座位的序号。
    /// 只对那一代有效；没设置或已过期时，选词快捷键和翻译查询仍按高亮所在的固定页。
    pub(crate) display_page: Option<(u64, Vec<usize>)>,
    pub(crate) cached: EngineSnapshot,
    /// The Engine's own index for each seat of `cached`.
    ///
    /// `rerank`, `demote_runner_up_readings` and `normalize_online_slots` reorder the cached list, but the Engine knows nothing of that and selects by its own order. Every call that names a candidate to the Engine goes through [`Self::engine_index`]; without it, an AI candidate seated in slot 1 committed whatever the Engine held at 1.
    pub(crate) engine_order: Vec<usize>,
    /// 在线候选座位整理的排列与固定英文临时缓冲，跨次刷新保留容量。
    pub(crate) online_order_scratch: Vec<usize>,
    pub(crate) online_fixed_english_scratch: Vec<usize>,
    /// 落定重排重放插件操作时的座位顺序比较缓冲，跨次调用保留容量。
    pub(crate) settled_order_scratch: Vec<usize>,
    pub(crate) snapshot_valid: bool,
    pub(crate) character_width: CharacterWidth,
    pub(crate) punctuation_marks: PunctuationMarks,
    pub(crate) touch_keyboard_layout: TouchKeyboardLayout,
    /// 触屏宿主注册的字母键几何（`msime_client_set_key_geometry`）。由 runtime 持有而不是只交给引擎：宿主空闲时会重建引擎（`replace_engine`），新引擎要重新拿到它，否则触点会悄悄失效。
    pub(crate) key_geometry: Option<Arc<KeyGeometry>>,
    /// 整个候选列表只给出一个词的完整五笔码，是否在第四键提交。
    ///
    /// *什么*算这种码由 Engine 判定（[`EngineSnapshot::wubi_unique_four_code`]）；提交它是宿主的事，因为提交要跨过平台自己的组字边界。只有用户要求时才关：关了以后词和其他候选一样留在候选列表里，由空格或数字键选走。
    pub(crate) wubi_auto_commit_unique: bool,
    /// 完整四码后再敲字母时，是否把首选顶上去、并把这个字母留作下一组开头。
    ///
    /// 默认开，与来源出厂一致，也是接入这个开关之前唯一可能的行为。关掉只在混输打开时生效：那个字母并进混输串；混输关掉时它无处容纳，仍按顶字上屏，一个键都不丢。
    pub(crate) wubi_fifth_code_top_commit: bool,
    pub(crate) wubi_candidate_selection:
        msime_client_core::preferences::WubiCandidateSelectionPreferences,
    /// 通用「二三候选」键（`second_third_candidate`）：组字时 `;` 选当前页第二个、`'` 选第三个。
    /// 宿主编排状态，与 `wubi_candidate_selection` 同在 `candidate_shortcut` 拦截。
    pub(crate) second_third_candidate:
        msime_client_core::preferences::SecondThirdCandidatePreferences,
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
    /// 普通退格在拼写末尾是否回退已选词段；分段删除不受此开关影响。
    pub(crate) backspace_reverts_selection: bool,
    /// 造词时按 Esc 是否只丢掉还没选的拼写、保留已经选好的词（Windows `input.escape_keeps_selected_word`）。默认关：Esc 连已选词一起丢掉，与旧行为一致。
    pub(crate) escape_keeps_selected_word: bool,
    /// 左右方向键在光标已经停在输入串端点、编辑器动不了它时改成移动候选高亮（偏好 `navigation.left_right`）。和 `backspace_reverts_selection` 一样是宿主状态：判断在 runtime 里做，宿主只照常发光标命令。
    pub(crate) left_right_navigation: bool,
    /// 候选窗是竖排：左右键与候选的排列方向垂直，只编辑输入串，`left_right_navigation` 不接管。由 host-api 按偏好 `candidate_layout` 设，只有桌面宿主会是竖排，触屏的候选栏总是横排。
    pub(crate) vertical_candidates: bool,
    /// 中文状态下首页最多展示几个英文候选（`mixed_input.first_page_count`，1–3）：在线座位整理时英文组在头段的留位数，第一行总能留位。
    pub(crate) english_first_page_count: usize,
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
    pub(crate) reranker: Option<Box<dyn CandidateScorer>>,
    /// A second, larger model run once the user stops typing, when one is attached.
    ///
    /// Capacity is the most effective lever the model has — the 24M preset beats the 6.8M one by
    /// 49 points of top-1 on the harvested failure set — and it is also the one the keystroke path
    /// cannot afford: the same model measures p95 153ms against a 16ms frame, with the slowest
    /// keystroke at 342ms. Both numbers are real and they do not have to be reconciled, because
    /// they are answers to different questions. While the user is typing, the first row has to be
    /// plausible now; when the user stops to read the candidates, it has to be right. The fast
    /// model owns the first job and this one owns the second.
    pub(crate) settled_reranker: Option<Box<dyn CandidateScorer>>,
    /// Whether [`Runtime::rerank_settled`] runs the settled model: the product's desktop sentence model switch. On unless a host turns it off, so a caller that attaches a model and says nothing else gets it.
    pub(crate) settled_rerank_enabled: bool,
    // 开关和模型分别拥有不复用的身份，防止在途评分跨配置发布或恢复旧模型。
    settled_settings_epoch: Arc<()>,
    settled_model_epoch: Arc<()>,
    /// 最近一次 `dispatch` 或 `focus` 的旁路事实，见 [`DispatchFacts`]。
    pub(crate) facts: DispatchFacts,
    /// `facts.selection` 那个座位上候选的文本，用引擎调用之前的快照记下；没有选择时为空。`DispatchFacts` 要保持 `Copy`，文本单独放在这里，缓冲区在调用之间复用。
    pub(crate) selected_text: String,
    /// 混合行按语言段输出的简繁偏好与本次选择；普通候选保持原有宿主转换路径。
    pub(crate) multilingual_output: (
        bool,
        msime_client_core::chinese_conversion::TraditionalVariant,
    ),
    pub(crate) selected_multilingual: Option<crate::multilingual::SelectedMultilingual>,
    /// 组字（预编辑、读音、光标、整句改字、暂存词段）每变一次加一。宿主在一次调用前后各读一次，不同就说明组字变了；只看 refresh 里的比较不够，暂存词段不经过引擎快照。
    pub(crate) composition_epoch: u64,
    /// 候选视图（列表、注释、来源、高亮、页、释义）每变一次加一。高亮移动和翻页在 refresh 之前就返回，所以单独计数。
    pub(crate) candidates_epoch: u64,
    /// 脚本插件的候选变换，见 `transform` 模块。只在桌面编译。
    #[cfg(any(
        target_os = "macos",
        target_os = "windows",
        all(target_os = "linux", not(target_env = "ohos"))
    ))]
    pub(crate) plugins: crate::transform::PluginState,
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

/// 「二三候选」键在这个方案的组字里能不能选候选。日文把 `;`/`'` 用在罗马字输入里（`n'` 打出ん）；
/// 韩文、注音、越南文、藏文由宿主自己组字或自己开候选列表。与 Windows
/// `SecondThirdCandidatePolicy.h` 的 `second_third_candidate_scheme` 同一张表：
/// 方案号未知时不接管的两边都不算，照样接管。
fn second_third_candidate_scheme(scheme: u8) -> bool {
    !matches!(
        scheme_type(scheme),
        Some(
            SchemeType::JapaneseRomaji
                | SchemeType::Korean
                | SchemeType::Zhuyin
                | SchemeType::Vietnamese
                | SchemeType::Tibetan
        )
    )
}

/// 微软双拼（及零声母编码用 `;` 的方案）把 `;` 当韵母 ing：光标前、自上一个 `'` 起的片段
/// 长度为奇数（刚打完声母）时它是输入。与 Windows `microsoft_shuangpin_final_position`
/// 和引擎 `accepts_ing_key` 同一条判断；`editing` 是 ASCII 组字原文，`caret` 是它的字节偏移。
fn microsoft_shuangpin_final_position(editing: &[u8], caret: usize) -> bool {
    let caret = caret.min(editing.len());
    let mut start = caret;
    while start > 0 && editing[start - 1] != b'\'' {
        start -= 1;
    }
    (caret - start) % 2 == 1
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

/// 稀疏释义不为全候选列表建集合；任一侧最多 8 条时，比较次数仍受小侧上限约束。
fn retain_candidate_translations(
    translations: &mut HashMap<String, String>,
    candidates: &[String],
) {
    if translations.is_empty() {
        return;
    }
    if candidates.is_empty() {
        translations.clear();
    } else if translations.len() <= 8 || candidates.len() <= 8 {
        translations.retain(|text, _| candidates.contains(text));
    } else {
        let candidates: HashSet<&str> = candidates.iter().map(String::as_str).collect();
        translations.retain(|text, _| candidates.contains(text.as_str()));
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
    .with_static_evidence(&snapshot.candidate_static_evidence)
}

/// 复制候选行，供停顿模型在会话线程之外评分。
struct SettledRerankRows {
    texts: Vec<String>,
    sources: Vec<u8>,
    answers_key: Vec<bool>,
    corrected: Vec<bool>,
    static_evidence: Vec<Option<StaticEvidence>>,
}

/// 脱离会话借用的停顿评分请求；模型随请求转移，必须经
/// [`Runtime::apply_settled_rerank`] 或 [`Runtime::discard_settled_rerank`] 归还。
pub struct SettledRerankRequest {
    generation: u64,
    settings_epoch: Arc<()>,
    model_epoch: Arc<()>,
    context: String,
    scheme: u8,
    answered_by_pinyin_fallback: bool,
    rows: SettledRerankRows,
    reranker: Box<dyn CandidateScorer>,
}

/// [`score_settled_rerank`] 产生的模型和待提升的候选行。
pub struct SettledRerankScore {
    generation: u64,
    settings_epoch: Arc<()>,
    model_epoch: Arc<()>,
    promote: Option<usize>,
    reranker: Box<dyn CandidateScorer>,
}

/// [`Runtime::apply_settled_rerank`] 对评分的发布结果。
pub struct SettledRerankApplied {
    /// 评分仍属于当前组字和配置；按键、开关或模型改变后为假且不重排。
    /// 为真表示本次组字已评分，无论顺序是否变化。
    pub current: bool,
    /// 已发布的候选顺序发生变化。
    pub moved: bool,
}

/// 计算 [`Runtime::begin_settled_rerank`] 取出的请求；拥有模型及候选副本，
/// 不访问会话，可以在任意线程调用。
pub fn score_settled_rerank(mut request: SettledRerankRequest) -> SettledRerankScore {
    let promote = {
        let rows = ParallelOrderRows::new(
            &request.rows.texts,
            &request.rows.sources,
            &request.rows.answers_key,
            &request.rows.corrected,
        )
        .with_static_evidence(&request.rows.static_evidence);
        rerank_pick(
            request.reranker.as_mut(),
            &request.context,
            request.scheme,
            request.answered_by_pinyin_fallback,
            &rows,
        )
    };
    SettledRerankScore {
        generation: request.generation,
        settings_epoch: request.settings_epoch,
        model_epoch: request.model_epoch,
        promote,
        reranker: request.reranker,
    }
}

impl Runtime<Session> {
    /// 打开或关闭云候选固定席位；宿主偏好变化时立即刷新当前组字。
    pub fn set_cloud_candidates_enabled(&mut self, enabled: bool) -> Result<(), RuntimeError> {
        if self.engine.cloud_candidates_enabled() == enabled {
            return Ok(());
        }
        // 开关会插入或移除云占位行，必须在修改引擎前使旧候选身份失效。
        self.advance()?;
        self.engine.set_cloud_candidates_enabled(enabled);
        self.refresh()
    }

    /// 英文在线授权的改变立即失效旧查询，并刷新已经显示的英文列表。
    pub fn set_english_online_candidates_enabled(
        &mut self,
        enabled: bool,
    ) -> Result<(), RuntimeError> {
        if self.engine.english_online_candidates_enabled() == enabled {
            return Ok(());
        }
        self.advance()?;
        self.engine.set_english_online_candidates_enabled(enabled);
        self.refresh()
    }

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
            mode: query.mode,
            scheme: query.scheme,
            generation: query.generation,
            identity: query.identity,
            query_text: query.query_text,
            cache_key: query.cache_key,
            pinyin_segments: query.pinyin_segments,
            cloud_eligible: query.cloud_eligible,
            ai_eligible: query.ai_eligible,
            cloud_candidates: true,
            cloud_request: None,
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
            mode: query.mode,
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
            mode,
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
            mode,
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
        if self.generation == u64::MAX {
            return Err(RuntimeError::IdentityExhausted);
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
            self.refresh_online_candidates()
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
            mode: query.mode,
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
            mode,
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
            mode,
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

    /// Remove cached and visible rows for one online provider.
    pub fn clear_online_candidates(&mut self, source: u8) -> Result<(), RuntimeError> {
        if source > 1 {
            return Err(RuntimeError::Engine(
                "invalid online candidate source".into(),
            ));
        }
        if self.generation == u64::MAX {
            return Err(RuntimeError::IdentityExhausted);
        }
        self.engine
            .clear_online_candidates(source)
            .map_err(|error| RuntimeError::Engine(error.to_string()))?;
        self.advance()?;
        self.refresh_online_candidates()
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
        if self.generation == u64::MAX {
            return Err(RuntimeError::IdentityExhausted);
        }
        let applied = self
            .engine
            .apply_online_candidates(&query, candidates, source)
            .map_err(|error| RuntimeError::Engine(error.to_string()))?;
        if applied {
            self.advance()?;
            self.refresh_online_candidates()
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

    /// 一次遍历排列，同步交换候选字段和引擎座位映射，共用循环访问缓冲。
    pub(crate) fn apply_candidate_order(&mut self, order: &[usize]) {
        ensure_engine_order(&mut self.engine_order, order.len());
        let snapshot = &mut self.cached;
        debug_assert_eq!(snapshot.candidates.len(), order.len());
        debug_assert_eq!(snapshot.candidate_codes.len(), order.len());
        debug_assert_eq!(snapshot.candidate_annotations.len(), order.len());
        debug_assert_eq!(snapshot.candidate_sources.len(), order.len());
        debug_assert_eq!(snapshot.candidate_positions.len(), order.len());
        debug_assert_eq!(snapshot.candidate_corrected.len(), order.len());
        debug_assert_eq!(snapshot.candidate_answers_key.len(), order.len());
        let engine_order = &mut self.engine_order;
        apply_order_with(order, |left, right| {
            snapshot.candidates.swap(left, right);
            snapshot.candidate_codes.swap(left, right);
            snapshot.candidate_annotations.swap(left, right);
            snapshot.candidate_sources.swap(left, right);
            snapshot.candidate_positions.swap(left, right);
            snapshot.candidate_corrected.swap(left, right);
            snapshot.candidate_answers_key.swap(left, right);
            if snapshot.candidate_static_evidence.len() == order.len() {
                snapshot.candidate_static_evidence.swap(left, right);
            }
            if snapshot.candidate_contest.len() == order.len() {
                snapshot.candidate_contest.swap(left, right);
            }
            if snapshot.candidate_touch_supported.len() == order.len() {
                snapshot.candidate_touch_supported.swap(left, right);
            }
            if snapshot.candidate_confident_correction.len() == order.len() {
                snapshot.candidate_confident_correction.swap(left, right);
            }
            engine_order.swap(left, right);
        });
    }

    /// 把当前候选座位对应的引擎下标写入可复用缓冲；引擎未建立排列时按座位顺序生成。
    pub(crate) fn copy_seat_order(&self, out: &mut Vec<usize>) {
        out.clear();
        if self.engine_order.len() == self.cached.candidates.len() {
            out.extend_from_slice(&self.engine_order);
        } else {
            out.extend(0..self.cached.candidates.len());
        }
    }

    /// 比较候选座位对应的引擎下标，不为一次落定重排临时建立排列向量。
    pub(crate) fn seat_order_matches(&self, expected: &[usize]) -> bool {
        if expected.len() != self.cached.candidates.len() {
            return false;
        }
        if self.engine_order.len() == expected.len() {
            expected == self.engine_order
        } else {
            expected
                .iter()
                .enumerate()
                .all(|(seat, &index)| seat == index)
        }
    }

    pub fn set_dedicated_english(&mut self, enabled: bool) -> Result<(), RuntimeError> {
        self.advance()?;
        self.engine.set_dedicated_english(enabled)?;
        // 引擎在模式翻转时取消读音，runtime 暂存的词段和撤回记录也属于同一段组字。先清理再刷新，刷新失败也不能留下旧词段；重复设置同一模式则保留。
        if self.cached.dedicated_english != enabled {
            self.phrase_selections.clear();
            if !self.phrase_prefix.is_empty() {
                self.phrase_prefix.clear();
                self.bump_composition_epoch();
            }
        }
        self.refresh()
    }

    /// 更新即时隐私门禁，并使之前显示的联想候选身份过期。
    pub fn set_prediction_allowed(&mut self, allowed: bool) -> Result<(), RuntimeError> {
        self.advance()?;
        self.engine.set_prediction_allowed(allowed);
        self.refresh()
    }

    /// 整个候选列表只回一个词的完整五笔码，是否在第四键提交。
    ///
    /// 这是宿主状态而不是 Engine 状态：它决定本 runtime 要不要执行提交，所以组字中途改也安全，不需要重建 Engine。下一键生效，因此已经打进组字的四码不会因为把开关关掉而自己上屏。
    pub fn set_wubi_auto_commit_unique(&mut self, enabled: bool) {
        self.wubi_auto_commit_unique = enabled;
    }

    /// 完整四码后再敲字母时是否顶字上屏。
    ///
    /// 宿主状态而不是 Engine 状态：它决定本 runtime 要不要先 `select(0)` 再把这个字母送进新组合，所以组字中途改也安全，下一键生效。
    pub fn set_wubi_fifth_code_top_commit(&mut self, enabled: bool) {
        self.wubi_fifth_code_top_commit = enabled;
    }

    pub fn wubi_fifth_code_top_commit(&self) -> bool {
        self.wubi_fifth_code_top_commit
    }

    /// 只影响下一次退格，不清空已选词段或历史，也不重建 Engine。
    pub fn set_backspace_reverts_selection(&mut self, enabled: bool) {
        self.backspace_reverts_selection = enabled;
    }

    /// 造词时第一次 Esc 只丢掉还没选的拼写、保留已经选好的词，再按一次才整体取消。只影响下一次 Esc，不清空已选词段或历史，也不重建 Engine。
    pub fn set_escape_keeps_selected_word(&mut self, enabled: bool) {
        self.escape_keeps_selected_word = enabled;
    }

    /// 左右键的候选回退（`navigation.left_right`）：只影响左右键这一次怎么解释，不动组字状态，也不重建 Engine。
    pub fn set_left_right_navigation(&mut self, enabled: bool) {
        self.left_right_navigation = enabled;
    }

    /// 首页英文候选上限（`mixed_input.first_page_count`）：1–3，越界按 1 处理；和 `set_left_right_navigation` 一样是宿主状态，下一次座位整理就生效。
    pub fn set_english_first_page_count(&mut self, count: u8) {
        self.english_first_page_count = usize::from(count.clamp(1, 3));
    }

    /// 候选窗是否竖排：和 `set_left_right_navigation` 一样只影响左右键这一次怎么解释。
    pub fn set_vertical_candidates(&mut self, vertical: bool) {
        self.vertical_candidates = vertical;
    }

    /// 选词字符是宿主编排状态，切换配置无需等待 Engine 重建。
    pub fn set_wubi_candidate_selection(
        &mut self,
        settings: msime_client_core::preferences::WubiCandidateSelectionPreferences,
    ) {
        self.wubi_candidate_selection = settings;
    }

    pub fn wubi_candidate_selection(
        &self,
    ) -> &msime_client_core::preferences::WubiCandidateSelectionPreferences {
        &self.wubi_candidate_selection
    }

    /// 「二三候选」键同样是宿主编排状态，切换配置无需等待 Engine 重建。
    pub fn set_second_third_candidate(
        &mut self,
        settings: msime_client_core::preferences::SecondThirdCandidatePreferences,
    ) {
        self.second_third_candidate = settings;
    }

    pub fn second_third_candidate(
        &self,
    ) -> &msime_client_core::preferences::SecondThirdCandidatePreferences {
        &self.second_third_candidate
    }

    /// 将已解码的选词字符转成当前页的候选动作，供统计与生命周期共用：五笔的自定义键在前，通用「二三候选」键在后。
    pub fn candidate_shortcut(&self, action: Action) -> Action {
        if !self.focused || self.cached.prediction {
            return action;
        }
        // 宿主显示页上的座位可能不在固定页里，普通选择只认固定页，所以按全部候选选择（同样核对代次与范围）。
        let shown = self.display_page_current();
        self.wubi_candidate_shortcut(&action)
            .or_else(|| self.second_third_candidate_shortcut(&action))
            .map(|id| {
                if shown {
                    Action::SelectAnyCandidate(id)
                } else {
                    Action::Select(id)
                }
            })
            .unwrap_or(action)
    }

    fn wubi_candidate_shortcut(&self, action: &Action) -> Option<CandidateId> {
        if !self.snapshot_valid
            || self.cached.scheme != SchemeType::Wubi as u8
            || self.cached.dedicated_english
            || self.cached.local_mode != "none"
            || self.cached.editing_text.is_empty()
        {
            return None;
        }
        let value = match action {
            Action::Character { value, .. }
            | Action::Punctuation(value)
            | Action::PunctuationAscii(value) => *value,
            _ => return None,
        };
        let slot = self.wubi_candidate_selection.slot(value)?;
        self.page_seat_candidate(slot)
    }

    /// 宿主报出这一代实际显示的候选座位：选词快捷键与翻译查询都按看得见的这一组，空列表清掉。
    /// 代次不是当前这一代或有座位越界时不改动，返回假。
    pub fn set_display_page(&mut self, generation: u64, indices: Vec<usize>) -> bool {
        if generation != self.generation {
            return false;
        }
        if indices.is_empty() {
            self.display_page = None;
            return true;
        }
        let visible = self.visible_len();
        if indices.iter().any(|&index| index >= visible) {
            return false;
        }
        self.display_page = Some((generation, indices));
        true
    }

    /// 宿主报的显示页是不是这一代的。
    fn display_page_current(&self) -> bool {
        self.display_page
            .as_ref()
            .is_some_and(|(generation, _)| *generation == self.generation)
    }

    /// 当前这一页的座位：宿主报了这一代的显示页就用它，否则是高亮所在的固定页。
    fn page_seats(&self) -> Vec<usize> {
        if let Some((_, indices)) = self
            .display_page
            .as_ref()
            .filter(|_| self.display_page_current())
        {
            return indices.clone();
        }
        let start = (self.highlighted / self.page_size) * self.page_size;
        (start..self.visible_len().min(start + self.page_size)).collect()
    }

    /// 当前页第 `slot` 个座位上的候选；这一页没有这么多座位时为空，键照原路处理。
    fn page_seat_candidate(&self, slot: usize) -> Option<CandidateId> {
        let index = *self.page_seats().get(slot)?;
        (index < self.visible_len()).then_some(CandidateId {
            session: self.session,
            generation: self.generation,
            index,
        })
    }

    /// 通用「二三候选」键：`;` 选当前页第二个、`'` 选第三个。边界与 `wubi_candidate_shortcut`
    /// 相同（快照有效、组字中、无本地模式、非英文模式），方案与输入排除照 Windows
    /// `SecondThirdCandidatePolicy.h` 的 `second_third_candidate_scheme` 与
    /// `second_third_candidate_engine_input`。
    fn second_third_candidate_shortcut(&self, action: &Action) -> Option<CandidateId> {
        if !self.second_third_candidate.enabled
            || !self.snapshot_valid
            || self.cached.dedicated_english
            || self.cached.local_mode != "none"
            || self.cached.editing_text.is_empty()
            || !second_third_candidate_scheme(self.cached.scheme)
        {
            return None;
        }
        let value = match action {
            Action::Character { value, .. }
            | Action::Punctuation(value)
            | Action::PunctuationAscii(value) => *value,
            _ => return None,
        };
        let slot = self.second_third_candidate.slot(value)?;
        // 引擎在当前状态下把这个键当输入时不接管：网址、V 模式的拼写符号，
        // 以及微软双拼声母后的韵母 ing。
        if self.cached.spelling_symbols.contains(char::from(value))
            || (value == b';'
                && self.cached.microsoft_shuangpin
                && microsoft_shuangpin_final_position(
                    self.cached.editing_text.as_bytes(),
                    self.cached.caret_position,
                ))
        {
            return None;
        }
        self.page_seat_candidate(slot)
    }

    /// 左右键的候选导航（`navigation.left_right`）：不带修饰键的左右键在宿主那里是编辑输入串的光标命令，这里按高亮决定这一次按键是编辑还是移动候选，移动候选与 ↑/↓ 落在同一段代码上（[`Action::PreviousCandidate`] / [`Action::NextCandidate`]）。
    ///
    /// 只在候选横排时接管：方向键沿着候选的排列方向移动高亮，竖排时由 ↑/↓ 移高亮，左右键只编辑输入串。
    ///
    /// 高亮在首选（全部候选里的第一个，不是当前页的第一个）时，← 编辑输入串；→ 在光标没到末尾时把光标往末尾送，到了末尾才选下一个。高亮不在首选时，左右键只移动候选，不再编辑输入串：离开首选的那一下光标就在末尾，用 ↓ 离开首选时光标可能停在中间，那时 → 也照样选下一个。
    ///
    /// 只在当前页有候选时接管，与 `candidate_shortcut` 同理：没有候选可移时左右键照旧交给引擎，于是仍被吞掉，不会作为应用的按键落进文档。
    ///
    /// 整句改字和已经打开的候选列表都不在这里接管。改字期间左右键移的是改字的光标，它不在输入串里（`caret_position` 停在末尾），交给引擎才对；打开的列表（韩文汉字、注音）本来就有自己的左右键规则，各宿主也先一步把四个方向键都归给高亮。
    fn caret_edge_candidate_shortcut(&self, action: Action) -> Action {
        if !self.left_right_navigation
            || self.vertical_candidates
            || !self.snapshot_valid
            || self.visible_len() == 0
            || self.cached.candidate_list_open
            || !self.cached.conversion.is_empty()
        {
            return action;
        }
        let backwards = match action {
            Action::Command(Command::MoveLeft | Command::ConversionLeft) => true,
            Action::Command(Command::MoveRight | Command::ConversionRight) => false,
            _ => return action,
        };
        let edits = self.highlighted == 0
            && (backwards || self.cached.caret_position < self.cached.editing_text.len());
        if edits {
            return action;
        }
        if backwards {
            Action::PreviousCandidate
        } else {
            Action::NextCandidate
        }
    }

    /// Home 键分两段：高亮不在首选时第一下回到首选，已经在首选时才把输入串的光标移到开头。打开的候选列表（韩文汉字、注音）里没有可编辑的光标，那里一直是回到首选。整句改字时也一直是回到首选：改字的光标不在输入串里，`MoveHome` 会让引擎退出改字、丢掉已经钉住的段，而刚进改字时高亮就在首选，按一下 Home 就会把改好的句子扔掉；End 在改字里同样只跳到末选。
    ///
    /// 宿主只在 Home 键上发这条命令；日文转换用 [`Action::FirstCandidate`] 循环回首选，不能因为高亮已在首选就去动读音的光标。
    fn home_key_action(&self, action: Action) -> Action {
        if !matches!(action, Action::FirstCandidateOrHome) {
            return action;
        }
        let caret_home = self.snapshot_valid
            && self.highlighted == 0
            && !self.cached.candidate_list_open
            && self.cached.conversion.is_empty()
            && !self.cached.editing_text.is_empty();
        if caret_home {
            Action::Command(Command::MoveHome)
        } else {
            Action::FirstCandidate
        }
    }

    pub fn wubi_auto_commit_unique(&self) -> bool {
        self.wubi_auto_commit_unique
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

    /// 把宿主算好的插件指令交给引擎；打开的 `/` 列表据此重建，所以视图要刷新。引擎缓存的插件行随之清掉。标题也由引擎保管，插件注释格画标题时向引擎要。
    pub fn set_plugin_commands(&mut self, commands: &[PluginCommand]) -> Result<(), RuntimeError> {
        self.advance()?;
        self.engine.set_plugin_commands(commands)?;
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

    /// `page_size` 既是分页，也作为首屏门槛交给引擎（`InputEngine::set_first_page_rows`），见 [`Self::set_page_size`]。
    pub fn new(engine: E, page_size: u8) -> Result<Self, RuntimeError> {
        Self::new_with_touch_layout(engine, page_size, TouchKeyboardLayout::default())
    }

    pub fn new_with_touch_layout(
        mut engine: E,
        page_size: u8,
        touch_keyboard_layout: TouchKeyboardLayout,
    ) -> Result<Self, RuntimeError> {
        if !(1..=MAX_CANDIDATE_PAGE_SIZE).contains(&page_size) {
            return Err(RuntimeError::InvalidPageSize);
        }
        // 引擎的首屏门槛和分页同源，建立时就交给它，不靠调用方在引擎选项里另设一份：只在 `set_page_size` 下发时，直接 `Runtime::new` 的示例、测试和评测工具分页是 9，引擎却停在 `prepare_options` 的默认 6 行，有 6 到 8 个精确同音词的输入首页和任何宿主都不一样。
        engine.set_first_page_rows(page_size.into());
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
            settled_settings_epoch: Arc::new(()),
            settled_model_epoch: Arc::new(()),
            focused: false,
            page_size: page_size.into(),
            highlighted: 0,
            translations: HashMap::new(),
            highlight_translations: HashMap::new(),
            translation_scope_highlighted: false,
            display_page: None,
            // The initial snapshot is already in Engine order. Keep the mapping empty until a
            // presentation reorder actually needs it; engine_index falls back to the seat while
            // the list remains untouched.
            engine_order: Vec::new(),
            online_order_scratch: Vec::new(),
            online_fixed_english_scratch: Vec::new(),
            settled_order_scratch: Vec::new(),
            cached,
            snapshot_valid: true,
            character_width: CharacterWidth::Halfwidth,
            punctuation_marks: PunctuationMarks::default(),
            touch_keyboard_layout,
            key_geometry: None,
            wubi_auto_commit_unique: true,
            wubi_fifth_code_top_commit: true,
            wubi_candidate_selection: Default::default(),
            second_third_candidate: Default::default(),
            phrase_preedit: false,
            backspace_reverts_selection: true,
            escape_keeps_selected_word: false,
            left_right_navigation: false,
            english_first_page_count: 1,
            vertical_candidates: false,
            phrase_prefix: String::new(),
            phrase_selections: Vec::new(),
            facts: DispatchFacts::default(),
            selected_text: String::new(),
            multilingual_output: (false, Default::default()),
            selected_multilingual: None,
            composition_epoch: 0,
            candidates_epoch: 0,
            #[cfg(any(
                target_os = "macos",
                target_os = "windows",
                all(target_os = "linux", not(target_env = "ohos"))
            ))]
            plugins: Default::default(),
        })
    }

    /// 将尚未完成的词段暂存在组字区，等整段完成后再上屏。
    ///
    /// 宿主保证在编辑文字前绘制 [`View::phrase_prefix`]；由宿主决定的原因见该字段。
    /// 重复设置同一状态保留组词与撤回历史；关闭时返回暂存文字，避免丢掉已选词段。
    pub fn set_phrase_preedit(&mut self, enabled: bool) -> Option<String> {
        if self.phrase_preedit == enabled {
            return None;
        }
        self.phrase_preedit = enabled;
        if enabled || self.phrase_prefix.is_empty() {
            self.phrase_selections.clear();
            return None;
        }
        self.phrase_selections.clear();
        self.bump_composition_epoch();
        Some(std::mem::take(&mut self.phrase_prefix))
    }

    /// Attach a candidate reranker. Hosts load the model themselves, because where a model file
    /// lives is a packaging question that differs per platform and the runtime has no business
    /// guessing at it.
    pub fn set_reranker(&mut self, reranker: Option<Reranker>) {
        self.set_candidate_scorer(reranker.map(|r| Box::new(r) as Box<dyn CandidateScorer>));
    }

    /// 同 [`Runtime::set_reranker`]，挂任意实现了 [`CandidateScorer`] 的逐键打分器，比如需要原生运行库的模型后端。
    ///
    /// 挂上的打分器报 [`CandidateScorer::scores_prediction_pool_within_a_frame`] 时，上屏后联想的词库续写也交给它排序（见 [`Self::order_prediction`]），引擎为此多取一池续写；拿掉打分器或它不报时，引擎只取 8 条，联想与没挂打分器时逐行相同。
    pub fn set_candidate_scorer(&mut self, scorer: Option<Box<dyn CandidateScorer>>) {
        self.reranker = scorer;
        self.engine.set_prediction_pool(self.orders_prediction());
    }

    /// Whether a per-keystroke reranker is attached, so a host can attach or drop it when the preference that governs it changes.
    pub fn has_reranker(&self) -> bool {
        self.reranker.is_some()
    }

    /// 安装或移除停顿模型；替换后不得恢复在途评分携带的旧模型。
    pub fn set_settled_reranker(&mut self, reranker: Option<Reranker>) {
        self.set_settled_candidate_scorer(
            reranker.map(|r| Box::new(r) as Box<dyn CandidateScorer>),
        );
    }

    /// 同 [`Runtime::set_settled_reranker`]，挂任意实现了 [`CandidateScorer`] 的停顿打分器。
    pub fn set_settled_candidate_scorer(&mut self, scorer: Option<Box<dyn CandidateScorer>>) {
        self.settled_model_epoch = Arc::new(());
        self.settled_reranker = scorer;
    }

    /// 开关改变时作废在途评分，但保留模型，重新开启时不必加载。
    pub fn set_settled_rerank_enabled(&mut self, enabled: bool) {
        if self.settled_rerank_enabled != enabled {
            self.settled_settings_epoch = Arc::new(());
            self.settled_rerank_enabled = enabled;
        }
    }

    pub fn settled_rerank_enabled(&self) -> bool {
        self.settled_rerank_enabled
    }

    /// Re-rank the current candidates with the settled model, reporting whether the order moved. Nothing happens without an attached model or while the settled rerank is switched off.
    ///
    /// The host decides when this is: it owns the clock and already runs a settle timer for cloud candidates. The runtime has no timer of its own and should not grow one — a keystroke that arrives while this is deciding makes the whole answer stale, and only the host knows that a keystroke arrived.
    ///
    /// Returns false when nothing changed, so a host can skip redrawing the candidate window. A window that repaints identically on every pause is a flicker the user cannot explain.
    ///
    /// 本方法在调用线程运行模型；输入线程应使用 [`Runtime::begin_settled_rerank`] 和
    /// [`score_settled_rerank`] 异步计算，再经 [`Runtime::apply_settled_rerank`] 归还模型。
    pub fn rerank_settled(&mut self) -> bool {
        if self.settled_rerank_blocked() {
            return false;
        }
        self.snapshot_settled_seats();
        let marked = self.settled_highlight();
        std::mem::swap(&mut self.reranker, &mut self.settled_reranker);
        // 使用快速重排相同的后处理，保留固定席位。九宫格的裁决标记不交给落定模型，见 `rerank`。
        let moved = self.rerank(false);
        std::mem::swap(&mut self.reranker, &mut self.settled_reranker);
        self.finish_settled_presentation(moved, marked)
    }

    /// 后台评分可能改变顺序时取出停顿模型。
    ///
    /// `None` 保留模型，调用方可运行 [`Runtime::rerank_settled`] 完成无需模型的呈现整理。
    /// `Some` 转移模型所有权，但按键仍使用快速模型；调用方必须经
    /// [`Runtime::apply_settled_rerank`] 或 [`Runtime::discard_settled_rerank`] 归还。
    pub fn begin_settled_rerank(&mut self) -> Option<SettledRerankRequest> {
        if self.settled_rerank_blocked() || !self.settled_model_would_score() {
            return None;
        }
        self.snapshot_settled_seats();
        let generation = self.generation;
        let context = self.ai_context.clone();
        let scheme = self.cached.scheme;
        let answered_by_pinyin_fallback = self.cached.answered_by_pinyin_fallback;
        let texts = self.cached.candidates.clone();
        let sources = self.cached.candidate_sources.clone();
        let answers_key = self.cached.candidate_answers_key.clone();
        let corrected = self.cached.candidate_corrected.clone();
        let static_evidence = if self.cached.candidate_static_evidence.len() == texts.len() {
            self.cached.candidate_static_evidence.clone()
        } else {
            Vec::new()
        };
        let reranker = self.settled_reranker.take()?;
        Some(SettledRerankRequest {
            generation,
            settings_epoch: self.settled_settings_epoch.clone(),
            model_epoch: self.settled_model_epoch.clone(),
            context,
            scheme,
            answered_by_pinyin_fallback,
            rows: SettledRerankRows {
                texts,
                sources,
                answers_key,
                corrected,
                static_evidence,
            },
            reranker,
        })
    }

    /// 仅发布同一组字、开关与模型版本的评分；开关变化不推进可见候选的身份。
    pub fn apply_settled_rerank(&mut self, score: SettledRerankScore) -> SettledRerankApplied {
        let SettledRerankScore {
            generation,
            settings_epoch,
            model_epoch,
            promote,
            reranker,
        } = score;
        let current = generation == self.generation
            && self.settled_rerank_enabled
            && Arc::ptr_eq(&settings_epoch, &self.settled_settings_epoch)
            && Arc::ptr_eq(&model_epoch, &self.settled_model_epoch);
        self.restore_settled_reranker(reranker, &model_epoch);
        if !current {
            return SettledRerankApplied {
                current: false,
                moved: false,
            };
        }
        // 评分期间用户仍可移动高亮，按发布前的实际选择恢复。
        let marked = self.settled_highlight();
        let moved = promote.is_some_and(|index| self.promote_row(index));
        SettledRerankApplied {
            current: true,
            moved: self.finish_settled_presentation(moved, marked),
        }
    }

    /// 放弃发布；模型未被替换或移除时才归还，关闭开关仍保留模型供下次使用。
    pub fn discard_settled_rerank(&mut self, score: SettledRerankScore) {
        self.restore_settled_reranker(score.reranker, &score.model_epoch);
    }

    fn settled_rerank_blocked(&self) -> bool {
        // 重排必须推进候选代次，避免旧身份选到新座位；代次耗尽时不得重排。
        !self.settled_rerank_enabled
            || self.settled_reranker.is_none()
            || self.is_idle()
            || self.generation.checked_add(1).is_none()
            // 落定重排按座位挪行，插件插入的行没有引擎下标，会被当成引擎的候选挪走；有插入的行时不做。
            || self.has_inserted_rows()
    }

    /// 判断是否需要模型前向计算；提前退出的情况可留在会话线程处理。
    fn settled_model_would_score(&self) -> bool {
        if !self.cached.conversion.is_empty() || self.cached.caret_edit_suffix {
            return false;
        }
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
        rerank_needs_model(
            snapshot.scheme,
            snapshot.answered_by_pinyin_fallback,
            &order_rows(snapshot),
        )
    }

    fn snapshot_settled_seats(&mut self) {
        // 脚本插件的操作只按候选身份重放，不运行脚本：停顿的时机不能透露给脚本。是否挪动按重放之后的座位比较。
        if !self.has_candidate_ops() {
            return;
        }
        let mut order = std::mem::take(&mut self.settled_order_scratch);
        self.copy_seat_order(&mut order);
        self.settled_order_scratch = order;
    }

    fn restore_settled_reranker(
        &mut self,
        reranker: Box<dyn CandidateScorer>,
        model_epoch: &Arc<()>,
    ) {
        if Arc::ptr_eq(model_epoch, &self.settled_model_epoch) && self.settled_reranker.is_none() {
            self.settled_reranker = Some(reranker);
        }
    }

    fn settled_highlight(&self) -> Option<(usize, String, Option<String>)> {
        if self.highlighted == 0 {
            return None;
        }
        Some((
            self.highlighted,
            self.cached.candidates.get(self.highlighted)?.clone(),
            self.cached.candidate_codes.get(self.highlighted).cloned(),
        ))
    }

    fn finish_settled_presentation(
        &mut self,
        rerank_moved: bool,
        marked: Option<(usize, String, Option<String>)>,
    ) -> bool {
        let mut moved = rerank_moved;
        moved |= self.demote_runner_up_readings();
        moved |= self.normalize_online_slots();
        if self.has_candidate_ops() {
            let marks_changed = self.transform_candidates();
            moved = marks_changed || !self.seat_order_matches(&self.settled_order_scratch);
        }
        if moved {
            self.snapshot_valid = true;
            // 用户还停在第一条时，落定重排换上来的词就是新的第一条。已经翻页或移动过高亮时，
            // 停在原来那一条上，不能弹回第一页。
            if let Some((previous_highlight, text, code)) = &marked {
                self.keep_highlight_on_same_candidate(*previous_highlight, text, code.as_deref());
            } else {
                self.highlighted = 0;
            }
            let _ = self.advance();
            self.bump_candidates_epoch();
        }
        moved
    }

    pub fn set_character_width(&mut self, width: CharacterWidth) {
        self.character_width = width;
    }

    /// 记下引擎当前用的标点样式，只用于在 View 里报告；出字由引擎按自己的选项决定。宿主在建会话和每次重建引擎后设它。
    pub fn set_punctuation_marks(&mut self, marks: PunctuationMarks) {
        self.punctuation_marks = marks;
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

    /// 注册或清除触屏宿主的字母键几何（`None` 清除），见 [`msime_engine::touch::KeyGeometry`]。只在布局变化时调用；引擎被替换后由 runtime 重新下发。不刷新候选。
    pub fn set_key_geometry(&mut self, geometry: Option<KeyGeometry>) {
        self.key_geometry = geometry.map(Arc::new);
        self.engine.set_key_geometry(self.key_geometry.clone());
    }

    /// 当前注册的几何代次，没有几何时为 `None`。只有测试用来核对注册和清除。
    #[cfg(test)]
    pub(crate) fn key_geometry_id(&self) -> Option<u32> {
        self.key_geometry.as_ref().map(|geometry| geometry.id())
    }

    /// 宿主按下点 (x, y) 落在 `key` 上的触点。代次 `geometry_id` 与当前注册的不符、坐标不是有限数、`key` 不在几何里时为 `None`，这一键就按没有触点打，与 `msime_client_character` 逐字节相同。
    pub fn key_touch(&self, key: u8, geometry_id: u32, x: f32, y: f32) -> Option<KeyTouch> {
        self.key_geometry
            .as_ref()
            .filter(|geometry| geometry.id() == geometry_id)?
            .touch(key, x, y)
    }

    /// 下一次派发的 `Action::Character` 的触点；派发完不论结果如何都要 `set_key_touch(None)`。引擎只把它交给 26 键双拼的小写字母和 `;`，其余入口一开头就丢掉它。
    pub fn set_key_touch(&mut self, touch: Option<KeyTouch>) {
        self.engine.set_key_touch(touch);
    }

    /// 移动引擎前缀解码的光标；`None` 恢复到组字末尾。候选可能随前缀改变，旧候选 ID 必须失效。
    pub fn set_caret(&mut self, caret: Option<usize>) -> Result<(), RuntimeError> {
        self.advance()?;
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

    /// 换触屏的组码网格。九键要求全拼或注音，14 键要求全拼或五笔，十八键和九键双拼要求全拼，双拼 14 键和 18 键要求双拼；方案不符报 `InvalidNineKeyScheme`，组字中切换报 `CompositionActive`。
    pub fn set_key_grid(&mut self, grid: Option<KeyGrid>) -> Result<(), RuntimeError> {
        let scheme = scheme_type(self.cached.scheme);
        let supported = match grid {
            None => true,
            Some(KeyGrid::NineKey) => scheme.is_some_and(SchemeType::nine_key),
            // 五笔 14 键与全拼共用键面和组码，由引擎按方案分别解码。
            Some(KeyGrid::FourteenKey) => {
                matches!(scheme, Some(SchemeType::Quanpin | SchemeType::Wubi))
            }
            Some(KeyGrid::EighteenKey | KeyGrid::NineKeyShuangpin(_)) => {
                scheme == Some(SchemeType::Quanpin)
            }
            Some(KeyGrid::FourteenKeyShuangpin(_) | KeyGrid::EighteenKeyShuangpin(_)) => {
                scheme == Some(SchemeType::Shuangpin)
            }
        };
        if !supported {
            return Err(RuntimeError::InvalidNineKeyScheme);
        }
        if !self.is_idle() {
            return Err(RuntimeError::CompositionActive);
        }
        if self.cached.key_grid == grid {
            return Ok(());
        }
        self.advance()?;
        self.engine.set_key_grid(grid)?;
        self.refresh()
    }

    pub fn set_nine_key_enabled(&mut self, enabled: bool) -> Result<(), RuntimeError> {
        if enabled && !scheme_type(self.cached.scheme).is_some_and(SchemeType::nine_key) {
            return Err(RuntimeError::InvalidNineKeyScheme);
        }
        if !self.is_idle() {
            return Err(RuntimeError::CompositionActive);
        }
        // 比的是网格而不只是九键：14 键开着时关九键也要真的关掉网格。
        if self.cached.key_grid == enabled.then_some(KeyGrid::NineKey) {
            return Ok(());
        }
        self.advance()?;
        self.engine.set_nine_key_enabled(enabled)?;
        self.refresh()
    }

    pub fn view(&self) -> View {
        let page = self.highlighted / self.page_size;
        let start = page * self.page_size;
        // 只认可见前缀：被插件隐藏的行在列表末尾，不进视图也不进分页。
        let visible = self.visible_len();
        let page_len = visible.saturating_sub(start).min(self.page_size);
        // 「仅当前高亮」时译文只跟着高亮走：高亮行一直留出译文那一行（回答还没到时也留），竖排窗口移动高亮时才不会一会儿矮一行、一会儿高一行。
        let reserve_translation = self.translation_scope_highlighted
            && page_len > 0
            && !self.cached.prediction
            && self.page_requests_translations();
        let mut candidates = Vec::with_capacity(page_len);
        candidates.extend(
            self.cached
                .candidates
                .iter()
                .enumerate()
                .skip(start)
                .take(page_len)
                .map(|(index, text)| self.candidate_row(index, text, reserve_translation)),
        );
        View {
            prediction: self.cached.prediction,
            scheme: self.cached.scheme,
            chinese_text: scheme_type(self.cached.scheme).is_some_and(SchemeType::is_chinese),
            script_conversion: script_conversion(self.cached.scheme, &self.cached.local_mode),
            nine_key: self.cached.nine_key,
            key_grid: key_grid_name(self.cached.key_grid),
            grid_composing: self.cached.grid_composing,
            nine_key_spellings: self.cached.nine_key_spellings.clone(),
            nine_key_reading: self.cached.nine_key_reading.clone(),
            nine_key_single_character: self.cached.nine_key_single_character,
            nine_key_strokes: self.cached.nine_key_strokes.clone(),
            touch_keyboard_layout: self.touch_keyboard_layout,
            character_width: self.character_width,
            punctuation_marks: self.punctuation_marks,
            microsoft_shuangpin: self.cached.microsoft_shuangpin,
            shuangpin_profile: self.cached.shuangpin_profile.clone(),
            answered_by_pinyin_fallback: self.cached.answered_by_pinyin_fallback,
            local_mode: self.cached.local_mode.clone(),
            // 暂存词段后不开放新模式；Tab 仅在剩余拼写能接收末尾辅助码时由引擎公布。
            spelling_symbols: if self.phrase_prefix.is_empty() || self.cached.local_mode != "none" {
                self.cached.spelling_symbols.clone()
            } else if self.cached.spelling_symbols.contains('\t') {
                "\t".to_owned()
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
            display_reading: self.cached.display_reading.clone(),
            editing_text: self.cached.editing_text.clone(),
            caret_position: self.cached.caret_position,
            conversion: self.cached.conversion.clone(),
            conversion_focus_start: self.cached.conversion_focus_start,
            conversion_focus_end: self.cached.conversion_focus_end,
            caret_edit_suffix: self.cached.caret_edit_suffix,
            caret_edit_prefix: self.cached.caret_edit_prefix.clone(),
            page,
            page_size: self.page_size,
            page_count: visible.div_ceil(self.page_size),
            candidate_list_open: self.cached.candidate_list_open,
            candidates,
        }
    }

    /// Number of candidates on the currently published page, without building candidate rows.
    /// Hosts that only need the count for a punctuation decision can avoid materializing a full
    /// [`View`].
    pub fn candidate_page_len(&self) -> usize {
        let start = (self.highlighted / self.page_size) * self.page_size;
        self.visible_len().saturating_sub(start).min(self.page_size)
    }

    /// The Engine scheme ordinal of the applied state, without materializing a [`View`].
    pub fn scheme(&self) -> u8 {
        self.cached.scheme
    }

    /// 当前本地模式的名字（`View::local_mode`），不构造 [`View`]。生命周期核心在派发前读它，按键路径上不能为此克隆一份视图。
    pub fn local_mode(&self) -> &str {
        &self.cached.local_mode
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
    /// 候选取看得见的那一页：宿主报了显示页就按它，否则按高亮所在的固定页。
    pub fn translation_candidates(&self) -> Option<TranslationCandidates> {
        let seats = self.page_seats();
        let visible = self.visible_len();
        let mut candidates = Vec::with_capacity(seats.len());
        let mut highlighted = None;
        for index in seats.into_iter().filter(|&index| index < visible) {
            let Some(text) = self.cached.candidates.get(index) else {
                continue;
            };
            if index == self.highlighted {
                highlighted = Some(candidates.len());
            }
            candidates.push(TranslationCandidate {
                text: text.clone(),
                source: self
                    .cached
                    .candidate_sources
                    .get(index)
                    .copied()
                    .unwrap_or_default(),
            });
        }
        (!candidates.is_empty()).then_some(TranslationCandidates {
            generation: self.generation,
            scheme: self.cached.scheme,
            local_mode: self.cached.local_mode.clone(),
            highlighted,
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
        self.release_held_candidates();
        self.all_candidates_cached()
    }

    /// 座位 `start` 起最多 `count` 行候选，给每键都要重画的横向候选条用：只序列化这一批，不像 [`Self::all_candidates`] 那样整代复制。
    ///
    /// 引擎暂扣的尾部只在 `start` 已经走到放出来的末尾（宿主把已有的都建完了还要更多）时才放，和翻页宿主翻到最后一页时一样；放出来会重排座位并换一代，宿主看到 `generation` 变了就要按新的一代从头取。首字母打下去时因此不必把整张声母表查出来。
    pub fn candidate_range(&mut self, start: usize, count: usize) -> CandidateRange {
        if count > 0 && start >= self.visible_len() {
            self.release_held_candidates();
        }
        let total = self.visible_len();
        let end = start.saturating_add(count).min(total);
        let candidates = self
            .cached
            .candidates
            .iter()
            .enumerate()
            .take(end)
            .skip(start)
            .map(|(index, text)| self.candidate(index, text))
            .collect();
        CandidateRange {
            session: self.session,
            generation: self.generation,
            preedit: self.cached.preedit.clone(),
            reading: self.cached.reading.clone(),
            total,
            candidates,
        }
    }

    /// 让引擎放出暂扣的候选尾部；放出来了就换一代。
    fn release_held_candidates(&mut self) {
        // Candidate IDs are tied to the generation.  Once that identity space is
        // exhausted we cannot publish a reordered snapshot safely: advancing would
        // fail and retaining the old generation would let an ID from the previous
        // seat select a different candidate.  Keep the currently published view
        // stable; callers can still inspect the candidates already released.
        if self.generation == u64::MAX {
            return;
        }
        // The released tail reorders the list, so the page's IDs must not keep selecting by seat.
        if self.expand_cached_candidates().unwrap_or(false) {
            // 上面已经排除了 `generation == u64::MAX`，所以这里不会失败。推进必须写在断言外面：release 构建不执行 `debug_assert!` 里的表达式。
            let advanced = self.advance();
            debug_assert!(advanced.is_ok());
        }
    }

    /// The generation as it stands, without asking the engine for more.
    fn all_candidates_cached(&self) -> CandidateSnapshot {
        let visible = self.visible_len();
        let mut candidates = Vec::with_capacity(visible);
        candidates.extend(
            self.cached
                .candidates
                .iter()
                .take(visible)
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

    /// 视图里这一行的文字：光标后编辑的首选前面接上固定的那段（`EngineSnapshot::caret_edit_prefix`），其余照旧。runtime 在本模式里不重排，所以首位就是引擎的首行。
    fn displayed_candidate_text(&self, index: usize, text: &str) -> String {
        let text = self.multilingual_candidate(index, text);
        let text = if self.cached.candidate_sources.get(index)
            == Some(&(msime_engine::CandidateSource::QuickPhrase as u8))
        {
            msime_engine::expand_quick_phrase(&text).text.into_owned()
        } else {
            text
        };
        if index == 0 && !self.cached.caret_edit_prefix.is_empty() {
            let mut shown = self.cached.caret_edit_prefix.clone();
            shown.push_str(&text);
            return shown;
        }
        text
    }

    fn candidate(&self, index: usize, text: &str) -> Candidate {
        self.candidate_row(index, text, false)
    }

    /// `reserve_translation` 为真时，高亮那一行标出 [`Candidate::translation_reserved`]，见 [`Self::view`]。
    fn candidate_row(&self, index: usize, text: &str, reserve_translation: bool) -> Candidate {
        let highlighted = index == self.highlighted;
        // 不合格来源不能继承同名普通候选的释义；分段也只来自实际显示的这一份字符串。
        // 「仅当前高亮」时候选词翻译只画在高亮行，其他行只画不受范围限制的离线释义。
        let translation = ((!self.translations.is_empty()
            || !self.highlight_translations.is_empty())
            && !msime_client_core::translation::source_gets_no_gloss(
                self.cached
                    .candidate_sources
                    .get(index)
                    .copied()
                    .unwrap_or_default(),
            ))
        .then(|| {
            highlighted
                .then(|| self.highlight_translations.get(text))
                .flatten()
                .or_else(|| self.translations.get(text))
                .cloned()
        })
        .flatten();
        let translation_runs = translation
            .as_deref()
            .map(msime_client_core::translation::translation_runs)
            .unwrap_or_default();
        Candidate {
            id: CandidateId {
                session: self.session,
                generation: self.generation,
                index,
            },
            text: self.displayed_candidate_text(index, text),
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
            // 只标真正坐在第一位的那一行：插件或固定位置把它挪开以后，它只是一条普通的纠错行。
            confident_correction: index == 0
                && self
                    .cached
                    .candidate_confident_correction
                    .first()
                    .copied()
                    .unwrap_or(false),
            fixed_position: self
                .cached
                .candidate_positions
                .get(index)
                .copied()
                .unwrap_or_default(),
            highlighted,
            translation_reserved: reserve_translation && highlighted,
            translation,
            translation_runs,
            #[cfg(any(
                target_os = "macos",
                target_os = "windows",
                all(target_os = "linux", not(target_env = "ohos"))
            ))]
            script_annotation: self.script_annotation(index),
        }
    }

    /// A held phrase counts as a composition even when its reading is empty.
    pub fn is_idle(&self) -> bool {
        self.snapshot_valid
            && self.phrase_prefix.is_empty()
            && self.cached.preedit.is_empty()
            && self.cached.editing_text.is_empty()
            && (self.cached.prediction || self.cached.candidates.is_empty())
    }

    /// 是否有组字：暂存词段、预编辑、读音或候选任一非空。
    ///
    /// 与 [`Self::is_idle`] 的区别是不看 `snapshot_valid`：一次失败的 refresh 让快照失效，但用户眼里并没有多出一段组字，生命周期核心据此判断组字开始和结束时不能把它算进去。
    pub fn has_composition(&self) -> bool {
        !self.phrase_prefix.is_empty()
            || !self.cached.preedit.is_empty()
            || !self.cached.editing_text.is_empty()
            || (!self.cached.prediction && !self.cached.candidates.is_empty())
    }

    /// 最近一次 `dispatch` 或 `focus` 里发生了什么，见 [`DispatchFacts`]。
    pub fn facts(&self) -> DispatchFacts {
        self.facts
    }

    /// 最近一次调用选中的候选的文本，和 [`DispatchFacts::selection`] 同生同灭；没有选择时为 `None`。脚本插件经 `read_commit` 读它。
    pub fn selected_text(&self) -> Option<&str> {
        self.facts
            .selection
            .is_some()
            .then_some(self.selected_text.as_str())
    }

    /// 组字的单调计数器，见字段说明。只比较相等与否，不解释数值。
    pub fn composition_epoch(&self) -> u64 {
        self.composition_epoch
    }

    /// 候选视图的单调计数器，见字段说明。
    pub fn candidates_epoch(&self) -> u64 {
        self.candidates_epoch
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
        if self.generation == u64::MAX {
            return Err(RuntimeError::IdentityExhausted);
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

    /// 释义请求的配置变化时，清掉旧释义并推进候选代次，使旧异步回包不能落在新配置下。
    pub fn invalidate_translations(&mut self) -> Result<(), RuntimeError> {
        self.advance()?;
        if !self.translations.is_empty() || !self.highlight_translations.is_empty() {
            self.translations.clear();
            self.highlight_translations.clear();
            self.bump_candidates_epoch();
        }
        Ok(())
    }

    /// 同代次非空释义合并，空数组显式清理；旧代次回包不能更新展示。回答都按候选词翻译处理（不带 `gloss` 标记），见 [`Self::apply_translation_answers`]。
    pub fn apply_translations(
        &mut self,
        generation: u64,
        translations: impl IntoIterator<Item = (String, String)>,
    ) -> bool {
        self.apply_translation_answers(
            generation,
            translations
                .into_iter()
                .map(|(text, translation)| (text, translation, false)),
        )
    }

    /// 同代次非空释义合并，空数组显式清理；旧代次回包不能更新展示。每条回答是（候选文字、释义、是否为不受范围限制的离线释义）。
    ///
    /// 「仅当前高亮」生效、且这一代的翻译查询为高亮行请求了候选词翻译（高亮在这一页的座位里）时，不带 `gloss` 标记的回答是候选词翻译，进 [`Self::highlight_translations`]，只画在高亮的那一行——没升级的宿主照旧把整页答回来，也只有高亮行画出来；带标记的回答（翻译查询里 `gloss_query` 的离线释义）进 [`Self::translations`]，每一行都画。高亮不在这一页时查询只剩那份不带服务的离线释义文档，宿主照普通查询处理、不加标记，这时的回答也都进 [`Self::translations`]。代次相同就说明高亮没动过：移动高亮会推进代次，所以这里的高亮就是宿主发查询时的高亮。
    pub fn apply_translation_answers(
        &mut self,
        generation: u64,
        answers: impl IntoIterator<Item = (String, String, bool)>,
    ) -> bool {
        if generation != self.generation {
            return false;
        }
        let scoped = self.translation_scope_highlighted && self.highlight_on_page_seats();
        let mut highlight_translations: HashMap<String, String> = HashMap::new();
        // 范围不生效时整批进同一张表，按回包长度一次建好，与这个选项出现以前的分配相同。
        let translations: HashMap<String, String> = if scoped {
            let mut translations = HashMap::new();
            for (text, translation, gloss) in answers {
                if gloss {
                    translations.insert(text, translation);
                } else {
                    highlight_translations.insert(text, translation);
                }
            }
            translations
        } else {
            answers
                .into_iter()
                .map(|(text, translation, _)| (text, translation))
                .collect()
        };
        let any = !translations.is_empty() || !highlight_translations.is_empty();
        // The answer to a `/fy` request arrives through the same host path as candidate glosses, so a host forwarding `translation_query` needs nothing else: it becomes the row that commits the translation, not a gloss under the English.
        if let Some(query) = self.command_translation() {
            if let Some(translation) = translations
                .get(&query.text)
                .or_else(|| highlight_translations.get(&query.text))
            {
                let translation = translation.clone();
                return self
                    .apply_command_translation(&query, &translation)
                    .unwrap_or(false);
            }
        }
        // 释义画在候选行下面，有变化就是候选视图的变化。
        let mut changed = false;
        if !any {
            changed = !self.translations.is_empty() || !self.highlight_translations.is_empty();
            self.translations.clear();
            self.highlight_translations.clear();
        } else {
            for (incoming, stored) in [
                (translations, &mut self.translations),
                (highlight_translations, &mut self.highlight_translations),
            ] {
                let mut incoming = incoming;
                retain_candidate_translations(&mut incoming, &self.cached.candidates);
                for (text, translation) in incoming {
                    if stored.get(&text) != Some(&translation) {
                        stored.insert(text, translation);
                        changed = true;
                    }
                }
            }
        }
        if changed {
            self.bump_candidates_epoch();
        }
        true
    }

    /// 候选词翻译开着且范围是「仅当前高亮」时为真，由 host-api 跟着最新请求的偏好设置。只改画法：释义查询的范围由 host-api 的 `msime_client_translation_query` 按同一份偏好决定。切换时的旧释义由 [`Self::invalidate_translations`] 清理（host-api 在释义相关的偏好变化时调用它）。
    pub fn set_translation_scope_highlighted(&mut self, highlighted: bool) {
        if self.translation_scope_highlighted != highlighted {
            self.translation_scope_highlighted = highlighted;
            self.bump_candidates_epoch();
        }
    }

    /// 高亮那一行在不在当前这一页的座位里（不建座位表）：宿主报了这一代的显示页就查它，否则固定页就是高亮所在的那一页。
    fn highlight_on_page_seats(&self) -> bool {
        if self.highlighted >= self.visible_len() {
            return false;
        }
        match self
            .display_page
            .as_ref()
            .filter(|_| self.display_page_current())
        {
            Some((_, indices)) => indices.contains(&self.highlighted),
            None => true,
        }
    }

    /// 当前这一页会不会请求释义：方案显示释义，不是临时日文或网址模式，这一页也没有脚本插件的行（插件的文字不能交给释义服务）。翻译查询和「仅当前高亮」为高亮行留的那一行用同一条判断。
    pub fn page_requests_translations(&self) -> bool {
        scheme_type(self.cached.scheme).is_some_and(SchemeType::shows_glosses)
            && !matches!(
                self.cached.local_mode.as_str(),
                "temporary_japanese" | "url"
            )
            && !self.page_seats().into_iter().any(|index| {
                self.cached.candidate_sources.get(index)
                    == Some(&(msime_engine::CandidateSource::Plugin as u8))
            })
    }

    /// Presentation-only resize; a live composition keeps its numeric key map.
    pub fn set_page_size(&mut self, page_size: u8) -> Result<(), RuntimeError> {
        if !(1..=MAX_CANDIDATE_PAGE_SIZE).contains(&page_size) {
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
        // 引擎的首屏门槛问的是「首页放得下几行」，和分页同源。建立和换引擎时都下发过，这里改了分页不跟着下发，它就停在旧的那一份：每页从 6 改成 9 时，有 6 到 8 个精确同音词的输入整组补全词都被丢掉，而窗口其实有 9 个座位。
        self.engine.set_first_page_rows(page_size.into());
        Ok(())
    }

    /// Preserve the host handle/focus while invalidating every old candidate ID.
    /// Validate the replacement before changing any live state.
    ///
    /// `page_size` 同 [`Self::new`]：既是分页，也是交给新引擎的首屏门槛。
    pub fn replace_engine(&mut self, engine: E, page_size: u8) -> Result<(), RuntimeError> {
        self.replace_engine_with_touch_layout(engine, page_size, self.touch_keyboard_layout)
    }

    pub fn replace_engine_with_touch_layout(
        &mut self,
        mut engine: E,
        page_size: u8,
        touch_keyboard_layout: TouchKeyboardLayout,
    ) -> Result<(), RuntimeError> {
        if !(1..=MAX_CANDIDATE_PAGE_SIZE).contains(&page_size) {
            return Err(RuntimeError::InvalidPageSize);
        }
        if !self.is_idle() {
            return Err(RuntimeError::CompositionActive);
        }
        // 新引擎的首屏门槛同样跟着这一次的分页，理由见 `new_with_touch_layout`。
        engine.set_first_page_rows(page_size.into());
        let cached = engine.snapshot()?;
        self.advance()?;
        self.engine = engine;
        // The committed text belongs to the client, not the engine, so the replacement ranks against it as its predecessor did.
        self.engine.set_rescoring_context(&self.ai_context);
        // 键位几何同样属于宿主：布局没变，宿主不会重发。
        self.engine.set_key_geometry(self.key_geometry.clone());
        // 打分器挂在 runtime 上，新引擎同样要按它的速度决定多不多取联想的候选池。
        self.engine.set_prediction_pool(self.orders_prediction());
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
    ///
    /// 「留在原地」只在这次展开确实没有长出新页时才成立。展开把隐藏的候选放出来，`len` 变大，
    /// 末页随之后移：原来停的地方可能已经不是末页了。这时如果还返回 `true`，高亮就停在旧页上，
    /// 而候选窗已经换成了新的一帧。数字键选词读的是高亮推出的页码（`ServerSession.cpp` 按槽位取
    /// 当前页），于是按 1 上屏的是翻页前那一页的第一个候选（#9517）。所以只有展开之后页数没变、
    /// 当前页只是从短变满，才留在原地。
    fn expand_for_next_page(&mut self) -> Result<bool, RuntimeError> {
        let len = self.visible_len();
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
        let grown = self.visible_len();
        let stayed = page == last_page && !page_was_full;
        // 展开长出了新页：候选集已经变了，高亮必须跟着前进，否则页面和内容对不上。
        let grown_last_page = grown.saturating_sub(1) / self.page_size;
        if stayed && grown_last_page > last_page {
            return Ok(false);
        }
        Ok(stayed)
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
        let len = self.visible_len();
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
        self.order_presentation();
        // 派发类调用里的展开重新运行插件，其余（`all_candidates`）只重放。
        self.transform_candidates();
        self.bump_candidates_epoch();
        Ok(true)
    }

    /// Take a snapshot straight from the Engine, whose seats are still in the Engine's order.
    fn load_snapshot(&mut self, snapshot: EngineSnapshot) {
        // An empty mapping means the cached order is the Engine's order; build it lazily only if
        // one of the presentation reorderings below actually moves a candidate.
        self.engine_order.clear();
        self.cached = snapshot;
        self.forget_inserted_rows();
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

    fn bump_composition_epoch(&mut self) {
        self.composition_epoch = self.composition_epoch.wrapping_add(1);
    }

    fn bump_candidates_epoch(&mut self) {
        self.candidates_epoch = self.candidates_epoch.wrapping_add(1);
    }

    /// 记下 `seat` 上的选择（见 [`Self::selection_fact`]）和它的文本；对不上时两者都清空。
    fn record_selection(
        &mut self,
        how: SelectionHow,
        seat: usize,
        result: &EngineResult,
        edge: Option<CandidateEdge>,
    ) {
        self.facts.selection = self.selection_fact(how, seat, result, edge);
        self.selected_text.clear();
        self.selected_multilingual = None;
        if self.facts.selection.is_some() {
            if let Some(text) = self.cached.candidates.get(seat) {
                self.selected_text.push_str(text);
                self.capture_multilingual_selection(seat, edge);
            }
        }
    }

    /// 一次把候选交给引擎的调用是否真的选中了 `seat` 上的候选，用的是引擎调用之前（refresh 之前）的快照。
    ///
    /// 只有引擎的上屏带着这个候选时才算：常规路径要求上屏以候选文本开头（结束组字、顶字后面还会接上别的），以词定字要求上屏与引擎判定的首个或末个汉字匹配。整句改字里、以及注音打开的列表里，点选、数字或空格选中一段只把这段钉住、不上屏，记为钉住；韩文汉字列表也是打开的列表，但它的选择会上屏，走上面的分支。对不上的一律不记，宁可漏报，也不把别的文本说成用户选中的候选。
    fn selection_fact(
        &self,
        how: SelectionHow,
        seat: usize,
        result: &EngineResult,
        edge: Option<CandidateEdge>,
    ) -> Option<SelectionFact> {
        let text = self.cached.candidates.get(seat)?;
        let outcome = if result.has_commit {
            let matches = if self.cached.caret_edit_suffix {
                // 光标后编辑：上屏是「光标前固定的那段 + 光标后那一行」（固定的那段已经暂存过时只有这一行），所以按结尾对。
                let tail = if how == SelectionHow::Edge {
                    edge.map_or("", |edge| edge.character(text))
                } else {
                    text.as_str()
                };
                !tail.is_empty() && result.commit.ends_with(tail)
            } else if how == SelectionHow::Edge {
                edge.is_some_and(|edge| {
                    let character = edge.character(text);
                    !character.is_empty() && result.commit.starts_with(character)
                })
            } else {
                !text.is_empty() && result.commit.starts_with(text.as_str())
            };
            if !matches {
                return None;
            }
            SelectionOutcome::Committed
        } else if result.handled
            && (!self.cached.conversion.is_empty() || self.cached.candidate_list_open)
            && matches!(
                how,
                SelectionHow::Select
                    | SelectionHow::AnyCandidate
                    | SelectionHow::Digit
                    | SelectionHow::Highlighted
                    | SelectionHow::SpaceOverList
            )
        {
            SelectionOutcome::Pinned
        } else {
            return None;
        };
        Some(SelectionFact {
            seat: u32::try_from(seat).unwrap_or(u32::MAX),
            source: self
                .cached
                .candidate_sources
                .get(seat)
                .copied()
                .unwrap_or_default(),
            how,
            outcome,
        })
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

    /// 回退暂存词段：普通退格在拼写末尾剩零或一个字符时恢复最近一次选词；
    /// `backspace_reverts_selection` 关闭时直接交给引擎删除拼写，删空后由现有收尾逻辑上屏已选文字。
    /// 分段退格在光标前没有拼写时丢掉最近词段，不恢复读音，不受此开关影响。
    /// 只有启用 `phrase_preedit`、宿主能显示暂存词段时才处理；其他动作照常交给引擎。
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
                if !self.backspace_reverts_selection
                    || reading.chars().count() > 1
                    || caret != reading.len()
                {
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
        self.bump_composition_epoch();
        self.facts.retracted = Some(if restore {
            SelectionRetraction::Restore
        } else {
            SelectionRetraction::Drop
        });
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
            // 字母归组键盘的组码只认 `grid_key`；分词键 `'` 和九键的数字本来就从 `character` 进。
            if matches!(
                selection.grid,
                Some(
                    KeyGrid::FourteenKey
                        | KeyGrid::EighteenKey
                        | KeyGrid::FourteenKeyShuangpin(_)
                        | KeyGrid::EighteenKeyShuangpin(_)
                )
            ) && byte != b'\''
            {
                self.engine.grid_key(byte)?;
            } else {
                self.engine.character(byte, byte.is_ascii_uppercase())?;
            }
        }
        self.refresh()?;
        Ok(Some(self.transition(empty_result(true))))
    }

    /// 暂存只消耗部分拼写的选词结果，等整段组字完成后一次上屏。
    /// 仍在组字时把刚选的词放进 `phrase_prefix`；有最终提交时把前缀接在它前面。
    /// 拼写删空而没有提交时，取消丢掉前缀；第一次 Esc 保留已选词时把词段留在组字里；
    /// 分段编辑在仍有可回退历史时保留前缀；其余动作上屏前缀，避免丢掉已选文字。
    /// 普通退格关闭回退时也走这条删空收尾路径。
    /// 只剩暂存词段时，提交或丢弃它都算处理了按键，防止回车、空格再穿透到应用。
    fn hold_phrase_progress(
        &mut self,
        picked: bool,
        discard: bool,
        keep_empty: bool,
        keep_for_escape: bool,
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
                    grid: self.cached.key_grid.filter(|_| self.cached.grid_composing),
                });
            }
            self.phrase_prefix.push_str(&result.commit);
            result.has_commit = false;
            result.commit = String::new();
            // 暂存分支只说明词段变长了；只有这次确实选中了候选，选择的结果才是"暂存"。上屏被收走，尾巴也就无从谈起。
            self.facts.held_grew = true;
            self.facts.tail_chars = 0;
            if let Some(selection) = self.facts.selection.as_mut() {
                selection.outcome = SelectionOutcome::Held;
            }
            self.bump_composition_epoch();
            return;
        }
        if self.phrase_prefix.is_empty() || composing {
            return;
        }
        // 第一次 Esc 保留已选词：引擎的 Cancel 已经把没选的拼写清掉，词段留在组字里等下一次 Esc 或别的键。
        // 不要求有可回退历史：选择没有消耗拼写时 `phrase_selections` 可以为空，词段仍要留下。
        if keep_for_escape && !discard && !result.has_commit {
            result.handled = true;
            return;
        }
        if keep_empty && !discard && !result.has_commit && !self.phrase_selections.is_empty() {
            result.handled = true;
            return;
        }
        let held = std::mem::take(&mut self.phrase_prefix);
        self.phrase_selections.clear();
        self.bump_composition_epoch();
        self.facts.held_flushed = !discard;
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
            script_conversion: script_conversion(self.cached.scheme, &self.cached.local_mode)
                && !self.commits_mixed_japanese()
                && !self.commits_multilingual(),
            // 插件给的行（`/` 指令行、插入普通候选的行）不是用户打的字，不计打字统计。
            typing_statistics: local_mode_counts_as_typing(&self.cached.local_mode)
                && !self.commits_plugin_row(),
            plugin_row: self.commits_plugin_row(),
        }
    }

    /// 这次上屏的是脚本插件给的行（`CandidateSource::Plugin`，`/` 指令里的行或插进普通候选的行）：它不进 AI 上文、不计打字统计，宿主也不把它转成全角。
    fn commits_plugin_row(&self) -> bool {
        const PLUGIN: u8 = msime_engine::CandidateSource::Plugin as u8;
        self.facts.selection.is_some_and(|selection| {
            selection.source == PLUGIN && selection.outcome == SelectionOutcome::Committed
        })
    }

    /// 高亮的是插件指令行（插进普通候选的行不算，见 [`Self::commit_inserted`]）。
    fn highlights_plugin_row(&self) -> bool {
        const PLUGIN: u8 = msime_engine::CandidateSource::Plugin as u8;
        self.cached.candidate_sources.get(self.highlighted) == Some(&PLUGIN)
            && !self.is_inserted(self.highlighted)
    }

    /// 上屏插件插进普通候选的那一行：丢掉组字（不经过引擎选词，所以不进词库、不调词频），文本原样上屏。
    fn commit_inserted(&mut self, seat: usize) -> Result<EngineResult, RuntimeError> {
        let text = self
            .cached
            .candidates
            .get(seat)
            .cloned()
            .unwrap_or_default();
        let mut result = self.discard_composition()?;
        result.handled = true;
        result.has_commit = !text.is_empty();
        result.commit = text;
        Ok(result)
    }

    /// 选中 `seat` 上的候选：插入的行由 runtime 上屏，其余交给引擎。
    fn select_seat(&mut self, seat: usize) -> Result<EngineResult, RuntimeError> {
        if self.is_inserted(seat) {
            return self.commit_inserted(seat);
        }
        self.engine.select(self.engine_index(seat))
    }

    /// 从 `seat` 上的候选结束组字：插入的行由 runtime 上屏，其余交给引擎。
    fn finish_seat(&mut self, seat: usize) -> Result<EngineResult, RuntimeError> {
        if self.is_inserted(seat) {
            return self.commit_inserted(seat);
        }
        self.engine.finish(self.engine_index(seat))
    }

    /// 这次上屏的是日文混输行（`CandidateSource::MixedJapanese`），前面也没有暂存的中文词段：上屏的整段都是日文，不做简繁转换。前面暂存着中文词段时，暂存段和日文一起上屏，宿主只能整段转或整段不转，这时照旧转换，中文不被漏掉。
    fn commits_mixed_japanese(&self) -> bool {
        const MIXED_JAPANESE: u8 = 13;
        self.phrase_prefix.is_empty()
            && self.facts.selection.is_some_and(|selection| {
                selection.source == MIXED_JAPANESE
                    && selection.outcome == SelectionOutcome::Committed
            })
    }

    /// 只展开已确认上屏的快捷短语正文；暂存段和标点尾巴保持各自的来源边界。
    fn expand_quick_phrase_commit(&self, result: &mut EngineResult) -> Option<String> {
        if !result.has_commit
            || self.facts.raw
            || !self.facts.selection.is_some_and(|selection| {
                selection.source == msime_engine::CandidateSource::QuickPhrase as u8
                    && selection.outcome == SelectionOutcome::Committed
                    && selection.how != SelectionHow::Edge
            })
        {
            return None;
        }
        let (body, tail) = self.facts.split_commit(&result.commit);
        let prefix = body.strip_suffix(&self.selected_text)?;
        let expanded = msime_engine::expand_quick_phrase(&self.selected_text);
        if matches!(expanded.text, std::borrow::Cow::Borrowed(_)) {
            return None;
        }
        let suffix = expanded.cursor_byte.map(|index| {
            let mut suffix = expanded.text[index..].to_owned();
            suffix.push_str(tail);
            suffix
        });
        result.commit = format!("{prefix}{}{tail}", expanded.text);
        suffix
    }

    fn transition(&mut self, mut result: EngineResult) -> Transition {
        self.convert_multilingual_commit(&mut result);
        let commit_cursor_suffix = self.expand_quick_phrase_commit(&mut result);
        // 每一次上屏都经过这里，所以 AI 上文只在这一处喂；插件给的行不是用户打的字，不进 AI 和重排的上文。
        if result.has_commit && !self.commits_plugin_row() {
            let before_cursor = commit_cursor_suffix
                .as_deref()
                .and_then(|suffix| result.commit.strip_suffix(suffix))
                .unwrap_or(&result.commit);
            self.remember_commit(before_cursor);
        }
        Transition {
            commit_context: result.has_commit.then(|| self.output_context()),
            handled: result.handled,
            commit: result.has_commit.then_some(result.commit),
            commit_cursor_suffix,
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
    ///
    /// AI 行不按上表的座位坐（#6888）。其余行先排好：有云候选时按上表分组，没有云候选时保持引擎的次序，和 AI 回答到达之前一模一样。第一条 AI 候选再插进第一页最后一格（下标 `page_size - 1`；每页只有一格时是下标 1，不顶掉首选；第一页排不满时紧跟在其余行之后），同一批多出来的排在整个列表最后。回答到达时第一页前 `page_size - 1` 格一格都不动，原来在最后一格的候选和它后面的行各往后挪一格。`page_size` 在组字中途不会变（`set_page_size` 只在空闲时生效），翻页、展开刷新时这里按同一个值重算，AI 行始终在第一页最后一格，数字键按的就是画出来的那一格。
    fn normalize_online_slots(&mut self) -> bool {
        const CLOUD: u8 = 2;
        const CLOUD_PENDING: u8 = 15;
        const AI: u8 = 3;
        const ENGLISH: u8 = 4;
        const EMOJI: u8 = 6;
        const KAOMOJI: u8 = 7;
        const AI_PENDING: u8 = 12;
        const JAPANESE: u8 = 13;
        let is_ai = |source: u8| matches!(source, AI | AI_PENDING);

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
            .any(|source| *source == CLOUD || is_ai(*source))
        {
            return false;
        }
        let mut order_scratch = std::mem::take(&mut self.online_order_scratch);
        let mut fixed_english_scratch = std::mem::take(&mut self.online_fixed_english_scratch);

        const LOCAL_GROUP: usize = 0;
        const CLOUD_GROUP: usize = 1;
        const AI_GROUP: usize = 2;
        const JAPANESE_GROUP: usize = 3;
        const ENGLISH_GROUP: usize = 4;
        const EMOJI_GROUP: usize = 5;
        const KAOMOJI_GROUP: usize = 6;
        let group_of = |source: u8| match source {
            CLOUD | CLOUD_PENDING => CLOUD_GROUP,
            AI | AI_PENDING => AI_GROUP,
            JAPANESE => JAPANESE_GROUP,
            ENGLISH => ENGLISH_GROUP,
            EMOJI => EMOJI_GROUP,
            KAOMOJI => KAOMOJI_GROUP,
            _ => LOCAL_GROUP,
        };
        let mut counts = [0; 7];
        let mut fixed_english_count = 0;
        for (source, position) in snapshot
            .candidate_sources
            .iter()
            .zip(&snapshot.candidate_positions)
        {
            counts[group_of(*source)] += 1;
            fixed_english_count += usize::from(*source == ENGLISH && *position > 0);
        }
        let has_cloud = counts[CLOUD_GROUP] > 0;
        let ai_count = counts[AI_GROUP];
        let non_ai = count - ai_count;
        // 联系人姓名与组合文本占据开头的连续位置，云与 AI 候选都不能拆开这一组。
        let contact_prefix = snapshot
            .candidate_sources
            .iter()
            .take_while(|source| **source == msime_engine::CandidateSource::Contact as u8)
            .count();
        // 第一条 AI 候选通常坐第一页最后一格；小页数也必须让完整联系人组保持在它前面。
        let ai_seat = self
            .page_size
            .saturating_sub(1)
            .max(1)
            .max(contact_prefix)
            .min(non_ai);

        // 有云候选时其余行每组有优先段和剩余段，组内按引擎原顺序排列。
        let mut head_starts = [0; 7];
        let mut head_counts = [0; 7];
        let mut tail_starts = [0; 7];
        if has_cloud {
            // 引擎把完全匹配或固定在首位的英文保留在首位；领先的日文混输行也不被在线行挤走。
            let promoted_english =
                counts[LOCAL_GROUP] > 0 && snapshot.candidate_sources[0] == ENGLISH;
            let promoted_japanese = counts[LOCAL_GROUP] > 0
                && snapshot.candidate_sources[usize::from(promoted_english)] == JAPANESE;
            let mut reading = snapshot.reading.chars();
            let single_kana = scheme_type(snapshot.scheme) == Some(SchemeType::JapaneseRomaji)
                && matches!((reading.next(), reading.next()), (Some(kana), None) if ('\u{3041}'..='\u{3096}').contains(&kana));
            let local_prefix = (if single_kana { 2 } else { 1 }).max(contact_prefix);

            let mut next_seat = 0;
            let mut head = |group: usize, limit: usize| {
                let limit = if group == ENGLISH_GROUP {
                    // 额外英文只占首页余位，并为 AI 保留一格；原有第一行英文优先位继续保留。
                    let following = if promoted_english {
                        counts[LOCAL_GROUP].min(local_prefix)
                            + counts[CLOUD_GROUP]
                            + counts[JAPANESE_GROUP].min(1)
                    } else {
                        0
                    };
                    limit.min(
                        self.page_size
                            .saturating_sub(next_seat + following + usize::from(ai_count > 0))
                            .max(1),
                    )
                } else {
                    limit
                };
                head_starts[group] = next_seat;
                head_counts[group] = counts[group].min(limit);
                next_seat += head_counts[group];
            };
            if promoted_english {
                head(ENGLISH_GROUP, self.english_first_page_count);
            }
            if promoted_japanese {
                head(JAPANESE_GROUP, 1);
            }
            head(LOCAL_GROUP, local_prefix);
            head(CLOUD_GROUP, counts[CLOUD_GROUP]);
            if !promoted_japanese {
                head(JAPANESE_GROUP, 1);
            }
            if !promoted_english {
                head(ENGLISH_GROUP, self.english_first_page_count);
            }
            head(EMOJI_GROUP, 1);
            head(KAOMOJI_GROUP, 1);
            for group in [
                LOCAL_GROUP,
                ENGLISH_GROUP,
                JAPANESE_GROUP,
                EMOJI_GROUP,
                KAOMOJI_GROUP,
            ] {
                tail_starts[group] = next_seat;
                next_seat += counts[group] - head_counts[group];
            }
            debug_assert_eq!(next_seat, non_ai);
        }

        // 第二次扫描直接计算每行的新座位；全部未变位时不建立索引表。
        let mut order = None;
        let mut seen = [0; 7];
        let mut arranged_without_ai = 0;
        for (index, source) in snapshot.candidate_sources.iter().enumerate() {
            let group = group_of(*source);
            let ordinal = seen[group];
            seen[group] += 1;
            let seat = if group == AI_GROUP {
                if ordinal == 0 {
                    ai_seat
                } else {
                    non_ai + ordinal
                }
            } else {
                // 这一行在不含 AI 行的排列里的座位；第一条 AI 候选插进去后，它和它后面的行各往后挪一格。
                let arranged = if !has_cloud {
                    arranged_without_ai
                } else if ordinal < head_counts[group] {
                    head_starts[group] + ordinal
                } else {
                    tail_starts[group] + ordinal - head_counts[group]
                };
                arranged_without_ai += 1;
                arranged + usize::from(ai_count > 0 && arranged >= ai_seat)
            };
            if seat != index && order.is_none() {
                let mut candidate_order = std::mem::take(&mut order_scratch);
                candidate_order.clear();
                candidate_order.extend(0..count);
                order = Some(candidate_order);
            }
            if let Some(order) = &mut order {
                order[seat] = index;
            }
        }
        // 有云候选时固定英文行在在线座位排好后归位，云候选不能把它挤离指定座位（参考 server/src/ipc/candidate_selection_policy.h 的 NormalizeMixedCandidateOrder）。位置从 1 开始，超出末尾时夹到末尾；来源统计给出实际行数，空缓冲不申请存储。没有云候选时其余行保持引擎的次序，引擎已经按固定位置排好了它们。
        if has_cloud && fixed_english_count > 0 {
            let order = order.get_or_insert_with(|| {
                let mut candidate_order = std::mem::take(&mut order_scratch);
                candidate_order.clear();
                candidate_order.extend(0..count);
                candidate_order
            });
            // 固定英文只在其余行里归位：先把第一条 AI 候选移回其余行之后，归位完再放回它的座位；AI 的尾部始终留在原处。
            if ai_count > 0 {
                order[ai_seat..=non_ai].rotate_left(1);
            }
            fixed_english_scratch.clear();
            fixed_english_scratch.reserve_exact(fixed_english_count);
            let mut write = 0;
            for read in 0..non_ai {
                let index = order[read];
                if snapshot.candidate_sources[index] == ENGLISH
                    && snapshot.candidate_positions[index] > 0
                {
                    fixed_english_scratch.push(index);
                } else {
                    order[write] = index;
                    write += 1;
                }
            }
            fixed_english_scratch.sort_by_key(|index| snapshot.candidate_positions[*index]);
            for &index in &fixed_english_scratch {
                let seat = usize::from(snapshot.candidate_positions[index] - 1)
                    .max(contact_prefix)
                    .min(write);
                order.copy_within(seat..write, seat + 1);
                order[seat] = index;
                write += 1;
            }
            debug_assert_eq!(write, non_ai);
            if ai_count > 0 {
                order[ai_seat..=non_ai].rotate_right(1);
            }
        }
        let Some(order) = order else {
            self.online_order_scratch = order_scratch;
            self.online_fixed_english_scratch = fixed_english_scratch;
            return false;
        };
        // 排列必须包含全部候选，恒等排列不应用到候选数组。
        debug_assert_eq!(order.len(), count);
        if order.len() != count || order.iter().enumerate().all(|(seat, index)| seat == *index) {
            self.online_order_scratch = order;
            self.online_fixed_english_scratch = fixed_english_scratch;
            return false;
        }
        self.apply_candidate_order(&order);
        self.online_order_scratch = order;
        self.online_fixed_english_scratch = fixed_english_scratch;
        true
    }

    /// 内置的呈现排序：逐键重排、整句读法降位、在线候选的座位。refresh、展开和落定重排三处都走这里，免得三处的步骤漂移。返回是否挪动了候选。
    fn order_presentation(&mut self) -> bool {
        if self.cached.prediction {
            return false;
        }
        let mut moved = self.rerank(true);
        moved |= self.demote_runner_up_readings();
        moved |= self.normalize_online_slots();
        moved
    }

    /// `contests` 为真时把九宫格标的窄裁决点（`EngineSnapshot::candidate_contest`）交给排序决策，只有逐键模型这样调：裁决点的 margin 是在随包的键盘模型上调出来的，落定模型（`rerank_settled`、`score_settled_rerank`）没有量过，照旧按 `trusted_leader` 不越过可信的词典首选。
    fn rerank(&mut self, contests: bool) -> bool {
        // 整句改字的候选是光标处一段的替换，按读音长短排好，不参与整句的重排。光标后编辑的首选接着固定的那段，必须留在首位。
        if !self.cached.conversion.is_empty() || self.cached.caret_edit_suffix {
            return false;
        }
        let promote = {
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
            // 方案、五笔表码和上文窗口的判断都在 `rerank_pick` 里，这里同步旋转候选元数据和引擎座位映射。
            let rows = order_rows(snapshot);
            let rows = if contests {
                rows.with_contests(&snapshot.candidate_contest)
            } else {
                rows
            };
            rerank_pick(
                reranker.as_mut(),
                &self.ai_context,
                snapshot.scheme,
                snapshot.answered_by_pinyin_fallback,
                &rows,
            )
        };
        let Some(promote) = promote else {
            return false;
        };
        self.promote_row(promote)
    }

    /// 把 `promote` 旋到首位，并同步候选元数据和引擎座位映射。下标为 0 或越界时不动。
    fn promote_row(&mut self, promote: usize) -> bool {
        let count = self.cached.candidates.len();
        if promote == 0 || promote >= count {
            return false;
        }
        let snapshot = &self.cached;
        if snapshot.candidate_sources.len() != count
            || snapshot.candidate_codes.len() != count
            || snapshot.candidate_annotations.len() != count
            || snapshot.candidate_positions.len() != count
            || snapshot.candidate_corrected.len() != count
            || snapshot.candidate_answers_key.len() != count
        {
            return false;
        }
        ensure_engine_order(&mut self.engine_order, count);
        let snapshot = &mut self.cached;
        rotate_to_front(&mut snapshot.candidates, promote);
        rotate_to_front(&mut snapshot.candidate_codes, promote);
        rotate_to_front(&mut snapshot.candidate_annotations, promote);
        rotate_to_front(&mut snapshot.candidate_sources, promote);
        rotate_to_front(&mut snapshot.candidate_positions, promote);
        rotate_to_front(&mut snapshot.candidate_corrected, promote);
        rotate_to_front(&mut snapshot.candidate_answers_key, promote);
        if snapshot.candidate_static_evidence.len() == count {
            rotate_to_front(&mut snapshot.candidate_static_evidence, promote);
        }
        if snapshot.candidate_contest.len() == count {
            rotate_to_front(&mut snapshot.candidate_contest, promote);
        }
        if snapshot.candidate_touch_supported.len() == count {
            rotate_to_front(&mut snapshot.candidate_touch_supported, promote);
        }
        if snapshot.candidate_confident_correction.len() == count {
            rotate_to_front(&mut snapshot.candidate_confident_correction, promote);
        }
        rotate_to_front(&mut self.engine_order, promote);
        true
    }

    /// 将领先的整句读法保持在顶部附近，其余读法移到列表末尾。
    ///
    /// 规则本身（保留几条整句读法、哪些算整句、为什么挪而不删）见 `msime_engine::ordering::runner_up_order`；这里只把排列同步应用到候选元数据和引擎座位映射上。
    pub(crate) fn demote_runner_up_readings(&mut self) -> bool {
        if !self.cached.conversion.is_empty() {
            return false;
        }
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
        let Some(order) = runner_up_order_with(
            snapshot.scheme,
            &order_rows(snapshot),
            &snapshot.candidate_touch_supported,
        ) else {
            return false;
        };
        self.apply_candidate_order(&order);
        true
    }

    pub(crate) fn refresh_online_candidates(&mut self) -> Result<(), RuntimeError> {
        // 在线结果不改变组字，只保留前后都存在的词条释义；代次校验照旧。
        let mut translations = std::mem::take(&mut self.translations);
        let mut highlight_translations = std::mem::take(&mut self.highlight_translations);
        retain_candidate_translations(&mut translations, &self.cached.candidates);
        retain_candidate_translations(&mut highlight_translations, &self.cached.candidates);
        self.refresh()?;
        retain_candidate_translations(&mut translations, &self.cached.candidates);
        retain_candidate_translations(&mut highlight_translations, &self.cached.candidates);
        self.translations = translations;
        self.highlight_translations = highlight_translations;
        Ok(())
    }

    /// 组字没变时，异步改写候选不能把用户已经翻到的页弹回第一页。
    ///
    /// 高亮还停在第一条时留在第一条：云候选插进第二格，空格选中的仍是新的第一条。
    /// 已经翻页或移动过高亮时，按文本和编码找回那一条。它若被排回更前的页，不跟回去，
    /// 停在原来那一页的开头。否则云联想回包一到，`+` 刚翻开的下一页就会弹回第一页。
    fn keep_highlight_on_same_candidate(
        &mut self,
        previous_highlight: usize,
        text: &str,
        code: Option<&str>,
    ) {
        let visible = self.visible_len();
        if visible == 0 {
            self.highlighted = 0;
            return;
        }
        let page_size = self.page_size.max(1);
        let page = previous_highlight / page_size;
        let page_start = page * page_size;
        let found = self
            .cached
            .candidates
            .iter()
            .enumerate()
            .filter(|(index, candidate)| {
                *index < visible
                    && *index / page_size >= page
                    && candidate.as_str() == text
                    && code.is_none_or(|expected| {
                        self.cached.candidate_codes.get(*index).map(String::as_str)
                            == Some(expected)
                    })
            })
            .min_by_key(|(index, _)| index.abs_diff(previous_highlight))
            .map(|(index, _)| index);
        self.highlighted = found.unwrap_or(page_start).min(visible - 1);
    }

    /// 挂着的逐键打分器快到能在上屏时同步排联想（[`CandidateScorer::scores_prediction_pool_within_a_frame`]）。引擎的联想候选池按它开关。
    fn orders_prediction(&self) -> bool {
        self.reranker
            .as_deref()
            .is_some_and(CandidateScorer::scores_prediction_pool_within_a_frame)
    }

    /// 上屏后联想的词库续写交给逐键打分器排序，排序规则和「每次上屏只排一次」都在引擎里（`Session::order_prediction`）；没挂打分器、或打分器不够快（[`Self::orders_prediction`]）时什么都不做。
    ///
    /// 在取快照之前做，引擎的行已经是最终顺序：座位就是引擎下标，选词照常按代次核对，插件的变换看到的也是排好的行。这次上屏的文字要等 [`Self::transition`] 才进 `ai_context`，所以上文只到这次上屏之前，引擎自己接上刚上屏的词；暂存的词段（`phrase_prefix`）会和这次的词一起上屏，接在上文末尾。
    fn order_prediction(&mut self) {
        if !self.orders_prediction() {
            return;
        }
        let Some(scorer) = self.reranker.as_mut() else {
            return;
        };
        let before = if self.phrase_prefix.is_empty() {
            std::borrow::Cow::Borrowed(self.ai_context.as_str())
        } else {
            std::borrow::Cow::Owned(format!("{}{}", self.ai_context, self.phrase_prefix))
        };
        self.engine.order_prediction(scorer.as_mut(), &before);
    }

    pub(crate) fn refresh(&mut self) -> Result<(), RuntimeError> {
        // 派发类调用里先问插件指令，行在取快照之前写进引擎：同一次派发，代次不变。
        self.run_command_provider();
        self.snapshot_valid = false;
        let had_translations =
            !self.translations.is_empty() || !self.highlight_translations.is_empty();
        self.translations.clear();
        self.highlight_translations.clear();
        self.engine_order.clear();
        let previous_highlight = self.highlighted;
        self.highlighted = 0;
        self.order_prediction();
        // Drop cached candidate identities even if fetching the replacement fails.
        let snapshot = match self.engine.snapshot() {
            Ok(snapshot) => snapshot,
            Err(error) => {
                self.forget_inserted_rows();
                self.cached = EngineSnapshot {
                    prediction: false,
                    scheme: 255,
                    nine_key: false,
                    key_grid: None,
                    nine_key_spellings: Vec::new(),
                    nine_key_reading: String::new(),
                    nine_key_single_character: false,
                    nine_key_strokes: String::new(),
                    grid_composing: false,
                    candidate_annotations: Vec::new(),
                    candidate_codes: Vec::new(),
                    candidate_sources: Vec::new(),
                    multilingual_conversion: None,
                    candidate_positions: Vec::new(),
                    candidate_corrected: Vec::new(),
                    candidate_answers_key: Vec::new(),
                    candidate_static_evidence: Vec::new(),
                    candidate_contest: Vec::new(),
                    candidate_touch_supported: Vec::new(),
                    candidate_confident_correction: Vec::new(),
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
                    display_reading: String::new(),
                    editing_text: String::new(),
                    caret_position: 0,
                    segment_raw_boundaries: vec![],
                    candidates: Vec::new(),
                    conversion: String::new(),
                    conversion_focus_start: 0,
                    conversion_focus_end: 0,
                    caret_edit_suffix: false,
                    caret_edit_prefix: String::new(),
                };
                // 失败占位快照已经没有候选，旧插件操作不能在下一次旁路刷新时复活。
                self.transform_candidates();
                // 占位快照替换了整个视图，两边都按变了算。
                self.bump_composition_epoch();
                self.bump_candidates_epoch();
                return Err(error);
            }
        };
        let previous = std::mem::replace(&mut self.cached, snapshot);
        self.forget_inserted_rows();
        self.order_presentation();
        // 派发类调用里的 refresh 重新运行插件，其余只重放；隐藏的行数和插件注释的变化也算候选视图的变化。
        let plugin_marks_changed = self.transform_candidates();
        self.snapshot_valid = true;
        let composition_held = self.cached.editing_text == previous.editing_text
            && self.cached.scheme == previous.scheme
            && self.cached.local_mode == previous.local_mode
            && self.cached.reading == previous.reading
            && self.cached.dedicated_english == previous.dedicated_english
            && self.cached.conversion == previous.conversion
            && self.cached.conversion_focus_start == previous.conversion_focus_start;
        if composition_held {
            let candidates_held = self.cached.candidates == previous.candidates
                && self.cached.candidate_codes == previous.candidate_codes
                && self.cached.candidate_annotations == previous.candidate_annotations
                && self.cached.candidate_sources == previous.candidate_sources
                && self.cached.multilingual_conversion == previous.multilingual_conversion
                && self.cached.candidate_positions == previous.candidate_positions
                && self.cached.candidate_corrected == previous.candidate_corrected
                && self.cached.candidate_answers_key == previous.candidate_answers_key
                && self.cached.candidate_static_evidence == previous.candidate_static_evidence
                && self.cached.candidate_contest == previous.candidate_contest
                && self.cached.candidate_touch_supported == previous.candidate_touch_supported
                && self.cached.candidate_confident_correction
                    == previous.candidate_confident_correction;
            if candidates_held {
                self.highlighted = previous_highlight.min(self.visible_len().saturating_sub(1));
            } else if previous_highlight > 0 {
                if let Some(text) = previous.candidates.get(previous_highlight) {
                    let code = previous
                        .candidate_codes
                        .get(previous_highlight)
                        .map(String::as_str);
                    self.keep_highlight_on_same_candidate(previous_highlight, text, code);
                }
            }
        }
        let cached = &self.cached;
        if cached.preedit != previous.preedit
            || cached.editing_text != previous.editing_text
            || cached.caret_position != previous.caret_position
            || cached.reading != previous.reading
            || cached.display_reading != previous.display_reading
            || cached.conversion != previous.conversion
            || cached.conversion_focus_start != previous.conversion_focus_start
            || cached.conversion_focus_end != previous.conversion_focus_end
            || cached.caret_edit_suffix != previous.caret_edit_suffix
            || cached.caret_edit_prefix != previous.caret_edit_prefix
        {
            self.bump_composition_epoch();
        }
        let cached = &self.cached;
        if had_translations
            || plugin_marks_changed
            || self.highlighted != previous_highlight
            || cached.candidates != previous.candidates
            || cached.candidate_codes != previous.candidate_codes
            || cached.candidate_annotations != previous.candidate_annotations
            || cached.candidate_sources != previous.candidate_sources
            || cached.multilingual_conversion != previous.multilingual_conversion
            || cached.candidate_positions != previous.candidate_positions
            || cached.candidate_corrected != previous.candidate_corrected
            || cached.candidate_confident_correction != previous.candidate_confident_correction
            || cached.candidate_list_open != previous.candidate_list_open
            || cached.prediction != previous.prediction
        {
            self.bump_candidates_epoch();
        }
        Ok(())
    }

    pub fn focus(&mut self, focused: bool) -> Result<Transition, RuntimeError> {
        self.facts = DispatchFacts::default();
        self.selected_text.clear();
        self.selected_multilingual = None;
        self.advance()?;
        // Invalidate the client before cancellation, including on engine failure.
        self.focused = false;
        // 获得焦点只重置组字，不送出旧词段；先清掉暂存和撤回记录，取消或刷新失败也不能在下一次聚焦时补交。
        if focused {
            self.phrase_selections.clear();
            if !self.phrase_prefix.is_empty() {
                self.phrase_prefix.clear();
                self.bump_composition_epoch();
            }
        }
        // A Korean syllable is text the user already wrote, not a reading still to be converted, so leaving the client commits it, as it does for every scheme that commits on blur; every other composition is cancelled. Attaching a client (`focused`) stays a pure reset: a syllable typed in the previous client must never be written into the new one.
        let commits_on_blur = !focused
            && scheme_type(self.cached.scheme).is_some_and(SchemeType::commits_on_blur)
            && !self.cached.dedicated_english
            && self.cached.local_mode == "none"
            && !self.cached.editing_text.is_empty();
        let commit_context = self.output_context();
        let result = if commits_on_blur {
            self.engine.finish(0)
        } else {
            self.discard_composition()
        };
        let may_commit = result
            .as_ref()
            .is_ok_and(|result| result.has_commit || (!focused && !self.phrase_prefix.is_empty()));
        let refresh_error = match self.refresh() {
            Ok(()) => None,
            Err(error) if may_commit => Some(error),
            Err(error) => return Err(error),
        };
        let mut result = result?;
        if let Some(error) = refresh_error {
            result.diagnostic = format!("Candidate refresh failed: {error}");
        }
        // 失焦时送出用户已经选好的暂存段；获得焦点时旧词段已丢弃，不会产生提交。
        self.hold_phrase_progress(false, false, false, false, "", &mut result);
        self.focused = focused;
        // A different client is a different sentence, so context never leaks
        // from one application into another. The Engine's committed-word context follows the same rule: the next word
        // no longer follows the last one, and no later pick may take back what a commit in the other client taught.
        self.ai_context.clear();
        self.engine.set_rescoring_context("");
        self.engine.reset_context();
        let mut transition = self.transition(result);
        if transition.commit.is_some() {
            transition.commit_context = Some(commit_context);
        }
        Ok(transition)
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
        let mut finished = self.finish_seat(self.highlighted)?;
        self.record_selection(
            SelectionHow::PunctuationFinish,
            self.highlighted,
            &finished,
            None,
        );
        let punctuation = match self.engine.punctuation(value) {
            Ok(result) => result,
            Err(error) if finished.has_commit => {
                // Completion already changed Engine state: never discard that commit.
                finished.commit.push(char::from(value));
                self.facts.tail_chars = 1;
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
                self.facts.tail_chars = 1;
                return Ok(literal_mark(value, punctuation.diagnostic));
            }
            // 没有组字可结束时上屏的只有标点；暂存段若随后拼在前面，标点仍是尾巴。
            if punctuation.has_commit {
                self.facts.tail_chars = char_count(&punctuation.commit);
            }
            return Ok(punctuation);
        }
        finished.handled = true;
        if punctuation.has_commit {
            finished.commit.push_str(&punctuation.commit);
            self.facts.tail_chars = char_count(&punctuation.commit);
        } else if !punctuation.handled {
            // ASCII mode/unsupported symbols still terminate composition in one commit.
            finished.commit.push(char::from(value));
            self.facts.tail_chars = 1;
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
        // 已发布的组字符号仍交给引擎；空闲的模式入口和普通拼音的网址触发键保留 ASCII 标点语义。
        let spells = self.cached.local_mode != "none"
            || (self.phrase_prefix.is_empty()
                && (scheme_type(self.cached.scheme)
                    .is_some_and(|scheme| !scheme.opens_table_modes())
                    || (!self.cached.editing_text.is_empty()
                        && !matches!(value, b'/' | b'@' | b'.' | b':'))));
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
            let raw = self.engine.command(Command::CommitRaw)?;
            self.facts.raw = raw.has_commit;
            raw
        } else {
            let finished = self.finish_seat(self.highlighted)?;
            self.record_selection(
                SelectionHow::PunctuationFinish,
                self.highlighted,
                &finished,
                None,
            );
            finished
        };
        if !finished.has_commit {
            // As in `punctuation`: a held phrase piece takes the key, so the mark goes out with it.
            if !self.phrase_prefix.is_empty() {
                self.facts.tail_chars = 1;
                return Ok(literal_mark(value, finished.diagnostic));
            }
            return Ok(finished);
        }
        finished.handled = true;
        finished.commit.push(char::from(value));
        self.facts.tail_chars = 1;
        Ok(finished)
    }

    pub fn dispatch(&mut self, action: Action) -> Result<Transition, RuntimeError> {
        // 桌面上只有派发类调用运行脚本插件的候选变换，旁路调用只重放，见 `transform` 模块。
        #[cfg(any(
            target_os = "macos",
            target_os = "windows",
            all(target_os = "linux", not(target_env = "ohos"))
        ))]
        {
            self.dispatch_live(action)
        }
        #[cfg(not(any(
            target_os = "macos",
            target_os = "windows",
            all(target_os = "linux", not(target_env = "ohos"))
        )))]
        {
            self.dispatch_action(action)
        }
    }

    pub(crate) fn dispatch_action(&mut self, action: Action) -> Result<Transition, RuntimeError> {
        // 每次调用都从空白开始，提前返回的路径（报错、失焦）也不会留下上一键的事实。
        self.facts = DispatchFacts::default();
        self.selected_text.clear();
        self.selected_multilingual = None;
        if matches!(&action, Action::Punctuation(value) | Action::PunctuationAscii(value) if !value.is_ascii_punctuation())
        {
            return Err(RuntimeError::InvalidPunctuation);
        }
        // Cache maintenance belongs to the session, including while its host
        // has no focus. Ordinary input must still pass through unchanged.
        if !self.focused && !matches!(&action, Action::ResetCache) {
            return Ok(self.transition(empty_result(false)));
        }
        // 空闲联想没有可编辑的组字；导航键清掉建议后交还编辑器，不能沿用普通候选的高亮导航。
        if self.cached.prediction
            && matches!(
                &action,
                Action::NextCandidate
                    | Action::PreviousCandidate
                    | Action::NextPage
                    | Action::PreviousPage
                    | Action::FirstCandidate
                    | Action::FirstCandidateOrHome
                    | Action::LastCandidate
                    | Action::Command(
                        Command::MoveLeft
                            | Command::MoveRight
                            | Command::MoveHome
                            | Command::MoveEnd
                            | Command::ConversionLeft
                            | Command::ConversionRight
                    )
                    | Action::SegmentMoveLeft
                    | Action::SegmentMoveRight
            )
        {
            self.advance()?;
            self.engine.command(Command::Cancel)?;
            self.refresh()?;
            return Ok(self.transition(empty_result(false)));
        }
        // 只在当前页有目标候选时接管；否则继续原标点路径，不能像越界数字键那样吞掉。
        let action = self.candidate_shortcut(action);
        // 横排候选里，首选上左右键编辑输入串（→ 到末尾为止），其余候选上左右键移动候选（偏好 `navigation.left_right`）；竖排时左右键只编辑。
        let action = self.caret_edge_candidate_shortcut(action);
        let action = self.home_key_action(action);
        if let Action::SelectAnyCandidate(id) = &action {
            if id.session != self.session
                || id.generation != self.generation
                || id.index >= self.visible_len()
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
                || id.index >= (start + self.page_size).min(self.visible_len())
            {
                return Err(RuntimeError::StaleCandidate);
            }
        }
        if let Action::ChooseNineKeySpelling(id) = &action {
            if id.session != self.session
                || id.generation != self.generation
                // 九键和 14 键共用这一列拼写。
                || self.cached.key_grid.is_none()
                || id.index >= self.cached.nine_key_spellings.len()
            {
                return Err(RuntimeError::StaleNineKeySpelling);
            }
        }
        if let Action::FixCandidatePosition(_, position) = &action {
            if !(1..=5).contains(position) {
                return Err(RuntimeError::Engine(
                    "Candidate position must be between 1 and 5".into(),
                ));
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
        // 高亮、数字、空格和点选都只落在可见前缀里，被隐藏的行选不到。
        let len = self.visible_len();
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
            // 这里不经过 refresh，候选视图的变化只能在这里记。
            if index != self.highlighted {
                self.bump_candidates_epoch();
            }
            self.highlighted = index;
            return Ok(self.transition(empty_result(true)));
        }
        // Going back into the phrase, before the Engine sees the key: both rules replace what the
        // key would otherwise do.
        if let Some(transition) = self.retreat_phrase_selection(&action)? {
            return Ok(transition);
        }
        // 选词前保留读音，消耗的前缀才可用于撤回。数字键可能选词，CommitCandidate 命令
        // 也可能消费引擎首行；标点和导航不在这里记录读音。
        let selection_action = matches!(
            &action,
            Action::Character { .. }
                | Action::Select(_)
                | Action::SelectAnyCandidate(_)
                | Action::SelectEdge(..)
                | Action::SelectHighlighted
                | Action::Command(Command::CommitCandidate)
        );
        let reading_before =
            (self.phrase_preedit && selection_action).then(|| self.cached.editing_text.clone());
        // A digit on the candidate page picks a candidate; the Engine is asked the same question as
        // for Select, so it can begin a phrase the same way.
        let mut selected_by_digit = false;
        // 五笔 14 键的一键与字母键一样可能凑满四码。
        let character_action = matches!(action, Action::Character { .. } | Action::GridKey(_));
        // 四码已有五笔候选时，后续字母通常顶字上屏并开始下一组编码。混输拼音能继续当前候选的读音时，
        // 由引擎保留组字，避免把五字母拼音截成四码。判定本身是纯函数（`wubi_top_commit_on_next_key`），
        // 开关、混输、四码完整性与读音续输四项都在那里，方便单测覆盖。
        let wubi_top_commit = match action {
            Action::Character { value, shift } if value.is_ascii_alphabetic() => {
                wubi_top_commit_on_next_key(&WubiTopCommitRequest {
                    top_commit_enabled: self.wubi_fifth_code_top_commit,
                    mixed_pinyin: self.engine.wubi_mixed_pinyin(),
                    four_code_is_complete: self.snapshot_valid
                        && wubi_four_code_is_complete(&self.cached),
                    phrase_prefix_empty: self.phrase_prefix.is_empty(),
                    pinyin_continues: self.engine.wubi_pinyin_continues(value, shift),
                })
            }
            // 五笔 14 键：组码不进混输拼音，四码之后的一键只能顶字，不看混输开关。
            Action::GridKey(_) => wubi_top_commit_on_next_key(&WubiTopCommitRequest {
                top_commit_enabled: self.wubi_fifth_code_top_commit,
                mixed_pinyin: false,
                four_code_is_complete: self.snapshot_valid
                    && self.cached.grid_composing
                    && wubi_four_code_is_complete(&self.cached),
                phrase_prefix_empty: self.phrase_prefix.is_empty(),
                pinyin_continues: false,
            }),
            _ => false,
        };
        // 九宫格组码已经到上限（`DIGIT_LIMIT`）时再按一键：先替用户选掉首选腾出位置，再把这一键接着打，和顶字一样不丢键。引擎只在光标在末尾、首选确实占用组码时说可以；否则这一键照旧被拦下并给出提示。
        let grid_limit_pick = match action {
            Action::Character { value, .. } => self.engine.grid_limit_pick_ready(value, false),
            Action::GridKey(letter) => self.engine.grid_limit_pick_ready(letter, true),
            _ => false,
        };
        // 这次把哪个座位的候选交给了引擎、经由哪条路径，引擎调用之后再与它的上屏核对（`selection_fact`）。标点结束组字的两条路由在 `punctuation`、`punctuation_ascii` 里自己记。
        let mut pick: Option<(SelectionHow, usize)> = None;
        // 原样上屏读音的路由：bare 前缀上的 Finish 和 `CommitRaw` 命令。
        let mut raw_route = false;
        // 顶字时接在首选后面的那段文本的字符数。
        let mut top_tail = 0;
        // Windows `input.escape_keeps_selected_word`（出厂关）：造词时第一次 Esc 只丢掉还没选的拼写、
        // 留下已经选好的词，再按一次才整体取消（小狼毫的 ClearNonConfirmedComposition）。判据与
        // `ShouldEscapeKeepSelectedWord` 相同：开关开着、还有没选的拼写、不在临时日文（R 模式只能整段重置）。
        // 词段由 runtime 暂存，引擎的 Cancel 正好清掉拼写而留下它。
        let escape_keeps_word = self.escape_keeps_selected_word
            && matches!(action, Action::Command(Command::Cancel))
            && !self.cached.editing_text.is_empty()
            && !self.phrase_prefix.is_empty()
            && self.cached.local_mode != "temporary_japanese";
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
            Action::Finish | Action::FinishOnFocusLoss if self.bare_mode_prefix() => {
                raw_route = true;
                self.engine.command(Command::CommitRaw)
            }
            // 失焦时用户没在看候选窗：插件给的行不替用户上屏，写进文档的是用户自己打的 `/xxx`。
            Action::FinishOnFocusLoss if self.highlights_plugin_row() => {
                raw_route = true;
                self.engine.command(Command::CommitRaw)
            }
            // 失焦时插件插进普通候选的行也不替用户上屏：结束在引擎的首行上，和没有插件时一样。
            Action::FinishOnFocusLoss if self.is_inserted(self.highlighted) => {
                let seat = self.first_engine_seat();
                pick = Some((SelectionHow::Finish, seat));
                self.engine.finish(self.engine_index(seat))
            }
            Action::Finish | Action::FinishOnFocusLoss => {
                pick = Some((SelectionHow::Finish, self.highlighted));
                self.finish_seat(self.highlighted)
            }
            // 顶字键不能丢：引擎提交的候选先上屏，再把这一键交给下一组组字；引擎放开的字母则按字面接在候选后面。
            Action::Character { value, shift } if wubi_top_commit => {
                let first = self.first_engine_seat();
                pick = Some((SelectionHow::WubiTop, first));
                self.engine
                    .select(self.engine_index(first))
                    .and_then(|mut committed| {
                        let next = self.engine.character(value, shift)?;
                        let tail = if next.has_commit {
                            next.commit
                        } else if !next.handled {
                            char::from(value).to_string()
                        } else {
                            String::new()
                        };
                        if !tail.is_empty() {
                            top_tail = char_count(&tail);
                            committed.commit.push_str(&tail);
                            committed.handled = true;
                            committed.has_commit = true;
                        }
                        Ok(committed)
                    })
            }
            // 五笔 14 键四码之后的一键：先顶首选上屏，这一键再开始下一组编码，与 26 键的第五码相同。
            Action::GridKey(letter) if wubi_top_commit => {
                let first = self.first_engine_seat();
                pick = Some((SelectionHow::WubiTop, first));
                self.engine
                    .select(self.engine_index(first))
                    .and_then(|picked| Ok(join_after_pick(picked, self.engine.grid_key(letter)?)))
            }
            Action::Character { value, shift } if grid_limit_pick => {
                let first = self.first_engine_seat();
                pick = Some((SelectionHow::GridLimit, first));
                self.engine
                    .select(self.engine_index(first))
                    .and_then(|picked| {
                        Ok(join_after_pick(
                            picked,
                            self.engine.character(value, shift)?,
                        ))
                    })
            }
            Action::GridKey(letter) if grid_limit_pick => {
                let first = self.first_engine_seat();
                pick = Some((SelectionHow::GridLimit, first));
                self.engine
                    .select(self.engine_index(first))
                    .and_then(|picked| Ok(join_after_pick(picked, self.engine.grid_key(letter)?)))
            }
            // 暂存词段后的模式入口按标点结束；Tab 属于剩余拼写的辅助码，仍走字符接口。
            Action::Character { value, .. }
                if value != b'\t'
                    && !self.phrase_prefix.is_empty()
                    && self.cached.local_mode == "none"
                    && self.cached.spelling_symbols.as_bytes().contains(&value) =>
            {
                self.punctuation(value)
            }
            Action::Character { value, shift } => {
                self.engine.character(value, shift).and_then(|result| {
                    // 九键和 14 键的分词键 `'` 是键面上的切分动作，不是中文引号标点。
                    if !result.handled && self.cached.key_grid.is_some() && value == b'\'' {
                        return Ok(result);
                    }
                    // 已有提交文本时直接交还结果，再跑标点路由会丢掉快捷模式刚提交的原文。
                    if !result.handled && !result.has_commit && value.is_ascii_punctuation() {
                        return self.punctuation(value);
                    }
                    // Space the Engine let go over an open list, with nothing committed, is a pick of the highlighted row, as the Space command is (Zhuyin leaves it to the runtime; Korean commits its syllable first and so is not a pick).
                    if !result.handled
                        && !result.has_commit
                        && value == b' '
                        && self.cached.candidate_list_open
                        && len > 0
                    {
                        pick = Some((SelectionHow::SpaceOverList, self.highlighted));
                        return self.select_seat(self.highlighted);
                    }
                    // Let Engine consume numeric input (Unicode mode, nine-key, etc.) first. A result that already committed (a Korean syllable the digit ended) is final: selecting now would replace that commit and lose the text. A digit the scheme spells with (a Zhuyin tone or phonetic key) is never a pick, even one the Engine let go: Zhuyin leaves 1-9 to selection only while its list is open, when they are not spelling symbols, so `0` there stays ㄢ.
                    let Some(slot) = candidate_digit_slot(value, self.page_size) else {
                        return Ok(result);
                    };
                    if result.handled
                        || result.has_commit
                        || (self.cached.local_mode == "none"
                            && self.cached.spelling_symbols.as_bytes().contains(&value))
                        || self.cached.nine_key
                        || self.cached.key_grid.is_some_and(KeyGrid::uses_digits)
                        || self.cached.prediction
                        || len == 0
                    {
                        return Ok(result);
                    }
                    let page_start = (self.highlighted / self.page_size) * self.page_size;
                    if slot >= self.page_size || page_start + slot >= len {
                        return Ok(empty_result(true));
                    }
                    selected_by_digit = true;
                    pick = Some((SelectionHow::Digit, page_start + slot));
                    self.select_seat(page_start + slot)
                })
            }
            // 进入整句改字时改的是用户高亮着的那一句，它可能被重排过，不在引擎的首位。
            // 高亮在插件插入的行上时从引擎的首行进入。
            Action::Command(Command::ConversionLeft) if self.cached.conversion.is_empty() => {
                let seat = if self.is_inserted(self.highlighted) {
                    self.first_engine_seat()
                } else {
                    self.highlighted
                };
                self.engine.conversion_left_from(self.engine_index(seat))
            }
            // 14 键的组码不是可上屏的字母，两种原样提交都与 `Finish` 一样从宿主高亮行结束组字；它们在这里都不涉及英文原文学词。
            Action::Command(Command::CommitRaw | Command::CommitRawWithoutLearning)
                if self.cached.grid_composing
                    && self
                        .cached
                        .key_grid
                        .is_some_and(|grid| !grid.offers_raw_key())
                    && len > 0 =>
            {
                pick = Some((SelectionHow::GridRaw, self.highlighted));
                self.finish_seat(self.highlighted)
            }
            Action::Command(Command::CommitCandidate) => {
                // 命令由引擎选择原始首行，展示可能已经重排；不能把它记成高亮行。
                let seat = if self.engine_order.is_empty() {
                    (!self.cached.candidates.is_empty()).then_some(0)
                } else {
                    self.engine_order.iter().position(|index| *index == 0)
                };
                pick = seat.map(|seat| (SelectionHow::FirstCandidateCommand, seat));
                self.engine.command(Command::CommitCandidate)
            }
            Action::Command(command) => {
                raw_route = matches!(
                    command,
                    Command::CommitRaw | Command::CommitRawWithoutLearning
                );
                self.engine.command(command)
            }
            Action::SegmentBackspace => self.engine.segment_command(SegmentCommand::Backspace),
            Action::SegmentMoveLeft => self.engine.segment_command(SegmentCommand::MoveLeft),
            Action::SegmentMoveRight => self.engine.segment_command(SegmentCommand::MoveRight),
            Action::Select(id) => {
                pick = Some((SelectionHow::Select, id.index));
                self.select_seat(id.index)
            }
            Action::SelectAnyCandidate(id) => {
                pick = Some((SelectionHow::AnyCandidate, id.index));
                self.select_seat(id.index)
            }
            // 插件插入的行不是词库里的词：以词定字、置顶、删除、固定位置对它什么都不做。
            Action::SelectEdge(id, _)
            | Action::PinCandidate(id)
            | Action::RemoveCandidate(id)
            | Action::FixCandidatePosition(id, _)
            | Action::ClearCandidatePosition(id)
                if self.is_inserted(id.index) =>
            {
                Ok(empty_result(true))
            }
            Action::SelectEdge(id, edge) => {
                pick = Some((SelectionHow::Edge, id.index));
                self.engine.select_edge(self.engine_index(id.index), edge)
            }
            Action::PinCandidate(id) => self.engine.pin_candidate(self.engine_index(id.index)),
            Action::RemoveCandidate(id) => {
                self.engine.remove_candidate(self.engine_index(id.index))
            }
            Action::FixCandidatePosition(id, position) => self
                .engine
                .fix_candidate_position(self.engine_index(id.index), position),
            Action::ClearCandidatePosition(id) => self
                .engine
                .clear_candidate_position(self.engine_index(id.index)),
            Action::ChooseNineKeySpelling(id) => self.engine.choose_nine_key_spelling(id.index),
            Action::GridKey(letter) => self.engine.grid_key(letter),
            Action::SetNineKeyFilter {
                single_character,
                ref strokes,
            } => self.engine.set_nine_key_filter(single_character, strokes),
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
            Action::SelectHighlighted if len > 0 && !self.cached.prediction => {
                pick = Some((SelectionHow::Highlighted, self.highlighted));
                self.select_seat(self.highlighted)
            }
            Action::SelectHighlighted => self.engine.command(Command::CommitCandidate),
            _ => return Ok(self.transition(empty_result(false))),
        };
        // 快照还是引擎调用之前的那份，座位和候选文本都按用户当时看到的核对。
        if let Ok(result) = &result {
            if let Some((how, seat)) = pick {
                self.record_selection(
                    how,
                    seat,
                    result,
                    match action {
                        Action::SelectEdge(_, edge) => Some(edge),
                        _ => None,
                    },
                );
            }
            if raw_route {
                self.facts.raw = result.has_commit;
            }
            if top_tail > 0 {
                self.facts.tail_chars = top_tail;
            }
        }
        // Keep the pre-refresh mode only when this action can produce a commit. Most keystrokes
        // leave the composition open, so copying local_mode for them is wasted work. A held phrase
        // and the automatic Wubi top-commit can produce a commit after the Engine result itself
        // says otherwise.
        let needs_commit_context = result.as_ref().is_ok_and(|result| result.has_commit)
            || !self.phrase_prefix.is_empty()
            || self.wubi_should_auto_commit(character_action);
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
        if self.wubi_should_auto_commit(character_action) {
            let first = self.first_engine_seat();
            result = self.engine.select(self.engine_index(first))?;
            // 这次的结果整个换成了自动上屏，之前记下的事实随之作废。
            self.record_selection(SelectionHow::WubiAuto, first, &result, None);
            self.facts.tail_chars = 0;
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
        // 五笔顶字不是造词中途的选词：第五码只是开始下一组编码，顶下来的首选已经是用户定下的字，与 14 键顶字（`GridKey` 不算选择动作）一样立即上屏，不进暂存词段。暂存会让它挡住之后每一次顶字和四码唯一自动上屏（两者都要求没有暂存词段），续打的字母被引擎截在四码外丢掉，Esc 还会连已顶的字一起丢。
        let picked = (selected_by_digit || selection_action || grid_limit_pick) && !wubi_top_commit;
        // Esc 默认丢弃整段组字和已选词，与参考实现的 _HandleCancel 一致。
        // 开启保留已选词时，第一次只丢掉未选拼写，词段留下。
        let discarded = matches!(action, Action::Command(Command::Cancel)) && !escape_keeps_word;
        // Segment editing on an emptied reading leaves the held phrase for the next segment key or Backspace, instead of sending it to the document (the reference's `keep_creating_word_after_empty_raw`).
        let keep_empty = matches!(
            action,
            Action::SegmentBackspace | Action::SegmentMoveLeft | Action::SegmentMoveRight
        );
        self.hold_phrase_progress(
            picked,
            discarded,
            keep_empty,
            escape_keeps_word,
            consumed,
            &mut result,
        );
        // 选中的文本上屏了，读音却还在：这次选择只用掉了一部分读音。顶字时剩下的是新打的那一键，不算。
        if let Some(selection) = self.facts.selection.as_mut() {
            if selection.outcome == SelectionOutcome::Committed
                && selection.how != SelectionHow::WubiTop
                && !(selection.how == SelectionHow::GridLimit
                    && self.cached.editing_text.chars().count() <= 1)
                && !self.cached.editing_text.is_empty()
            {
                selection.outcome = SelectionOutcome::Partial;
            }
        }
        let mut transition = self.transition(result);
        if let Some(commit_context) = commit_context {
            if transition.commit.is_some() {
                transition.commit_context = Some(commit_context);
            }
        }
        Ok(transition)
    }

    /// 本次按键是否以唯一五笔候选被提交收尾。读缓存快照，所以 refresh 前后的两处调用会故意看到不同代。
    fn wubi_should_auto_commit(&self, character_action: bool) -> bool {
        wubi_auto_commit_on_fourth_key(&WubiAutoCommitRequest {
            auto_commit_enabled: self.wubi_auto_commit_unique,
            character_action,
            snapshot_valid: self.snapshot_valid,
            unique_four_code: self.cached.wubi_unique_four_code,
            phrase_prefix_empty: self.phrase_prefix.is_empty(),
        })
    }
}

/// 先选掉首选、再把同一键交给引擎时，两步合成一个结果：选中的词在前，这一键若也上屏了什么就接在后面。这一键本身已经算处理过了。
fn join_after_pick(mut picked: EngineResult, next: EngineResult) -> EngineResult {
    if next.has_commit {
        picked.commit.push_str(&next.commit);
        picked.has_commit = true;
    }
    picked.handled = true;
    if picked.diagnostic.is_empty() {
        picked.diagnostic = next.diagnostic;
    }
    picked
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

/// `DispatchFacts::tail_chars` 用的字符数（Unicode 标量），超出 `u32` 时取上限。
fn char_count(text: &str) -> u32 {
    u32::try_from(text.chars().count()).unwrap_or(u32::MAX)
}

/// 数字键在当前页里选的格子：`1`–`9` 是前九格；`0` 只在每页十个时选第十格，每页不到十个时它不是选词键，照旧交回去。
fn candidate_digit_slot(value: u8, page_size: usize) -> Option<usize> {
    match value {
        b'1'..=b'9' => Some(usize::from(value - b'1')),
        b'0' if page_size >= usize::from(MAX_CANDIDATE_PAGE_SIZE) => Some(9),
        _ => None,
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

/// `View::key_grid` 的取值。
fn key_grid_name(grid: Option<KeyGrid>) -> &'static str {
    match grid {
        None => "none",
        Some(KeyGrid::NineKey) => "nine_key",
        Some(KeyGrid::EighteenKey) => "eighteen_key",
        Some(KeyGrid::FourteenKey | KeyGrid::FourteenKeyShuangpin(_)) => "fourteen_key",
        Some(KeyGrid::NineKeyShuangpin(_)) => "nine_key_shuangpin",
        Some(KeyGrid::EighteenKeyShuangpin(_)) => "eighteen_key",
    }
}

/// 四码唯一自动上屏的五项输入。
pub(crate) struct WubiAutoCommitRequest {
    /// 用户开着「五码…四码唯一候选自动上屏」。
    pub auto_commit_enabled: bool,
    /// 这一键是字符键：退格、方向键和恢复组字都不该触发上屏。
    pub character_action: bool,
    /// 快照还没失效，`unique_four_code` 是这一代的答案。
    pub snapshot_valid: bool,
    /// 引擎答过的完整四码，且整个候选列表只有这一个词。
    pub unique_four_code: bool,
    /// 没有暂存的词段：造词时 raw 还是正在拼的前缀，提交它会提前结束这个词。
    pub phrase_prefix_empty: bool,
}

/// 第四键是否把唯一候选自动上屏。
///
/// 五项全是用户或引擎给出的事实，判定本身不再问任何状态，宿主重排、插件和平台适配都不参与。
pub(crate) fn wubi_auto_commit_on_fourth_key(request: &WubiAutoCommitRequest) -> bool {
    request.auto_commit_enabled
        && request.character_action
        && request.snapshot_valid
        && request.unique_four_code
        && request.phrase_prefix_empty
}

/// 顶字判定的五项输入，每一项都由调用方先问出来，函数本身只看布尔值。
pub(crate) struct WubiTopCommitRequest {
    /// 用户开着「第五码将首选上屏」。
    pub top_commit_enabled: bool,
    /// 五笔拼音混输开着。
    pub mixed_pinyin: bool,
    /// 当前组字是码表答过的完整四码（[`wubi_four_code_is_complete`]），且快照没有失效、没有暂存词段。
    pub four_code_is_complete: bool,
    /// 没有暂存的词段：造词时 raw 还是正在拼的前缀，提交它会提前结束这个词。
    pub phrase_prefix_empty: bool,
    /// 引擎说这个字母还能继续当前混输拼音的读音。
    pub pinyin_continues: bool,
}

/// 完整四码之后再敲一个字母时，是否先顶字上屏首选、再把这个字母留作下一组开头。
///
/// 关掉只在混输打开时生效：那时字母并进混输串，用户想继续打拼音。混输关掉时引擎会把码裁回四位，
/// 字母无处容纳，宿主已经按下的键不能丢，所以仍按顶字上屏——与来源 `ShouldCommitCompleteWubiCodeOnNextKey`
/// 的 `(top_commit_enabled || !mixed_pinyin_enabled)` 是同一个条件。`!pinyin_continues` 是水杉自己加的（#8619）：字母能继续混输拼音的读音时，无论开关如何都保留组字，否则 `guiyi` 这类首四码碰上五笔整码的拼音打不出来。
pub(crate) fn wubi_top_commit_on_next_key(request: &WubiTopCommitRequest) -> bool {
    (request.top_commit_enabled || !request.mixed_pinyin)
        && request.four_code_is_complete
        && request.phrase_prefix_empty
        && !request.pinyin_continues
}

/// 从引擎快照判断本码表是否答过完整四码：非英文或快捷模式、非拼音回退，光标在末尾且有候选。
/// 带 `z` 的猜码仍需用户确认，不能把它当成完整编码自动顶字。
pub(crate) fn wubi_four_code_is_complete(snapshot: &EngineSnapshot) -> bool {
    const WUBI_COMPLETE_CODE_LENGTH: usize = 4;
    scheme_type(snapshot.scheme) == Some(SchemeType::Wubi)
        && !snapshot.dedicated_english
        && snapshot.local_mode == "none"
        && !snapshot.nine_key
        && !snapshot.answered_by_pinyin_fallback
        && snapshot.editing_text.len() == WUBI_COMPLETE_CODE_LENGTH
        && !snapshot.editing_text.contains('z')
        && msime_client_core::is_ascii_alphabetic(&snapshot.editing_text)
        && snapshot.caret_position == WUBI_COMPLETE_CODE_LENGTH
        && !snapshot.candidates.is_empty()
}
