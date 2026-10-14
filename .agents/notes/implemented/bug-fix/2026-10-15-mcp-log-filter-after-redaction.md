# Agent Note: MCP 诊断日志只按脱敏后的内容过滤

Status: implemented

## Problem

`read_diagnostic_log` 返回 Windows TIP 按键记录时会遮盖 `vk=`、`wch=` 和 `key=`，但原实现先在原始行上执行 `contains` 过滤。调用方可以通过命中行和 `has_more` 探测被遮盖的按键字段。

## Decision

日志行先经过时间窗口筛选和按键字段脱敏，再用脱敏后的文本执行 `contains` 过滤。`lines` 和 `has_more` 都来自同一批脱敏后的匹配行；时间戳仍从原始行读取，以保持无时间戳续行继承前一条时间窗口的规则。

## Alternatives considered

- 只在输出时继续脱敏，并拒绝包含 `vk=`、`wch=` 或 `key=` 的查询：可以挡住常见字面查询，但其他子串仍可能探测字段值。
- 把整份日志先脱敏再筛选：语义同样安全，但会复制完整文本；逐行脱敏能保持现有时间窗口筛选并限制额外分配。

## Consequences

调用方仍能按公开事件名和脱敏占位符查找日志，按键原值不再影响搜索结果或分页提示。进入时间窗口的每行会在匹配前生成脱敏文本，增加一次按行分配；日志读取仍受既有尾部字节上限约束。

## Verification

单元回归使用合成按键记录，确认 `vk=0x41`、`wch=U+0061` 和 `key=a` 查询均无结果，`key=-` 能命中脱敏行。MCP stdio 回归同时检查 `has_more=false` 和公开字段查询。完整测试、格式、笔记校验和 quick 门禁随本次提交运行。
