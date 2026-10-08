# Agent Note: `@` 地点补全使用固定行缓冲

Status: implemented

## Problem

地点补全的 exact 和 prefix 结果各自最多 18 行，但每次查询仍会为两组临时结果创建堆上的 `Vec`，最终只需要一个合并后的输出列表。

## Decision

用两个固定 18 槽的栈上数组收集 exact/prefix 地点行，再一次性创建最终输出 `Vec`。保持 exact 优先、prefix 截断、表格顺序、已有名称去重和结果上限。

## Alternatives considered

继续保留两组动态 `Vec` 会为固定上限的中间状态分配额外堆内存；直接写入最终列表会丢失 exact 与 prefix 分组的合并规则。固定数组只替换中间缓冲，输出接口不变。

## Verification

新增地点查询只产生最终输出分配的回归测试；mention 模块测试、引擎完整测试、Clippy、Rustfmt、差异检查、笔记检查和 quick 门禁覆盖实现。

## Consequences

启用地点补全的 `@` 查询少两组临时行缓冲分配，地点结果内容、顺序和去重语义保持不变。
