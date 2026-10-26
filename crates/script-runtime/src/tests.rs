//! 用 WAT 写的模块测 ABI 校验、资源限制、配额隔离、快照复位、观察类和修改类 import。仓库不提交 `.wasm`；wasmi 的 `wat` 特性只在 dev-dependencies 里打开。

use crate::abi::{candidate_field, commit_field, composition_field, event_field, status};
use crate::limits;
use crate::module::compile_unchecked_format;
use crate::*;
use std::sync::{Arc, Mutex};

fn declaration(capabilities: &[Capability], hooks: &[Hook]) -> Declaration {
    Declaration {
        capabilities: capabilities.iter().copied().collect(),
        hooks: hooks.iter().copied().collect(),
        memory_pages: limits::DEFAULT_MEMORY_PAGES,
    }
}

fn compile_wat(wat: &str, declaration: Declaration) -> Result<ScriptModule, ModuleError> {
    compile_unchecked_format(wat.as_bytes(), declaration)
}

/// 最小的合规模块：`imports` 是 import 声明，`body` 是 `msime_on_event` 的函数体（参数 0 是 kind）。
fn module_wat(imports: &str, extra: &str, body: &str) -> String {
    format!(
        r#"(module
            {imports}
            (memory (export "memory") 1)
            (func (export "msime_abi") (result i32) i32.const 1)
            {extra}
            (func (export "msime_on_event") (param $kind i32) (result i32) {body}))"#
    )
}

fn instance(wat: &str, declaration: Declaration) -> ScriptInstance {
    let module = compile_wat(wat, declaration).expect("module compiles");
    ScriptInstance::new(&module, declaration.capabilities, limits::INIT_FUEL)
        .expect("module instantiates")
}

fn input_event() -> Event<'static> {
    let mut event = Event::new(Hook::Input);
    event.scalars.local_mode = Some(0);
    event.scalars.input_class = Some(0);
    event
}

fn failure(outcome: Outcome) -> Failure {
    match outcome {
        Outcome::Failed { failure, .. } => failure,
        other => panic!("expected a failure, got {other:?}"),
    }
}

fn status_of(outcome: Outcome) -> i32 {
    match outcome {
        Outcome::Delivered { status, .. } => status,
        other => panic!("expected a delivery, got {other:?}"),
    }
}

// ---- 校验 ----

#[test]
fn a_minimal_module_compiles_and_runs() {
    let mut script = instance(
        &module_wat("", "", "local.get $kind"),
        declaration(&[], &[Hook::Input]),
    );
    assert_eq!(
        status_of(script.deliver(1, &input_event())),
        Hook::Input.code()
    );
}

#[test]
fn the_public_entry_point_accepts_only_binary_modules() {
    let wat = module_wat("", "", "i32.const 0");
    assert_eq!(
        validate(wat.as_bytes(), declaration(&[], &[Hook::Input])).unwrap_err(),
        ModuleError::NotBinary
    );
    let oversized = vec![0u8; limits::MAX_MODULE_BYTES + 1];
    assert_eq!(
        validate(&oversized, declaration(&[], &[Hook::Input])).unwrap_err(),
        ModuleError::TooLarge
    );
}

#[test]
fn declarations_are_checked_before_the_module() {
    let wat = module_wat("", "", "i32.const 0");
    assert_eq!(
        compile_wat(&wat, declaration(&[], &[])).unwrap_err(),
        ModuleError::NoHooks
    );
    for pages in [0, limits::MAX_MEMORY_PAGES + 1] {
        let mut declared = declaration(&[], &[Hook::Input]);
        declared.memory_pages = pages;
        assert_eq!(
            compile_wat(&wat, declared).unwrap_err(),
            ModuleError::MemoryPages
        );
    }
}

#[test]
fn rejected_proposals_do_not_compile() {
    let cases = [
        (
            "multi-memory",
            r#"(module (memory (export "memory") 1) (memory 1)
                (func (export "msime_abi") (result i32) i32.const 1)
                (func (export "msime_on_event") (param i32) (result i32) i32.const 0))"#,
        ),
        (
            "tail-call",
            r#"(module (memory (export "memory") 1)
                (func $abi (export "msime_abi") (result i32) i32.const 1)
                (func (export "msime_on_event") (param i32) (result i32) return_call $abi))"#,
        ),
        (
            "extended-const",
            r#"(module (memory (export "memory") 1)
                (global i32 (i32.add (i32.const 1) (i32.const 2)))
                (func (export "msime_abi") (result i32) i32.const 1)
                (func (export "msime_on_event") (param i32) (result i32) i32.const 0))"#,
        ),
        (
            "simd",
            r#"(module (memory (export "memory") 1)
                (func (export "msime_abi") (result i32) i32.const 1)
                (func (export "msime_on_event") (param i32) (result i32)
                    v128.const i32x4 1 2 3 4
                    i32x4.extract_lane 0))"#,
        ),
        (
            "memory64",
            r#"(module (memory (export "memory") i64 1)
                (func (export "msime_abi") (result i32) i32.const 1)
                (func (export "msime_on_event") (param i32) (result i32) i32.const 0))"#,
        ),
        (
            "threads",
            r#"(module (memory (export "memory") 1 1 shared)
                (func (export "msime_abi") (result i32) i32.const 1)
                (func (export "msime_on_event") (param i32) (result i32) i32.const 0))"#,
        ),
        (
            "start function",
            r#"(module (memory (export "memory") 1)
                (func $start)
                (start $start)
                (func (export "msime_abi") (result i32) i32.const 1)
                (func (export "msime_on_event") (param i32) (result i32) i32.const 0))"#,
        ),
    ];
    for (proposal, wat) in cases {
        let error = compile_wat(wat, declaration(&[], &[Hook::Input])).unwrap_err();
        assert!(
            matches!(error, ModuleError::Invalid(_)),
            "{proposal} should be rejected by validation, got {error:?}"
        );
    }
}

#[test]
fn allowed_proposals_compile() {
    // MVP 之外允许的：mutable-global、sign-ext、sat-float-to-int、multi-value、bulk-memory、reference-types。
    let wat = r#"(module
        (memory (export "memory") 1)
        (table 1 funcref)
        (global $g (export "g") (mut i32) (i32.const 0))
        (func (export "msime_abi") (result i32) i32.const 1)
        (func $pair (result i32 i32) i32.const 1 i32.const 2)
        (func (export "msime_on_event") (param i32) (result i32)
            (memory.fill (i32.const 0) (i32.const 0) (i32.const 4))
            (drop (table.get 0 (i32.const 0)))
            (drop (i32.trunc_sat_f32_s (f32.const 1.5)))
            (call $pair) i32.add
            i32.extend8_s))"#;
    compile_wat(wat, declaration(&[], &[Hook::Input])).expect("allowed proposals compile");
}

#[test]
fn memory_and_exports_are_required() {
    let no_memory = r#"(module
        (func (export "msime_abi") (result i32) i32.const 1)
        (func (export "msime_on_event") (param i32) (result i32) i32.const 0))"#;
    assert_eq!(
        compile_wat(no_memory, declaration(&[], &[Hook::Input])).unwrap_err(),
        ModuleError::MissingMemory
    );
    let too_big = r#"(module (memory (export "memory") 17)
        (func (export "msime_abi") (result i32) i32.const 1)
        (func (export "msime_on_event") (param i32) (result i32) i32.const 0))"#;
    assert_eq!(
        compile_wat(too_big, declaration(&[], &[Hook::Input])).unwrap_err(),
        ModuleError::BadMemory
    );
    let no_on_event = r#"(module (memory (export "memory") 1)
        (func (export "msime_abi") (result i32) i32.const 1))"#;
    assert_eq!(
        compile_wat(no_on_event, declaration(&[], &[Hook::Input])).unwrap_err(),
        ModuleError::MissingExport(abi::EXPORT_ON_EVENT)
    );
    let bad_init = module_wat("", r#"(func (export "msime_init"))"#, "i32.const 0");
    assert_eq!(
        compile_wat(&bad_init, declaration(&[], &[Hook::Input])).unwrap_err(),
        ModuleError::ExportSignature(abi::EXPORT_INIT)
    );
}

#[test]
fn imports_outside_the_whitelist_are_rejected() {
    let cases = [
        r#"(import "env" "event_scalar" (func (param i32) (result i64)))"#,
        r#"(import "msime" "read_clipboard" (func (param i32) (result i64)))"#,
        r#"(import "msime" "event_scalar" (global i32))"#,
        r#"(import "wasi_snapshot_preview1" "fd_write" (func (param i32 i32 i32 i32) (result i32)))"#,
    ];
    for import in cases {
        let error = compile_wat(
            &module_wat(import, "", "i32.const 0"),
            declaration(&[], &[Hook::Input]),
        )
        .unwrap_err();
        assert!(
            matches!(error, ModuleError::ImportNotAllowed { .. }),
            "{import}: {error:?}"
        );
    }
    let imported_memory = r#"(module (import "msime" "memory" (memory 1))
        (func (export "msime_abi") (result i32) i32.const 1)
        (func (export "msime_on_event") (param i32) (result i32) i32.const 0))"#;
    assert!(matches!(
        compile_wat(imported_memory, declaration(&[], &[Hook::Input])).unwrap_err(),
        ModuleError::MissingMemory | ModuleError::ImportNotAllowed { .. }
    ));
    let wrong_signature = module_wat(
        r#"(import "msime" "event_scalar" (func (param i32) (result i32)))"#,
        "",
        "i32.const 0",
    );
    assert_eq!(
        compile_wat(&wrong_signature, declaration(&[], &[Hook::Input])).unwrap_err(),
        ModuleError::ImportSignature("event_scalar")
    );
}

#[test]
fn an_import_needs_its_capability() {
    let wat = module_wat(
        r#"(import "msime" "commit_text" (func (param i32 i32 i32) (result i32)))"#,
        "",
        "i32.const 0",
    );
    assert_eq!(
        compile_wat(&wat, declaration(&[], &[Hook::Commit])).unwrap_err(),
        ModuleError::ImportWithoutCapability {
            import: "commit_text",
            capability: Capability::ReadCommit,
        }
    );
    compile_wat(
        &wat,
        declaration(&[Capability::ReadCommit], &[Hook::Commit]),
    )
    .expect("declared capability admits the import");
}

#[test]
fn a_capability_needs_an_import() {
    let wat = module_wat(
        r#"(import "msime" "commit_text" (func (param i32 i32 i32) (result i32)))"#,
        "",
        "i32.const 0",
    );
    assert_eq!(
        compile_wat(
            &wat,
            declaration(
                &[Capability::ReadCommit, Capability::ReadCandidates],
                &[Hook::Commit]
            )
        )
        .unwrap_err(),
        ModuleError::UnusedCapability(Capability::ReadCandidates)
    );
    // 修改类能力同样要导入它自己的函数，读能力的 import 替它不了。
    assert_eq!(
        compile_wat(
            &wat,
            declaration(
                &[Capability::ReadCommit, Capability::RewriteCommit],
                &[Hook::BeforeCommit]
            )
        )
        .unwrap_err(),
        ModuleError::UnusedCapability(Capability::RewriteCommit)
    );
}

#[test]
fn capability_rules_hold() {
    let wat = module_wat(
        r#"(import "msime" "rewrite_commit" (func (param i32 i32) (result i32)))
           (import "msime" "commit_text" (func (param i32 i32 i32) (result i32)))
           (import "msime" "composition_text" (func (param i32 i32 i32) (result i32)))"#,
        "",
        "i32.const 0",
    );
    assert!(matches!(
        compile_wat(
            &wat,
            declaration(
                &[
                    Capability::ReadCommit,
                    Capability::RewriteCommit,
                    Capability::ReadComposition
                ],
                &[Hook::BeforeCommit]
            )
        )
        .unwrap_err(),
        ModuleError::Capabilities(CapabilityRuleError::Exclusive { .. })
    ));
    let rerank_alone: CapabilitySet = [Capability::RerankCandidates].into_iter().collect();
    assert_eq!(
        rerank_alone.check_rules(),
        Err(CapabilityRuleError::MissingDependency {
            capability: Capability::RerankCandidates,
            requires: Capability::ReadCandidates,
        })
    );
    for capability in Capability::ALL {
        assert_eq!(Capability::parse(capability.as_str()), Some(capability));
    }
    for hook in Hook::ALL {
        assert_eq!(Hook::parse(hook.as_str()), Some(hook));
    }
    assert_eq!(Hook::parse("key_down"), None);
}

// ---- 执行限制 ----

#[test]
fn an_infinite_loop_runs_out_of_its_hook_fuel() {
    let mut script = instance(
        &module_wat("", "", "(loop $spin (br $spin)) i32.const 0"),
        declaration(&[], &[Hook::Input]),
    );
    assert_eq!(
        script.deliver(1, &input_event()),
        Outcome::Failed {
            failure: Failure::OutOfFuel,
            fuel: limits::OBSERVE_FUEL,
        }
    );
    assert!(!script.is_usable());
    assert_eq!(
        failure(script.deliver(2, &input_event())),
        Failure::Unusable
    );
}

#[test]
fn unreachable_traps() {
    let mut script = instance(
        &module_wat("", "", "unreachable"),
        declaration(&[], &[Hook::Input]),
    );
    assert_eq!(
        failure(script.deliver(1, &input_event())),
        Failure::Unreachable
    );
    assert!(!script.is_usable());
}

#[test]
fn out_of_bounds_access_traps() {
    let mut script = instance(
        &module_wat("", "", "(i32.load (i32.const 0x7fff_fff0))"),
        declaration(&[], &[Hook::Input]),
    );
    assert_eq!(
        failure(script.deliver(1, &input_event())),
        Failure::MemoryOutOfBounds
    );
}

#[test]
fn memory_is_reserved_up_front_and_cannot_grow() {
    let size = instance(
        &module_wat("", "", "memory.size"),
        declaration(&[], &[Hook::Input]),
    )
    .deliver(1, &input_event());
    assert_eq!(status_of(size), limits::DEFAULT_MEMORY_PAGES as i32);

    let mut grower = instance(
        &module_wat("", "", "(drop (memory.grow (i32.const 1))) i32.const 0"),
        declaration(&[], &[Hook::Input]),
    );
    assert_eq!(
        failure(grower.deliver(1, &input_event())),
        Failure::MemoryLimit
    );
}

#[test]
fn a_smaller_declared_maximum_caps_the_reservation() {
    let wat = r#"(module (memory (export "memory") 1 4)
        (func (export "msime_abi") (result i32) i32.const 1)
        (func (export "msime_on_event") (param i32) (result i32) memory.size))"#;
    let module = compile_wat(wat, declaration(&[], &[Hook::Input])).unwrap();
    assert_eq!(module.reserved_memory_bytes(), 4 * limits::PAGE_BYTES);
    let mut script = ScriptInstance::new(&module, CapabilitySet::EMPTY, limits::INIT_FUEL).unwrap();
    assert_eq!(status_of(script.deliver(1, &input_event())), 4);
}

#[test]
fn deep_recursion_hits_the_stack_limit() {
    let mut script = instance(
        &module_wat(
            "",
            "(func $down (param i32) (result i32) (call $down (i32.add (local.get 0) (i32.const 1))))",
            "(call $down (i32.const 0))",
        ),
        declaration(&[], &[Hook::Input]),
    );
    assert_eq!(
        failure(script.deliver(1, &input_event())),
        Failure::StackOverflow
    );
}

#[test]
fn a_negative_return_is_a_failure_but_keeps_the_instance() {
    let mut script = instance(
        &module_wat("", "", "i32.const -1"),
        declaration(&[], &[Hook::Input]),
    );
    assert_eq!(
        failure(script.deliver(1, &input_event())),
        Failure::NegativeReturn
    );
    assert!(script.is_usable());
}

#[test]
fn undeclared_hooks_are_not_delivered() {
    let mut script = instance(
        &module_wat("", "", "unreachable"),
        declaration(&[], &[Hook::Commit]),
    );
    assert_eq!(script.deliver(1, &input_event()), Outcome::NotSubscribed);
    assert_eq!(script.turn_fuel_left(), limits::TURN_FUEL);
}

#[test]
fn initialisation_failures() {
    let wrong_abi = r#"(module (memory (export "memory") 1)
        (func (export "msime_abi") (result i32) i32.const 2)
        (func (export "msime_on_event") (param i32) (result i32) i32.const 0))"#;
    let module = compile_wat(wrong_abi, declaration(&[], &[Hook::Input])).unwrap();
    assert_eq!(
        ScriptInstance::new(&module, CapabilitySet::EMPTY, limits::INIT_FUEL)
            .err()
            .map(|error| error.failure),
        Some(Failure::AbiMismatch)
    );

    let failing_init = module_wat(
        "",
        r#"(func (export "msime_init") (result i32) i32.const -5)"#,
        "i32.const 0",
    );
    let module = compile_wat(&failing_init, declaration(&[], &[Hook::Input])).unwrap();
    assert_eq!(
        ScriptInstance::new(&module, CapabilitySet::EMPTY, limits::INIT_FUEL)
            .err()
            .map(|error| error.failure),
        Some(Failure::InitFailed)
    );

    // 初始化同样受 fuel 约束，失败时报告用掉的额度，供宿主计入 focus 的合计。
    let spinning_init = module_wat(
        "",
        r#"(func (export "msime_init") (result i32) (loop $spin (br $spin)) i32.const 0)"#,
        "i32.const 0",
    );
    let module = compile_wat(&spinning_init, declaration(&[], &[Hook::Input])).unwrap();
    assert_eq!(
        ScriptInstance::new(&module, CapabilitySet::EMPTY, 40_000).err(),
        Some(InitFailure {
            failure: Failure::OutOfFuel,
            fuel: 40_000,
        })
    );
}

// ---- 配额 ----

/// 每次投递做 `iterations` 次循环、返回 0 的模块。
fn burner(iterations: u32) -> String {
    module_wat(
        "",
        "",
        &format!(
            "(local $i i32)
             (local.set $i (i32.const {iterations}))
             (loop $again
                (local.set $i (i32.sub (local.get $i) (i32.const 1)))
                (br_if $again (local.get $i)))
             i32.const 0"
        ),
    )
}

#[test]
fn the_turn_quota_spans_deliveries_and_refills_on_the_next_turn() {
    // 每次投递约七千 fuel，不到观察类钩子上限的一半，被截断的那一次用掉的只会更少。
    let mut script = instance(&burner(1_000), declaration(&[], &[Hook::Input]));
    let mut spent = 0;
    let mut deliveries = 0;
    loop {
        match script.deliver(7, &input_event()) {
            Outcome::Delivered { fuel, .. } => {
                assert!(fuel < limits::OBSERVE_FUEL / 2);
                spent += fuel;
                deliveries += 1;
            }
            // 截断它的是本轮剩下的配额，不是钩子自己的上限，这一次用掉的也不到上限的一半：不算失败。
            Outcome::TurnExhausted { fuel } => {
                assert!(fuel < limits::OBSERVE_FUEL / 2);
                spent += fuel;
                break;
            }
            other => panic!("{other:?}"),
        }
        assert!(deliveries < 1_000, "the turn quota never ran out");
    }
    assert!(deliveries > 1);
    assert!(spent <= limits::TURN_FUEL);
    assert_eq!(script.turn_fuel_left(), 0);
    // 撞上本轮配额的那一次运行到一半被截断，实例要换新的；新实例在新一轮里重新拿到完整配额。
    let mut fresh = instance(&burner(1_000), declaration(&[], &[Hook::Input]));
    assert!(matches!(
        fresh.deliver(8, &input_event()),
        Outcome::Delivered { .. }
    ));
    assert!(fresh.turn_fuel_left() < limits::TURN_FUEL);
    // 新一轮剩下的恰好是满额减去这一次：没有重新给满时还要再减去第 8 轮那一次。
    let refilled = match fresh.deliver(9, &input_event()) {
        Outcome::Delivered { fuel, .. } => fuel,
        other => panic!("{other:?}"),
    };
    assert_eq!(fresh.turn_fuel_left(), limits::TURN_FUEL - refilled);
}

#[test]
fn an_exhausted_turn_is_skipped_rather_than_failed() {
    // 宿主在同一轮里重建实例时交过来的剩余是 0（之前的投递已经花光，派发没经过 `msime_client_key` 开新一轮）：不运行，不算失败，实例照样可用。
    let mut script = instance(&burner(2_000), declaration(&[], &[Hook::Input]));
    script.resume_turn(3, 0, limits::BEFORE_COMMIT_FUEL, limits::COMMAND_FUEL);
    assert_eq!(
        script.deliver(3, &input_event()),
        Outcome::TurnExhausted { fuel: 0 }
    );
    assert!(script.is_usable());
    assert!(matches!(
        script.deliver(4, &input_event()),
        Outcome::Delivered { .. }
    ));
    // 本轮只剩一点：运行到一半被截断，不算失败，但实例不再可用；不够下一段指令的零头也记为用完。
    let mut cut = instance(&burner(2_000), declaration(&[], &[Hook::Input]));
    cut.resume_turn(5, 100, limits::BEFORE_COMMIT_FUEL, limits::COMMAND_FUEL);
    let outcome = cut.deliver(5, &input_event());
    assert!(
        matches!(outcome, Outcome::TurnExhausted { fuel } if fuel > 0 && fuel <= 100),
        "{outcome:?}"
    );
    assert!(!cut.is_usable());
    assert_eq!(cut.turn_fuel_left(), 0);
    assert_eq!(cut.before_commit_fuel_left(), limits::BEFORE_COMMIT_FUEL);
    // `before_commit` 自己那一份只剩一点：同样不算失败；它跑在用完即弃的快照实例上，持久实例照样可用。记为用完的只是它自己那一份，其余钩子的配额不动。
    let mut snapshot = instance(
        &burner(2_000),
        declaration(&[], &[Hook::Input, Hook::BeforeCommit]),
    );
    snapshot.resume_turn(6, limits::TURN_FUEL, 100, limits::COMMAND_FUEL);
    let event = before_commit_event("abc", "abc");
    let outcome = snapshot.deliver(6, &event);
    assert!(
        matches!(outcome, Outcome::TurnExhausted { fuel } if fuel > 0 && fuel <= 100),
        "{outcome:?}"
    );
    assert!(snapshot.is_usable());
    assert_eq!(snapshot.before_commit_fuel_left(), 0);
    assert_eq!(snapshot.turn_fuel_left(), limits::TURN_FUEL);
    assert_eq!(
        snapshot.deliver(6, &event),
        Outcome::TurnExhausted { fuel: 0 }
    );
    assert!(matches!(
        snapshot.deliver(6, &input_event()),
        Outcome::Delivered { .. }
    ));
}

/// 所有钩子都是死循环的模块。
fn spinner() -> String {
    module_wat("", "", "(loop $spin (br $spin)) i32.const 0")
}

/// `hook` 这一次的事件：`before_commit` 带上屏文本，`command` 带触发词，其余只有标量。
fn event_for(hook: Hook) -> Event<'static> {
    if hook == Hook::BeforeCommit {
        return before_commit_event("abc", "abc");
    }
    if hook == Hook::Command {
        return command_event("uuid", "", [0; limits::RANDOM_BYTES_PER_EVENT]);
    }
    let mut event = input_event();
    event.hook = hook;
    event
}

#[test]
fn a_loop_in_candidates_after_input_spent_fuel_is_a_failure() {
    // `input` 先花掉几千 fuel，同一轮里 `candidates` 死循环：它能用的只剩本轮的余额，比自己的上限（与轮配额相等）少一点，于是先被轮配额截断。这一次用掉的远超自己上限的一半，按失败算，否则熔断永远不会落到这样的插件上。
    let wat = module_wat(
        "",
        "",
        &format!(
            "(local $i i32)
             (if (i32.eq (local.get $kind) (i32.const {candidates}))
                (then (loop $spin (br $spin))))
             (local.set $i (i32.const 1000))
             (loop $again
                (local.set $i (i32.sub (local.get $i) (i32.const 1)))
                (br_if $again (local.get $i)))
             i32.const 0",
            candidates = Hook::Candidates.code(),
        ),
    );
    let mut script = instance(&wat, declaration(&[], &[Hook::Input, Hook::Candidates]));
    let spent = match script.deliver(1, &input_event()) {
        Outcome::Delivered { fuel, .. } => fuel,
        other => panic!("{other:?}"),
    };
    assert!(spent > 0 && spent < limits::CANDIDATES_FUEL / 2);
    // wasmi 在剩下的 fuel 不够下一段指令时就停，用掉的可能比本轮余额少一点零头。
    let outcome = script.deliver(1, &event_for(Hook::Candidates));
    assert!(
        matches!(outcome, Outcome::Failed { failure: Failure::OutOfFuel, fuel }
            if fuel > limits::TURN_FUEL - spent - 10 && fuel <= limits::TURN_FUEL - spent),
        "{outcome:?}"
    );
    // 和超出自己上限的那种失败一样：实例不可用，本轮记为用完，宿主这次聚焦里不再重建。
    assert!(!script.is_usable());
    assert_eq!(script.turn_fuel_left(), 0);
}

#[test]
fn a_cut_run_counts_as_a_failure_from_half_of_its_own_limit() {
    for hook in [
        Hook::Input,
        Hook::Candidates,
        Hook::BeforeCommit,
        Hook::Command,
    ] {
        let half = limits::hook_fuel(hook) / 2;
        let resume = |script: &mut ScriptInstance, left: u64| match hook {
            Hook::BeforeCommit => {
                script.resume_turn(1, limits::TURN_FUEL, left, limits::COMMAND_FUEL)
            }
            Hook::Command => {
                script.resume_turn(1, limits::TURN_FUEL, limits::BEFORE_COMMIT_FUEL, left)
            }
            _ => script.resume_turn(1, left, limits::BEFORE_COMMIT_FUEL, limits::COMMAND_FUEL),
        };
        let declared = declaration(&[], &[hook]);
        // 开头只剩一点（远不到自己上限的一半）：这是没经过 `msime_client_key` 的派发接着上一轮的零头，被截断不算失败。
        let mut small = instance(&spinner(), declared);
        resume(&mut small, 1_000);
        assert_eq!(
            small.deliver(1, &event_for(hook)),
            Outcome::TurnExhausted { fuel: 1_000 },
            "{hook:?}"
        );
        // 差一点到一半：仍然不算。
        let mut below = instance(&spinner(), declared);
        resume(&mut below, half - 1);
        let outcome = below.deliver(1, &event_for(hook));
        assert!(
            matches!(outcome, Outcome::TurnExhausted { fuel } if fuel > half - 10 && fuel < half),
            "{hook:?}: {outcome:?}"
        );
        // 正好用掉一半（死循环每圈的 fuel 是偶数，余额是偶数时恰好用完）：算一次 fuel 耗尽的失败，和超出自己上限的那种一样。
        let mut at = instance(&spinner(), declared);
        resume(&mut at, half);
        assert_eq!(
            at.deliver(1, &event_for(hook)),
            Outcome::Failed {
                failure: Failure::OutOfFuel,
                fuel: half,
            },
            "{hook:?}"
        );
        // 两种情形里本轮这一份都记为用完；`before_commit` 和 `command` 跑在快照上，持久实例照样可用。
        for script in [&below, &at] {
            assert_eq!(script.is_usable(), hook.runs_on_snapshot(), "{hook:?}");
            let left = match hook {
                Hook::BeforeCommit => script.before_commit_fuel_left(),
                Hook::Command => script.command_fuel_left(),
                _ => script.turn_fuel_left(),
            };
            assert_eq!(left, 0, "{hook:?}");
        }
    }
}

#[test]
fn before_commit_has_its_own_turn_quota() {
    // `before_commit` 循环 1 万次，其余钩子 1 千次。
    let wat = module_wat(
        "",
        "",
        &format!(
            "(local $i i32)
             (local.set $i (select (i32.const 10000) (i32.const 1000) (i32.eq (local.get $kind) (i32.const {before_commit}))))
             (loop $again
                (local.set $i (i32.sub (local.get $i) (i32.const 1)))
                (br_if $again (local.get $i)))
             i32.const 0",
            before_commit = Hook::BeforeCommit.code(),
        ),
    );
    let mut script = instance(&wat, declaration(&[], &[Hook::Input, Hook::BeforeCommit]));
    let per_input = match script.deliver(3, &input_event()) {
        Outcome::Delivered { fuel, .. } => fuel,
        other => panic!("{other:?}"),
    };
    // 持久实例把这一轮的配额烧到放不下下一次 input。
    while script.turn_fuel_left() >= per_input {
        assert!(matches!(
            script.deliver(3, &input_event()),
            Outcome::Delivered { .. }
        ));
    }
    let hooks_left = script.turn_fuel_left();
    let event = before_commit_event("abc", "abc");
    let spent = match script.deliver(3, &event) {
        Outcome::Delivered { fuel, .. } => fuel,
        other => panic!("{other:?}"),
    };
    // 共用一份配额时这次会 fuel 耗尽，改写被丢掉；现在它拿到的和持久实例用掉多少无关，也不扣其余钩子的配额。
    assert!(spent > hooks_left);
    assert_eq!(script.turn_fuel_left(), hooks_left);
    assert_eq!(
        script.before_commit_fuel_left(),
        limits::BEFORE_COMMIT_FUEL - spent
    );
    // 新一轮三份都补满。
    assert!(matches!(
        script.deliver(4, &input_event()),
        Outcome::Delivered { .. }
    ));
    assert_eq!(script.before_commit_fuel_left(), limits::BEFORE_COMMIT_FUEL);
    assert_eq!(script.command_fuel_left(), limits::COMMAND_FUEL);
    assert_eq!(script.turn_fuel_left(), limits::TURN_FUEL - per_input);
}

#[test]
fn one_plugin_burning_its_quota_does_not_touch_another() {
    let mut greedy = instance(
        &module_wat("", "", "(loop $spin (br $spin)) i32.const 0"),
        declaration(&[], &[Hook::Input]),
    );
    let mut modest = instance(&burner(100), declaration(&[], &[Hook::Input]));
    let alone = match modest.deliver(1, &input_event()) {
        Outcome::Delivered { fuel, .. } => fuel,
        other => panic!("{other:?}"),
    };
    assert_eq!(
        failure(greedy.deliver(2, &input_event())),
        Failure::OutOfFuel
    );
    assert_eq!(
        modest.deliver(2, &input_event()),
        Outcome::Delivered {
            status: 0,
            fuel: alone,
        }
    );
    assert_eq!(modest.turn_fuel_left(), limits::TURN_FUEL - alone);
}

#[test]
fn imports_cost_fuel() {
    let plain = instance(
        &module_wat("", "", "i32.const 0"),
        declaration(&[], &[Hook::Input]),
    );
    let calling = instance(
        &module_wat(
            r#"(import "msime" "event_scalar" (func $scalar (param i32) (result i64)))"#,
            "",
            "(drop (call $scalar (i32.const 0))) i32.const 0",
        ),
        declaration(&[], &[Hook::Input]),
    );
    let fuel = |mut script: ScriptInstance| match script.deliver(1, &input_event()) {
        Outcome::Delivered { fuel, .. } => fuel,
        other => panic!("{other:?}"),
    };
    assert!(fuel(calling) >= fuel(plain) + limits::IMPORT_CALL_FUEL);
}

// ---- 快照复位 ----

const SNAPSHOT_WAT: &str = r#"(module
    (memory (export "memory") 1)
    (global $exported (export "counter") (mut i32) (i32.const 1))
    (global $hidden (mut i32) (i32.const 1))
    (data (i32.const 200) "abc")
    (func (export "msime_abi") (result i32) i32.const 1)
    (func (export "msime_init") (result i32)
        (i32.store8 (i32.const 100) (i32.const 7))
        (i32.store8 (i32.const 200) (i32.const 0))
        (i32.store8 (i32.const 201) (i32.const 0))
        (i32.store8 (i32.const 202) (i32.const 0))
        (global.set $exported (i32.const 5))
        (global.set $hidden (i32.const 5))
        i32.const 0)
    (func (export "msime_on_event") (param $kind i32) (result i32)
        (if (i32.eq (local.get $kind) (i32.const 9))
            (then
                ;; before_commit：把看到的状态编码进返回值，再弄脏它。
                (i32.add
                    (i32.add
                        (i32.mul (i32.load8_u (i32.const 100)) (i32.const 1000))
                        (i32.mul (global.get $exported) (i32.const 100)))
                    (i32.add
                        (i32.mul (global.get $hidden) (i32.const 10))
                        (i32.load8_u (i32.const 200))))
                (i32.store8 (i32.const 100) (i32.const 55))
                (global.set $exported (i32.const 8))
                (global.set $hidden (i32.const 8))
                return))
        ;; 其余事件：在持久实例上留下状态，返回之前的内存值。
        (i32.load8_u (i32.const 100))
        (i32.store8 (i32.const 100) (i32.const 99))
        (i32.store8 (i32.const 200) (i32.const 3))
        (global.set $exported (i32.const 9))
        (global.set $hidden (i32.const 9))))"#;

#[test]
fn before_commit_runs_on_the_post_init_snapshot() {
    let mut script = instance(
        SNAPSHOT_WAT,
        declaration(&[], &[Hook::Commit, Hook::BeforeCommit]),
    );
    let before_commit = Event::new(Hook::BeforeCommit);
    let commit = Event::new(Hook::Commit);
    // 内存 [100] 是 init 写的 7，导出的 global 是 init 设的 5，未导出的 global 回到声明的初值 1，数据段被 init 清掉的 [200] 仍是 0。
    let expected = 7 * 1000 + 5 * 100 + 10;
    assert_eq!(status_of(script.deliver(1, &before_commit)), expected);
    // 观察类事件改了持久实例：它们的状态进不了下一次 before_commit。
    assert_eq!(status_of(script.deliver(2, &commit)), 7);
    assert_eq!(status_of(script.deliver(3, &before_commit)), expected);
    // before_commit 自己的改动也不留下，持久实例看到的仍是它自己上次写的 99。
    assert_eq!(status_of(script.deliver(4, &commit)), 99);
    assert_eq!(status_of(script.deliver(5, &before_commit)), expected);
}

#[test]
fn a_trap_on_the_snapshot_keeps_the_persistent_instance() {
    let wat = module_wat(
        "",
        "",
        "(if (i32.eq (local.get $kind) (i32.const 9)) (then unreachable)) i32.const 0",
    );
    let mut script = instance(&wat, declaration(&[], &[Hook::Commit, Hook::BeforeCommit]));
    assert_eq!(
        failure(script.deliver(1, &Event::new(Hook::BeforeCommit))),
        Failure::Unreachable
    );
    assert!(script.is_usable());
    assert_eq!(status_of(script.deliver(1, &Event::new(Hook::Commit))), 0);
}

// ---- import ----

const READER_IMPORTS: &str = r#"
    (import "msime" "event_scalar" (func $event (param i32) (result i64)))
    (import "msime" "commit_text" (func $commit_text (param i32 i32 i32) (result i32)))
    (import "msime" "commit_scalar" (func $commit_scalar (param i32) (result i64)))
    (import "msime" "composition_text" (func $composition_text (param i32 i32 i32) (result i32)))
    (import "msime" "candidate_count" (func $count (result i32)))
    (import "msime" "candidate_text" (func $candidate_text (param i32 i32 i32) (result i32)))
    (import "msime" "candidate_meta" (func $candidate_meta (param i32 i32) (result i64)))
    (import "msime" "log" (func $log (param i32 i32 i32)))
"#;

/// 由导出的 global `$probe` 选择做哪一项探测，结果写进 [0..) 并作为返回值。
fn reader(probe_body: &str) -> String {
    format!(
        r#"(module
            {READER_IMPORTS}
            (memory (export "memory") 1)
            (func (export "msime_abi") (result i32) i32.const 1)
            (func (export "msime_on_event") (param $kind i32) (result i32) {probe_body}))"#
    )
}

const READS: [Capability; 3] = [
    Capability::ReadComposition,
    Capability::ReadCommit,
    Capability::ReadCandidates,
];

fn reader_instance(probe_body: &str, grants: &[Capability]) -> ScriptInstance {
    let module =
        compile_wat(&reader(probe_body), declaration(&READS, &Hook::ALL)).expect("reader compiles");
    ScriptInstance::new(&module, grants.iter().copied().collect(), limits::INIT_FUEL).unwrap()
}

fn commit_event(text: &'static str) -> Event<'static> {
    let mut event = Event::new(Hook::Commit);
    event.scalars.local_mode = Some(0);
    event.scalars.origin = Some(0);
    event.commit.commit = Some(text);
    event.commit.body = Some(text);
    event.commit.seat = Some(2);
    event
}

#[test]
fn event_scalars_need_no_capability() {
    let mut script = reader_instance(
        &format!(
            "(i32.wrap_i64 (i64.add (i64.mul (call $event (i32.const {})) (i64.const 100)) (call $event (i32.const {}))))",
            event_field::KIND,
            event_field::ORIGIN
        ),
        &[],
    );
    assert_eq!(
        status_of(script.deliver(1, &commit_event("你好"))),
        Hook::Commit.code() * 100
    );
    let mut absent = reader_instance(
        &format!(
            "(i32.wrap_i64 (call $event (i32.const {})))",
            event_field::HOW
        ),
        &[],
    );
    assert_eq!(
        failure(absent.deliver(1, &commit_event("你好"))),
        Failure::NegativeReturn
    );
    let mut invalid = reader_instance(
        "(i32.wrap_i64 (i64.add (call $event (i32.const 99)) (i64.const 100)))",
        &[],
    );
    assert_eq!(
        status_of(invalid.deliver(1, &commit_event("你好"))),
        100 + status::INVALID
    );
}

/// 把 commit 文本复制到 [16..) 后返回 `commit_text` 的返回值。
fn copy_commit(cap: i32) -> String {
    format!(
        "(call $commit_text (i32.const {}) (i32.const 16) (i32.const {cap}))",
        commit_field::COMMIT
    )
}

#[test]
fn text_imports_copy_only_what_is_granted() {
    let text = "你好，世界";
    let mut granted = reader_instance(&copy_commit(64), &[Capability::ReadCommit]);
    assert_eq!(
        status_of(granted.deliver(1, &commit_event(text))),
        text.len() as i32
    );
    // 授权里没有 read_commit：DENIED。
    let mut ungranted = reader_instance(
        &format!("(i32.sub (i32.const 0) {})", copy_commit(64)),
        &[Capability::ReadComposition],
    );
    assert_eq!(
        status_of(ungranted.deliver(1, &commit_event(text))),
        -status::DENIED
    );
    // 屏蔽名单里的本地模式（提及）：与没有这个字段一样是 ABSENT。
    let mut blocked = reader_instance(
        &format!("(i32.sub (i32.const 0) {})", copy_commit(64)),
        &[Capability::ReadCommit],
    );
    let mut mention = commit_event(text);
    mention.scalars.local_mode = Some(6);
    assert_eq!(status_of(blocked.deliver(1, &mention)), -status::ABSENT);
    let mut unknown_mode = commit_event(text);
    unknown_mode.scalars.local_mode = None;
    assert_eq!(
        status_of(blocked.deliver(2, &unknown_mode)),
        -status::ABSENT
    );
    // 缓冲区太小：LIMIT，什么都不复制。
    let mut small = reader_instance(
        &format!("(i32.sub (i32.const 0) {})", copy_commit(4)),
        &[Capability::ReadCommit],
    );
    assert_eq!(
        status_of(small.deliver(1, &commit_event(text))),
        -status::LIMIT
    );
}

#[test]
fn copied_text_lands_in_linear_memory() {
    // 复制后把第一个字节和长度一起编码返回：长度 * 1000 + 首字节。
    let body = format!(
        "(i32.add (i32.mul {} (i32.const 1000)) (i32.load8_u (i32.const 16)))",
        copy_commit(64)
    );
    let mut script = reader_instance(&body, &[Capability::ReadCommit]);
    assert_eq!(
        status_of(script.deliver(1, &commit_event("abc"))),
        3 * 1000 + i32::from(b'a')
    );
    let mut seat = reader_instance(
        &format!(
            "(i32.wrap_i64 (call $commit_scalar (i32.const {})))",
            commit_field::SEAT
        ),
        &[Capability::ReadCommit],
    );
    assert_eq!(status_of(seat.deliver(1, &commit_event("abc"))), 2);
}

#[test]
fn an_out_of_bounds_destination_traps() {
    let mut script = reader_instance(
        &format!(
            "(call $commit_text (i32.const {}) (i32.const 65530) (i32.const 64))",
            commit_field::COMMIT
        ),
        &[Capability::ReadCommit],
    );
    // 16 页预留之内是合法的；越过预留才 trap。
    assert!(matches!(
        script.deliver(1, &commit_event("abc")),
        Outcome::Delivered { .. }
    ));
    let mut beyond = reader_instance(
        &format!(
            "(call $commit_text (i32.const {}) (i32.const 0x00ff_fff0) (i32.const 64))",
            commit_field::COMMIT
        ),
        &[Capability::ReadCommit],
    );
    assert_eq!(
        failure(beyond.deliver(1, &commit_event("abc"))),
        Failure::MemoryOutOfBounds
    );
}

#[test]
fn composition_and_candidate_reads() {
    let mut composition = reader_instance(
        &format!(
            "(call $composition_text (i32.const {}) (i32.const 0) (i32.const 64))",
            composition_field::PREEDIT
        ),
        &[Capability::ReadComposition],
    );
    let mut update = Event::new(Hook::CompositionUpdate);
    update.scalars.local_mode = Some(0);
    update.composition.preedit = Some("ni hao");
    assert_eq!(status_of(composition.deliver(1, &update)), 6);

    let rows = [
        CandidateRow {
            text: "你好",
            code: "nihao",
            annotation: "",
            source: 0,
        },
        CandidateRow {
            text: "拟好",
            code: "nihao",
            annotation: "",
            source: 3,
        },
    ];
    let mut shown = Event::new(Hook::CandidatesShown);
    shown.scalars.local_mode = Some(0);
    shown.candidates = Some(Candidates {
        rows: &rows,
        page: Some(0),
        highlighted: Some(1),
    });
    let mut candidates = reader_instance(
        &format!(
            "(i32.add (i32.mul (call $count) (i32.const 100))
                (i32.add
                    (i32.mul (i32.wrap_i64 (call $candidate_meta (i32.const 1) (i32.const {}))) (i32.const 10))
                    (i32.wrap_i64 (call $candidate_meta (i32.const 0) (i32.const {})))))",
            candidate_field::SOURCE,
            candidate_field::HIGHLIGHTED
        ),
        &[Capability::ReadCandidates],
    );
    assert_eq!(
        status_of(candidates.deliver(1, &shown)),
        2 * 100 + 3 * 10 + 1
    );
    let mut text = reader_instance(
        "(call $candidate_text (i32.const 1) (i32.const 0) (i32.const 64))",
        &[Capability::ReadCandidates],
    );
    assert_eq!(status_of(text.deliver(1, &shown)), "拟好".len() as i32);
    let mut missing_row = reader_instance(
        "(i32.sub (i32.const 0) (call $candidate_text (i32.const 2) (i32.const 0) (i32.const 64)))",
        &[Capability::ReadCandidates],
    );
    assert_eq!(status_of(missing_row.deliver(1, &shown)), -status::ABSENT);
}

#[test]
fn grants_beyond_the_declaration_are_ignored() {
    let wat = module_wat(
        r#"(import "msime" "commit_text" (func $commit_text (param i32 i32 i32) (result i32)))"#,
        "",
        "i32.const 0",
    );
    let module = compile_wat(
        &wat,
        declaration(&[Capability::ReadCommit], &[Hook::Commit]),
    )
    .unwrap();
    let all: CapabilitySet = Capability::ALL.into_iter().collect();
    let script = ScriptInstance::new(&module, all, limits::INIT_FUEL).unwrap();
    assert_eq!(
        script.grants(),
        CapabilitySet::EMPTY.with(Capability::ReadCommit)
    );
}

// ---- 修改类 import ----

/// 读 body、改写上屏的模块：`data` 是放在 [64..) 的数据段，`body` 是 `msime_on_event` 的函数体。
fn rewriter(data: &str, body: &str) -> ScriptInstance {
    let wat = module_wat(
        r#"(import "msime" "commit_text" (func $commit_text (param i32 i32 i32) (result i32)))
           (import "msime" "rewrite_commit" (func $rewrite (param i32 i32) (result i32)))"#,
        &format!(r#"(data (i32.const 64) "{data}")"#),
        body,
    );
    instance(
        &wat,
        declaration(
            &[Capability::ReadCommit, Capability::RewriteCommit],
            &[Hook::Commit, Hook::BeforeCommit],
        ),
    )
}

/// 一次可改写的 `before_commit`，body 是 `body`，尾巴是一个全角逗号。
fn before_commit_event(body: &'static str, full: &'static str) -> Event<'static> {
    let mut event = Event::new(Hook::BeforeCommit);
    event.scalars.local_mode = Some(0);
    event.scalars.origin = Some(0);
    event.scalars.rewritable = Some(true);
    event.commit.commit = Some(full);
    event.commit.body = Some(body);
    event
}

/// 改写 [64, 64 + len) 并返回 `rewrite_commit` 的返回码加 10：成功是 10，`DENIED` 是 9，`INVALID` 是 7，`LIMIT` 是 6。
fn rewrite_status(data: &str, len: usize, event: &Event<'_>) -> (i32, Option<String>) {
    let mut script = rewriter(
        data,
        &format!("(i32.add (call $rewrite (i32.const 64) (i32.const {len})) (i32.const 10))"),
    );
    let status = status_of(script.deliver(1, event));
    (status, script.actions().rewrite.clone())
}

#[test]
fn rewrite_commit_stages_a_valid_body() {
    let event = before_commit_event("你好world", "你好world，");
    let pangu = "你好 world";
    assert_eq!(
        rewrite_status(pangu, pangu.len(), &event),
        (10, Some(pangu.to_owned()))
    );
}

#[test]
fn rewrite_commit_rejects_controls_formats_empty_and_oversized_text() {
    let event = before_commit_event("abc", "abc");
    // 换行是 Cc，U+202E（从右到左覆盖）和 U+200B（零宽空格）是 Cf，U+2028、U+2029 和换行一样断行。
    assert_eq!(rewrite_status("\\0a", 1, &event), (7, None));
    assert_eq!(rewrite_status("a\\e2\\80\\aeb", 5, &event), (7, None));
    assert_eq!(rewrite_status("a\\e2\\80\\8bb", 5, &event), (7, None));
    assert_eq!(rewrite_status("a\\e2\\80\\a8b", 5, &event), (7, None));
    assert_eq!(rewrite_status("a\\e2\\80\\a9b", 5, &event), (7, None));
    assert_eq!(rewrite_status("", 0, &event), (7, None));
    // 不是 UTF-8。
    assert_eq!(rewrite_status("\\ff", 1, &event), (7, None));
    // 原文 3 个码元，上限取 256：257 个字母放不下，256 个正好。
    let long = "a".repeat(257);
    assert_eq!(rewrite_status(&long, 257, &event), (6, None));
    assert_eq!(rewrite_status(&long, 256, &event).0, 10);
    // 超过 4096 字节，不论原文多长。
    let huge = "a".repeat(limits::MAX_REWRITE_BYTES + 1);
    let mut long_body = before_commit_event("", "");
    let original: &'static str = "b".repeat(2000).leak();
    long_body.commit.body = Some(original);
    long_body.commit.commit = Some(original);
    assert_eq!(rewrite_status(&huge, huge.len(), &long_body), (6, None));
}

#[test]
fn the_rewrite_limit_follows_the_original_body_not_an_earlier_rewrite() {
    // 前一个插件把 100 个码元的原文缩成了 3 个：上限仍是 400，300 个字母放得下。
    let mut shortened = before_commit_event("abc", "abc");
    shortened.commit.original_body_utf16 = Some(100);
    let three_hundred = "a".repeat(300);
    assert_eq!(rewrite_status(&three_hundred, 300, &shortened).0, 10);
    // 前一个插件把 2 个码元的原文放大到 256 个：上限仍是 256，不是 1024。
    let grown: &'static str = "b".repeat(256).leak();
    let mut lengthened = before_commit_event(grown, grown);
    lengthened.commit.original_body_utf16 = Some(2);
    assert_eq!(rewrite_status(&three_hundred, 300, &lengthened), (6, None));
    assert_eq!(rewrite_status(&three_hundred, 256, &lengthened).0, 10);
}

#[test]
fn rewrite_commit_is_denied_outside_a_rewritable_before_commit() {
    let mut observe_only = before_commit_event("abc", "abc");
    observe_only.scalars.rewritable = Some(false);
    assert_eq!(rewrite_status("xyz", 3, &observe_only), (9, None));
    let mut commit = before_commit_event("abc", "abc");
    commit.hook = Hook::Commit;
    assert_eq!(rewrite_status("xyz", 3, &commit), (9, None));
    // 声明了能力却没被授予。
    let wat = module_wat(
        r#"(import "msime" "commit_text" (func $commit_text (param i32 i32 i32) (result i32)))
           (import "msime" "rewrite_commit" (func $rewrite (param i32 i32) (result i32)))"#,
        r#"(data (i32.const 64) "xyz")"#,
        "(i32.add (call $rewrite (i32.const 64) (i32.const 3)) (i32.const 10))",
    );
    let declared = declaration(
        &[Capability::ReadCommit, Capability::RewriteCommit],
        &[Hook::BeforeCommit],
    );
    let module = compile_wat(&wat, declared).unwrap();
    let mut script = ScriptInstance::new(
        &module,
        CapabilitySet::EMPTY.with(Capability::ReadCommit),
        limits::INIT_FUEL,
    )
    .unwrap();
    assert_eq!(
        status_of(script.deliver(1, &before_commit_event("abc", "abc"))),
        9
    );
}

#[test]
fn a_failed_hook_hands_over_no_actions() {
    let event = before_commit_event("abc", "abc");
    // 改写之后返回负数、trap、越界：这一次暂存的改写全部丢弃。
    for (body, expected) in [
        (
            "(drop (call $rewrite (i32.const 64) (i32.const 3))) (i32.const -1)",
            Failure::NegativeReturn,
        ),
        (
            "(drop (call $rewrite (i32.const 64) (i32.const 3))) unreachable",
            Failure::Unreachable,
        ),
        (
            "(drop (call $rewrite (i32.const -8) (i32.const 100))) (i32.const 0)",
            Failure::MemoryOutOfBounds,
        ),
    ] {
        let mut script = rewriter("xyz", body);
        assert_eq!(failure(script.deliver(1, &event)), expected, "{body}");
        assert!(script.actions().is_empty(), "{body}");
    }
    // 下一次正常返回，只交出这一次的改写。
    let mut script = rewriter(
        "xyz",
        "(drop (call $rewrite (i32.const 64) (i32.const 3))) (i32.const 0)",
    );
    status_of(script.deliver(1, &event));
    assert_eq!(script.actions().rewrite.as_deref(), Some("xyz"));
    // 不是修改类钩子的投递把上一次的动作清掉。
    status_of(script.deliver(2, &commit_event("abc")));
    assert!(script.actions().is_empty());
}

const CANDIDATE_IMPORTS: &str = r#"(import "msime" "candidate_count" (func $count (result i32)))
    (import "msime" "candidates_order" (func $order (param i32 i32) (result i32)))
    (import "msime" "candidates_hide" (func $hide (param i32 i32) (result i32)))
    (import "msime" "candidates_annotate" (func $annotate (param i32 i32 i32) (result i32)))"#;

const TRANSFORMS: [Capability; 4] = [
    Capability::ReadCandidates,
    Capability::RerankCandidates,
    Capability::AnnotateCandidates,
    Capability::FilterCandidates,
];

/// 候选变换的模块：[0..) 是数据段 `data`。
fn transformer(data: &str, body: &str, grants: &[Capability]) -> ScriptInstance {
    let wat = module_wat(
        CANDIDATE_IMPORTS,
        &format!(r#"(data (i32.const 0) "{data}")"#),
        body,
    );
    let module = compile_wat(
        &wat,
        declaration(&TRANSFORMS, &[Hook::Candidates, Hook::CandidatesShown]),
    )
    .unwrap();
    ScriptInstance::new(&module, grants.iter().copied().collect(), limits::INIT_FUEL).unwrap()
}

const WINDOW: [CandidateRow<'static>; 3] = [
    CandidateRow {
        text: "你好",
        code: "nihao",
        annotation: "",
        source: 0,
    },
    CandidateRow {
        text: "拟好",
        code: "nihao",
        annotation: "",
        source: 0,
    },
    CandidateRow {
        text: "泥好",
        code: "nihao",
        annotation: "",
        source: 0,
    },
];

fn candidates_event(hook: Hook, local_mode: u8) -> Event<'static> {
    let mut event = Event::new(hook);
    event.scalars.local_mode = Some(local_mode);
    event.candidates = Some(Candidates {
        rows: &WINDOW,
        page: None,
        highlighted: None,
    });
    event
}

/// 跑一次 `body` 并返回它的返回值（返回码加 10）。
fn transform_status(data: &str, body: &str, event: &Event<'_>) -> i32 {
    status_of(transformer(data, body, &TRANSFORMS).deliver(1, event))
}

#[test]
fn candidate_imports_stage_order_hide_and_annotations() {
    // [0..8) 是行号 2、0；[8..12) 是行号 1；[16..) 是注释"常用"。
    let data = r"\02\00\00\00\00\00\00\00\01\00\00\00\00\00\00\00\e5\b8\b8\e7\94\a8";
    let mut script = transformer(
        data,
        "(drop (call $order (i32.const 0) (i32.const 2)))
         (drop (call $hide (i32.const 8) (i32.const 1)))
         (drop (call $hide (i32.const 8) (i32.const 1)))
         (drop (call $annotate (i32.const 2) (i32.const 16) (i32.const 6)))
         (i32.const 0)",
        &TRANSFORMS,
    );
    status_of(script.deliver(1, &candidates_event(Hook::Candidates, 0)));
    assert_eq!(
        script.actions(),
        &Actions {
            rewrite: None,
            order: vec![2, 0],
            hidden: vec![1],
            annotations: vec![(2, "常用".to_owned())],
            rows: Vec::new(),
            inserts: Vec::new(),
        }
    );
}

#[test]
fn candidate_imports_reject_bad_rows_and_text() {
    let event = candidates_event(Hook::Candidates, 0);
    let call = |call: &str| format!("(i32.add {call} (i32.const 10))");
    // 重复的行号、越界的行号、比窗口还多的行。
    assert_eq!(
        transform_status(
            r"\00\00\00\00\00\00\00\00",
            &call("(call $order (i32.const 0) (i32.const 2))"),
            &event
        ),
        7
    );
    assert_eq!(
        transform_status(
            r"\03\00\00\00",
            &call("(call $hide (i32.const 0) (i32.const 1))"),
            &event
        ),
        7
    );
    assert_eq!(
        transform_status(
            "",
            &call("(call $order (i32.const 0) (i32.const 4))"),
            &event
        ),
        7
    );
    assert_eq!(
        transform_status(
            "",
            &call("(call $order (i32.const 0) (i32.const -1))"),
            &event
        ),
        7
    );
    // 注释：33 个码元放不下，32 个正好；格式字符、行分隔符和越界的行号不行。
    let a33 = "a".repeat(33);
    assert_eq!(
        transform_status(
            &a33,
            &call("(call $annotate (i32.const 0) (i32.const 0) (i32.const 33))"),
            &event
        ),
        6
    );
    assert_eq!(
        transform_status(
            &a33,
            &call("(call $annotate (i32.const 0) (i32.const 0) (i32.const 32))"),
            &event
        ),
        10
    );
    assert_eq!(
        transform_status(
            r"a\e2\80\8b",
            &call("(call $annotate (i32.const 0) (i32.const 0) (i32.const 4))"),
            &event
        ),
        7
    );
    assert_eq!(
        transform_status(
            r"a\e2\80\a8",
            &call("(call $annotate (i32.const 0) (i32.const 0) (i32.const 4))"),
            &event
        ),
        7
    );
    assert_eq!(
        transform_status(
            "a",
            &call("(call $annotate (i32.const 3) (i32.const 0) (i32.const 1))"),
            &event
        ),
        7
    );
    // 行号区间越界直接 trap。
    let mut script = transformer(
        "",
        "(call $order (i32.const -4) (i32.const 2))",
        &TRANSFORMS,
    );
    assert_eq!(
        failure(script.deliver(1, &event)),
        Failure::MemoryOutOfBounds
    );
}

#[test]
fn candidate_imports_need_the_candidates_hook_the_grant_and_an_allowed_mode() {
    let order = "(i32.add (call $order (i32.const 0) (i32.const 1)) (i32.const 10))";
    // `candidates_shown` 只能读，不能改。
    assert_eq!(
        transform_status(
            r"\00\00\00\00",
            order,
            &candidates_event(Hook::CandidatesShown, 0)
        ),
        9
    );
    // 本地模式不在白名单里（提及）：窗口按屏蔽处理。
    assert_eq!(
        transform_status(
            r"\00\00\00\00",
            order,
            &candidates_event(Hook::Candidates, 6)
        ),
        8
    );
    // 只授予了读：三种修改都 DENIED。
    let mut script = transformer(
        r"\00\00\00\00",
        "(i32.add (i32.add (call $order (i32.const 0) (i32.const 1)) (call $hide (i32.const 0) (i32.const 1)))
             (i32.add (call $annotate (i32.const 0) (i32.const 0) (i32.const 1)) (i32.const 10)))",
        &[Capability::ReadCandidates],
    );
    assert_eq!(
        status_of(script.deliver(1, &candidates_event(Hook::Candidates, 0))),
        10 + 3 * status::DENIED
    );
    assert!(script.actions().is_empty());
}

#[test]
fn the_plugin_text_check_rejects_cc_cf_and_line_separators() {
    for c in [
        '\n',
        '\u{7f}',
        '\u{ad}',
        '\u{200b}',
        '\u{200d}',
        '\u{202e}',
        '\u{2028}',
        '\u{2029}',
        '\u{2066}',
        '\u{feff}',
        '\u{e0041}',
    ] {
        assert!(abi::is_forbidden_plugin_char(c), "{c:?}");
    }
    for c in ['a', ' ', '你', '\u{3000}', '\u{fe0f}', '😀'] {
        assert!(!abi::is_forbidden_plugin_char(c), "{c:?}");
    }
}

/// 不是 Cf、却看不见或只改字形的字符：组合用字形连接符、韩文填充符、高棉文固有元音、蒙古文自由变体选择符、盲文空白、变体选择符 VS1..VS14 和表意文字变体选择符。
const INVISIBLE_NON_FORMAT: [char; 22] = [
    '\u{34f}',
    '\u{115f}',
    '\u{1160}',
    '\u{17b4}',
    '\u{17b5}',
    '\u{180b}',
    '\u{180c}',
    '\u{180d}',
    '\u{180f}',
    '\u{2800}',
    '\u{3164}',
    '\u{ffa0}',
    '\u{fe00}',
    '\u{fe01}',
    '\u{fe07}',
    '\u{fe0c}',
    '\u{fe0d}',
    '\u{e0100}',
    '\u{e0101}',
    '\u{e0150}',
    '\u{e01ee}',
    '\u{e01ef}',
];

/// 把文本写成 WAT 数据段里的逐字节转义。
fn wat_bytes(text: &str) -> String {
    text.bytes().map(|byte| format!("\\{byte:02x}")).collect()
}

#[test]
fn the_plugin_text_check_rejects_invisible_fillers_and_variation_selectors() {
    for c in INVISIBLE_NON_FORMAT {
        assert!(abi::is_forbidden_plugin_char(c), "{c:?}");
    }
    // 范围的两端之外照旧放行：VS15、VS16 和表意文字变体选择符之后的第一个码位。
    for c in [
        '\u{fe0e}',
        '\u{fe0f}',
        '\u{e01f0}',
        '\u{17b3}',
        '\u{17b6}',
        '\u{2801}',
    ] {
        assert!(!abi::is_forbidden_plugin_char(c), "{c:?}");
    }
}

#[test]
fn rewrites_and_annotations_with_invisible_fillers_are_invalid() {
    let commit = before_commit_event("abc", "abc");
    let candidates = candidates_event(Hook::Candidates, 0);
    for c in INVISIBLE_NON_FORMAT {
        let text = format!("a{c}b");
        let data = wat_bytes(&text);
        assert_eq!(
            rewrite_status(&data, text.len(), &commit),
            (7, None),
            "{c:?}"
        );
        let annotate = format!(
            "(i32.add (call $annotate (i32.const 0) (i32.const 0) (i32.const {})) (i32.const 10))",
            text.len()
        );
        assert_eq!(transform_status(&data, &annotate, &candidates), 7, "{c:?}");
    }
    // 跟在 emoji 后面的 VS16 照常放行：改写和注释都成功。
    let heart = "\u{2764}\u{fe0f}";
    let data = wat_bytes(heart);
    assert_eq!(
        rewrite_status(&data, heart.len(), &commit),
        (10, Some(heart.to_owned()))
    );
    let annotate = format!(
        "(i32.add (call $annotate (i32.const 0) (i32.const 0) (i32.const {})) (i32.const 10))",
        heart.len()
    );
    assert_eq!(transform_status(&data, &annotate, &candidates), 10);
}

/// 调试输出只旁观：修改类 import 照样返回 `DENIED`，`before_commit` 的快照实例上的调用也交给同一个输出，`msime_init` 里的 `log` 在实例化前接上的输出里。
#[test]
fn debug_sinks_observe_denied_modifications_and_init_logs() {
    let wat = module_wat(
        r#"(import "msime" "commit_text" (func (param i32 i32 i32) (result i32)))
           (import "msime" "rewrite_commit" (func $rewrite (param i32 i32) (result i32)))
           (import "msime" "log" (func $log (param i32 i32 i32)))"#,
        r#"(data (i32.const 0) "up")
           (func (export "msime_init") (result i32)
               (call $log (i32.const 2) (i32.const 0) (i32.const 2))
               i32.const 0)"#,
        "(i32.sub (i32.const 0) (call $rewrite (i32.const 0) (i32.const 3)))",
    );
    let declared = declaration(
        &[Capability::ReadCommit, Capability::RewriteCommit],
        &[Hook::BeforeCommit, Hook::Commit],
    );
    let module = compile_wat(&wat, declared).unwrap();
    let logs = Arc::new(Mutex::new(Vec::new()));
    let actions = Arc::new(Mutex::new(Vec::new()));
    let (log_sink, action_sink) = (Arc::clone(&logs), Arc::clone(&actions));
    let mut script = ScriptInstance::with_debug(
        &module,
        declared.capabilities,
        limits::INIT_FUEL,
        DebugSinks {
            log: Some(Box::new(move |level, text| {
                log_sink.lock().unwrap().push((level, text.to_owned()));
            })),
            actions: Some(Box::new(move |import, status| {
                action_sink.lock().unwrap().push((import, status));
            })),
        },
    )
    .unwrap();
    assert_eq!(*logs.lock().unwrap(), vec![(2, "up".to_owned())]);

    let mut event = commit_event("abc");
    event.hook = Hook::BeforeCommit;
    assert_eq!(status_of(script.deliver(1, &event)), -status::DENIED);
    assert_eq!(
        status_of(script.deliver(2, &commit_event("abc"))),
        -status::DENIED
    );
    assert_eq!(
        *actions.lock().unwrap(),
        vec![
            ("rewrite_commit", status::DENIED),
            ("rewrite_commit", status::DENIED)
        ]
    );
    // 输出交还给了持久实例：之后的调用照样看得到。
    assert_eq!(status_of(script.deliver(3, &event)), -status::DENIED);
    assert_eq!(actions.lock().unwrap().len(), 3);
}

/// `event_scalar` 的取值表：名字和数值一一对应，本地模式表里 0 到 5 正好是 U6 白名单。
#[test]
fn the_scalar_value_tables_are_one_to_one() {
    use crate::abi::scalar_values::{self as values, name, value};
    for table in [
        values::ORIGIN,
        values::HOW,
        values::OUTCOME,
        values::REASON,
        values::LOCAL_MODE,
        values::INPUT_CLASS,
        values::RETRACTION,
    ] {
        for (entry, number) in table {
            assert_eq!(value(table, entry), Some(*number));
            assert_eq!(name(table, *number), Some(*entry));
        }
    }
    for (entry, number) in values::LOCAL_MODE {
        assert_eq!(
            abi::local_mode_allows_text(*number),
            *number <= 5,
            "{entry}"
        );
    }
}

#[test]
fn init_sees_no_event_and_texts_do_not_outlive_the_call() {
    // init 把 commit_text 的返回值存到 [0]；事件里返回它。
    let wat = format!(
        r#"(module
            {READER_IMPORTS}
            (memory (export "memory") 1)
            (func (export "msime_abi") (result i32) i32.const 1)
            (func (export "msime_init") (result i32)
                (i32.store (i32.const 0)
                    (call $commit_text (i32.const {commit}) (i32.const 16) (i32.const 64)))
                i32.const 0)
            (func (export "msime_on_event") (param $kind i32) (result i32)
                (if (i32.eq (local.get $kind) (i32.const {focus_in}))
                    (then (return (i32.sub (i32.const 0)
                        (call $commit_text (i32.const {commit}) (i32.const 16) (i32.const 64))))))
                (i32.sub (i32.const 0) (i32.load (i32.const 0)))))"#,
        commit = commit_field::COMMIT,
        focus_in = Hook::FocusIn.code(),
    );
    let module = compile_wat(&wat, declaration(&READS, &Hook::ALL)).unwrap();
    let mut script = ScriptInstance::new(
        &module,
        CapabilitySet::EMPTY.with(Capability::ReadCommit),
        limits::INIT_FUEL,
    )
    .unwrap();
    assert_eq!(
        status_of(script.deliver(1, &commit_event("abc"))),
        -status::ABSENT
    );
    // 上一次 commit 的文本不会留到下一个事件里。
    let mut focus = Event::new(Hook::FocusIn);
    focus.scalars.local_mode = Some(0);
    assert_eq!(status_of(script.deliver(2, &focus)), -status::ABSENT);
}

#[test]
fn log_is_a_no_op_without_a_sink() {
    let wat = reader(
        r#"(i32.store8 (i32.const 0) (i32.const 104))
           (i32.store8 (i32.const 1) (i32.const 105))
           (call $log (i32.const 1) (i32.const 0) (i32.const 2))
           i32.const 0"#,
    );
    let module = compile_wat(&wat, declaration(&READS, &Hook::ALL)).unwrap();
    let mut script = ScriptInstance::new(&module, CapabilitySet::EMPTY, limits::INIT_FUEL).unwrap();
    assert_eq!(status_of(script.deliver(1, &input_event())), 0);

    let lines = Arc::new(Mutex::new(Vec::new()));
    let sink_lines = Arc::clone(&lines);
    script.set_log_sink(Some(Box::new(move |level, text| {
        sink_lines.lock().unwrap().push((level, text.to_owned()));
    })));
    assert_eq!(status_of(script.deliver(2, &input_event())), 0);
    assert_eq!(*lines.lock().unwrap(), vec![(1, "hi".to_owned())]);
}

#[test]
fn the_text_mode_whitelist_matches_u6() {
    // none、临时英文、临时日文、超级简拼、emoji、颜文字可用；其余（含不认识的 255）屏蔽。
    for mode in 0..=5 {
        assert!(abi::local_mode_allows_text(mode));
    }
    for mode in [6, 7, 8, 9, 10, 11, 12, 13, 255] {
        assert!(!abi::local_mode_allows_text(mode));
    }
}

// ---- 插件指令 ----

/// 一次 `command` 事件：用户在 `/` 指令里打了 `trigger` 和后面的 `argument`。
fn command_event(
    trigger: &'static str,
    argument: &'static str,
    random: [u8; limits::RANDOM_BYTES_PER_EVENT],
) -> Event<'static> {
    let mut event = Event::new(Hook::Command);
    event.scalars.local_mode = Some(8);
    event.command = Some(CommandEvent {
        trigger,
        argument,
        random,
        local_time: None,
    });
    event
}

const COMMAND_IMPORTS: &str = r#"(import "msime" "command_text" (func $command_text (param i32 i32 i32) (result i32)))
    (import "msime" "command_row" (func $row (param i32 i32 i32 i32) (result i32)))
    (import "msime" "random_fill" (func $random (param i32 i32) (result i32)))"#;

/// 指令插件：申请 `provide_commands`，声明 `command` 和 `hooks` 里的其余钩子，[0..) 是数据段 `data`。
fn commander(data: &str, body: &str, hooks: &[Hook], grants: CapabilitySet) -> ScriptInstance {
    let wat = module_wat(
        COMMAND_IMPORTS,
        &format!(r#"(data (i32.const 0) "{data}")"#),
        body,
    );
    let module = compile_wat(&wat, declaration(&[Capability::ProvideCommands], hooks)).unwrap();
    ScriptInstance::new(&module, grants, limits::INIT_FUEL).unwrap()
}

fn provides() -> CapabilitySet {
    CapabilitySet::EMPTY.with(Capability::ProvideCommands)
}

#[test]
fn the_new_capability_and_hook_have_their_names_and_numbers() {
    assert_eq!(
        Capability::parse("provide_commands"),
        Some(Capability::ProvideCommands)
    );
    assert_eq!(Capability::ProvideCommands.as_str(), "provide_commands");
    assert_eq!(Capability::ProvideCommands.requires(), None);
    assert!(Capability::ProvideCommands.reads_text());
    assert!(provides().check_rules().is_ok());
    // 各占一位；能力的总数由第二批能力的测试核对。
    let all: CapabilitySet = Capability::ALL.into_iter().collect();
    assert_eq!(all.iter().count(), Capability::ALL.len());
    assert_eq!(Hook::parse("command"), Some(Hook::Command));
    assert_eq!(Hook::Command.as_str(), "command");
    assert_eq!(Hook::Command.code(), 18);
    assert_eq!(Hook::ALL.len(), 14);
    assert!(Hook::Command.modifies() && Hook::Command.runs_on_snapshot());
    assert!(Hook::BeforeCommit.runs_on_snapshot() && !Hook::Candidates.runs_on_snapshot());
    assert_eq!(limits::hook_fuel(Hook::Command), limits::COMMAND_FUEL);
    // 两个指令类 import 属于 `provide_commands`，`random_fill` 不需要能力。
    let imports: Vec<_> = abi::imports().collect();
    assert_eq!(
        &imports[15..18],
        [
            ("command_text", Some(Capability::ProvideCommands)),
            ("command_row", Some(Capability::ProvideCommands)),
            ("random_fill", None),
        ]
    );
}

#[test]
fn command_imports_follow_the_static_least_privilege_rules() {
    let row_only = module_wat(
        r#"(import "msime" "command_row" (func $row (param i32 i32 i32 i32) (result i32)))"#,
        "",
        "i32.const 0",
    );
    assert!(matches!(
        compile_wat(&row_only, declaration(&[], &[Hook::Command])),
        Err(ModuleError::ImportWithoutCapability {
            import: "command_row",
            capability: Capability::ProvideCommands,
        })
    ));
    let random_only = module_wat(
        r#"(import "msime" "random_fill" (func $random (param i32 i32) (result i32)))"#,
        "",
        "i32.const 0",
    );
    assert!(compile_wat(&random_only, declaration(&[], &[Hook::Command])).is_ok());
    assert_eq!(
        compile_wat(
            &random_only,
            declaration(&[Capability::ProvideCommands], &[Hook::Command])
        )
        .unwrap_err(),
        ModuleError::UnusedCapability(Capability::ProvideCommands)
    );
}

/// 三个 import 各调一次，返回值编码成 (`command_text` + 10) × 10000 + (`command_row` + 10) × 100 + (`random_fill` + 10)。
const PROBE: &str = "(i32.add
        (i32.add
            (i32.mul (i32.add (call $command_text (i32.const 0) (i32.const 256) (i32.const 64)) (i32.const 10)) (i32.const 10000))
            (i32.mul (i32.add (call $row (i32.const 0) (i32.const 3) (i32.const 0) (i32.const 0)) (i32.const 10)) (i32.const 100)))
        (i32.add (call $random (i32.const 512) (i32.const 1)) (i32.const 10)))";

#[test]
fn command_imports_need_the_grant_and_the_command_hook() {
    let hooks = [Hook::Command, Hook::Input];
    let event = command_event("uuid", "", [7; limits::RANDOM_BYTES_PER_EVENT]);
    // 授予了、在 `command` 里：读到 4 字节的触发词，暂存第 0 行，取到 1 字节随机数。
    let mut granted = commander("abc", PROBE, &hooks, provides());
    assert_eq!(
        status_of(granted.deliver(1, &event)),
        14 * 10000 + 10 * 100 + 11
    );
    assert_eq!(
        granted.actions().rows,
        vec![("abc".to_owned(), String::new())]
    );
    // 没授予：两个指令类 import 都是 DENIED；随机数不需要能力，照样有。
    let mut ungranted = commander("abc", PROBE, &hooks, CapabilitySet::EMPTY);
    assert_eq!(
        status_of(ungranted.deliver(1, &event)),
        9 * 10000 + 9 * 100 + 11
    );
    assert!(ungranted.actions().is_empty());
    // 别的钩子里：没有触发词（ABSENT），不能给行（DENIED），没有随机数（LIMIT）。
    let mut elsewhere = commander("abc", PROBE, &hooks, provides());
    assert_eq!(
        status_of(elsewhere.deliver(1, &input_event())),
        8 * 10000 + 9 * 100 + 6
    );
    assert!(elsewhere.actions().is_empty());
    // 宿主在别的钩子里误带了 `CommandEvent` 也一样：触发词、参数和随机数只交给 `command`。
    let mut smuggled = input_event();
    smuggled.command = command_event("uuid", "", [7; limits::RANDOM_BYTES_PER_EVENT]).command;
    let mut misdelivered = commander("abc", PROBE, &hooks, provides());
    assert_eq!(
        status_of(misdelivered.deliver(1, &smuggled)),
        8 * 10000 + 9 * 100 + 6
    );
    assert!(misdelivered.actions().is_empty());
    // 别的钩子里取 0 字节随机数不算错。
    let mut zero = commander(
        "",
        "(call $random (i32.const 0) (i32.const 0))",
        &hooks,
        provides(),
    );
    assert_eq!(status_of(zero.deliver(1, &input_event())), 0);
}

#[test]
fn command_text_reads_the_trigger_and_the_argument() {
    // 把参数复制到 [256..)，返回长度 × 1000 + 首字节；字段编号不对是 INVALID，放不下是 LIMIT。
    let body = format!(
        "(i32.add (i32.mul (call $command_text (i32.const {}) (i32.const 256) (i32.const 64)) (i32.const 1000))
            (i32.load8_u (i32.const 256)))",
        abi::command_field::ARGUMENT
    );
    let mut script = commander("", &body, &[Hook::Command], provides());
    assert_eq!(
        status_of(script.deliver(1, &command_event("cz", "huo", [0; 64]))),
        3 * 1000 + i32::from(b'h')
    );
    // 参数为空（`/uuid`）：读到 0 字节，不是 ABSENT。
    assert_eq!(
        status_of(script.deliver(2, &command_event("uuid", "", [0; 64]))),
        0
    );
    let mut invalid = commander(
        "",
        "(i32.sub (i32.const 0) (call $command_text (i32.const 2) (i32.const 0) (i32.const 64)))",
        &[Hook::Command],
        provides(),
    );
    assert_eq!(
        status_of(invalid.deliver(1, &command_event("uuid", "", [0; 64]))),
        -status::INVALID
    );
    let mut small = commander(
        "",
        "(i32.sub (i32.const 0) (call $command_text (i32.const 0) (i32.const 0) (i32.const 3)))",
        &[Hook::Command],
        provides(),
    );
    assert_eq!(
        status_of(small.deliver(1, &command_event("uuid", "", [0; 64]))),
        -status::LIMIT
    );
}

#[test]
fn a_command_event_carries_no_other_text() {
    // 同时有 `read_commit`、`read_composition` 的插件在 `command` 里读不到宿主误填的上屏和组字：`command` 的输出只能取决于触发词、参数和随机数。
    let wat = module_wat(
        &format!(
            r#"{COMMAND_IMPORTS}
            (import "msime" "commit_text" (func $commit_text (param i32 i32 i32) (result i32)))
            (import "msime" "composition_text" (func $composition_text (param i32 i32 i32) (result i32)))"#
        ),
        "",
        &format!(
            "(i32.add
                (i32.mul (i32.add (call $commit_text (i32.const {}) (i32.const 0) (i32.const 64)) (i32.const 10)) (i32.const 100))
                (i32.add (call $composition_text (i32.const {}) (i32.const 0) (i32.const 64)) (i32.const 10)))",
            commit_field::COMMIT,
            composition_field::PREEDIT
        ),
    );
    let capabilities = [
        Capability::ReadComposition,
        Capability::ReadCommit,
        Capability::ProvideCommands,
    ];
    let declared = declaration(&capabilities, &[Hook::Command, Hook::Commit]);
    let module = compile_wat(&wat, declared).unwrap();
    let mut script =
        ScriptInstance::new(&module, declared.capabilities, limits::INIT_FUEL).unwrap();
    let mut event = command_event("uuid", "", [0; 64]);
    event.scalars.local_mode = Some(0);
    event.commit.commit = Some("秘密");
    event.composition.preedit = Some("mimi");
    assert_eq!(
        status_of(script.deliver(1, &event)),
        (status::ABSENT + 10) * 100 + status::ABSENT + 10
    );
    // 对照：同一个实例在 `commit` 里两段都读得到。
    let mut commit = commit_event("秘密");
    commit.composition.preedit = Some("mimi");
    assert_eq!(
        status_of(script.deliver(2, &commit)),
        ("秘密".len() as i32 + 10) * 100 + "mimi".len() as i32 + 10
    );
}

#[test]
fn a_command_event_carries_only_the_local_mode_among_the_scalars() {
    // 返回 KIND × 10000 + LOCAL_MODE × 100 + 其余九个标量各自加 10 之和：宿主误填了上屏来源这类标量，插件在 `command` 里也只读到 ABSENT，`command` 的输出只能取决于触发词、参数和随机数。
    let others = [
        event_field::ORIGIN,
        event_field::HOW,
        event_field::OUTCOME,
        event_field::REASON,
        event_field::INPUT_CLASS,
        event_field::RETRACTION,
        event_field::REWRITABLE,
        event_field::CONTINUES,
        event_field::GENERATED,
    ];
    let sum = others
        .iter()
        .fold("(i64.const 0)".to_owned(), |sum, field| {
            format!("(i64.add {sum} (i64.add (call $scalar (i32.const {field})) (i64.const 10)))")
        });
    let body = format!(
        "(i32.wrap_i64 (i64.add
            (i64.add
                (i64.mul (call $scalar (i32.const {})) (i64.const 10000))
                (i64.mul (call $scalar (i32.const {})) (i64.const 100)))
            {sum}))",
        event_field::KIND,
        event_field::LOCAL_MODE
    );
    let wat = module_wat(
        &format!(
            r#"{COMMAND_IMPORTS}
            (import "msime" "event_scalar" (func $scalar (param i32) (result i64)))"#
        ),
        "",
        &body,
    );
    let declared = declaration(
        &[Capability::ProvideCommands],
        &[Hook::Command, Hook::Commit],
    );
    let module = compile_wat(&wat, declared).unwrap();
    let mut script = ScriptInstance::new(&module, provides(), limits::INIT_FUEL).unwrap();
    let filled = Scalars {
        origin: Some(1),
        how: Some(1),
        outcome: Some(1),
        reason: Some(1),
        local_mode: Some(8),
        input_class: Some(1),
        retraction: Some(1),
        rewritable: Some(true),
        continues: Some(true),
        generated: Some(true),
    };
    let mut event = command_event("uuid", "", [0; 64]);
    event.scalars = filled;
    assert_eq!(
        status_of(script.deliver(1, &event)),
        Hook::Command.code() * 10000 + 8 * 100 + 9 * (status::ABSENT + 10)
    );
    // 对照：同一个实例在 `commit` 里九个标量都读得到（各是 1）。
    let mut commit = Event::new(Hook::Commit);
    commit.scalars = filled;
    assert_eq!(
        status_of(script.deliver(2, &commit)),
        Hook::Commit.code() * 10000 + 8 * 100 + 9 * 11
    );
}

#[test]
fn a_command_without_its_words_or_with_too_long_ones_does_not_run() {
    let mut script = commander("", "unreachable", &[Hook::Command], provides());
    assert_eq!(
        script.deliver(1, &Event::new(Hook::Command)),
        Outcome::NotSubscribed
    );
    let long: &'static str = "a".repeat(limits::MAX_COMMAND_ARGUMENT_BYTES + 1).leak();
    assert_eq!(
        script.deliver(1, &command_event("uuid", long, [0; 64])),
        Outcome::NotSubscribed
    );
    let trigger: &'static str = "a".repeat(limits::MAX_COMMAND_TRIGGER_BYTES + 1).leak();
    assert_eq!(
        script.deliver(1, &command_event(trigger, "", [0; 64])),
        Outcome::NotSubscribed
    );
    assert_eq!(script.command_fuel_left(), limits::COMMAND_FUEL);
    // 正好 64 字节的参数照常投递（这个模块一运行就 trap）。
    let longest: &'static str = "a".repeat(limits::MAX_COMMAND_ARGUMENT_BYTES).leak();
    assert_eq!(
        failure(script.deliver(1, &command_event("uuid", longest, [0; 64]))),
        Failure::Unreachable
    );
}

/// 暂存一行：文本在 [0, `text_len`)，注释在 [512, 512 + `note_len`)，两段都是 WAT 数据段的写法。返回 `command_row` 的返回码加 10 和暂存的行。
fn row_status(
    text: &str,
    text_len: usize,
    note: &str,
    note_len: usize,
) -> (i32, Vec<(String, String)>) {
    let wat = module_wat(
        COMMAND_IMPORTS,
        &format!(r#"(data (i32.const 0) "{text}") (data (i32.const 512) "{note}")"#),
        &format!(
            "(i32.add (call $row (i32.const 0) (i32.const {text_len}) (i32.const 512) (i32.const {note_len})) (i32.const 10))"
        ),
    );
    let module = compile_wat(
        &wat,
        declaration(&[Capability::ProvideCommands], &[Hook::Command]),
    )
    .unwrap();
    let mut script = ScriptInstance::new(&module, provides(), limits::INIT_FUEL).unwrap();
    let status = status_of(script.deliver(1, &command_event("uuid", "", [0; 64])));
    (status, script.actions().rows.clone())
}

fn row_text(text: &str) -> i32 {
    row_status(&wat_bytes(text), text.len(), "", 0).0
}

#[test]
fn command_rows_are_checked_like_text_that_will_be_typed() {
    let uuid = "0f8fad5b-d9cb-469f-a165-70867728950e";
    assert_eq!(
        row_status(
            &wat_bytes(uuid),
            uuid.len(),
            &wat_bytes("小写"),
            "小写".len()
        ),
        (10, vec![(uuid.to_owned(), "小写".to_owned())])
    );
    assert_eq!(row_text("焱"), 10);
    // 空串、首尾空白、全是空白、换行、零宽字符、新增的不可见字符、不是 UTF-8：INVALID。
    for text in [
        "",
        " a",
        "a ",
        "\u{3000}",
        "  ",
        "a\nb",
        "a\u{200b}b",
        "a\u{34f}b",
    ] {
        assert_eq!(row_text(text), 7, "{text:?}");
    }
    assert_eq!(row_status("\\ff", 1, "", 0).0, 7);
    // 不让插件行成为链接：`://`、`www.`、`mailto:`，不分大小写。
    for text in [
        "https://example.com",
        "a://b",
        "WWW.example",
        "MailTo:x",
        "见www.x",
    ] {
        assert_eq!(row_text(text), 7, "{text:?}");
    }
    assert_eq!(row_text("www"), 10);
    // 64 个码元正好，65 个放不下；按码元算，33 个 emoji 是 66 个。
    assert_eq!(row_text(&"a".repeat(64)), 10);
    assert_eq!(row_text(&"a".repeat(65)), 6);
    assert_eq!(row_text(&"😀".repeat(32)), 10);
    assert_eq!(row_text(&"😀".repeat(33)), 6);
    // 注释：32 个码元正好，33 个放不下；含禁用字符 INVALID；首尾空白照注释的规则放行。
    let a32 = "a".repeat(32);
    assert_eq!(row_status("x", 1, &a32, 32).0, 10);
    assert_eq!(row_status("x", 1, &"a".repeat(33), 33).0, 6);
    let bidi = wat_bytes("a\u{202e}b");
    assert_eq!(row_status("x", 1, &bidi, 5).0, 7);
    assert_eq!(row_status("x", 1, " a", 2).0, 10);
}

#[test]
fn a_command_gives_at_most_a_page_of_rows_and_numbers_them() {
    // 连续暂存 10 行：前 9 行依次返回行号 0 到 8，第 10 行 LIMIT；返回值是 10 次返回码之和。
    let mut calls = String::from("(local $sum i32)");
    for _ in 0..10 {
        calls.push_str(
            "(local.set $sum (i32.add (local.get $sum) (call $row (i32.const 0) (i32.const 1) (i32.const 0) (i32.const 0))))",
        );
    }
    calls.push_str("(local.get $sum)");
    let mut script = commander("x", &calls, &[Hook::Command], provides());
    assert_eq!(
        status_of(script.deliver(1, &command_event("uuid", "", [0; 64]))),
        (0..9).sum::<i32>() + status::LIMIT
    );
    assert_eq!(script.actions().rows.len(), limits::MAX_COMMAND_ROWS);
}

#[test]
fn a_failed_command_hands_over_no_rows() {
    let event = command_event("uuid", "", [0; 64]);
    for (body, expected) in [
        (
            "(drop (call $row (i32.const 0) (i32.const 1) (i32.const 0) (i32.const 0))) (i32.const -1)",
            Failure::NegativeReturn,
        ),
        (
            "(drop (call $row (i32.const 0) (i32.const 1) (i32.const 0) (i32.const 0))) unreachable",
            Failure::Unreachable,
        ),
        (
            "(drop (call $row (i32.const 0) (i32.const 1) (i32.const 0) (i32.const 0)))
             (drop (call $row (i32.const 0) (i32.const 1) (i32.const -8) (i32.const 100))) (i32.const 0)",
            Failure::MemoryOutOfBounds,
        ),
    ] {
        let mut script = commander("x", body, &[Hook::Command], provides());
        assert_eq!(failure(script.deliver(1, &event)), expected, "{body}");
        assert!(script.actions().rows.is_empty(), "{body}");
        // 跑在快照上：持久实例不受影响。
        assert!(script.is_usable(), "{body}");
    }
}

#[test]
fn random_fill_hands_out_the_events_bytes_in_order() {
    let mut random = [0u8; limits::RANDOM_BYTES_PER_EVENT];
    for (index, byte) in random.iter_mut().enumerate() {
        *byte = index as u8 + 1;
    }
    // 取 16 字节到 [0..)，再取 44 字节到 [16..)，再要 5 字节放不下（只剩 4，什么都不写），然后取 4 字节到 [60..)，最后再要 1 字节放不下。返回值是取到的字节数之和，加上两次 LIMIT 之和加 10 再乘 100，加上 [0] 乘 1000、[63] 乘 100000。
    let body = "(local $sum i32) (local $limits i32)
        (local.set $sum (call $random (i32.const 0) (i32.const 16)))
        (local.set $sum (i32.add (local.get $sum) (call $random (i32.const 16) (i32.const 44))))
        (local.set $limits (call $random (i32.const 60) (i32.const 5)))
        (local.set $sum (i32.add (local.get $sum) (call $random (i32.const 60) (i32.const 4))))
        (local.set $limits (i32.add (local.get $limits) (call $random (i32.const 64) (i32.const 1))))
        (i32.add
            (i32.add (local.get $sum) (i32.mul (i32.add (local.get $limits) (i32.const 10)) (i32.const 100)))
            (i32.add (i32.mul (i32.load8_u (i32.const 0)) (i32.const 1000))
                (i32.mul (i32.load8_u (i32.const 63)) (i32.const 100000))))";
    let mut script = commander("", body, &[Hook::Command], provides());
    let expected = 64 + (2 * status::LIMIT + 10) * 100 + 1000 + 64 * 100_000;
    let event = command_event("uuid", "", random);
    assert_eq!(status_of(script.deliver(1, &event)), expected);
    // 每次事件都是新的 64 字节，从头取。
    assert_eq!(status_of(script.deliver(2, &event)), expected);
}

/// B1：`command` 跑在 `msime_init` 之后的快照上。模块在 `command` 里把计数加一、写进行的开头，再接上之前在 `commit` 里存下的上屏文本；快照上的计数总是从 0 起，持久实例存下的上屏文本也带不进来。
#[test]
fn command_runs_on_the_post_init_snapshot_and_cannot_carry_earlier_text() {
    let wat = format!(
        r#"(module
            {COMMAND_IMPORTS}
            (import "msime" "commit_text" (func $commit_text (param i32 i32 i32) (result i32)))
            (memory (export "memory") 1)
            (global $count (mut i32) (i32.const 0))
            (func (export "msime_abi") (result i32) i32.const 1)
            (func (export "msime_on_event") (param $kind i32) (result i32)
                (if (i32.eq (local.get $kind) (i32.const {commit}))
                    (then
                        (i32.store (i32.const 252)
                            (call $commit_text (i32.const {field}) (i32.const 300) (i32.const 64)))
                        (return (i32.load (i32.const 252)))))
                (global.set $count (i32.add (global.get $count) (i32.const 1)))
                (i32.store8 (i32.const 200) (i32.add (i32.load8_u (i32.const 200)) (i32.const 1)))
                (i32.store8 (i32.const 299)
                    (i32.add (i32.const 48) (i32.add (global.get $count) (i32.load8_u (i32.const 200)))))
                (call $row (i32.const 299) (i32.add (i32.const 1) (i32.load (i32.const 252))) (i32.const 0) (i32.const 0))))"#,
        commit = Hook::Commit.code(),
        field = commit_field::COMMIT,
    );
    let capabilities = [Capability::ReadCommit, Capability::ProvideCommands];
    let declared = declaration(&capabilities, &[Hook::Commit, Hook::Command]);
    let module = compile_wat(&wat, declared).unwrap();
    let mut script =
        ScriptInstance::new(&module, declared.capabilities, limits::INIT_FUEL).unwrap();
    let command = command_event("uuid", "", [0; 64]);
    let rows = |script: &mut ScriptInstance, turn| {
        status_of(script.deliver(turn, &command));
        script.actions().rows.clone()
    };
    let first = vec![("2".to_owned(), String::new())];
    for turn in 1..=3 {
        assert_eq!(rows(&mut script, turn), first, "{turn}");
    }
    // 持久实例在 `commit` 里存下了上屏文本……
    assert_eq!(
        status_of(script.deliver(4, &commit_event("秘密"))),
        "秘密".len() as i32
    );
    // ……之后的 `command` 照样只给出快照上的那一行，没有这段文本。
    assert_eq!(rows(&mut script, 5), first);
}

#[test]
fn command_has_its_own_turn_quota() {
    // `input` 每次循环 1000 次，`command` 死循环。
    let body = format!(
        "(local $i i32)
         (if (i32.eq (local.get $kind) (i32.const {command}))
            (then (loop $spin (br $spin))))
         (local.set $i (i32.const 1000))
         (loop $again
            (local.set $i (i32.sub (local.get $i) (i32.const 1)))
            (br_if $again (local.get $i)))
         i32.const 0",
        command = Hook::Command.code(),
    );
    let mut script = commander("", &body, &[Hook::Input, Hook::Command], provides());
    let per_input = match script.deliver(3, &input_event()) {
        Outcome::Delivered { fuel, .. } => fuel,
        other => panic!("{other:?}"),
    };
    // `input` 把这一轮其余钩子的配额烧到放不下下一次。
    while script.turn_fuel_left() >= per_input {
        assert!(matches!(
            script.deliver(3, &input_event()),
            Outcome::Delivered { .. }
        ));
    }
    let hooks_left = script.turn_fuel_left();
    // 同一轮里 `command` 照样拿到完整的 100 000：死循环烧完自己的上限，按 fuel 耗尽失败，不是本轮配额耗尽；其余钩子的配额不动。
    assert_eq!(script.command_fuel_left(), limits::COMMAND_FUEL);
    let outcome = script.deliver(3, &command_event("uuid", "", [0; 64]));
    assert!(
        matches!(outcome, Outcome::Failed { failure: Failure::OutOfFuel, fuel }
            if fuel > limits::COMMAND_FUEL - 10 && fuel > hooks_left),
        "{outcome:?}"
    );
    assert_eq!(script.turn_fuel_left(), hooks_left);
    assert!(script.command_fuel_left() < 10);
    assert_eq!(script.before_commit_fuel_left(), limits::BEFORE_COMMIT_FUEL);
    // 本轮的 `command` 配额只剩零头：被它截断，不算失败。新一轮补满。
    assert!(matches!(
        script.deliver(3, &command_event("uuid", "", [0; 64])),
        Outcome::TurnExhausted { .. }
    ));
    assert_eq!(script.command_fuel_left(), 0);
    assert!(matches!(
        script.deliver(4, &command_event("uuid", "", [0; 64])),
        Outcome::Failed {
            failure: Failure::OutOfFuel,
            ..
        }
    ));
}

#[test]
fn the_command_row_text_check() {
    for text in [
        "UUID",
        "α",
        "{0F8FAD5B-D9CB-469F-A165-70867728950E}",
        "❤\u{fe0f}",
    ] {
        assert!(abi::valid_command_row_text(text), "{text:?}");
    }
    for text in [
        "",
        " x",
        "x\t",
        "ftp://x",
        "xWwW.y",
        "MAILTO:",
        "a\u{2028}b",
    ] {
        assert!(!abi::valid_command_row_text(text), "{text:?}");
    }
}

// ---- 测量（默认不跑）：`cargo test -p msime-script-runtime -- --ignored --nocapture measure` ----

mod measure {
    use super::*;
    use std::time::Instant;

    /// 手工拼一个 `bytes` 字节左右的二进制模块：`functions` 个相同的函数体。这只是测量用的夹具，不是解析器：没有 `wat` crate 的直接依赖，WAT 文本又会把解析耗时混进编译耗时。
    fn synthetic_module(bytes: usize) -> Vec<u8> {
        fn leb(mut value: u32, out: &mut Vec<u8>) {
            loop {
                let byte = (value & 0x7f) as u8;
                value >>= 7;
                if value == 0 {
                    out.push(byte);
                    return;
                }
                out.push(byte | 0x80);
            }
        }
        fn section(id: u8, body: &[u8], out: &mut Vec<u8>) {
            out.push(id);
            leb(body.len() as u32, out);
            out.extend_from_slice(body);
        }
        fn name(text: &str, out: &mut Vec<u8>) {
            leb(text.len() as u32, out);
            out.extend_from_slice(text.as_bytes());
        }
        // 一个 i32 局部变量；循环体是 `local.get 0; i32.load; local.get 0; i32.add; local.set 0`，依赖内存和局部变量，翻译时折叠不掉。
        let mut body = vec![0x01, 0x01, 0x7f];
        while body.len() < 1_000 {
            body.extend_from_slice(&[0x20, 0x00, 0x28, 0x02, 0x00, 0x20, 0x00, 0x6a, 0x21, 0x00]);
        }
        body.extend_from_slice(&[0x20, 0x00, 0x0b]); // local.get 0; end
        let functions = (bytes / (body.len() + 2)).max(2) as u32;

        let mut out = b"\0asm\x01\0\0\0".to_vec();
        // 类型 0：() -> i32；类型 1：(i32) -> i32。
        section(
            1,
            &[0x02, 0x60, 0x00, 0x01, 0x7f, 0x60, 0x01, 0x7f, 0x01, 0x7f],
            &mut out,
        );
        let mut funcs = Vec::new();
        leb(functions, &mut funcs);
        funcs.push(0x00); // msime_abi
        funcs.push(0x01); // msime_on_event
        funcs.extend(std::iter::repeat_n(0x00, functions as usize - 2));
        section(3, &funcs, &mut out);
        section(5, &[0x01, 0x00, 0x01], &mut out); // 一块内存，最小 1 页
        let mut exports = vec![0x03];
        name("memory", &mut exports);
        exports.extend_from_slice(&[0x02, 0x00]);
        name("msime_abi", &mut exports);
        exports.extend_from_slice(&[0x00, 0x00]);
        name("msime_on_event", &mut exports);
        exports.extend_from_slice(&[0x00, 0x01]);
        section(7, &exports, &mut out);
        let mut code = Vec::new();
        leb(functions, &mut code);
        let abi_body = [0x00, 0x41, 0x01, 0x0b];
        leb(abi_body.len() as u32, &mut code);
        code.extend_from_slice(&abi_body);
        for _ in 1..functions {
            leb(body.len() as u32, &mut code);
            code.extend_from_slice(&body);
        }
        section(10, &code, &mut out);
        out
    }

    #[test]
    #[ignore]
    fn measure_compile_and_instantiate_a_512_kib_module() {
        // 模块上限已经下调到 256 KiB，这个测量照设计量 512 KiB 的普通代码，所以绕过大小检查。
        let wasm = synthetic_module(512 * 1024);
        let declared = declaration(&[], &[Hook::Input]);
        let started = Instant::now();
        let before = counting::net();
        let module = compile_unchecked_format(&wasm, declared).expect("synthetic module compiles");
        let retained = counting::net() - before;
        let compiled = started.elapsed();
        let started = Instant::now();
        let script = ScriptInstance::new(&module, CapabilitySet::EMPTY, limits::INIT_FUEL).unwrap();
        let instantiated = started.elapsed();
        println!(
            "module {} bytes: validate + eager compile {compiled:?}, instantiate {instantiated:?}, retained {} bytes ({:.2}x)",
            wasm.len(),
            retained,
            retained as f64 / wasm.len() as f64
        );
        drop(script);
    }

    #[test]
    #[ignore]
    fn measure_a_minimal_hook() {
        let mut script = instance(
            &module_wat("", "", "i32.const 0"),
            declaration(&[], &[Hook::Input]),
        );
        let event = input_event();
        for turn in 0..1_000 {
            script.deliver(turn, &event);
        }
        let rounds = 1_000_000u64;
        let started = Instant::now();
        for turn in 0..rounds {
            script.deliver(turn, &event);
        }
        println!(
            "minimal hook: {:?} per delivery",
            started.elapsed() / rounds as u32
        );
    }

    #[test]
    #[ignore]
    fn measure_snapshot_reset() {
        for pages in [16u32, 64] {
            let wat = format!(
                r#"(module (memory (export "memory") {pages})
                    (func (export "msime_abi") (result i32) i32.const 1)
                    (func (export "msime_init") (result i32)
                        (memory.fill (i32.const 0) (i32.const 1) (i32.const {bytes}))
                        i32.const 0)
                    (func (export "msime_on_event") (param i32) (result i32) i32.const 0))"#,
                bytes = pages as usize * limits::PAGE_BYTES
            );
            let mut declared = declaration(&[], &[Hook::BeforeCommit]);
            declared.memory_pages = pages;
            let module = compile_wat(&wat, declared).unwrap();
            let mut script =
                ScriptInstance::new(&module, CapabilitySet::EMPTY, limits::INIT_FUEL).unwrap();
            let event = Event::new(Hook::BeforeCommit);
            for turn in 0..20 {
                script.deliver(turn, &event);
            }
            let rounds = 200u64;
            let started = Instant::now();
            for turn in 0..rounds {
                assert!(matches!(
                    script.deliver(turn, &event),
                    Outcome::Delivered { .. }
                ));
            }
            println!(
                "snapshot reset, {pages} pages fully dirty: {:?} per before_commit",
                started.elapsed() / rounds as u32
            );
        }
    }

    #[test]
    #[ignore]
    fn measure_expansion_of_adversarial_modules() {
        // 从 2 KiB 到模块上限按 1.07 倍取样，每种形状记下倍数最大的那一点。
        let mut worst: std::collections::BTreeMap<&str, (f64, usize, usize)> = Default::default();
        for (shape, bytes, retained) in super::expansion::points(1.07) {
            let ratio = retained as f64 / bytes.max(1) as f64;
            let entry = worst.entry(shape).or_insert((0.0, 0, 0));
            if ratio > entry.0 {
                *entry = (ratio, bytes, retained);
            }
        }
        for (shape, (ratio, bytes, retained)) in worst {
            println!(
                "{shape}: worst {ratio:.2}x ({retained} bytes retained by a {bytes} byte module)"
            );
        }
    }
}

/// 翻译后的模块没有办法在产品里直接量（库不能替宿主进程装全局分配器），内存总账只能按模块大小估算。这里用计数分配器量专门放大翻译结果的模块形状，守住估算确实是上界：哪天 wasmi 的翻译变了、或者有人改小了倍数，这里就变红。
///
/// 留下来的有两部分。一是翻译结果本身：函数表按 2 的幂分桶，一个最小的函数（4 字节）留下约 100 字节，元素段的一个条目（1 字节）约 24 字节。二是 Engine 留着复用的翻译缓冲，大小取决于最大的那个函数：`block` 嵌套每层 3 字节却要约 330 字节（缓冲按 2 的幂扩容），`br_table` 的一个目标 1 字节要约 90 字节，3 万个局部变量要约 370 KiB。模块有自己的 Engine，这些随模块一起释放；共用一个 Engine 时，它们会按见过的最大函数一直留着。
mod expansion {
    use super::*;

    #[test]
    fn the_ledger_estimate_bounds_every_adversarial_shape() {
        for (shape, bytes, retained) in points(1.5) {
            assert!(
                retained <= limits::module_memory_bytes(bytes) - limits::MAX_STACK_HEIGHT,
                "{shape}: {retained} bytes retained by a {bytes} byte module ({:.2}x)",
                retained as f64 / bytes.max(1) as f64
            );
        }
    }

    /// 每种形状从 2 KiB 到模块上限按 `step` 倍取样，外加函数表每个桶刚多出一个函数的位置；返回 (形状, 模块字节数, 编译结果留下的字节数)。
    pub(super) fn points(step: f64) -> Vec<(&'static str, usize, usize)> {
        let mut sizes = Vec::new();
        let mut size = 2048.0f64;
        while size < limits::MAX_MODULE_BYTES as f64 {
            sizes.push(size as usize);
            size *= step;
        }
        sizes.push(limits::MAX_MODULE_BYTES);
        let minimal = Shape::default().encode();
        let mut points = vec![("minimal", minimal.len(), retained(&minimal))];
        for bytes in sizes {
            for (shape, wasm) in shapes(bytes) {
                points.push((shape, wasm.len(), retained(&wasm)));
            }
        }
        // 函数表的第一个桶 32 个函数，之后每个桶翻倍；骨架自带 2 个函数。
        for k in 6..=20u32 {
            let wasm = Shape {
                bodies: vec![vec![0x00, 0x0b]; (1usize << k) - 32 + 1 - 2],
                ..Shape::default()
            }
            .encode();
            if wasm.len() > limits::MAX_MODULE_BYTES {
                break;
            }
            points.push((
                "tiny functions at a bucket edge",
                wasm.len(),
                retained(&wasm),
            ));
        }
        points
    }

    fn leb(mut value: u32, out: &mut Vec<u8>) {
        loop {
            let byte = (value & 0x7f) as u8;
            value >>= 7;
            if value == 0 {
                out.push(byte);
                return;
            }
            out.push(byte | 0x80);
        }
    }

    fn section(id: u8, body: &[u8], out: &mut Vec<u8>) {
        out.push(id);
        leb(body.len() as u32, out);
        out.extend_from_slice(body);
    }

    fn name(text: &str, out: &mut Vec<u8>) {
        leb(text.len() as u32, out);
        out.extend_from_slice(text.as_bytes());
    }

    /// 合规模块的骨架：类型 0 `() -> i32`、类型 1 `(i32) -> i32`、类型 2 `() -> ()`，外加 `types` 个重复的类型 2；函数 0 是 `msime_abi`，函数 1 是 `msime_on_event`，之后是 `bodies`（都是类型 2）；`exports` 个额外导出都指向函数 0。`globals` 是全局变量数，`elements` 是一个被动元素段里的条目数，`element_segments` 和 `datas` 是空的被动元素段和数据段的个数。
    #[derive(Default)]
    struct Shape {
        types: u32,
        bodies: Vec<Vec<u8>>,
        exports: u32,
        globals: u32,
        elements: u32,
        element_segments: u32,
        datas: u32,
    }

    impl Shape {
        fn encode(&self) -> Vec<u8> {
            let mut out = b"\0asm\x01\0\0\0".to_vec();
            let mut types = Vec::new();
            leb(3 + self.types, &mut types);
            types.extend_from_slice(&[0x60, 0x00, 0x01, 0x7f, 0x60, 0x01, 0x7f, 0x01, 0x7f]);
            for _ in 0..=self.types {
                types.extend_from_slice(&[0x60, 0x00, 0x00]);
            }
            section(1, &types, &mut out);

            let mut funcs = Vec::new();
            leb(2 + self.bodies.len() as u32, &mut funcs);
            funcs.extend_from_slice(&[0x00, 0x01]);
            funcs.extend(std::iter::repeat_n(0x02, self.bodies.len()));
            section(3, &funcs, &mut out);
            section(5, &[0x01, 0x00, 0x01], &mut out);
            if self.globals > 0 {
                let mut globals = Vec::new();
                leb(self.globals, &mut globals);
                for _ in 0..self.globals {
                    // 不可变的 i32，初值 `i32.const 0`。
                    globals.extend_from_slice(&[0x7f, 0x00, 0x41, 0x00, 0x0b]);
                }
                section(6, &globals, &mut out);
            }

            let mut exports = Vec::new();
            leb(3 + self.exports, &mut exports);
            name("memory", &mut exports);
            exports.extend_from_slice(&[0x02, 0x00]);
            name("msime_abi", &mut exports);
            exports.extend_from_slice(&[0x00, 0x00]);
            name("msime_on_event", &mut exports);
            exports.extend_from_slice(&[0x00, 0x01]);
            for index in 0..self.exports {
                name(&format!("{index:x}"), &mut exports);
                exports.extend_from_slice(&[0x00, 0x00]);
            }
            section(7, &exports, &mut out);

            if self.elements > 0 || self.element_segments > 0 {
                let mut elements = Vec::new();
                leb(
                    u32::from(self.elements > 0) + self.element_segments,
                    &mut elements,
                );
                if self.elements > 0 {
                    // 一个被动段，`elements` 个函数下标，每个 1 字节。
                    elements.extend_from_slice(&[0x01, 0x00]);
                    leb(self.elements, &mut elements);
                    elements.extend(std::iter::repeat_n(0x00, self.elements as usize));
                }
                for _ in 0..self.element_segments {
                    // 空的被动段，3 字节。
                    elements.extend_from_slice(&[0x01, 0x00, 0x00]);
                }
                section(9, &elements, &mut out);
            }

            let mut code = Vec::new();
            leb(2 + self.bodies.len() as u32, &mut code);
            for body in [&[0x00, 0x41, 0x01, 0x0b][..], &[0x00, 0x41, 0x00, 0x0b]] {
                leb(body.len() as u32, &mut code);
                code.extend_from_slice(body);
            }
            for body in &self.bodies {
                leb(body.len() as u32, &mut code);
                code.extend_from_slice(body);
            }
            section(10, &code, &mut out);

            if self.datas > 0 {
                // `datas` 个空的被动段，每个 2 字节。
                let mut datas = Vec::new();
                leb(self.datas, &mut datas);
                for _ in 0..self.datas {
                    datas.extend_from_slice(&[0x01, 0x00]);
                }
                section(11, &datas, &mut out);
            }
            out
        }
    }

    /// 每种形状按 `bytes` 字节左右生成（不超过它）。
    fn shapes(bytes: usize) -> Vec<(&'static str, Vec<u8>)> {
        let fill = |make: &dyn Fn(u32) -> Shape| {
            // 先用 0 个和 4096 个条目量出骨架和每个条目的平均大小，再按比例放大，最后往回收到不超过 `bytes`。
            let base = make(0).encode().len();
            let each = (make(4096).encode().len() - base) as f64 / 4096.0;
            let mut count = ((bytes - base) as f64 / each) as u32;
            loop {
                let wasm = make(count).encode();
                if wasm.len() <= bytes {
                    return wasm;
                }
                count -= count / 64 + 1;
            }
        };
        let br_table = |targets: u32| {
            // 一个 i32 局部变量; block; block; local.get 0; br_table（目标在 0 和 1 之间交替，每个 1 字节）; end; end; end。下标不能是常量，否则翻译时直接折叠成一次跳转。
            let mut body = vec![0x01, 0x01, 0x7f, 0x02, 0x40, 0x02, 0x40, 0x20, 0x00, 0x0e];
            leb(targets, &mut body);
            body.extend((0..=targets).map(|target| (target % 2) as u8));
            body.extend_from_slice(&[0x0b, 0x0b, 0x0b]);
            body
        };
        // wasmparser 对元素段和数据段的个数各有 10 万的上限。
        let segments = |n: u32| n.min(100_000);
        vec![
            (
                "tiny functions",
                fill(&|n| Shape {
                    bodies: vec![vec![0x00, 0x0b]; n as usize],
                    ..Shape::default()
                }),
            ),
            (
                "functions with 30000 locals",
                fill(&|n| Shape {
                    bodies: vec![vec![0x01, 0xb0, 0xea, 0x01, 0x7f, 0x0b]; n as usize],
                    ..Shape::default()
                }),
            ),
            (
                "dense br_table",
                fill(&|n| Shape {
                    // wasmparser 限制一个 br_table 最多 128 Ki 个目标。
                    bodies: (0..n.div_ceil(100_000))
                        .map(|index| br_table((n - index * 100_000).min(100_000)))
                        .collect(),
                    ..Shape::default()
                }),
            ),
            (
                "deep block nesting",
                fill(&|n| {
                    // `block` 套 n 层再逐层 `end`，每层 3 字节。
                    let mut body = vec![0x00];
                    body.extend(std::iter::repeat_n([0x02, 0x40], n as usize).flatten());
                    body.extend(std::iter::repeat_n(0x0b, n as usize));
                    body.push(0x0b);
                    Shape {
                        bodies: vec![body],
                        ..Shape::default()
                    }
                }),
            ),
            (
                "deep operand stack",
                fill(&|n| {
                    // 连压 `local.get 0` 再逐个 `drop`，每层 3 字节。wasmi 每个函数的寄存器有上限，每个函数压 10000 层。
                    let deep = |depth: u32| {
                        let mut body = vec![0x01, 0x01, 0x7f];
                        body.extend(std::iter::repeat_n([0x20, 0x00], depth as usize).flatten());
                        body.extend(std::iter::repeat_n(0x1a, depth as usize));
                        body.push(0x0b);
                        body
                    };
                    Shape {
                        bodies: (0..n.div_ceil(10_000))
                            .map(|index| deep((n - index * 10_000).min(10_000)))
                            .collect(),
                        ..Shape::default()
                    }
                }),
            ),
            (
                "duplicate types",
                fill(&|n| Shape {
                    types: n,
                    ..Shape::default()
                }),
            ),
            (
                "exports",
                fill(&|n| Shape {
                    exports: n,
                    ..Shape::default()
                }),
            ),
            (
                "globals",
                fill(&|n| Shape {
                    globals: n,
                    ..Shape::default()
                }),
            ),
            (
                "element items",
                fill(&|n| Shape {
                    elements: n,
                    ..Shape::default()
                }),
            ),
            (
                "empty element segments",
                fill(&|n| Shape {
                    element_segments: segments(n),
                    ..Shape::default()
                }),
            ),
            (
                "empty data segments",
                fill(&|n| Shape {
                    datas: segments(n),
                    ..Shape::default()
                }),
            ),
        ]
    }

    /// 编译一个模块，返回编译结果留下的字节数。编译不过的形状说明夹具写错了。
    fn retained(wasm: &[u8]) -> usize {
        let declared = declaration(&[], &[Hook::Input]);
        let before = counting::net();
        let module = compile(wasm, declared).expect("adversarial module compiles");
        let retained = counting::net() - before;
        drop(module);
        retained.max(0) as usize
    }
}

/// 只数当前线程的计数分配器，量编译后留下的字节数。别的测试在别的线程上分配，不影响读数。
#[allow(unsafe_code)]
mod counting {
    use std::alloc::{GlobalAlloc, Layout, System};
    use std::cell::Cell;

    thread_local! {
        static NET: Cell<isize> = const { Cell::new(0) };
    }

    struct Counting;

    // SAFETY: 原样转交给系统分配器，只额外记一个线程局部的计数。
    unsafe impl GlobalAlloc for Counting {
        unsafe fn alloc(&self, layout: Layout) -> *mut u8 {
            let _ = NET.try_with(|net| net.set(net.get() + layout.size() as isize));
            // SAFETY: 调用方满足 `GlobalAlloc::alloc` 的约定，原样转交。
            unsafe { System.alloc(layout) }
        }

        unsafe fn dealloc(&self, ptr: *mut u8, layout: Layout) {
            let _ = NET.try_with(|net| net.set(net.get() - layout.size() as isize));
            // SAFETY: `ptr` 来自上面的 `alloc`，即系统分配器。
            unsafe { System.dealloc(ptr, layout) }
        }

        unsafe fn realloc(&self, ptr: *mut u8, layout: Layout, new_size: usize) -> *mut u8 {
            let _ =
                NET.try_with(|net| net.set(net.get() + new_size as isize - layout.size() as isize));
            // SAFETY: 同 `dealloc`。
            unsafe { System.realloc(ptr, layout, new_size) }
        }
    }

    #[global_allocator]
    static ALLOCATOR: Counting = Counting;

    /// 当前线程到目前为止净分配的字节数。
    pub fn net() -> isize {
        NET.with(Cell::get)
    }
}

/// 解释器的原生栈不随执行的指令数增长：每个钩子都在 256 KiB 的线程栈上跑满自己的配额。dev 构建里 wasmi 打开 debug-assertions 时尾调用分派会失效，栈随指令线性增长，见根 `Cargo.toml` 的 `[profile.dev.package.wasmi]`。
#[test]
fn a_hook_running_out_of_fuel_fits_a_small_native_stack() {
    for hook in [Hook::Input, Hook::Candidates, Hook::BeforeCommit] {
        let outcome = std::thread::Builder::new()
            .stack_size(256 * 1024)
            .spawn(move || {
                let mut script = instance(
                    &module_wat("", "", "(loop $spin (br $spin)) i32.const 0"),
                    declaration(&[], &[hook]),
                );
                let mut event = input_event();
                event.hook = hook;
                script.deliver(1, &event)
            })
            .unwrap()
            .join()
            .unwrap();
        assert_eq!(
            outcome,
            Outcome::Failed {
                failure: Failure::OutOfFuel,
                fuel: limits::hook_fuel(hook),
            },
            "{hook:?}"
        );
    }
}

// ---- 第二批能力：设置项、随包数据、加候选、指令时间、持久存储 ----

const V2_IMPORTS: &str = r#"(import "msime" "event_scalar" (func $scalar (param i32) (result i64)))
    (import "msime" "candidate_count" (func $count (result i32)))
    (import "msime" "commit_text" (func $commit_text (param i32 i32 i32) (result i32)))
    (import "msime" "setting_count" (func $setting_count (result i32)))
    (import "msime" "setting_value" (func $setting_value (param i32 i32 i32) (result i32)))
    (import "msime" "data_len" (func $data_len (param i32) (result i64)))
    (import "msime" "data_read" (func $data_read (param i32 i32 i32 i32) (result i32)))
    (import "msime" "candidates_insert" (func $insert (param i32 i32 i32 i32 i32) (result i32)))
    (import "msime" "storage_len" (func $storage_len (result i32)))
    (import "msime" "storage_read" (func $storage_read (param i32 i32 i32) (result i32)))
    (import "msime" "storage_write" (func $storage_write (param i32 i32) (result i32)))"#;

const V2_CAPS: [Capability; 4] = [
    Capability::ReadCommit,
    Capability::ReadCandidates,
    Capability::InsertCandidates,
    Capability::PersistData,
];

/// 第二批 import 的探测模块：申请 [`V2_CAPS`]，声明全部钩子。`extra` 是额外的定义（数据段、`msime_init`），`body` 是 `msime_on_event` 的函数体。
fn v2(extra: &str, body: &str, grants: &[Capability], resources: Resources) -> ScriptInstance {
    let wat = module_wat(V2_IMPORTS, extra, body);
    let module = compile_wat(&wat, declaration(&V2_CAPS, &Hook::ALL)).expect("v2 module compiles");
    ScriptInstance::with_resources(
        &module,
        grants.iter().copied().collect(),
        limits::INIT_FUEL,
        resources,
        DebugSinks::default(),
    )
    .expect("v2 module instantiates")
}

fn resources(settings: &[&str], data: &[&[u8]], storage: Option<&Storage>) -> Resources {
    Resources {
        settings: settings
            .iter()
            .map(|value| (*value).to_owned())
            .collect::<Vec<_>>()
            .into(),
        data: data
            .iter()
            .map(|bytes| Arc::<[u8]>::from(*bytes))
            .collect::<Vec<_>>()
            .into(),
        storage: storage.cloned(),
    }
}

fn fuel_of(outcome: Outcome) -> u64 {
    match outcome {
        Outcome::Delivered { fuel, .. } => fuel,
        other => panic!("expected a delivery, got {other:?}"),
    }
}

/// 一次 `commit`，本地模式是 `local_mode`。
fn commit_in_mode(local_mode: u8) -> Event<'static> {
    let mut event = commit_event("秘密");
    event.scalars.local_mode = Some(local_mode);
    event
}

#[test]
fn the_second_batch_capabilities_have_their_names_bits_and_rules() {
    assert_eq!(Capability::ALL.len(), 10);
    for (capability, name, requires) in [
        (
            Capability::InsertCandidates,
            "insert_candidates",
            Some(Capability::ReadCandidates),
        ),
        (Capability::PersistData, "persist_data", None),
    ] {
        assert_eq!(Capability::parse(name), Some(capability));
        assert_eq!(capability.as_str(), name);
        assert_eq!(capability.requires(), requires);
        assert!(!capability.reads_text());
    }
    // 第 9、10 项各占自己的一位，不和前 8 项撞车。
    for capability in Capability::ALL {
        let single = CapabilitySet::EMPTY.with(capability);
        assert_eq!(single.iter().collect::<Vec<_>>(), vec![capability]);
    }
    let set = |capabilities: &[Capability]| capabilities.iter().copied().collect::<CapabilitySet>();
    assert_eq!(
        set(&[Capability::InsertCandidates]).check_rules(),
        Err(CapabilityRuleError::MissingDependency {
            capability: Capability::InsertCandidates,
            requires: Capability::ReadCandidates,
        })
    );
    assert_eq!(
        set(&[
            Capability::ReadCommit,
            Capability::RewriteCommit,
            Capability::PersistData
        ])
        .check_rules(),
        Err(CapabilityRuleError::Exclusive {
            first: Capability::PersistData,
            second: Capability::RewriteCommit,
        })
    );
    // 存储可以和读文本、加候选、加注释同时申请：存下的内容可能出现在候选和注释里，这是授权页要写明的风险。
    assert!(set(&[
        Capability::ReadComposition,
        Capability::ReadCommit,
        Capability::ReadCandidates,
        Capability::InsertCandidates,
        Capability::AnnotateCandidates,
        Capability::PersistData,
    ])
    .check_rules()
    .is_ok());
    let imports: Vec<_> = abi::imports().collect();
    assert_eq!(
        &imports[18..],
        [
            ("setting_count", None),
            ("setting_value", None),
            ("data_len", None),
            ("data_read", None),
            ("candidates_insert", Some(Capability::InsertCandidates)),
            ("storage_len", Some(Capability::PersistData)),
            ("storage_read", Some(Capability::PersistData)),
            ("storage_write", Some(Capability::PersistData)),
        ]
    );
    assert_eq!(
        (
            event_field::LOCAL_DATE,
            event_field::LOCAL_MINUTE,
            event_field::WEEKDAY,
            event_field::UTC_OFFSET_MINUTES
        ),
        (11, 12, 13, 14)
    );
}

#[test]
fn the_new_imports_follow_the_static_least_privilege_rules() {
    let insert_only = module_wat(
        r#"(import "msime" "candidates_insert" (func (param i32 i32 i32 i32 i32) (result i32)))"#,
        "",
        "i32.const 0",
    );
    assert_eq!(
        compile_wat(&insert_only, declaration(&[], &[Hook::Candidates])).unwrap_err(),
        ModuleError::ImportWithoutCapability {
            import: "candidates_insert",
            capability: Capability::InsertCandidates,
        }
    );
    let storage_only = module_wat(
        r#"(import "msime" "storage_write" (func (param i32 i32) (result i32)))"#,
        "",
        "i32.const 0",
    );
    assert_eq!(
        compile_wat(&storage_only, declaration(&[], &[Hook::Input])).unwrap_err(),
        ModuleError::ImportWithoutCapability {
            import: "storage_write",
            capability: Capability::PersistData,
        }
    );
    assert!(compile_wat(
        &storage_only,
        declaration(&[Capability::PersistData], &[Hook::Input])
    )
    .is_ok());
    // 设置项和数据不需要能力；只导入它们却申请了 `persist_data` 是多申请。
    let resources_only = module_wat(
        r#"(import "msime" "setting_value" (func (param i32 i32 i32) (result i32)))
           (import "msime" "data_read" (func (param i32 i32 i32 i32) (result i32)))"#,
        "",
        "i32.const 0",
    );
    assert!(compile_wat(&resources_only, declaration(&[], &[Hook::Input])).is_ok());
    assert_eq!(
        compile_wat(
            &resources_only,
            declaration(&[Capability::PersistData], &[Hook::Input])
        )
        .unwrap_err(),
        ModuleError::UnusedCapability(Capability::PersistData)
    );
    // `persist_data` 和 `rewrite_commit` 同时申请：看模块之前就拒绝。
    assert_eq!(
        compile_wat(
            &storage_only,
            declaration(
                &[
                    Capability::ReadCommit,
                    Capability::RewriteCommit,
                    Capability::PersistData
                ],
                &[Hook::BeforeCommit]
            )
        )
        .unwrap_err(),
        ModuleError::Capabilities(CapabilityRuleError::Exclusive {
            first: Capability::PersistData,
            second: Capability::RewriteCommit,
        })
    );
}

#[test]
fn settings_are_read_by_index_in_every_hook_init_included() {
    let settings = resources(&["1", "42", "中文"], &[], None);
    let run = |body: &str, event: &Event<'_>| {
        status_of(v2("", body, &[], settings.clone()).deliver(1, event))
    };
    assert_eq!(run("(call $setting_count)", &input_event()), 3);
    // 第 1 项：2 字节，首字节 '4'。
    let value = "(i32.add (i32.mul (call $setting_value (i32.const 1) (i32.const 256) (i32.const 64)) (i32.const 1000))
        (i32.load8_u (i32.const 256)))";
    let expected = 2 * 1000 + i32::from(b'4');
    // 不需要能力、不随钩子和本地模式变化：快照钩子、`command`、屏蔽文本的模式里照样读得到。
    for event in [
        input_event(),
        before_commit_event("abc", "abc"),
        command_event("uuid", "", [0; 64]),
        candidates_event(Hook::Candidates, 7),
        commit_in_mode(7),
    ] {
        assert_eq!(run(value, &event), expected, "{:?}", event.hook);
    }
    // 越界 INVALID（坏指针也不 trap），放不下 LIMIT，目标越界 trap。
    for index in [3, -1] {
        assert_eq!(
            run(
                &format!(
                    "(i32.sub (i32.const 0) (call $setting_value (i32.const {index}) (i32.const -8) (i32.const 64)))"
                ),
                &input_event()
            ),
            -status::INVALID
        );
    }
    assert_eq!(
        run(
            "(i32.sub (i32.const 0) (call $setting_value (i32.const 2) (i32.const 256) (i32.const 5)))",
            &input_event()
        ),
        -status::LIMIT
    );
    assert_eq!(
        failure(
            v2(
                "",
                "(call $setting_value (i32.const 0) (i32.const -8) (i32.const 64))",
                &[],
                settings.clone()
            )
            .deliver(1, &input_event())
        ),
        Failure::MemoryOutOfBounds
    );
    // `msime_init` 里读到的值进快照，`command` 和持久实例都看得到。
    let init = r#"(func (export "msime_init") (result i32)
        (drop (call $setting_value (i32.const 2) (i32.const 512) (i32.const 64)))
        (i32.const 0))"#;
    let mut script = v2(init, "(i32.load8_u (i32.const 512))", &[], settings.clone());
    assert_eq!(status_of(script.deliver(1, &input_event())), 0xE4);
    assert_eq!(
        status_of(script.deliver(2, &command_event("uuid", "", [0; 64]))),
        0xE4
    );
    // 没有设置项：0 项，读第 0 项是 INVALID。
    assert_eq!(
        status_of(
            v2("", "(call $setting_count)", &[], Resources::default()).deliver(1, &input_event())
        ),
        0
    );
}

#[test]
fn data_files_are_read_by_window_in_every_hook_init_included() {
    let data = resources(&[], &[b"hello world", b""], None);
    let run = |body: &str, event: &Event<'_>| {
        status_of(v2("", body, &[], data.clone()).deliver(1, event))
    };
    let len = |file: i32| {
        run(
            &format!(
                "(i32.wrap_i64 (i64.add (call $data_len (i32.const {file})) (i64.const 100)))"
            ),
            &input_event(),
        ) - 100
    };
    assert_eq!(
        (len(0), len(1), len(2), len(-1)),
        (11, 0, status::INVALID, status::INVALID)
    );
    let read = |file: i32, offset: i32, dst: i32, cap: i32| {
        run(
            &format!(
                "(i32.add (i32.mul (i32.add (call $data_read (i32.const {file}) (i32.const {offset}) (i32.const {dst}) (i32.const {cap})) (i32.const 10)) (i32.const 1000))
                    (i32.load8_u (i32.const 256)))"
            ),
            &input_event(),
        )
    };
    // 复制 min(cap, len - offset)：从 6 起是 "world"，cap 3 只复制 3 个，正好到末尾是 0 个。
    assert_eq!(read(0, 6, 256, 64), 15 * 1000 + i32::from(b'w'));
    assert_eq!(read(0, 0, 256, 3), 13 * 1000 + i32::from(b'h'));
    assert_eq!(read(0, 11, 256, 64), 10 * 1000);
    assert_eq!(read(1, 0, 256, 64), 10 * 1000);
    // 偏移越界、为负，文件越界：INVALID，坏指针也不 trap。
    for (file, offset) in [(0, 12), (0, -1), (2, 0), (-1, 0)] {
        assert_eq!(read(file, offset, -8, 64), (status::INVALID + 10) * 1000);
    }
    assert_eq!(
        failure(
            v2(
                "",
                "(call $data_read (i32.const 0) (i32.const 0) (i32.const -8) (i32.const 64))",
                &[],
                data.clone()
            )
            .deliver(1, &input_event())
        ),
        Failure::MemoryOutOfBounds
    );
    // 不随钩子和本地模式变化，`msime_init` 里也读得到。
    let first = "(drop (call $data_read (i32.const 0) (i32.const 4) (i32.const 256) (i32.const 1))) (i32.load8_u (i32.const 256))";
    for event in [
        before_commit_event("abc", "abc"),
        command_event("uuid", "", [0; 64]),
        commit_in_mode(7),
    ] {
        assert_eq!(run(first, &event), i32::from(b'o'), "{:?}", event.hook);
    }
    let init = r#"(func (export "msime_init") (result i32)
        (drop (call $data_read (i32.const 0) (i32.const 0) (i32.const 512) (i32.const 64)))
        (i32.const 0))"#;
    let mut script = v2(init, "(i32.load8_u (i32.const 516))", &[], data.clone());
    assert_eq!(
        status_of(script.deliver(1, &command_event("uuid", "", [0; 64]))),
        i32::from(b'o')
    );
}

#[test]
fn resource_reads_and_storage_cost_fuel_per_byte() {
    let storage = Storage::new(vec![1; 4000]);
    let shared = resources(&[&"x".repeat(400)], &[&[7; 4000]], Some(&storage));
    let fuel = |body: String| {
        fuel_of(
            v2(
                &format!(r#"(data (i32.const 0) "{}")"#, "a".repeat(64)),
                &format!("(drop {body}) (i32.const 0)"),
                &V2_CAPS,
                shared.clone(),
            )
            .deliver(1, &candidates_event(Hook::Candidates, 0)),
        )
    };
    // 每 4 字节 1 fuel，其余指令完全一样。
    let data = |cap: i32| {
        fuel(format!(
            "(call $data_read (i32.const 0) (i32.const 0) (i32.const 256) (i32.const {cap}))"
        ))
    };
    assert_eq!(data(4000) - data(0), 1000);
    let storage_read = |cap: i32| {
        fuel(format!(
            "(call $storage_read (i32.const 0) (i32.const 256) (i32.const {cap}))"
        ))
    };
    assert_eq!(storage_read(4000) - storage_read(0), 1000);
    let storage_write = |len: i32| {
        fuel(format!(
            "(call $storage_write (i32.const 256) (i32.const {len}))"
        ))
    };
    assert_eq!(storage_write(4000) - storage_write(0), 1000);
    // 设置项放不下时只扣调用的 100，复制了 400 字节再扣 100。
    let setting = |cap: i32| {
        fuel(format!(
            "(call $setting_value (i32.const 0) (i32.const 256) (i32.const {cap}))"
        ))
    };
    assert_eq!(setting(400) - setting(0), 100);
    let insert = |len: i32| {
        fuel(format!("(call $insert (i32.const 0) (i32.const 0) (i32.const {len}) (i32.const 0) (i32.const 0))"))
    };
    assert_eq!(insert(64) - insert(4), 15);
    // 每次调用至少 100。
    let one = fuel("(call $data_len (i32.const 0))".to_owned());
    let two =
        fuel("(i64.add (call $data_len (i32.const 0)) (call $data_len (i32.const 0)))".to_owned());
    assert!(two - one >= limits::IMPORT_CALL_FUEL);
}

fn time_probe(field: i32, event: &Event<'_>) -> i64 {
    let mut script = v2(
        "",
        &format!("(i32.wrap_i64 (i64.add (call $scalar (i32.const {field})) (i64.const 100000)))"),
        &[],
        Resources::default(),
    );
    i64::from(status_of(script.deliver(1, event))) - 100_000
}

fn timed_command(time: Option<LocalTime>) -> Event<'static> {
    let mut event = command_event("uuid", "", [0; 64]);
    if let Some(command) = event.command.as_mut() {
        command.local_time = time;
    }
    event
}

const TIME_FIELDS: [i32; 4] = [
    event_field::LOCAL_DATE,
    event_field::LOCAL_MINUTE,
    event_field::WEEKDAY,
    event_field::UTC_OFFSET_MINUTES,
];

#[test]
fn local_time_is_only_in_command_and_only_when_it_is_valid() {
    let time = LocalTime {
        date: 20261026,
        minute: 870,
        weekday: 1,
        utc_offset_minutes: 540,
    };
    let read = |event: &Event<'_>| TIME_FIELDS.map(|field| time_probe(field, event));
    assert_eq!(read(&timed_command(Some(time))), [20261026, 870, 1, 540]);
    let absent = [i64::from(status::ABSENT); 4];
    assert_eq!(read(&timed_command(None)), absent);
    // 西半球的负偏移照常交出。
    let west = LocalTime {
        utc_offset_minutes: -300,
        ..time
    };
    assert_eq!(read(&timed_command(Some(west)))[3], -300);
    // 不合规的时间四个字段一起 ABSENT，包括和返回码撞车的 -1 到 -4。
    for bad in [
        LocalTime {
            minute: 1440,
            ..time
        },
        LocalTime { weekday: 0, ..time },
        LocalTime { weekday: 8, ..time },
        LocalTime {
            date: 20261301,
            ..time
        },
        LocalTime {
            date: 20261000,
            ..time
        },
        LocalTime { date: 1026, ..time },
        LocalTime {
            utc_offset_minutes: -3,
            ..time
        },
        LocalTime {
            utc_offset_minutes: 1081,
            ..time
        },
    ] {
        assert_eq!(read(&timed_command(Some(bad))), absent, "{bad:?}");
    }
    // 别的钩子里没有时间，宿主误带了 `CommandEvent` 也一样。
    let mut smuggled = input_event();
    smuggled.command = timed_command(Some(time)).command;
    assert_eq!(read(&smuggled), absent);
    assert_eq!(read(&candidates_event(Hook::Candidates, 0)), absent);
}

/// 插一行：文本在 [0, `text_len`)，注释在 [512, 512 + `note_len`)。返回 `candidates_insert` 的返回码加 10 和暂存的行。
fn insert_status(
    text: &str,
    text_len: usize,
    note: &str,
    note_len: usize,
    position: i32,
) -> (i32, Vec<(u32, String, String)>) {
    let mut script = v2(
        &format!(r#"(data (i32.const 0) "{text}") (data (i32.const 512) "{note}")"#),
        &format!(
            "(i32.add (call $insert (i32.const {position}) (i32.const 0) (i32.const {text_len}) (i32.const 512) (i32.const {note_len})) (i32.const 10))"
        ),
        &V2_CAPS,
        Resources::default(),
    );
    let status = status_of(script.deliver(1, &candidates_event(Hook::Candidates, 0)));
    (status, script.actions().inserts.clone())
}

fn insert_text(text: &str) -> i32 {
    insert_status(&wat_bytes(text), text.len(), "", 0, 0).0
}

#[test]
fn candidates_insert_stages_rows_checked_like_command_rows() {
    let emoji = "🙂";
    assert_eq!(
        insert_status(
            &wat_bytes(emoji),
            emoji.len(),
            &wat_bytes("表情"),
            "表情".len(),
            1
        ),
        (10, vec![(1, emoji.to_owned(), "表情".to_owned())])
    );
    // 位置 0 到窗口行数（3）都行，4 和负数 INVALID。
    for position in [0, 3] {
        assert_eq!(insert_status("x", 1, "", 0, position).0, 10);
    }
    for position in [4, -1] {
        assert_eq!(insert_status("x", 1, "", 0, position), (7, Vec::new()));
    }
    // 文本规则同 `command_row`。
    for text in [
        "",
        " a",
        "a ",
        "a\nb",
        "a\u{200b}b",
        "a\u{34f}b",
        "https://x",
        "WWW.x",
        "mailto:y",
    ] {
        assert_eq!(insert_text(text), 7, "{text:?}");
    }
    assert_eq!(insert_status("\\ff", 1, "", 0, 0).0, 7);
    assert_eq!(insert_text(&"a".repeat(64)), 10);
    assert_eq!(insert_text(&"a".repeat(65)), 6);
    assert_eq!(insert_text(&"😀".repeat(33)), 6);
    assert_eq!(insert_status("x", 1, &"a".repeat(32), 32, 0).0, 10);
    assert_eq!(insert_status("x", 1, &"a".repeat(33), 33, 0).0, 6);
    assert_eq!(insert_status("x", 1, &wat_bytes("a\u{202e}b"), 5, 0).0, 7);
    // 每次最多 3 行：依次返回 0、1、2，第 4 行 LIMIT。
    let mut calls = String::from("(local $sum i32)");
    for _ in 0..4 {
        calls.push_str(
            "(local.set $sum (i32.add (local.get $sum) (call $insert (i32.const 0) (i32.const 0) (i32.const 1) (i32.const 0) (i32.const 0))))",
        );
    }
    calls.push_str("(i32.add (local.get $sum) (i32.const 10))");
    let mut script = v2(
        r#"(data (i32.const 0) "x")"#,
        &calls,
        &V2_CAPS,
        Resources::default(),
    );
    assert_eq!(
        status_of(script.deliver(1, &candidates_event(Hook::Candidates, 0))),
        3 + status::LIMIT + 10
    );
    assert_eq!(
        script.actions().inserts.len(),
        limits::MAX_INSERTED_ROWS_PER_PLUGIN
    );
}

#[test]
fn candidates_insert_needs_the_grant_the_candidates_hook_and_mode_none() {
    // 坏指针：门控没过时不看区间，过了就 trap。
    let body = "(i32.add (call $insert (i32.const 0) (i32.const -8) (i32.const 100) (i32.const 0) (i32.const 0)) (i32.const 10))";
    let run = |grants: &[Capability], event: &Event<'_>| {
        v2("", body, grants, Resources::default()).deliver(1, event)
    };
    let all = V2_CAPS;
    let without = [
        Capability::ReadCommit,
        Capability::ReadCandidates,
        Capability::PersistData,
    ];
    let denied = 10 + status::DENIED;
    assert_eq!(
        status_of(run(&without, &candidates_event(Hook::Candidates, 0))),
        denied
    );
    assert_eq!(
        status_of(run(&all, &candidates_event(Hook::CandidatesShown, 0))),
        denied
    );
    assert_eq!(status_of(run(&all, &input_event())), denied);
    // 只在本地模式 `none`：白名单里的临时英文、emoji 也不行，屏蔽文本的模式更不行。
    for mode in [1, 4, 7, 8] {
        assert_eq!(
            status_of(run(&all, &candidates_event(Hook::Candidates, mode))),
            denied,
            "{mode}"
        );
    }
    let mut no_mode = candidates_event(Hook::Candidates, 0);
    no_mode.scalars.local_mode = None;
    assert_eq!(status_of(run(&all, &no_mode)), denied);
    // 宿主没交候选：ABSENT。
    let mut empty = Event::new(Hook::Candidates);
    empty.scalars.local_mode = Some(0);
    assert_eq!(status_of(run(&all, &empty)), 10 + status::ABSENT);
    assert_eq!(
        failure(run(&all, &candidates_event(Hook::Candidates, 0))),
        Failure::MemoryOutOfBounds
    );
}

#[test]
fn a_failed_candidates_hook_hands_over_no_inserted_rows() {
    let insert = "(drop (call $insert (i32.const 0) (i32.const 0) (i32.const 1) (i32.const 0) (i32.const 0)))";
    for (tail, expected) in [
        ("(i32.const -1)", Failure::NegativeReturn),
        ("unreachable", Failure::Unreachable),
        ("(loop $spin (br $spin)) (i32.const 0)", Failure::OutOfFuel),
    ] {
        let mut script = v2(
            r#"(data (i32.const 0) "x")"#,
            &format!("{insert} {tail}"),
            &V2_CAPS,
            Resources::default(),
        );
        assert_eq!(
            failure(script.deliver(1, &candidates_event(Hook::Candidates, 0))),
            expected
        );
        assert!(script.actions().inserts.is_empty());
    }
}

#[test]
fn storage_is_read_and_replaced_in_persistent_hooks() {
    let storage = Storage::new(b"abc".to_vec());
    let shared = resources(&[], &[], Some(&storage));
    let run = |body: &str, event: &Event<'_>| {
        status_of(
            v2(
                r#"(data (i32.const 0) "hello")"#,
                body,
                &V2_CAPS,
                shared.clone(),
            )
            .deliver(1, event),
        )
    };
    assert_eq!(run("(call $storage_len)", &input_event()), 3);
    assert_eq!(
        run(
            "(i32.add (i32.mul (call $storage_read (i32.const 1) (i32.const 256) (i32.const 64)) (i32.const 1000)) (i32.load8_u (i32.const 256)))",
            &commit_event("x")
        ),
        2 * 1000 + i32::from(b'b')
    );
    // 偏移越界 INVALID，坏指针不 trap。
    assert_eq!(
        run("(i32.sub (i32.const 0) (call $storage_read (i32.const 4) (i32.const -8) (i32.const 64)))", &input_event()),
        -status::INVALID
    );
    assert!(!storage.is_dirty());
    // 写进暂存：同一个钩子里读到的是新的一份，正常返回之后才写进缓冲。
    let write = "(drop (call $storage_write (i32.const 0) (i32.const 5)))
        (i32.add (i32.mul (call $storage_len) (i32.const 1000))
            (i32.add (call $storage_read (i32.const 4) (i32.const 256) (i32.const 64)) (i32.load8_u (i32.const 256))))";
    assert_eq!(
        run(write, &candidates_event(Hook::Candidates, 0)),
        5 * 1000 + 1 + i32::from(b'o')
    );
    assert_eq!(storage.contents(), b"hello");
    assert!(storage.is_dirty());
    assert_eq!(storage.take_dirty(), Some(b"hello".to_vec()));
    assert_eq!(storage.take_dirty(), None);
    // 写入同样的内容不算改动，不必写盘。
    run(write, &input_event());
    assert!(!storage.is_dirty());
    // 空写清空存储。
    run(
        "(call $storage_write (i32.const 0) (i32.const 0))",
        &input_event(),
    );
    assert!(storage.is_empty() && storage.is_dirty());
}

#[test]
fn one_plugins_instances_share_its_storage() {
    let storage = Storage::default();
    let shared = resources(&[], &[], Some(&storage));
    let mut writer = v2(
        r#"(data (i32.const 0) "abcd")"#,
        "(call $storage_write (i32.const 0) (i32.const 4))",
        &V2_CAPS,
        shared.clone(),
    );
    let mut reader = v2("", "(call $storage_len)", &V2_CAPS, shared);
    assert_eq!(status_of(reader.deliver(1, &input_event())), 0);
    assert_eq!(status_of(writer.deliver(1, &input_event())), 0);
    assert_eq!(status_of(reader.deliver(2, &input_event())), 4);
}

#[test]
fn storage_is_denied_in_snapshot_hooks_init_and_without_the_grant() {
    let storage = Storage::new(b"abc".to_vec());
    let shared = resources(&[], &[], Some(&storage));
    // 坏指针：门控没过时不看区间。
    let probe = "(i32.add
        (i32.mul (i32.add (call $storage_len) (i32.const 10)) (i32.const 10000))
        (i32.add
            (i32.mul (i32.add (call $storage_read (i32.const 0) (i32.const -8) (i32.const 64)) (i32.const 10)) (i32.const 100))
            (i32.add (call $storage_write (i32.const -8) (i32.const 64)) (i32.const 10))))";
    let denied = (status::DENIED + 10) * 10101;
    for event in [
        before_commit_event("abc", "abc"),
        command_event("uuid", "", [0; 64]),
    ] {
        let mut script = v2("", probe, &V2_CAPS, shared.clone());
        assert_eq!(
            status_of(script.deliver(1, &event)),
            denied,
            "{:?}",
            event.hook
        );
    }
    // 没授予 `persist_data`。
    let mut ungranted = v2("", probe, &[Capability::ReadCandidates], shared.clone());
    assert_eq!(status_of(ungranted.deliver(1, &input_event())), denied);
    // 授予了、宿主没交缓冲：ABSENT。
    let mut unsupplied = v2("", probe, &V2_CAPS, Resources::default());
    assert_eq!(
        status_of(unsupplied.deliver(1, &input_event())),
        (status::ABSENT + 10) * 10101
    );
    // `msime_init` 里也不行：初始化之后取的快照不能含有存储的内容。
    let init = r#"(func (export "msime_init") (result i32)
        (i32.store (i32.const 600) (call $storage_len))
        (i32.store (i32.const 604) (call $storage_read (i32.const 0) (i32.const 700) (i32.const 64)))
        (i32.const 0))"#;
    let mut script = v2(
        init,
        "(i32.sub (i32.const 0) (i32.add (i32.load (i32.const 600)) (i32.load (i32.const 604))))",
        &V2_CAPS,
        shared.clone(),
    );
    assert_eq!(
        status_of(script.deliver(1, &input_event())),
        -2 * status::DENIED
    );
    assert_eq!(storage.contents(), b"abc");
    assert!(!storage.is_dirty());
}

#[test]
fn what_the_persistent_instance_read_from_storage_does_not_reach_snapshot_hooks() {
    let storage = Storage::new(b"secret".to_vec());
    let body = format!(
        "(if (i32.eq (local.get $kind) (i32.const {input}))
            (then (drop (call $storage_read (i32.const 0) (i32.const 700) (i32.const 64)))))
         (i32.load8_u (i32.const 700))",
        input = Hook::Input.code()
    );
    let mut script = v2("", &body, &V2_CAPS, resources(&[], &[], Some(&storage)));
    assert_eq!(
        status_of(script.deliver(1, &input_event())),
        i32::from(b's')
    );
    assert_eq!(
        status_of(script.deliver(2, &command_event("uuid", "", [0; 64]))),
        0
    );
    assert_eq!(
        status_of(script.deliver(3, &before_commit_event("abc", "abc"))),
        0
    );
}

#[test]
fn storage_writes_are_capped_at_64_kib() {
    let storage = Storage::default();
    let shared = resources(&[], &[], Some(&storage));
    let write = |len: usize| {
        v2(
            "",
            &format!(
                "(i32.add (call $storage_write (i32.const 0) (i32.const {len})) (i32.const 10))"
            ),
            &V2_CAPS,
            shared.clone(),
        )
        .deliver(1, &input_event())
    };
    assert_eq!(status_of(write(limits::MAX_STORAGE_BYTES)), 10);
    assert_eq!(storage.len(), limits::MAX_STORAGE_BYTES);
    storage.clear();
    assert_eq!(
        status_of(write(limits::MAX_STORAGE_BYTES + 1)),
        10 + status::LIMIT
    );
    assert!(storage.is_empty() && !storage.is_dirty());
    assert_eq!(
        failure(
            v2(
                "",
                "(call $storage_write (i32.const -8) (i32.const 64))",
                &V2_CAPS,
                shared.clone()
            )
            .deliver(1, &input_event())
        ),
        Failure::MemoryOutOfBounds
    );
    assert!(storage.is_empty() && !storage.is_dirty());
    // 盘上读回的初值超过上限：整块不要。
    assert!(Storage::new(vec![1; limits::MAX_STORAGE_BYTES + 1]).is_empty());
    assert_eq!(
        Storage::new(vec![1; limits::MAX_STORAGE_BYTES]).len(),
        limits::MAX_STORAGE_BYTES
    );
}

#[test]
fn a_failed_hook_does_not_write_storage() {
    let write = r#"(drop (call $storage_write (i32.const 0) (i32.const 3)))"#;
    for (tail, expected) in [
        ("(i32.const -1)", Failure::NegativeReturn),
        ("unreachable", Failure::Unreachable),
        ("(loop $spin (br $spin)) (i32.const 0)", Failure::OutOfFuel),
    ] {
        let storage = Storage::new(b"old".to_vec());
        let mut script = v2(
            r#"(data (i32.const 0) "new")"#,
            &format!("{write} {tail}"),
            &V2_CAPS,
            resources(&[], &[], Some(&storage)),
        );
        assert_eq!(failure(script.deliver(1, &input_event())), expected);
        assert_eq!(storage.contents(), b"old", "{tail}");
        assert!(!storage.is_dirty());
    }
    // 被本轮配额截断（不算失败）同样作废。
    let storage = Storage::new(b"old".to_vec());
    let mut script = v2(
        r#"(data (i32.const 0) "new")"#,
        &format!("{write} (loop $spin (br $spin)) (i32.const 0)"),
        &V2_CAPS,
        resources(&[], &[], Some(&storage)),
    );
    script.resume_turn(1, 100, limits::BEFORE_COMMIT_FUEL, limits::COMMAND_FUEL);
    assert!(matches!(
        script.deliver(1, &input_event()),
        Outcome::TurnExhausted { .. }
    ));
    assert_eq!(storage.contents(), b"old");
}

#[test]
fn local_mode_masks_text_but_not_settings_data_or_storage() {
    let storage = Storage::new(b"abc".to_vec());
    let body = format!(
        "(i32.add
            (i32.mul (i32.add (call $commit_text (i32.const {commit}) (i32.const 256) (i32.const 64)) (i32.const 10)) (i32.const 10000))
            (i32.add
                (i32.mul (i32.add (call $storage_len) (i32.const 10)) (i32.const 100))
                (i32.add (i32.add (call $setting_count) (i32.wrap_i64 (call $data_len (i32.const 0)))) (i32.const 10))))",
        commit = commit_field::COMMIT
    );
    let shared = resources(&["1"], &[b"xy"], Some(&storage));
    let run =
        |event: &Event<'_>| status_of(v2("", &body, &V2_CAPS, shared.clone()).deliver(1, event));
    assert_eq!(
        run(&commit_in_mode(0)),
        ("秘密".len() as i32 + 10) * 10000 + 13 * 100 + 13
    );
    for mode in [7, 8, 13, 255] {
        assert_eq!(
            run(&commit_in_mode(mode)),
            (status::ABSENT + 10) * 10000 + 13 * 100 + 13,
            "{mode}"
        );
    }
}

#[test]
fn debug_sinks_observe_inserts_and_storage_writes() {
    let actions = Arc::new(Mutex::new(Vec::new()));
    let sink = Arc::clone(&actions);
    let wat = module_wat(
        V2_IMPORTS,
        r#"(data (i32.const 0) "x")"#,
        "(drop (call $insert (i32.const 0) (i32.const 0) (i32.const 1) (i32.const 0) (i32.const 0)))
         (drop (call $storage_write (i32.const 0) (i32.const 1)))
         (i32.const 0)",
    );
    let module = compile_wat(&wat, declaration(&V2_CAPS, &Hook::ALL)).unwrap();
    let mut script = ScriptInstance::with_resources(
        &module,
        V2_CAPS.into_iter().collect(),
        limits::INIT_FUEL,
        resources(&[], &[], Some(&Storage::default())),
        DebugSinks {
            log: None,
            actions: Some(Box::new(move |import, status| {
                sink.lock().unwrap().push((import, status));
            })),
        },
    )
    .unwrap();
    status_of(script.deliver(1, &candidates_event(Hook::Candidates, 0)));
    status_of(script.deliver(2, &input_event()));
    assert_eq!(
        *actions.lock().unwrap(),
        vec![
            ("candidates_insert", 0),
            ("storage_write", 0),
            ("candidates_insert", status::DENIED),
            ("storage_write", 0),
        ]
    );
}

#[test]
fn storage_debug_output_and_clearing_do_not_show_its_contents() {
    let storage = Storage::new(b"secret".to_vec());
    let printed = format!(
        "{storage:?} {:?}",
        resources(&["hunter2"], &[b"data"], Some(&storage))
    );
    assert!(printed.contains("len: 6"), "{printed}");
    assert!(
        !printed.contains("secret") && !printed.contains("115") && !printed.contains("hunter2"),
        "{printed}"
    );
    storage.mark_dirty();
    storage.clear();
    assert!(storage.is_empty() && !storage.is_dirty());
}
