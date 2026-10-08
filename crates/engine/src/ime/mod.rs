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
use crate::paths::RuntimePaths;
use crate::pinyin::autocorrect::autocorrect_suppression_key;
use crate::quanpin::QuanpinScheme;
use crate::shuangpin::profile::profile;
use crate::shuangpin::query::{
    apply_segmentation_cases, detect_active_double_helpcode_length, remove_manual_delimiters,
    segment_input, to_quanpin_segmentation, trim_trailing_letters_preserve_delimiters,
};
use crate::shuangpin::ShuangpinProfile;
use crate::types::{
    autocorrect_type, CandidateSource, FuzzyPinyinOptions, QueryRequest, SchemeKey, SchemeSet,
    SchemeType, SentenceAssociationOptions, ShuangpinProfileKind, WordItem, WubiInputOptions,
};
use crate::user_dictionary::typo_profile::PersonalTypoProfile;
use crate::vietnamese::{
    InputMethod as VietnameseInputMethod, ToneStyle as VietnameseToneStyle, VietnameseScheme,
};
use crate::zhuyin::scheme::{ListCandidate, ZhuyinKey, ZhuyinScheme};

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
    profile: ShuangpinProfileKind,
    vietnamese_method: VietnameseInputMethod,
    vietnamese_style: VietnameseToneStyle,
    wubi_options: WubiInputOptions,
    /// Mixed wubi only: a pinyin candidate was picked out of this composition, so the rest decodes as quanpin until the composition ends (product decision 2026-09-30, the intent of test_wubi_mixed_input_session.cpp:194-212). Cleared by `reset`, `switch_scheme`, turning mixed input off, and an emptied composition.
    pinyin_tail: bool,
    /// 注音九键模式。注音编辑器被换掉再新建时（切到别的方案再切回来）由这里带过去。
    zhuyin_nine_key: bool,
    autocorrect_types: u32,
    quanpin_helpcode: bool,
    shuangpin_helpcode: bool,
    fuzzy: FuzzyPinyinOptions,
    sentence_alternatives: bool,
    sentence_association: SentenceAssociationOptions,
    rescoring_context: String,
    typo_profile: Arc<PersonalTypoProfile>,
}

impl ImeSession {
    /// ime_session.cpp:42-49. `cantonese_dictionary`, `zhuyin_dictionary` and `stroke_dictionary` are where `msime-cantonese.db`, `msime-zhuyin.db` and `msime-stroke.db` are, each read only when its scheme is activated; starting in Cantonese, Zhuyin or Stroke fails as `switch_scheme` does when its file cannot be opened. `japanese_dictionary` 是 `msime-japanese.dat` 的位置，为空时读资源目录里的那份。`enabled` 是会话允许运行的方案，`scheme` 不在其中时报 `INPUT_SCHEME_NOT_ENABLED`。
    #[allow(clippy::too_many_arguments)]
    pub fn new(
        scheme: SchemeType,
        enabled: SchemeSet,
        profile: ShuangpinProfileKind,
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
                registry.cantonese_inventory(),
                zhuyin,
            )?,
            registry,
            state: CompositionState::default(),
            profile,
            vietnamese_method: VietnameseInputMethod::default(),
            vietnamese_style: VietnameseToneStyle::default(),
            wubi_options: WubiInputOptions::default(),
            pinyin_tail: false,
            zhuyin_nine_key: false,
            autocorrect_types: 0,
            quanpin_helpcode: false,
            shuangpin_helpcode: false,
            fuzzy: FuzzyPinyinOptions::default(),
            sentence_alternatives: false,
            sentence_association: SentenceAssociationOptions::default(),
            rescoring_context: String::new(),
            typo_profile: PersonalTypoProfile::shared(&paths.user(assets::USER_JOURNAL)),
        };
        session.bind_wubi_scheme();
        Ok(session)
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

    pub fn request(&self) -> &QueryRequest {
        &self.state.request
    }

    pub fn preedit(&self) -> &str {
        &self.state.preedit
    }

    pub fn current_scheme_type(&self) -> SchemeType {
        self.scheme.scheme_type()
    }

    /// 会话允许运行的方案。
    pub fn enabled_schemes(&self) -> SchemeSet {
        self.registry.enabled()
    }

    /// `scheme.handle_key` then `refresh_candidates`; `SchemeKey::Requery` only refreshes.
    pub fn handle_key(&mut self, key: SchemeKey) {
        if key != SchemeKey::Requery {
            self.scheme.handle_key(key);
        } else if self.current_scheme_type() == SchemeType::Cantonese {
            // 粤拼词典只读，候选不受在线词或会话设置影响；重查询保留当前快照即可。
            return;
        }
        self.refresh_candidates();
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
        }
        self.bind_wubi_scheme();
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

    /// Takes the letters a selected Cantonese row covers out of the composition and answers what is left; returns whether letters are left composing.
    pub fn select_cantonese(&mut self, item: &WordItem) -> bool {
        let composing = self.scheme.select_cantonese(item);
        self.refresh_candidates();
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

    /// Whether the active scheme is wubi and its code is exactly four letters.
    pub fn wubi_has_complete_code(&self) -> bool {
        self.scheme
            .as_wubi()
            .is_some_and(|wubi| wubi.has_complete_code())
    }

    /// 全拼词典里每个键（以 `'` 连接的完整音节）最好那一行的权重，供滑行输入使用。
    pub fn quanpin_best_weights(&self, keys: &[String]) -> HashMap<String, i64> {
        self.registry.quanpin_best_weights(keys)
    }

    /// Candidates for a raw prefix through a scratch scheme of the current type, leaving the live composition alone (caret-prefix decoding, overlays.md §7.6).
    pub fn query_raw_candidates(&mut self, raw: &str, raw_with_cases: &str) -> Vec<WordItem> {
        let request = self.raw_request(raw, raw_with_cases);
        if !request.valid {
            return Vec::new();
        }
        self.decode(&request).candidates
    }

    /// `expand_initial_candidates` for a list `query_raw_candidates` returned for the same raw prefix.
    pub fn expand_raw_initial_candidates(
        &mut self,
        raw: &str,
        raw_with_cases: &str,
        candidates: &mut Vec<WordItem>,
    ) -> bool {
        let request = self.raw_request(raw, raw_with_cases);
        request.valid
            && self
                .registry
                .expand_initial_candidates(&request, candidates)
    }

    /// The request a scratch scheme of the current type builds for `raw`, with the session's switches applied. The current scheme was activated before it became current, so building its scratch twin cannot fail, except for Zhuyin, whose `msime-zhuyin.db` connection belongs to the live editor: an invalid request stands for both, and Zhuyin keeps its caret at the end, so it never decodes a caret prefix.
    fn raw_request(&self, raw: &str, raw_with_cases: &str) -> QueryRequest {
        let Ok(mut scratch) = Scheme::new(
            self.current_scheme_type(),
            self.profile,
            self.vietnamese_method,
            self.vietnamese_style,
            self.registry.cantonese_inventory(),
            None,
        ) else {
            return QueryRequest::default();
        };
        if let Some(wubi) = scratch.as_wubi_mut() {
            wubi.set_mixed_pinyin_allowed(self.wubi_options.mixed_pinyin);
            wubi.set_extended_length_allowed(self.wubi_options.mixed_pinyin);
        }
        scratch.set_raw_input(raw, raw_with_cases);
        self.prepare_request(&scratch)
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

    pub fn set_shuangpin_helpcode_enabled(&mut self, enabled: bool) {
        self.shuangpin_helpcode = enabled;
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
        let request = self.prepare_request(&self.scheme);
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
                self.registry
                    .query_cantonese_into(&request, &mut candidates);
                Decoded {
                    candidates,
                    wubi_table_answered: false,
                }
            }
            None => self.decode(&request),
        };
        // A fifth letter is only allowed once the table has failed the code typed so far.
        let extended = self.wubi_options.mixed_pinyin && !decoded.wubi_table_answered;
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
        apply_shuangpin_helpcode_segmentation(&mut request, profile(self.profile));
        request
    }

    fn apply_request_options(&self, request: &mut QueryRequest) {
        request.enable_shuangpin_helpcode = self.shuangpin_helpcode;
        request.enable_quanpin_helpcode = self.quanpin_helpcode;
        request.sentence_alternatives = self.sentence_alternatives;
        request.enable_quanpin_autocorrect_transposition =
            self.autocorrect_types & autocorrect_type::TRANSPOSITION != 0;
        request.enable_quanpin_autocorrect_neighbor =
            self.autocorrect_types & autocorrect_type::NEIGHBOR != 0;
        request.fuzzy_pinyin = self.fuzzy;
        request.sentence_association = self.sentence_association;
        request
            .rescoring_context
            .clone_from(&self.rescoring_context);
    }

    /// An input the user committed raw while a correction was offered is answered as typed: both autocorrect switches off and a literal re-cut, because the correction aliases apply whatever the mask says (ime_session.cpp:285-297). Only consulted while some autocorrect type is on.
    fn apply_autocorrect_suppression(&self, request: &mut QueryRequest) {
        if request.scheme != SchemeType::Quanpin || self.autocorrect_types == 0 {
            return;
        }
        self.typo_profile.refresh_if_changed();
        let key = autocorrect_suppression_key(&request.raw_input, &request.raw_input_with_cases);
        if !self.typo_profile.suppressed(&key) {
            return;
        }
        request.enable_quanpin_autocorrect_transposition = false;
        request.enable_quanpin_autocorrect_neighbor = false;
        QuanpinScheme::apply_literal_segmentation(request);
    }

    /// Query the request's provider and, for wubi with mixed pinyin, append the quanpin rows for the same letters (ime_session.cpp:333-368). The quanpin request gets the session switches and the autocorrect suppression, so a correction the user refused by committing raw stays refused when the same letters arrive through mixed Wubi. A pinyin tail skips the wubi table: those letters are the rest of a spelling, not a code.
    fn decode(&mut self, request: &QueryRequest) -> Decoded {
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
        if !self.wubi_options.mixed_pinyin {
            return Decoded {
                candidates,
                wubi_table_answered,
            };
        }

        let mut pinyin = QuanpinScheme::new();
        pinyin.set_raw_input(&request.raw_input, &request.raw_input_with_cases);
        let mut mixed = pinyin.build_request();
        self.apply_request_options(&mut mixed);
        self.apply_autocorrect_suppression(&mut mixed);
        if !mixed.valid {
            return Decoded {
                candidates,
                wubi_table_answered,
            };
        }
        let pinyin_rows = self.registry.query(&mixed);
        Decoded {
            candidates: merge_pinyin_fallback(candidates, pinyin_rows),
            wubi_table_answered,
        }
    }

    fn bind_wubi_scheme(&mut self) {
        let mixed_pinyin = self.wubi_options.mixed_pinyin;
        if let Some(wubi) = self.scheme.as_wubi_mut() {
            wubi.set_mixed_pinyin_allowed(mixed_pinyin);
        }
    }
}

/// 从已构造的请求复用方案显示文本，避免刷新时再次调用 `Scheme::preedit()` 分配同一份字符串。
fn reuse_request_preedit(request: &QueryRequest, destination: &mut String) {
    let source = match request.scheme {
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
    };
    source.clone_into(destination);
}

/// The open Zhuyin list as session rows, in list order. Each row is keyed by nothing: Zhuyin learns nothing, so no row is ever written back under a reading.
fn reuse_zhuyin_rows(source: &[ListCandidate], destination: &mut Vec<WordItem>) {
    let common = source.len().min(destination.len());
    for (target, candidate) in destination.iter_mut().take(common).zip(source.iter()) {
        target.pinyin.clear();
        target.canonical_pinyin.clear();
        target.word.clone_from(&candidate.text);
        target.weight = 0;
        target.source = CandidateSource::Database;
        target.scheme = SchemeType::Zhuyin;
        target.fixed_position = 0;
        target.fuzzy = false;
        target.corrected_from.clear();
        target.sentence_association = false;
        target.sentence_words.clear();
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

/// Wubi rows first, so wubi ranking and fixed positions keep precedence, then the quanpin rows whose word is not shown yet, in quanpin order. Every row keeps its producer's scheme: the session reads "answered by the pinyin fallback" and routes pins, removals and learning from those tags (overlays.md §3.3), so no list-level flag is kept here.
fn merge_pinyin_fallback(
    mut candidates: Vec<WordItem>,
    mut pinyin_rows: Vec<WordItem>,
) -> Vec<WordItem> {
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

    request.raw_segmentation = append_help_codes(
        apply_segmentation_cases(&base_segmentation, &base_raw_with_cases),
        help_codes,
    );
    request.normalized_segmentation = append_help_codes(
        to_quanpin_segmentation(&base_segmentation, profile),
        help_codes,
    );
    request
        .segmentation
        .clone_from(&request.normalized_segmentation);
}

fn append_help_codes(mut segmentation: String, help_codes: &str) -> String {
    segmentation.push('\'');
    segmentation.push_str(help_codes);
    segmentation
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
        let result = append_help_codes(segmentation, "ab");
        assert_eq!(result, "ni'hao'ab");
        assert_eq!(result.capacity(), result.len());
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
}
