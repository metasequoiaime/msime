//! The providers, one per scheme family (`R/providers/provider_registry.cpp`, `pinyin_candidate_provider.cpp`): pinyin (quanpin and shuangpin engines), wubi and Japanese. Korean syllables are the text, so a Korean query answers nothing until the user opens the Hanja list, and then the embedded Hanja table (`korean::hanja`) answers it. Cantonese is answered by `msime-cantonese.db`, which is opened the first time the scheme is activated and then kept for the session. Zhuyin's editor reads `msime-zhuyin.db` itself while it converts, so the registry opens that file the first time the scheme is activated, lends the connection to each Zhuyin scheme built and takes it back when that scheme is replaced; its list rows reach the session through the scheme, never through `query`. Stroke is answered by `msime-stroke.db` the way Cantonese is: opened the first time the scheme is activated, kept for the session, and read by `query`.
//!
//! The registry answers queries and lookups only. The reference also routed `create_word` / `update_weight_by_pinyin_and_word` / `delete_by_pinyin_and_word` through it; here the session writes pins, removals and frequency learning into user_dictionary itself, choosing the dictionary kind from the selected row's scheme (overlays.md §3.3), and phrases through its own canonical-pinyin `QuanpinEngine`, so a second writer path would only diverge from it.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Arc;

use crate::assets;
use crate::cantonese::{CantoneseDictionary, CantoneseScheme, Inventory};
use crate::diagnostics;
use crate::error::{EngineError, Result};
use crate::helpcode::SharedKeymap;
use crate::japanese::JapaneseProvider;
use crate::korean::hanja;
use crate::language_dictionary::{self, LanguageDictionary};
use crate::paths::RuntimePaths;
use crate::quanpin::QuanpinEngine;
use crate::shuangpin::profile::profile;
use crate::shuangpin::ShuangpinEngine;
use crate::stroke::StrokeScheme;
use crate::types::{
    CandidateSource, QueryRequest, SchemeSet, SchemeType, ShuangpinProfileKind, WordItem,
    WubiProfileKind,
};
use crate::wubi::provider::WubiProvider;

pub struct ProviderRegistry {
    /// 会话允许运行的方案；不在其中的方案没有 provider，`activate` 拒绝它们。
    enabled: SchemeSet,
    /// 全拼或五笔在 `enabled` 里时才有：五笔混拼的拼音行由它查出。
    quanpin: Option<QuanpinEngine>,
    /// 双拼在 `enabled` 里时才有。它会再打开一次 `msime-pinyin.db` 并预热 n-gram 表，这正是收窄方案要省下的。
    shuangpin: Option<ShuangpinEngine>,
    wubi: Option<WubiProvider>,
    japanese: Option<JapaneseProvider>,
    keymap: Option<SharedKeymap>,
    /// Where `msime-cantonese.db` is; empty when the host has none.
    cantonese_path: PathBuf,
    cantonese: Option<CantoneseDictionary>,
    /// Where `msime-zhuyin.db` is; empty when the host has none.
    zhuyin_path: PathBuf,
    /// `msime-zhuyin.db` opened by `activate`; `None` before that and while the live Zhuyin scheme holds it.
    zhuyin: Option<LanguageDictionary>,
    /// Where `msime-stroke.db` is; empty when the host has none.
    stroke_path: PathBuf,
    /// `msime-stroke.db` opened by `activate`; `None` before that.
    stroke: Option<LanguageDictionary>,
}

/// 五笔码表所在的数据库。准备代次时单独发布的 `msime-wubi.db` 已并回工作主词库，学习、删词与个人词典也写那里，所以优先读代次的 `msime-pinyin.db`，读写落在同一个文件上。代次目录就是资源目录（只读布局）时读其中的 `msime-wubi.db`；没有代次工作副本时退回资源目录，先找拆分后的 `msime-wubi.db`，再找旧的合并发布。
pub(crate) fn wubi_database(paths: &RuntimePaths) -> PathBuf {
    [
        paths.dictionary(assets::WUBI_DICTIONARY),
        paths.dictionary(assets::MAIN_DICTIONARY),
        paths.resource(assets::WUBI_DICTIONARY),
    ]
    .into_iter()
    .find(|path| path.is_file())
    .unwrap_or_else(|| paths.resource(assets::MAIN_DICTIONARY))
}

impl ProviderRegistry {
    /// 五笔读 `wubi_database` 选出的文件，通常是代次里的 `msime-pinyin.db`； the Japanese model is the immutable resource (provider_registry.cpp:4-10). `japanese_path` 非空时改读这个位置（例如按需下载的那份），为空时读资源目录里的 `msime-japanese.dat`。`enabled` 为 [`SchemeSet::ALL`] 时四个 provider 都构造，与收窄之前相同；否则只构造 `enabled` 用得到的那些，查询不在其中的方案一律答空。
    pub fn new(
        enabled: SchemeSet,
        profile_kind: ShuangpinProfileKind,
        paths: &RuntimePaths,
        cantonese_path: PathBuf,
        zhuyin_path: PathBuf,
        stroke_path: PathBuf,
        japanese_path: PathBuf,
    ) -> Self {
        let japanese_model = if japanese_path.as_os_str().is_empty() {
            paths.resource(assets::JAPANESE_MODEL)
        } else {
            japanese_path
        };
        let quanpin_needed =
            enabled.contains(SchemeType::Quanpin) || enabled.contains(SchemeType::Wubi);
        Self {
            enabled,
            quanpin: quanpin_needed.then(|| QuanpinEngine::new(paths)),
            shuangpin: enabled
                .contains(SchemeType::Shuangpin)
                .then(|| ShuangpinEngine::new(profile(profile_kind), paths)),
            wubi: enabled
                .contains(SchemeType::Wubi)
                .then(|| WubiProvider::new(&wubi_database(paths))),
            japanese: enabled
                .contains(SchemeType::JapaneseRomaji)
                .then(|| JapaneseProvider::new(&japanese_model)),
            keymap: None,
            cantonese_path,
            cantonese: None,
            zhuyin_path,
            zhuyin: None,
            stroke_path,
            stroke: None,
        }
    }

    /// Opens what `scheme` reads before it becomes active, once per session: `msime-cantonese.db` for Cantonese, `msime-zhuyin.db` for Zhuyin and `msime-stroke.db` for Stroke, failing as `language_dictionary::open_read_only` does when the file is missing or of an unknown version. Nothing for the other schemes. The caller does not activate Zhuyin while a Zhuyin scheme holds the connection, which would open the file again. 不在 `enabled` 里的方案报 `INPUT_SCHEME_NOT_ENABLED`，什么也不打开。
    pub fn activate(&mut self, scheme: SchemeType) -> Result<()> {
        if !self.enabled.contains(scheme) {
            return Err(EngineError::invalid(diagnostics::INPUT_SCHEME_NOT_ENABLED));
        }
        if scheme == SchemeType::Cantonese && self.cantonese.is_none() {
            self.cantonese = Some(CantoneseDictionary::open(&self.cantonese_path)?);
        }
        if scheme == SchemeType::Zhuyin && self.zhuyin.is_none() {
            self.zhuyin = Some(language_dictionary::open_read_only(&self.zhuyin_path)?);
        }
        if scheme == SchemeType::Stroke && self.stroke.is_none() {
            self.stroke = Some(language_dictionary::open_read_only(&self.stroke_path)?);
        }
        Ok(())
    }

    /// Lends the `msime-zhuyin.db` connection `activate` opened to the Zhuyin scheme about to be built; `None` for any other scheme, and for Zhuyin before it has been activated.
    pub fn take_dictionary(&mut self, scheme: SchemeType) -> Option<LanguageDictionary> {
        if scheme == SchemeType::Zhuyin {
            self.zhuyin.take()
        } else {
            None
        }
    }

    /// Takes back the `msime-zhuyin.db` connection of a Zhuyin scheme being replaced, so switching back to Zhuyin reuses it instead of opening the file again.
    pub fn return_dictionary(&mut self, dictionary: LanguageDictionary) {
        self.zhuyin = Some(dictionary);
    }

    /// The syllable inventory of the open `msime-cantonese.db`; `None` until Cantonese has been activated.
    pub fn cantonese_inventory(&self) -> Option<Arc<Inventory>> {
        self.cantonese.as_ref().map(CantoneseDictionary::inventory)
    }

    /// 会话允许运行的方案。
    pub fn enabled(&self) -> SchemeSet {
        self.enabled
    }

    /// 切换五笔码表版本；provider 下一次查询起读对应的表。没有五笔 provider 时什么也不做。
    pub fn set_wubi_profile(&mut self, profile: WubiProfileKind) {
        if let Some(wubi) = &mut self.wubi {
            wubi.set_profile(profile);
        }
    }

    /// Cached pinyin answers carry the old table's annotations and the online rows stored beside them, so both pinyin engines drop their caches, as the reference's setters did (quanpin/engine.h:37-41, shuangpin/shuangpin_dictionary.h:250-254). The reference left the shuangpin fuzzy cache alone; clearing it too only costs one requery.
    pub fn set_helpcode_keymap(&mut self, keymap: Option<SharedKeymap>) {
        self.keymap = keymap;
        self.reset_pinyin_caches();
    }

    fn reset_pinyin_caches(&mut self) {
        if let Some(quanpin) = &mut self.quanpin {
            quanpin.reset_cache();
        }
        if let Some(shuangpin) = &mut self.shuangpin {
            shuangpin.reset_cache();
        }
    }

    /// Pinyin rows are stamped with the request's scheme (pinyin_candidate_provider.cpp:12-28); wubi rows carry `Wubi` from their provider, and Japanese rows keep the default scheme, as the reference recorded them. 方案没有 provider（不在 `enabled` 里）时答空。
    pub fn query(&mut self, request: &QueryRequest) -> Vec<WordItem> {
        if !request.valid {
            return Vec::new();
        }
        let keymap = self.keymap.as_deref();
        let mut candidates = match request.scheme {
            SchemeType::Quanpin => match &mut self.quanpin {
                Some(quanpin) => quanpin.query(request, keymap),
                None => return Vec::new(),
            },
            SchemeType::Shuangpin => match &mut self.shuangpin {
                Some(shuangpin) => shuangpin.query(request, keymap),
                None => return Vec::new(),
            },
            SchemeType::Wubi => {
                return self
                    .wubi
                    .as_mut()
                    .map_or_else(Vec::new, |wubi| wubi.query(request))
            }
            SchemeType::JapaneseRomaji => {
                return self
                    .japanese
                    .as_mut()
                    .map_or_else(Vec::new, |japanese| japanese.query(request))
            }
            SchemeType::Korean if request.korean_hanja => return hanja::candidates(request),
            SchemeType::Cantonese => return self.cantonese_candidates(request),
            SchemeType::Stroke => return self.stroke_candidates(request),
            // 越南文和藏文在组字里直接拼出文字，没有候选；注音的列表来自它的编辑器。
            SchemeType::Korean
            | SchemeType::Zhuyin
            | SchemeType::Vietnamese
            | SchemeType::Tibetan => return Vec::new(),
        };
        for item in &mut candidates {
            item.scheme = request.scheme;
        }
        candidates
    }

    /// 五笔、日文、韩文、粤拼、注音、越南文、藏文和笔画从不回答查找（wubi_candidate_provider.h:19-22；日文那个读的是已删除的 `japanese_lexicon`）。
    pub fn find_candidate(&self, scheme: SchemeType, key: &str, value: &str) -> Option<WordItem> {
        match scheme {
            SchemeType::Quanpin => self.quanpin.as_ref()?.find_candidate(key, value),
            SchemeType::Shuangpin => self.shuangpin.as_ref()?.find_candidate(key, value),
            SchemeType::Wubi
            | SchemeType::JapaneseRomaji
            | SchemeType::Korean
            | SchemeType::Cantonese
            | SchemeType::Zhuyin
            | SchemeType::Vietnamese
            | SchemeType::Tibetan
            | SchemeType::Stroke => None,
        }
    }

    /// 全拼词典里每个键最好那一行的权重，供滑行输入使用；会话没有全拼 provider 时为空。
    pub fn quanpin_best_weights(&self, keys: &[String]) -> HashMap<String, i64> {
        self.quanpin
            .as_ref()
            .map_or_else(HashMap::new, |quanpin| quanpin.best_weights(keys))
    }

    /// 为候选展示查询完整五笔编码；反查结果与候选一一对应，查不到时保留空字符串。没有构造五笔 provider 的会话（方案集合里没有五笔）一律是空字符串。
    pub fn reverse_wubi_codes(&mut self, candidates: &[WordItem], destination: &mut Vec<String>) {
        if destination.len() > candidates.len() {
            destination.truncate(candidates.len());
        }
        destination.resize_with(candidates.len(), String::new);
        for (code, candidate) in destination.iter_mut().zip(candidates) {
            if self
                .wubi
                .as_mut()
                .is_none_or(|wubi| !wubi.reverse_code_into(&candidate.word, code))
            {
                code.clear();
            }
        }
    }

    /// Either pinyin scheme resets both pinyin engines (pinyin_candidate_provider.cpp:44-48).
    pub fn reset_cache(&mut self, scheme: SchemeType) {
        match scheme {
            SchemeType::Quanpin | SchemeType::Shuangpin => self.reset_pinyin_caches(),
            SchemeType::Wubi => {
                if let Some(wubi) = &mut self.wubi {
                    wubi.reset_cache();
                }
            }
            SchemeType::JapaneseRomaji => {
                if let Some(japanese) = &mut self.japanese {
                    japanese.reset_cache();
                }
            }
            // `msime-cantonese.db` and `msime-stroke.db` are read-only and their rows are never rewritten, so there is no cache to drop.
            SchemeType::Korean
            | SchemeType::Cantonese
            | SchemeType::Zhuyin
            | SchemeType::Vietnamese
            | SchemeType::Tibetan
            | SchemeType::Stroke => {}
        }
    }

    /// Online rows for a request: quanpin and shuangpin series caches, the Japanese dynamic row. Wubi and Japanese take a batch only through the single-word default of `ICandidateProvider` (candidate_provider.h:27-31); wubi then accepts and drops it (wubi_candidate_provider.cpp:102-106).
    pub fn cache_dynamic_candidates_for_request(
        &mut self,
        request: &QueryRequest,
        words: &[String],
        source: CandidateSource,
    ) -> bool {
        match request.scheme {
            SchemeType::Quanpin => self
                .quanpin
                .as_mut()
                .is_some_and(|quanpin| quanpin.insert_online_words(request, words, source)),
            SchemeType::Shuangpin => self
                .shuangpin
                .as_mut()
                .is_some_and(|shuangpin| shuangpin.insert_online_words(request, words, source)),
            SchemeType::Wubi => self.wubi.is_some() && words.len() == 1,
            SchemeType::JapaneseRomaji => match (&mut self.japanese, words) {
                (Some(japanese), [word]) => {
                    japanese.cache_dynamic_candidate(&request.raw_input, word, source)
                }
                _ => false,
            },
            // 韩文、粤拼、注音、越南文、藏文和笔画不接收在线候选。
            SchemeType::Korean
            | SchemeType::Cantonese
            | SchemeType::Zhuyin
            | SchemeType::Vietnamese
            | SchemeType::Tibetan
            | SchemeType::Stroke => false,
        }
    }

    /// Remove one online provider's rows from every provider cache.
    pub fn clear_online_candidates(&mut self, source: CandidateSource) {
        if let Some(quanpin) = &mut self.quanpin {
            quanpin.clear_online_candidates(source);
        }
        if let Some(shuangpin) = &mut self.shuangpin {
            shuangpin.clear_online_candidates(source);
        }
        if let Some(japanese) = &mut self.japanese {
            japanese.clear_online_candidates(source);
        }
    }

    /// Always the pinyin provider, which answers only quanpin and shuangpin requests (pinyin_candidate_provider.cpp:31-42).
    pub fn expand_initial_candidates(
        &mut self,
        request: &QueryRequest,
        candidates: &mut Vec<WordItem>,
    ) -> bool {
        match request.scheme {
            SchemeType::Quanpin => self
                .quanpin
                .as_mut()
                .is_some_and(|quanpin| quanpin.expand_initial_candidates(request, candidates)),
            SchemeType::Shuangpin => self
                .shuangpin
                .as_mut()
                .is_some_and(|shuangpin| shuangpin.expand_initial_candidates(request, candidates)),
            SchemeType::Wubi
            | SchemeType::JapaneseRomaji
            | SchemeType::Korean
            | SchemeType::Cantonese
            | SchemeType::Zhuyin
            | SchemeType::Vietnamese
            | SchemeType::Tibetan
            | SchemeType::Stroke => false,
        }
    }

    /// The `msime-cantonese.db` rows for the request's letters, read again through the activated inventory, as `CantoneseScheme::candidates` lists them. Each row is keyed by the typed letters it covers (`pinyin`, apostrophes kept) and the dictionary key it was found under (`canonical_pinyin`), which is what selecting it takes out of the composition. A read that fails answers nothing, like the wubi table.
    fn cantonese_candidates(&self, request: &QueryRequest) -> Vec<WordItem> {
        let Some(dictionary) = &self.cantonese else {
            return Vec::new();
        };
        let mut scheme = CantoneseScheme::new(dictionary.inventory());
        scheme.set_raw_input(&request.raw_input);
        let Ok(candidates) = scheme.candidates(dictionary.dictionary()) else {
            return Vec::new();
        };
        let input = scheme.input();
        candidates
            .into_iter()
            .map(|candidate| {
                let mut item = WordItem::new(
                    &input[..candidate.end],
                    candidate.text,
                    candidate.weight,
                    CandidateSource::Database,
                    candidate.key,
                );
                item.scheme = SchemeType::Cantonese;
                item
            })
            .collect()
    }

    /// `msime-stroke.db` 对请求笔画的单字候选，顺序同 `StrokeScheme::candidates`。每行以键入的笔画为 `pinyin`、以该字的完整笔画码为 `canonical_pinyin`；笔画不学习，这两个键只用于显示，从不写回任何词典。读失败时不给候选，与粤拼一样。
    fn stroke_candidates(&self, request: &QueryRequest) -> Vec<WordItem> {
        let Some(dictionary) = &self.stroke else {
            return Vec::new();
        };
        let mut scheme = StrokeScheme::new();
        scheme.set_raw_input(&request.raw_input);
        let Ok(candidates) = scheme.candidates(dictionary) else {
            return Vec::new();
        };
        let input = scheme.input();
        candidates
            .into_iter()
            .map(|candidate| {
                let mut item = WordItem::new(
                    input,
                    candidate.text,
                    candidate.weight,
                    CandidateSource::Database,
                    candidate.key,
                );
                item.scheme = SchemeType::Stroke;
                item
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn registry(enabled: SchemeSet) -> ProviderRegistry {
        ProviderRegistry::new(
            enabled,
            ShuangpinProfileKind::Xiaohe,
            &RuntimePaths::default(),
            PathBuf::new(),
            PathBuf::new(),
            PathBuf::new(),
            PathBuf::new(),
        )
    }

    /// 全部方案时四个 provider 都在；只有五笔时只有五笔和混拼要用的全拼，双拼和日文都不构造；只有全拼时没有五笔。
    #[test]
    fn providers_follow_the_enabled_schemes() {
        let all = registry(SchemeSet::ALL);
        assert!(all.quanpin.is_some() && all.shuangpin.is_some());
        assert!(all.wubi.is_some() && all.japanese.is_some());

        let wubi = registry(SchemeSet::of(&[SchemeType::Wubi]));
        assert!(wubi.quanpin.is_some() && wubi.wubi.is_some());
        assert!(wubi.shuangpin.is_none() && wubi.japanese.is_none());

        let quanpin = registry(SchemeSet::of(&[SchemeType::Quanpin]));
        assert!(quanpin.quanpin.is_some());
        assert!(quanpin.wubi.is_none() && quanpin.shuangpin.is_none());

        let korean = registry(SchemeSet::of(&[SchemeType::Korean]));
        assert!(korean.quanpin.is_none() && korean.wubi.is_none());
    }

    /// 没有 provider 的方案：查询答空，查找答 `None`，在线行不收，激活报 `INPUT_SCHEME_NOT_ENABLED`。
    #[test]
    fn schemes_without_a_provider_answer_nothing() {
        let mut registry = registry(SchemeSet::of(&[SchemeType::Wubi]));
        for scheme in [SchemeType::Shuangpin, SchemeType::JapaneseRomaji] {
            let request = QueryRequest {
                scheme,
                raw_input: "ka".to_owned(),
                raw_input_with_cases: "ka".to_owned(),
                valid: true,
                ..QueryRequest::default()
            };
            assert!(registry.query(&request).is_empty(), "{scheme:?}");
            assert!(registry.find_candidate(scheme, "ka", "か").is_none());
            assert!(!registry.cache_dynamic_candidates_for_request(
                &request,
                &["か".to_owned()],
                CandidateSource::Database,
            ));
            let mut candidates = Vec::new();
            assert!(!registry.expand_initial_candidates(&request, &mut candidates));
            registry.reset_cache(scheme);
            assert_eq!(
                registry.activate(scheme).unwrap_err().to_string(),
                diagnostics::INPUT_SCHEME_NOT_ENABLED
            );
        }
        registry.activate(SchemeType::Wubi).unwrap();
        registry.set_helpcode_keymap(None);
    }
}
