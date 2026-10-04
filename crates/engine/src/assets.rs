//! File names of the runtime asset contract (`contracts/assets/assets.json`, contract version 1). Every module names files through these constants so a rename stays one edit.

pub const CONTRACT_VERSION: i32 = 1;

/// The main dictionary (pinyin tables, `wubi86`, `quick_parases`); a working copy lives in each generation.
pub const MAIN_DICTIONARY: &str = "msime-pinyin.db";
/// Separate immutable Wubi tables, split from the pinyin build output.
pub const WUBI_DICTIONARY: &str = "msime-wubi.db";
/// The English dictionary; a working copy lives in each generation.
pub const ENGLISH_DICTIONARY: &str = "msime-english.db";
/// Lattice n-gram tables, copied beside each generation.
pub const BIGRAM_TABLE: &str = "bigram.bin";
pub const TRIGRAM_TABLE: &str = "trigram.bin";
/// Emoji, kaomoji and symbol catalogs; read-only resource.
pub const OTHER_DICTIONARY: &str = "others.db";
pub const JAPANESE_MODEL: &str = "msime-japanese.dat";
pub const JAPANESE_NOTICE: &str = "mozc_dictionary_oss_README.txt";
pub const DICTIONARY_MANIFEST: &str = "dictionary-manifest.json";
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
