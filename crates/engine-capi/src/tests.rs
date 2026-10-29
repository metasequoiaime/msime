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
