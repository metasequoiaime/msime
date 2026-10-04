//! `EngineOptions` and its mapping onto `SessionOptions` (api-contract §2, bridge.cpp:306-407, 698-734).

use std::fs::File;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

use crate::assets;
use crate::diagnostics;
use crate::error::{EngineError, Result};
use crate::helpcode::SharedKeymap;
use crate::paths::RuntimePaths;
use crate::session::SessionOptions;
use crate::types::{
    autocorrect_type, fuzzy_rule, CommandTableEntry, EnglishInputOptions, FrequencyAdjustmentMode,
    FrequencyAdjustmentOptions, FuzzyPinyinOptions, LocalModeOptions, MentionEntry,
    MixedExpressiveOptions, QuickPhraseEntry, SchemeSet, SchemeType, SentenceAssociationOptions,
    ShuangpinProfileKind, WubiInputOptions, WubiProfileKind,
};
use crate::user_dictionary::generation::prepare_runtime_paths_for;
use crate::vietnamese::{InputMethod as VietnameseInputMethod, ToneStyle as VietnameseToneStyle};

const MAX_TRANSLATION_SIDECAR_BYTES: u64 = 1024 * 1024;

/// Every field is listed at every construction site; there is deliberately no `Default`.
#[derive(Debug, Clone, PartialEq)]
pub struct EngineOptions {
    pub resources: String,
    pub user_data: String,
    pub cache: String,
    pub dictionaries: String,
    /// 0 全拼，1 双拼，2 五笔，3 日文，4 韩文，5 粤拼，6 注音，7 越南文，8 藏文，9 笔画。
    pub scheme: u8,
    /// 会话允许运行的方案，`prepare_options` 填 [`SchemeSet::ALL`]，`prepare_options_for` 填调用方给的集合。宿主按产品版本收窄它：`scheme` 不在其中时建会话失败（`INPUT_SCHEME_NOT_ENABLED`），不在其中的方案不构造 provider。
    pub enabled_schemes: SchemeSet,
    /// 0 xiaohe, 1 ziranma, 2 shoudao, 3 microsoft. 双拼不在 `enabled_schemes` 里时不校验，不合法的值按小鹤处理。
    pub shuangpin_profile: u8,
    pub shuangpin_preedit_uses_raw: bool,
    pub learning: bool,
    pub autocorrect_transposition: bool,
    pub autocorrect_neighbor: bool,
    /// Masked with `fuzzy_rule::ALL`.
    pub fuzzy_pinyin_rules: u32,
    pub wubi_mixed_pinyin: bool,
    /// 0 是 86 五笔，1 是 98 五笔（`WubiProfileKind`）。
    pub wubi_profile: u8,
    pub helpcode: bool,
    /// Display only: filtering stays on while annotations are hidden.
    pub show_helpcode: bool,
    pub helpcode_schema: String,
    /// 宿主给的辅助码表（已安装的辅助码表插件）；有它时 Engine 直接用它，`helpcode_schema` 只用来校验和作为回退。
    pub helpcode_table: Option<SharedKeymap>,
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
    /// `V`: calculator, Chinese numerals and dates.
    pub local_expression: bool,
    /// `/`: built-in commands and `command_table`.
    pub local_command: bool,
    /// `@`: `mention_entries`.
    pub local_mention: bool,
    /// The `/` mode's commands beyond the built-in ones. Rows the engine cannot use are dropped, and at most `local::command::TABLE_LIMIT` are kept.
    pub command_table: Vec<CommandTableEntry>,
    /// The `@` mode's names and places, the user's own list. Entries the engine cannot use are dropped, and at most `local::mention::LIST_LIMIT` are kept.
    pub mention_entries: Vec<MentionEntry>,
    /// K 模式在数据库行之后追加的宿主短语（已启用的短语表插件）。用不了的行被丢弃，最多保留 `local::quick_phrase::TABLE_LIMIT` 行。
    pub quick_phrase_table: Vec<QuickPhraseEntry>,
    pub sentence_association: SentenceAssociationOptions,
    pub rescoring_context: String,
    /// Ask for every whole-sentence reading; the runtime reorders and crops them.
    pub sentence_alternatives: bool,
    /// 0 Telex, 1 VNI (`vietnamese::InputMethod`).
    pub vietnamese_input_method: u8,
    /// 0 modern (hoà), 1 classic (hòa) (`vietnamese::ToneStyle`).
    pub vietnamese_tone_style: u8,
    /// Absolute path of `cantonese.db`, empty when the host has none; the Cantonese scheme is unavailable without it.
    pub cantonese_dictionary: String,
    /// Absolute path of `zhuyin.db`, empty when the host has none; the Zhuyin scheme is unavailable without it.
    pub zhuyin_dictionary: String,
    /// Absolute path of `stroke.db`, empty when the host has none; the Stroke scheme is unavailable without it.
    pub stroke_dictionary: String,
    /// `dict_japanese.dat` 的绝对路径；为空时从资源目录读取。
    pub japanese_dictionary: String,
}

/// Stage the generation (`prepare_runtime_paths`) and fill the product defaults: quanpin, xiaohe, Telex with modern tone placement and no language dictionaries, learning off, autocorrect and fuzzy off, helpcode on with `ziranma`, frequency `promote` 1/1, mixed English from 5 letters, every Shift+letter local mode of the reference on and the expression, command and mention modes off with empty tables, and explicit values for the fields the C++ left default-initialised (`shuangpin_preedit_uses_raw = true`, `wubi_mixed_pinyin = false`, `sentence_association` default, `sentence_alternatives = false`).
pub fn prepare_options(
    resources: &str,
    user_data: &str,
    cache: &str,
    content_id: &str,
) -> Result<EngineOptions> {
    prepare_options_for(resources, user_data, cache, content_id, SchemeSet::ALL)
}

/// [`prepare_options`] 按会话允许的方案准备，`enabled_schemes` 也填它。集合不读 `msime.db`（`SchemeSet::reads_main_dictionary`，例如只有日文、越南文或藏文的版本）时，资源目录里不需要 `msime.db`，代次里也没有它（`prepare_runtime_paths_for`）；读它的集合与 [`prepare_options`] 准备出的代次相同。
pub fn prepare_options_for(
    resources: &str,
    user_data: &str,
    cache: &str,
    content_id: &str,
    enabled_schemes: SchemeSet,
) -> Result<EngineOptions> {
    let paths = prepare_runtime_paths_for(
        Path::new(resources),
        Path::new(user_data),
        Path::new(cache),
        content_id,
        enabled_schemes,
    )?;
    // The C++ handed the paths back through `u8string()`; every root came in as UTF-8 and the generation only appends an ASCII content id, so the lossy conversion never actually replaces anything.
    let text = |path: &Path| path.to_string_lossy().into_owned();
    Ok(EngineOptions {
        resources: text(&paths.resources),
        user_data: text(&paths.user_data),
        cache: text(&paths.cache),
        dictionaries: text(&paths.dictionaries),
        scheme: SchemeType::Quanpin as u8,
        enabled_schemes,
        shuangpin_profile: ShuangpinProfileKind::Xiaohe as u8,
        shuangpin_preedit_uses_raw: true,
        learning: false,
        autocorrect_transposition: false,
        autocorrect_neighbor: false,
        fuzzy_pinyin_rules: 0,
        wubi_mixed_pinyin: false,
        wubi_profile: WubiProfileKind::Wubi86 as u8,
        helpcode: true,
        show_helpcode: true,
        helpcode_schema: "ziranma".to_owned(),
        helpcode_table: None,
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
        local_expression: false,
        local_command: false,
        local_mention: false,
        command_table: Vec::new(),
        mention_entries: Vec::new(),
        quick_phrase_table: Vec::new(),
        sentence_association: SentenceAssociationOptions::default(),
        rescoring_context: String::new(),
        sentence_alternatives: false,
        vietnamese_input_method: VietnameseInputMethod::Telex as u8,
        vietnamese_tone_style: VietnameseToneStyle::Modern as u8,
        cantonese_dictionary: String::new(),
        zhuyin_dictionary: String::new(),
        stroke_dictionary: String::new(),
        japanese_dictionary: String::new(),
    })
}

/// `options_for`: map and validate (`UNSUPPORTED_INPUT_SCHEME`, `UNSUPPORTED_SHUANGPIN_PROFILE`, `UNSUPPORTED_WUBI_PROFILE`, `UNSUPPORTED_FREQUENCY_MODE`, `UNSUPPORTED_VIETNAMESE_METHOD`, `UNSUPPORTED_VIETNAMESE_TONE_STYLE`), and refresh the translations sidecar in the generation directory.
pub fn session_options(options: &EngineOptions) -> Result<SessionOptions> {
    prepare_translation_sidecar(options)?;
    let scheme = SchemeType::from_u8(options.scheme)
        .ok_or_else(|| EngineError::invalid(diagnostics::UNSUPPORTED_INPUT_SCHEME))?;
    let shuangpin_profile = shuangpin_profile(options)?;
    let wubi_profile = WubiProfileKind::from_u8(options.wubi_profile)
        .ok_or_else(|| EngineError::invalid(diagnostics::UNSUPPORTED_WUBI_PROFILE))?;
    let mode = FrequencyAdjustmentMode::from_name(&options.frequency_mode)
        .ok_or_else(|| EngineError::invalid(diagnostics::UNSUPPORTED_FREQUENCY_MODE))?;
    let vietnamese_input_method =
        VietnameseInputMethod::from_u8(options.vietnamese_input_method)
            .ok_or_else(|| EngineError::invalid(diagnostics::UNSUPPORTED_VIETNAMESE_METHOD))?;
    let vietnamese_tone_style = VietnameseToneStyle::from_u8(options.vietnamese_tone_style)
        .ok_or_else(|| EngineError::invalid(diagnostics::UNSUPPORTED_VIETNAMESE_TONE_STYLE))?;
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
    session.enabled_schemes = options.enabled_schemes;
    session.shuangpin_profile = shuangpin_profile;
    session.shuangpin_preedit_uses_raw = options.shuangpin_preedit_uses_raw;
    session.vietnamese_input_method = vietnamese_input_method;
    session.vietnamese_tone_style = vietnamese_tone_style;
    session.cantonese_dictionary = PathBuf::from(&options.cantonese_dictionary);
    session.zhuyin_dictionary = PathBuf::from(&options.zhuyin_dictionary);
    session.stroke_dictionary = PathBuf::from(&options.stroke_dictionary);
    session.japanese_dictionary = PathBuf::from(&options.japanese_dictionary);
    session.learning = options.learning;
    session.autocorrect_types = autocorrect_types;
    session.fuzzy_pinyin = FuzzyPinyinOptions {
        rules: options.fuzzy_pinyin_rules & fuzzy_rule::ALL,
    };
    session.wubi = WubiInputOptions {
        mixed_pinyin: options.wubi_mixed_pinyin,
        profile: wubi_profile,
    };
    session.chinese_punctuation = options.chinese_punctuation;
    session.paired_punctuation = options.paired_punctuation;
    session.punctuation_lock = i32::from(options.punctuation_lock);
    session.helpcode = options.helpcode;
    session.helpcode_schema = options.helpcode_schema.clone();
    session.helpcode_table = options.helpcode_table.clone();
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
        expression: options.local_expression,
        command: options.local_command,
        mention: options.local_mention,
    };
    session.command_table = options.command_table.clone();
    session.mention_entries = options.mention_entries.clone();
    session.quick_phrase_table = options.quick_phrase_table.clone();
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
    let parent = target
        .parent()
        .ok_or_else(|| EngineError::failed(diagnostics::TRANSLATION_SIDECAR_FAILED))?;
    reject_storage_ancestors(parent)
        .map_err(|_| EngineError::failed(diagnostics::TRANSLATION_SIDECAR_FAILED))?;
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
    let source_file = File::open(&source)
        .map_err(|_| EngineError::failed(diagnostics::TRANSLATION_SIDECAR_FAILED))?;
    let source_size = source_file
        .metadata()
        .map_err(|_| EngineError::failed(diagnostics::TRANSLATION_SIDECAR_FAILED))?
        .len();
    if source_size > MAX_TRANSLATION_SIDECAR_BYTES {
        return Err(EngineError::failed(diagnostics::TRANSLATION_SIDECAR_FAILED));
    }
    let mut contents = Vec::with_capacity(source_size as usize);
    if source_file
        .take(MAX_TRANSLATION_SIDECAR_BYTES + 1)
        .read_to_end(&mut contents)
        .is_err()
        || contents.len() as u64 > MAX_TRANSLATION_SIDECAR_BYTES
    {
        return Err(EngineError::failed(diagnostics::TRANSLATION_SIDECAR_FAILED));
    }
    let copied = std::fs::create_dir_all(parent).and_then(|()| {
        let metadata = std::fs::symlink_metadata(parent)?;
        if !metadata.is_dir() || metadata.file_type().is_symlink() {
            return Err(io::Error::new(
                io::ErrorKind::InvalidInput,
                "translation sidecar parent is not a real directory",
            ));
        }
        std::fs::write(&target, &contents).map(|()| contents.len() as u64)
    });
    copied
        .map(|_| ())
        .map_err(|_| EngineError::failed(diagnostics::TRANSLATION_SIDECAR_FAILED))
}

fn reject_storage_ancestors(path: &Path) -> io::Result<()> {
    let mut current = PathBuf::new();
    for component in path.components() {
        current.push(component.as_os_str());
        match std::fs::symlink_metadata(&current) {
            Ok(metadata) if metadata.file_type().is_symlink() => {
                if !crate::paths::is_trusted_system_alias(&current) {
                    return Err(io::Error::new(
                        io::ErrorKind::InvalidInput,
                        "translation sidecar path has a symbolic-link ancestor",
                    ));
                }
            }
            Ok(metadata) if !metadata.is_dir() => {
                return Err(io::Error::new(
                    io::ErrorKind::NotADirectory,
                    "translation sidecar parent is not a directory",
                ));
            }
            Ok(_) => {}
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(()),
            Err(error) => return Err(error),
        }
    }
    Ok(())
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

/// 双拼方案的键位。双拼不在 `enabled_schemes` 里时这个值用不上（没有双拼 provider，也切不到双拼），所以不校验：不合法的值按小鹤处理，免得一份只对双拼有意义的偏好让没有双拼的会话建不起来。
pub(super) fn shuangpin_profile(options: &EngineOptions) -> Result<ShuangpinProfileKind> {
    let profile = ShuangpinProfileKind::from_u8(options.shuangpin_profile);
    if !options.enabled_schemes.contains(SchemeType::Shuangpin) {
        return Ok(profile.unwrap_or_default());
    }
    profile.ok_or_else(|| EngineError::invalid(diagnostics::UNSUPPORTED_SHUANGPIN_PROFILE))
}
