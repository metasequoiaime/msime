//! ABI 测试：经 C 函数本身驱动，夹具与 C 示例共用 `tests/fixture` 的建表语句。

use std::ffi::{c_char, CStr};
use std::path::{Path, PathBuf};
use std::ptr;

use rusqlite::Connection;
use tempfile::TempDir;

use crate::abi::{
    msime_engine_candidate, msime_engine_paths, msime_engine_result, msime_engine_state,
    msime_engine_str, Failure,
};
use crate::session::msime_engine_session;
use crate::*;

const PINYIN_SQL: &str = include_str!("../tests/fixture/msime-pinyin.sql");
const ENGLISH_SQL: &str = include_str!("../tests/fixture/msime-english.sql");

/// 三个互不重叠的根目录，资源目录里是夹具词库。
struct Fixture {
    _root: TempDir,
    resources: String,
    user_data: String,
    cache: String,
    dictionaries: String,
}

impl Fixture {
    fn new() -> Self {
        let root = tempfile::tempdir().expect("temporary directory");
        let resources = root.path().join("resources");
        std::fs::create_dir_all(&resources).expect("resources");
        for (name, sql) in [
            ("msime-pinyin.db", PINYIN_SQL),
            ("msime-english.db", ENGLISH_SQL),
        ] {
            Connection::open(resources.join(name))
                .and_then(|connection| connection.execute_batch(sql))
                .expect(name);
        }
        let text = |path: PathBuf| path.to_str().expect("UTF-8 temporary path").to_owned();
        let mut fixture = Self {
            resources: text(resources),
            user_data: text(root.path().join("user")),
            cache: text(root.path().join("cache")),
            dictionaries: String::new(),
            _root: root,
        };
        let mut directory: *mut c_char = ptr::null_mut();
        let id = "fixture";
        // SAFETY: 路径结构和输出参数都在这里活着。
        let status = unsafe {
            msime_engine_prepare_runtime(
                &fixture.paths(),
                id.as_ptr().cast(),
                id.len(),
                &mut directory,
            )
        };
        assert_eq!(status, MSIME_ENGINE_OK, "{}", last_error());
        fixture.dictionaries = take_string(directory);
        fixture
    }

    fn paths(&self) -> msime_engine_paths {
        msime_engine_paths {
            struct_size: size_of::<msime_engine_paths>() as u32,
            resources: borrowed(&self.resources),
            user_data: borrowed(&self.user_data),
            cache: borrowed(&self.cache),
            dictionaries: borrowed(&self.dictionaries),
        }
    }

    fn options(&self) -> String {
        serde_json::json!({
            "version": 1,
            "paths": {
                "resources": self.resources,
                "user_data": self.user_data,
                "cache": self.cache,
                "dictionaries": self.dictionaries,
            },
            "scheme": "quanpin",
        })
        .to_string()
    }

    fn session(&self) -> *mut msime_engine_session {
        new_session(&self.options()).expect("session")
    }
}

fn borrowed(text: &str) -> msime_engine_str {
    msime_engine_str {
        ptr: text.as_ptr().cast(),
        len: text.len(),
    }
}

fn new_session(options: &str) -> Result<*mut msime_engine_session, i32> {
    let mut session = ptr::null_mut();
    // SAFETY: 选项字符串和输出参数都在这里活着。
    match unsafe { msime_engine_session_new(options.as_ptr().cast(), options.len(), &mut session) }
    {
        MSIME_ENGINE_OK => Ok(session),
        status => {
            assert!(session.is_null());
            Err(status)
        }
    }
}

fn free(session: *mut msime_engine_session) {
    // SAFETY: 句柄来自 `new_session`，只释放一次。
    assert_eq!(
        unsafe { msime_engine_session_free(session) },
        MSIME_ENGINE_OK
    );
}

fn last_error() -> String {
    // SAFETY: 库保证返回 NUL 结尾的字符串，到本线程下一次调用前有效。
    unsafe { CStr::from_ptr(msime_engine_last_error()) }
        .to_string_lossy()
        .into_owned()
}

fn take_string(text: *mut c_char) -> String {
    assert!(!text.is_null());
    // SAFETY: 库交出的 NUL 结尾字符串，读完就释放。
    let owned = unsafe { CStr::from_ptr(text) }
        .to_str()
        .expect("UTF-8")
        .to_owned();
    // SAFETY: 同上，只释放一次。
    unsafe { msime_engine_string_free(text) };
    owned
}

/// 借来的文本；只在下一次改变状态的调用之前读。
fn text(value: msime_engine_str) -> String {
    assert!(!value.ptr.is_null());
    // SAFETY: 库借出的 `len` 个字节，调用方在有效期内读。
    let bytes = unsafe { std::slice::from_raw_parts(value.ptr.cast::<u8>(), value.len) };
    std::str::from_utf8(bytes).expect("UTF-8").to_owned()
}

fn empty_result() -> msime_engine_result {
    msime_engine_result {
        struct_size: size_of::<msime_engine_result>() as u32,
        handled: 0,
        has_commit: 0,
        diagnostic_code: 0,
        revision: 0,
        commit: msime_engine_str::EMPTY,
        diagnostic: msime_engine_str::EMPTY,
    }
}

fn empty_state() -> msime_engine_state {
    msime_engine_state {
        struct_size: size_of::<msime_engine_state>() as u32,
        scheme: 0,
        revision: 0,
        candidate_count: 0,
        flags: 0,
        local_mode: 0,
        caret_byte: 0,
        conversion_focus_start_scalar: 0,
        conversion_focus_end_scalar: 0,
        preedit: msime_engine_str::EMPTY,
        editing_text: msime_engine_str::EMPTY,
        conversion: msime_engine_str::EMPTY,
        spelling_symbols: msime_engine_str::EMPTY,
    }
}

fn empty_candidate() -> msime_engine_candidate {
    msime_engine_candidate {
        struct_size: size_of::<msime_engine_candidate>() as u32,
        index: 0,
        source: 0,
        scheme: 0,
        flags: 0,
        fixed_position: 0,
        word: msime_engine_str::EMPTY,
        code: msime_engine_str::EMPTY,
        annotation: msime_engine_str::EMPTY,
        corrected_from: msime_engine_str::EMPTY,
    }
}

fn type_keys(session: *mut msime_engine_session, keys: &str) {
    for key in keys.bytes() {
        let mut result = empty_result();
        // SAFETY: 句柄有效，结果结构按自己的大小声明。
        let status = unsafe { msime_engine_key_char(session, key, 0, &mut result) };
        assert_eq!(status, MSIME_ENGINE_OK, "{}", last_error());
        assert_eq!(result.handled, 1, "{:?} was not handled", key as char);
    }
}

fn state(session: *mut msime_engine_session) -> msime_engine_state {
    let mut state = empty_state();
    // SAFETY: 句柄有效。
    assert_eq!(
        unsafe { msime_engine_state_get(session, &mut state) },
        MSIME_ENGINE_OK,
        "{}",
        last_error()
    );
    state
}

fn candidate(
    session: *mut msime_engine_session,
    revision: u64,
    index: u32,
) -> Result<msime_engine_candidate, i32> {
    let mut candidate = empty_candidate();
    // SAFETY: 句柄有效。
    match unsafe { msime_engine_candidate_get(session, revision, index, &mut candidate) } {
        MSIME_ENGINE_OK => Ok(candidate),
        status => Err(status),
    }
}

fn words(session: *mut msime_engine_session) -> Vec<String> {
    let current = state(session);
    (0..current.candidate_count)
        .map(|index| {
            text(
                candidate(session, current.revision, index)
                    .expect("candidate")
                    .word,
            )
        })
        .collect()
}

fn user_dictionary(fixture: &Fixture) -> serde_json::Value {
    let mut json = ptr::null_mut();
    // SAFETY: 路径结构和输出参数都在这里活着。
    let status = unsafe { msime_engine_user_dict_list(&fixture.paths(), 0, 0, 0, &mut json) };
    assert_eq!(status, MSIME_ENGINE_OK, "{}", last_error());
    serde_json::from_str(&take_string(json)).expect("list JSON")
}

/// `Cargo.toml` 里某一节的 `version = "..."`。
fn manifest_version(section: &str) -> &'static str {
    let manifest = include_str!("../Cargo.toml");
    manifest
        .split(section)
        .nth(1)
        .and_then(|body| body.split("\n[").next())
        .and_then(|body| {
            body.lines()
                .find_map(|line| line.strip_prefix("version = \""))
        })
        .and_then(|rest| rest.strip_suffix('"'))
        .unwrap_or_else(|| panic!("{section} has no version"))
}

/// README 的「快速上手」照着 GitHub Releases 上的归档名写，这些名字带着 crate 版本；README 随包进 crates.io，发出去就改不了，所以升版本时漏改它，这里就要失败（0.1.1 的包里写的还是 0.1.0 的归档名）。示例打印的版本行同样跟着 crate 版本。
#[test]
fn readme_names_this_version() {
    let readme = include_str!("../README.md");
    let version = env!("CARGO_PKG_VERSION");
    let prefix = "msime-engine-capi-";
    let mut names = 0;
    for (start, _) in readme.match_indices(prefix) {
        let rest = &readme[start + prefix.len()..];
        let Some(named) = rest.split('-').next() else {
            continue;
        };
        if named.starts_with(|c: char| c.is_ascii_digit()) {
            assert_eq!(
                named, version,
                "README names {prefix}{named}-…, the crate is {version}"
            );
            names += 1;
        }
    }
    assert!(names > 0, "README no longer names any release archive");
    assert!(
        readme.contains(&format!("msime-engine {version}, ABI ")),
        "README's sample output does not print msime-engine {version}"
    );
    // 数据包按锁定的词库发布命名（msime-engine-data-quanpin-<词库版本>），换词库时同样不能漏改。
    let lock: serde_json::Value = serde_json::from_str(include_str!(
        "../../../resources/desktop-dictionary.lock.json"
    ))
    .expect("the dictionary lock is JSON");
    let url = lock["artifacts"][0]["url"]
        .as_str()
        .expect("an artifact URL");
    let dictionary = url
        .split("/dict-v")
        .nth(1)
        .and_then(|rest| rest.split('/').next())
        .expect("the URL names a dict-v release");
    let pack = "msime-engine-data-quanpin-";
    let mut packs = 0;
    for (start, _) in readme.match_indices(pack) {
        let named = readme[start + pack.len()..]
            .split(|c: char| !(c.is_ascii_digit() || c == '.'))
            .next()
            .unwrap_or_default()
            .trim_end_matches('.');
        if named.starts_with(|c: char| c.is_ascii_digit()) {
            assert_eq!(
                named, dictionary,
                "README names {pack}{named}, the dictionary lock is dict-v{dictionary}"
            );
            packs += 1;
        }
    }
    assert!(packs > 0, "README no longer names the data pack");
}

#[test]
fn library_version_follows_the_abi() {
    let abi = format!("{MSIME_ENGINE_ABI_MAJOR}.{MSIME_ENGINE_ABI_MINOR}.0");
    assert_eq!(manifest_version("[package.metadata.capi.library]"), abi);
    assert_eq!(manifest_version("[package.metadata.capi.pkg_config]"), abi);
    assert_eq!(
        msime_engine_abi_version(),
        (MSIME_ENGINE_ABI_MAJOR << 16) | MSIME_ENGINE_ABI_MINOR
    );
}

#[test]
fn version_is_a_static_c_string() {
    // SAFETY: 静态的 NUL 结尾字符串。
    let version = unsafe { CStr::from_ptr(msime_engine_version()) };
    assert_eq!(version.to_str(), Ok(env!("CARGO_PKG_VERSION")));
}

#[test]
fn null_arguments_are_reported_without_touching_anything() {
    let mut result = empty_result();
    // SAFETY: 测的就是空指针。
    unsafe {
        assert_eq!(
            msime_engine_key_char(ptr::null_mut(), b'a', 0, &mut result),
            MSIME_ENGINE_NULL
        );
        assert!(last_error().contains("session"));
        assert_eq!(
            msime_engine_session_new(ptr::null(), 0, ptr::null_mut()),
            MSIME_ENGINE_NULL
        );
        assert_eq!(msime_engine_reset_learning(ptr::null()), MSIME_ENGINE_NULL);
        assert_eq!(msime_engine_session_free(ptr::null_mut()), MSIME_ENGINE_OK);
        msime_engine_string_free(ptr::null_mut());
    }
    // 成功的调用清掉上一条说明。
    assert_eq!(msime_engine_close_databases(), MSIME_ENGINE_OK);
    assert_eq!(last_error(), "");
}

#[test]
fn options_reject_unknown_keys_versions_and_values() {
    let fixture = Fixture::new();
    let mut options: serde_json::Value = serde_json::from_str(&fixture.options()).unwrap();
    for (key, value, needle) in [
        ("version", serde_json::json!(2), "version 2"),
        (
            "sentence_alternatives",
            serde_json::json!(true),
            "unknown field",
        ),
        ("scheme", serde_json::json!("dvorak"), "unknown variant"),
        ("first_page_rows", serde_json::json!(0), "first_page_rows"),
    ] {
        let mut changed = options.clone();
        changed[key] = value;
        assert_eq!(
            new_session(&changed.to_string()),
            Err(MSIME_ENGINE_INVALID_ARGUMENT),
            "{key}"
        );
        assert!(last_error().contains(needle), "{key}: {}", last_error());
    }
    options["paths"]["cache"] = serde_json::json!("relative/cache");
    assert_eq!(
        new_session(&options.to_string()),
        Err(MSIME_ENGINE_INVALID_ARGUMENT)
    );
    assert_eq!(new_session("{"), Err(MSIME_ENGINE_INVALID_ARGUMENT));
}

/// C 示例做的事：打一句全拼、核对修订号、选词上屏、冲刷学习，之后个人词典里有这句。
#[test]
fn types_a_sentence_selects_it_and_learns_it() {
    let fixture = Fixture::new();
    let session = fixture.session();
    type_keys(session, "nihaoshijie");
    let current = state(session);
    assert_eq!(text(current.editing_text), "nihaoshijie");
    // 给用户看的组字按音节分开，和 msime 自己的宿主一样。
    assert_eq!(text(current.preedit), "ni'hao'shi'jie");
    assert_eq!(current.caret_byte, 11);
    assert_eq!(current.scheme, MSIME_ENGINE_SCHEME_QUANPIN);
    assert_eq!(current.local_mode, MSIME_ENGINE_LOCAL_MODE_NONE);
    assert!(current.candidate_count > 0);
    let first = candidate(session, current.revision, 0).expect("first candidate");
    assert_eq!(text(first.word), "你好世界");
    assert_eq!(first.index, 0);
    assert_ne!(first.flags & MSIME_ENGINE_CANDIDATE_ANSWERS_KEY, 0);
    assert_eq!(
        candidate(session, current.revision, current.candidate_count).err(),
        Some(MSIME_ENGINE_INVALID_ARGUMENT)
    );
    assert_eq!(
        candidate(session, current.revision + 1, 0).err(),
        Some(MSIME_ENGINE_STALE)
    );

    let mut result = empty_result();
    // SAFETY: 句柄有效。
    unsafe {
        assert_eq!(
            msime_engine_select(session, current.revision + 1, 0, &mut result),
            MSIME_ENGINE_STALE
        );
        assert!(last_error().contains("stale"));
        // 被拒绝的选择什么也没做。
        assert_eq!(state(session).revision, current.revision);
        assert_eq!(
            msime_engine_select(session, current.revision, 0, &mut result),
            MSIME_ENGINE_OK
        );
    }
    assert_eq!(result.handled, 1);
    assert_eq!(result.has_commit, 1);
    assert_eq!(text(result.commit), "你好世界");
    assert_eq!(result.diagnostic_code, 0, "{}", text(result.diagnostic));
    assert_ne!(result.revision, current.revision);
    // 只读调用不让借出的上屏文字失效。
    let after = state(session);
    assert_eq!(after.revision, result.revision);
    assert_eq!(text(after.editing_text), "");
    assert_eq!(text(result.commit), "你好世界");

    assert_eq!(msime_engine_flush_learning(0), MSIME_ENGINE_OK);
    free(session);
    let listed = user_dictionary(&fixture);
    assert!(
        listed["entries"]
            .as_array()
            .unwrap()
            .iter()
            .any(|entry| entry["value"] == "你好世界"),
        "{listed}"
    );
}

#[test]
fn state_json_is_owned_by_the_caller() {
    let fixture = Fixture::new();
    let session = fixture.session();
    type_keys(session, "nihao");
    let mut json = ptr::null_mut();
    // SAFETY: 句柄有效。
    assert_eq!(
        unsafe { msime_engine_state_json(session, &mut json) },
        MSIME_ENGINE_OK
    );
    let value: serde_json::Value = serde_json::from_str(&take_string(json)).unwrap();
    assert_eq!(value["editing_text"], "nihao");
    assert_eq!(value["candidates"][0]["word"], "你好");
    free(session);
}

#[test]
fn struct_size_is_negotiated() {
    let fixture = Fixture::new();
    let session = fixture.session();
    type_keys(session, "ni");

    // 比 1.0 的布局还小：拒绝，什么也不写。
    let mut small = empty_state();
    small.struct_size = 8;
    small.revision = 77;
    // SAFETY: 句柄有效，结构体比它声称的大。
    assert_eq!(
        unsafe { msime_engine_state_get(session, &mut small) },
        MSIME_ENGINE_INVALID_ARGUMENT
    );
    assert_eq!(small.revision, 77);

    // 以后的调用方的结构更大：库只写自己知道的部分，其余清零，`struct_size` 改成写入的大小。
    #[repr(C)]
    struct Larger {
        state: msime_engine_state,
        tail: [u8; 24],
    }
    let mut larger = Larger {
        state: empty_state(),
        tail: [0xAA; 24],
    };
    larger.state.struct_size = size_of::<Larger>() as u32;
    // SAFETY: 句柄有效，`Larger` 以 `msime_engine_state` 开头。
    let status = unsafe { msime_engine_state_get(session, ptr::addr_of_mut!(larger).cast()) };
    assert_eq!(status, MSIME_ENGINE_OK);
    assert_eq!(
        larger.state.struct_size as usize,
        size_of::<msime_engine_state>()
    );
    assert_eq!(larger.tail, [0; 24]);
    assert_eq!(text(larger.state.editing_text), "ni");

    // 输入结构同理：比 1.0 小的拒绝。
    let mut paths = fixture.paths();
    paths.struct_size = 4;
    // SAFETY: 结构体比它声称的大。
    assert_eq!(
        unsafe { msime_engine_reset_learning(&paths) },
        MSIME_ENGINE_INVALID_ARGUMENT
    );
    free(session);
}

/// `struct_size` 太小的结果结构体在改变会话之前就被拒绝：键没有进组字，选择没有上屏，之后照常可用。
#[test]
fn a_too_small_result_is_rejected_before_the_session_changes() {
    let fixture = Fixture::new();
    let session = fixture.session();
    type_keys(session, "nihao");
    let before = state(session);
    let mut small = empty_result();
    small.struct_size = 0;
    // SAFETY: 句柄有效，结构体比它声称的大。
    unsafe {
        assert_eq!(
            msime_engine_key_char(session, b's', 0, &mut small),
            MSIME_ENGINE_INVALID_ARGUMENT
        );
        assert_eq!(
            msime_engine_key_punct(session, b',', &mut small),
            MSIME_ENGINE_INVALID_ARGUMENT
        );
        assert_eq!(
            msime_engine_command(session, MSIME_ENGINE_COMMAND_BACKSPACE, &mut small),
            MSIME_ENGINE_INVALID_ARGUMENT
        );
        assert_eq!(
            msime_engine_select(session, before.revision, 0, &mut small),
            MSIME_ENGINE_INVALID_ARGUMENT
        );
    }
    assert!(last_error().contains("struct_size"), "{}", last_error());
    let after = state(session);
    assert_eq!(text(after.editing_text), "nihao");
    assert_eq!(after.revision, before.revision);
    assert_eq!(small.struct_size, 0);

    let mut result = empty_result();
    // SAFETY: 句柄有效。
    assert_eq!(
        unsafe { msime_engine_select(session, before.revision, 0, &mut result) },
        MSIME_ENGINE_OK
    );
    assert_eq!(text(result.commit), "你好");
    free(session);
}

/// README 按键路由第 4 条：数字没被引擎处理时当选择键，用这次按键交回的修订号选。
#[test]
fn an_unhandled_digit_selects_with_the_revision_it_returned() {
    let fixture = Fixture::new();
    let session = fixture.session();
    type_keys(session, "nihao");
    let before = state(session);
    let rows = words(session);
    assert!(rows.len() > 1, "{rows:?}");
    let mut result = empty_result();
    // SAFETY: 句柄有效。
    unsafe {
        assert_eq!(
            msime_engine_key_char(session, b'2', 0, &mut result),
            MSIME_ENGINE_OK
        );
    }
    assert_eq!(result.handled, 0);
    assert_eq!(result.has_commit, 0);
    // 没处理的键也让修订号变，候选行不变。
    assert_ne!(result.revision, before.revision);
    assert_eq!(words(session), rows);
    // SAFETY: 句柄有效。
    unsafe {
        assert_eq!(
            msime_engine_select(session, before.revision, 1, &mut result),
            MSIME_ENGINE_STALE
        );
        let revision = state(session).revision;
        assert_eq!(
            msime_engine_select(session, revision, 1, &mut result),
            MSIME_ENGINE_OK
        );
    }
    assert_eq!(text(result.commit), rows[1]);
    free(session);
}

/// `msime_engine_prepare_runtime` 不读 `paths.dictionaries`：调用方在准备之前没有代次目录。
#[test]
fn prepare_runtime_ignores_the_dictionaries_field() {
    let fixture = Fixture::new();
    let garbage = [0xFFu8, 0xFE];
    let mut paths = fixture.paths();
    paths.dictionaries = msime_engine_str {
        ptr: garbage.as_ptr().cast(),
        len: garbage.len(),
    };
    let mut directory: *mut c_char = ptr::null_mut();
    let id = "fixture";
    // SAFETY: 路径结构、id 和输出参数都在这里活着。
    let status = unsafe {
        msime_engine_prepare_runtime(&paths, id.as_ptr().cast(), id.len(), &mut directory)
    };
    assert_eq!(status, MSIME_ENGINE_OK, "{}", last_error());
    assert_eq!(take_string(directory), fixture.dictionaries);
    // 带点的 id 不是合法的代次 id。
    let dotted = "dict-v2.0.39";
    // SAFETY: 同上。
    let status = unsafe {
        msime_engine_prepare_runtime(
            &paths,
            dotted.as_ptr().cast(),
            dotted.len(),
            ptr::null_mut(),
        )
    };
    assert_eq!(status, MSIME_ENGINE_INVALID_ARGUMENT);
}

#[test]
fn concurrent_calls_into_one_session_report_busy() {
    let fixture = Fixture::new();
    let session = fixture.session();
    {
        let _held = crate::session::lock_for_test(session);
        let mut result = empty_result();
        // SAFETY: 句柄有效；锁被上面占着。
        unsafe {
            assert_eq!(
                msime_engine_key_char(session, b'n', 0, &mut result),
                MSIME_ENGINE_BUSY
            );
            assert_eq!(msime_engine_session_free(session), MSIME_ENGINE_BUSY);
        }
    }
    type_keys(session, "ni");
    free(session);
}

#[test]
fn a_session_moves_between_threads() {
    struct Handle(*mut msime_engine_session);
    // SAFETY: ABI 允许会话在线程之间移动，只要调用不重叠。
    unsafe impl Send for Handle {}
    let fixture = Fixture::new();
    let handle = Handle(fixture.session());
    let handle = std::thread::spawn(move || {
        let handle = handle;
        type_keys(handle.0, "nihao");
        handle
    })
    .join()
    .unwrap();
    assert_eq!(words(handle.0)[0], "你好");
    free(handle.0);
}

#[test]
fn a_panic_poisons_only_its_session() {
    let fixture = Fixture::new();
    let session = fixture.session();
    let status =
        crate::session::run_for_test(session, || -> Result<(), Failure> { panic!("boom") });
    assert_eq!(status, MSIME_ENGINE_PANIC);
    assert!(last_error().contains("boom"), "{}", last_error());
    let mut state = empty_state();
    // SAFETY: 句柄有效。
    assert_eq!(
        unsafe { msime_engine_state_get(session, &mut state) },
        MSIME_ENGINE_PANIC
    );
    let other = fixture.session();
    type_keys(other, "ni");
    free(session);
    free(other);
}

#[test]
fn maintenance_waits_for_every_session_on_the_paths() {
    let fixture = Fixture::new();
    let session = fixture.session();
    let edit = r#"{"previous":null,"replacement":{"kind":"pinyin","key":"ni'hao","value":"妮好"},"request_id":"add-1"}"#;
    // SAFETY: 路径结构和编辑字符串都在这里活着。
    unsafe {
        assert_eq!(
            msime_engine_user_dict_edit(&fixture.paths(), edit.as_ptr().cast(), edit.len()),
            MSIME_ENGINE_BUSY
        );
        assert_eq!(
            msime_engine_reset_learning(&fixture.paths()),
            MSIME_ENGINE_BUSY
        );
    }
    free(session);
    // SAFETY: 同上。
    unsafe {
        assert_eq!(
            msime_engine_user_dict_edit(&fixture.paths(), edit.as_ptr().cast(), edit.len()),
            MSIME_ENGINE_OK,
            "{}",
            last_error()
        );
    }
    let listed = user_dictionary(&fixture);
    assert_eq!(listed["entries"][0]["value"], "妮好", "{listed}");
    assert_eq!(listed["entries"][0]["kind"], "pinyin");

    // 维护进行时建不了会话；这里用引擎的租约代替一次进行中的维护。
    let paths = PathBuf::from(&fixture.user_data);
    let lease =
        msime_engine::DictionaryAccess::try_maintenance(&paths, Path::new(&fixture.dictionaries))
            .unwrap()
            .expect("maintenance lease");
    assert_eq!(new_session(&fixture.options()), Err(MSIME_ENGINE_BUSY));
    drop(lease);

    // SAFETY: 路径结构活着。
    assert_eq!(
        unsafe { msime_engine_reset_learning(&fixture.paths()) },
        MSIME_ENGINE_OK,
        "{}",
        last_error()
    );
    assert_eq!(user_dictionary(&fixture)["entries"], serde_json::json!([]));
}

#[test]
fn options_and_inputs_are_checked() {
    let fixture = Fixture::new();
    let session = fixture.session();
    let schema = "wubi86";
    // SAFETY: 句柄有效，字符串活着。
    unsafe {
        assert_eq!(
            msime_engine_set_int(session, 999, 1),
            MSIME_ENGINE_UNSUPPORTED
        );
        assert_eq!(
            msime_engine_set_int(session, MSIME_ENGINE_OPTION_HELPCODE_SCHEMA, 1),
            MSIME_ENGINE_INVALID_ARGUMENT
        );
        assert_eq!(
            msime_engine_set_int(session, MSIME_ENGINE_OPTION_CHINESE_PUNCT, 2),
            MSIME_ENGINE_INVALID_ARGUMENT
        );
        assert_eq!(
            msime_engine_set_int(session, MSIME_ENGINE_OPTION_PUNCT_LOCK, 3),
            MSIME_ENGINE_INVALID_ARGUMENT
        );
        assert_eq!(
            msime_engine_set_int(session, MSIME_ENGINE_OPTION_FIRST_PAGE_ROWS, 9),
            MSIME_ENGINE_OK
        );
        assert_eq!(
            msime_engine_set_string(
                session,
                MSIME_ENGINE_OPTION_DEDICATED_ENGLISH,
                schema.as_ptr().cast(),
                schema.len()
            ),
            MSIME_ENGINE_INVALID_ARGUMENT
        );
        assert_eq!(
            msime_engine_set_string(session, 999, schema.as_ptr().cast(), schema.len()),
            MSIME_ENGINE_UNSUPPORTED
        );
        assert_eq!(
            msime_engine_switch_scheme(session, 42),
            MSIME_ENGINE_INVALID_ARGUMENT
        );
        assert_eq!(
            msime_engine_command(session, 99, ptr::null_mut()),
            MSIME_ENGINE_UNSUPPORTED
        );
        assert_eq!(
            msime_engine_key_char(session, 0xE4, 0, ptr::null_mut()),
            MSIME_ENGINE_INVALID_ARGUMENT
        );
        assert_eq!(
            msime_engine_key_char(session, b'a', 0x80, ptr::null_mut()),
            MSIME_ENGINE_UNSUPPORTED
        );
        assert_eq!(
            msime_engine_key_punct(session, b'a', ptr::null_mut()),
            MSIME_ENGINE_INVALID_ARGUMENT
        );
        assert_eq!(msime_engine_flush_learning(0x80), MSIME_ENGINE_UNSUPPORTED);
        assert_eq!(
            msime_engine_preload_model(9, ptr::null(), 0, ptr::null(), 0),
            MSIME_ENGINE_UNSUPPORTED
        );
        let path = "/nonexistent/msime-japanese.dat";
        let garbage = [0u8; 16];
        assert_eq!(
            msime_engine_preload_model(
                MSIME_ENGINE_MODEL_JAPANESE,
                path.as_ptr().cast(),
                path.len(),
                garbage.as_ptr(),
                garbage.len()
            ),
            MSIME_ENGINE_INVALID_ARGUMENT
        );
    }
    free(session);
}

#[test]
fn punctuation_and_commands_route_through_the_session() {
    let fixture = Fixture::new();
    let session = fixture.session();
    type_keys(session, "nihao");
    let mut result = empty_result();
    // SAFETY: 句柄有效。
    unsafe {
        assert_eq!(
            msime_engine_key_punct(session, b',', &mut result),
            MSIME_ENGINE_OK
        );
    }
    assert_eq!(text(result.commit), "你好，");
    type_keys(session, "ni");
    // SAFETY: 同上。
    unsafe {
        assert_eq!(
            msime_engine_command(session, MSIME_ENGINE_COMMAND_BACKSPACE, &mut result),
            MSIME_ENGINE_OK
        );
        assert_eq!(text(state(session).editing_text), "n");
        assert_eq!(
            msime_engine_command(session, MSIME_ENGINE_COMMAND_COMMIT_RAW, &mut result),
            MSIME_ENGINE_OK
        );
    }
    assert_eq!(text(result.commit), "n");
    free(session);
}

#[test]
fn last_error_belongs_to_the_calling_thread() {
    // SAFETY: 空指针就是要测的失败。
    assert_eq!(
        unsafe { msime_engine_reset_learning(ptr::null()) },
        MSIME_ENGINE_NULL
    );
    let other = std::thread::spawn(last_error).join().unwrap();
    assert_eq!(other, "");
    assert!(!last_error().is_empty());
}

/// 只带韩语的数据（空的资源目录）：`prepare_runtime_for` 只为韩语准备，代次里没有拼音库和英文库，会话照样建起来打字；`reset_learning_for` 只清空日志。按全部方案准备同一个目录仍然失败；空集合和不认识的位不合法。
#[test]
fn prepare_runtime_for_needs_only_the_schemes_data() {
    let root = tempfile::tempdir().expect("temporary directory");
    let resources = root.path().join("resources");
    std::fs::create_dir_all(&resources).expect("resources");
    let utf8 = |path: PathBuf| path.to_str().expect("UTF-8 temporary path").to_owned();
    let mut fixture = Fixture {
        resources: utf8(resources),
        user_data: utf8(root.path().join("user")),
        cache: utf8(root.path().join("cache")),
        dictionaries: String::new(),
        _root: root,
    };
    let korean = 1 << MSIME_ENGINE_SCHEME_KOREAN;
    let id = "korean";
    let mut directory: *mut c_char = ptr::null_mut();
    // SAFETY: 路径结构、id 和输出参数都在这里活着。
    let status = unsafe {
        msime_engine_prepare_runtime_for(
            &fixture.paths(),
            id.as_ptr().cast(),
            id.len(),
            korean,
            &mut directory,
        )
    };
    assert_eq!(status, MSIME_ENGINE_OK, "{}", last_error());
    fixture.dictionaries = take_string(directory);
    let generation = Path::new(&fixture.dictionaries);
    assert!(!generation.join("msime-pinyin.db").exists());
    assert!(!generation.join("msime-english.db").exists());

    let options = serde_json::json!({
        "version": 1,
        "paths": {
            "resources": fixture.resources,
            "user_data": fixture.user_data,
            "cache": fixture.cache,
            "dictionaries": fixture.dictionaries,
        },
        "scheme": "korean",
        "enabled_schemes": ["korean"],
    })
    .to_string();
    let session = new_session(&options).expect("Korean session");
    type_keys(session, "dkssud");
    let mut result = empty_result();
    // SAFETY: 句柄有效。
    let status =
        unsafe { msime_engine_command(session, MSIME_ENGINE_COMMAND_COMMIT_RAW, &mut result) };
    assert_eq!(status, MSIME_ENGINE_OK, "{}", last_error());
    assert_eq!(text(result.commit), "녕");
    free(session);

    // SAFETY: 路径结构活着。
    let status = unsafe { msime_engine_reset_learning_for(&fixture.paths(), korean) };
    assert_eq!(status, MSIME_ENGINE_OK, "{}", last_error());
    assert!(!generation.join("msime-english.db").exists());

    for (schemes, expected) in [
        (0, MSIME_ENGINE_INVALID_ARGUMENT),
        (1 << 15, MSIME_ENGINE_INVALID_ARGUMENT),
        (
            korean | 1 << MSIME_ENGINE_SCHEME_QUANPIN,
            MSIME_ENGINE_FAILED,
        ),
    ] {
        let other = "other";
        // SAFETY: 同上。
        let status = unsafe {
            msime_engine_prepare_runtime_for(
                &fixture.paths(),
                other.as_ptr().cast(),
                other.len(),
                schemes,
                ptr::null_mut(),
            )
        };
        assert_eq!(status, expected, "{schemes:#x}: {}", last_error());
    }
    // SAFETY: 同上。
    let status = unsafe {
        msime_engine_prepare_runtime(
            &fixture.paths(),
            id.as_ptr().cast(),
            id.len(),
            ptr::null_mut(),
        )
    };
    assert_ne!(status, MSIME_ENGINE_OK);
    // SAFETY: 同上。
    assert_eq!(
        unsafe { msime_engine_reset_learning_for(&fixture.paths(), 0) },
        MSIME_ENGINE_INVALID_ARGUMENT
    );
}

/// 回车学词：缺省只学专用英文；`english.learn_typed_spellings_on_enter` 打开后拼不成音节的拼音也学成英文词；`COMMIT_RAW_WITHOUT_LEARNING` 上屏同样的原文、不学。
#[test]
fn enter_learning_follows_the_english_options() {
    let fixture = Fixture::new();
    let english_words = || -> Vec<String> {
        user_dictionary(&fixture)["entries"]
            .as_array()
            .expect("entries")
            .iter()
            .filter(|entry| entry["kind"] == "english")
            .map(|entry| entry["key"].as_str().expect("key").to_owned())
            .collect()
    };
    let commit = |session, command| {
        let mut result = empty_result();
        // SAFETY: 句柄有效。
        let status = unsafe { msime_engine_command(session, command, &mut result) };
        assert_eq!(status, MSIME_ENGINE_OK, "{}", last_error());
        text(result.commit)
    };

    let session = fixture.session();
    type_keys(session, "xyz");
    assert_eq!(commit(session, MSIME_ENGINE_COMMAND_COMMIT_RAW), "xyz");
    free(session);
    assert!(english_words().is_empty());

    let mut options: serde_json::Value = serde_json::from_str(&fixture.options()).unwrap();
    options["english"] = serde_json::json!({ "learn_typed_spellings_on_enter": true });
    let session = new_session(&options.to_string()).expect("session");
    type_keys(session, "xyz");
    assert_eq!(commit(session, MSIME_ENGINE_COMMAND_COMMIT_RAW), "xyz");
    type_keys(session, "qwz");
    assert_eq!(
        commit(session, MSIME_ENGINE_COMMAND_COMMIT_RAW_WITHOUT_LEARNING),
        "qwz"
    );
    free(session);
    assert_eq!(english_words(), ["xyz"]);

    options["english"] = serde_json::json!({
        "learn_on_enter": false,
        "learn_typed_spellings_on_enter": true,
    });
    let session = new_session(&options.to_string()).expect("session");
    type_keys(session, "zzq");
    assert_eq!(commit(session, MSIME_ENGINE_COMMAND_COMMIT_RAW), "zzq");
    free(session);
    assert_eq!(english_words(), ["xyz"]);
}

/// 选项 JSON 的调频、混排与本地模式（ABI 1.1）：不认识的键和越界的值被拒绝，`/`、`@` 两个模式不在这里；本地模式的开关决定 Shift 加字母进不进模式。
#[test]
fn options_carry_frequency_expressive_and_local_modes() {
    let fixture = Fixture::new();
    let options: serde_json::Value = serde_json::from_str(&fixture.options()).unwrap();
    for (key, value, needle) in [
        (
            "frequency",
            serde_json::json!({"trigger_count": 0}),
            "frequency.trigger_count",
        ),
        (
            "frequency",
            serde_json::json!({"linear_step": 11}),
            "frequency.linear_step",
        ),
        (
            "frequency",
            serde_json::json!({"mode": "sometimes"}),
            "unknown variant",
        ),
        (
            "local_modes",
            serde_json::json!({"command": true}),
            "unknown field",
        ),
        (
            "local_modes",
            serde_json::json!({"mention": true}),
            "unknown field",
        ),
        (
            "expressive",
            serde_json::json!({"stickers": true}),
            "unknown field",
        ),
    ] {
        let mut changed = options.clone();
        changed[key] = value;
        assert_eq!(
            new_session(&changed.to_string()),
            Err(MSIME_ENGINE_INVALID_ARGUMENT),
            "{changed}"
        );
        assert!(last_error().contains(needle), "{key}: {}", last_error());
    }

    let shift = |session, key| {
        let mut result = empty_result();
        // SAFETY: 句柄有效。
        let status = unsafe {
            msime_engine_key_char(session, key, MSIME_ENGINE_KEY_SHIFT_ONLY, &mut result)
        };
        assert_eq!(status, MSIME_ENGINE_OK, "{}", last_error());
        let mode = state(session).local_mode;
        // SAFETY: 同上。
        let status =
            unsafe { msime_engine_command(session, MSIME_ENGINE_COMMAND_CANCEL, ptr::null_mut()) };
        assert_eq!(status, MSIME_ENGINE_OK, "{}", last_error());
        mode
    };
    let session = fixture.session();
    assert_eq!(shift(session, b'U'), MSIME_ENGINE_LOCAL_MODE_UNICODE);
    assert_eq!(shift(session, b'V'), MSIME_ENGINE_LOCAL_MODE_NONE);
    free(session);

    let mut changed = options.clone();
    changed["local_modes"] = serde_json::json!({"unicode": false, "expression": true});
    changed["expressive"] = serde_json::json!({
        "emoji_candidates": true,
        "kaomoji_candidates": true,
        "symbol_candidates": true,
    });
    changed["frequency"] =
        serde_json::json!({"mode": "linear", "trigger_count": 2, "linear_step": 3});
    let session = new_session(&changed.to_string()).expect("session");
    assert_eq!(shift(session, b'U'), MSIME_ENGINE_LOCAL_MODE_NONE);
    assert_eq!(shift(session, b'V'), MSIME_ENGINE_LOCAL_MODE_EXPRESSION);
    free(session);

    changed["local_modes"] = serde_json::json!({"shift_letter_entry": false, "expression": true});
    let session = new_session(&changed.to_string()).expect("session");
    assert_eq!(shift(session, b'V'), MSIME_ENGINE_LOCAL_MODE_NONE);
    free(session);
}

/// 调频既是建会话的选项，也能在会话里换（ABI 1.1）：缺省不调频，选了靠后的词以后排序不变；换成固定首位之后再选一次，它就排到首位。越界的方式、次数和步长被拒绝。
#[test]
fn frequency_is_a_session_option_and_a_runtime_option() {
    let fixture = Fixture::new();
    let position = |session, word: &str| words(session).iter().position(|row| row == word);
    let pick = |session, word: &str| {
        type_keys(session, "ni");
        let index = position(session, word).expect("the word is a candidate");
        assert!(index > 0, "{word} already leads");
        let revision = state(session).revision;
        let mut result = empty_result();
        // SAFETY: 句柄有效。
        let status = unsafe { msime_engine_select(session, revision, index as u32, &mut result) };
        assert_eq!(status, MSIME_ENGINE_OK, "{}", last_error());
        assert_eq!(text(result.commit), word);
        index
    };
    let session = fixture.session();
    let before = pick(session, "拟");
    type_keys(session, "ni");
    assert_eq!(position(session, "拟"), Some(before));
    // SAFETY: 句柄有效。
    unsafe {
        assert_eq!(
            msime_engine_command(session, MSIME_ENGINE_COMMAND_CANCEL, ptr::null_mut()),
            MSIME_ENGINE_OK
        );
        let revision = state(session).revision;
        assert_eq!(
            msime_engine_set_int(
                session,
                MSIME_ENGINE_OPTION_FREQUENCY_MODE,
                i64::from(MSIME_ENGINE_FREQUENCY_PIN)
            ),
            MSIME_ENGINE_OK,
            "{}",
            last_error()
        );
        assert_ne!(state(session).revision, revision);
    }
    pick(session, "拟");
    type_keys(session, "ni");
    assert_eq!(position(session, "拟"), Some(0));
    let mode = "pin";
    // SAFETY: 句柄有效，字符串活着。
    unsafe {
        for (option, value) in [
            (MSIME_ENGINE_OPTION_FREQUENCY_MODE, 5),
            (MSIME_ENGINE_OPTION_FREQUENCY_MODE, -1),
            (MSIME_ENGINE_OPTION_FREQUENCY_TRIGGER_COUNT, 0),
            (MSIME_ENGINE_OPTION_FREQUENCY_LINEAR_STEP, 11),
            (MSIME_ENGINE_OPTION_WUBI_MIXED_PINYIN, 2),
        ] {
            assert_eq!(
                msime_engine_set_int(session, option, value),
                MSIME_ENGINE_INVALID_ARGUMENT,
                "{option} = {value}"
            );
        }
        for (option, value) in [
            (MSIME_ENGINE_OPTION_FREQUENCY_TRIGGER_COUNT, 10),
            (MSIME_ENGINE_OPTION_FREQUENCY_LINEAR_STEP, 1),
            (MSIME_ENGINE_OPTION_WUBI_MIXED_PINYIN, 1),
        ] {
            assert_eq!(
                msime_engine_set_int(session, option, value),
                MSIME_ENGINE_OK,
                "{option} = {value}: {}",
                last_error()
            );
        }
        assert_eq!(
            msime_engine_set_string(
                session,
                MSIME_ENGINE_OPTION_FREQUENCY_MODE,
                mode.as_ptr().cast(),
                mode.len()
            ),
            MSIME_ENGINE_INVALID_ARGUMENT
        );
    }
    free(session);

    // 选项 JSON 里的同一个方式：在新的数据上选「泥」一次就排到首位。
    let fixture = Fixture::new();
    let mut options: serde_json::Value = serde_json::from_str(&fixture.options()).unwrap();
    options["frequency"] = serde_json::json!({"mode": "pin"});
    let session = new_session(&options.to_string()).expect("session");
    pick(session, "泥");
    type_keys(session, "ni");
    assert_eq!(position(session, "泥"), Some(0));
    free(session);
}

/// 数字选词键（ABI 1.1）：有组字时选第一页的第 `digit - 1` 行，没有组字时不处理；只收 ASCII 数字。
#[test]
fn candidate_key_selects_on_the_first_page() {
    let fixture = Fixture::new();
    let session = fixture.session();
    type_keys(session, "nihao");
    let rows = words(session);
    assert!(rows.len() > 1, "{rows:?}");
    let mut result = empty_result();
    // SAFETY: 句柄有效。
    unsafe {
        assert_eq!(
            msime_engine_candidate_key(session, b'a', &mut result),
            MSIME_ENGINE_INVALID_ARGUMENT
        );
        assert_eq!(
            msime_engine_candidate_key(session, b'2', &mut result),
            MSIME_ENGINE_OK,
            "{}",
            last_error()
        );
    }
    assert_eq!(result.handled, 1);
    assert_eq!(text(result.commit), rows[1]);
    assert_eq!(result.revision, state(session).revision);
    // SAFETY: 同上。
    unsafe {
        assert_eq!(
            msime_engine_candidate_key(session, b'1', &mut result),
            MSIME_ENGINE_OK
        );
    }
    assert_eq!(result.handled, 0);
    assert_eq!(result.has_commit, 0);
    free(session);
}

/// 个人词条的检查（ABI 1.1）：合法的交回规范化后的词条，不合法的给出引擎的原因；不读写任何文件。
#[test]
fn user_dict_validate_normalizes_and_explains() {
    let validate = |entry: &str| -> Result<serde_json::Value, (i32, String)> {
        let mut json = ptr::null_mut();
        // SAFETY: 字符串和输出参数都在这里活着。
        let status = unsafe {
            msime_engine_user_dict_validate(entry.as_ptr().cast(), entry.len(), &mut json)
        };
        if status != MSIME_ENGINE_OK {
            assert!(json.is_null());
            return Err((status, last_error()));
        }
        Ok(serde_json::from_str(&take_string(json)).expect("entry JSON"))
    };
    assert_eq!(
        validate(r#"{"kind":"pinyin","key":"NI HAO","value":"你好"}"#),
        Ok(
            serde_json::json!({"kind": "pinyin", "key": "ni'hao", "value": "你好", "weight": 100000})
        )
    );
    assert_eq!(
        validate(r#"{"kind":"english","key":"Dont","value":"don't","weight":7}"#),
        Ok(serde_json::json!({"kind": "english", "key": "dont", "value": "don't", "weight": 7}))
    );
    for (entry, needle) in [
        (
            r#"{"kind":"pinyin","key":"nh","value":"你好"}"#,
            "complete pinyin syllables",
        ),
        (
            r#"{"kind":"pinyin","key":"ni","value":"你好"}"#,
            "one pinyin syllable",
        ),
        (
            r#"{"kind":"wubi","key":"wqvbx","value":"你"}"#,
            "Wubi codes",
        ),
        (
            r#"{"kind":"pinyin","key":"ni","value":"你","weight":0}"#,
            "Weight",
        ),
        (r#"{"kind":"pinyin","key":"ni"}"#, "entry:"),
        (r#"{"kind":"emoji","key":"ni","value":"你"}"#, "entry:"),
    ] {
        let (status, error) = validate(entry).expect_err(entry);
        assert_eq!(status, MSIME_ENGINE_INVALID_ARGUMENT, "{entry}");
        assert!(error.contains(needle), "{entry}: {error}");
    }
    let entry = r#"{"kind":"pinyin","key":"ni","value":"你"}"#;
    // SAFETY: 测的就是空的输出参数。
    assert_eq!(
        unsafe {
            msime_engine_user_dict_validate(entry.as_ptr().cast(), entry.len(), ptr::null_mut())
        },
        MSIME_ENGINE_NULL
    );
}

/// 日志回调收集到的行、每行是否按 `message_len` 正好以 NUL 结尾，以及回调里换回调得到的返回码。回调里不断言：`extern "C"` 函数里的 panic 会让整个测试进程中止。
#[derive(Default)]
struct LogLines {
    lines: Vec<String>,
    terminated: Vec<bool>,
    nested: Vec<i32>,
}

unsafe extern "C" fn collect_log_line(
    user_data: *mut std::ffi::c_void,
    message: *const c_char,
    message_len: usize,
) {
    // SAFETY: 注册时交的是活着的 `Mutex<LogLines>`；库交来 `message_len` 个字节，后面跟着 NUL。
    let (lines, bytes, terminated) = unsafe {
        (
            &*user_data.cast::<std::sync::Mutex<LogLines>>(),
            std::slice::from_raw_parts(message.cast::<u8>(), message_len),
            CStr::from_ptr(message).to_bytes().len() == message_len,
        )
    };
    // 回调里不许换回调：库报 BUSY，不会死锁。
    // SAFETY: 测的就是从回调里调进来。
    let nested = unsafe { msime_engine_set_log_callback(None, ptr::null_mut()) };
    let Ok(mut lines) = lines.lock() else {
        return;
    };
    lines
        .lines
        .push(String::from_utf8_lossy(bytes).into_owned());
    lines.terminated.push(terminated);
    lines.nested.push(nested);
}

/// 日志回调（ABI 1.1）：引擎的警告交给回调，不带 `msime: ` 前缀；回调里换回调报 BUSY；换回 NULL 之后不再收到。日志出口是进程级的，并行的测试记的行也可能落进来，所以只找自己的那一行。
#[test]
fn the_log_callback_receives_engine_warnings_until_it_is_removed() {
    let fixture = Fixture::new();
    let collected = std::sync::Mutex::new(LogLines::default());
    let corrupt_journal_and_prepare = || {
        std::fs::write(
            Path::new(&fixture.user_data).join("msime_user.db"),
            b"not a database",
        )
        .expect("journal");
        let id = "fixture";
        // SAFETY: 路径结构和 id 都在这里活着。
        let status = unsafe {
            msime_engine_prepare_runtime(
                &fixture.paths(),
                id.as_ptr().cast(),
                id.len(),
                ptr::null_mut(),
            )
        };
        assert_eq!(status, MSIME_ENGINE_OK, "{}", last_error());
    };
    let ours = |lines: &LogLines| {
        lines
            .lines
            .iter()
            .filter(|line| line.starts_with("user dictionary journal is not a database"))
            .count()
    };
    // SAFETY: `collected` 活到换回 NULL 之后。
    let status = unsafe {
        msime_engine_set_log_callback(
            Some(collect_log_line),
            ptr::from_ref(&collected).cast_mut().cast(),
        )
    };
    assert_eq!(status, MSIME_ENGINE_OK);
    corrupt_journal_and_prepare();
    // SAFETY: 回到缺省的标准错误。
    assert_eq!(
        unsafe { msime_engine_set_log_callback(None, ptr::null_mut()) },
        MSIME_ENGINE_OK
    );
    {
        let lines = collected.lock().unwrap();
        assert_eq!(ours(&lines), 1, "{:?}", lines.lines);
        assert!(lines.terminated.iter().all(|terminated| *terminated));
        assert!(!lines.nested.is_empty());
        assert!(lines
            .nested
            .iter()
            .all(|status| *status == MSIME_ENGINE_BUSY));
    }
    corrupt_journal_and_prepare();
    assert_eq!(ours(&collected.lock().unwrap()), 1);
}
