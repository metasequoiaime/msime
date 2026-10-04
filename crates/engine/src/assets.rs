//! File names of the runtime asset contract (`contracts/assets/assets.json`, contract version 1). Every module names files through these constants so a rename stays one edit.

pub const CONTRACT_VERSION: i32 = 1;

/// 主词库（拼音各表与 `quick_parases`）。每个代次里有一份工作副本，准备代次时把 `WUBI_DICTIONARY` 的五笔码表并进这份副本。
pub const MAIN_DICTIONARY: &str = "msime-pinyin.db";
/// 单独发布的只读五笔码表（`wubi86`、`wubi98`），从拼音构建产物中拆出。
pub const WUBI_DICTIONARY: &str = "msime-wubi.db";
/// The English dictionary; a working copy lives in each generation.
pub const ENGLISH_DICTIONARY: &str = "msime-english.db";
/// Lattice n-gram tables, copied beside each generation.
pub const BIGRAM_TABLE: &str = "msime-bigram.bin";
pub const TRIGRAM_TABLE: &str = "msime-trigram.bin";
/// Emoji, kaomoji and symbol catalogs; read-only resource.
pub const OTHER_DICTIONARY: &str = "msime-others.db";
pub const JAPANESE_MODEL: &str = "msime-japanese.dat";
pub const JAPANESE_NOTICE: &str = "msime-mozc_dictionary_oss_README.txt";
pub const DICTIONARY_MANIFEST: &str = "msime-dictionary-manifest.json";
/// Hand-written translations; copied from user data or resources into the generation as a sidecar.
pub const TRANSLATIONS: &str = "custom_translations.txt";
/// The user journal every learning write goes through.
pub const USER_JOURNAL: &str = "msime_user.db";
/// The gloss cache name the asset contract gives. msime writes `LEARNED_GLOSSES` instead; that is what users have on disk.
pub const GLOSS_CACHE: &str = "gloss_cache.db";
/// The learned-gloss store the host writes (`save_candidate_gloss`) and reads first.
pub const LEARNED_GLOSSES: &str = "translation-glosses.db";
/// Written last into a staged generation; its presence means the generation is complete.
pub const GENERATION_READY: &str = ".ready";

/// Keyboard sentence model (speed), `CandidateSource::NeuralKeyboard`.
pub const NEURAL_MODEL_KEYBOARD: &str = "sentence-model.safetensors";

/// Built-in helpcode schemas and their files under the resource root.
pub const HELPCODES: [(&str, &str); 6] = [
    ("lantian", "helpcodes/helpcode.txt"),
    ("ziranma", "helpcodes/zrm_helpcode_big_unique.txt"),
    ("shouyou2_0", "helpcodes/shouyou2_0_helpcode.txt"),
    ("shouyouplus", "helpcodes/shouyouplus_helpcode.txt"),
    ("xiaohe", "helpcodes/xiaohe_helpcode.txt"),
    ("jiajia", "helpcodes/jiajia_helpcode.txt"),
];

/// `custom/<stem>` schemas resolve to `helpcodes/custom/<stem>.txt`.
pub const CUSTOM_HELPCODE_PREFIX: &str = "custom/";
pub const CUSTOM_HELPCODE_DIRECTORY: &str = "helpcodes/custom";
