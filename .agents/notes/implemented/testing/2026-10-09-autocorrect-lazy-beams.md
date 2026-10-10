# Agent Note: 自动纠错搜索缓冲延迟分配

Status: implemented

## Problem

`pinyin::autocorrect::Search::new` 为输入的每个字节位置都创建容量为 `k` 的 beam，并按 `length * k` 为序列 interning 的 `HashMap` 预留桶位。许多位置没有任何可达路径，尤其是无法解释的输入仍会为每个空位置和空序列表分配存储。

## Decision

初始化时只建立空的 position `Vec` 和空的序列 `HashMap`，第一条路径写入某个位置前再精确预留 `k` 个 beam 槽位，并在首次序列 interning 前按原来的 `length * k` 提示预留哈希桶位。已有路径仍按原来的容量和追加顺序工作；序列编号、最终排序、去重、截断和回溯结构保持不变。

## Alternatives considered

- **继续为所有位置预留 `k`**：实现最简单，但空位置的分配与输入长度成正比。
- **所有 beam 共用一块连续缓冲**：可以减少外层分配，但会改变按位置保存和 predecessor 索引的结构，扩大回溯改动范围。
- **使用固定栈数组承载 beam**：常用 `k` 为 9，但公开接口允许更大的 `k`，固定容量会引入截断或额外回退路径。
- **继续初始化序列 `HashMap`**：可以保留首次插入的低成本路径，但不可达输入永远不会使用这块按 `length * k` 计算的存储。

## Verification

新增测试证明 `Search::new(64, 9)` 的 position beam 和序列表都保持零容量，首条路径到达时仍精确获得 `k` 个 beam 槽位并按原提示建立序列表；不可读的合成输入 `qqqqqqqq` 查询保持空结果，分配从 3 次降为 2 次。自动纠错测试、引擎全量单元测试、golden 测试、doc tests、Clippy、Rustfmt、差异检查、笔记校验和 `bash scripts/verify-local.sh --quick` 均通过。

## Consequences

无可达路径的位置和整个搜索的序列表不再分配缓冲；可达路径的容量、候选顺序、纠错成本、去重结果和回溯内容保持不变。非空路径仍按原始提示为序列表预留，避免把空结果优化变成逐项扩容。
