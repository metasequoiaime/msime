//! `EngineOptions` and its mapping onto `SessionOptions` (api-contract §2, bridge.cpp:306-407, 698-734).

use std::path::{Path, PathBuf};

use crate::assets;
use crate::diagnostics;
use crate::error::{EngineError, Result};
use crate::paths::RuntimePaths;
use crate::session::SessionOptions;
use crate::types::{
    autocorrect_type, fuzzy_rule, EnglishInputOptions, FrequencyAdjustmentMode,
    FrequencyAdjustmentOptions, FuzzyPinyinOptions, LocalModeOptions, MixedExpressiveOptions,
    SchemeType, SentenceAssociationOptions, ShuangpinProfileKind, WubiInputOptions,
};
use crate::user_dictionary::generation::prepare_runtime_paths;

/// Every field is listed at every construction site; there is deliberately no `Default`.
#[derive(Debug, Clone, PartialEq)]
pub struct EngineOptions {
    pub resources: String,
    pub user_data: String,
    pub cache: String,
    pub dictionaries: String,
    /// 0 quanpin, 1 shuangpin, 2 wubi, 3 Japanese.
    pub scheme: u8,
    /// 0 xiaohe, 1 ziranma, 2 shoudao, 3 microsoft.
    pub shuangpin_profile: u8,
    pub shuangpin_preedit_uses_raw: bool,
    pub learning: bool,
    pub autocorrect_transposition: bool,
    pub autocorrect_neighbor: bool,
    /// Masked with `fuzzy_rule::ALL`.
    pub fuzzy_pinyin_rules: u32,
    pub wubi_mixed_pinyin: bool,
    pub helpcode: bool,
    /// Display only: filtering stays on while annotations are hidden.
    pub show_helpcode: bool,
    pub helpcode_schema: String,
    pub chinese_punctuation: bool,
    pub paired_punctuation: bool,
    pub punctuation_lock: u8,
    pub frequency_mode: String,
    pub frequency_trigger_count: u8,
    pub frequency_linear_step: u8,
    pub mixed_english: bool,
    pub english_minimum_prefix: u8,
    pub mixed_emoji: bool,
    pub mixed_kaomoji: bool,
    pub local_unicode: bool,
    pub local_date_time: bool,
    pub local_quick_phrase: bool,
    pub local_emoji: bool,
    pub local_kaomoji: bool,
    pub local_super_jianpin: bool,
    pub local_temporary_english: bool,
    pub local_temporary_japanese: bool,
    pub sentence_association: SentenceAssociationOptions,
    pub rescoring_context: String,
    /// Ask for every whole-sentence reading; the runtime reorders and crops them.
    pub sentence_alternatives: bool,
}

/// Stage the generation (`prepare_runtime_paths`) and fill the product defaults: quanpin, xiaohe, learning off, autocorrect and fuzzy off, helpcode on with `ziranma`, frequency `promote` 1/1, mixed English from 5 letters, every local mode on, and explicit values for the fields the C++ left default-initialised (`shuangpin_preedit_uses_raw = true`, `wubi_mixed_pinyin = false`, `sentence_association` default, `sentence_alternatives = false`).
pub fn prepare_options(
    resources: &str,
    user_data: &str,
    cache: &str,
    content_id: &str,
) -> Result<EngineOptions> {
    let paths = prepare_runtime_paths(
        Path::new(resources),
        Path::new(user_data),
        Path::new(cache),
        content_id,
    )?;
    // The C++ handed the paths back through `u8string()`; every root came in as UTF-8 and the generation only appends an ASCII content id, so the lossy conversion never actually replaces anything.
    let text = |path: &Path| path.to_string_lossy().into_owned();
    Ok(EngineOptions {
        resources: text(&paths.resources),
        user_data: text(&paths.user_data),
        cache: text(&paths.cache),
        dictionaries: text(&paths.dictionaries),
        scheme: SchemeType::Quanpin as u8,
        shuangpin_profile: ShuangpinProfileKind::Xiaohe as u8,
        shuangpin_preedit_uses_raw: true,
        learning: false,
        autocorrect_transposition: false,
        autocorrect_neighbor: false,
        fuzzy_pinyin_rules: 0,
        wubi_mixed_pinyin: false,
        helpcode: true,
        show_helpcode: true,
        helpcode_schema: "ziranma".to_owned(),
        chinese_punctuation: true,
        paired_punctuation: true,
        punctuation_lock: 0,
        frequency_mode: FrequencyAdjustmentMode::Promote.name().to_owned(),
        frequency_trigger_count: 1,
        frequency_linear_step: 1,
        mixed_english: true,
        english_minimum_prefix: 5,
        mixed_emoji: false,
        mixed_kaomoji: false,
        local_unicode: true,
        local_date_time: true,
        local_quick_phrase: true,
        local_emoji: true,
        local_kaomoji: true,
        local_super_jianpin: true,
        local_temporary_english: true,
        local_temporary_japanese: true,
        sentence_association: SentenceAssociationOptions::default(),
        rescoring_context: String::new(),
        sentence_alternatives: false,
    })
}

/// `options_for`: map and validate (`UNSUPPORTED_INPUT_SCHEME`, `UNSUPPORTED_SHUANGPIN_PROFILE`, `UNSUPPORTED_FREQUENCY_MODE`), and refresh the translations sidecar in the generation directory.
pub fn session_options(options: &EngineOptions) -> Result<SessionOptions> {
    prepare_translation_sidecar(options)?;
    let scheme = SchemeType::from_u8(options.scheme)
        .ok_or_else(|| EngineError::invalid(diagnostics::UNSUPPORTED_INPUT_SCHEME))?;
    let shuangpin_profile = shuangpin_profile(options)?;
    let mode = FrequencyAdjustmentMode::from_name(&options.frequency_mode)
        .ok_or_else(|| EngineError::invalid(diagnostics::UNSUPPORTED_FREQUENCY_MODE))?;
    // Only the two user switches; the quanpin dictionary widens them with missing and extra letters per request (`request_autocorrect_mask`), as the C++ did (bridge.cpp:376-378).
    let autocorrect_types = if options.autocorrect_transposition {
        autocorrect_type::TRANSPOSITION
    } else {
        0
    } | if options.autocorrect_neighbor {
        autocorrect_type::NEIGHBOR
    } else {
        0
    };
    let mut session = SessionOptions::new(runtime_paths(options));
    session.scheme = scheme;
    session.shuangpin_profile = shuangpin_profile;
    session.shuangpin_preedit_uses_raw = options.shuangpin_preedit_uses_raw;
    session.learning = options.learning;
    session.autocorrect_types = autocorrect_types;
    session.fuzzy_pinyin = FuzzyPinyinOptions {
        rules: options.fuzzy_pinyin_rules & fuzzy_rule::ALL,
    };
    session.wubi = WubiInputOptions {
        mixed_pinyin: options.wubi_mixed_pinyin,
    };
    session.chinese_punctuation = options.chinese_punctuation;
    session.paired_punctuation = options.paired_punctuation;
    session.punctuation_lock = i32::from(options.punctuation_lock);
    session.helpcode = options.helpcode;
    session.helpcode_schema = options.helpcode_schema.clone();
    session.frequency = FrequencyAdjustmentOptions {
        mode,
        trigger_count: i32::from(options.frequency_trigger_count),
        linear_step: i32::from(options.frequency_linear_step),
    };
    session.english = EnglishInputOptions {
        mixed_candidates: options.mixed_english,
        minimum_prefix: usize::from(options.english_minimum_prefix),
    };
    session.expressive = MixedExpressiveOptions {
        emoji_candidates: options.mixed_emoji,
        kaomoji_candidates: options.mixed_kaomoji,
    };
    session.local_modes = LocalModeOptions {
        unicode: options.local_unicode,
        date_time: options.local_date_time,
        quick_phrase: options.local_quick_phrase,
        emoji: options.local_emoji,
        kaomoji: options.local_kaomoji,
        super_jianpin: options.local_super_jianpin,
        temporary_english: options.local_temporary_english,
        temporary_japanese: options.local_temporary_japanese,
    };
    session.sentence_alternatives = options.sentence_alternatives;
    session.sentence_association = options.sentence_association;
    session.rescoring_context = options.rescoring_context.clone();
    // `personal_context` keeps its default: the bridge never exposed it (api-contract §1a).
    Ok(session)
}

/// Copy `user_data/custom_translations.txt`, else the resource one, over `dictionaries/custom_translations.txt`, or delete it when neither exists (`TRANSLATION_SIDECAR_FAILED`).
pub fn prepare_translation_sidecar(options: &EngineOptions) -> Result<()> {
    let paths = runtime_paths(options);
    // A relative root would resolve against the host's working directory, so nothing is written until the roots are known to be absolute; the session refuses them with this same error right after.
    paths.validate()?;
    let target = paths.dictionary(assets::TRANSLATIONS);
    let user = paths.user(assets::TRANSLATIONS);
    let source = if is_real_file(&user) {
        user
    } else {
        paths.resource(assets::TRANSLATIONS)
    };
    if !is_real_file(&source) {
        // The C++ removed the stale sidecar with an ignored error_code (bridge.cpp:325-327): a missing target is the normal case, and a sidecar that cannot be removed only keeps glosses the user already had.
        let _ = std::fs::remove_file(&target);
        return Ok(());
    }
    if std::fs::symlink_metadata(&target)
        .map(|metadata| !metadata.file_type().is_file())
        .unwrap_or(false)
    {
        return Err(EngineError::failed(diagnostics::TRANSLATION_SIDECAR_FAILED));
    }
    let copied = match target.parent() {
        Some(parent) => {
            std::fs::create_dir_all(parent).and_then(|()| std::fs::copy(&source, &target))
        }
        None => Err(std::io::ErrorKind::NotFound.into()),
    };
    copied
        .map(|_| ())
        .map_err(|_| EngineError::failed(diagnostics::TRANSLATION_SIDECAR_FAILED))
}

fn is_real_file(path: &Path) -> bool {
    std::fs::symlink_metadata(path)
        .map(|metadata| metadata.file_type().is_file())
        .unwrap_or(false)
}

/// `paths_for` (bridge.cpp:112-117).
pub(super) fn runtime_paths(options: &EngineOptions) -> RuntimePaths {
    RuntimePaths {
        resources: PathBuf::from(&options.resources),
        user_data: PathBuf::from(&options.user_data),
        cache: PathBuf::from(&options.cache),
        dictionaries: PathBuf::from(&options.dictionaries),
    }
}

pub(super) fn shuangpin_profile(options: &EngineOptions) -> Result<ShuangpinProfileKind> {
    ShuangpinProfileKind::from_u8(options.shuangpin_profile)
        .ok_or_else(|| EngineError::invalid(diagnostics::UNSUPPORTED_SHUANGPIN_PROFILE))
}
