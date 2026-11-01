//! `ImeSession` (core-session.md §6): owns the active scheme and the providers, turns every key into a fresh request and candidate list, and runs the wubi mixed-pinyin fallback merge. Also the list-level helpers the input session builds on: local and mixed candidate queries, the online batch rule and the personal context rerank.

pub mod online_batch;
pub mod personal_rerank;
pub mod queries;
pub mod registry;
pub mod scheme;

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::Arc;

use crate::assets;
use crate::error::Result;
use crate::helpcode::SharedKeymap;
use crate::japanese::matrix::BestPath;
use crate::japanese::provider::WholeReadingLemma;
use crate::japanese::romaji::is_romaji_complete;
use crate::japanese::JapaneseRomajiScheme;
use crate::paths::RuntimePaths;
use crate::pinyin::autocorrect::autocorrect_suppression_key;
use crate::quanpin::QuanpinScheme;
use crate::shuangpin::query::{
    apply_segmentation_cases_into, detect_active_double_helpcode_length, remove_manual_delimiters,
    segment_input, to_quanpin_segmentation, trim_trailing_letters_preserve_delimiters,
};
use crate::shuangpin::ShuangpinProfile;
use crate::shuangpin::ShuangpinScheme;
use crate::touch::{KeyGeometry, KeyTouch, TouchTuning};
use crate::types::{
    autocorrect_type, CandidateSource, FirstPageRows, FuzzyPinyinOptions, QueryRequest, SchemeKey,
    SchemeSet, SchemeType, SentenceAssociationOptions, ShuangpinProfileKind, TextConversion,
    WordItem, WubiInputOptions,
};
use crate::user_dictionary::typo_profile::PersonalTypoProfile;
use crate::vietnamese::{
    InputMethod as VietnameseInputMethod, ToneStyle as VietnameseToneStyle, VietnameseScheme,
};
use crate::wubi::scheme::PINYIN_LOOKUP_PREFIX;
use crate::zhuyin::scheme::{ListCandidate, ZhuyinKey, ZhuyinScheme};

/// 日文混输「认得」的成本上限：最佳整句的路径成本不超过每个假名这么多。随包模型里每个假名（连小写的ぁぃぅぇぉ）都有单字词条，未知假名回退从来用不上，只看回退等于什么都认得（`xiexie` 是ぃエィ絵），所以再按成本卡。按真实模型校准（`real_model_calibrates_mixed_japanese_signals`）：拼音拆不开又没有精确词条的日文（食べます 1430、わかった 1448、疲れた 1575、待って 1768 每假名）都在 1800 以内，`konnichiha` 的今イチは（2114）、`hello` 的へっぉ（3593）、`tokyo` 的ときょ（2985）在外面；拿 `quanpin-words-v1` 的拼音删一个键或对调两键造出的 36180 个拆不开的输入里，没有精确词条却过了这条线的有 155 个（0.43%），多是 `n` 规则拼出的ちゃうん、じゃん一类。
const MIXED_JAPANESE_KNOWN_COST_PER_KANA: i64 = 1_800;

/// 日文混输「精确」信号要求的最短读音：整段读音至少这么多拍（`count_morae`，拗音的小写假名不单算）才算精确。mozc 词库里两拍的读音几乎都有词条（人名、片假名词），只看有没有词条时 421 个单音节拼音里 177 个（`ai` あい、`hao` はお、`kai` かい、`you` よう、`shou` しょう……）会混进日文；按真实模型校准（`real_model_calibrates_mixed_japanese_signals`），要求三拍以后单音节剩 20 个（`bian` びあん、`jian` 一类），`quanpin-words-v1` 25119 个词里从 1537 个降到 931 个，`sushi`（すし）也不再混进来；按字符数算三个时拗音、外来音的读法（`shou`、`shen` しぇん、`fan` ふぁん）挡不住，单音节还有 45 个。
const MIXED_JAPANESE_EXACT_MIN_MORAE: usize = 3;

/// 日文混输的「精确」信号：整段读音有完全相同读音的词条，并且读音至少 `MIXED_JAPANESE_EXACT_MIN_MORAE` 拍。
pub(crate) fn counts_as_exact_japanese(lemma: WholeReadingLemma) -> bool {
    lemma.exact && lemma.morae >= MIXED_JAPANESE_EXACT_MIN_MORAE
}

/// 日文混输的「认得」信号：最佳整句没有未知假名，成本也在 `MIXED_JAPANESE_KNOWN_COST_PER_KANA` 以内。
pub(crate) fn reads_as_known_japanese(path: &BestPath) -> bool {
    path.unknown_kana == 0
        && path.cost
            <= MIXED_JAPANESE_KNOWN_COST_PER_KANA
                .saturating_mul(i64::try_from(path.kana).unwrap_or(i64::MAX))
}

// 混输拼音回退的短批次直接扫描已有词，避免为一次合并创建临时哈希表。
const SMALL_PINYIN_FALLBACK: usize = 64;

use registry::ProviderRegistry;
use scheme::Scheme;

/// What the providers answered for the current composition.
#[derive(Debug, Clone, PartialEq, Default)]
pub struct CompositionState {
    pub preedit: String,
    pub request: QueryRequest,
    pub candidates: Vec<WordItem>,
    /// 候选对应的完整五笔 86 编码，仅供宿主显示反查提示。
    pub wubi_codes: Vec<String>,
}

/// One decode of a request, before it is stored as the live state.
struct Decoded {
    candidates: Vec<WordItem>,
    /// Some row matches the whole wubi code; prefix rows do not count.
    wubi_table_answered: bool,
}

pub struct ImeSession {
    scheme: Scheme,
    registry: ProviderRegistry,
    state: CompositionState,
    /// 光标前缀查询复用的临时请求存储，避免每次分页重新分配请求字段。
    scratch_request: QueryRequest,
    profile: &'static ShuangpinProfile,
    vietnamese_method: VietnameseInputMethod,
    vietnamese_style: VietnameseToneStyle,
    korean_layout: crate::KoreanLayout,
    force_cantonese_shuangpin: bool,
    wubi_options: WubiInputOptions,
    /// Mixed wubi only: a pinyin candidate was picked out of this composition, so the rest decodes as quanpin until the composition ends (product decision 2026-09-30, the intent of test_wubi_mixed_input_session.cpp:194-212). Cleared by `reset`, `switch_scheme`, turning mixed input off, and an emptied composition.
    pinyin_tail: bool,
    /// 注音九键模式。注音编辑器被换掉再新建时（切到别的方案再切回来）由这里带过去。
    zhuyin_nine_key: bool,
    autocorrect_types: u32,
    quanpin_helpcode: bool,
    strict_quanpin_helpcode: bool,
    shuangpin_helpcode: bool,
    shuangpin_helpcode_modes: crate::types::ShuangpinHelpcodeModes,
    fuzzy: FuzzyPinyinOptions,
    sentence_alternatives: bool,
    /// 首页行数随每个请求交给各方案引擎；只影响词库要不要并入补全词。
    first_page_rows: FirstPageRows,
    sentence_association: SentenceAssociationOptions,
    rescoring_context: String,
    typo_profile: Arc<PersonalTypoProfile>,
    /// 五笔混输拼音回退使用的方案和请求缓冲，逐键刷新时复用。
    mixed_pinyin_scheme: QuanpinScheme,
    mixed_pinyin_request: QueryRequest,
    /// 日文混输把拼音组字当罗马字查日文时用的方案和请求缓冲，逐键复用。
    mixed_japanese_scheme: JapaneseRomajiScheme,
    mixed_japanese_request: QueryRequest,
    /// 触屏宿主注册的键位几何，换方案后新建的双拼方案也拿到它；桌面永远是 `None`。
    key_geometry: Option<Arc<KeyGeometry>>,
    /// 按触点计价的参数，随每个请求带给双拼引擎。
    touch_tuning: TouchTuning,
}

/// 日文混输对当前组字的判断：整段按罗马字转换出的最佳日文，以及决定要不要给、放在哪里的信号。
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MixedJapaneseConversion {
    /// 整段读音的最佳整句，与日文方案候选的第一行相同。
    pub text: String,
    /// 整段读音至少三拍，并且正好是日文词库里某个词条的读音（`counts_as_exact_japanese`）。
    pub exact: bool,
    /// 最佳整句每个假名都落在词条里（没有用到未知假名回退），并且路径成本不超过每个假名 `MIXED_JAPANESE_KNOWN_COST_PER_KANA`。
    pub known: bool,
    /// 拼音拆不成完整音节（全拼 `is_complete_pinyin_input`，双拼 `is_complete_input` 为假），与混输英文领先用的是同一个判定。
    pub pinyin_incomplete: bool,
}

impl ImeSession {
    /// ime_session.cpp:42-49. `cantonese_dictionary`, `zhuyin_dictionary` and `stroke_dictionary` are where `msime-cantonese.db`, `msime-zhuyin.db` and `msime-stroke.db` are, each read only when its scheme is activated; starting in Cantonese, Zhuyin or Stroke fails as `switch_scheme` does when its file cannot be opened. `japanese_dictionary` 是 `msime-japanese.dat` 的位置，为空时读资源目录里的那份。`enabled` 是会话允许运行的方案，`scheme` 不在其中时报 `INPUT_SCHEME_NOT_ENABLED`。
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        scheme: SchemeType,
        enabled: SchemeSet,
        profile: &'static ShuangpinProfile,
        paths: &RuntimePaths,
        cantonese_dictionary: PathBuf,
        zhuyin_dictionary: PathBuf,
        stroke_dictionary: PathBuf,
        japanese_dictionary: PathBuf,
    ) -> Result<Self> {
        let mut registry = ProviderRegistry::new(
            enabled,
            profile,
            paths,
            cantonese_dictionary,
            zhuyin_dictionary,
            stroke_dictionary,
            japanese_dictionary,
        );
        registry.activate(scheme)?;
        let zhuyin = registry.take_dictionary(scheme);
        let mut session = Self {
            scheme: Scheme::new(
                scheme,
                profile,
                VietnameseInputMethod::default(),
                VietnameseToneStyle::default(),
                crate::KoreanLayout::default(),
                registry.cantonese_inventory(),
                zhuyin,
            )?,
            registry,
            state: CompositionState::default(),
            scratch_request: QueryRequest::default(),
            profile,
            vietnamese_method: VietnameseInputMethod::default(),
            vietnamese_style: VietnameseToneStyle::default(),
            korean_layout: crate::KoreanLayout::default(),
            force_cantonese_shuangpin: false,
            wubi_options: WubiInputOptions::default(),
            pinyin_tail: false,
            zhuyin_nine_key: false,
            autocorrect_types: 0,
            quanpin_helpcode: false,
            strict_quanpin_helpcode: false,
            shuangpin_helpcode: false,
            shuangpin_helpcode_modes: crate::types::ShuangpinHelpcodeModes::default(),
            fuzzy: FuzzyPinyinOptions::default(),
            sentence_alternatives: false,
            first_page_rows: FirstPageRows::default(),
            sentence_association: SentenceAssociationOptions::default(),
            rescoring_context: String::new(),
            typo_profile: PersonalTypoProfile::shared(&paths.user(assets::USER_JOURNAL)),
            mixed_pinyin_scheme: QuanpinScheme::new(),
            mixed_pinyin_request: QueryRequest::default(),
            mixed_japanese_scheme: JapaneseRomajiScheme::new(),
            mixed_japanese_request: QueryRequest::default(),
            key_geometry: None,
            touch_tuning: TouchTuning::default(),
        };
        session.bind_wubi_scheme();
        Ok(session)
    }

    /// 选中的触屏入口决定粤拼如何解释同形两键码；新建或切回粤拼时沿用。
    pub fn set_force_cantonese_shuangpin(&mut self, enabled: bool) {
        self.force_cantonese_shuangpin = enabled;
        if let Scheme::Cantonese(scheme) = &mut self.scheme {
            scheme.set_force_shuangpin(enabled);
        }
    }

    /// 粤拼候选的输出转换（例如繁转简）；`None` 照词库原样输出。只影响之后的查询。
    pub fn set_cantonese_output_conversion(&mut self, conversion: Option<TextConversion>) {
        self.registry.set_cantonese_output_conversion(conversion);
    }

    pub fn candidates(&self) -> &[WordItem] {
        &self.state.candidates
    }

    pub fn candidate_wubi_code(&self, word: &str) -> Option<&str> {
        self.state
            .candidates
            .iter()
            .zip(&self.state.wubi_codes)
            .find(|(candidate, _)| candidate.word == word)
            .and_then(|(_, code)| (!code.is_empty()).then_some(code.as_str()))
    }

    /// 仅在候选身份仍占据同一座位且已有编码时按位置读取反查码；身份不一致或该座位没有编码时交给调用方回退扫描。
    pub(crate) fn candidate_wubi_code_at_if_item(
        &self,
        index: usize,
        item: &WordItem,
    ) -> Option<&str> {
        let candidate = self.state.candidates.get(index)?;
        if !same_candidate_identity(candidate, item) {
            return None;
        }
        let code = self.state.wubi_codes.get(index)?.as_str();
        (!code.is_empty()).then_some(code)
    }

    /// 按候选身份回退到已有的反查码；只读缓存，不重新查询五笔词库。
    pub(crate) fn candidate_wubi_code_if_item(&self, item: &WordItem) -> Option<&str> {
        self.state
            .candidates
            .iter()
            .zip(&self.state.wubi_codes)
            .find(|(candidate, _)| same_candidate_identity(candidate, item))
            .and_then(|(_, code)| (!code.is_empty()).then_some(code.as_str()))
    }

    pub fn request(&self) -> &QueryRequest {
        &self.state.request
    }

    pub fn preedit(&self) -> &str {
        &self.state.preedit
    }

    pub fn current_scheme_type(&self) -> SchemeType {
        self.scheme.scheme_type()
    }

    /// 当前粤拼组字是否已由首个双拼码切换到粤语双拼解释。
    pub fn cantonese_shuangpin_active(&self) -> bool {
        self.scheme
            .as_cantonese()
            .is_some_and(crate::cantonese::CantoneseScheme::is_shuangpin)
    }

    /// 当前粤拼组字是否接受 `;` 作为双拼韵母键。
    pub fn cantonese_shuangpin_accepts_semicolon(&self) -> bool {
        self.scheme
            .as_cantonese()
            .is_some_and(crate::cantonese::CantoneseScheme::accepts_semicolon)
    }

    /// 会话允许运行的方案。
    pub fn enabled_schemes(&self) -> SchemeSet {
        self.registry.enabled()
    }

    /// 日文混输开着时在会话构造里调用：后台预热日文模型。会话不带日文时什么也不做。
    pub fn warm_up_japanese(&self) {
        self.registry.warm_up_japanese();
    }

    /// 日文混输（实验选项）：把当前全拼、双拼组字的整段输入当成罗马字，判断要不要混进一行日文。只在可以给出一行时答 `Some`，也就是整段读音至少三拍且有完全相同读音的词条（`exact`），或者拼音拆不开且最佳整句的每个假名都认得（`pinyin_incomplete && known`）。领不领先由会话按这几个信号定（`InputSession::mixed_japanese_row`）。输入必须只有 a-z（不带撇号、`;`）、整段都能转成假名、至少两个假名；方案不是全拼、双拼，会话不带日文（没有 provider），或者日文模型缺失时一律答 `None`，只能拼出假名的输入不算。先算便宜的两个信号，拼音拆得开又没有精确词条时不跑整句搜索。
    pub fn mixed_japanese_conversion(&mut self) -> Option<MixedJapaneseConversion> {
        let scheme = self.current_scheme_type();
        if !matches!(scheme, SchemeType::Quanpin | SchemeType::Shuangpin) {
            return None;
        }
        let raw = self.state.request.raw_input.as_str();
        if raw.is_empty()
            || !raw.bytes().all(|byte| byte.is_ascii_lowercase())
            || !is_romaji_complete(raw)
        {
            return None;
        }
        let pinyin_incomplete = !queries::spells_whole_syllables(raw, scheme, self.profile);
        self.mixed_japanese_scheme.set_raw_input(raw, "");
        let mut request = std::mem::take(&mut self.mixed_japanese_request);
        self.mixed_japanese_scheme.build_request_into(&mut request);
        let conversion = self
            .registry
            .japanese_whole_reading_lemma(&request)
            .map(counts_as_exact_japanese)
            .filter(|exact| *exact || pinyin_incomplete)
            .and_then(|exact| {
                let path = self.registry.japanese_whole_reading_best_path(&request)?;
                let known = reads_as_known_japanese(&path);
                (exact || (pinyin_incomplete && known)).then_some(MixedJapaneseConversion {
                    text: path.text,
                    exact,
                    known,
                    pinyin_incomplete,
                })
            });
        self.mixed_japanese_request = request;
        conversion
    }

    /// `scheme.handle_key` then `refresh_candidates`; `SchemeKey::Requery` only refreshes.
    pub fn handle_key(&mut self, key: SchemeKey) -> bool {
        if let SchemeKey::Letter(letter) = key {
            if self.wubi_pinyin_continues(letter) {
                if let Some(wubi) = self.scheme.as_wubi_mut() {
                    wubi.set_extended_length_allowed(true);
                }
            }
        }
        let changed = if key != SchemeKey::Requery {
            self.scheme.handle_key(key)
        } else if self.current_scheme_type() == SchemeType::Cantonese {
            // 粤拼词典只读，候选不受在线词或会话设置影响；重查询保留当前快照即可。
            return false;
        } else {
            false
        };
        self.refresh_candidates();
        changed
    }

    /// Opens what `scheme` reads (`msime-cantonese.db` for Cantonese, `msime-zhuyin.db` for Zhuyin, `msime-stroke.db` for Stroke) without switching to it, so a caller can learn that the scheme is unavailable before it discards anything; `switch_scheme` to an activated scheme cannot fail. A live Zhuyin scheme already holds `msime-zhuyin.db`, so activating Zhuyin again opens nothing.
    pub fn activate(&mut self, scheme: SchemeType) -> Result<()> {
        if scheme == SchemeType::Zhuyin && self.scheme.as_zhuyin().is_some() {
            return Ok(());
        }
        self.registry.activate(scheme)
    }

    /// A new scheme and an empty state. Cantonese, Zhuyin and Stroke open their dictionary the first time they are activated and keep it for the session; when that fails (`LANGUAGE_DICTIONARY_UNAVAILABLE`, `LANGUAGE_DICTIONARY_VERSION_UNSUPPORTED`) the scheme is unavailable and the current scheme and its composition stay as they were. 不在 `enabled_schemes` 里的方案同样不可用（`INPUT_SCHEME_NOT_ENABLED`），当前方案和组合保持不变。
    pub fn switch_scheme(&mut self, scheme: SchemeType) -> Result<()> {
        self.activate(scheme)?;
        if let Some(zhuyin) = self
            .scheme
            .as_zhuyin_mut()
            .filter(|_| scheme == SchemeType::Zhuyin)
        {
            // The live editor holds `msime-zhuyin.db`; an idle editor over the same connection is the new scheme.
            zhuyin.reset();
        } else {
            let next = Scheme::new(
                scheme,
                self.profile,
                self.vietnamese_method,
                self.vietnamese_style,
                self.korean_layout,
                self.registry.cantonese_inventory(),
                self.registry.take_dictionary(scheme),
            )?;
            if let Some(dictionary) =
                std::mem::replace(&mut self.scheme, next).into_zhuyin_dictionary()
            {
                self.registry.return_dictionary(dictionary);
            }
            // 新建的注音编辑器沿用会话的九键模式；上面复用编辑器的分支只 `reset`，模式本来就在。
            let nine_key = self.zhuyin_nine_key;
            if let Some(zhuyin) = self.scheme.as_zhuyin_mut() {
                zhuyin.set_nine_key(nine_key);
            }
            if let Scheme::Cantonese(cantonese) = &mut self.scheme {
                cantonese.set_force_shuangpin(self.force_cantonese_shuangpin);
            }
        }
        self.bind_wubi_scheme();
        self.bind_shuangpin_scheme();
        self.state = CompositionState::default();
        self.pinyin_tail = false;
        Ok(())
    }

    pub fn reset(&mut self) {
        self.scheme.reset();
        self.state = CompositionState::default();
        self.pinyin_tail = false;
    }

    /// A pinyin candidate was picked out of a mixed wubi composition: what is left keeps decoding as quanpin, never as wubi codes, until the composition ends. Nothing for any other scheme or with mixed input off.
    pub fn keep_pinyin_tail(&mut self) {
        self.pinyin_tail =
            self.current_scheme_type() == SchemeType::Wubi && self.wubi_options.mixed_pinyin;
    }

    /// Host editing for whichever scheme is active; wubi also re-derives its length allowance from the mixed-pinyin option (ime_session.cpp:103-157, 199-238).
    pub fn replace_active_raw_input(&mut self, raw: &str, raw_with_cases: &str) {
        // Host editing rewrites text the composition already holds, so its clip length comes from the setting, not from what the last query answered; otherwise moving the caret through a mixed composition drops everything past the fourth letter. `refresh_candidates` restores the query-derived value.
        let mixed_pinyin = self.wubi_options.mixed_pinyin;
        if let Some(wubi) = self.scheme.as_wubi_mut() {
            wubi.set_extended_length_allowed(mixed_pinyin);
        }
        self.scheme.set_raw_input(raw, raw_with_cases);
        self.refresh_candidates();
    }

    pub fn cycle_japanese_kana_variant(&mut self) -> bool {
        let Scheme::Japanese(japanese) = &mut self.scheme else {
            return false;
        };
        if !japanese.cycle_last_kana_variant() {
            return false;
        }
        self.refresh_candidates();
        true
    }

    /// Opens the Hanja list of the composing Korean syllable. False, with the list left closed, for every other scheme and for a composition the table has no Hanja for (a lone jamo).
    pub fn open_korean_hanja(&mut self) -> bool {
        let Scheme::Korean(korean) = &mut self.scheme else {
            return false;
        };
        korean.open_hanja();
        self.refresh_candidates();
        if !self.state.candidates.is_empty() {
            return true;
        }
        self.close_korean_hanja();
        false
    }

    /// Closes the Hanja list, so the composition answers nothing again; false when no list was open.
    pub fn close_korean_hanja(&mut self) -> bool {
        let Scheme::Korean(korean) = &mut self.scheme else {
            return false;
        };
        if !korean.hanja_open() {
            return false;
        }
        korean.close_hanja();
        self.refresh_candidates();
        true
    }

    pub fn korean_hanja_open(&self) -> bool {
        matches!(&self.scheme, Scheme::Korean(korean) if korean.hanja_open())
    }

    /// The Korean syllables the last key finished, which leave the composition as a commit; empty for every other scheme.
    pub fn take_korean_commit(&mut self) -> String {
        let Scheme::Korean(korean) = &mut self.scheme else {
            return String::new();
        };
        korean.take_committed()
    }

    /// The Telex or VNI method and the tone style Vietnamese spells with. A composing Vietnamese word keeps its keystrokes and is shown again under the new rules, or as the raw keys if the first Esc already showed them.
    pub fn set_vietnamese_options(
        &mut self,
        method: VietnameseInputMethod,
        style: VietnameseToneStyle,
    ) {
        self.vietnamese_method = method;
        self.vietnamese_style = style;
        let Scheme::Vietnamese(vietnamese) = &mut self.scheme else {
            return;
        };
        let raw = vietnamese.raw().to_owned();
        let locked = vietnamese.raw_locked();
        *vietnamese = VietnameseScheme::new(method, style);
        vietnamese.set_raw_input(&raw, &raw);
        if locked {
            vietnamese.restore_raw();
        }
        self.refresh_candidates();
    }

    /// 仅在没有组字时设置布局；宿主偏好刷新在空闲时重建会话。
    pub fn set_korean_layout(&mut self, layout: crate::KoreanLayout) {
        if !self.state.request.raw_input.is_empty() {
            return;
        }
        self.korean_layout = layout;
        if let Scheme::Korean(korean) = &mut self.scheme {
            *korean = crate::korean::KoreanScheme::with_layout(layout);
        }
    }

    /// The first Esc of a Vietnamese word: the display becomes the raw keystrokes. False for every other scheme, with nothing composing, or when the raw keys already show, so the caller cancels instead.
    pub fn restore_vietnamese_raw(&mut self) -> bool {
        let Scheme::Vietnamese(vietnamese) = &mut self.scheme else {
            return false;
        };
        if !vietnamese.restore_raw() {
            return false;
        }
        self.refresh_candidates();
        true
    }

    /// The non-letter keys the composing Vietnamese word spells with (VNI's digits); empty for every other scheme.
    pub fn vietnamese_spelling_symbols(&self) -> &'static str {
        match &self.scheme {
            Scheme::Vietnamese(vietnamese) => vietnamese.spelling_symbols(),
            _ => "",
        }
    }

    /// 藏文音节串的第一次 Esc：显示切换为威利原文。其他方案、没有组字或原文已在显示时返回 false，由调用方取消组字。
    pub fn restore_tibetan_raw(&mut self) -> bool {
        let Scheme::Tibetan(tibetan) = &mut self.scheme else {
            return false;
        };
        if !tibetan.restore_raw() {
            return false;
        }
        self.refresh_candidates();
        true
    }

    /// 藏文组字已被 Esc 锁定为原文：此时它只是拉丁字母，结束键不再附加音节点或垂符。其他方案为 false。
    pub fn tibetan_raw_locked(&self) -> bool {
        matches!(&self.scheme, Scheme::Tibetan(tibetan) if tibetan.raw_locked())
    }

    /// 藏文当前状态下这个字母是否进入威利原文（威利读不了的字母不接收，由会话原样写出）；其他方案为 false。
    pub fn tibetan_claims_letter(&self, letter: u8) -> bool {
        matches!(&self.scheme, Scheme::Tibetan(tibetan) if tibetan.claims_letter(letter))
    }

    /// 藏文当前状态下要作为字符交给会话的非字母键（拼写符号和 `/`）；其他方案为空。
    pub fn tibetan_spelling_symbols(&self) -> &'static str {
        match &self.scheme {
            Scheme::Tibetan(tibetan) => tibetan.spelling_symbols(),
            _ => "",
        }
    }

    /// 按候选覆盖的字节数选择粤语候选，提交时无需复制完整候选行。
    pub fn select_cantonese_end(&mut self, end: usize) -> bool {
        let composing = self.scheme.select_cantonese_end(end);
        if composing {
            self.refresh_candidates();
        }
        composing
    }

    /// Hands one key to the Zhuyin editor and shows its new state; returns whether the editor claimed the key. False for every other scheme. Text the key committed waits in `take_zhuyin_committed`.
    pub fn handle_zhuyin_key(&mut self, key: ZhuyinKey) -> Result<bool> {
        let Some(zhuyin) = self.scheme.as_zhuyin_mut() else {
            return Ok(false);
        };
        let claimed = zhuyin.handle_key(key);
        self.refresh_candidates();
        claimed
    }

    /// Pins the text of Zhuyin list row `index` and closes the list, committing nothing; false when no Zhuyin list is open or it has no such row.
    pub fn select_zhuyin(&mut self, index: usize) -> Result<bool> {
        let Some(zhuyin) = self.scheme.as_zhuyin_mut() else {
            return Ok(false);
        };
        let selected = zhuyin.select(index);
        self.refresh_candidates();
        selected
    }

    /// The text the last Zhuyin key committed (Enter, Shift punctuation, an auto-shift); empty for every other scheme.
    pub fn take_zhuyin_committed(&mut self) -> String {
        self.scheme
            .as_zhuyin_mut()
            .map(ZhuyinScheme::take_committed)
            .unwrap_or_default()
    }

    /// Ends the Zhuyin composition and returns its converted text; the pending syllable is dropped. Empty for every other scheme.
    pub fn take_zhuyin_text(&mut self) -> String {
        let Some(zhuyin) = self.scheme.as_zhuyin_mut() else {
            return String::new();
        };
        let text = zhuyin.take_text();
        self.refresh_candidates();
        text
    }

    /// 打开或关闭注音九键模式。当前方案是注音时立即作用到编辑器（丢掉它的组字），否则在下次建出注音编辑器时生效。
    pub fn set_zhuyin_nine_key(&mut self, enabled: bool) {
        self.zhuyin_nine_key = enabled;
        if let Some(zhuyin) = self.scheme.as_zhuyin_mut() {
            zhuyin.set_nine_key(enabled);
            self.refresh_candidates();
        }
    }

    /// 把注音九键的第 `index` 个候选读音钉为目标音节的读音；没有注音编辑器、列表打开、没有目标或下标越界时为 false。
    pub fn choose_zhuyin_spelling(&mut self, index: usize) -> Result<bool> {
        let Some(zhuyin) = self.scheme.as_zhuyin_mut() else {
            return Ok(false);
        };
        let chosen = zhuyin.choose_spelling(index);
        self.refresh_candidates();
        chosen
    }

    /// 注音九键供用户钉读音的候选读音；其他方案为空。
    pub fn zhuyin_spellings(&self) -> &[String] {
        self.scheme.as_zhuyin().map_or(&[], ZhuyinScheme::spellings)
    }

    pub fn zhuyin_list_open(&self) -> bool {
        self.scheme.as_zhuyin().is_some_and(ZhuyinScheme::list_open)
    }

    /// The non-letter keys the Zhuyin editor claims in its state; empty for every other scheme.
    pub fn zhuyin_spelling_symbols(&self) -> &'static str {
        self.scheme
            .as_zhuyin()
            .map_or("", ZhuyinScheme::spelling_symbols)
    }

    /// 五笔 14 键的一键（`representative` 见 [`crate::wubi::grid::representative`]），然后 `refresh_candidates`。不是五笔、26 键组字中或码已满时为假，组字不变。
    pub fn handle_wubi_grid_key(&mut self, representative: u8) -> bool {
        let Some(wubi) = self.scheme.as_wubi_mut() else {
            return false;
        };
        if !wubi.handle_grid_key(representative) {
            return false;
        }
        self.refresh_candidates();
        true
    }

    /// 当前组字是五笔 14 键打的组码。
    pub fn wubi_grid_composing(&self) -> bool {
        self.scheme
            .as_wubi()
            .is_some_and(crate::wubi::scheme::WubiScheme::grid_composing)
    }

    /// Whether the active scheme is wubi and its code is exactly four letters.
    pub fn wubi_has_complete_code(&self) -> bool {
        self.scheme
            .as_wubi()
            .is_some_and(|wubi| wubi.has_complete_code())
    }

    /// 四码命中五笔时，后续小写字母能否继续当前拼音候选的完整读音或开始下一个音节。
    /// 只匹配真实读音前缀，简拼、纠错和模糊音不能把不相关的第五码当成拼音。
    /// 不看已放开的延长状态：四码时它只是从前三码“有任何全拼行”继承来的，`wqi` 的简拼或纠错行也会让 `wqiy` 之后的任意字母被当成续写，顶字就失效了。
    pub fn wubi_pinyin_continues(&self, letter: u8) -> bool {
        // 14 键组码不是拼音字母，不进混输。
        if !self.wubi_options.mixed_pinyin
            || self.wubi_grid_composing()
            || !self.wubi_has_complete_code()
            || !letter.is_ascii_lowercase()
        {
            return false;
        }
        let raw = &self.state.request.raw_input;
        if pinyin_prefix_can_continue_after_code(raw, letter) {
            return true;
        }
        let starts_new_pinyin_syllable = crate::pinyin::segment::is_complete_pinyin_input(raw)
            && std::str::from_utf8(&[letter]).is_ok_and(crate::pinyin::syllables::is_prefix);
        self.state.candidates.iter().any(|item| {
            item.scheme == SchemeType::Quanpin
                && (item
                    .canonical_pinyin
                    .bytes()
                    .filter(|&byte| byte != b'\'')
                    .take(raw.len() + 1)
                    .eq(raw.bytes().chain(std::iter::once(letter)))
                    || (starts_new_pinyin_syllable
                        && item
                            .canonical_pinyin
                            .bytes()
                            .filter(|&byte| byte != b'\'')
                            .eq(raw.bytes())))
        })
    }

    /// 全拼词典里每个键（以 `'` 连接的完整音节）最好那一行的权重，供滑行输入使用。
    pub fn quanpin_best_weights(&self, keys: &[String]) -> HashMap<String, i64> {
        self.registry.quanpin_best_weights(keys)
    }

    /// Candidates for a raw prefix through a scratch scheme of the current type, leaving the live composition alone (caret-prefix decoding, overlays.md §7.6).
    pub fn query_raw_candidates(&mut self, raw: &str, raw_with_cases: &str) -> Vec<WordItem> {
        self.with_raw_request(
            |session, request| session.build_raw_request(raw, raw_with_cases, request),
            |session, request| {
                if request.valid {
                    session.decode(request).candidates
                } else {
                    Vec::new()
                }
            },
        )
    }

    /// 与 [`Self::query_raw_candidates`] 相同，另外交回查询用的请求：调用方要按这份请求判断某一行是否覆盖了整段 `raw`（光标后编辑固定光标前那段时用）。
    pub fn query_raw_candidates_with_request(
        &mut self,
        raw: &str,
        raw_with_cases: &str,
    ) -> (Vec<WordItem>, QueryRequest) {
        self.with_raw_request(
            |session, request| session.build_raw_request(raw, raw_with_cases, request),
            |session, request| {
                let candidates = if request.valid {
                    session.decode(request).candidates
                } else {
                    Vec::new()
                };
                (candidates, request.clone())
            },
        )
    }

    /// 扩展当前请求的光标前缀，直接借用请求里的带大小写原文，避免调用方先复制前缀。
    pub fn expand_current_raw_prefix_initial_candidates(
        &mut self,
        raw: &str,
        prefix_end: usize,
        candidates: &mut Vec<WordItem>,
    ) -> bool {
        self.expand_current_raw_range_initial_candidates(raw, 0..prefix_end, candidates)
    }

    /// 扩展当前请求原文里 `range` 那一段（光标前缀，或光标后编辑的光标后那段）的首字母候选。
    pub fn expand_current_raw_range_initial_candidates(
        &mut self,
        raw: &str,
        range: std::ops::Range<usize>,
        candidates: &mut Vec<WordItem>,
    ) -> bool {
        let raw_with_cases = if self.state.request.raw_input_with_cases.is_empty() {
            &self.state.request.raw_input
        } else {
            &self.state.request.raw_input_with_cases
        };
        if raw_with_cases.get(range.clone()).is_none() {
            return false;
        }
        self.with_raw_request(
            |session, request| {
                let raw_with_cases = if session.state.request.raw_input_with_cases.is_empty() {
                    &session.state.request.raw_input
                } else {
                    &session.state.request.raw_input_with_cases
                };
                let raw_with_cases = &raw_with_cases[range];
                session.build_raw_request(raw, raw_with_cases, request);
            },
            |session, request| {
                request.valid
                    && session
                        .registry
                        .expand_initial_candidates(request, candidates)
            },
        )
    }

    /// The request a scratch scheme of the current type builds for `raw`, with the session's switches applied. The current scheme was activated before it became current, so building its scratch twin cannot fail, except for Zhuyin, whose `msime-zhuyin.db` connection belongs to the live editor: an invalid request stands for both, and Zhuyin keeps its caret at the end, so it never decodes a caret prefix.
    fn build_raw_request(&self, raw: &str, raw_with_cases: &str, request: &mut QueryRequest) {
        match self.current_scheme_type() {
            SchemeType::Quanpin => {
                QuanpinScheme::build_request_from_raw_into(raw, raw_with_cases, request)
            }
            SchemeType::Shuangpin => ShuangpinScheme::build_request_from_raw_into(
                self.profile,
                raw,
                raw_with_cases,
                self.helpcode_uppercase_triggers(),
                request,
            ),
            _ => return self.raw_request_through_scheme(raw, raw_with_cases, request),
        }
        self.apply_request_options(request);
        self.apply_autocorrect_suppression(request);
        apply_shuangpin_helpcode_segmentation(request, self.profile);
    }

    fn raw_request_through_scheme(
        &self,
        raw: &str,
        raw_with_cases: &str,
        request: &mut QueryRequest,
    ) {
        let Ok(mut scratch) = Scheme::new(
            self.current_scheme_type(),
            self.profile,
            self.vietnamese_method,
            self.vietnamese_style,
            self.korean_layout,
            self.registry.cantonese_inventory(),
            None,
        ) else {
            *request = QueryRequest::default();
            return;
        };
        if let Scheme::Cantonese(cantonese) = &mut scratch {
            cantonese.set_force_shuangpin(self.force_cantonese_shuangpin);
        }
        if let Some(wubi) = scratch.as_wubi_mut() {
            wubi.set_mixed_pinyin_allowed(self.wubi_options.mixed_pinyin);
            wubi.set_extended_length_allowed(self.wubi_options.mixed_pinyin);
            wubi.set_z_wildcard(self.wubi_options.z_wildcard);
            wubi.set_pinyin_lookup_allowed(self.pinyin_lookup_allowed());
        }
        scratch.set_raw_input(raw, raw_with_cases);
        let built = self.prepare_request(&scratch);
        request.clone_from(&built);
    }

    fn with_raw_request<R>(
        &mut self,
        build: impl FnOnce(&Self, &mut QueryRequest),
        use_request: impl FnOnce(&mut Self, &QueryRequest) -> R,
    ) -> R {
        let mut request = std::mem::take(&mut self.scratch_request);
        build(self, &mut request);
        let result = use_request(self, &request);
        self.scratch_request = request;
        result
    }

    pub fn set_helpcode_keymap(&mut self, keymap: Option<SharedKeymap>) {
        self.registry.set_helpcode_keymap(keymap);
        self.refresh_candidates();
    }

    pub fn wubi_input_options(&self) -> WubiInputOptions {
        self.wubi_options
    }

    pub fn set_wubi_input_options(&mut self, options: WubiInputOptions) {
        self.wubi_options = options;
        self.registry.set_wubi_profile(options.profile);
        self.pinyin_tail &= options.mixed_pinyin;
        self.bind_wubi_scheme();
    }

    pub fn set_autocorrect_types(&mut self, types: u32) {
        self.autocorrect_types = types;
    }

    pub fn set_quanpin_helpcode_enabled(&mut self, enabled: bool) {
        self.quanpin_helpcode = enabled;
    }

    pub fn set_strict_quanpin_helpcode(&mut self, enabled: bool) {
        self.strict_quanpin_helpcode = enabled;
    }

    pub fn set_shuangpin_helpcode_enabled(&mut self, enabled: bool) {
        self.shuangpin_helpcode = enabled;
        // 末尾大写触发归谁同时看总开关，方案层要重算。
        self.bind_shuangpin_scheme();
    }

    /// 双拼辅助码的触发方式；请求每次构建都带上，改动即时生效。
    pub fn set_shuangpin_helpcode_modes(&mut self, modes: crate::types::ShuangpinHelpcodeModes) {
        // 手动模式只改变引擎的有效触发方式，偏好仍保留原值，关闭后可以恢复。
        self.shuangpin_helpcode_modes = if modes.manual_tab {
            crate::types::ShuangpinHelpcodeModes {
                manual_tab: true,
                direct: false,
                trigger_backtick: false,
                trigger_semicolon: false,
                trigger_uppercase: false,
                mid_sentence: false,
                mid_sentence_backtick: false,
                mid_sentence_semicolon: false,
                mid_sentence_uppercase: false,
                direct_helpcode: false,
                direct_helpcode_slash: false,
                direct_helpcode_uppercase: false,
            }
        } else {
            modes
        };
        self.bind_shuangpin_scheme();
    }

    /// 大写字母开哪种辅助码段（`ime_session.h` 的 `helpcode_uppercase_triggers`）。
    pub(super) fn helpcode_uppercase_triggers(
        &self,
    ) -> crate::shuangpin::mid_sentence::UppercaseTriggers {
        crate::shuangpin::mid_sentence::uppercase_triggers(
            &self.shuangpin_helpcode_modes,
            self.shuangpin_helpcode,
        )
    }

    /// 大写段按当时的触发方式解析，方案层要拿到和敲键时一致的设置（`configure_shuangpin_scheme`）。键位几何同样跟着方案走。
    fn bind_shuangpin_scheme(&mut self) {
        let triggers = self.helpcode_uppercase_triggers();
        let geometry = self.key_geometry.clone();
        if let Some(shuangpin) = self.scheme.as_shuangpin_mut() {
            shuangpin.set_helpcode_uppercase_triggers(triggers);
            shuangpin.set_key_geometry(geometry);
        }
    }

    /// 宿主注册（或清除）的键位几何，见 [`crate::touch::KeyGeometry`]。不刷新候选：几何只影响之后的查询。
    pub fn set_key_geometry(&mut self, geometry: Option<Arc<KeyGeometry>>) {
        self.key_geometry = geometry;
        self.bind_shuangpin_scheme();
    }

    pub fn set_touch_tuning(&mut self, tuning: TouchTuning) {
        self.touch_tuning = tuning;
    }

    /// 紧接着的那一次 `handle_key` 的触点；只有双拼方案收，其余方案没有触点可用。
    pub fn set_pending_touch(&mut self, touch: Option<KeyTouch>) {
        if let Some(shuangpin) = self.scheme.as_shuangpin_mut() {
            shuangpin.set_pending_touch(touch);
        }
    }

    pub fn set_fuzzy_pinyin_options(&mut self, options: FuzzyPinyinOptions) {
        self.fuzzy = options;
    }

    pub fn set_sentence_alternatives(&mut self, enabled: bool) {
        self.sentence_alternatives = enabled;
    }

    /// Carried on every request; the engines reset their caches when it changes (overlays.md §1.6.1).
    pub fn set_sentence_association(&mut self, options: SentenceAssociationOptions) {
        self.sentence_association = options;
    }

    /// 首页行数随每个请求交给各方案引擎；只影响词库要不要并入补全词。
    pub fn set_first_page_rows(&mut self, rows: FirstPageRows) {
        self.first_page_rows = rows;
    }

    pub fn set_rescoring_context(&mut self, context: &str) {
        context.clone_into(&mut self.rescoring_context);
    }

    /// Routed by the producing row's scheme in mixed wubi (overlays.md §3.3). The provider layer only reads: the session writes pins, removals and frequency learning into user_dictionary, keyed by the selected row's scheme, and learned phrases through its own canonical-pinyin `QuanpinEngine`; `reset_cache` then makes the providers see them.
    pub fn find_candidate(&self, scheme: SchemeType, key: &str, value: &str) -> Option<WordItem> {
        self.registry.find_candidate(scheme, key, value)
    }

    /// Resets the current scheme's provider, and quanpin's too while mixed wubi is on.
    pub fn reset_cache(&mut self) {
        let scheme = self.current_scheme_type();
        self.registry.reset_cache(scheme);
        // A mixed list holds rows of both dictionaries, and a pin or removal of a quanpin row has to show on the next refresh.
        if scheme == SchemeType::Wubi && self.wubi_options.mixed_pinyin {
            self.registry.reset_cache(SchemeType::Quanpin);
        }
        self.refresh_candidates();
    }

    pub fn expand_initial_candidates(&mut self) -> bool {
        let grew = self
            .registry
            .expand_initial_candidates(&self.state.request, &mut self.state.candidates);
        if grew {
            self.refresh_wubi_codes();
        }
        grew
    }

    /// Insert online rows for the current request and refresh; false when the provider could not take them.
    pub fn apply_dynamic_candidates(&mut self, words: &[String], source: CandidateSource) -> bool {
        if !self
            .registry
            .cache_dynamic_candidates_for_request(&self.state.request, words, source)
        {
            return false;
        }
        self.refresh_candidates();
        true
    }

    /// Remove one provider's rows from all provider caches and rebuild the live candidate list.
    pub fn clear_online_candidates(&mut self, source: CandidateSource) {
        if !source.is_online() {
            return;
        }
        self.registry.clear_online_candidates(source);
        self.refresh_candidates();
    }

    /// ime_session.cpp:299-369.
    fn refresh_candidates(&mut self) {
        let mut request = std::mem::take(&mut self.state.request);
        self.scheme.build_request_into(&mut request);
        self.apply_request_options(&mut request);
        self.apply_autocorrect_suppression(&mut request);
        apply_shuangpin_helpcode_segmentation(&mut request, self.profile);
        reuse_request_preedit(&request, &mut self.state.preedit);
        if !request.valid {
            // An emptied composition is an invalid request, and Backspace never goes through `reset`: the next code must be answered by the wubi table again.
            self.pinyin_tail = false;
            self.state.request = request;
            self.state.candidates.clear();
            self.state.wubi_codes.clear();
            return;
        }

        // The Zhuyin list is the editor's own: its rows exist only while the user has it open.
        let decoded = match self.scheme.as_zhuyin() {
            Some(zhuyin) => {
                let mut candidates = std::mem::take(&mut self.state.candidates);
                reuse_zhuyin_rows(zhuyin.candidates(), &mut candidates);
                Decoded {
                    candidates,
                    wubi_table_answered: false,
                }
            }
            None if request.scheme == SchemeType::Cantonese => {
                let mut candidates = std::mem::take(&mut self.state.candidates);
                if let Some(scheme) = self.scheme.as_cantonese() {
                    self.registry
                        .query_cantonese_scheme_into(scheme, &mut candidates);
                } else {
                    self.registry
                        .query_cantonese_into(&request, &mut candidates);
                }
                Decoded {
                    candidates,
                    wubi_table_answered: false,
                }
            }
            None if request.scheme == SchemeType::JapaneseRomaji => {
                let mut candidates = std::mem::take(&mut self.state.candidates);
                self.registry.query_japanese_into(&request, &mut candidates);
                Decoded {
                    candidates,
                    wubi_table_answered: false,
                }
            }
            None if request.scheme == SchemeType::Korean && request.korean_hanja => {
                let mut candidates = std::mem::take(&mut self.state.candidates);
                self.registry
                    .query_korean_hanja_into(&request, &mut candidates);
                Decoded {
                    candidates,
                    wubi_table_answered: false,
                }
            }
            None if request.scheme == SchemeType::Stroke => {
                let mut candidates = std::mem::take(&mut self.state.candidates);
                self.registry.query_stroke_into(&request, &mut candidates);
                Decoded {
                    candidates,
                    wubi_table_answered: false,
                }
            }
            None => self.decode(&request),
        };
        // 未命中五笔码时允许延长；四码碰撞但能续输拼音时，由上一键的拼音候选保留延长状态。
        // 不能只看当前四码的拼音列表：`hang`、`cang` 这类输入会被切成一个完整音节，
        // 更长的 `han'guo`、`can'guan` 候选要到下一次切分后才会出现。
        let previous_pinyin_candidates = request.scheme == SchemeType::Wubi
            && self.wubi_options.mixed_pinyin
            && self.state.request.raw_input.len() >= 3
            && self.state.candidates.iter().any(|item| {
                item.scheme == SchemeType::Quanpin && !item.canonical_pinyin.is_empty()
            });
        let was_extended = self
            .scheme
            .as_wubi()
            .is_some_and(|wubi| wubi.extended_length_allowed());
        let inherited_pinyin_continuation = request.scheme == SchemeType::Wubi
            && self.wubi_options.mixed_pinyin
            && ((request.raw_input.len() == 4 && previous_pinyin_candidates)
                || (request.raw_input.len() > 4 && was_extended));
        let extended = self.wubi_options.mixed_pinyin
            && !request.wubi_grid
            && (!decoded.wubi_table_answered || inherited_pinyin_continuation);
        if request.wubi_grid {
            grid_display_code(
                &decoded.candidates,
                &request.normalized_input,
                &mut self.state.preedit,
            );
        }
        if let Some(wubi) = self.scheme.as_wubi_mut() {
            wubi.set_extended_length_allowed(extended);
        }
        self.state.request = request;
        self.state.candidates = decoded.candidates;
        self.refresh_wubi_codes();
    }

    /// 宿主只在五笔方案里显示反查编码（含混输拼音的候选），其他方案的每次刷新都不必逐个候选去查五笔表。
    fn refresh_wubi_codes(&mut self) {
        if self.current_scheme_type() != SchemeType::Wubi {
            self.state.wubi_codes.clear();
            return;
        }
        let mut codes = std::mem::take(&mut self.state.wubi_codes);
        self.registry
            .reverse_wubi_codes(&self.state.candidates, &mut codes);
        self.state.wubi_codes = codes;
    }

    /// The scheme's request with the session's switches, autocorrect suppression and the shuangpin double-helpcode segmentation applied.
    fn prepare_request(&self, scheme: &Scheme) -> QueryRequest {
        let mut request = scheme.build_request();
        self.apply_request_options(&mut request);
        self.apply_autocorrect_suppression(&mut request);
        apply_shuangpin_helpcode_segmentation(&mut request, self.profile);
        request
    }

    fn apply_request_options(&self, request: &mut QueryRequest) {
        request.enable_shuangpin_helpcode = self.shuangpin_helpcode;
        request.shuangpin_helpcode_modes = self.shuangpin_helpcode_modes;
        // 末尾段只在「辅助码开、末尾直接关、万象式直接没接管」时筛候选（ime_session.cpp:386-404）；
        // 串里残留的旧段降级成分隔符。
        if !(self.shuangpin_helpcode
            && !request.shuangpin_helpcode_modes.direct
            && !request.shuangpin_helpcode_modes.direct_helpcode)
        {
            request.end_helpcodes.clear();
        }
        request.enable_quanpin_helpcode = self.quanpin_helpcode;
        request.strict_quanpin_helpcode = self.strict_quanpin_helpcode;
        request.sentence_alternatives = self.sentence_alternatives;
        request.enable_quanpin_autocorrect_transposition =
            self.autocorrect_types & autocorrect_type::TRANSPOSITION != 0;
        request.enable_quanpin_autocorrect_neighbor =
            self.autocorrect_types & autocorrect_type::NEIGHBOR != 0;
        request.fuzzy_pinyin = self.fuzzy;
        request.sentence_association = self.sentence_association.clone();
        request.first_page_rows = self.first_page_rows;
        request
            .rescoring_context
            .clone_from(&self.rescoring_context);
        request.touch_tuning = self.touch_tuning;
    }

    /// An input the user committed raw while a correction was offered is answered as typed: both autocorrect switches off and a literal re-cut, because the correction aliases apply whatever the mask says (ime_session.cpp:285-297). Only consulted while some autocorrect type is on.
    fn apply_autocorrect_suppression(&self, request: &mut QueryRequest) {
        if request.scheme == SchemeType::Shuangpin {
            self.apply_shuangpin_autocorrect_suppression(request);
            return;
        }
        if request.scheme != SchemeType::Quanpin || self.autocorrect_types == 0 {
            return;
        }
        self.typo_profile.refresh_if_changed();
        let cases = if request.enable_quanpin_helpcode {
            &request.raw_input_with_cases
        } else {
            &request.raw_input
        };
        let key = autocorrect_suppression_key(&request.raw_input, cases);
        if !self.typo_profile.suppressed(&key) {
            return;
        }
        request.enable_quanpin_autocorrect_transposition = false;
        request.enable_quanpin_autocorrect_neighbor = false;
        QuanpinScheme::apply_literal_segmentation(request);
    }

    /// 双拼：带触点的输入被用户按原样上屏、拒绝过触点纠错（`learn_rejected_correction`）之后，同一串输入按字面回答：两个纠错开关都关掉，触点也不再带，请求和没有触点、没开纠错时一样。只看带触点的请求，桌面和实体键盘的请求从不带触点，不受影响。自定义方案不学，也就没有可查的。
    fn apply_shuangpin_autocorrect_suppression(&self, request: &mut QueryRequest) {
        if self.autocorrect_types == 0
            || request.key_touches.is_empty()
            || self.profile.kind == ShuangpinProfileKind::Custom
        {
            return;
        }
        self.typo_profile.refresh_if_changed();
        if !self
            .typo_profile
            .shuangpin_suppressed(self.profile.kind, &request.raw_input)
        {
            return;
        }
        request.enable_quanpin_autocorrect_transposition = false;
        request.enable_quanpin_autocorrect_neighbor = false;
        request.key_touches.clear();
        request.key_geometry = None;
    }

    /// Query the request's provider and, for wubi with mixed pinyin, append the quanpin rows for the same letters (ime_session.cpp:333-368). The quanpin request gets the session switches and the autocorrect suppression, so a correction the user refused by committing raw stays refused when the same letters arrive through mixed Wubi. A pinyin tail skips the wubi table: those letters are the rest of a spelling, not a code.
    fn decode(&mut self, request: &QueryRequest) -> Decoded {
        if let Some((raw, raw_with_cases)) = self.pinyin_lookup_spelling(request) {
            return Decoded {
                candidates: self.query_pinyin_lookup(raw, raw_with_cases),
                wubi_table_answered: false,
            };
        }
        let pinyin_tail = request.scheme == SchemeType::Wubi
            && self.wubi_options.mixed_pinyin
            && self.pinyin_tail;
        let candidates = if pinyin_tail {
            Vec::new()
        } else {
            self.registry.query(request)
        };
        if request.scheme != SchemeType::Wubi {
            return Decoded {
                candidates,
                wubi_table_answered: false,
            };
        }
        let wubi_table_answered = wubi_table_answered(&candidates, &request.normalized_input);
        // 14 键组码每一位是一组字母，不是拼音，不查混输的拼音回退。
        if !self.wubi_options.mixed_pinyin || request.wubi_grid {
            return Decoded {
                candidates,
                wubi_table_answered,
            };
        }

        self.mixed_pinyin_scheme
            .set_raw_input(&request.raw_input, &request.raw_input_with_cases);
        let mut mixed = std::mem::take(&mut self.mixed_pinyin_request);
        self.mixed_pinyin_scheme.build_request_into(&mut mixed);
        self.apply_request_options(&mut mixed);
        self.apply_autocorrect_suppression(&mut mixed);
        if !mixed.valid {
            self.mixed_pinyin_request = mixed;
            return Decoded {
                candidates,
                wubi_table_answered,
            };
        }
        let pinyin_rows = self.registry.query(&mixed);
        self.mixed_pinyin_request = mixed;
        Decoded {
            // 没有完整五笔码命中时，码表返回的只是更长编码的前缀补全。
            // 这类行不能盖过当前输入的拼音精确候选：`edu` 的 `edui`、`eduq`
            // 会把“额度”挤到后面，空格就会选中罕见的五笔补全字。
            candidates: merge_mixed_candidates(candidates, pinyin_rows, wubi_table_answered),
            wubi_table_answered,
        }
    }

    /// 五笔请求是临时拼音反查时，去掉引导键 `z` 之后的拼音原文（不带大小写的和带大小写的）。混输关闭时五笔码里不会出现 `z`，所以以 `z` 开头的五笔请求只能来自反查；混输打开时 `z` 是拼音字母，没有反查。
    fn pinyin_lookup_spelling<'a>(&self, request: &'a QueryRequest) -> Option<(&'a str, &'a str)> {
        if request.scheme != SchemeType::Wubi || request.wubi_grid || !self.pinyin_lookup_allowed()
        {
            return None;
        }
        let raw = request
            .raw_input
            .strip_prefix(char::from(PINYIN_LOOKUP_PREFIX))?;
        let raw_with_cases = request.raw_input_with_cases.get(1..).unwrap_or(raw);
        Some((raw, raw_with_cases))
    }

    /// 反查只查全拼词库，不查五笔码表：`z` 后面的字母是拼音，不是五笔码。全拼请求带上会话的开关和自动纠错抑制，与混输的拼音回退一致；同一个词只留第一行。
    fn query_pinyin_lookup(&mut self, raw: &str, raw_with_cases: &str) -> Vec<WordItem> {
        self.mixed_pinyin_scheme.set_raw_input(raw, raw_with_cases);
        let mut request = std::mem::take(&mut self.mixed_pinyin_request);
        self.mixed_pinyin_scheme.build_request_into(&mut request);
        self.apply_request_options(&mut request);
        self.apply_autocorrect_suppression(&mut request);
        let mut rows = if request.valid {
            self.registry.query(&request)
        } else {
            Vec::new()
        };
        self.mixed_pinyin_request = request;
        retain_unique_pinyin_rows(&mut rows);
        rows
    }

    /// 当前组字是五笔的临时拼音反查（`z` 引导）。
    pub fn wubi_pinyin_lookup_active(&self) -> bool {
        self.scheme
            .as_wubi()
            .is_some_and(|wubi| wubi.in_pinyin_lookup())
    }

    fn bind_wubi_scheme(&mut self) {
        let mixed_pinyin = self.wubi_options.mixed_pinyin;
        let z_wildcard = self.wubi_options.z_wildcard;
        let lookup_allowed = self.pinyin_lookup_allowed();
        if let Some(wubi) = self.scheme.as_wubi_mut() {
            wubi.set_mixed_pinyin_allowed(mixed_pinyin);
            wubi.set_z_wildcard(z_wildcard);
            // 混输打开时 `z` 已经是拼音字母；通配打开时它是未知码位。两条路都占着这个键，引导反查只在两者都关时才有意义。
            wubi.set_pinyin_lookup_allowed(lookup_allowed);
        }
    }

    /// `z` 引导的临时拼音反查能否打开：混输关闭、通配也关闭。
    ///
    /// 五笔码不用 `z`，所以三条路不重叠地各占一种语义：拼音字母、未知码位、拼音反查。
    fn pinyin_lookup_allowed(&self) -> bool {
        !self.wubi_options.mixed_pinyin && !self.wubi_options.z_wildcard
    }
}

fn same_candidate_identity(left: &WordItem, right: &WordItem) -> bool {
    left.word == right.word
        && left.source == right.source
        && left.pinyin == right.pinyin
        && left.canonical_pinyin == right.canonical_pinyin
}

/// 四码同时落在一个更长拼音的音节边界中时，当前默认切分可能只显示整音节候选。
/// 例如 `hang` 既是五笔整码，也可以按 `han'g...` 继续输入“韩国”。只接受合法的完整首音节和合法的未完成尾音节，避免把任意第五码当成拼音。
/// 整段加上下一字母仍是一个音节的前缀（`chan` 接 `g`）也算续写。
fn pinyin_prefix_can_continue_after_code(raw: &str, letter: u8) -> bool {
    if !letter.is_ascii_lowercase() {
        return false;
    }
    let next = [letter];
    let Ok(next) = std::str::from_utf8(&next) else {
        return false;
    };
    pinyin_prefix_with_next(raw, next.as_bytes()[0])
        || (1..raw.len()).any(|split| {
            let (head, tail) = raw.split_at(split);
            crate::pinyin::segment::is_complete_pinyin_input(head)
                && pinyin_prefix_with_next(tail, next.as_bytes()[0])
        })
}

fn pinyin_prefix_with_next(tail: &str, next: u8) -> bool {
    let mut bytes = [0; 16];
    if tail.len() + 1 > bytes.len() {
        return false;
    }
    bytes[..tail.len()].copy_from_slice(tail.as_bytes());
    bytes[tail.len()] = next;
    std::str::from_utf8(&bytes[..tail.len() + 1]).is_ok_and(crate::pinyin::syllables::is_prefix)
}

/// 从已构造的请求复用方案显示文本，避免刷新时再次调用 `Scheme::preedit()` 分配同一份字符串。
fn reuse_request_preedit(request: &QueryRequest, destination: &mut String) {
    // 带辅助码段时预编辑必须是用户敲的原串（含段标记），去段串只用于查询。
    let source = if !request.raw_input_with_helpcode_marks.is_empty() {
        &request.raw_input_with_helpcode_marks
    } else {
        match request.scheme {
            SchemeType::Quanpin
            | SchemeType::Shuangpin
            | SchemeType::Wubi
            | SchemeType::JapaneseRomaji
            | SchemeType::Cantonese => &request.raw_input_with_cases,
            SchemeType::Korean
            | SchemeType::Zhuyin
            | SchemeType::Vietnamese
            | SchemeType::Tibetan
            | SchemeType::Stroke => &request.normalized_segmentation,
        }
    };
    source.clone_into(destination);
}

/// The open Zhuyin list as session rows, in list order. Each row is keyed by nothing: Zhuyin learns nothing, so no row is ever written back under a reading.
fn reuse_zhuyin_rows(source: &[ListCandidate], destination: &mut Vec<WordItem>) {
    let common = source.len().min(destination.len());
    for (target, candidate) in destination.iter_mut().take(common).zip(source.iter()) {
        target.pinyin.clear();
        target.canonical_pinyin.clear();
        target.word.clear();
        target.word.push_str(&candidate.text);
        target.weight = 0;
        target.source = CandidateSource::Database;
        target.scheme = SchemeType::Zhuyin;
        target.fixed_position = 0;
        target.fuzzy = false;
        target.corrected_from.clear();
        target.sentence_association = false;
        target.sentence_words.clear();
        target.touch_supported = false;
        target.confident_correction = false;
    }
    if destination.len() > source.len() {
        destination.truncate(source.len());
    } else {
        destination.extend(source[common..].iter().map(|candidate| {
            let mut item = WordItem::new("", &candidate.text, 0, CandidateSource::Database, "");
            item.scheme = SchemeType::Zhuyin;
            item
        }));
    }
}

/// A row for the whole code answers it; the prefix rows the wubi query also returns do not, so they must not suppress the pinyin fallback.
fn wubi_table_answered(candidates: &[WordItem], code: &str) -> bool {
    candidates.iter().any(|item| item.pinyin == code)
}

/// 五笔 14 键组字时预编辑显示的编码：首选是码表行、编码落在这几组上时取它的前几位（`qq` 的首选是「你」就显示 `wq`），否则原样显示组码。用户看到的是最可能的那串字母，与全拼 14 键读音行显示首选的拼音一样。
fn grid_display_code(candidates: &[WordItem], representatives: &str, destination: &mut String) {
    let code = candidates
        .first()
        .filter(|item| item.scheme == SchemeType::Wubi)
        .map(|item| item.pinyin.as_str())
        .filter(|code| crate::wubi::grid::code_matches(code, representatives))
        .map_or(representatives, |code| &code[..representatives.len()]);
    destination.clear();
    destination.push_str(code);
}

/// 按当前输入是否命中完整五笔码决定两组候选的先后；每行保留自己的生产方案，选词、固定、删除和学习仍按行的来源路由（overlays.md §3.3）。
#[cfg(test)]
fn merge_pinyin_fallback(candidates: Vec<WordItem>, pinyin_rows: Vec<WordItem>) -> Vec<WordItem> {
    merge_mixed_candidates(candidates, pinyin_rows, true)
}

/// 合并五笔和拼音候选。完整五笔码命中时保留五笔优先；只有更长编码的前缀补全时，
/// 拼音精确候选优先，避免用户输入拼音时被前缀补全抢走首位。
fn merge_mixed_candidates(
    candidates: Vec<WordItem>,
    pinyin_rows: Vec<WordItem>,
    wubi_first: bool,
) -> Vec<WordItem> {
    let (mut candidates, mut pinyin_rows) = if wubi_first {
        (candidates, pinyin_rows)
    } else {
        (pinyin_rows, candidates)
    };
    if pinyin_rows.is_empty() {
        return candidates;
    }
    if candidates.is_empty() {
        retain_unique_pinyin_rows(&mut pinyin_rows);
        return pinyin_rows;
    }
    if candidates.len().saturating_add(pinyin_rows.len()) <= SMALL_PINYIN_FALLBACK {
        candidates.reserve(pinyin_rows.len());
        for item in pinyin_rows {
            if candidates.iter().any(|existing| existing.word == item.word) {
                continue;
            }
            candidates.push(item);
        }
        return candidates;
    }
    // Keep deduplication keys borrowed until the pinyin rows are ready to move into the result.
    let mut seen: HashSet<&str> = candidates.iter().map(|item| item.word.as_str()).collect();
    let duplicates = pinyin_rows
        .iter()
        .enumerate()
        .filter_map(|(index, item)| (!seen.insert(item.word.as_str())).then_some(index))
        .collect::<Vec<_>>();
    drop(seen);
    let unique_count = pinyin_rows.len() - duplicates.len();
    candidates.reserve(unique_count);
    let mut duplicates = duplicates.into_iter().peekable();
    candidates.extend(
        pinyin_rows
            .into_iter()
            .enumerate()
            .filter_map(|(index, item)| {
                if duplicates.peek() == Some(&index) {
                    duplicates.next();
                    None
                } else {
                    Some(item)
                }
            }),
    );
    candidates
}

fn retain_unique_pinyin_rows(rows: &mut Vec<WordItem>) {
    if rows.len() > SMALL_PINYIN_FALLBACK {
        // 借用词面计算重复项，释放集合后再原地保留唯一行。
        let mut seen = HashSet::with_capacity(rows.len());
        let duplicates = rows
            .iter()
            .enumerate()
            .filter_map(|(index, item)| (!seen.insert(item.word.as_str())).then_some(index))
            .collect::<Vec<_>>();
        drop(seen);
        let mut duplicates = duplicates.into_iter().peekable();
        let mut write = 0;
        for read in 0..rows.len() {
            if duplicates.peek() == Some(&read) {
                duplicates.next();
                continue;
            }
            if write != read {
                rows.swap(write, read);
            }
            write += 1;
        }
        rows.truncate(write);
        return;
    }
    let mut write = 0;
    for read in 0..rows.len() {
        if rows[..write]
            .iter()
            .any(|existing| existing.word == rows[read].word)
        {
            continue;
        }
        if write != read {
            rows.swap(write, read);
        }
        write += 1;
    }
    rows.truncate(write);
}

/// With a double helpcode after a complete shuangpin base, the segmentations become the base's plus `'` and the two help letters (ime_session.cpp:15-39). The detector counts in delimiter-free space, so the split is made there too; slicing raw bytes would push a pinyin letter into the base and a manual `'` into the help codes.
fn apply_shuangpin_helpcode_segmentation(request: &mut QueryRequest, profile: &ShuangpinProfile) {
    const HELPCODE_LENGTH: usize = 2;
    if request.scheme != SchemeType::Shuangpin
        || !request.enable_shuangpin_helpcode
        || !request.shuangpin_helpcode_modes.direct
        || detect_active_double_helpcode_length(
            &request.raw_input,
            &request.raw_input_with_cases,
            profile,
        ) != HELPCODE_LENGTH
    {
        return;
    }
    let base_raw = trim_trailing_letters_preserve_delimiters(&request.raw_input, HELPCODE_LENGTH);
    let base_raw_with_cases =
        trim_trailing_letters_preserve_delimiters(&request.raw_input_with_cases, HELPCODE_LENGTH);
    let base_segmentation = segment_input(&base_raw, profile);
    let effective_with_cases = remove_manual_delimiters(&request.raw_input_with_cases);
    let help_codes = &effective_with_cases[effective_with_cases.len() - HELPCODE_LENGTH..];

    apply_segmentation_cases_into(
        &base_segmentation,
        &base_raw_with_cases,
        &mut request.raw_segmentation,
    );
    append_help_codes(&mut request.raw_segmentation, help_codes);
    request
        .normalized_segmentation
        .clone_from(&to_quanpin_segmentation(&base_segmentation, profile));
    append_help_codes(&mut request.normalized_segmentation, help_codes);
    request
        .segmentation
        .clone_from(&request.normalized_segmentation);
}

fn append_help_codes(segmentation: &mut String, help_codes: &str) {
    segmentation.push('\'');
    segmentation.push_str(help_codes);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn wubi(code: &str, word: &str) -> WordItem {
        let mut item = WordItem::new(code, word, 10, CandidateSource::Database, "");
        item.scheme = SchemeType::Wubi;
        item
    }

    fn quanpin(pinyin: &str, word: &str) -> WordItem {
        WordItem::new(pinyin, word, 10, CandidateSource::Database, pinyin)
    }

    /// The session's `answered_by_pinyin_fallback` predicate over the merged rows.
    fn only_pinyin_rows(list: &[WordItem]) -> bool {
        !list.is_empty() && list.iter().all(|item| item.scheme != SchemeType::Wubi)
    }

    fn words(list: &[WordItem]) -> Vec<&str> {
        list.iter().map(|item| item.word.as_str()).collect()
    }

    #[test]
    fn helpcode_segmentation_reuses_the_case_output_buffer() {
        let profile =
            crate::shuangpin::profile::profile(crate::types::ShuangpinProfileKind::Xiaohe).unwrap();
        let mut request = QueryRequest {
            scheme: SchemeType::Shuangpin,
            raw_input: "nihcab".to_owned(),
            raw_input_with_cases: "nihcAB".to_owned(),
            enable_shuangpin_helpcode: true,
            ..QueryRequest::default()
        };
        apply_shuangpin_helpcode_segmentation(&mut request, profile);
        let expected = request.clone();
        let pointer = request.raw_segmentation.as_ptr();
        let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
            apply_shuangpin_helpcode_segmentation(&mut request, profile);
        });
        assert_eq!(request, expected);
        assert_eq!(request.raw_segmentation, "ni'hc'AB");
        assert_eq!(request.raw_segmentation.as_ptr(), pointer);
        assert_eq!(allocations, 7);
    }

    #[test]
    fn request_preedit_reuses_existing_storage() {
        let mut request = QueryRequest {
            scheme: SchemeType::Quanpin,
            raw_input_with_cases: "NiHao".to_owned(),
            normalized_segmentation: "ni'hao".to_owned(),
            ..QueryRequest::default()
        };
        let mut destination = String::with_capacity(request.raw_input_with_cases.len());
        destination.push_str("old");
        let pointer = destination.as_ptr();

        let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
            reuse_request_preedit(&request, &mut destination);
        });

        assert_eq!(allocations, 0);
        assert_eq!(destination, "NiHao");
        assert_eq!(destination.as_ptr(), pointer);

        request.scheme = SchemeType::Korean;
        request.normalized_segmentation = "你好".to_owned();
        reuse_request_preedit(&request, &mut destination);
        assert_eq!(destination, "你好");
    }

    #[test]
    fn only_a_whole_code_row_answers_the_table() {
        // wubi86 prefix rows: `wq` also returns wqb 爷 and wqbb 父子.
        let rows = [wubi("wq", "你"), wubi("wqb", "爷"), wubi("wqbb", "父子")];
        assert!(wubi_table_answered(&rows, "wq"));
        assert!(!wubi_table_answered(&rows[1..], "wq"));
        assert!(!wubi_table_answered(&[], "nihao"));
    }

    #[test]
    fn a_matched_code_keeps_its_wubi_rows_first() {
        // test_wubi_mixed_input_session.cpp:121-128 fixture: wubi wq 你好; ni'hao 你好 / 拟好.
        let list = merge_pinyin_fallback(
            vec![wubi("wq", "你好"), wubi("wqaa", "众人")],
            vec![quanpin("ni'hao", "你好"), quanpin("ni'hao", "拟好")],
        );
        assert_eq!(words(&list), vec!["你好", "众人", "拟好"]);
        assert_eq!(list[0].scheme, SchemeType::Wubi);
        assert_eq!(list[2].scheme, SchemeType::Quanpin);
        assert!(!only_pinyin_rows(&list));
    }

    #[test]
    fn unmatched_wubi_prefix_rows_follow_exact_pinyin_rows() {
        let list = merge_mixed_candidates(
            vec![wubi("edui", "膫"), wubi("eduq", "须将")],
            vec![quanpin("e'du", "额度")],
            false,
        );
        assert_eq!(words(&list), vec!["额度", "膫", "须将"]);
        assert_eq!(list[0].scheme, SchemeType::Quanpin);
        assert_eq!(list[1].scheme, SchemeType::Wubi);
    }

    #[test]
    fn an_unmatched_code_is_answered_by_quanpin_alone() {
        // test_wubi_mixed_input_session.cpp:106-116: `nihao` has no wubi row, so the list equals the quanpin list.
        let pinyin = vec![quanpin("ni'hao", "你好"), quanpin("ni'hao", "拟好")];
        let list = merge_pinyin_fallback(Vec::new(), pinyin.clone());
        assert_eq!(list, pinyin);
        assert!(only_pinyin_rows(&list));
    }

    #[test]
    fn pinyin_fallback_reserves_unique_rows() {
        let pinyin: Vec<_> = (0..23)
            .map(|index| quanpin("ni'hao", &format!("字{index:02}")))
            .collect();
        let pointer = pinyin.as_ptr();
        let list = merge_pinyin_fallback(Vec::new(), pinyin);
        assert_eq!(list.len(), 23);
        assert_eq!(list.capacity(), list.len());
        assert_eq!(list.as_ptr(), pointer);
    }

    #[test]
    fn short_pinyin_fallback_dedup_reuses_the_input_buffer() {
        let pinyin: Vec<_> = (0..16)
            .map(|index| quanpin("ni'hao", &format!("字{}", index % 8)))
            .collect();
        let (list, allocations) = crate::ime::personal_rerank::allocations::count(|| {
            merge_pinyin_fallback(Vec::new(), pinyin)
        });

        assert_eq!(allocations, 0);
        assert_eq!(
            words(&list),
            (0..8).map(|index| format!("字{index}")).collect::<Vec<_>>()
        );
    }

    #[test]
    fn large_duplicate_pinyin_fallback_dedups_without_a_second_hash_scan() {
        let mut pinyin: Vec<_> = (0..64)
            .map(|index| quanpin("ni'hao", &format!("字{index:02}")))
            .collect();
        pinyin.push(quanpin("ni'hao", "字00"));

        let (list, allocations) = crate::ime::personal_rerank::allocations::count(|| {
            merge_pinyin_fallback(Vec::new(), pinyin)
        });

        assert_eq!(words(&list).len(), 64);
        assert!(
            allocations <= 2,
            "large fallback should use one hash pass and the duplicate index: {allocations}"
        );
    }

    #[test]
    fn zhuyin_rows_reuse_existing_word_storage() {
        let source = vec![ListCandidate {
            text: "你好".to_owned(),
            start: 0,
            key: "ㄋㄧˇ ㄏㄠˇ".to_owned(),
        }];
        let mut destination = vec![WordItem::new(
            "old",
            "旧候选",
            42,
            CandidateSource::Generated,
            "old",
        )];
        let word_pointer = destination[0].word.as_ptr();

        let ((), allocations) = crate::ime::personal_rerank::allocations::count(|| {
            reuse_zhuyin_rows(&source, &mut destination);
        });

        assert_eq!(allocations, 0);
        assert_eq!(destination[0].word, "你好");
        assert_eq!(destination[0].word.as_ptr(), word_pointer);
        assert_eq!(destination[0].scheme, SchemeType::Zhuyin);
    }

    #[test]
    fn nothing_at_all_is_not_a_fallback_answer() {
        let list = merge_pinyin_fallback(Vec::new(), Vec::new());
        assert!(list.is_empty());
        assert!(!only_pinyin_rows(&list));
    }

    #[test]
    fn appending_help_codes_extends_the_existing_segmentation() {
        let mut segmentation = String::with_capacity("ni'hao'ab".len());
        segmentation.push_str("ni'hao");
        let pointer = segmentation.as_ptr();
        append_help_codes(&mut segmentation, "ab");
        assert_eq!(segmentation, "ni'hao'ab");
        assert_eq!(segmentation.capacity(), segmentation.len());
        assert_eq!(segmentation.as_ptr(), pointer);
    }

    #[test]
    fn quanpin_duplicates_among_themselves_collapse_to_the_first() {
        let list = merge_pinyin_fallback(
            vec![wubi("nihao", "妳")],
            vec![
                quanpin("ni'hao", "你好"),
                quanpin("ni'hao", "妳"),
                quanpin("ni'ha'o", "你好"),
            ],
        );
        assert_eq!(words(&list), vec!["妳", "你好"]);
        assert_eq!(list[1].pinyin, "ni'hao");
        assert!(!only_pinyin_rows(&list));
    }

    #[test]
    fn pinyin_prefix_split_can_continue() {
        assert!(pinyin_prefix_can_continue_after_code("hang", b'u'));
        assert!(pinyin_prefix_can_continue_after_code("cang", b'u'));
        assert!(pinyin_prefix_can_continue_after_code("chan", b'g'));
        assert!(!pinyin_prefix_can_continue_after_code("wqiy", b'a'));
    }
}
