//! The providers, one per scheme family (`R/providers/provider_registry.cpp`, `pinyin_candidate_provider.cpp`): pinyin (quanpin and shuangpin engines), wubi and Japanese. Korean syllables are the text, so a Korean query answers nothing until the user opens the Hanja list, and then the embedded Hanja table (`korean::hanja`) answers it. Cantonese is answered by `cantonese.db`, which is opened the first time the scheme is activated and then kept for the session. Zhuyin's editor reads `zhuyin.db` itself while it converts, so the registry opens that file the first time the scheme is activated, lends the connection to each Zhuyin scheme built and takes it back when that scheme is replaced; its list rows reach the session through the scheme, never through `query`. Stroke is answered by `stroke.db` the way Cantonese is: opened the first time the scheme is activated, kept for the session, and read by `query`.
//!
//! The registry answers queries and lookups only. The reference also routed `create_word` / `update_weight_by_pinyin_and_word` / `delete_by_pinyin_and_word` through it; here the session writes pins, removals and frequency learning into user_dictionary itself, choosing the dictionary kind from the selected row's scheme (overlays.md §3.3), and phrases through its own canonical-pinyin `QuanpinEngine`, so a second writer path would only diverge from it.

use std::path::PathBuf;
use std::sync::Arc;

use crate::assets;
use crate::cantonese::{CantoneseDictionary, CantoneseScheme, Inventory};
use crate::error::Result;
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
    CandidateSource, QueryRequest, SchemeType, ShuangpinProfileKind, WordItem, WubiProfileKind,
};
use crate::wubi::provider::WubiProvider;

pub struct ProviderRegistry {
    quanpin: QuanpinEngine,
    shuangpin: ShuangpinEngine,
    wubi: WubiProvider,
    japanese: JapaneseProvider,
    keymap: Option<SharedKeymap>,
    /// Where `cantonese.db` is; empty when the host has none.
    cantonese_path: PathBuf,
    cantonese: Option<CantoneseDictionary>,
    /// Where `zhuyin.db` is; empty when the host has none.
    zhuyin_path: PathBuf,
    /// `zhuyin.db` opened by `activate`; `None` before that and while the live Zhuyin scheme holds it.
    zhuyin: Option<LanguageDictionary>,
    /// Where `stroke.db` is; empty when the host has none.
    stroke_path: PathBuf,
    /// `stroke.db` opened by `activate`; `None` before that.
    stroke: Option<LanguageDictionary>,
}

impl ProviderRegistry {
    /// Wubi reads the generation's `msime.db`; the Japanese model is the immutable resource (provider_registry.cpp:4-10). `japanese_path` 非空时改读这个位置（例如按需下载的那份），为空时读资源目录里的 `dict_japanese.dat`。
    pub fn new(
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
        Self {
            quanpin: QuanpinEngine::new(paths),
            shuangpin: ShuangpinEngine::new(profile(profile_kind), paths),
            wubi: WubiProvider::new(&paths.dictionary(assets::MAIN_DICTIONARY)),
            japanese: JapaneseProvider::new(&japanese_model),
            keymap: None,
            cantonese_path,
            cantonese: None,
            zhuyin_path,
            zhuyin: None,
            stroke_path,
            stroke: None,
        }
    }

    /// Opens what `scheme` reads before it becomes active, once per session: `cantonese.db` for Cantonese, `zhuyin.db` for Zhuyin and `stroke.db` for Stroke, failing as `language_dictionary::open_read_only` does when the file is missing or of an unknown version. Nothing for the other schemes. The caller does not activate Zhuyin while a Zhuyin scheme holds the connection, which would open the file again.
    pub fn activate(&mut self, scheme: SchemeType) -> Result<()> {
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

    /// Lends the `zhuyin.db` connection `activate` opened to the Zhuyin scheme about to be built; `None` for any other scheme, and for Zhuyin before it has been activated.
    pub fn take_dictionary(&mut self, scheme: SchemeType) -> Option<LanguageDictionary> {
        if scheme == SchemeType::Zhuyin {
            self.zhuyin.take()
        } else {
            None
        }
    }

    /// Takes back the `zhuyin.db` connection of a Zhuyin scheme being replaced, so switching back to Zhuyin reuses it instead of opening the file again.
    pub fn return_dictionary(&mut self, dictionary: LanguageDictionary) {
        self.zhuyin = Some(dictionary);
    }

    /// The syllable inventory of the open `cantonese.db`; `None` until Cantonese has been activated.
    pub fn cantonese_inventory(&self) -> Option<Arc<Inventory>> {
        self.cantonese.as_ref().map(CantoneseDictionary::inventory)
    }

    /// 切换五笔码表版本；provider 下一次查询起读对应的表。
    pub fn set_wubi_profile(&mut self, profile: WubiProfileKind) {
        self.wubi.set_profile(profile);
    }

    /// Cached pinyin answers carry the old table's annotations and the online rows stored beside them, so both pinyin engines drop their caches, as the reference's setters did (quanpin/engine.h:37-41, shuangpin/shuangpin_dictionary.h:250-254). The reference left the shuangpin fuzzy cache alone; clearing it too only costs one requery.
    pub fn set_helpcode_keymap(&mut self, keymap: Option<SharedKeymap>) {
        self.keymap = keymap;
        self.quanpin.reset_cache();
        self.shuangpin.reset_cache();
    }

    /// Pinyin rows are stamped with the request's scheme (pinyin_candidate_provider.cpp:12-28); wubi rows carry `Wubi` from their provider, and Japanese rows keep the default scheme, as the reference recorded them.
    pub fn query(&mut self, request: &QueryRequest) -> Vec<WordItem> {
        if !request.valid {
            return Vec::new();
        }
        let keymap = self.keymap.as_deref();
        let mut candidates = match request.scheme {
            SchemeType::Quanpin => self.quanpin.query(request, keymap),
            SchemeType::Shuangpin => self.shuangpin.query(request, keymap),
            SchemeType::Wubi => return self.wubi.query(request),
            SchemeType::JapaneseRomaji => return self.japanese.query(request),
            SchemeType::Korean if request.korean_hanja => return hanja::candidates(request),
            SchemeType::Cantonese => return self.cantonese_candidates(request),
            SchemeType::Stroke => return self.stroke_candidates(request),
            // Vietnamese composes its text in the preedit and has no candidates; the Zhuyin list comes from its editor.
            SchemeType::Korean | SchemeType::Zhuyin | SchemeType::Vietnamese => return Vec::new(),
        };
        for item in &mut candidates {
            item.scheme = request.scheme;
        }
        candidates
    }

    /// Wubi, Japanese, Korean, Cantonese, Zhuyin, Vietnamese and Stroke never answer a lookup (wubi_candidate_provider.h:19-22; the Japanese one read the dropped `japanese_lexicon`).
    pub fn find_candidate(&self, scheme: SchemeType, key: &str, value: &str) -> Option<WordItem> {
        match scheme {
            SchemeType::Quanpin => self.quanpin.find_candidate(key, value),
            SchemeType::Shuangpin => self.shuangpin.find_candidate(key, value),
            SchemeType::Wubi
            | SchemeType::JapaneseRomaji
            | SchemeType::Korean
            | SchemeType::Cantonese
            | SchemeType::Zhuyin
            | SchemeType::Vietnamese
            | SchemeType::Stroke => None,
        }
    }

    /// Either pinyin scheme resets both pinyin engines (pinyin_candidate_provider.cpp:44-48).
    pub fn reset_cache(&mut self, scheme: SchemeType) {
        match scheme {
            SchemeType::Quanpin | SchemeType::Shuangpin => {
                self.quanpin.reset_cache();
                self.shuangpin.reset_cache();
            }
            SchemeType::Wubi => self.wubi.reset_cache(),
            SchemeType::JapaneseRomaji => self.japanese.reset_cache(),
            // `cantonese.db` and `stroke.db` are read-only and their rows are never rewritten, so there is no cache to drop.
            SchemeType::Korean
            | SchemeType::Cantonese
            | SchemeType::Zhuyin
            | SchemeType::Vietnamese
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
            SchemeType::Quanpin => self.quanpin.insert_online_words(request, words, source),
            SchemeType::Shuangpin => self.shuangpin.insert_online_words(request, words, source),
            SchemeType::Wubi => words.len() == 1,
            SchemeType::JapaneseRomaji => match words {
                [word] => self
                    .japanese
                    .cache_dynamic_candidate(&request.raw_input, word, source),
                _ => false,
            },
            // Korean, Cantonese, Zhuyin, Vietnamese and Stroke take no online rows.
            SchemeType::Korean
            | SchemeType::Cantonese
            | SchemeType::Zhuyin
            | SchemeType::Vietnamese
            | SchemeType::Stroke => false,
        }
    }

    /// Always the pinyin provider, which answers only quanpin and shuangpin requests (pinyin_candidate_provider.cpp:31-42).
    pub fn expand_initial_candidates(
        &mut self,
        request: &QueryRequest,
        candidates: &mut Vec<WordItem>,
    ) -> bool {
        match request.scheme {
            SchemeType::Quanpin => self.quanpin.expand_initial_candidates(request, candidates),
            SchemeType::Shuangpin => self
                .shuangpin
                .expand_initial_candidates(request, candidates),
            SchemeType::Wubi
            | SchemeType::JapaneseRomaji
            | SchemeType::Korean
            | SchemeType::Cantonese
            | SchemeType::Zhuyin
            | SchemeType::Vietnamese
            | SchemeType::Stroke => false,
        }
    }

    /// The `cantonese.db` rows for the request's letters, read again through the activated inventory, as `CantoneseScheme::candidates` lists them. Each row is keyed by the typed letters it covers (`pinyin`, apostrophes kept) and the dictionary key it was found under (`canonical_pinyin`), which is what selecting it takes out of the composition. A read that fails answers nothing, like the wubi table.
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

    /// `stroke.db` 对请求笔画的单字候选，顺序同 `StrokeScheme::candidates`。每行以键入的笔画为 `pinyin`、以该字的完整笔画码为 `canonical_pinyin`；笔画不学习，这两个键只用于显示，从不写回任何词典。读失败时不给候选，与粤拼一样。
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
