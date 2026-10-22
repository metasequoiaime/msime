# Agent Note: 双拼未完成尾音节保持较长候选优先

Status: implemented

## Problem

#9264：小鹤双拼输入完整词组后继续输入下一个音节的首键时，末尾直接辅助码路径会把这个裸字母当成单字辅助码。若该字母不匹配前缀候选中的任何汉字，短的整段回退行会排到完整前缀词组之前，候选看起来会随输入变长而变短。

## Decision

在双拼单字辅助码查询中，若尾部辅助码没有匹配任何基础候选，把前缀候选和把尾字母按普通拼音解释的整段回退行合并后按实际按键覆盖长度排序，完整前缀行自然排在较短回退行之前。仍有辅助码匹配时保留原有顺序，避免改变真正的辅助码选词语义。

## Alternatives considered

- **关闭末尾直接辅助码**：会改变用户现有辅助码设置，且不能修复仅在未匹配尾字母时发生的候选排序问题。
- **删除整段回退行**：会丢失用户把尾字母当作普通拼音继续输入时的候选，破坏既有单辅助码兼容行为。
- **所有情况下都把基础候选放到整段回退前**：会改变实际命中辅助码时的首选顺序，因此只在没有任何辅助码命中时调整。

## Consequences

输入 `fgkluvu` 这类“完整的 `疯狂水` 加下一个未完成音节首键”的序列时，`疯狂水` 保持在 `疯狂` 前面；真正命中辅助码的输入继续按辅助码匹配优先级排列。

## Verification

- `cargo test -p msime-engine shuangpin_unmatched_tail -- --nocapture`（远端 `rbuild`，2 项通过）
- `cargo test -p msime-engine shuangpin -- --nocapture`（远端 `rbuild`，191 项通过，1 项忽略）
- `bash scripts/verify-local.sh --quick`（远端 `rbuild`；Rust、Linux、Android、Wasm、macOS 等阶段通过，Windows 本机既有 provider-list 断言失败）
- `pnpm run verify-notes`、`git diff --check`
