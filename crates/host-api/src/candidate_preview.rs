//! 在本进程里按用户的设置打一串键，看输入法会给出哪些候选（`msime-mcp candidates`，#9663）。
//!
//! 走的是桌面宿主打字的同一条路：用同一份运行时选项（偏好换成现行的 preferences.json）经 `create_session` 建 `HostSession`，聚焦，逐键经 `dispatch_value` 送 `Action::Character`（宿主把字母、`'`、`;` 和标点键都这样送），把光标左移到要的位置，再像宿主停顿 150 ms 后那样跑一次停顿重排，最后取展开后的整张候选表。排序、插件、模糊音、学过的词频和用户词都是输入法自己的那一份，不另写一套。为什么不连到正在运行的输入法进程，见 `.agents/notes/implemented/feature/2026-10-25-command-line-candidates.md`。
//!
//! 会话只用来看：建会话时把 `learning` 关掉，选词、上屏（数字键、标点、五笔四码唯一时的自动上屏）都不会调词频、造词或记上下文；`SessionUse::Preview` 不记打字统计，也不向设置页报告词库代次已经载入。云候选和 AI 候选要宿主替会话去网络上取，这里不取，占位行也不列出。

use crate::ffi::session::{create_session, focus_session, SessionUse};
use crate::*;

/// 一次最多打多少个键。比 `lookup_candidates` 的 64 宽一些，够打一整句双拼。
pub const MAX_PREVIEW_KEYS: usize = 128;
/// 一次最多列多少个候选。
pub const MAX_PREVIEW_CANDIDATES: usize = 500;

/// 打完一串键之后的样子。
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct CandidatePreview {
    /// 打下的键，原样。
    pub keys: String,
    /// 光标在 `keys` 里的位置（它前面有几个键），也就是请求的位置。
    pub caret: usize,
    /// 宿主画在行内的预编辑串，例如全拼的 `ni hao`。
    pub preedit: String,
    /// 还在组字的那段按键（最后一次上屏之后打的键）。
    pub editing_text: String,
    /// 引擎报告的光标在 `editing_text` 里的字节位置。
    pub editing_caret: usize,
    /// 打这串键的途中上屏的文字，按先后：数字键或标点选了词、五笔四码唯一时自动上屏、第五码顶出首选等。宿主会把它们写进文档，不再出现在候选里。
    pub committed: Vec<String>,
    /// 候选窗每页几个，前这么多个就是第一页。
    pub page_size: usize,
    /// 候选，按候选窗里的顺序。
    pub candidates: Vec<PreviewCandidate>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct PreviewCandidate {
    /// 上屏的文字；用户开了繁体输出时已经按宿主的做法转成繁体。
    pub text: String,
    /// 这个候选对应的编码，例如 `ni'hao`。
    pub code: String,
    /// 候选从哪里来，见 [`source_name`]。
    pub source: &'static str,
    /// 引擎附在候选后面的注释（辅助码、韩文汉字释义等），没有时为空。
    pub annotation: String,
    /// 引擎纠正了拼写才给出的候选。
    pub corrected: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PreviewError {
    /// 运行时选项不是宿主能用的文档，或者偏好不合规。
    Options(String),
    /// 词库正在维护（导入、恢复、换代次），会话暂时建不起来。
    Busy,
    /// 词库打不开，或者会话因为别的原因建不起来。
    Unavailable(String),
    /// 按键或光标位置不合规，或者输入法不收其中某个键。
    Input(String),
}

impl std::fmt::Display for PreviewError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Options(reason) | Self::Unavailable(reason) | Self::Input(reason) => {
                formatter.write_str(reason)
            }
            Self::Busy => formatter.write_str(
                "the dictionaries are being updated or imported; try again when that finishes",
            ),
        }
    }
}

impl std::error::Error for PreviewError {}

/// 候选来源在 JSON 里的名字。
pub fn source_name(source: u8) -> &'static str {
    use msime_engine::CandidateSource as Source;
    match Source::from_u8(source) {
        Some(Source::Database) => "dictionary",
        Some(Source::UserDatabase) => "user_dictionary",
        Some(Source::CloudSuggestion) => "cloud",
        Some(Source::AiSuggestion) => "ai",
        Some(Source::EnglishDictionary) => "english",
        Some(Source::QuickPhrase) => "quick_phrase",
        Some(Source::Emoji) => "emoji",
        Some(Source::Kaomoji) => "kaomoji",
        Some(Source::Generated) => "generated",
        Some(Source::Fallback) => "fallback",
        Some(Source::NeuralKeyboard) => "sentence_model",
        Some(Source::AiPending) => "ai_pending",
        Some(Source::MixedJapanese) => "japanese",
        Some(Source::Plugin) => "plugin",
        Some(Source::CloudPending) => "cloud_pending",
        Some(Source::Contact) => "contact",
        Some(Source::Collocation) => "collocation",
        Some(Source::Multilingual) => "multilingual",
        None => "other",
    }
}

/// 按键和光标位置是否合规：1 到 [`MAX_PREVIEW_KEYS`] 个可打印的 ASCII 字符（不含空格），光标不超过键数。
fn check_keys(keys: &str, caret: usize) -> Result<(), PreviewError> {
    if keys.is_empty() || keys.len() > MAX_PREVIEW_KEYS {
        return Err(PreviewError::Input(format!(
            "type 1 to {MAX_PREVIEW_KEYS} keys"
        )));
    }
    if let Some(position) = keys.bytes().position(|byte| !byte.is_ascii_graphic()) {
        return Err(PreviewError::Input(format!(
            "key {} is not a printable ASCII character other than space",
            position + 1
        )));
    }
    if caret > keys.len() {
        return Err(PreviewError::Input(format!(
            "the caret at {caret} is past the {} keys",
            keys.len()
        )));
    }
    Ok(())
}

/// 用 `host_options`（宿主建会话用的那份运行时选项，偏好已经是现行的）打 `keys`，光标停在第 `caret` 个键之后，列出至多 `limit` 个候选。每次调用都新建一个会话、用完就销毁，不在两次调用之间占着词库的租约，词库维护随时可以进行，下一次调用也就读到维护之后的词库。必须在调用它的线程上完成，会话不跨线程。
pub fn preview_candidates(
    host_options: &Value,
    keys: &str,
    caret: usize,
    limit: usize,
) -> Result<CandidatePreview, PreviewError> {
    if !(1..=MAX_PREVIEW_CANDIDATES).contains(&limit) {
        return Err(PreviewError::Input(format!(
            "the limit must be between 1 and {MAX_PREVIEW_CANDIDATES}"
        )));
    }
    check_keys(keys, caret)?;
    let mut document = host_options.clone();
    with_host_requests(&mut document);
    // 只看不学：选词和上屏都不改用户的词频、造词和上下文。排序只在一串键中途上屏之后才和打字时不同（上下文重排要求学习开着），见模块说明。
    if let Some(preferences) = document
        .get_mut("preferences")
        .and_then(Value::as_object_mut)
    {
        preferences.insert("learning".into(), Value::Bool(false));
    }
    let bytes =
        serde_json::to_vec(&document).map_err(|error| PreviewError::Options(error.to_string()))?;
    if bytes.len() > HOST_OPTIONS_DOCUMENT_LIMIT {
        return Err(PreviewError::Options(
            "the runtime options are too large".into(),
        ));
    }
    catch_unwind(AssertUnwindSafe(|| {
        let created =
            create_session(&bytes, SessionUse::Preview).map_err(|error| match error.as_str() {
                "dictionary maintenance busy" => PreviewError::Busy,
                "invalid options document" | "unsupported host API version" => {
                    PreviewError::Options(format!("the runtime options are not usable: {error}"))
                }
                _ => PreviewError::Unavailable(format!("cannot open the dictionaries: {error}")),
            })?;
        let handle = created["session"]
            .as_u64()
            .ok_or_else(|| PreviewError::Unavailable("the session has no handle".into()))?;
        let session = Destroy(handle);
        type_keys(session.0, keys, caret, limit)
    }))
    .unwrap_or_else(|_| Err(PreviewError::Unavailable("internal runtime failure".into())))
}

/// 本平台的宿主建会话时在运行时选项之外另外请求的能力：三个桌面宿主都请求 `phrase_preedit`（macOS `InputController.mm` 的 `MSIMESessionOptions`、Linux `ClientEngine.cpp` 和 `FcitxEngine.cpp`、Windows `PhrasePreeditOptions.h`），macOS 和 Linux 还请求 `quick_phrase_candidates`，让快捷短语编码混进普通候选（`HostCapabilities::quick_phrase_candidates` 记着哪些宿主这样做）。少了后者，快捷短语就不会出现在候选里，和打字时不一样。
fn with_host_requests(document: &mut Value) {
    let Some(fields) = document.as_object_mut() else {
        return;
    };
    let capabilities = HostCapabilities::for_platform(ffi::host::compiled_platform());
    fields.insert("phrase_preedit".into(), Value::Bool(true));
    if capabilities.quick_phrase_candidates {
        fields.insert("quick_phrase_candidates".into(), Value::Bool(true));
    }
}

/// 用完销毁会话，出错返回时也一样。
struct Destroy(u64);

impl Drop for Destroy {
    fn drop(&mut self) {
        let _ = crate::ffi::lifecycle::destroy_session(self.0);
    }
}

fn failed(error: String) -> PreviewError {
    PreviewError::Unavailable(error)
}

fn type_keys(
    handle: u64,
    keys: &str,
    caret: usize,
    limit: usize,
) -> Result<CandidatePreview, PreviewError> {
    // 没聚焦的运行时丢掉每一个键；宿主在输入框获得焦点时也先这样做。
    focus_session(handle, true).map_err(failed)?;
    let mut committed = Vec::new();
    let mut view = Value::Null;
    for (index, value) in keys.bytes().enumerate() {
        let mut transition = dispatch_value(
            handle,
            Action::Character {
                value,
                shift: value.is_ascii_uppercase(),
            },
            None,
        )
        .map_err(failed)?;
        if transition["handled"] != true {
            return Err(PreviewError::Input(format!(
                "the input method does not take {} (key {}) here; typing it would go to the application as it is",
                char::from(value),
                index + 1
            )));
        }
        if let Some(text) = transition["commit"].as_str() {
            committed.push(text.to_owned());
        }
        view = transition["view"].take();
    }
    // 光标从末尾往回移；横排候选、高亮在首选时左方向键编辑输入串，和宿主收到 ← 时一样。
    let back = keys.len() - caret;
    let editing = view["editing_text"].as_str().unwrap_or_default().len();
    let at_end = view["caret_position"].as_u64().unwrap_or_default() as usize;
    if back > at_end {
        return Err(PreviewError::Input(format!(
            "the caret at {caret} is among keys that were already committed"
        )));
    }
    for _ in 0..back {
        let mut transition =
            dispatch_value(handle, Action::Command(Command::MoveLeft), None).map_err(failed)?;
        view = transition["view"].take();
    }
    let editing_caret = view["caret_position"].as_u64().unwrap_or_default() as usize;
    if editing_caret != at_end - back {
        return Err(PreviewError::Input(format!(
            "the input method cannot put the caret at {caret} in this scheme"
        )));
    }
    debug_assert!(editing_caret <= editing);
    with_session(handle, |session| {
        crate::ffi::settled::settle_now(session)?;
        let view = session.runtime.view();
        let traditional = session.applied.traditional_chinese_output && view.script_conversion;
        let variant = session.applied.traditional_chinese_variant;
        let snapshot = session.runtime.all_candidates();
        let candidates = snapshot
            .candidates
            .into_iter()
            // 在线候选的占位行等着宿主取回答案，这里不去取，列出来只是一行空占位。
            .filter(|candidate| {
                !msime_engine::CandidateSource::from_u8(candidate.source).is_some_and(|source| {
                    matches!(
                        source,
                        msime_engine::CandidateSource::CloudPending
                            | msime_engine::CandidateSource::AiPending
                    )
                })
            })
            .take(limit)
            .map(|candidate| PreviewCandidate {
                text: if traditional {
                    msime_client_core::chinese_conversion::convert_to_traditional(
                        &candidate.text,
                        variant,
                    )
                } else {
                    candidate.text
                },
                code: candidate.code,
                source: source_name(candidate.source),
                annotation: candidate.annotation,
                corrected: candidate.corrected,
            })
            .collect();
        Ok(CandidatePreview {
            keys: keys.to_owned(),
            caret,
            preedit: view.preedit,
            editing_text: view.editing_text,
            editing_caret,
            committed,
            page_size: view.page_size,
            candidates,
        })
    })
    .map_err(failed)
}

#[cfg(test)]
mod tests {
    use super::*;
    use msime_client_core::preferences::{DefaultImeMode, Preferences};

    /// 合成词库：你、好、书三个单字，「你好」「拟好」两个词，和编码 `dh` 的快捷短语「电话」。
    fn host_options(root: &std::path::Path, preferences: Preferences) -> Value {
        let path = |name: &str| {
            let path = root.join(name);
            std::fs::create_dir_all(&path).unwrap();
            path
        };
        let fixture = "CREATE TABLE tbl_1_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);
                       INSERT INTO tbl_1_n VALUES('ni','n','你',100);
                       CREATE TABLE tbl_1_h(key TEXT,jp TEXT,value TEXT,weight INTEGER);
                       INSERT INTO tbl_1_h VALUES('hao','h','好',100);
                       CREATE TABLE tbl_1_s(key TEXT,jp TEXT,value TEXT,weight INTEGER);
                       INSERT INTO tbl_1_s VALUES('shu','s','书',100);
                       CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);
                       INSERT INTO tbl_2_n VALUES('ni''hao','nh','你好',1000),('ni''hao','nh','拟好',80);
                       CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);
                       CREATE TABLE quick_parases(key TEXT,value TEXT,weight INTEGER);
                       CREATE INDEX idx_quick_parases_key_weight ON quick_parases(key,weight DESC);
                       INSERT INTO quick_parases VALUES('dh','电话',10);";
        let resources = path("resources");
        let dictionaries = path("dictionaries");
        for directory in [&resources, &dictionaries] {
            rusqlite::Connection::open(directory.join("msime-pinyin.db"))
                .unwrap()
                .execute_batch(fixture)
                .unwrap();
        }
        json!({
            "api_version": 1,
            "resources": resources,
            "user_data": path("user"),
            "cache": path("cache"),
            "dictionaries": dictionaries,
            "preferences": preferences,
            "preferences_directory": root,
        })
    }

    fn chinese() -> Preferences {
        Preferences {
            default_ime_mode: DefaultImeMode::Chinese,
            ..Preferences::default()
        }
    }

    fn texts(preview: &CandidatePreview) -> Vec<&str> {
        preview
            .candidates
            .iter()
            .map(|candidate| candidate.text.as_str())
            .collect()
    }

    #[test]
    fn typing_keys_lists_the_candidates_the_session_offers() {
        let directory = tempfile::tempdir().unwrap();
        let options = host_options(directory.path(), chinese());
        let preview = preview_candidates(&options, "nihao", 5, 50).unwrap();
        assert_eq!(preview.keys, "nihao");
        assert_eq!(preview.caret, 5);
        assert_eq!(preview.editing_text, "nihao");
        assert_eq!(preview.editing_caret, 5);
        assert!(preview.committed.is_empty());
        assert_eq!(texts(&preview)[..2], ["你好", "拟好"]);
        assert_eq!(preview.candidates[0].source, "dictionary");
        assert_eq!(preview.candidates[0].code, "ni'hao");
        assert_eq!(
            preview.page_size,
            usize::from(chinese().candidate_page_size)
        );
        assert!(!preview.preedit.is_empty());
        // `limit` 截断整张表。
        let first = preview_candidates(&options, "nihao", 5, 1).unwrap();
        assert_eq!(texts(&first), ["你好"]);
    }

    #[test]
    fn the_caret_decides_which_part_of_the_keys_is_converted() {
        let directory = tempfile::tempdir().unwrap();
        let options = host_options(directory.path(), chinese());
        let preview = preview_candidates(&options, "nihao", 2, 50).unwrap();
        assert_eq!(preview.caret, 2);
        assert_eq!(preview.editing_text, "nihao");
        assert_eq!(preview.editing_caret, 2);
        assert_eq!(texts(&preview).first(), Some(&"你"));
        assert!(!texts(&preview).contains(&"你好"), "{:?}", texts(&preview));
    }

    #[test]
    fn traditional_output_converts_the_candidates_as_the_hosts_do() {
        let directory = tempfile::tempdir().unwrap();
        let options = host_options(
            directory.path(),
            Preferences {
                traditional_chinese_output: true,
                ..chinese()
            },
        );
        let preview = preview_candidates(&options, "shu", 3, 50).unwrap();
        assert_eq!(texts(&preview).first(), Some(&"書"));
    }

    #[test]
    fn a_preview_that_commits_learns_nothing_and_counts_nothing() {
        let directory = tempfile::tempdir().unwrap();
        let options = host_options(directory.path(), chinese());
        // 数字键 2 选第二个候选，和打字时一样上屏；默认的调频模式下真打字会把它往前调。
        for _ in 0..3 {
            let preview = preview_candidates(&options, "nihao2", 6, 50).unwrap();
            assert_eq!(preview.committed, ["拟好"]);
            assert!(preview.candidates.is_empty());
        }
        let preview = preview_candidates(&options, "nihao", 5, 50).unwrap();
        assert_eq!(texts(&preview)[..2], ["你好", "拟好"]);
        let statistics = msime_client_services::typing_statistics::TypingStatisticsStore::new(
            directory.path().join("user"),
        )
        .load()
        .unwrap();
        assert_eq!(statistics.selections, Default::default());
        // 设置页据 loaded.json 判断新下载的词库代次是否已被输入法载入，查候选不能替输入法报告。
        std::fs::create_dir_all(directory.path().join("dictionary-updates")).unwrap();
        preview_candidates(&options, "nihao", 5, 50).unwrap();
        assert!(!directory
            .path()
            .join("dictionary-updates/loaded.json")
            .exists());
    }

    #[test]
    fn quick_phrases_join_the_candidates_where_the_host_asks_for_them() {
        let directory = tempfile::tempdir().unwrap();
        let mut preferences = chinese();
        preferences.mixed_input.quick_phrase = true;
        let options = host_options(directory.path(), preferences);
        let preview = preview_candidates(&options, "dh", 2, 50).unwrap();
        let requested =
            HostCapabilities::for_platform(ffi::host::compiled_platform()).quick_phrase_candidates;
        assert_eq!(
            texts(&preview).contains(&"电话"),
            requested,
            "{:?}",
            texts(&preview)
        );
        if requested {
            let phrase = preview
                .candidates
                .iter()
                .find(|candidate| candidate.text == "电话")
                .unwrap();
            assert_eq!(phrase.source, "quick_phrase");
        }
    }

    #[test]
    fn bad_keys_and_carets_are_refused_before_typing() {
        let directory = tempfile::tempdir().unwrap();
        let options = host_options(directory.path(), chinese());
        for (keys, caret) in [("", 0), ("ni hao", 6), ("nihao", 6), ("你好", 2)] {
            assert!(
                matches!(
                    preview_candidates(&options, keys, caret, 50),
                    Err(PreviewError::Input(_))
                ),
                "{keys:?} {caret}"
            );
        }
        let long = "a".repeat(MAX_PREVIEW_KEYS + 1);
        assert!(matches!(
            preview_candidates(&options, &long, long.len(), 50),
            Err(PreviewError::Input(_))
        ));
        for limit in [0, MAX_PREVIEW_CANDIDATES + 1] {
            assert!(matches!(
                preview_candidates(&options, "ni", 2, limit),
                Err(PreviewError::Input(_))
            ));
        }
        // 没在组字时数字键不归输入法，打字时它会原样进应用。
        let refused = preview_candidates(&options, "1", 1, 50).unwrap_err();
        assert!(
            matches!(&refused, PreviewError::Input(reason) if reason.contains("key 1")),
            "{refused:?}"
        );
        // 光标落在已经上屏的键里。
        assert!(matches!(
            preview_candidates(&options, "nihao2ni", 3, 50),
            Err(PreviewError::Input(_))
        ));
    }

    #[test]
    fn dictionary_maintenance_and_broken_options_are_told_apart() {
        let directory = tempfile::tempdir().unwrap();
        let options = host_options(directory.path(), chinese());
        let maintenance =
            msime_client_services::dictionary::access::DictionaryAccess::try_maintenance(
                &directory.path().join("user"),
                &directory.path().join("dictionaries"),
            )
            .unwrap()
            .unwrap();
        assert_eq!(
            preview_candidates(&options, "nihao", 5, 50),
            Err(PreviewError::Busy)
        );
        drop(maintenance);
        assert!(preview_candidates(&options, "nihao", 5, 50).is_ok());

        assert!(matches!(
            preview_candidates(&json!({ "api_version": 1 }), "nihao", 5, 50),
            Err(PreviewError::Options(_))
        ));
        let mut missing = options.clone();
        missing["dictionaries"] = json!(directory.path().join("nowhere"));
        assert!(matches!(
            preview_candidates(&missing, "nihao", 5, 50),
            Err(PreviewError::Unavailable(_))
        ));
    }

    #[test]
    fn every_source_has_a_name() {
        for source in 0..=18 {
            if msime_engine::CandidateSource::from_u8(source).is_some() {
                assert_ne!(source_name(source), "other", "{source}");
            }
        }
        assert_eq!(source_name(200), "other");
    }
}
