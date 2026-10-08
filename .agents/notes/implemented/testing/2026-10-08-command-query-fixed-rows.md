# Agent Note: `/` 查询使用固定行缓冲

Status: implemented

## Problem

`query_command` 的结果最多保留 18 行，但每次查询都会先为临时 `(trigger, text)` 行创建堆上的 `Vec`，即使输入没有任何命令匹配也会分配容量。

## Decision

使用 18 槽的栈上引用数组保存拥有的临时行，并在达到结果上限后停止收集；最终仍创建接口要求的 `WordItem` 结果。保持翻译行、表格命令、内置日期时间行的优先级、去重和权重顺序。

## Alternatives considered

继续使用动态 `Vec` 会为受 18 行上限约束的临时结果保留堆缓冲；直接写入最终 `WordItem` 会让文本去重与命令行优先级处理耦合。固定数组只替换临时行容器，最终输出接口不变。

## Verification

新增未知命令零行缓冲分配测试；命令模块测试、引擎完整测试、Clippy、Rustfmt、差异检查、笔记检查和 quick 门禁覆盖实现。

## Consequences

没有匹配结果的 `/` 查询不再为临时行缓冲分配堆内存；有结果的查询只保留最多 18 个临时行，输出内容和权重保持不变。
