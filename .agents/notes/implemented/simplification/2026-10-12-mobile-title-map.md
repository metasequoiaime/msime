# Agent Note: 移动端主标签标题复用映射

Status: implemented

## Problem

`mobileTabTitle` 和 `mobilePageTitle` 分别维护“设置、统计、我的”的相同标题映射。两份映射可能在页面标题和底部标签之间产生不一致。

## Decision

提取 `mobilePrimaryTitles` 作为共享来源；标签函数继续追加“对话”特殊标题，页面函数继续保留皮肤、候选栏等页面专用标题。

## Alternatives considered

- 让两个函数继续各自维护映射：实现简单，但会重复主标签规则。
- 合并两个完整函数：会混淆标签和页面的不同命名语义。

## Consequences

主标签标题只需在一处维护，两个投影函数的专用标题行为保持不变。现有移动标题测试覆盖共享映射和各自特殊项。
