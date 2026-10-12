//! 脚本插件经 C ABI 的行为：运行条件和隐私门控、按授权的文本、每次 focus 重建实例、每插件配额、按轮的 fuel、熔断与健康文件、panic 总开关、后台编译线程，以及不投递的事件。
//!
//! 模块用 WAT 写（`msime-script-runtime` 的 `test-support`），编译好直接放进进程缓存，哈希每个测试各不相同；只有后台编译线程那一例从磁盘上的包走完整路径，模块按字节手写。脚本经 `log` 把看到的东西写出来：级别是事件类型或 import 的返回码，文本是读到的文本。测试线程有自己的进程开关和 panic 总开关，互不影响。

use super::*;
use crate::lifecycle::{
    LifecycleEvent, KEY_PASSTHROUGH, TRAIT_ELEVATED, TRAIT_KNOWN, TRAIT_PRIVATE, TRAIT_SECURE,
};
use crate::scripts::{
    insert_ready, TEST_COMPILE_PANIC_ID, TEST_PANIC, TEST_PANICKED, TEST_SCRIPT_LOG, TEST_SLOW,
    TEST_SWITCH,
};
use msime_client_core::plugins::{import, script_grants, MANIFEST_FILE};
use msime_script_runtime::abi::commit_field;
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

/// 空转 `rounds` 圈再写出事件类型。
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
    let options = json!({ "api_version": 1, "resources": resources, "user_data": user_data, "cache": path("cache"), "dictionaries": dictionaries, "preferences_directory": state, "preferences": chinese_preferences() }).to_string();
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
    // 开发构建和 release 构建（`cargo test --release`）一样：macOS 和 Linux 上打开就投递，授权文件不签名，本进程也没有任何密钥。Windows 的原生设置应用还没有授权页，那里只有开发构建打开。
    let runs = !cfg!(all(target_os = "windows", not(debug_assertions)));
    assert_eq!(msime_client_scripts_enable(true), runs);
    commit_nihao(fixture.handle);
    assert_eq!(!take_log().is_empty(), runs);
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
    // 第一键：input, start, update, shown；之后四键各 input, update, shown；空格选首选：input, selected, commit, end, hidden。before_commit 第 4 阶段才投递。
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
        &[Script::new("burner", burner(2000), &[Hook::Input])],
        1,
    );
    opt_in(&fixture, TRAIT_KNOWN);
    // 每次 input 花几千 fuel，单次远低于观察类钩子的 25 000 上限。
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
        &[Script::new("burner", burner(2000), &[Hook::Input])],
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
        &[Script::new("burner", burner(2000), &[Hook::Input])],
        1,
    );
    opt_in(&fixture, TRAIT_KNOWN);
    // 修饰键组合、快捷键和失焦命令不经过 msime_client_key，这些派发和上一次按键共用一轮。每次聚焦都把这一轮花光：插件只是被跳过，连着四次也不熔断（按失败计时第三次就停用了）。
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
    // 类型：(i32 i32 i32) -> ()、() -> i32、(i32) -> i32。
    section(
        1,
        &[
            3, 0x60, 3, 0x7f, 0x7f, 0x7f, 0, 0x60, 0, 1, 0x7f, 0x60, 1, 0x7f, 1, 0x7f,
        ],
        &mut out,
    );
    let mut imports = vec![1];
    name("msime", &mut imports);
    name("log", &mut imports);
    imports.extend_from_slice(&[0x00, 0]);
    section(2, &imports, &mut out);
    section(3, &[2, 1, 2], &mut out);
    section(5, &[1, 0x00, 1], &mut out);
    let mut exports = vec![3];
    for (export, kind, index) in [
        ("memory", 2, 0),
        ("msime_abi", 0, 1),
        ("msime_on_event", 0, 2),
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
            let scripts = state.scripts_mut();
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
