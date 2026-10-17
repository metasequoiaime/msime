# Agent Note: 九键表情读法借用普通网格边界

Status: implemented

## Problem

九键混排会为每条完整读法路径调用 `reading_boundaries`。普通九键和 14 键中，数字边界在所有读法之间完全相同，但旧实现仍为每条路径复制一个 `Vec<usize>`；这些边界随后只用于按相同边界分组。

## Decision

`expressive_readings` 以 `Cow<[usize]>` 携带边界：普通网格借用调用方的边界切片，九键双拼仍拥有按读法换算出的字母位置。混排分组只在首次出现某组边界时物化一份 `Vec`，读法文字、分组顺序、查询边界和候选顺序保持不变。

## Historical audit

已检索九键表情读法、边界转换、混排路径和分配缓冲相关的 `proposed`、`implemented`、`rejected` 笔记。表情候选功能、混排资源路径和读法行缓冲已有独立契约；本片只处理普通网格边界的所有权，不改变表情查询或双拼边界换算。

## Alternatives considered

- 每条路径继续复制边界：实现最直接，但普通九键的相同边界重复分配。
- 只在 `expressive_candidates` 外层缓存一份并让读取函数返回索引：可避免 `Cow`，但需要改变内部返回契约并为双拼维护额外索引；借用/拥有统一表达更小。
- 所有网格都借用边界：会丢失九键双拼每条读法的字母位置换算；双拼必须保留拥有型结果。

## Verification

固定 `634486` 普通九键表情读法路径的分配计数从旧实现 85 降为 45；新增测试门槛为不超过 84。九键模块 136 项测试通过，完整 engine/runtime 为引擎 2254、golden 31、runtime 318 通过（引擎 40 项既有忽略），Clippy（`-D warnings`）、Rustfmt、笔记检查和 `git diff --check` 通过。`verify-local.sh --quick` 的 Rust workspace、Android host Java、WASM、Windows 契约和多数仓库级检查通过；Windows 测试保留两项宿主差异（`backspace_selected_phrase`、`host_online_candidates`），Apple bridge 构建失败，quick 以失败结束。相关失败不在本片 Rust 代码路径，不能把 quick 记为全绿。

## Consequences

普通网格不再为每条读法路径申请相同边界向量；双拼仍按路径拥有转换后的边界，生命周期和语义不变。收益只发生在启用混排的读法路径，不能据此宣称九键整体 16 ms 目标完成。
