//! `msime-pack run`：金样输出（fuel 是固定值）、退出码、夹具和 `--grant` 的错误，以及经命令行从真实的包走完整路径。
//!
//! 模块用 WAT 写（`msime-script-runtime` 的 `test-support`），和 host-api 的脚本插件测试一样；走命令行的那几例要一个真的 `.wasm` 文件，按字节手写。仓库不提交 `.wasm`。fuel 的数值取决于 wasmi 的计量，升级 wasmi 时这里的金样会跟着变，那正是要看的东西。

#![cfg(any(
    target_os = "macos",
    target_os = "windows",
    all(target_os = "linux", not(target_env = "ohos"))
))]

use std::ffi::OsString;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use msime_pack_tool::run::{parse_fixture, run_module, GrantSpec};
use msime_pack_tool::{run, EXIT_INVALID, EXIT_OK, EXIT_USAGE};
use msime_script_runtime::{compile_text, Capability, CapabilitySet, Declaration, Hook};

/// 改写类插件：`before_commit` 读 body、记下来、请求改写；每个事件记下这个实例见过几个事件（个位数）。`msime_init` 记一行 `init`。
const REWRITER: &str = r#"(module
  (import "msime" "commit_text" (func $commit_text (param i32 i32 i32) (result i32)))
  (import "msime" "rewrite_commit" (func $rewrite (param i32 i32) (result i32)))
  (import "msime" "log" (func $log (param i32 i32 i32)))
  (memory (export "memory") 1)
  (global $seen (export "seen") (mut i32) (i32.const 0))
  (data (i32.const 0) "init")
  (data (i32.const 16) "absent")
  (data (i32.const 32) "denied")
  (func (export "msime_abi") (result i32) (i32.const 1))
  (func (export "msime_init") (result i32)
    (call $log (i32.const 1) (i32.const 0) (i32.const 4))
    (i32.const 0))
  (func (export "msime_on_event") (param $kind i32) (result i32) (local $n i32)
    (global.set $seen (i32.add (global.get $seen) (i32.const 1)))
    (i32.store8 (i32.const 512) (i32.add (i32.const 48) (i32.rem_u (global.get $seen) (i32.const 10))))
    (call $log (i32.const 0) (i32.const 512) (i32.const 1))
    (if (i32.eq (local.get $kind) (i32.const 9))
      (then
        (local.set $n (call $commit_text (i32.const 2) (i32.const 1024) (i32.const 256)))
        (if (i32.eq (local.get $n) (i32.const -2))
          (then (call $log (i32.const 2) (i32.const 16) (i32.const 6)) (return (i32.const 0))))
        (if (i32.eq (local.get $n) (i32.const -1))
          (then (call $log (i32.const 2) (i32.const 32) (i32.const 6)) (return (i32.const 0))))
        (call $log (i32.const 1) (i32.const 1024) (local.get $n))
        (drop (call $rewrite (i32.const 1024) (local.get $n)))))
    (i32.const 0)))"#;

/// 出错的插件：`input` 的类别是取消时 `unreachable`，`selection_retracted` 返回负数，`commit` 死循环，`composition_update` 每次转 2000 圈。
const FAILING: &str = r#"(module
  (import "msime" "event_scalar" (func $scalar (param i32) (result i64)))
  (memory (export "memory") 1)
  (func (export "msime_abi") (result i32) (i32.const 1))
  (func (export "msime_on_event") (param $kind i32) (result i32) (local $i i32)
    (if (i32.eq (local.get $kind) (i32.const 6))
      (then (if (i64.eq (call $scalar (i32.const 6)) (i64.const 6)) (then unreachable))))
    (if (i32.eq (local.get $kind) (i32.const 7)) (then (return (i32.const -1))))
    (if (i32.eq (local.get $kind) (i32.const 10)) (then (loop $forever (br $forever))))
    (if (i32.eq (local.get $kind) (i32.const 13))
      (then (loop $spin
        (local.set $i (i32.add (local.get $i) (i32.const 1)))
        (br_if $spin (i32.lt_u (local.get $i) (i32.const 2000))))))
    (i32.const 0)))"#;

/// `msime_init` 返回负数。
const INIT_FAILS: &str = r#"(module
  (memory (export "memory") 1)
  (func (export "msime_abi") (result i32) (i32.const 1))
  (func (export "msime_init") (result i32) (i32.const -1))
  (func (export "msime_on_event") (param $kind i32) (result i32) (i32.const 0)))"#;

/// 指令插件：每次 `command` 先给出一行 `abc`（注释 `n`），参数不为空时再把参数作为一行，最后取 1 字节随机数、模 26 换成小写字母作为一行。
const COMMANDER: &str = r#"(module
  (import "msime" "command_text" (func $text (param i32 i32 i32) (result i32)))
  (import "msime" "command_row" (func $row (param i32 i32 i32 i32) (result i32)))
  (import "msime" "random_fill" (func $random (param i32 i32) (result i32)))
  (memory (export "memory") 1)
  (data (i32.const 0) "abcn")
  (func (export "msime_abi") (result i32) (i32.const 1))
  (func (export "msime_on_event") (param $kind i32) (result i32) (local $n i32)
    (drop (call $row (i32.const 0) (i32.const 3) (i32.const 3) (i32.const 1)))
    (local.set $n (call $text (i32.const 1) (i32.const 256) (i32.const 64)))
    (if (i32.gt_s (local.get $n) (i32.const 0))
      (then (drop (call $row (i32.const 256) (local.get $n) (i32.const 0) (i32.const 0)))))
    (if (i32.eq (call $random (i32.const 512) (i32.const 1)) (i32.const 1))
      (then
        (i32.store8 (i32.const 513)
          (i32.add (i32.const 97) (i32.rem_u (i32.load8_u (i32.const 512)) (i32.const 26))))
        (drop (call $row (i32.const 513) (i32.const 1) (i32.const 0) (i32.const 0)))))
    (i32.const 0)))"#;

fn declaration(capabilities: &[Capability], hooks: &[Hook]) -> Declaration {
    Declaration {
        capabilities: capabilities.iter().copied().collect(),
        hooks: hooks.iter().copied().collect(),
        memory_pages: 1,
    }
}

/// 编译 WAT，按 `grants` 运行夹具，返回退出码和输出。
fn run_wat(
    wat: &str,
    declared: Declaration,
    grants: CapabilitySet,
    fixture: &str,
) -> (i32, String) {
    let module = compile_text(wat, declared).expect("module compiles");
    let steps = parse_fixture(fixture).expect("fixture parses");
    let mut out = Vec::new();
    let status = run_module(&module, grants, &steps, &mut out);
    (status, String::from_utf8(out).unwrap())
}

/// 逐行比较，失败时打出完整的实际输出，方便更新金样。
fn assert_golden(actual: &str, expected: &str) {
    assert_eq!(
        actual.lines().collect::<Vec<_>>(),
        expected.lines().collect::<Vec<_>>(),
        "actual output:\n{actual}"
    );
}

const REWRITER_HOOKS: [Hook; 3] = [Hook::FocusIn, Hook::BeforeCommit, Hook::Commit];
const REWRITER_CAPABILITIES: [Capability; 2] = [Capability::ReadCommit, Capability::RewriteCommit];

#[test]
fn a_rewriter_is_shown_its_snapshot_its_grants_and_its_rewrite() {
    let declared = declaration(&REWRITER_CAPABILITIES, &REWRITER_HOOKS);
    let fixture = r#"{"event":"focus_in"}
{"event":"commit","key":true,"origin":"selection","body":"你好"}
{"event":"before_commit","key":true,"origin":"selection","body":"用Rust写","tail":"。"}
{"event":"commit","origin":"selection","body":"用Rust写","tail":"。"}
{"event":"before_commit","key":true,"origin":"raw","local_mode":"url","body":"example.com"}
{"event":"input","key":true,"input_class":"character","input_char":"a"}
{"event":"focus_out"}
{"event":"focus_in"}
"#;
    let (status, out) = run_wat(REWRITER, declared, declared.capabilities, fixture);
    assert_eq!(status, EXIT_OK);
    assert_golden(
        &out,
        r#"{"seq":1,"event":"focus_in","status":"ok","fuel":121,"init_fuel":109,"actions":[],"log":[{"level":"info","text":"init"},{"level":"debug","text":"1"}]}
{"seq":2,"event":"commit","status":"ok","fuel":121,"actions":[],"log":[{"level":"debug","text":"2"}]}
{"seq":3,"event":"before_commit","status":"ok","fuel":448,"actions":[{"import":"rewrite_commit","result":"ok"}],"log":[{"level":"debug","text":"1"},{"level":"info","text":"用Rust写"}]}
{"seq":4,"event":"commit","status":"ok","fuel":121,"actions":[],"log":[{"level":"debug","text":"3"}]}
{"seq":5,"event":"before_commit","status":"ok","fuel":349,"actions":[],"log":[{"level":"debug","text":"1"},{"level":"warn","text":"absent"}]}
{"seq":6,"event":"input","status":"not_subscribed","fuel":0,"actions":[],"log":[]}
{"seq":7,"event":"focus_out","status":"not_subscribed","fuel":0,"actions":[],"log":[]}
{"seq":8,"event":"focus_in","status":"ok","fuel":121,"init_fuel":109,"actions":[],"log":[{"level":"info","text":"init"},{"level":"debug","text":"1"}]}
"#,
    );
}

#[test]
fn without_the_grant_the_text_is_denied_and_the_rest_runs_the_same() {
    let declared = declaration(&REWRITER_CAPABILITIES, &REWRITER_HOOKS);
    let fixture = r#"{"event":"before_commit","key":true,"origin":"selection","body":"用Rust写"}
"#;
    let (status, out) = run_wat(REWRITER, declared, CapabilitySet::EMPTY, fixture);
    assert_eq!(status, EXIT_OK);
    assert_golden(
        &out,
        r#"{"seq":1,"event":"before_commit","status":"ok","fuel":349,"init_fuel":109,"actions":[],"log":[{"level":"info","text":"init"},{"level":"debug","text":"1"},{"level":"warn","text":"denied"}]}
"#,
    );
}

/// 每种失败都报告用掉的 fuel，并让退出码为 1。第 12 行被本轮配额截断（六次 `composition_update` 之后只剩 16 886），这一次用掉的超过 `composition_update` 上限 25 000 的一半，和超出上限一样算 `out_of_fuel`；第 15 行是超出 `commit` 自己的上限。
#[test]
fn failures_are_reported_with_their_fuel_and_fail_the_run() {
    let hooks = [
        Hook::FocusIn,
        Hook::Input,
        Hook::SelectionRetracted,
        Hook::Commit,
        Hook::CompositionUpdate,
    ];
    let declared = declaration(&[], &hooks);
    let fixture = r#"{"event":"input","key":true,"input_class":"character"}
{"event":"selection_retracted","retraction":"restore"}
{"event":"input","key":true,"input_class":"cancel"}
{"event":"input","input_class":"character"}
{"event":"focus_in"}
{"event":"composition_update","key":true,"preedit":"n"}
{"event":"composition_update","preedit":"ni"}
{"event":"composition_update","preedit":"nih"}
{"event":"composition_update","preedit":"niha"}
{"event":"composition_update","preedit":"nihao"}
{"event":"composition_update","preedit":"nihao"}
{"event":"composition_update","preedit":"nihao"}
{"event":"composition_update","key":true,"preedit":"nihao"}
{"event":"focus_in"}
{"event":"commit","key":true,"origin":"selection","body":"你好"}
"#;
    let (status, out) = run_wat(FAILING, declared, CapabilitySet::EMPTY, fixture);
    assert_eq!(status, EXIT_INVALID);
    assert_golden(
        &out,
        r#"{"seq":1,"event":"input","status":"ok","fuel":124,"init_fuel":2,"actions":[],"log":[]}
{"seq":2,"event":"selection_retracted","status":"negative_return","fuel":20,"actions":[],"log":[]}
{"seq":3,"event":"input","status":"unreachable","fuel":125,"actions":[],"log":[]}
{"seq":4,"event":"input","status":"suspended","fuel":0,"actions":[],"log":[]}
{"seq":5,"event":"focus_in","status":"ok","fuel":18,"init_fuel":2,"actions":[],"log":[]}
{"seq":6,"event":"composition_update","status":"ok","fuel":18019,"actions":[],"log":[]}
{"seq":7,"event":"composition_update","status":"ok","fuel":18019,"actions":[],"log":[]}
{"seq":8,"event":"composition_update","status":"ok","fuel":18019,"actions":[],"log":[]}
{"seq":9,"event":"composition_update","status":"ok","fuel":18019,"actions":[],"log":[]}
{"seq":10,"event":"composition_update","status":"ok","fuel":18019,"actions":[],"log":[]}
{"seq":11,"event":"composition_update","status":"ok","fuel":18019,"actions":[],"log":[]}
{"seq":12,"event":"composition_update","status":"out_of_fuel","fuel":16885,"actions":[],"log":[]}
{"seq":13,"event":"composition_update","status":"suspended","fuel":0,"actions":[],"log":[]}
{"seq":14,"event":"focus_in","status":"ok","fuel":18,"init_fuel":2,"actions":[],"log":[]}
{"seq":15,"event":"commit","status":"out_of_fuel","fuel":24999,"actions":[],"log":[]}
"#,
    );
}

/// `before_commit` 在一次性的快照实例上运行，它 trap 不会让持久实例失效：之后的 `commit` 照常投递，不是 `suspended`。作者文档的「失败」一节按这条写。
#[test]
fn a_before_commit_trap_leaves_the_instance_usable() {
    const TRAPS_BEFORE_COMMIT: &str = r#"(module
  (memory (export "memory") 1)
  (func (export "msime_abi") (result i32) (i32.const 1))
  (func (export "msime_on_event") (param $kind i32) (result i32)
    (if (i32.eq (local.get $kind) (i32.const 9)) (then unreachable))
    (i32.const 0)))"#;
    let declared = declaration(&[], &[Hook::BeforeCommit, Hook::Commit]);
    let fixture = r#"{"event":"before_commit","key":true,"origin":"selection","body":"x"}
{"event":"commit","origin":"selection","body":"x"}
{"event":"before_commit","key":true,"origin":"selection","body":"y"}
"#;
    let (status, out) = run_wat(TRAPS_BEFORE_COMMIT, declared, CapabilitySet::EMPTY, fixture);
    assert_eq!(status, EXIT_INVALID);
    let statuses: Vec<String> = out
        .lines()
        .map(|line| {
            let report: serde_json::Value = serde_json::from_str(line).unwrap();
            report["status"].as_str().unwrap().to_owned()
        })
        .collect();
    assert_eq!(statuses, ["unreachable", "ok", "unreachable"]);
}

#[test]
fn an_initialisation_failure_disables_the_plugin() {
    let declared = declaration(&[], &[Hook::Commit]);
    let fixture = r#"{"event":"commit","key":true,"origin":"selection","body":"x"}
{"event":"focus_in"}
{"event":"commit","key":true,"origin":"selection","body":"x"}
"#;
    let (status, out) = run_wat(INIT_FAILS, declared, CapabilitySet::EMPTY, fixture);
    assert_eq!(status, EXIT_INVALID);
    assert_golden(
        &out,
        r#"{"seq":1,"event":"commit","status":"init:init_failed","fuel":0,"init_fuel":4,"actions":[],"log":[]}
{"seq":2,"event":"focus_in","status":"disabled","fuel":0,"actions":[],"log":[]}
{"seq":3,"event":"commit","status":"disabled","fuel":0,"actions":[],"log":[]}
"#,
    );
}

/// 同一轮里重建的实例接着用这一轮剩下的配额：`composition_update` 空转 2 200 圈，六次用掉 118 914，`focus_in` 丢掉实例再重建（花掉 18），第七次只剩 6 068，本轮配额耗尽。被截断的这一次用掉的不到 `composition_update` 上限 25 000 的一半，是 `turn_exhausted`，不算失败，所以退出码是 0（用掉一半以上就是 `out_of_fuel`，见 [`failures_are_reported_with_their_fuel_and_fail_the_run`]）；带 `key` 的 `focus_in` 开始新的一轮，重建的实例拿到满额。
#[test]
fn an_instance_rebuilt_in_the_same_turn_resumes_its_fuel() {
    let module = FAILING.replace("(i32.const 2000)", "(i32.const 2200)");
    assert_ne!(module, FAILING);
    let declared = declaration(&[], &[Hook::FocusIn, Hook::CompositionUpdate]);
    let fixture = r#"{"event":"composition_update","key":true,"preedit":"n"}
{"event":"composition_update","preedit":"ni"}
{"event":"composition_update","preedit":"nih"}
{"event":"composition_update","preedit":"niha"}
{"event":"composition_update","preedit":"nihao"}
{"event":"composition_update","preedit":"nihao"}
{"event":"focus_in"}
{"event":"composition_update","preedit":"nihao"}
{"event":"focus_in","key":true}
{"event":"composition_update","preedit":"n"}
"#;
    let (status, out) = run_wat(&module, declared, CapabilitySet::EMPTY, fixture);
    assert_eq!(status, EXIT_OK);
    assert_golden(
        &out,
        r#"{"seq":1,"event":"composition_update","status":"ok","fuel":19819,"init_fuel":2,"actions":[],"log":[]}
{"seq":2,"event":"composition_update","status":"ok","fuel":19819,"actions":[],"log":[]}
{"seq":3,"event":"composition_update","status":"ok","fuel":19819,"actions":[],"log":[]}
{"seq":4,"event":"composition_update","status":"ok","fuel":19819,"actions":[],"log":[]}
{"seq":5,"event":"composition_update","status":"ok","fuel":19819,"actions":[],"log":[]}
{"seq":6,"event":"composition_update","status":"ok","fuel":19819,"actions":[],"log":[]}
{"seq":7,"event":"focus_in","status":"ok","fuel":18,"init_fuel":2,"actions":[],"log":[]}
{"seq":8,"event":"composition_update","status":"turn_exhausted","fuel":6067,"actions":[],"log":[]}
{"seq":9,"event":"focus_in","status":"ok","fuel":18,"init_fuel":2,"actions":[],"log":[]}
{"seq":10,"event":"composition_update","status":"ok","fuel":19819,"actions":[],"log":[]}
"#,
    );
}

/// `msime_init` 转 `ITERATIONS` 圈，每圈 9 fuel，加上 4 的固定开销。单次调用故意压在六万以内：dev 构建里 wasmi 的 tail-call 分派不成立，每条指令都占栈，测试线程的 2 MiB 栈在八万左右就溢出。
const SLOW_INIT: &str = r#"(module
  (memory (export "memory") 1)
  (func (export "msime_abi") (result i32) (i32.const 1))
  (func (export "msime_init") (result i32) (local $i i32)
    (loop $spin
      (local.set $i (i32.add (local.get $i) (i32.const 1)))
      (br_if $spin (i32.lt_u (local.get $i) (i32.const ITERATIONS))))
    (i32.const 0))
  (func (export "msime_on_event") (param $kind i32) (result i32) (i32.const 0)))"#;

/// 同一轮里反复 `focus_out` 再投递，每次都重建实例，初始化合计逐次扣减。剩下的预算不够一次初始化时是 `deferred`，正好用完时不再尝试，也是 `deferred`；两者都不算失败，也不停用；下一轮给满预算，照常实例化。
#[test]
fn an_exhausted_initialisation_budget_defers_until_the_next_turn() {
    // (圈数, 每次初始化的 fuel, focus_out 再投递的次数, 最后四行)
    let cases = [
        // 45 004 × 111 之后剩 4 556，第 112 次初始化做到一半没了预算；剩下的 7 在下一次只够调 `msime_abi`。
        (
            5000,
            45_004,
            111,
            r#"{"seq":222,"event":"focus_out","status":"not_subscribed","fuel":0,"actions":[],"log":[]}
{"seq":223,"event":"commit","status":"deferred","fuel":0,"init_fuel":4549,"actions":[],"log":[]}
{"seq":224,"event":"commit","status":"deferred","fuel":0,"init_fuel":4,"actions":[],"log":[]}
{"seq":225,"event":"commit","status":"ok","fuel":2,"init_fuel":45004,"actions":[],"log":[]}"#,
        ),
        // 40 000 × 125 正好用完五百万，之后连实例化都不尝试。
        (
            4444,
            40_000,
            125,
            r#"{"seq":250,"event":"focus_out","status":"not_subscribed","fuel":0,"actions":[],"log":[]}
{"seq":251,"event":"commit","status":"deferred","fuel":0,"actions":[],"log":[]}
{"seq":252,"event":"commit","status":"deferred","fuel":0,"actions":[],"log":[]}
{"seq":253,"event":"commit","status":"ok","fuel":2,"init_fuel":40000,"actions":[],"log":[]}"#,
        ),
    ];
    for (iterations, init_fuel, rebuilds, tail) in cases {
        let declared = declaration(&[], &[Hook::Commit]);
        let wat = SLOW_INIT.replace("ITERATIONS", &iterations.to_string());
        let mut fixture = String::from(
            "{\"event\":\"commit\",\"key\":true,\"origin\":\"selection\",\"body\":\"x\"}\n",
        );
        for _ in 0..rebuilds {
            fixture.push_str("{\"event\":\"focus_out\"}\n");
            fixture.push_str("{\"event\":\"commit\",\"origin\":\"selection\",\"body\":\"y\"}\n");
        }
        fixture.push_str("{\"event\":\"commit\",\"origin\":\"selection\",\"body\":\"z\"}\n");
        fixture.push_str(
            "{\"event\":\"commit\",\"key\":true,\"origin\":\"selection\",\"body\":\"w\"}\n",
        );
        let (status, out) = run_wat(&wat, declared, CapabilitySet::EMPTY, &fixture);
        assert_eq!(status, EXIT_OK);
        let lines: Vec<&str> = out.lines().collect();
        assert_eq!(lines.len(), 1 + 2 * rebuilds + 2);
        // 前面每次实例化都成功，用的初始化 fuel 一样。
        for (index, line) in lines[..lines.len() - 4].iter().enumerate() {
            let expected = if index % 2 == 0 {
                format!(
                    r#"{{"seq":{},"event":"commit","status":"ok","fuel":2,"init_fuel":{init_fuel},"actions":[],"log":[]}}"#,
                    index + 1
                )
            } else {
                format!(
                    r#"{{"seq":{},"event":"focus_out","status":"not_subscribed","fuel":0,"actions":[],"log":[]}}"#,
                    index + 1
                )
            };
            assert_eq!(*line, expected);
        }
        assert_golden(&lines[lines.len() - 4..].join("\n"), tail);
    }
}

/// `command` 事件：输出多一个 `rows`，每行是 `[文本, 注释]`；随机数取自夹具的 `random`，不写是全 0；别的事件没有 `rows`。没授权时两个指令类 import 都是 `denied`，`rows` 为空，随机数照样取得到。
#[test]
fn a_command_plugin_shows_its_rows() {
    let declared = declaration(&[Capability::ProvideCommands], &[Hook::Command]);
    let fixture = format!(
        r#"{{"event":"command","key":true,"trigger":"uuid"}}
{{"event":"command","trigger":"uuid","argument":"xyz","random":"19{}"}}
{{"event":"input","key":true,"input_class":"character"}}
"#,
        "00".repeat(63)
    );
    let (status, out) = run_wat(COMMANDER, declared, declared.capabilities, &fixture);
    assert_eq!(status, EXIT_OK);
    assert_golden(
        &out,
        r#"{"seq":1,"event":"command","status":"ok","fuel":437,"init_fuel":2,"actions":[{"import":"command_row","result":"ok"},{"import":"command_row","result":"ok"}],"log":[],"rows":[["abc","n"],["a",""]]}
{"seq":2,"event":"command","status":"ok","fuel":543,"actions":[{"import":"command_row","result":"ok"},{"import":"command_row","result":"ok"},{"import":"command_row","result":"ok"}],"log":[],"rows":[["abc","n"],["xyz",""],["z",""]]}
{"seq":3,"event":"input","status":"not_subscribed","fuel":0,"actions":[],"log":[]}
"#,
    );
    let (status, out) = run_wat(COMMANDER, declared, CapabilitySet::EMPTY, &fixture);
    assert_eq!(status, EXIT_OK);
    assert_golden(
        &out,
        r#"{"seq":1,"event":"command","status":"ok","fuel":436,"init_fuel":2,"actions":[{"import":"command_row","result":"denied"},{"import":"command_row","result":"denied"}],"log":[],"rows":[]}
{"seq":2,"event":"command","status":"ok","fuel":436,"actions":[{"import":"command_row","result":"denied"},{"import":"command_row","result":"denied"}],"log":[],"rows":[]}
{"seq":3,"event":"input","status":"not_subscribed","fuel":0,"actions":[],"log":[]}
"#,
    );
}

#[test]
fn fixture_mistakes_name_the_line() {
    for (fixture, expected) in [
        ("{\"event\":\"keydown\"}", "第 1 行：没有 keydown 这个事件"),
        (
            "\n{\"event\":\"focus_in\",\"body\":\"x\"}",
            "第 2 行：focus_in 事件没有 body 字段",
        ),
        (
            "{\"event\":\"commit\",\"body\":\"x\"}",
            "第 1 行：commit 事件必须写 origin",
        ),
        (
            "{\"event\":\"commit\",\"origin\":\"typed\",\"body\":\"x\"}",
            "第 1 行：origin 不认识 typed，只能是 selection、raw、held_flush、scheme_text、focus_loss",
        ),
        (
            "{\"event\":\"input\",\"input_class\":\"character\",\"input_char\":\"ab\"}",
            "第 1 行：input_char 必须正好是一个字符",
        ),
        (
            "{\"event\":\"focus_in\",\"turn\":3}",
            "第 1 行：不是有效的事件：unknown field `turn`",
        ),
        ("{\"event\":\"command\"}", "第 1 行：command 事件必须写 trigger"),
        (
            "{\"event\":\"input\",\"input_class\":\"character\",\"trigger\":\"uuid\"}",
            "第 1 行：input 事件没有 trigger 字段",
        ),
        (
            "{\"event\":\"command\",\"trigger\":\"UU\"}",
            "第 1 行：trigger 必须是 2 到 16 个小写字母",
        ),
        (
            "{\"event\":\"command\",\"trigger\":\"uuid\",\"argument\":\"a1\"}",
            "第 1 行：argument 只能是至多 64 个小写字母",
        ),
        (
            "{\"event\":\"command\",\"trigger\":\"uuid\",\"random\":\"00\"}",
            "第 1 行：random 必须是 128 位十六进制",
        ),
        (
            "{\"event\":\"command\",\"trigger\":\"uuid\",\"local_mode\":\"none\"}",
            "第 1 行：command 事件的 local_mode 只能是 command",
        ),
    ] {
        let error = parse_fixture(fixture).unwrap_err();
        assert!(error.starts_with(expected), "{error}");
    }
}

#[test]
fn grants_follow_the_grant_page_rules() {
    let declared: CapabilitySet = REWRITER_CAPABILITIES.into_iter().collect();
    assert_eq!(
        GrantSpec::Nothing.resolve(declared),
        Ok(CapabilitySet::EMPTY)
    );
    assert_eq!(GrantSpec::parse("all").resolve(declared), Ok(declared));
    assert_eq!(
        GrantSpec::parse("read_commit").resolve(declared),
        Ok(CapabilitySet::EMPTY.with(Capability::ReadCommit))
    );
    assert_eq!(
        GrantSpec::parse("read_commit,write_disk").resolve(declared),
        Err("--grant 里有未知的能力 write_disk".to_owned())
    );
    assert_eq!(
        GrantSpec::parse("read_candidates").resolve(declared),
        Err("清单没有申请 read_candidates".to_owned())
    );
    assert_eq!(
        GrantSpec::parse("rewrite_commit").resolve(declared),
        Err("申请 rewrite_commit 必须同时申请 read_commit".to_owned())
    );
}

#[test]
fn the_template_fixture_is_a_valid_fixture() {
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../docs/script-plugin-template/fixture.jsonl");
    let steps = parse_fixture(&fs::read_to_string(fixture).unwrap()).unwrap();
    assert!(!steps.is_empty());
}

// ---- 经命令行：真实的包 ----

/// 按字节手写的 ABI v1 模块：导入 `msime.log`，一页内存；`msime_on_event` 先 `log(事件类型, 0, 0)`，事件是 `selection_retracted` 时 `unreachable`，否则返回 0。
fn binary_module() -> Vec<u8> {
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
        0x00, // 没有局部变量
        0x20, 0x00, 0x41, 0x00, 0x41, 0x00, 0x10, 0x00, // log(kind, 0, 0)
        0x20, 0x00, 0x41, 0x07, 0x46, 0x04, 0x40, 0x00, 0x0b, // if kind == 7 { unreachable }
        0x41, 0x00, 0x0b, // 0
    ];
    let mut code = vec![2, abi.len() as u8];
    code.extend_from_slice(&abi);
    code.push(on_event.len() as u8);
    code.extend_from_slice(&on_event);
    section(10, &code, &mut out);
    out
}

/// 按字节手写的指令插件：导入 `msime.command_row`，`msime_on_event` 什么都不做、返回 0。
fn binary_commander() -> Vec<u8> {
    let mut out = b"\0asm\x01\0\0\0".to_vec();
    let mut section = |id: u8, content: &[u8]| {
        out.push(id);
        out.push(u8::try_from(content.len()).unwrap());
        out.extend_from_slice(content);
    };
    // 类型：(i32 i32 i32 i32) -> i32、() -> i32、(i32) -> i32。
    section(
        1,
        &[
            3, 0x60, 4, 0x7f, 0x7f, 0x7f, 0x7f, 1, 0x7f, 0x60, 0, 1, 0x7f, 0x60, 1, 0x7f, 1, 0x7f,
        ],
    );
    let mut imports = vec![1, 5];
    imports.extend_from_slice(b"msime");
    imports.push(11);
    imports.extend_from_slice(b"command_row");
    imports.extend_from_slice(&[0x00, 0]);
    section(2, &imports);
    section(3, &[2, 1, 2]);
    section(5, &[1, 0x00, 1]);
    let mut exports = vec![3];
    for (export, kind, index) in [
        ("memory", 2, 0),
        ("msime_abi", 0, 1),
        ("msime_on_event", 0, 2),
    ] {
        exports.push(u8::try_from(export.len()).unwrap());
        exports.extend_from_slice(export.as_bytes());
        exports.extend_from_slice(&[kind, index]);
    }
    section(7, &exports);
    section(
        10,
        &[2, 4, 0x00, 0x41, 0x01, 0x0b, 4, 0x00, 0x41, 0x00, 0x0b],
    );
    out
}

const COMMANDER_MANIFEST: &str = "schema_version = 2\nkind = 'script'\nid = 'generator'\nname = 'UUID'\nversion = '1.0.0'\nlicense = 'MIT'\npermissions = ['provide_commands']\n[script]\nabi = 1\nmodule = 'main.wasm'\nhooks = ['command']\nmemory_pages = 1\n[[script.commands]]\ntrigger = 'uuid'\ntitle = 'UUID'\n";

/// `validate` 按清单规则查 `[[script.commands]]`，`run` 只投递清单里声明过的触发词。
#[test]
fn slash_commands_are_validated_and_only_declared_triggers_run() {
    let files = tempfile::tempdir().unwrap();
    let pack = files.path().join("uuid");
    fs::create_dir_all(&pack).unwrap();
    fs::write(pack.join("plugin.toml"), COMMANDER_MANIFEST).unwrap();
    fs::write(pack.join("main.wasm"), binary_commander()).unwrap();
    let (status, out, _) = invoke(&[Path::new("validate"), &pack]);
    assert_eq!(
        (status, out.as_str()),
        (EXIT_OK, "ok generator script 1.0.0\n")
    );

    let events = files.path().join("events.jsonl");
    fs::write(
        &events,
        "{\"event\":\"command\",\"key\":true,\"trigger\":\"uuid\"}\n",
    )
    .unwrap();
    let (status, out, err) = invoke(&[
        Path::new("run"),
        &pack,
        Path::new("--events"),
        &events,
        Path::new("--grant"),
        Path::new("all"),
    ]);
    assert_eq!((status, err.as_str()), (EXIT_OK, ""));
    assert!(out.contains(r#""event":"command","status":"ok""#), "{out}");
    assert!(out.trim_end().ends_with(r#""rows":[]}"#), "{out}");

    let undeclared = files.path().join("undeclared.jsonl");
    fs::write(
        &undeclared,
        "{\"event\":\"command\",\"key\":true,\"trigger\":\"uuid\"}\n{\"event\":\"command\",\"trigger\":\"guid\"}\n",
    )
    .unwrap();
    let (status, out, err) = invoke(&[Path::new("run"), &pack, Path::new("--events"), &undeclared]);
    assert_eq!(
        (status, out.as_str(), err),
        (
            EXIT_USAGE,
            "",
            format!(
                "error {}: 第 2 个事件的触发词 guid 不在清单的 commands 里\n",
                undeclared.display()
            )
        )
    );

    // 和内置指令冲突的触发词：validate 报出原因。
    fs::write(
        pack.join("plugin.toml"),
        COMMANDER_MANIFEST.replace("trigger = 'uuid'", "trigger = 'fy'"),
    )
    .unwrap();
    let (status, out, _) = invoke(&[Path::new("validate"), &pack]);
    assert_eq!(status, EXIT_INVALID);
    assert_eq!(
        out,
        format!(
            "error {}: plugin_invalid: 指令触发词 fy 和内置指令 /fy 相同或互为前缀\n",
            pack.display()
        )
    );
}

const PACK_MANIFEST: &str = "schema_version = 2\nkind = 'script'\nid = 'logger'\nname = '记录'\nversion = '1.0.0'\nlicense = 'MIT'\npermissions = []\n[script]\nabi = 1\nmodule = 'main.wasm'\nhooks = ['commit', 'selection_retracted']\nmemory_pages = 1\n";

/// 在 `parent` 下写一个脚本包和一份夹具。
fn pack_and_fixture(parent: &Path, fixture: &str) -> (PathBuf, PathBuf) {
    let pack = parent.join("logger");
    fs::create_dir_all(&pack).unwrap();
    fs::write(pack.join("plugin.toml"), PACK_MANIFEST).unwrap();
    fs::write(pack.join("main.wasm"), binary_module()).unwrap();
    let events = parent.join("events.jsonl");
    fs::write(&events, fixture).unwrap();
    (pack, events)
}

fn invoke(args: &[&Path]) -> (i32, String, String) {
    let args: Vec<OsString> = args.iter().map(|arg| arg.as_os_str().to_owned()).collect();
    let (mut out, mut err) = (Vec::new(), Vec::new());
    let status = run(&args, &mut out, &mut err);
    (
        status,
        String::from_utf8(out).unwrap(),
        String::from_utf8(err).unwrap(),
    )
}

const COMMIT_THEN_RETRACT: &str = "{\"event\":\"commit\",\"key\":true,\"origin\":\"selection\",\"body\":\"x\"}\n{\"event\":\"selection_retracted\",\"retraction\":\"drop\"}\n{\"event\":\"commit\",\"origin\":\"selection\",\"body\":\"y\"}\n";

#[test]
fn a_pack_on_disk_runs_through_the_command_line() {
    let files = tempfile::tempdir().unwrap();
    let (pack, events) = pack_and_fixture(files.path(), COMMIT_THEN_RETRACT);
    let (status, out, err) = invoke(&[Path::new("run"), &pack, Path::new("--events"), &events]);
    assert_eq!((status, err.as_str()), (EXIT_INVALID, ""));
    assert_golden(
        &out,
        r#"{"seq":1,"event":"commit","status":"ok","fuel":110,"init_fuel":2,"actions":[],"log":[{"level":"10","text":""}]}
{"seq":2,"event":"selection_retracted","status":"unreachable","fuel":111,"actions":[],"log":[{"level":"7","text":""}]}
{"seq":3,"event":"commit","status":"suspended","fuel":0,"actions":[],"log":[]}
"#,
    );

    // validate 照旧一行：脚本包也是 `ok <id> <kind> <version>`。
    let (status, out, _) = invoke(&[Path::new("validate"), &pack]);
    assert_eq!(
        (status, out.as_str()),
        (EXIT_OK, "ok logger script 1.0.0\n")
    );

    // 只有不失败的事件时退出码为 0；参数顺序不限。
    let calm = files.path().join("calm.jsonl");
    fs::write(
        &calm,
        "{\"event\":\"commit\",\"origin\":\"raw\",\"body\":\"x\"}\n",
    )
    .unwrap();
    let (status, _, err) = invoke(&[
        Path::new("run"),
        Path::new("--grant"),
        Path::new("all"),
        Path::new("--events"),
        &calm,
        &pack,
    ]);
    assert_eq!((status, err.as_str()), (EXIT_OK, ""));
}

#[test]
fn the_binary_runs_and_exits_the_same_way() {
    let files = tempfile::tempdir().unwrap();
    let (pack, events) = pack_and_fixture(files.path(), COMMIT_THEN_RETRACT);
    let output = Command::new(env!("CARGO_BIN_EXE_msime-pack"))
        .arg("run")
        .arg(&pack)
        .arg("--events")
        .arg(&events)
        .output()
        .unwrap();
    assert_eq!(output.status.code(), Some(EXIT_INVALID));
    let (_, expected, _) = invoke(&[Path::new("run"), &pack, Path::new("--events"), &events]);
    assert_eq!(String::from_utf8_lossy(&output.stdout), expected);
}

#[test]
fn bad_packs_grants_and_command_lines_are_refused() {
    let files = tempfile::tempdir().unwrap();
    let (pack, events) = pack_and_fixture(files.path(), COMMIT_THEN_RETRACT);
    let template = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../docs/plugin-template");

    // 不是脚本包、包不合规：和 validate 同样的错误行，写到标准错误，退出码 1。
    let (status, out, err) = invoke(&[Path::new("run"), &template, Path::new("--events"), &events]);
    assert_eq!((status, out.as_str()), (EXIT_INVALID, ""));
    assert_eq!(
        err,
        format!(
            "error {}: plugin_invalid: 这是 sound 插件，只有脚本插件（kind = \"script\"）能运行\n",
            template.display()
        )
    );
    let broken = files.path().join("broken");
    fs::create_dir_all(&broken).unwrap();
    fs::write(
        broken.join("plugin.toml"),
        PACK_MANIFEST.replace("'logger'", "'broken'"),
    )
    .unwrap();
    fs::write(broken.join("main.wasm"), b"\0asm\x01\0\0\0").unwrap();
    let (status, _, err) = invoke(&[Path::new("run"), &broken, Path::new("--events"), &events]);
    assert_eq!(status, EXIT_INVALID);
    assert!(
        err.starts_with(&format!(
            "error {}: plugin_invalid: main.wasm 不是有效的脚本模块",
            broken.display()
        )),
        "{err}"
    );

    // 授权超出清单、夹具写错、读不出夹具：退出码 2。
    let (status, _, err) = invoke(&[
        Path::new("run"),
        &pack,
        Path::new("--events"),
        &events,
        Path::new("--grant"),
        Path::new("read_commit"),
    ]);
    assert_eq!(
        (status, err.as_str()),
        (EXIT_USAGE, "error: 清单没有申请 read_commit\n")
    );
    let bad = files.path().join("bad.jsonl");
    fs::write(&bad, "{\"event\":\"commit\"}\n").unwrap();
    let (status, _, err) = invoke(&[Path::new("run"), &pack, Path::new("--events"), &bad]);
    assert_eq!(
        (status, err),
        (
            EXIT_USAGE,
            format!(
                "error {}: 第 1 行：commit 事件必须写 origin\n",
                bad.display()
            )
        )
    );
    let missing = files.path().join("missing.jsonl");
    let (status, _, err) = invoke(&[Path::new("run"), &pack, Path::new("--events"), &missing]);
    assert_eq!(status, EXIT_USAGE);
    assert!(
        err.starts_with(&format!("error {}: 读不出夹具", missing.display())),
        "{err}"
    );

    for args in [
        &[Path::new("run")][..],
        &[Path::new("run"), &pack][..],
        &[Path::new("run"), &pack, Path::new("--events")][..],
        &[
            Path::new("run"),
            &pack,
            Path::new("--events"),
            &events,
            Path::new("--events"),
            &events,
        ][..],
        &[
            Path::new("run"),
            &pack,
            &pack,
            Path::new("--events"),
            &events,
        ][..],
        &[
            Path::new("run"),
            &pack,
            Path::new("--events"),
            &events,
            Path::new("--fuel"),
            Path::new("9"),
        ][..],
    ] {
        let (status, out, err) = invoke(args);
        assert_eq!(status, EXIT_USAGE, "{args:?}");
        assert!(out.is_empty());
        assert!(err.starts_with("usage: msime-pack validate"), "{err}");
    }
}

#[test]
fn hash_prints_the_content_hash_the_grants_bind() {
    let files = tempfile::tempdir().unwrap();
    let (pack, _) = pack_and_fixture(files.path(), COMMIT_THEN_RETRACT);
    let (status, out, err) = invoke(&[Path::new("hash"), &pack]);
    assert_eq!((status, err.as_str()), (EXIT_OK, ""));
    let expected = msime_client_services::plugins::script_pack::content_hash(
        PACK_MANIFEST.as_bytes(),
        &binary_module(),
    );
    assert_eq!(out, format!("{expected}\n"));

    // 不是脚本包、不是包、命令行写错。
    let (status, out, _) = invoke(&[Path::new("hash"), &files.path().join("missing")]);
    assert_eq!(status, EXIT_INVALID);
    assert!(out.contains("plugin_unsupported_source"), "{out}");
    for args in [
        &[Path::new("hash")][..],
        &[Path::new("hash"), &pack, &pack][..],
        &[Path::new("hash"), Path::new("--builtin")][..],
    ] {
        let (status, _, err) = invoke(args);
        assert_eq!(status, EXIT_USAGE, "{args:?}");
        assert!(err.starts_with("usage: msime-pack validate"));
    }
}

#[test]
fn official_ids_need_the_builtin_flag_everywhere() {
    let files = tempfile::tempdir().unwrap();
    let (logger, events) = pack_and_fixture(files.path(), COMMIT_THEN_RETRACT);
    // 随桌面安装包分发的官方脚本插件的 id：导入和社区库拒绝它，只有仓库的构建脚本加 `--builtin`。
    let pack = files.path().join("uuid");
    fs::create_dir_all(&pack).unwrap();
    fs::write(
        pack.join("plugin.toml"),
        PACK_MANIFEST.replace("id = 'logger'", "id = 'uuid'"),
    )
    .unwrap();
    fs::copy(logger.join("main.wasm"), pack.join("main.wasm")).unwrap();
    let reserved = format!(
        "error {}: plugin_reserved: 这个 id 属于内置插件，不能导入\n",
        pack.display()
    );

    let (status, out, _) = invoke(&[Path::new("validate"), &pack]);
    assert_eq!((status, out.as_str()), (EXIT_INVALID, reserved.as_str()));
    let (status, out, _) = invoke(&[Path::new("validate"), Path::new("--builtin"), &pack]);
    assert_eq!((status, out.as_str()), (EXIT_OK, "ok uuid script 1.0.0\n"));

    let (status, out, _) = invoke(&[Path::new("hash"), &pack]);
    assert_eq!((status, out.as_str()), (EXIT_INVALID, reserved.as_str()));
    let (status, out, _) = invoke(&[Path::new("hash"), Path::new("--builtin"), &pack]);
    assert_eq!(status, EXIT_OK);
    assert_eq!(out.trim_end().len(), 64);

    let (status, out, err) = invoke(&[Path::new("run"), &pack, Path::new("--events"), &events]);
    assert_eq!(status, EXIT_INVALID);
    assert!(out.is_empty());
    assert_eq!(err, reserved);
    let (status, out, err) = invoke(&[
        Path::new("run"),
        Path::new("--builtin"),
        &pack,
        Path::new("--events"),
        &events,
    ]);
    // 和 logger 同一个模块：第二个事件 trap，所以是 1；要看的是它真的运行了。
    assert_eq!((status, err.as_str()), (EXIT_INVALID, ""));
    assert_eq!(out.lines().count(), 3, "{out}");

    // `--builtin` 只能出现一次，也不能当作包。
    let (status, _, _) = invoke(&[
        Path::new("validate"),
        Path::new("--builtin"),
        Path::new("--builtin"),
    ]);
    assert_eq!(status, EXIT_USAGE);
    let (status, _, _) = invoke(&[
        Path::new("run"),
        &pack,
        Path::new("--builtin"),
        Path::new("--builtin"),
        Path::new("--events"),
        &events,
    ]);
    assert_eq!(status, EXIT_USAGE);
}
