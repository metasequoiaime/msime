# Agent Note: 完整候选列表放出尾部时在 release 构建里也推进代次

Status: implemented

## Problem

`crates/input-runtime/src/runtime.rs` 的 `all_candidates` 在放出引擎扣下的尾部候选之后推进代次，因为放出的尾部会重排座位，旧代次的候选 ID 若仍有效就会按新座位选中别的候选。推进写在 `debug_assert!(self.advance().is_ok())` 里，而 release 构建不执行 `debug_assert!` 里的表达式，所以发布出去的输入法从不推进：打开完整候选面板之后，宿主手里旧页的 ID 仍然能选，选中的可能是重排后同一座位上的另一个候选。debug 构建和全部现有测试都看不到这个问题。

## Decision

把推进移到断言外面，断言只检查结果：`let advanced = self.advance(); debug_assert!(advanced.is_ok());`。上面已经单独处理了 `generation == u64::MAX`，所以推进在这里不会失败，断言的含义不变。

## Alternatives considered

- **把断言改成 `assert!`**：release 构建里多一次永远不会触发的检查，而且一旦触发就是整个输入进程 panic；推进失败本来已有 `u64::MAX` 分支兜底，不需要在这里终止进程。

## Consequences

release 构建里打开完整列表并放出尾部后，旧页的 ID 失效，宿主须按新快照的 ID 选择，与 debug 构建一致。全仓按 `debug_assert!` 带副作用的写法扫过一遍，只有这一处。

## Verification

`crates/input-runtime/src/tests.rs` 的 `releasing_the_held_tail_publishes_a_new_generation` 断言放出尾部后快照的代次大于之前、并与当前 view 一致。修复前用 `cargo test -p msime-input-runtime --release` 运行它在 `full.generation > before` 处失败，debug 构建下修复前后都通过；修复后 release 和 debug 下整个 crate 的 182 个测试都通过。

[Android 展开候选后同步视图](2026-10-15-android-expanded-candidate-view.md)补齐宿主读取完整候选导致的代次失步，保留本篇的运行时身份围栏。
