# Agent Note: 笔画候选目标容量预留

Status: implemented

## Problem

笔画 registry 查询把新候选行追加到会话持有的目标向量时没有按缺口预留容量。短缓冲从一行增长到两行会触发 `Vec` 的几何扩容，保留了多余容量；逐键刷新时会重复承担这次增长。

## Decision

在笔画查询的追加分支调用 `reserve_exact(candidates.len() - destination.len())`，再追加缺少的 `WordItem`。已有行继续按字段复用，缩短结果仍直接截断。

## Alternatives considered

- **直接继续 `extend`**：会沿用 `Vec` 的几何扩容，在短缓冲增长时保留超出实际结果的容量。
- **每次重建目标向量**：能精确构造容量，但会丢失已有 `WordItem` 的字符串缓冲复用。
- **使用普通 `reserve`**：仍可能按增长策略多分配，不能保证只准备当前缺口。

## Verification

新增合成笔画词典回归：目标缓冲容量为 1、查询返回两行时，旧实现容量为 4 并失败，新实现容量保持为 2。相关 registry 测试、Engine 测试、Clippy、Rustfmt、差异检查、笔记检查和 quick 门禁覆盖提交。

## Consequences

笔画候选列表增长只准备实际缺少的外层行容量，候选内容、顺序和已有字符串复用行为不变。
