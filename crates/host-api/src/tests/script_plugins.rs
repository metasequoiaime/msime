//! 脚本插件经 C ABI 的行为：运行条件和隐私门控、按授权的文本、每次 focus 重建实例、每插件配额、按轮的 fuel、熔断与健康文件、panic 总开关、后台编译线程、不投递的事件，以及两个修改类钩子（改写上屏、候选的重排、隐藏和注释）。
//!
//! 模块用 WAT 写（`msime-script-runtime` 的 `test-support`），编译好直接放进进程缓存，哈希每个测试各不相同；只有后台编译线程那一例从磁盘上的包走完整路径，模块按字节手写。脚本经 `log` 把看到的东西写出来：级别是事件类型或 import 的返回码，文本是读到的文本。测试线程有自己的进程开关和 panic 总开关，互不影响。

use super::*;
use crate::lifecycle::{
    LifecycleEvent, COMMIT_OBSERVE_ONLY, HOST_HOLDS_PIECES, KEY_PASSTHROUGH, TRAIT_ELEVATED,
    TRAIT_KNOWN, TRAIT_PRIVATE, TRAIT_SECURE,
};
use crate::scripts::{
    insert_ready, TEST_COMPILE_PANIC_ID, TEST_PANIC, TEST_PANICKED, TEST_SCRIPT_LOG, TEST_SLOW,
    TEST_SWITCH,
};
use msime_client_core::plugins::{import, script_grants, MANIFEST_FILE};
use msime_input_runtime::{CandidateOps, TransformRow};
use msime_script_runtime::abi::{commit_field, event_field};
use msime_script_runtime::{
    abi::status, compile_text, Capability, CapabilitySet, Declaration, Hook, HookSet,
};
use std::path::PathBuf;
use std::time::{Duration, Instant};

/// 观察类钩子，全部。
const OBSERVE: [Hook; 11] = [
    Hook::FocusIn,
    Hook::FocusOut,
    Hook::Input,
    Hook::SelectionRetracted,
    Hook::CandidateSelected,
    Hook::Commit,
    Hook::CompositionEnd,
    Hook::CompositionStart,
    Hook::CompositionUpdate,
    Hook::CandidatesShown,
    Hook::CandidatesHidden,
];

const HEADER: &str = r#"(memory (export "memory") 1)
  (func (export "msime_abi") (result i32) (i32.const 1))"#;

/// 每个事件都 `log(事件类型, 0, 0)`。
fn logger() -> String {
    format!(
        r#"(module
  (import "msime" "log" (func $log (param i32 i32 i32)))
  {HEADER}
  (func (export "msime_on_event") (param $kind i32) (result i32)
    (call $log (local.get $kind) (i32.const 0) (i32.const 0))
    (i32.const 0)))"#
    )
}

/// 读 `commit_text(field)`：读到了就 `log(事件类型, 文本)`，返回码为负就 `log(返回码, "")`。
fn commit_reader(field: i32) -> String {
    format!(
        r#"(module
  (import "msime" "commit_text" (func $text (param i32 i32 i32) (result i32)))
  (import "msime" "log" (func $log (param i32 i32 i32)))
  {HEADER}
  (func (export "msime_on_event") (param $kind i32) (result i32) (local $n i32)
    (local.set $n (call $text (i32.const {field}) (i32.const 0) (i32.const 1024)))
    (if (i32.lt_s (local.get $n) (i32.const 0))
      (then (call $log (local.get $n) (i32.const 0) (i32.const 0)))
      (else (call $log (local.get $kind) (i32.const 0) (local.get $n))))
    (i32.const 0)))"#
    )
}

/// 读 `composition_text(PREEDIT)` 和 `composition_text(INPUT_CHAR)`，同上写出。
fn composition_reader() -> String {
    format!(
        r#"(module
  (import "msime" "composition_text" (func $text (param i32 i32 i32) (result i32)))
  (import "msime" "log" (func $log (param i32 i32 i32)))
  {HEADER}
  (func $field (param $kind i32) (param $field i32) (local $n i32)
    (local.set $n (call $text (local.get $field) (i32.const 0) (i32.const 1024)))
    (if (i32.lt_s (local.get $n) (i32.const 0))
      (then (call $log (local.get $n) (i32.const 0) (i32.const 0)))
      (else (call $log (local.get $kind) (i32.const 0) (local.get $n)))))
  (func (export "msime_on_event") (param $kind i32) (result i32)
    (call $field (local.get $kind) (i32.const 0))
    (call $field (local.get $kind) (i32.const 2))
    (i32.const 0)))"#
    )
}

/// 一个实例里的计数器：每个事件加一并写出当前值。实例重建后从 1 开始。
fn counter() -> String {
    format!(
        r#"(module
  (import "msime" "log" (func $log (param i32 i32 i32)))
  {HEADER}
  (global $n (mut i32) (i32.const 0))
  (func (export "msime_on_event") (param $kind i32) (result i32)
    (global.set $n (i32.add (global.get $n) (i32.const 1)))
    (call $log (global.get $n) (i32.const 0) (i32.const 0))
    (i32.const 0)))"#
    )
}

/// 死循环，按钩子上限耗尽 fuel。
fn spinner() -> String {
    format!(
        r#"(module
  {HEADER}
  (func (export "msime_on_event") (param $kind i32) (result i32)
    (loop $forever (br $forever))
    (i32.const 0)))"#
    )
}

/// 先写出事件类型，再 `unreachable`。
fn trapper() -> String {
    format!(
        r#"(module
  (import "msime" "log" (func $log (param i32 i32 i32)))
  {HEADER}
  (func (export "msime_on_event") (param $kind i32) (result i32)
    (call $log (local.get $kind) (i32.const 0) (i32.const 0))
    unreachable))"#
    )
}

/// `msime_init` 返回 -1。
fn init_failure() -> String {
    format!(
        r#"(module
  (import "msime" "log" (func $log (param i32 i32 i32)))
  {HEADER}
  (func (export "msime_init") (result i32) (i32.const -1))
  (func (export "msime_on_event") (param $kind i32) (result i32)
    (call $log (local.get $kind) (i32.const 0) (i32.const 0))
    (i32.const 0)))"#
    )
}

/// 空转 `rounds` 圈再写出事件类型。一千圈约七千 fuel，不到观察类钩子上限的一半：被本轮配额截断时不算失败。
fn burner(rounds: u32) -> String {
    format!(
        r#"(module
  (import "msime" "log" (func $log (param i32 i32 i32)))
  {HEADER}
  (func (export "msime_on_event") (param $kind i32) (result i32) (local $i i32)
    (loop $spin
      (local.set $i (i32.add (local.get $i) (i32.const 1)))
      (br_if $spin (i32.lt_u (local.get $i) (i32.const {rounds}))))
    (call $log (local.get $kind) (i32.const 0) (i32.const 0))
    (i32.const 0)))"#
    )
}

/// 一个要装进授权的脚本。
struct Script {
    id: &'static str,
    wat: String,
    hooks: &'static [Hook],
    capabilities: CapabilitySet,
    granted: CapabilitySet,
}

impl Script {
    fn new(id: &'static str, wat: String, hooks: &'static [Hook]) -> Self {
        Self {
            id,
            wat,
            hooks,
            capabilities: CapabilitySet::EMPTY,
            granted: CapabilitySet::EMPTY,
        }
    }

    fn reading(mut self, capability: Capability, granted: bool) -> Self {
        self.capabilities.insert(capability);
        if granted {
            self.granted.insert(capability);
        }
        self
    }
}

/// 一个测试的环境：带状态根目录的全拼会话、插件根目录、user_data。
struct Fixture {
    _directory: tempfile::TempDir,
    plugins: PathBuf,
    user_data: PathBuf,
    handle: u64,
}

impl Drop for Fixture {
    fn drop(&mut self) {
        read(msime_client_destroy(self.handle));
    }
}

/// 本测试线程打开进程开关、清空脚本日志，建一个全拼会话（`ni`、`hao`、`nihao` 都有候选）。
fn fixture() -> Fixture {
    fixture_with(chinese_preferences())
}

/// 同 [`fixture`]，偏好由调用方给。
fn fixture_with(preferences: Preferences) -> Fixture {
    TEST_SWITCH.with(|switch| switch.set(Some(true)));
    TEST_PANICKED.with(|panicked| panicked.set(false));
    TEST_SLOW.with(|slow| slow.set(false));
    TEST_SCRIPT_LOG.with(|log| log.borrow_mut().clear());
    let directory = tempfile::tempdir().unwrap();
    let path = |name: &str| {
        let path = directory.path().join(name);
        std::fs::create_dir_all(&path).unwrap();
        path
    };
    let resources = path("resources");
    let dictionaries = path("dictionaries");
    let state = path("state");
    let plugins = state.join("plugins");
    std::fs::create_dir_all(&plugins).unwrap();
    let fixture = "CREATE TABLE tbl_1_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);
                   INSERT INTO tbl_1_n VALUES('ni','n','你',100);
                   CREATE TABLE tbl_1_h(key TEXT,jp TEXT,value TEXT,weight INTEGER);
                   INSERT INTO tbl_1_h VALUES('hao','h','好',100);
                   CREATE TABLE tbl_2_n(key TEXT,jp TEXT,value TEXT,weight INTEGER);
                   INSERT INTO tbl_2_n VALUES('ni''hao','nh','你好',1000),('ni''hao','nh','拟好',80);
                   CREATE TABLE wubi86(key TEXT,value TEXT,weight INTEGER);
                   CREATE TABLE quick_parases(key TEXT,value TEXT,weight INTEGER);
                   CREATE INDEX idx_quick_parases_key_weight ON quick_parases(key,weight DESC);";
    for directory in [&resources, &dictionaries] {
        rusqlite::Connection::open(directory.join("msime-pinyin.db"))
            .unwrap()
            .execute_batch(fixture)
            .unwrap();
    }
    let user_data = path("user");
    let options = json!({ "api_version": 1, "resources": resources, "user_data": user_data, "cache": path("cache"), "dictionaries": dictionaries, "preferences_directory": state, "preferences": preferences }).to_string();
    let created = read(unsafe { msime_client_create(options.as_ptr(), options.len()) });
    assert_eq!(created["ok"], true, "{created}");
    Fixture {
        handle: created["value"]["session"].as_u64().unwrap(),
        _directory: directory,
        plugins,
        user_data,
    }
}

/// 只属于这个测试的内容哈希：同一个进程里的测试共用模块缓存和熔断表。
fn unique_hash(fixture: &Fixture, id: &str) -> String {
    hex::encode(Sha256::digest(format!(
        "{}\0{id}",
        fixture.plugins.display()
    )))
}

/// 把脚本编译好放进进程缓存，写一份按顺序启用它们的授权文件。返回各自的哈希。
fn install(fixture: &Fixture, scripts: &[Script], revision: u64) -> Vec<String> {
    let mut entries = Vec::new();
    let mut hashes = Vec::new();
    for script in scripts {
        let hash = unique_hash(fixture, script.id);
        let declaration = Declaration {
            capabilities: script.capabilities,
            hooks: script.hooks.iter().copied().collect::<HookSet>(),
            memory_pages: 1,
        };
        let module = compile_text(&script.wat, declaration)
            .unwrap_or_else(|error| panic!("{}: {error}", script.id));
        insert_ready(&fixture.plugins, &hash, module);
        entries.push(json!({
            "id": script.id,
            "hash": hash,
            "enabled": true,
            "granted": script.granted.iter().map(Capability::as_str).collect::<Vec<_>>(),
        }));
        hashes.push(hash);
    }
    write_grants(fixture, revision, entries);
    hashes
}

fn write_grants(fixture: &Fixture, revision: u64, plugins: Vec<Value>) {
    let document = json!({ "format_version": 1, "revision": revision, "plugins": plugins });
    std::fs::write(
        script_grants::document_path(&fixture.plugins),
        serde_json::to_vec(&document).unwrap(),
    )
    .unwrap();
}

/// 选择加入生命周期、报告普通字段，再聚焦一次：脚本从这里开始按授权运行。
fn opt_in(fixture: &Fixture, traits: u32) {
    assert_eq!(msime_client_set_lifecycle(fixture.handle, 0), 0);
    msime_client_set_field_traits(fixture.handle, traits);
    assert_eq!(read(msime_client_focus(fixture.handle, true))["ok"], true);
}

/// 宿主打一个字母：先 `msime_client_key`，再派发。
fn letter(handle: u64, key: u8) {
    msime_client_key(handle, 0);
    assert_eq!(read(msime_client_character(handle, key, false))["ok"], true);
}

/// 打 `nihao`、空格选首选，返回上屏文本。
fn commit_nihao(handle: u64) -> String {
    for key in b"nihao" {
        letter(handle, *key);
    }
    msime_client_key(handle, 1);
    let picked = read(msime_client_command(handle, 1));
    assert_eq!(picked["ok"], true, "{picked}");
    picked["value"]["commit"].as_str().unwrap().to_owned()
}

/// 取走本线程的脚本日志。
fn take_log() -> Vec<(i32, String)> {
    TEST_SCRIPT_LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
}

fn kinds(log: &[(i32, String)]) -> Vec<i32> {
    log.iter().map(|(level, _)| *level).collect()
}

#[test]
fn the_real_process_switch_gates_delivery_in_debug_and_release_builds_without_any_key() {
    // 这个测试线程不覆盖开关，投递走产品里的全局开关。别的测试都在自己的线程上覆盖它，这里改全局开关不影响它们；没覆盖开关、也没写授权的会话计划是空的，同样不受影响。
    let fixture = fixture();
    TEST_SWITCH.with(|switch| switch.set(None));
    install(&fixture, &[Script::new("logger", logger(), &OBSERVE)], 1);
    assert!(!msime_client_scripts_enable(false));
    opt_in(&fixture, TRAIT_KNOWN);
    commit_nihao(fixture.handle);
    assert!(take_log().is_empty());
    // 开发构建和 release 构建（`cargo test --release`）一样：三个桌面平台上打开就投递，授权文件不签名，本进程也没有任何密钥。
    assert!(msime_client_scripts_enable(true));
    commit_nihao(fixture.handle);
    assert!(!take_log().is_empty());
    assert!(!msime_client_scripts_enable(false));
    commit_nihao(fixture.handle);
    assert!(take_log().is_empty());
}

#[test]
fn a_grant_with_a_leftover_mac_field_still_runs() {
    let fixture = fixture();
    let hash = unique_hash(&fixture, "legacy");
    insert_ready(
        &fixture.plugins,
        &hash,
        compile_text(
            &logger(),
            Declaration {
                capabilities: CapabilitySet::EMPTY,
                hooks: OBSERVE.iter().copied().collect::<HookSet>(),
                memory_pages: 1,
            },
        )
        .unwrap(),
    );
    // 早期开发构建在 macOS 上给条目写过 `mac`。授权不再签名，读取方把它当作未知字段忽略：不管它对不对，条目照常运行，所有平台一样。
    write_grants(
        &fixture,
        1,
        vec![
            json!({"id": "legacy", "hash": hash, "enabled": true, "granted": [], "mac": "0".repeat(64)}),
        ],
    );
    opt_in(&fixture, TRAIT_KNOWN);
    commit_nihao(fixture.handle);
    assert!(!take_log().is_empty());
}

#[test]
fn an_ordinary_field_delivers_the_observe_events_and_no_key_down() {
    let fixture = fixture();
    install(&fixture, &[Script::new("logger", logger(), &OBSERVE)], 1);
    opt_in(&fixture, TRAIT_KNOWN);
    assert_eq!(kinds(&take_log()), [Hook::FocusIn.code()]);

    // 只有按键的调用一个事件都不投递：key_down 不给脚本。
    msime_client_key(fixture.handle, 0);
    msime_client_key(fixture.handle, 1);
    assert!(take_log().is_empty());

    assert_eq!(commit_nihao(fixture.handle), "你好");
    let log = kinds(&take_log());
    let code = |hook: Hook| hook.code();
    // 第一键：input, start, update, shown；之后四键各 input, update, shown；空格选首选：input, selected, commit, end, hidden。before_commit 不在这个插件声明的钩子里。
    let mut expected = vec![
        code(Hook::Input),
        code(Hook::CompositionStart),
        code(Hook::CompositionUpdate),
        code(Hook::CandidatesShown),
    ];
    for _ in 0..4 {
        expected.extend([
            code(Hook::Input),
            code(Hook::CompositionUpdate),
            code(Hook::CandidatesShown),
        ]);
    }
    expected.extend([
        code(Hook::Input),
        code(Hook::CandidateSelected),
        code(Hook::Commit),
        code(Hook::CompositionEnd),
        code(Hook::CandidatesHidden),
    ]);
    assert_eq!(log, expected);

    read(msime_client_focus(fixture.handle, false));
    assert_eq!(kinds(&take_log()), [Hook::FocusOut.code()]);
}

#[test]
fn secure_private_elevated_and_unknown_fields_deliver_nothing() {
    let fixture = fixture();
    install(&fixture, &[Script::new("logger", logger(), &OBSERVE)], 1);
    for traits in [
        TRAIT_KNOWN | TRAIT_SECURE,
        TRAIT_KNOWN | TRAIT_PRIVATE,
        TRAIT_KNOWN | TRAIT_ELEVATED,
        0,
        // 只有 SECURE 位、没有 KNOWN 位：未知同样不运行。
        TRAIT_SECURE,
    ] {
        read(msime_client_focus(fixture.handle, false));
        opt_in(&fixture, traits);
        assert_eq!(commit_nihao(fixture.handle), "你好");
        read(msime_client_focus(fixture.handle, false));
        assert!(take_log().is_empty(), "traits {traits:#x} ran a script");
    }
    // 同一个会话换回普通字段就照常运行。
    opt_in(&fixture, TRAIT_KNOWN);
    commit_nihao(fixture.handle);
    assert!(!take_log().is_empty());
}

#[test]
fn a_new_field_runs_nothing_until_its_traits_are_confirmed() {
    let fixture = fixture();
    install(&fixture, &[Script::new("logger", logger(), &OBSERVE)], 1);
    opt_in(&fixture, TRAIT_KNOWN);
    assert_eq!(kinds(&take_log()), [Hook::FocusIn.code()]);
    read(msime_client_focus(fixture.handle, false));
    take_log();

    // 聚焦进一个新字段，宿主在聚焦前没有报告属性：上一个字段的普通属性不算数，`focus_in` 不投递，也不实例化。按契约宿主在第一个键之前才比较属性，这里它发现是密码框。
    read(msime_client_focus(fixture.handle, true));
    assert!(take_log().is_empty());
    msime_client_set_field_traits(fixture.handle, TRAIT_KNOWN | TRAIT_SECURE);
    assert_eq!(commit_nihao(fixture.handle), "你好");
    read(msime_client_focus(fixture.handle, false));
    assert!(take_log().is_empty());

    // 下一个字段是普通字段，属性和上一次报告的不同，宿主照样只在第一个键之前报告：从那个键开始运行。
    read(msime_client_focus(fixture.handle, true));
    assert!(take_log().is_empty());
    msime_client_set_field_traits(fixture.handle, TRAIT_KNOWN);
    letter(fixture.handle, b'n');
    assert!(kinds(&take_log()).contains(&Hook::Input.code()));
    read(msime_client_focus(fixture.handle, false));
    take_log();

    // 属性没变时宿主不报告，第一个键本身就是确认。
    read(msime_client_focus(fixture.handle, true));
    assert!(take_log().is_empty());
    letter(fixture.handle, b'n');
    assert!(kinds(&take_log()).contains(&Hook::Input.code()));
    read(msime_client_focus(fixture.handle, false));
    take_log();

    // 聚焦之前报告过属性（哪怕没变）：`focus_in` 照常投递。
    msime_client_set_field_traits(fixture.handle, TRAIT_KNOWN);
    read(msime_client_focus(fixture.handle, true));
    assert_eq!(kinds(&take_log()), [Hook::FocusIn.code()]);
}

#[test]
fn a_passthrough_key_does_not_confirm_the_traits_of_a_new_field() {
    let fixture = fixture();
    install(&fixture, &[Script::new("logger", logger(), &OBSERVE)], 1);
    opt_in(&fixture, TRAIT_KNOWN);
    read(msime_client_focus(fixture.handle, false));
    take_log();
    // 新字段，宿主没在聚焦前报告属性。Windows 上交给应用的键经 Aux 管道上报，和 Main 管道上新字段的属性帧没有先后保证：它不能当作属性已经比较过，之后的派发什么都不运行。
    read(msime_client_focus(fixture.handle, true));
    msime_client_key(fixture.handle, KEY_PASSTHROUGH);
    read(msime_client_reset_cache(fixture.handle));
    assert!(take_log().is_empty());
    // 不交给应用的键才算确认。
    letter(fixture.handle, b'n');
    assert!(kinds(&take_log()).contains(&Hook::Input.code()));
}

#[test]
fn a_field_turning_secure_drops_the_running_instance() {
    let fixture = fixture();
    install(
        &fixture,
        &[Script::new("counter", counter(), &[Hook::Commit])],
        1,
    );
    opt_in(&fixture, TRAIT_KNOWN);
    commit_nihao(fixture.handle);
    commit_nihao(fixture.handle);
    assert_eq!(kinds(&take_log()), [1, 2]);
    msime_client_set_field_traits(fixture.handle, TRAIT_KNOWN | TRAIT_SECURE);
    commit_nihao(fixture.handle);
    assert!(take_log().is_empty());
    // 回到普通字段：实例是新建的，计数从 1 开始。
    msime_client_set_field_traits(fixture.handle, TRAIT_KNOWN);
    commit_nihao(fixture.handle);
    assert_eq!(kinds(&take_log()), [1]);
}

#[test]
fn text_reaches_a_script_only_with_its_grant() {
    let fixture = fixture();
    install(
        &fixture,
        &[
            Script::new(
                "granted",
                commit_reader(commit_field::COMMIT),
                &[Hook::Commit],
            )
            .reading(Capability::ReadCommit, true),
            Script::new(
                "not-granted",
                commit_reader(commit_field::COMMIT),
                &[Hook::Commit],
            )
            .reading(Capability::ReadCommit, false),
        ],
        1,
    );
    opt_in(&fixture, TRAIT_KNOWN);
    assert_eq!(commit_nihao(fixture.handle), "你好");
    assert_eq!(
        take_log(),
        [
            (Hook::Commit.code(), "你好".to_owned()),
            (status::DENIED, String::new()),
        ]
    );
}

#[test]
fn read_commit_sees_the_selected_candidate_and_the_body_without_the_tail() {
    let fixture = fixture();
    install(
        &fixture,
        &[
            Script::new(
                "selected",
                commit_reader(commit_field::SELECTED),
                &[Hook::CandidateSelected, Hook::Commit],
            )
            .reading(Capability::ReadCommit, true),
            Script::new(
                "body",
                commit_reader(commit_field::COMMIT_BODY),
                &[Hook::Commit],
            )
            .reading(Capability::ReadCommit, true),
        ],
        1,
    );
    opt_in(&fixture, TRAIT_KNOWN);
    for key in b"nihao" {
        letter(fixture.handle, *key);
    }
    // 逗号结束组字：选中的是首选，上屏是首选加全角逗号，body 不带逗号。
    msime_client_key(fixture.handle, 0);
    let finished = read(msime_client_punctuation(fixture.handle, b','));
    assert_eq!(finished["value"]["commit"], "你好，");
    assert_eq!(
        take_log(),
        [
            (Hook::CandidateSelected.code(), "你好".to_owned()),
            // 选中的文本只在 candidate_selected 里有。
            (status::ABSENT, String::new()),
            (Hook::Commit.code(), "你好".to_owned()),
        ]
    );
}

#[test]
fn composition_text_follows_read_composition() {
    let fixture = fixture();
    install(
        &fixture,
        &[Script::new(
            "composer",
            composition_reader(),
            &[Hook::Input, Hook::CompositionUpdate],
        )
        .reading(Capability::ReadComposition, true)],
        1,
    );
    opt_in(&fixture, TRAIT_KNOWN);
    letter(fixture.handle, b'n');
    let input = Hook::Input.code();
    let update = Hook::CompositionUpdate.code();
    // input：预编辑串还没有（ABSENT），打的字符是 n；update：预编辑串是 n，没有打的字符。
    assert_eq!(
        take_log(),
        [
            (status::ABSENT, String::new()),
            (input, "n".to_owned()),
            (update, "n".to_owned()),
            (status::ABSENT, String::new()),
        ]
    );
}

#[test]
fn every_focus_rebuilds_the_instances() {
    let fixture = fixture();
    install(
        &fixture,
        &[Script::new(
            "counter",
            counter(),
            &[Hook::Commit, Hook::FocusOut],
        )],
        1,
    );
    opt_in(&fixture, TRAIT_KNOWN);
    commit_nihao(fixture.handle);
    commit_nihao(fixture.handle);
    // focus_out 还送到同一个实例，之后整个丢弃。
    read(msime_client_focus(fixture.handle, false));
    assert_eq!(kinds(&take_log()), [1, 2, 3]);
    read(msime_client_focus(fixture.handle, true));
    commit_nihao(fixture.handle);
    assert_eq!(kinds(&take_log()), [1]);
    // 只有 focus(true)、没有失焦（IBus 换字段、切中英都是这样）也重建。
    read(msime_client_focus(fixture.handle, true));
    commit_nihao(fixture.handle);
    assert_eq!(kinds(&take_log()), [1]);
}

#[test]
fn one_plugin_burning_its_quota_leaves_the_next_one_untouched() {
    let fixture = fixture();
    let hashes = install(
        &fixture,
        &[
            Script::new("spinner", spinner(), &[Hook::Commit]),
            Script::new("logger", logger(), &[Hook::Commit]),
        ],
        1,
    );
    opt_in(&fixture, TRAIT_KNOWN);
    for _ in 0..2 {
        assert_eq!(commit_nihao(fixture.handle), "你好");
        assert_eq!(kinds(&take_log()), [Hook::Commit.code()]);
    }
    // 死循环只在第一次运行：fuel 耗尽是 trap，这次聚焦里不再重建。
    let spinner = process_health(&fixture, "spinner", &hashes[0]);
    assert_eq!(spinner.0, 1);
    assert_eq!(process_health(&fixture, "logger", &hashes[1]).0, 0);
}

/// (最近 50 次投递里的失败次数, 停用的具体原因码)，直接读进程级的健康表。
fn process_health(fixture: &Fixture, id: &str, hash: &str) -> (u32, Option<&'static str>) {
    crate::scripts::health_snapshot(&fixture.plugins, id, hash)
}

#[test]
fn fuel_is_given_per_turn_and_a_new_key_refills_it() {
    let fixture = fixture();
    let hashes = install(
        &fixture,
        &[Script::new("burner", burner(1000), &[Hook::Input])],
        1,
    );
    opt_in(&fixture, TRAIT_KNOWN);
    // 每次 input 花几千 fuel，不到观察类钩子 25 000 上限的一半。
    for _ in 0..50 {
        msime_client_key(fixture.handle, 0);
        read(msime_client_reset_cache(fixture.handle));
    }
    assert_eq!(take_log().len(), 50);
    // 同一轮里（交给应用的键不开新的一轮）连发：轮配额 125 000 用完后跳过，不算失败。
    msime_client_key(fixture.handle, KEY_PASSTHROUGH);
    for _ in 0..50 {
        read(msime_client_reset_cache(fixture.handle));
    }
    assert!(take_log().len() < 50);
    assert_eq!(process_health(&fixture, "burner", &hashes[0]), (0, None));
}

#[test]
fn the_turn_quota_belongs_to_the_plugin_not_to_one_instance() {
    let fixture = fixture();
    let hashes = install(
        &fixture,
        &[Script::new("burner", burner(1000), &[Hook::Input])],
        1,
    );
    opt_in(&fixture, TRAIT_KNOWN);
    // 一轮的配额够投递几次：花光之后最后一次运行到一半被截断。
    msime_client_key(fixture.handle, 0);
    for _ in 0..50 {
        read(msime_client_reset_cache(fixture.handle));
    }
    let per_turn = take_log().len();
    assert!(per_turn > 3 && per_turn < 50, "{per_turn}");
    // 同一轮里换了个字段（没有按键）：这一轮还是花光的，跳过，也不为它重新初始化。
    msime_client_set_field_traits(fixture.handle, TRAIT_KNOWN);
    read(msime_client_focus(fixture.handle, true));
    read(msime_client_reset_cache(fixture.handle));
    assert!(take_log().is_empty());
    // 新的一轮投递 3 次，再在同一轮里换字段：新实例接着这一轮剩下的配额，而不是重新给满，两个实例合计和一轮一样多。
    msime_client_key(fixture.handle, 0);
    for _ in 0..3 {
        read(msime_client_reset_cache(fixture.handle));
    }
    assert_eq!(take_log().len(), 3);
    read(msime_client_focus(fixture.handle, false));
    msime_client_set_field_traits(fixture.handle, TRAIT_KNOWN);
    read(msime_client_focus(fixture.handle, true));
    for _ in 0..50 {
        read(msime_client_reset_cache(fixture.handle));
    }
    assert_eq!(take_log().len(), per_turn - 3);
    // 用完本轮配额不是插件的失败。
    assert_eq!(process_health(&fixture, "burner", &hashes[0]), (0, None));
}

#[test]
fn an_exhausted_turn_skips_the_plugin_and_never_trips_the_breaker() {
    let fixture = fixture();
    let hashes = install(
        &fixture,
        &[Script::new("burner", burner(1000), &[Hook::Input])],
        1,
    );
    opt_in(&fixture, TRAIT_KNOWN);
    // 修饰键组合、快捷键和失焦命令不经过 msime_client_key，这些派发和上一次按键共用一轮。每次聚焦都把这一轮花光：每次投递都不到自己上限的一半，被截断的那一次用掉的只会更少，插件只是被跳过，连着四次也不熔断（按失败计时第三次就停用了）。
    for _ in 0..4 {
        msime_client_set_field_traits(fixture.handle, TRAIT_KNOWN);
        read(msime_client_focus(fixture.handle, true));
        msime_client_key(fixture.handle, 0);
        for _ in 0..50 {
            read(msime_client_reset_cache(fixture.handle));
        }
        let ran = take_log().len();
        assert!(ran > 1 && ran < 50, "{ran}");
    }
    assert_eq!(process_health(&fixture, "burner", &hashes[0]), (0, None));
    // 被截断的实例这次聚焦里不再重建；下一次聚焦、新的一轮照常运行。
    read(msime_client_focus(fixture.handle, true));
    letter(fixture.handle, b'n');
    assert_eq!(kinds(&take_log()), [Hook::Input.code()]);
}

/// 每个事件先写出事件类型；`candidates` 接着死循环，其余钩子空转一千圈（几千 fuel）后返回。
fn candidates_spinner() -> String {
    format!(
        r#"(module
  (import "msime" "log" (func $log (param i32 i32 i32)))
  {HEADER}
  (func (export "msime_on_event") (param $kind i32) (result i32) (local $i i32)
    (call $log (local.get $kind) (i32.const 0) (i32.const 0))
    (if (i32.eq (local.get $kind) (i32.const {candidates}))
      (then (loop $forever (br $forever))))
    (loop $spin
      (local.set $i (i32.add (local.get $i) (i32.const 1)))
      (br_if $spin (i32.lt_u (local.get $i) (i32.const 1000))))
    (i32.const 0)))"#,
        candidates = Hook::Candidates.code(),
    )
}

#[test]
fn a_loop_in_candidates_after_input_trips_the_breaker() {
    let fixture = fixture();
    let hashes = install(
        &fixture,
        &[Script::new(
            "candidates-spinner",
            candidates_spinner(),
            &[Hook::Input, Hook::Candidates],
        )],
        1,
    );
    opt_in(&fixture, TRAIT_KNOWN);
    // 同一轮里 `input` 先花掉几千 fuel，`candidates` 的死循环于是先撞上轮配额（它的单次上限和轮配额相等）。这一次用掉的远超自己上限的一半，算一次失败：每次聚焦失败一次，第 3 次停用。
    for focus in 0..4 {
        if focus > 0 {
            read(msime_client_focus(fixture.handle, true));
        }
        // 死循环不影响打字；trap 之后这次聚焦里不再重建实例，第二个字母什么都不投递。
        letter(fixture.handle, b'n');
        letter(fixture.handle, b'i');
        let expected: &[i32] = if focus < 3 {
            &[Hook::Input.code(), Hook::Candidates.code()]
        } else {
            &[]
        };
        assert_eq!(kinds(&take_log()), expected, "focus {focus}");
    }
    assert_eq!(
        process_health(&fixture, "candidates-spinner", &hashes[0]),
        (3, Some("out_of_fuel"))
    );
}

#[test]
fn repeated_failures_trip_the_breaker_until_the_revision_changes() {
    let fixture = fixture();
    let scripts = || [Script::new("trapper", trapper(), &[Hook::Commit])];
    let hashes = install(&fixture, &scripts(), 1);
    opt_in(&fixture, TRAIT_KNOWN);
    let file = fixture.user_data.join("script-health.json");
    for focus in 0..4 {
        if focus > 0 {
            read(msime_client_focus(fixture.handle, true));
        }
        // 脚本 trap 不影响上屏；trap 之后这次聚焦里不再重建实例，第二次上屏不运行。
        for _ in 0..2 {
            assert_eq!(commit_nihao(fixture.handle), "你好");
        }
        if focus == 1 {
            // 还没停用：失焦时什么都不写，次数和原因不出进程。后台线程按顺序处理，屏障文件写出来时，之前交过去的写入都已经完成。
            read(msime_client_focus(fixture.handle, false));
            let barrier = fixture.user_data.join("barrier.json");
            crate::scripts::health_barrier(&barrier);
            wait_for(|| barrier.exists().then_some(()));
            assert!(!file.exists());
        }
    }
    // 第 3 次失败（第 3 次聚焦）停用，之后不再运行。
    assert_eq!(take_log().len(), 3);
    assert_eq!(
        process_health(&fixture, "trapper", &hashes[0]),
        (3, Some("unreachable"))
    );

    // 失焦时交给后台线程写出：只有停用的插件和停用的类别，没有次数，也不分是哪种 trap。
    read(msime_client_focus(fixture.handle, false));
    let health = wait_for(|| {
        let bytes = std::fs::read(&file).ok()?;
        serde_json::from_slice::<Value>(&bytes).ok()
    });
    assert_eq!(
        health,
        json!({"format_version": 1, "plugins": [{"id": "trapper", "hash": hashes[0], "disabled": "failed"}]})
    );

    // 授权文件的 revision 变了：恢复。
    install(&fixture, &scripts(), 2);
    read(msime_client_focus(fixture.handle, true));
    commit_nihao(fixture.handle);
    assert_eq!(kinds(&take_log()), [Hook::Commit.code()]);
}

#[test]
fn a_revision_wrapped_by_the_writer_resets_the_breaker() {
    let fixture = fixture();
    let scripts = || [Script::new("trapper", trapper(), &[Hook::Commit])];
    let hashes = install(&fixture, &scripts(), script_grants::MAX_REVISION - 1);
    opt_in(&fixture, TRAIT_KNOWN);
    for focus in 0..3 {
        if focus > 0 {
            read(msime_client_focus(fixture.handle, true));
        }
        commit_nihao(fixture.handle);
    }
    assert_eq!(take_log().len(), 3);
    assert_eq!(
        process_health(&fixture, "trapper", &hashes[0]).1,
        Some("unreachable")
    );
    read(msime_client_focus(fixture.handle, false));
    read(msime_client_focus(fixture.handle, true));
    commit_nihao(fixture.handle);
    assert!(
        take_log().is_empty(),
        "the same maximum revision must not reset the breaker"
    );

    // 写入方在这个上界之后改用当前时间的微秒数，下一份文件的代次会变小。
    // 它仍是用户刚改过的授权，熔断应恢复。
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap();
    let normal_revision = u64::try_from(now.as_micros()).unwrap();
    assert!(normal_revision < script_grants::MAX_REVISION - 1);
    install(&fixture, &scripts(), normal_revision);
    let document = script_grants::document_path(&fixture.plugins);
    let mut wrapped: Value = serde_json::from_slice(&std::fs::read(&document).unwrap()).unwrap();
    wrapped["revision_epoch"] = 1.into();
    let wrapped = serde_json::to_vec(&wrapped).unwrap();
    std::fs::write(&document, &wrapped).unwrap();
    read(msime_client_focus(fixture.handle, false));
    read(msime_client_focus(fixture.handle, true));
    commit_nihao(fixture.handle);
    assert_eq!(kinds(&take_log()), [Hook::Commit.code()]);

    // 回绕后的新代次让插件再次失败并熔断。旧的上界文件与新文件交替
    // 回放时，不应把每次切换都当成一次新的授权修改。
    for _ in 0..2 {
        read(msime_client_focus(fixture.handle, false));
        read(msime_client_focus(fixture.handle, true));
        commit_nihao(fixture.handle);
    }
    assert_eq!(take_log().len(), 2);
    assert_eq!(
        process_health(&fixture, "trapper", &hashes[0]).1,
        Some("unreachable")
    );
    for revision in [
        script_grants::MAX_REVISION - 2,
        script_grants::MAX_REVISION - 1,
        normal_revision,
    ] {
        if revision == normal_revision {
            std::fs::write(&document, &wrapped).unwrap();
        } else {
            install(&fixture, &scripts(), revision);
        }
        read(msime_client_focus(fixture.handle, false));
        read(msime_client_focus(fixture.handle, true));
        commit_nihao(fixture.handle);
        assert!(
            take_log().is_empty(),
            "replaying revision {revision} reset the breaker"
        );
    }
}

/// 失焦，等后台线程把这之前交过去的健康文件写完。
fn flush(fixture: &Fixture) {
    read(msime_client_focus(fixture.handle, false));
    let barrier = fixture.user_data.join("barrier.json");
    let _ = std::fs::remove_file(&barrier);
    crate::scripts::health_barrier(&barrier);
    wait_for(|| barrier.exists().then_some(()));
}

#[test]
fn putting_back_an_older_grant_file_never_resets_the_breaker() {
    let fixture = fixture();
    let scripts = || [Script::new("trapper", trapper(), &[Hook::Commit])];
    let hashes = install(&fixture, &scripts(), 5);
    opt_in(&fixture, TRAIT_KNOWN);
    for focus in 0..3 {
        if focus > 0 {
            read(msime_client_focus(fixture.handle, true));
        }
        commit_nihao(fixture.handle);
    }
    assert_eq!(take_log().len(), 3);
    assert_eq!(
        process_health(&fixture, "trapper", &hashes[0]),
        (3, Some("unreachable"))
    );
    let document = script_grants::document_path(&fixture.plugins);
    let original = std::fs::read(&document).unwrap();
    let mut older: Value = serde_json::from_slice(&original).unwrap();
    older["revision"] = 4.into();
    // 来回换文件：一份 revision 相同但没有条目的、一份更旧的、删掉文件，然后放回原来那份。每换一次就重新聚焦，让会话重读授权。
    let swaps: [Option<Vec<u8>>; 3] = [
        Some(br#"{"format_version":1,"revision":5,"plugins":[]}"#.to_vec()),
        Some(serde_json::to_vec(&older).unwrap()),
        None,
    ];
    for swap in swaps {
        match swap {
            Some(bytes) => std::fs::write(&document, bytes).unwrap(),
            None => std::fs::remove_file(&document).unwrap(),
        }
        read(msime_client_focus(fixture.handle, false));
        read(msime_client_focus(fixture.handle, true));
        // 放回原来那份（字节和原来的一样），模块也重新放进缓存：中间那次重读可能淘汰了它。
        install(&fixture, &scripts(), 5);
        assert_eq!(std::fs::read(&document).unwrap(), original);
        read(msime_client_focus(fixture.handle, false));
        read(msime_client_focus(fixture.handle, true));
        commit_nihao(fixture.handle);
        // 仍然停用：中间那份和放回来的那份，revision 都没有比以前看到过的大。
        assert!(take_log().is_empty());
        assert_eq!(
            process_health(&fixture, "trapper", &hashes[0]).1,
            Some("unreachable")
        );
    }
    // 设置应用写入一次（revision 变大）才恢复。授权文件不签名，同一用户下的程序直接写一份 revision 更大的文件同样能让它恢复，这是授权页写明的残余风险的一部分。
    install(&fixture, &scripts(), 7);
    read(msime_client_focus(fixture.handle, false));
    read(msime_client_focus(fixture.handle, true));
    commit_nihao(fixture.handle);
    assert_eq!(kinds(&take_log()), [Hook::Commit.code()]);
}

#[test]
fn a_restarted_process_clears_the_stopped_plugins_its_predecessor_recorded() {
    let fixture = fixture();
    // 上一个输入法进程停用过 logger；停用只持续到进程重启。本进程第一次读授权时接管这份文件，下一次失焦按本进程的状态重写它。
    let file = fixture.user_data.join("script-health.json");
    let hashes = install(
        &fixture,
        &[Script::new("logger", logger(), &[Hook::Commit])],
        1,
    );
    std::fs::write(
        &file,
        serde_json::to_vec(&json!({"format_version": 1, "plugins": [{"id": "logger", "hash": hashes[0], "disabled": "failed"}]})).unwrap(),
    )
    .unwrap();
    opt_in(&fixture, TRAIT_KNOWN);
    commit_nihao(fixture.handle);
    assert_eq!(kinds(&take_log()), [Hook::Commit.code()]);
    flush(&fixture);
    let health: Value = serde_json::from_slice(&std::fs::read(&file).unwrap()).unwrap();
    assert_eq!(health, json!({"format_version": 1, "plugins": []}));
}

#[test]
fn a_new_process_without_a_health_file_writes_none() {
    let fixture = fixture();
    install(
        &fixture,
        &[Script::new("logger", logger(), &[Hook::Commit])],
        1,
    );
    opt_in(&fixture, TRAIT_KNOWN);
    commit_nihao(fixture.handle);
    flush(&fixture);
    assert!(!fixture.user_data.join("script-health.json").exists());
}

#[test]
fn a_failed_init_disables_the_plugin_at_once() {
    let fixture = fixture();
    let hashes = install(
        &fixture,
        &[Script::new("init-failure", init_failure(), &[Hook::Commit])],
        1,
    );
    opt_in(&fixture, TRAIT_KNOWN);
    for _ in 0..2 {
        assert_eq!(commit_nihao(fixture.handle), "你好");
    }
    assert!(take_log().is_empty());
    assert_eq!(
        process_health(&fixture, "init-failure", &hashes[0]),
        (0, Some("init_failed"))
    );
}

#[test]
fn slow_deliveries_trip_the_breaker() {
    let fixture = fixture();
    let hashes = install(
        &fixture,
        &[Script::new("logger", logger(), &[Hook::Commit])],
        1,
    );
    opt_in(&fixture, TRAIT_KNOWN);
    TEST_SLOW.with(|slow| slow.set(true));
    for _ in 0..4 {
        commit_nihao(fixture.handle);
    }
    assert_eq!(take_log().len(), 3);
    assert_eq!(
        process_health(&fixture, "logger", &hashes[0]),
        (0, Some("slow"))
    );
}

#[test]
fn a_session_that_never_opted_in_runs_no_script() {
    let fixture = fixture();
    install(&fixture, &[Script::new("logger", logger(), &OBSERVE)], 1);
    read(msime_client_focus(fixture.handle, true));
    commit_nihao(fixture.handle);
    read(msime_client_focus(fixture.handle, false));
    assert!(take_log().is_empty());
}

#[test]
fn the_process_switch_gates_every_session() {
    let fixture = fixture();
    install(&fixture, &[Script::new("logger", logger(), &OBSERVE)], 1);
    TEST_SWITCH.with(|switch| switch.set(Some(false)));
    opt_in(&fixture, TRAIT_KNOWN);
    commit_nihao(fixture.handle);
    assert!(take_log().is_empty());
}

#[test]
fn side_channel_view_events_are_not_delivered() {
    let fixture = fixture();
    install(&fixture, &[Script::new("logger", logger(), &OBSERVE)], 1);
    opt_in(&fixture, TRAIT_KNOWN);
    letter(fixture.handle, b'n');
    take_log();
    // 组字中途切到英文：核心收到 composition_end 和 candidates_hidden，脚本收不到。
    assert_eq!(
        read(msime_client_set_english_mode(fixture.handle, true))["ok"],
        true
    );
    assert!(take_log().is_empty());
}

#[test]
fn a_host_side_panic_turns_every_script_off_and_the_commit_still_lands() {
    let fixture = fixture();
    install(
        &fixture,
        &[Script::new("logger", logger(), &[Hook::Commit])],
        1,
    );
    opt_in(&fixture, TRAIT_KNOWN);
    TEST_PANIC.with(|panic| panic.set(true));
    assert_eq!(commit_nihao(fixture.handle), "你好");
    assert!(TEST_PANICKED.with(std::cell::Cell::get));
    commit_nihao(fixture.handle);
    assert!(take_log().is_empty());
}

fn wait_for<T>(mut probe: impl FnMut() -> Option<T>) -> T {
    let deadline = Instant::now() + Duration::from_secs(10);
    loop {
        if let Some(value) = probe() {
            return value;
        }
        assert!(Instant::now() < deadline, "timed out");
        std::thread::sleep(Duration::from_millis(5));
    }
}

/// 最小的 ABI v1 模块，按字节手写：导入 `msime.log`，一页内存，`msime_on_event` 写出事件类型并返回 0。走后台线程那一例要一个真的 `.wasm` 文件，而 WAT 只能在测试里直接编译成模块。
fn logger_binary() -> Vec<u8> {
    logger_binary_with(false)
}

/// 同 [`logger_binary`]，`reads_commit` 时另外导入 `msime.commit_scalar`（不调用），清单要申请 `read_commit`：申请了却没导入的能力会被拒绝。
fn logger_binary_with(reads_commit: bool) -> Vec<u8> {
    fn section(id: u8, content: &[u8], out: &mut Vec<u8>) {
        out.push(id);
        out.push(u8::try_from(content.len()).unwrap());
        out.extend_from_slice(content);
    }
    fn name(text: &str, out: &mut Vec<u8>) {
        out.push(u8::try_from(text.len()).unwrap());
        out.extend_from_slice(text.as_bytes());
    }
    let mut out = b"\0asm\x01\0\0\0".to_vec();
    // 类型：(i32 i32 i32) -> ()、() -> i32、(i32) -> i32，读上屏时再加 (i32) -> i64。
    let mut types = vec![
        3, 0x60, 3, 0x7f, 0x7f, 0x7f, 0, 0x60, 0, 1, 0x7f, 0x60, 1, 0x7f, 1, 0x7f,
    ];
    if reads_commit {
        types[0] = 4;
        types.extend_from_slice(&[0x60, 1, 0x7f, 1, 0x7e]);
    }
    section(1, &types, &mut out);
    let imported = if reads_commit { 2 } else { 1 };
    let mut imports = vec![imported];
    name("msime", &mut imports);
    name("log", &mut imports);
    imports.extend_from_slice(&[0x00, 0]);
    if reads_commit {
        name("msime", &mut imports);
        name("commit_scalar", &mut imports);
        imports.extend_from_slice(&[0x00, 3]);
    }
    section(2, &imports, &mut out);
    section(3, &[2, 1, 2], &mut out);
    section(5, &[1, 0x00, 1], &mut out);
    let mut exports = vec![3];
    // 导入的函数排在前面，自己定义的两个函数紧随其后。
    for (export, kind, index) in [
        ("memory", 2, 0),
        ("msime_abi", 0, imported),
        ("msime_on_event", 0, imported + 1),
    ] {
        name(export, &mut exports);
        exports.extend_from_slice(&[kind, index]);
    }
    section(7, &exports, &mut out);
    let abi = [0x00, 0x41, 0x01, 0x0b];
    let on_event = [
        0x00, 0x20, 0x00, 0x41, 0x00, 0x41, 0x00, 0x10, 0x00, 0x41, 0x00, 0x0b,
    ];
    let mut code = vec![2, abi.len() as u8];
    code.extend_from_slice(&abi);
    code.push(on_event.len() as u8);
    code.extend_from_slice(&on_event);
    section(10, &code, &mut out);
    out
}

#[test]
fn an_installed_pack_compiles_on_the_background_thread_and_then_runs() {
    let fixture = fixture();
    let source = fixture.plugins.parent().unwrap().join("source").join("e2e");
    std::fs::create_dir_all(&source).unwrap();
    let manifest = "schema_version = 2\nkind = 'script'\nid = 'e2e'\nname = '记录'\nversion = '1.0.0'\nlicense = 'MIT'\npermissions = []\n[script]\nabi = 1\nmodule = 'main.wasm'\nhooks = ['commit']\nmemory_pages = 1\n";
    std::fs::write(source.join(MANIFEST_FILE), manifest).unwrap();
    std::fs::write(source.join("main.wasm"), logger_binary()).unwrap();
    let summary = import(&source, &fixture.plugins).unwrap();
    let msime_client_core::plugins::PluginContent::Script(pack) = summary.content else {
        panic!("not a script pack");
    };
    script_grants::set(
        &fixture.plugins,
        "e2e",
        &pack.content_hash,
        true,
        CapabilitySet::EMPTY,
    )
    .unwrap();
    opt_in(&fixture, TRAIT_KNOWN);
    // 编译在后台线程上：还没编译好的那几次跳过，不算失败，上屏照常。
    wait_for(|| {
        assert_eq!(commit_nihao(fixture.handle), "你好");
        (!take_log().is_empty()).then_some(())
    });
    assert_eq!(
        process_health(&fixture, "e2e", &pack.content_hash),
        (0, None)
    );
}

#[test]
fn a_panic_while_compiling_settles_the_module_instead_of_leaving_it_pending() {
    let fixture = fixture();
    let hash = unique_hash(&fixture, TEST_COMPILE_PANIC_ID);
    write_grants(
        &fixture,
        1,
        vec![json!({"id": TEST_COMPILE_PANIC_ID, "hash": hash, "enabled": true, "granted": []})],
    );
    opt_in(&fixture, TRAIT_KNOWN);
    // 后台线程编译时 panic：模块记为不合规，会话下一次查缓存时停用这个插件，而不是永远等它编译完。（同时关掉本进程的全部脚本；测试里那个总开关是后台线程自己的，不影响这个会话。）
    wait_for(|| {
        assert_eq!(commit_nihao(fixture.handle), "你好");
        process_health(&fixture, TEST_COMPILE_PANIC_ID, &hash)
            .1
            .filter(|reason| *reason == "compile_panicked")
    });
    assert!(take_log().is_empty());
}

#[test]
fn a_grant_whose_hash_no_longer_matches_the_pack_never_runs() {
    let fixture = fixture();
    let source = fixture
        .plugins
        .parent()
        .unwrap()
        .join("source")
        .join("stale");
    std::fs::create_dir_all(&source).unwrap();
    let manifest = "schema_version = 2\nkind = 'script'\nid = 'stale'\nname = '记录'\nversion = '1.0.0'\nlicense = 'MIT'\npermissions = []\n[script]\nabi = 1\nmodule = 'main.wasm'\nhooks = ['commit']\nmemory_pages = 1\n";
    std::fs::write(source.join(MANIFEST_FILE), manifest).unwrap();
    std::fs::write(source.join("main.wasm"), logger_binary()).unwrap();
    import(&source, &fixture.plugins).unwrap();
    // 授权的是另一份内容（哈希对不上）：后台线程核对后不编译它。
    let stale = unique_hash(&fixture, "stale");
    write_grants(
        &fixture,
        1,
        vec![json!({"id": "stale", "hash": stale, "enabled": true, "granted": []})],
    );
    opt_in(&fixture, TRAIT_KNOWN);
    // 编译请求在聚焦时交给后台线程；它按顺序处理，屏障文件写出来时那次核对已经做完，之后的上屏不再取决于后台线程有没有轮到它。
    let barrier = fixture.user_data.join("barrier.json");
    crate::scripts::health_barrier(&barrier);
    wait_for(|| barrier.exists().then_some(()));
    assert_eq!(commit_nihao(fixture.handle), "你好");
    assert!(take_log().is_empty());
    assert_eq!(process_health(&fixture, "stale", &stale), (0, None));
}

/// 本地测量（默认忽略）：宿主一侧投递一个事件的全部开销，包括运行条件检查、构造事件、按授权复制文本、执行一个空的钩子和记账。整次按键的派发是几十到几百微秒，这点开销淹没在它的抖动里，所以直接对着会话的脚本集合量。这是测试构建：墙钟熔断的计时被换成了测试开关，进程开关和 panic 总开关读的是线程局部变量，比产品里少一次 `Instant::elapsed`。`cargo test -p msime-host-api [--release] --lib -- --ignored --nocapture measure_delivery_cost`。
#[test]
#[ignore]
fn measure_delivery_cost() {
    let fixture = fixture();
    let noop = format!(
        r#"(module {HEADER} (func (export "msime_on_event") (param i32) (result i32) (i32.const 0)))"#
    );
    // 申请并被授予 `read_candidates`：每次 candidates_shown 都把候选页复制进插件的上下文。
    let candidates = format!(
        r#"(module
  (import "msime" "candidate_count" (func $count (result i32)))
  {HEADER}
  (func (export "msime_on_event") (param i32) (result i32) (i32.const 0)))"#
    );
    install(
        &fixture,
        &[
            Script::new("noop", noop, &[Hook::CompositionUpdate]),
            Script::new("candidates", candidates, &[Hook::CandidatesShown])
                .reading(Capability::ReadCandidates, true),
        ],
        1,
    );
    opt_in(&fixture, TRAIT_KNOWN);
    for key in b"ni" {
        letter(fixture.handle, *key);
    }
    let rounds = 100_000u32;
    let measure = |event: LifecycleEvent| {
        SESSIONS.with(|sessions| {
            let mut sessions = sessions.borrow_mut();
            let session = sessions.get_mut(&fixture.handle).unwrap();
            // 夹具的词库里 `ni` 只有一个候选；照着它补满一页 20 行，量复制整页的开销。
            let mut view = session.runtime.view();
            while view.candidates.len() < 20 {
                let row = view.candidates[0].clone();
                view.candidates.push(row);
            }
            let state = session.lifecycle.as_deref_mut().unwrap();
            let context = state.context();
            let texts = crate::scripts::CallTexts {
                view: Some(&view),
                ..Default::default()
            };
            let mut scripts = state.scripts_mut();
            let started = Instant::now();
            for _ in 0..rounds {
                scripts.next_turn();
                scripts.deliver(&event, context, &texts);
            }
            (started.elapsed() / rounds, view.candidates.len())
        })
    };
    let (update, _) = measure(LifecycleEvent::CompositionUpdate);
    let (shown, rows) = measure(LifecycleEvent::CandidatesShown {
        count: 0,
        page: 0,
        highlighted: Some(0),
    });
    println!(
        "per delivery ({} build): composition_update to an empty hook {update:?}; candidates_shown with read_candidates, {rows} rows copied {shown:?}",
        if cfg!(debug_assertions) { "dev" } else { "release" }
    );
    assert!(take_log().is_empty());
}

// ---- 修改类钩子 ----

/// `before_commit`：先写出这次能否改写，读 body、接上 `suffix` 后改写，再写出 `rewrite_commit` 的返回码；最后执行 `finish`（返回值或 trap）。
fn appender(suffix: &str, finish: &str) -> String {
    format!(
        r#"(module
  (import "msime" "event_scalar" (func $event (param i32) (result i64)))
  (import "msime" "commit_text" (func $text (param i32 i32 i32) (result i32)))
  (import "msime" "rewrite_commit" (func $rewrite (param i32 i32) (result i32)))
  (import "msime" "log" (func $log (param i32 i32 i32)))
  {HEADER}
  (data (i32.const 32) "{suffix}")
  (func (export "msime_on_event") (param $kind i32) (result i32) (local $n i32)
    (call $log (i32.wrap_i64 (call $event (i32.const {rewritable}))) (i32.const 0) (i32.const 0))
    (local.set $n (call $text (i32.const {body}) (i32.const 64) (i32.const 1024)))
    (if (i32.ge_s (local.get $n) (i32.const 0))
      (then
        (memory.copy (i32.add (i32.const 64) (local.get $n)) (i32.const 32) (i32.const {len}))
        (call $log (call $rewrite (i32.const 64) (i32.add (local.get $n) (i32.const {len}))) (i32.const 0) (i32.const 0))))
    {finish}))"#,
        rewritable = event_field::REWRITABLE,
        body = commit_field::COMMIT_BODY,
        len = suffix.len(),
    )
}

/// `before_commit`：不管原文，把 body 改写成数据段里的 `bytes`（WAT 字符串转义），写出返回码。
fn replacer(bytes: &str, len: usize) -> String {
    format!(
        r#"(module
  (import "msime" "commit_text" (func $text (param i32 i32 i32) (result i32)))
  (import "msime" "rewrite_commit" (func $rewrite (param i32 i32) (result i32)))
  (import "msime" "log" (func $log (param i32 i32 i32)))
  {HEADER}
  (data (i32.const 32) "{bytes}")
  (func (export "msime_on_event") (param $kind i32) (result i32)
    (call $log (call $rewrite (i32.const 32) (i32.const {len})) (i32.const 0) (i32.const 0))
    (i32.const 0)))"#
    )
}

fn rewriting(script: Script) -> Script {
    script
        .reading(Capability::ReadCommit, true)
        .reading(Capability::RewriteCommit, true)
}

/// `candidates`：写出事件类型，再按 `body` 调用修改类 import；[0..) 是数据段 `data`。
fn transformer(data: &str, body: &str) -> String {
    format!(
        r#"(module
  (import "msime" "candidate_count" (func $count (result i32)))
  (import "msime" "candidates_order" (func $order (param i32 i32) (result i32)))
  (import "msime" "candidates_hide" (func $hide (param i32 i32) (result i32)))
  (import "msime" "candidates_annotate" (func $annotate (param i32 i32 i32) (result i32)))
  (import "msime" "log" (func $log (param i32 i32 i32)))
  (memory (export "memory") 1)
  (func (export "msime_abi") (result i32) (i32.const 1))
  (data (i32.const 0) "{data}")
  (func (export "msime_on_event") (param $kind i32) (result i32)
    (call $log (local.get $kind) (i32.const 0) (i32.const 0))
    {body}
    (i32.const 0)))"#
    )
}

fn transforming(script: Script) -> Script {
    script
        .reading(Capability::ReadCandidates, true)
        .reading(Capability::RerankCandidates, true)
        .reading(Capability::FilterCandidates, true)
        .reading(Capability::AnnotateCandidates, true)
}

/// 读候选窗口：每一行写出 `事件类型 × 100 + 行号` 和文本，再写出 `事件类型 × 100 + 50 + 行号` 和引擎注释。
fn window_reader() -> String {
    r#"(module
  (import "msime" "candidate_count" (func $count (result i32)))
  (import "msime" "candidate_text" (func $text (param i32 i32 i32) (result i32)))
  (import "msime" "candidate_annotation" (func $annotation (param i32 i32 i32) (result i32)))
  (import "msime" "log" (func $log (param i32 i32 i32)))
  (memory (export "memory") 1)
  (func (export "msime_abi") (result i32) (i32.const 1))
  (func $write (param $level i32) (param $n i32)
    (call $log (local.get $level) (i32.const 0)
      (select (local.get $n) (i32.const 0) (i32.ge_s (local.get $n) (i32.const 0)))))
  (func (export "msime_on_event") (param $kind i32) (result i32) (local $i i32) (local $rows i32)
    (local.set $rows (call $count))
    (block $done
      (loop $next
        (br_if $done (i32.ge_s (local.get $i) (local.get $rows)))
        (call $write (i32.add (i32.mul (local.get $kind) (i32.const 100)) (local.get $i))
          (call $text (local.get $i) (i32.const 0) (i32.const 256)))
        (call $write (i32.add (i32.mul (local.get $kind) (i32.const 100)) (i32.add (local.get $i) (i32.const 50)))
          (call $annotation (local.get $i) (i32.const 0) (i32.const 256)))
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br $next)))
    (i32.const 0)))"#
        .to_owned()
}

/// 打 `nihao`，返回调用之后的视图。
fn compose_nihao(handle: u64) -> Value {
    for key in b"nihao" {
        letter(handle, *key);
    }
    read(msime_client_view(handle))["value"].clone()
}

fn view_texts(view: &Value) -> Vec<String> {
    view["candidates"]
        .as_array()
        .unwrap()
        .iter()
        .map(|candidate| candidate["text"].as_str().unwrap().to_owned())
        .collect()
}

/// 空格选首选，返回上屏文本。
fn space(handle: u64) -> String {
    msime_client_key(handle, 1);
    let picked = read(msime_client_command(handle, 1));
    assert_eq!(picked["ok"], true, "{picked}");
    picked["value"]["commit"].as_str().unwrap().to_owned()
}

#[test]
fn before_commit_rewrites_the_body_and_hands_the_tail_back() {
    let fixture = fixture();
    install(
        &fixture,
        &[
            rewriting(Script::new(
                "tilde",
                appender("~", "(i32.const 0)"),
                &[Hook::BeforeCommit],
            )),
            // 第二个插件读到的 body 是第一个改写过的。
            rewriting(Script::new(
                "bang",
                appender("!", "(i32.const 0)"),
                &[Hook::BeforeCommit],
            )),
            Script::new(
                "observer",
                commit_reader(commit_field::COMMIT),
                &[Hook::Commit],
            )
            .reading(Capability::ReadCommit, true),
        ],
        1,
    );
    opt_in(&fixture, TRAIT_KNOWN);
    compose_nihao(fixture.handle);
    take_log();
    // 逗号结束组字：body 是"你好"，tail 是全角逗号，改写后原样接回。
    msime_client_key(fixture.handle, 0);
    let finished = read(msime_client_punctuation(fixture.handle, b','));
    assert_eq!(finished["value"]["commit"], "你好~!，");
    assert_eq!(
        take_log(),
        [
            (1, String::new()),
            (0, String::new()),
            (1, String::new()),
            (0, String::new()),
            (Hook::Commit.code(), "你好~!，".to_owned()),
        ]
    );
    // 空格选词没有 tail。
    compose_nihao(fixture.handle);
    assert_eq!(space(fixture.handle), "你好~!");
}

#[test]
fn a_failed_or_rejected_rewrite_commits_the_original_text() {
    let cases: [(&str, String); 5] = [
        ("trap", appender("~", "unreachable")),
        ("negative", appender("~", "(i32.const -1)")),
        (
            "fuel",
            appender("~", "(loop $forever (br $forever)) (i32.const 0)"),
        ),
        ("newline", replacer(r"\0a", 1)),
        ("override", replacer(r"\e2\80\ae", 3)),
    ];
    for (name, wat) in cases {
        let fixture = fixture();
        install(
            &fixture,
            &[rewriting(Script::new(
                "rewriter",
                wat,
                &[Hook::BeforeCommit],
            ))],
            1,
        );
        opt_in(&fixture, TRAIT_KNOWN);
        compose_nihao(fixture.handle);
        assert_eq!(space(fixture.handle), "你好", "{name}");
        if name == "newline" || name == "override" {
            assert_eq!(kinds(&take_log()), [status::INVALID], "{name}");
        }
        take_log();
    }
}

#[test]
fn every_rewrite_is_limited_by_the_original_body() {
    // 原文"你好"是 2 个码元，上限取 256。第一个插件放大到 256 个，第二个插件读到的 body 虽有 256 个码元，上限仍是 256：它的 600 个字母拿到 LIMIT，上屏停在第一个插件的结果。
    let first = "a".repeat(256);
    let second = "b".repeat(600);
    let fixture = fixture();
    install(
        &fixture,
        &[
            rewriting(Script::new(
                "grow",
                replacer(&first, first.len()),
                &[Hook::BeforeCommit],
            )),
            rewriting(Script::new(
                "grow-more",
                replacer(&second, second.len()),
                &[Hook::BeforeCommit],
            )),
        ],
        1,
    );
    opt_in(&fixture, TRAIT_KNOWN);
    compose_nihao(fixture.handle);
    take_log();
    assert_eq!(space(fixture.handle), first);
    assert_eq!(kinds(&take_log()), [0, status::LIMIT]);
}

#[test]
fn observe_only_and_edge_commits_are_delivered_but_never_rewritten() {
    let fixture = fixture();
    install(
        &fixture,
        &[rewriting(Script::new(
            "tilde",
            appender("~", "(i32.const 0)"),
            &[Hook::BeforeCommit],
        ))],
        1,
    );
    opt_in(&fixture, TRAIT_KNOWN);
    // Windows 的做法：TIP 自己已经把文字写进文档的派发，Server 在派发前置 COMMIT_OBSERVE_ONLY、派发后清掉。空格选词和回车原样上屏都只能看。
    for (command, original) in [(1, "你好"), (2, "nihao")] {
        compose_nihao(fixture.handle);
        take_log();
        msime_client_set_lifecycle(fixture.handle, COMMIT_OBSERVE_ONLY);
        msime_client_key(fixture.handle, 1);
        let committed = read(msime_client_command(fixture.handle, command));
        msime_client_set_lifecycle(fixture.handle, 0);
        assert_eq!(committed["value"]["commit"], original);
        assert_eq!(kinds(&take_log()), [0, status::DENIED]);
    }
    // 清掉之后同样两条路径都能改写：选中候选和原样上屏的读音。
    for (command, rewritten) in [(1, "你好~"), (2, "nihao~")] {
        compose_nihao(fixture.handle);
        take_log();
        msime_client_key(fixture.handle, 1);
        let committed = read(msime_client_command(fixture.handle, command));
        assert_eq!(committed["value"]["commit"], rewritten);
        assert_eq!(kinds(&take_log()), [1, 0]);
    }
    // 以词定字一律不可改写。
    let view = compose_nihao(fixture.handle);
    take_log();
    let generation = view["generation"].as_u64().unwrap();
    msime_client_key(fixture.handle, 0);
    let edge = read(msime_client_select_edge(fixture.handle, generation, 0, 0));
    assert_eq!(edge["value"]["commit"], "你");
    assert_eq!(kinds(&take_log()), [0, status::DENIED]);
}

#[test]
fn private_fields_are_never_rewritten() {
    let fixture = fixture();
    install(
        &fixture,
        &[rewriting(Script::new(
            "tilde",
            appender("~", "(i32.const 0)"),
            &[Hook::BeforeCommit],
        ))],
        1,
    );
    for traits in [
        TRAIT_KNOWN | TRAIT_SECURE,
        TRAIT_KNOWN | TRAIT_PRIVATE,
        TRAIT_KNOWN | TRAIT_ELEVATED,
        0,
    ] {
        read(msime_client_focus(fixture.handle, false));
        opt_in(&fixture, traits);
        compose_nihao(fixture.handle);
        assert_eq!(space(fixture.handle), "你好", "traits {traits:#x}");
        assert!(take_log().is_empty());
    }
}

#[test]
fn a_reranked_list_commits_the_displayed_row_and_reads_never_see_other_plugins() {
    let fixture = fixture();
    install(
        &fixture,
        &[
            // 前两行对调，原来的首行加注释"注"。
            transforming(Script::new(
                "swap",
                transformer(
                    r"\01\00\00\00\00\00\00\00\e6\b3\a8",
                    "(drop (call $order (i32.const 0) (i32.const 2)))
                     (drop (call $annotate (i32.const 0) (i32.const 8) (i32.const 3)))",
                ),
                &[Hook::Candidates],
            )),
            Script::new(
                "reader",
                window_reader(),
                &[Hook::Candidates, Hook::CandidatesShown],
            )
            .reading(Capability::ReadCandidates, true),
        ],
        1,
    );
    opt_in(&fixture, TRAIT_KNOWN);
    let view = compose_nihao(fixture.handle);
    let log = take_log();
    // 最后一键：reader 在 candidates 里读到的是变换之前的窗口，显示的是对调之后的。
    let candidates = Hook::Candidates.code() * 100;
    let last_read = |level: i32| {
        log.iter()
            .rev()
            .find(|(logged, _)| *logged == level)
            .map(|(_, text)| text.clone())
            .unwrap()
    };
    let original_first = last_read(candidates);
    let original_second = last_read(candidates + 1);
    let rows = view_texts(&view);
    assert_eq!(rows[0], original_second);
    assert_eq!(rows[1], original_first);
    // 注释另放一格，跟着原来的首行走；引擎的注释和 reader 读到的注释里都没有它。
    assert_eq!(view["candidates"][1]["script_annotation"], "注");
    assert!(view["candidates"][0].get("script_annotation").is_none());
    assert!(log.iter().all(|(_, text)| text != "注"));
    // candidates_shown 读到的是显示的顺序（重排和隐藏是用户看到的），注释仍是引擎的。
    assert_eq!(
        last_read(Hook::CandidatesShown.code() * 100),
        original_second
    );
    // 空格选的是显示的首行。
    assert_eq!(space(fixture.handle), original_second);
}

#[test]
fn a_hidden_row_cannot_be_committed_and_row_zero_cannot_be_hidden() {
    let fixture = fixture();
    install(
        &fixture,
        &[transforming(Script::new(
            "filter",
            // 隐藏第 0、1 行：引擎下标 0 的那一行留下。
            transformer(
                r"\00\00\00\00\01\00\00\00",
                "(drop (call $hide (i32.const 0) (i32.const 2)))",
            ),
            &[Hook::Candidates],
        ))],
        1,
    );
    // 密码框里脚本不运行，先拿到原来的列表。
    opt_in(&fixture, TRAIT_KNOWN | TRAIT_SECURE);
    let original = view_texts(&compose_nihao(fixture.handle));
    read(msime_client_command(fixture.handle, 3));
    assert!(original.len() >= 2, "{original:?}");
    msime_client_set_field_traits(fixture.handle, TRAIT_KNOWN);
    let rows = view_texts(&compose_nihao(fixture.handle));
    read(msime_client_command(fixture.handle, 3));
    let mut expected = original.clone();
    expected.remove(1);
    assert_eq!(rows, expected);
    // 每个可见行都能用数字选中，选中的就是显示的那一行；可见行之后的数字什么都不上屏。
    for (slot, text) in rows.iter().enumerate() {
        assert_eq!(view_texts(&compose_nihao(fixture.handle)), rows);
        msime_client_key(fixture.handle, 0);
        let picked = read(msime_client_character(
            fixture.handle,
            b'1' + slot as u8,
            false,
        ));
        assert_eq!(picked["value"]["commit"], text.as_str());
    }
    compose_nihao(fixture.handle);
    msime_client_key(fixture.handle, 0);
    let beyond = read(msime_client_character(
        fixture.handle,
        b'1' + rows.len() as u8,
        false,
    ));
    assert!(beyond["value"]["commit"].is_null(), "{beyond}");
    read(msime_client_command(fixture.handle, 3));
}

#[test]
fn side_channel_calls_replay_the_plugins_without_running_them() {
    let fixture = fixture_with(Preferences {
        cloud_candidates: true,
        ..chinese_preferences()
    });
    install(
        &fixture,
        &[transforming(Script::new(
            "swap",
            transformer(
                r"\01\00\00\00\00\00\00\00",
                "(drop (call $order (i32.const 0) (i32.const 2)))",
            ),
            &[Hook::Candidates],
        ))],
        1,
    );
    opt_in(&fixture, TRAIT_KNOWN);
    let typed = view_texts(&compose_nihao(fixture.handle));
    assert!(!take_log().is_empty());
    let query = read(msime_client_online_query(fixture.handle))["value"].to_string();
    let candidates = serde_json::to_vec(&json!(["云候选"])).unwrap();
    let applied = read(unsafe {
        msime_client_apply_online_candidates(
            fixture.handle,
            query.as_ptr(),
            query.len(),
            candidates.as_ptr(),
            candidates.len(),
            0,
        )
    });
    assert_eq!(applied["value"]["applied"], true, "{applied}");
    // 云候选坐进它的座位，插件的对调按身份重放，插件本身没有运行。
    let rows = view_texts(&applied["value"]["view"]);
    assert_eq!(rows[0], typed[0]);
    assert!(rows.contains(&"云候选".to_owned()));
    read(msime_client_rerank_settled(fixture.handle));
    read(msime_client_view(fixture.handle));
    assert!(take_log().is_empty());
}

#[test]
fn the_candidates_hook_runs_only_in_whitelisted_local_modes() {
    let fixture = fixture();
    install(
        &fixture,
        &[transforming(Script::new(
            "swap",
            transformer(
                r"\01\00\00\00\00\00\00\00",
                "(drop (call $order (i32.const 0) (i32.const 2)))",
            ),
            &[Hook::Candidates],
        ))],
        1,
    );
    opt_in(&fixture, TRAIT_KNOWN);
    letter(fixture.handle, b'n');
    take_log();
    let window = [
        TransformRow {
            text: "你",
            code: "ni",
            annotation: "",
            source: 0,
        },
        TransformRow {
            text: "泥",
            code: "ni",
            annotation: "",
            source: 0,
        },
    ];
    let run = |local_mode: &str| {
        SESSIONS.with(|sessions| {
            let mut sessions = sessions.borrow_mut();
            let session = sessions.get_mut(&fixture.handle).unwrap();
            let state = session.lifecycle.as_deref_mut().unwrap();
            let mut out = Vec::new();
            state
                .scripts_mut()
                .candidates(&window, local_mode, &mut out);
            out
        })
    };
    // 提及、网址、命令等模式不在白名单里：一个插件都不运行。
    for mode in ["mention", "url", "command", "expression", "unknown"] {
        assert!(run(mode).is_empty(), "{mode}");
        assert!(take_log().is_empty(), "{mode}");
    }
    for mode in ["none", "temporary_english", "emoji"] {
        assert_eq!(
            run(mode),
            [CandidateOps {
                order: vec![1, 0],
                ..CandidateOps::default()
            }],
            "{mode}"
        );
        assert_eq!(kinds(&take_log()), [Hook::Candidates.code()], "{mode}");
    }
}

#[test]
fn pieces_the_host_holds_are_never_rewritten() {
    let fixture = fixture();
    install(
        &fixture,
        &[rewriting(Script::new(
            "tilde",
            appender("~", "(i32.const 0)"),
            &[Hook::BeforeCommit],
        ))],
        1,
    );
    opt_in(&fixture, TRAIT_KNOWN);
    // 选"你"只用掉 ni，读音还剩 hao：部分选词。
    let pick_ni = |handle: u64| {
        let view = compose_nihao(handle);
        let index = view_texts(&view)
            .iter()
            .position(|text| text == "你")
            .unwrap();
        let generation = view["generation"].as_u64().unwrap();
        take_log();
        msime_client_key(handle, 0);
        let picked = read(msime_client_select(handle, generation, index));
        assert_eq!(picked["ok"], true, "{picked}");
        picked["value"]["commit"].as_str().unwrap().to_owned()
    };
    // 宿主不暂存时，部分选词的那一段照样可改写。
    assert_eq!(pick_ni(fixture.handle), "你~");
    assert_eq!(kinds(&take_log()), [1, 0]);
    read(msime_client_command(fixture.handle, 3));
    // Windows 的做法：宿主自己暂存部分选词，补齐整个词时才把各段拼好写进文档。部分选词的那一段和补齐它的那一段都只能看。
    assert_eq!(
        msime_client_set_lifecycle(fixture.handle, HOST_HOLDS_PIECES),
        HOST_HOLDS_PIECES
    );
    assert_eq!(pick_ni(fixture.handle), "你");
    assert_eq!(kinds(&take_log()), [0, status::DENIED]);
    assert_eq!(space(fixture.handle), "好");
    assert_eq!(kinds(&take_log()), [0, status::DENIED]);
    // 组字结束之后，下一个整词照样可改写。
    compose_nihao(fixture.handle);
    take_log();
    assert_eq!(space(fixture.handle), "你好~");
    assert_eq!(kinds(&take_log()), [1, 0]);
}

#[test]
fn observe_only_dispatches_neither_run_nor_replay_the_transform() {
    let fixture = fixture_with(Preferences {
        cloud_candidates: true,
        ..chinese_preferences()
    });
    install(
        &fixture,
        &[transforming(Script::new(
            "swap",
            transformer(
                r"\01\00\00\00\00\00\00\00",
                "(drop (call $order (i32.const 0) (i32.const 2)))",
            ),
            &[Hook::Candidates],
        ))],
        1,
    );
    opt_in(&fixture, TRAIT_KNOWN);
    // Windows 上 TIP 自己选词的列表：Server 每次派发前设上 COMMIT_OBSERVE_ONLY、派发后清掉。插件一次都不运行，列表是引擎的顺序，数字 1 选的就是它的首行。
    for key in b"nihao" {
        msime_client_set_lifecycle(fixture.handle, COMMIT_OBSERVE_ONLY);
        letter(fixture.handle, *key);
        msime_client_set_lifecycle(fixture.handle, 0);
    }
    let plain = view_texts(&read(msime_client_view(fixture.handle))["value"]);
    assert!(plain.len() >= 2, "{plain:?}");
    assert!(take_log().is_empty());
    msime_client_set_lifecycle(fixture.handle, COMMIT_OBSERVE_ONLY);
    msime_client_key(fixture.handle, 0);
    let picked = read(msime_client_character(fixture.handle, b'1', false));
    msime_client_set_lifecycle(fixture.handle, 0);
    assert_eq!(picked["value"]["commit"], plain[0].as_str());
    assert!(take_log().is_empty());

    // 没设这个位的派发照常对调。
    let swapped = view_texts(&compose_nihao(fixture.handle));
    assert_eq!(swapped[..2], [plain[1].clone(), plain[0].clone()]);
    assert!(!take_log().is_empty());
    // 设着这个位时旁路调用也不重放：云候选到达后首行回到引擎的首行。
    msime_client_set_lifecycle(fixture.handle, COMMIT_OBSERVE_ONLY);
    let query = read(msime_client_online_query(fixture.handle))["value"].to_string();
    let candidates = serde_json::to_vec(&json!(["云候选"])).unwrap();
    let applied = read(unsafe {
        msime_client_apply_online_candidates(
            fixture.handle,
            query.as_ptr(),
            query.len(),
            candidates.as_ptr(),
            candidates.len(),
            0,
        )
    });
    msime_client_set_lifecycle(fixture.handle, 0);
    assert_eq!(applied["value"]["applied"], true, "{applied}");
    assert_eq!(view_texts(&applied["value"]["view"])[0], plain[0]);
    assert!(take_log().is_empty());
}

/// `event_scalar` 里的枚举值就是这些枚举的判别值，SDK 头文件和 `msime-pack run` 的夹具都按 `abi::scalar_values` 的名字和数值写。每个函数都是穷举的 match：枚举加了变体，这里编译不过，提醒同时在 ABI 里追加。
#[test]
fn the_abi_scalar_tables_are_the_lifecycle_discriminants() {
    use crate::lifecycle::{CommitOrigin, EndReason, InputClass, LocalModeClass};
    use msime_input_runtime::{SelectionHow, SelectionOutcome, SelectionRetraction};
    use msime_script_runtime::abi::scalar_values as values;

    fn check(table: &[(&'static str, u8)], pairs: &[(&'static str, u8)]) {
        assert_eq!(table, pairs);
    }
    let origin = |value: CommitOrigin| match value {
        CommitOrigin::Selection => "selection",
        CommitOrigin::Raw => "raw",
        CommitOrigin::HeldFlush => "held_flush",
        CommitOrigin::SchemeText => "scheme_text",
        CommitOrigin::FocusLoss => "focus_loss",
    };
    check(
        values::ORIGIN,
        &[
            CommitOrigin::Selection,
            CommitOrigin::Raw,
            CommitOrigin::HeldFlush,
            CommitOrigin::SchemeText,
            CommitOrigin::FocusLoss,
        ]
        .map(|value| (origin(value), value as u8)),
    );
    let how = |value: SelectionHow| match value {
        SelectionHow::Finish => "finish",
        SelectionHow::PunctuationFinish => "punctuation_finish",
        SelectionHow::WubiTop => "wubi_top",
        SelectionHow::SpaceOverList => "space_over_list",
        SelectionHow::Digit => "digit",
        SelectionHow::GridRaw => "grid_raw",
        SelectionHow::Select => "select",
        SelectionHow::AnyCandidate => "any_candidate",
        SelectionHow::Edge => "edge",
        SelectionHow::Highlighted => "highlighted",
        SelectionHow::WubiAuto => "wubi_auto",
    };
    check(
        values::HOW,
        &[
            SelectionHow::Finish,
            SelectionHow::PunctuationFinish,
            SelectionHow::WubiTop,
            SelectionHow::SpaceOverList,
            SelectionHow::Digit,
            SelectionHow::GridRaw,
            SelectionHow::Select,
            SelectionHow::AnyCandidate,
            SelectionHow::Edge,
            SelectionHow::Highlighted,
            SelectionHow::WubiAuto,
        ]
        .map(|value| (how(value), value as u8)),
    );
    let outcome = |value: SelectionOutcome| match value {
        SelectionOutcome::Committed => "committed",
        SelectionOutcome::Held => "held",
        SelectionOutcome::Pinned => "pinned",
        SelectionOutcome::Partial => "partial",
    };
    check(
        values::OUTCOME,
        &[
            SelectionOutcome::Committed,
            SelectionOutcome::Held,
            SelectionOutcome::Pinned,
            SelectionOutcome::Partial,
        ]
        .map(|value| (outcome(value), value as u8)),
    );
    let reason = |value: EndReason| match value {
        EndReason::Commit => "commit",
        EndReason::Cancel => "cancel",
        EndReason::Deleted => "deleted",
        EndReason::Focus => "focus",
        EndReason::Destroyed => "destroyed",
        EndReason::HostCommit => "host_commit",
        EndReason::Other => "other",
    };
    check(
        values::REASON,
        &[
            EndReason::Commit,
            EndReason::Cancel,
            EndReason::Deleted,
            EndReason::Focus,
            EndReason::Destroyed,
            EndReason::HostCommit,
            EndReason::Other,
        ]
        .map(|value| (reason(value), value as u8)),
    );
    // 本地模式的名字就是 runtime 的模式名：`LocalModeClass::of` 按名字归类，归回同一个数值。名字和是否放行文本都用穷举的 match 写，枚举加了变体这里编译不过。
    let mode_name = |value: LocalModeClass| match value {
        LocalModeClass::None => "none",
        LocalModeClass::TemporaryEnglish => "temporary_english",
        LocalModeClass::TemporaryJapanese => "temporary_japanese",
        LocalModeClass::SuperJianpin => "super_jianpin",
        LocalModeClass::Emoji => "emoji",
        LocalModeClass::Kaomoji => "kaomoji",
        LocalModeClass::Mention => "mention",
        LocalModeClass::Url => "url",
        LocalModeClass::Command => "command",
        LocalModeClass::Expression => "expression",
        LocalModeClass::Unicode => "unicode",
        LocalModeClass::DateTime => "date_time",
        LocalModeClass::QuickPhrase => "quick_phrase",
        LocalModeClass::Chaizi => "chaizi",
        LocalModeClass::Unknown => "unknown",
    };
    let allows_text = |value: LocalModeClass| match value {
        LocalModeClass::None
        | LocalModeClass::TemporaryEnglish
        | LocalModeClass::TemporaryJapanese
        | LocalModeClass::SuperJianpin
        | LocalModeClass::Emoji
        | LocalModeClass::Kaomoji => true,
        LocalModeClass::Mention
        | LocalModeClass::Url
        | LocalModeClass::Command
        | LocalModeClass::Expression
        | LocalModeClass::Unicode
        | LocalModeClass::DateTime
        | LocalModeClass::QuickPhrase
        | LocalModeClass::Chaizi
        | LocalModeClass::Unknown => false,
    };
    let modes = [
        LocalModeClass::None,
        LocalModeClass::TemporaryEnglish,
        LocalModeClass::TemporaryJapanese,
        LocalModeClass::SuperJianpin,
        LocalModeClass::Emoji,
        LocalModeClass::Kaomoji,
        LocalModeClass::Mention,
        LocalModeClass::Url,
        LocalModeClass::Command,
        LocalModeClass::Expression,
        LocalModeClass::Unicode,
        LocalModeClass::DateTime,
        LocalModeClass::QuickPhrase,
        LocalModeClass::Chaizi,
        LocalModeClass::Unknown,
    ];
    check(
        values::LOCAL_MODE,
        &modes.map(|mode| (mode_name(mode), mode as u8)),
    );
    for mode in modes {
        let name = mode_name(mode);
        if mode != LocalModeClass::Unknown {
            assert_eq!(LocalModeClass::of(name), mode, "{name}");
        }
        assert_eq!(
            msime_script_runtime::abi::local_mode_allows_text(mode as u8),
            allows_text(mode),
            "{name}"
        );
    }
    let class = |value: InputClass| match value {
        InputClass::Character => "character",
        InputClass::Punctuation => "punctuation",
        InputClass::Selection => "selection",
        InputClass::Navigation => "navigation",
        InputClass::Edit => "edit",
        InputClass::Commit => "commit",
        InputClass::Cancel => "cancel",
        InputClass::HostCommit => "host_commit",
        InputClass::Other => "other",
        InputClass::FocusLoss => "focus_loss",
    };
    check(
        values::INPUT_CLASS,
        &[
            InputClass::Character,
            InputClass::Punctuation,
            InputClass::Selection,
            InputClass::Navigation,
            InputClass::Edit,
            InputClass::Commit,
            InputClass::Cancel,
            InputClass::HostCommit,
            InputClass::Other,
            InputClass::FocusLoss,
        ]
        .map(|value| (class(value), value as u8)),
    );
    let retraction = |value: SelectionRetraction| match value {
        SelectionRetraction::Restore => "restore",
        SelectionRetraction::Drop => "drop",
    };
    check(
        values::RETRACTION,
        &[SelectionRetraction::Restore, SelectionRetraction::Drop]
            .map(|value| (retraction(value), value as u8)),
    );
}

/// 像 Windows 的原生设置应用那样调 `msime_client_plugins`：带 `script_page` 和输入法的 `user_data`。`script_page` 为假时就是 Android、HarmonyOS 的调用，两项都不带。
fn plugins_request(fixture: &Fixture, script_page: bool, action: Value) -> Value {
    let mut request = json!({
        "state_root": fixture.plugins.parent().unwrap(),
        "sound_packs": null,
        "action": action,
    });
    if script_page {
        request["script_page"] = json!(true);
        request["user_data"] = json!(fixture.user_data);
    }
    let bytes = serde_json::to_vec(&request).unwrap();
    read(unsafe { msime_client_plugins(bytes.as_ptr(), bytes.len()) })
}

/// 在插件根目录旁边写一个脚本包的源文件夹，`reads_commit` 时申请 `read_commit`。
fn script_source(fixture: &Fixture, id: &str, reads_commit: bool) -> PathBuf {
    let source = fixture.plugins.parent().unwrap().join("source").join(id);
    std::fs::create_dir_all(&source).unwrap();
    let permissions = if reads_commit { "'read_commit'" } else { "" };
    let manifest = format!("schema_version = 2\nkind = 'script'\nid = '{id}'\nname = '记录'\nversion = '1.0.0'\nlicense = 'MIT'\npermissions = [{permissions}]\n[script]\nabi = 1\nmodule = 'main.wasm'\nhooks = ['commit']\nmemory_pages = 1\n");
    std::fs::write(source.join(MANIFEST_FILE), manifest).unwrap();
    std::fs::write(source.join("main.wasm"), logger_binary_with(reads_commit)).unwrap();
    source
}

/// 目录里脚本插件的 id 和问题行的文件夹。
fn listed_scripts(catalog: &Value) -> (Vec<String>, Vec<String>) {
    assert_eq!(catalog["ok"], true, "{catalog}");
    let named = |key: &str, field: &str| -> Vec<String> {
        catalog["value"][key]
            .as_array()
            .unwrap()
            .iter()
            .filter(|item| item["kind"] == "script")
            .map(|item| item[field].as_str().unwrap().to_owned())
            .collect()
    };
    (named("packages", "id"), named("issues", "folder"))
}

#[test]
fn only_a_host_with_the_grant_page_sees_or_installs_script_packs() {
    let fixture = fixture();
    let source = script_source(&fixture, "reader", true);
    // 没有授权页的宿主（Android、HarmonyOS 的调用）：release 构建在安装之前拒绝，开发构建装得上。
    let refused = plugins_request(
        &fixture,
        false,
        json!({"operation": "import", "source": source}),
    );
    if cfg!(debug_assertions) {
        assert_eq!(refused["ok"], true, "{refused}");
    } else {
        assert_eq!(refused["error"], "plugin_invalid", "{refused}");
        assert!(!fixture.plugins.join("script/reader").exists());
    }
    // 有授权页的宿主（Windows 的原生设置应用）：开发构建和 release 构建都接受，带着钩子、申请的能力和内容哈希。
    let imported = plugins_request(
        &fixture,
        true,
        json!({"operation": "import", "source": source}),
    );
    assert_eq!(imported["ok"], true, "{imported}");
    assert_eq!(imported["value"]["kind"], "script");
    assert_eq!(imported["value"]["hooks"], json!(["commit"]));
    assert_eq!(imported["value"]["permissions"], json!(["read_commit"]));
    assert_eq!(
        imported["value"]["content_hash"].as_str().map(str::len),
        Some(64)
    );
    // 一个载入不了的脚本文件夹，问题行同样只给有授权页的宿主。
    let broken = fixture.plugins.join("script/broken");
    std::fs::create_dir_all(&broken).unwrap();
    std::fs::write(broken.join(MANIFEST_FILE), "schema_version = 2\n").unwrap();

    let catalog = json!({"operation": "catalog"});
    assert_eq!(
        listed_scripts(&plugins_request(&fixture, true, catalog.clone())),
        (vec!["reader".to_owned()], vec!["broken".to_owned()])
    );
    assert_eq!(
        listed_scripts(&plugins_request(&fixture, false, catalog)),
        (Vec::new(), Vec::new())
    );
}

#[test]
fn the_grant_page_actions_grant_reorder_and_revoke_and_the_input_method_honours_them() {
    let fixture = fixture();
    let mut hashes = Vec::new();
    for (id, reads_commit) in [("reader", true), ("watcher", false)] {
        let imported = plugins_request(
            &fixture,
            true,
            json!({"operation": "import", "source": script_source(&fixture, id, reads_commit)}),
        );
        assert_eq!(imported["ok"], true, "{imported}");
        hashes.push(
            imported["value"]["content_hash"]
                .as_str()
                .unwrap()
                .to_owned(),
        );
    }
    let (reader, watcher) = (&hashes[0], &hashes[1]);
    let grants =
        |fixture: &Fixture| plugins_request(fixture, true, json!({"operation": "script_grants"}));
    let empty = grants(&fixture);
    assert_eq!(empty["ok"], true, "{empty}");
    assert_eq!(empty["value"]["plugins"], json!([]));
    assert_eq!(empty["value"]["health"], json!([]));

    let set = |id: &str, hash: &str, enabled: bool, granted: Value| {
        plugins_request(
            &fixture,
            true,
            json!({"operation": "set_script_grant", "id": id, "hash": hash, "enabled": enabled, "granted": granted}),
        )
    };
    let first = set("reader", reader, true, json!(["read_commit"]));
    assert_eq!(first["ok"], true, "{first}");
    assert_eq!(
        first["value"]["plugins"],
        json!([{"id": "reader", "hash": reader, "enabled": true, "granted": ["read_commit"]}])
    );
    let mut revision = first["value"]["revision"].as_u64().unwrap();
    let second = set("watcher", watcher, true, json!([]));
    assert_eq!(second["ok"], true, "{second}");
    assert!(second["value"]["revision"].as_u64().unwrap() > revision);
    revision = second["value"]["revision"].as_u64().unwrap();

    // 宿主的拒绝原样带着桌面外壳的失败码：不认识的能力、清单没申请的能力、页面展示之后被换过的包。
    assert_eq!(
        set("reader", reader, true, json!(["read_everything"]))["error"],
        "plugin_script_grant_invalid"
    );
    assert_eq!(
        set("watcher", watcher, true, json!(["read_commit"]))["error"],
        "plugin_script_grant_invalid"
    );
    assert_eq!(
        set("reader", watcher, true, json!(["read_commit"]))["error"],
        "plugin_script_changed"
    );
    assert_eq!(
        set("nobody", reader, true, json!([]))["error"],
        "plugin_script_missing"
    );

    // 执行顺序：给出的 id 排到最前；重复或没有条目的 id 整个拒绝。
    let reorder = |ids: Value| {
        plugins_request(
            &fixture,
            true,
            json!({"operation": "reorder_scripts", "ids": ids}),
        )
    };
    let reordered = reorder(json!(["watcher"]));
    assert_eq!(reordered["ok"], true, "{reordered}");
    let order = |view: &Value| -> Vec<String> {
        view["value"]["plugins"]
            .as_array()
            .unwrap()
            .iter()
            .map(|grant| grant["id"].as_str().unwrap().to_owned())
            .collect()
    };
    assert_eq!(order(&reordered), ["watcher", "reader"]);
    assert!(reordered["value"]["revision"].as_u64().unwrap() > revision);
    assert_eq!(
        reorder(json!(["reader", "reader"]))["error"],
        "plugin_script_grant_invalid"
    );
    assert_eq!(
        reorder(json!(["nobody"]))["error"],
        "plugin_script_grant_invalid"
    );
    assert_eq!(order(&grants(&fixture)), ["watcher", "reader"]);

    // 回答带上 `user_data` 下输入法记下的停用；不带 `user_data` 时没有。
    let health = fixture.user_data.join(script_grants::HEALTH_FILE);
    std::fs::write(
        &health,
        format!(r#"{{"format_version":1,"plugins":[{{"id":"reader","hash":"{reader}","disabled":"slow"}}]}}"#),
    )
    .unwrap();
    assert_eq!(
        grants(&fixture)["value"]["health"],
        json!([{"id": "reader", "hash": reader, "disabled": "slow"}])
    );
    let raw = |user_data: Value| {
        let request = serde_json::to_vec(&json!({
            "state_root": fixture.plugins.parent().unwrap(),
            "sound_packs": null,
            "script_page": true,
            "user_data": user_data,
            "action": {"operation": "script_grants"},
        }))
        .unwrap();
        read(unsafe { msime_client_plugins(request.as_ptr(), request.len()) })
    };
    assert_eq!(raw(Value::Null)["value"]["health"], json!([]));
    assert_eq!(raw(json!("relative"))["error"], "invalid");
    std::fs::remove_file(&health).unwrap();

    // 输入法按页面写下的授权运行：两个插件都在上屏时收到事件。
    opt_in(&fixture, TRAIT_KNOWN);
    wait_for(|| {
        assert_eq!(commit_nihao(fixture.handle), "你好");
        (take_log().len() >= 2).then_some(())
    });

    // 撤销：停用并清空授予项，条目留着，位置不变。
    let revoked = set("reader", reader, false, json!([]));
    assert_eq!(revoked["ok"], true, "{revoked}");
    assert_eq!(
        revoked["value"]["plugins"][1],
        json!({"id": "reader", "hash": reader, "enabled": false, "granted": []})
    );
}
