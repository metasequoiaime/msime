# Agent Note: 五笔顶字保留后续按键

Status: implemented

## Problem

五笔四码已有候选时，第五个字母会先顶字上屏首选候选，再开始下一次输入。若第五个按键是引擎不接收的 Shift 大写字母，运行时原先丢弃了该按键；用户看到候选上屏，却少了实际按下的字母。

## Decision

顶字路径先让引擎处理后续按键，再把结果合并到已上屏的候选：引擎产生新的上屏文本时追加它；引擎明确不处理时按原样追加 ASCII 字母；引擎已经接管按键（例如 Shift+K 打开快捷短语）时不重复追加。合并结果继续携带原候选的提交上下文。

## Alternatives considered

- 只在宿主层重放第五个按键：各宿主对 Shift 状态和本地模式的判断不同，无法保证与引擎状态一致。
- 无条件把第五个字母追加到提交文本：Shift+K 等按键已经由引擎打开模式时会重复出现一个 `K`。
- 丢弃所有引擎未处理的后续按键：与普通字母能开始下一组字码的行为不一致，也会静默吞掉用户输入。

## Consequences

五笔顶字不会吞掉不属于编码的字母，同时保留本地模式入口和普通小写字母开始下一组编码的现有行为。提交文本和候选上下文仍作为一次转换返回给宿主。

## Verification

先让大写后续按键回归测试在旧实现上失败，再实现尾部合并。`cargo test -p msime-input-runtime --lib --locked`：160 passed；`cargo fmt --all -- --check`、`cargo clippy -p msime-input-runtime --all-targets --locked -- -D warnings`、`git diff --check` 和 `bash scripts/verify-local.sh --quick` 均通过。
