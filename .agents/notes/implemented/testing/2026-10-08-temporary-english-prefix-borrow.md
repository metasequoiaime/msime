# Agent Note: 临时英文前缀借用

Status: implemented

## Problem

临时英文候选查询每次都对已经是小写的输入调用 `to_ascii_lowercase()`，在按键路径上重复分配前缀字符串。

## Decision

小写 ASCII 前缀直接借用原字符串，只有包含大写或其他字符时才创建小写副本；词典查询和候选顺序保持不变。

## Alternatives considered

始终创建小写副本会保留重复分配；强制上游预先规范化会改变本地模式的输入边界，因此只在查询点按内容选择借用或拥有。

## Verification

新增小写前缀零分配测试；完整 `msime-engine` 单测与 golden、Clippy、格式检查、笔记检查和 quick 门禁覆盖提交。

## Consequences

临时英文模式处理常见小写输入时不再为查询前缀分配堆内存，大小写输入仍按原规则转换后查询。
