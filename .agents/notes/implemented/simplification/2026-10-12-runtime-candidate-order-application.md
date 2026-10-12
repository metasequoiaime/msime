# Agent Note: Runtime 共用候选排列应用

Status: implemented

## Problem

内置在线候选、整句读法降位和桌面脚本插件都需要把同一个排列同步应用到候选文本、编码、注释、来源、固定座位、纠正标记、答案键以及 `engine_order`。这些字段漏同步会让显示行与提交行脱节。

## Decision

在 `crates/input-runtime/src/runtime.rs` 增加 `Runtime::apply_candidate_order`，集中应用八个并行数组和引擎座位映射。插件变换继续在调用后同步自己的 `plugins.identities`，因为身份是插件侧的附加数组；排列规则和锁定逻辑仍保留在各自调用方。

## Alternatives considered

- 把插件身份数组也放进 Runtime 公共方法：该数组只在桌面插件模块存在，且不属于引擎候选快照，跨平台接口会被插件实现细节污染。
- 修改 Engine 的 `apply_order` 让它认识 Runtime 的所有数组：Engine 不拥有宿主的注释、来源和插件身份，违反分层边界。

## Consequences

候选排列的同步字段只有一个 Runtime 维护点，三条排序路径仍保留各自规则。现有输入运行时测试覆盖在线候选、重排、插件隐藏和座位映射。
