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
    script.resume_turn(3, 0, limits::BEFORE_COMMIT_FUEL);
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
    cut.resume_turn(5, 100, limits::BEFORE_COMMIT_FUEL);
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
    snapshot.resume_turn(6, limits::TURN_FUEL, 100);
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

/// `hook` 这一次的事件：`before_commit` 带上屏文本，其余只有标量。
fn event_for(hook: Hook) -> Event<'static> {
    if hook == Hook::BeforeCommit {
        return before_commit_event("abc", "abc");
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
    for hook in [Hook::Input, Hook::Candidates, Hook::BeforeCommit] {
        let half = limits::hook_fuel(hook) / 2;
        let resume = |script: &mut ScriptInstance, left: u64| {
            if hook == Hook::BeforeCommit {
                script.resume_turn(1, limits::TURN_FUEL, left);
            } else {
                script.resume_turn(1, left, limits::BEFORE_COMMIT_FUEL);
            }
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
        // 两种情形里本轮这一份都记为用完；`before_commit` 跑在快照上，持久实例照样可用。
        for script in [&below, &at] {
            assert_eq!(script.is_usable(), hook == Hook::BeforeCommit, "{hook:?}");
            let left = if hook == Hook::BeforeCommit {
                script.before_commit_fuel_left()
            } else {
                script.turn_fuel_left()
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
    // 新一轮两份都补满。
    assert!(matches!(
        script.deliver(4, &input_event()),
        Outcome::Delivered { .. }
    ));
    assert_eq!(script.before_commit_fuel_left(), limits::BEFORE_COMMIT_FUEL);
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
