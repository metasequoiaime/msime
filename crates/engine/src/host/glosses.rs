//! Display-only glosses and English completions (api-contract §1c, bridge.cpp:149-237, 554-572, 1138-1236).

use std::cell::RefCell;
use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags, OptionalExtension};

use crate::assets;
use crate::diagnostics;
use crate::dictionary::english::{
    load_custom_translations, upsert_gloss, CustomTranslations, EnglishDictionary,
};
use crate::error::{EngineError, Result};
use crate::japanese::decoder::FileIdentity;
use crate::types::CandidateSource;

/// Senses shown beside a candidate; the rest of a dictionary gloss is cut (bridge.cpp:209).
const MAXIMUM_GLOSS_SENSES: usize = 2;
const FULLWIDTH_SENSE_DELIMITER: &str = "；";
const MAXIMUM_ENGLISH_COMPLETIONS: usize = 32;

/// ASCII letters only, `limit` 1..=32, lowercased; empty prefix gives nothing (`INVALID_ENGLISH_COMPLETION_*`, `ENGLISH_DICTIONARY_UNAVAILABLE`).
pub fn english_completions(resources: &str, prefix: &str, limit: usize) -> Result<Vec<String>> {
    if limit == 0 || limit > MAXIMUM_ENGLISH_COMPLETIONS {
        return Err(EngineError::invalid(
            diagnostics::INVALID_ENGLISH_COMPLETION_LIMIT,
        ));
    }
    if !prefix.bytes().all(|byte| byte.is_ascii_alphabetic()) {
        return Err(EngineError::invalid(
            diagnostics::INVALID_ENGLISH_COMPLETION_PREFIX,
        ));
    }
    if prefix.is_empty() {
        return Ok(Vec::new());
    }
    let dictionary = EnglishDictionary::open(
        &Path::new(resources).join(assets::ENGLISH_DICTIONARY),
        None,
        None,
    );
    if !dictionary.ready() {
        return Err(EngineError::failed(
            diagnostics::ENGLISH_DICTIONARY_UNAVAILABLE,
        ));
    }
    // `word` is the display spelling, which is what the C++ `WordItem::word` returned too.
    Ok(dictionary
        .query_prefix(&prefix.to_ascii_lowercase(), limit)
        .into_iter()
        .map(|item| item.word.into_string())
        .collect())
}

thread_local! {
    // 每个宿主查询线程只保留最近一个固定资源目录，缓存不会随输入内容无限增长。
    static ENGLISH_CORRECTION: std::cell::RefCell<Option<(std::path::PathBuf, EnglishDictionary)>> = const { std::cell::RefCell::new(None) };
}

/// 释义词库的缓存键：词库路径、词库文件身份，以及 `EnglishDictionary::open` 一并读进来的同目录 `custom_translations.txt` 的身份。
type GlossDictionaryKey = (PathBuf, Option<FileIdentity>, Option<FileIdentity>);

/// 读好的自定义译文和它的缓存键：文件路径与文件身份。
type CachedCustomTranslations = ((PathBuf, Option<FileIdentity>), CustomTranslations);

/// 离线释义库连接最多留几份：汉字、词语两份英文表，加上六种离线语言各一份。
const TARGET_GLOSS_CONNECTION_CAPACITY: usize = 8;

/// Windows 上 SQLite 打开文件不带 `FILE_SHARE_DELETE`，查询线程一直开着的词库会让卸载离线释义包、更新资源时删不掉文件，而 Windows 的翻译工作线程与输入法服务同寿命；那里照旧每次查完就关，只缓存读好的自定义译文。
const KEEP_GLOSS_DATABASES_OPEN: bool = !cfg!(windows);

thread_local! {
    // 打开候选释义时每个按键都要查一遍：原来每次都重新打开 msime-english.db、学到的释义库、汉字和词语释义库，再读两遍 custom_translations.txt，低端机上一键要好几毫秒。这里按路径加文件身份留着打开过的词库和读好的自定义译文：文件没换就复用，下载了新的离线释义、设置页改了自定义译文、换了资源包，文件身份一变下一次查询就重新打开，结果和每次都重新打开相同。每个来源只留最近一个路径（离线释义库按上面的上限），内存有上限。学到的释义库由后台写入，SQLite 在每次读事务开始时都会看到已提交的新内容，文件被整个替换时身份也会变。
    static PACKAGED_GLOSS_DICTIONARY: RefCell<Option<(GlossDictionaryKey, EnglishDictionary)>> = const { RefCell::new(None) };
    static LEARNED_GLOSS_DICTIONARY: RefCell<Option<(GlossDictionaryKey, EnglishDictionary)>> = const { RefCell::new(None) };
    static CUSTOM_TRANSLATIONS: RefCell<Option<CachedCustomTranslations>> = const { RefCell::new(None) };
    static TARGET_GLOSS_CONNECTIONS: RefCell<Vec<(PathBuf, FileIdentity, Connection)>> = const { RefCell::new(Vec::new()) };
}

/// 关掉调用线程留着的释义词库连接，丢掉读好的自定义译文（发布词库、学到的释义库、自定义译文、离线释义库四份缓存）。缓存跟着查询线程活着，在鸿蒙上就是 ArkTS 的 UI 线程，等于整个进程的寿命；键盘收起、关掉释义以后它们本来一直留着。宿主在键盘收起、内存吃紧时调用，下一次查询照常重新打开，结果不变。只清调用线程自己的那份。
pub fn release_gloss_caches() {
    PACKAGED_GLOSS_DICTIONARY.with_borrow_mut(|slot| *slot = None);
    LEARNED_GLOSS_DICTIONARY.with_borrow_mut(|slot| *slot = None);
    CUSTOM_TRANSLATIONS.with_borrow_mut(|slot| *slot = None);
    TARGET_GLOSS_CONNECTIONS.with_borrow_mut(Vec::clear);
}

/// 手机英文始终以原始按键为第一行，仅在需要时附加唯一纠错；不创建或改变输入会话。
pub fn english_corrections(resources: &str, raw: &str) -> Result<Vec<String>> {
    validate_mobile_english(raw)?;
    let mut candidates = vec![raw.to_owned()];
    with_correction_dictionary(resources, |dictionary| {
        if let Some(correction) = dictionary.correct_mobile_english(raw) {
            candidates.push(correction);
        }
    });
    Ok(candidates)
}

/// 手机英文的合并展示，最多 `limit` 行：有纠错时只有原文与纠错；原文已是完整的词（词库精确命中、固定词形或受保护的网络缩写）时只有原文；否则原文后接前缀补全，首字母随原文大写。布尔值为真当且仅当第二行是纠错。不创建或改变输入会话。
pub fn english_correction_display(
    resources: &str,
    raw: &str,
    limit: usize,
) -> Result<(Vec<String>, bool)> {
    if limit == 0 || limit > MAXIMUM_ENGLISH_COMPLETIONS {
        return Err(EngineError::invalid(
            diagnostics::INVALID_ENGLISH_COMPLETION_LIMIT,
        ));
    }
    validate_mobile_english(raw)?;
    // 缺词库时与纠错查询一样：固定词形照常恢复，其余只有原文。
    let mut display = (vec![raw.to_owned()], false);
    with_correction_dictionary(resources, |dictionary| {
        display = dictionary.mobile_english_display(raw, limit);
    });
    Ok(display)
}

fn validate_mobile_english(raw: &str) -> Result<()> {
    if raw.is_empty()
        || raw.len() > 128
        || !raw
            .bytes()
            .all(|byte| byte.is_ascii_alphabetic() || byte == b'\'')
    {
        return Err(EngineError::invalid(
            diagnostics::INVALID_ENGLISH_COMPLETION_PREFIX,
        ));
    }
    Ok(())
}

/// 复用本线程最近一次打开的词库及其纠错缓存；资源目录变化或词库此前不可用时重新打开。
fn with_correction_dictionary(resources: &str, use_dictionary: impl FnOnce(&EnglishDictionary)) {
    ENGLISH_CORRECTION.with(|slot| {
        let mut slot = slot.borrow_mut();
        let path = Path::new(resources).join(assets::ENGLISH_DICTIONARY);
        if slot
            .as_ref()
            .is_none_or(|(cached, dictionary)| cached != &path || !dictionary.ready())
        {
            *slot = Some((path.clone(), EnglishDictionary::open(&path, None, None)));
        }
        if let Some((_, dictionary)) = slot.as_ref() {
            use_dictionary(dictionary);
        }
    });
}

/// Parallel to `candidates`; empty for ineligible rows (emoji, kaomoji, neither Latin nor Han).
pub fn candidate_glosses(resources: &str, candidates: &[(String, u8)]) -> Result<Vec<String>> {
    candidate_glosses_with_user(resources, "", candidates)
}

/// Hand-written translations outrank learned glosses, which outrank packaged ones; at most two senses shown. Empty `resources` means the learned store only.
pub fn candidate_glosses_with_user(
    resources: &str,
    user_data: &str,
    candidates: &[(String, u8)],
) -> Result<Vec<String>> {
    PACKAGED_GLOSS_DICTIONARY.with_borrow_mut(|packaged_slot| {
        LEARNED_GLOSS_DICTIONARY.with_borrow_mut(|learned_slot| {
            CUSTOM_TRANSLATIONS.with_borrow_mut(|custom_slot| {
                // An empty resource path requests only the user overlay, never a relative `msime-english.db` (bridge.cpp:1161-1162).
                let packaged = if resources.is_empty() {
                    None
                } else {
                    cached_gloss_dictionary(
                        packaged_slot,
                        &Path::new(resources).join(assets::ENGLISH_DICTIONARY),
                    )
                };
                // The user's own `custom_translations.txt` is read directly. The bridge reached it only as the learned store's sidecar (bridge.cpp:1147-1166), so before the first online gloss created `translation-glosses.db` the file Settings writes was ignored.
                let no_custom = CustomTranslations::default();
                let custom = if user_data.is_empty() {
                    &no_custom
                } else {
                    cached_custom_translations(
                        custom_slot,
                        &Path::new(user_data).join(assets::TRANSLATIONS),
                    )
                };
                let learned = if user_data.is_empty() {
                    None
                } else {
                    cached_gloss_dictionary(
                        learned_slot,
                        &Path::new(user_data).join(assets::LEARNED_GLOSSES),
                    )
                };
                let glosses = glosses_from(packaged, custom, learned, candidates);
                if !KEEP_GLOSS_DATABASES_OPEN {
                    *packaged_slot = None;
                    *learned_slot = None;
                }
                glosses
            })
        })
    })
}

/// 手写译文、学到的释义、发布词库三层按顺序查。
fn glosses_from(
    packaged: Option<&EnglishDictionary>,
    custom: &CustomTranslations,
    learned: Option<&EnglishDictionary>,
    candidates: &[(String, u8)],
) -> Result<Vec<String>> {
    if packaged.is_none() && learned.is_none() && custom.en_zh.is_empty() && custom.zh_en.is_empty()
    {
        return Err(EngineError::failed(
            diagnostics::CANDIDATE_GLOSS_UNAVAILABLE,
        ));
    }
    let lookup = |dictionary: &EnglishDictionary, key: &str, chinese_to_english: bool| {
        candidate_gloss_display(&if chinese_to_english {
            dictionary.query_english_gloss(key)
        } else {
            dictionary.query_chinese_gloss(key)
        })
    };
    Ok(candidates
        .iter()
        .map(|(text, source)| {
            let Some((key, chinese_to_english)) = candidate_gloss_key(text, *source) else {
                return String::new();
            };
            let hand_written = if chinese_to_english {
                custom.zh_en.get(&key)
            } else {
                custom.en_zh.get(&key)
            };
            if let Some(gloss) = hand_written {
                return candidate_gloss_display(gloss);
            }
            let gloss = learned
                .map(|dictionary| lookup(dictionary, &key, chinese_to_english))
                .unwrap_or_default();
            if !gloss.is_empty() {
                return gloss;
            }
            packaged
                .map(|dictionary| lookup(dictionary, &key, chinese_to_english))
                .unwrap_or_default()
        })
        .collect())
}

pub fn save_candidate_gloss(
    user_data: &str,
    chinese_to_english: bool,
    key: &str,
    gloss: &str,
) -> bool {
    upsert_gloss(
        &Path::new(user_data).join(assets::LEARNED_GLOSSES),
        chinese_to_english,
        key,
        gloss,
    )
}

/// Chinese rows only, from an offline `zh-<lang>.db` at `user_version = 1` whose `meta.target_language` matches; never creates the file.
pub fn candidate_target_glosses(
    database_path: &str,
    target_language: &str,
    candidates: &[(String, u8)],
) -> Result<Vec<String>> {
    // An empty path would open a private temporary database rather than fail.
    if database_path.is_empty() {
        return Err(EngineError::failed(diagnostics::OFFLINE_GLOSS_UNAVAILABLE));
    }
    let path = Path::new(database_path);
    TARGET_GLOSS_CONNECTIONS.with_borrow_mut(|connections| {
        // 版本和语言每次照旧检查，缓存只省掉打开文件、读结构和准备语句。不是普通文件（不存在、符号链接）时不进缓存，照原路打开，得到原来的错误。
        let identity = FileIdentity::at(path);
        let cached = identity.and_then(|identity| {
            connections.iter().position(|(cached, cached_identity, _)| {
                cached == path && *cached_identity == identity
            })
        });
        let index = match cached {
            Some(index) => index,
            None => {
                connections.retain(|(cached, _, _)| cached != path);
                let connection = open_target_gloss_database(path)?;
                let Some(identity) = identity.filter(|_| KEEP_GLOSS_DATABASES_OPEN) else {
                    return target_glosses_from(&connection, target_language, candidates);
                };
                if connections.len() >= TARGET_GLOSS_CONNECTION_CAPACITY {
                    connections.remove(0);
                }
                connections.push((path.to_path_buf(), identity, connection));
                connections.len() - 1
            }
        };
        target_glosses_from(&connections[index].2, target_language, candidates)
    })
}

fn open_target_gloss_database(path: &Path) -> Result<Connection> {
    let database_path = crate::paths::sqlite_read_only_path_no_follow(path)
        .map_err(|_| EngineError::failed(diagnostics::OFFLINE_GLOSS_UNAVAILABLE))?;
    let connection = Connection::open_with_flags(
        &database_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY
            | OpenFlags::SQLITE_OPEN_NOFOLLOW
            | OpenFlags::SQLITE_OPEN_FULL_MUTEX,
    )
    .map_err(|_| EngineError::failed(diagnostics::OFFLINE_GLOSS_UNAVAILABLE))?;
    // bridge.cpp:1194 set no busy timeout; rusqlite's default 5 s wait is not the reference's behaviour.
    connection
        .busy_timeout(std::time::Duration::ZERO)
        .map_err(|_| EngineError::failed(diagnostics::OFFLINE_GLOSS_UNAVAILABLE))?;
    Ok(connection)
}

fn target_glosses_from(
    connection: &Connection,
    target_language: &str,
    candidates: &[(String, u8)],
) -> Result<Vec<String>> {
    let unreadable = |_| EngineError::failed(diagnostics::OFFLINE_GLOSS_UNREADABLE);
    // SQLite opens lazily, so a file that is not a database first fails here, as it did in the C++ prepare (bridge.cpp:1198-1206).
    let version = connection
        .prepare_cached("PRAGMA user_version")
        .map_err(unreadable)?
        .query_row((), |row| row.get::<_, i64>(0))
        // A step that fails reads as a version the host cannot use, as the C++ check did (bridge.cpp:1207-1208).
        .ok();
    if version != Some(1) {
        return Err(EngineError::failed(
            diagnostics::OFFLINE_GLOSS_VERSION_UNSUPPORTED,
        ));
    }
    let stored = connection
        .prepare_cached("SELECT value FROM meta WHERE key = 'target_language'")
        .map_err(unreadable)?
        .query_row((), |row| row.get::<_, Option<String>>(0))
        .ok()
        .flatten();
    if stored.as_deref() != Some(target_language) {
        return Err(EngineError::failed(
            diagnostics::OFFLINE_GLOSS_LANGUAGE_MISMATCH,
        ));
    }
    let mut lookup = connection
        .prepare_cached("SELECT gloss FROM zh_glosses WHERE chinese = ?1")
        .map_err(unreadable)?;
    let mut output = Vec::with_capacity(candidates.len());
    for (text, source) in candidates {
        let gloss = match candidate_gloss_key(text, *source) {
            Some((key, true)) => lookup
                .query_row((key,), |row| row.get::<_, Option<String>>(0))
                .optional()
                .map_err(|_| EngineError::failed(diagnostics::OFFLINE_GLOSS_READ_FAILED))?
                .flatten()
                .map(|gloss| candidate_gloss_display(&gloss))
                .unwrap_or_default(),
            _ => String::new(),
        };
        output.push(gloss);
    }
    Ok(output)
}

/// IPA of lowercase English words from a `pronunciations/en-phonetic.db` built by scripts/build_pronunciations.py, parallel to `words`; an unknown word is empty. Like the offline gloss files, the version and kind it states must match, so a gloss database dropped in its place is refused rather than read as IPA; never creates the file.
pub fn english_phonetics(database_path: &str, words: &[String]) -> Result<Vec<String>> {
    phonetics(database_path, "en", words)
}

/// 读音表的语言：英文表来自 ECDICT，其余六种来自维基词典（scripts/build_target_pronunciations.py）。表名和 `meta.kind` 都由它拼出，所以只接受这几个固定值。
pub const PHONETIC_LANGUAGES: [&str; 7] = ["en", "fr", "ja", "es", "ru", "de", "ko"];

/// 从 `<language>-phonetic.db` 读 `words` 里每个词的读音，与 `words` 一一对应；查不到的词为空串。英文是 `pronunciations/en-phonetic.db`（键是小写，IPA 不带斜线）；其他语言是 scripts/build_target_pronunciations.py 生成的 `target-pronunciations/<language>-phonetic.db`，键是维基词典的写法，存的是带斜线或方括号的 IPA，日文是赫本式罗马字。文件必须声明版本 1 和种类 `<language>_phonetic`，放在它位置上的释义数据库或另一种语言的表都会被拒绝；从不创建文件。
pub fn phonetics(database_path: &str, language: &str, words: &[String]) -> Result<Vec<String>> {
    if !PHONETIC_LANGUAGES.contains(&language) {
        return Err(EngineError::failed(
            diagnostics::PRONUNCIATION_KIND_MISMATCH,
        ));
    }
    // An empty path would open a private temporary database rather than fail.
    if database_path.is_empty() {
        return Err(EngineError::failed(diagnostics::PRONUNCIATION_UNAVAILABLE));
    }
    let database_path = crate::paths::sqlite_read_only_path_no_follow(Path::new(database_path))
        .map_err(|_| EngineError::failed(diagnostics::PRONUNCIATION_UNAVAILABLE))?;
    let connection = Connection::open_with_flags(
        &database_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY
            | OpenFlags::SQLITE_OPEN_NOFOLLOW
            | OpenFlags::SQLITE_OPEN_FULL_MUTEX,
    )
    .map_err(|_| EngineError::failed(diagnostics::PRONUNCIATION_UNAVAILABLE))?;
    connection
        .busy_timeout(std::time::Duration::ZERO)
        .map_err(|_| EngineError::failed(diagnostics::PRONUNCIATION_UNAVAILABLE))?;
    let unreadable = |_| EngineError::failed(diagnostics::PRONUNCIATION_UNREADABLE);
    let version = connection
        .prepare("PRAGMA user_version")
        .map_err(unreadable)?
        .query_row((), |row| row.get::<_, i64>(0))
        .ok();
    if version != Some(1) {
        return Err(EngineError::failed(
            diagnostics::PRONUNCIATION_VERSION_UNSUPPORTED,
        ));
    }
    let kind = connection
        .prepare("SELECT value FROM meta WHERE key = 'kind'")
        .map_err(unreadable)?
        .query_row((), |row| row.get::<_, Option<String>>(0))
        .ok()
        .flatten();
    if kind.as_deref() != Some(format!("{language}_phonetic").as_str()) {
        return Err(EngineError::failed(
            diagnostics::PRONUNCIATION_KIND_MISMATCH,
        ));
    }
    let mut lookup = connection
        .prepare(&format!(
            "SELECT phonetic FROM {language}_phonetics WHERE word = ?1"
        ))
        .map_err(unreadable)?;
    words
        .iter()
        .map(|word| {
            lookup
                .query_row((word,), |row| row.get::<_, Option<String>>(0))
                .optional()
                .map(|phonetic| phonetic.flatten().unwrap_or_default())
                .map_err(|_| EngineError::failed(diagnostics::PRONUNCIATION_READ_FAILED))
        })
        .collect()
}

/// `open_dictionary` (bridge.cpp:1150-1158): only a regular file whose prefix statement prepares.
fn open_gloss_dictionary(path: &Path) -> Option<EnglishDictionary> {
    if !std::fs::symlink_metadata(path).ok()?.file_type().is_file() {
        return None;
    }
    Some(EnglishDictionary::open(path, None, None)).filter(EnglishDictionary::ready)
}

/// `slot` 里那份词库，词库或它旁边的 `custom_translations.txt` 换过时重新打开；打不开的不留，下一次照旧重试。
fn cached_gloss_dictionary<'a>(
    slot: &'a mut Option<(GlossDictionaryKey, EnglishDictionary)>,
    path: &Path,
) -> Option<&'a EnglishDictionary> {
    let sidecar = path
        .parent()
        .unwrap_or(Path::new(""))
        .join(assets::TRANSLATIONS);
    let key = (
        path.to_path_buf(),
        FileIdentity::at(path),
        FileIdentity::at(&sidecar),
    );
    if slot.as_ref().is_none_or(|(cached, _)| cached != &key) {
        *slot = open_gloss_dictionary(path).map(|dictionary| (key, dictionary));
    }
    slot.as_ref().map(|(_, dictionary)| dictionary)
}

/// `slot` 里读好的自定义译文，文件换过或改过时重新读。
fn cached_custom_translations<'a>(
    slot: &'a mut Option<CachedCustomTranslations>,
    path: &Path,
) -> &'a CustomTranslations {
    let key = (path.to_path_buf(), FileIdentity::at(path));
    if slot.as_ref().is_none_or(|(cached, _)| cached != &key) {
        *slot = Some((key, load_custom_translations(path)));
    }
    &slot
        .as_ref()
        .expect("custom translations were just cached")
        .1
}

/// `candidate_gloss_key` (bridge.cpp:149-188): `(key, chinese_to_english)`, or `None` for a row that gets no gloss. Emoji and kaomoji never do; text of ASCII letters, spaces, `-` and `'` with at least one letter is looked up lowercased as English; otherwise any Han character makes the whole text a Chinese key. `&str` is valid UTF-8, so the C++ rejection of malformed text cannot arise.
///
/// 日文混输行（`CandidateSource::MixedJapanese`）也没有释义：私、寿司 这样的汉字是日文，拿去查中文释义会在日文下面写出中文意思，和日文方案、临时日文不出释义一致。
///
/// 脚本插件的行（`CandidateSource::Plugin`）也没有释义：插件的文字不交给任何释义来源，宿主同样不为它请求在线释义或翻译。
pub(super) fn candidate_gloss_key(text: &str, source: u8) -> Option<(String, bool)> {
    if source == CandidateSource::Emoji as u8
        || source == CandidateSource::Kaomoji as u8
        || source == CandidateSource::MixedJapanese as u8
        || source == CandidateSource::Plugin as u8
    {
        return None;
    }
    let latin = text
        .bytes()
        .all(|byte| byte.is_ascii_alphabetic() || matches!(byte, b' ' | b'-' | b'\''));
    if latin && text.bytes().any(|byte| byte.is_ascii_alphabetic()) {
        return Some((text.to_ascii_lowercase(), false));
    }
    if text.chars().any(is_gloss_han) {
        return Some((text.to_owned(), true));
    }
    None
}

/// The bridge's Han ranges (bridge.cpp:143-148), narrower than `text::is_han`: no U+3007 and nothing past U+2FA1F.
fn is_gloss_han(character: char) -> bool {
    matches!(character as u32, 0x3400..=0x4DBF | 0x4E00..=0x9FFF | 0xF900..=0xFAFF | 0x20000..=0x2FA1F)
}

/// `candidate_gloss_display` (bridge.cpp:206-237): senses split on `;` or `；`, ASCII whitespace collapsed, empty senses dropped, at most two joined with `"; "`. A control character anywhere in the result withholds the whole gloss, because a learned gloss came from the network and must not reach the candidate window unfiltered.
pub(super) fn candidate_gloss_display(text: &str) -> String {
    // Joining two senses adds at most one byte beyond the source's separators.
    let mut output = String::with_capacity(text.len() + 1);
    let mut count = 0;
    let mut rest = text;
    while count < MAXIMUM_GLOSS_SENSES {
        let ascii = rest.find(';');
        let fullwidth = rest.find(FULLWIDTH_SENSE_DELIMITER);
        let (end, width) = match (ascii, fullwidth) {
            (_, Some(wide)) if ascii.is_none_or(|narrow| wide < narrow) => {
                (Some(wide), FULLWIDTH_SENSE_DELIMITER.len())
            }
            (narrow, _) => (narrow, 1),
        };
        let sense = collapse_ascii_whitespace(&rest[..end.unwrap_or(rest.len())]);
        if !sense.is_empty() {
            if !output.is_empty() {
                output.push_str("; ");
            }
            output.push_str(&sense);
            count += 1;
        }
        let Some(end) = end else {
            break;
        };
        rest = &rest[end + width..];
    }
    if output
        .chars()
        .any(|character| matches!(character as u32, 0..=0x1F | 0x7F..=0x9F))
    {
        return String::new();
    }
    output
}

/// bridge.cpp:189-205: runs of space, tab, CR and LF become one space; leading and trailing runs vanish.
fn collapse_ascii_whitespace(text: &str) -> String {
    let mut output = String::with_capacity(text.len());
    let mut pending_space = false;
    for character in text.chars() {
        if matches!(character, ' ' | '\t' | '\r' | '\n') {
            pending_space = !output.is_empty();
            continue;
        }
        if pending_space {
            output.push(' ');
            pending_space = false;
        }
        output.push(character);
    }
    output
}
