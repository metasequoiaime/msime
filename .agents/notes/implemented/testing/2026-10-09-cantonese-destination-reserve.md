# Agent Note: 粤拼候选目标缓冲预留容量

Status: implemented

## Problem

粤拼查询会复用目标 `Vec<WordItem>` 的现有行；当本次词典结果多于已有行时，扩展分支直接追加新行。目标容量不足时会先后经历多次几何扩容，冷查询或候选数增长会为外层向量产生额外分配。

## Decision

在粤拼目标缓冲进入追加分支时，按 `source.len() - destination.len()` 使用 `reserve_exact` 预留缺少的行容量，再追加新候选。目标已有行仍原地复用，结果顺序和行内容保持不变。

## Alternatives considered

- **继续让 `extend` 自动扩容**：代码最少，但容量增长由几何策略决定，可能发生多次外层向量重新分配。
- **每次查询按来源总数重建目标向量**：可以得到精确容量，但会丢掉已复用的 `WordItem` 行存储并增加字符串初始化成本。
- **在所有查询入口统一预留**：会把粤拼特有的来源数量和目标缓冲关系扩散到调用方，增加不必要的接口耦合。

## Verification

扩展粤拼 registry 查询回归，在首次从空目标缓冲查询时断言目标容量等于候选长度，防止回退到几何扩容。相关 registry 测试通过；随后运行 Engine 全量测试、Clippy、Rustfmt、差异检查、笔记检查和 quick 门禁。

## Consequences

粤拼候选数增长时外层 `Vec<WordItem>` 只需一次按缺口的容量准备，同时保留现有行和字符串容量复用。`reserve_exact` 只在目标容量不足的追加分支执行，不影响缩短结果时的截断路径。
