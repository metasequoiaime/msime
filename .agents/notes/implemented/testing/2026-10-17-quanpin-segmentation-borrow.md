# Agent Note: 全拼查询切分借用请求存储

Status: implemented

## Problem

`InputSession::pinyin_segmentation` 为只读调用复制 `normalized_segmentation` 或 `segmentation`。候选定位在没有规范化输入时、完整拼音判定以及双拼预编辑分隔符处理都只读取这段文本，临时 `String` 没有独立所有权需求。

## Decision

`pinyin_segmentation` 返回借用型 `Cow<'_, str>`，直接借用请求中已经构造好的切分字段。候选定位、在线候选学习和完整拼音判定沿用这份借用结果；双拼预编辑仍把 `Cow` 交给已有的尾部分隔符辅助函数，需要拥有结果时由调用方显式取得。请求字段、切分选择顺序和所有对外结果保持不变。

## Existing decisions

已检索全拼切分、借用和 `Cow` 相关笔记。[全拼显示分段按需拥有](2026-10-10-pinyin-display-cow.md)已经为带大小写的显示分段采用同一借用模式，本片扩展到不带大小写的查询切分；[全拼主切分标记复用已有字符串](2026-10-17-quanpin-marking-buffer.md)只复用词典解析投影的可变标记字段，不改变会话请求的只读借用。两篇职责不同，均继续保留。

## Alternatives considered

- **继续复制 `String`**：改动最小，但每次只读定位或完整性判定都申请临时存储。
- **让每个调用方直接选择请求字段**：可以避免复制，但会在多个调用点重复「规范切分为空时回退原切分」的规则，容易与显示逻辑分叉。
- **把切分缓存到会话状态**：可避免重复读取，但需要随请求刷新维护失效状态，并增加跨请求所有权；请求本身已经持有正确生命周期的字段，因此不采用。

## Verification

旧实现上的 `pinyin_segmentation_borrows_explicit_segmentation_when_normalized_input_is_empty` 先因返回拥有型 `String` 无法满足借用断言而失败；新实现通过。冻结旧实现的完整拼音判定在合成输入 `ni` 上分配 1 次，新实现为 0 次；字段内容和完整性结果相同。引擎会话测试 318 项、全量引擎库测试 2259 项（40 项既有忽略）、golden 31 项、Clippy、格式和 `git diff --check` 均通过。`pnpm --config.verify-deps-before-run=false run verify-notes` 校验 1241 篇笔记通过；`TMPDIR=/Volumes/Workspace/tmp/msime CARGO_BUILD_JOBS=4 MSIME_BUILD_JOBS=4 bash scripts/verify-local.sh --quick` 退出 0，Windows 本机 157 项通过、2 项宿主差异排除，Android 宿主与 Java、WASM、macOS 原生宿主和共享 Apple bridge 通过；Linux Docker、MinGW、HarmonyOS 真工程与 pipe-only 按环境跳过。

## Consequences

全拼只读切分调用不再复制请求字段，双拼尾部处理继续按需拥有；请求刷新和候选排序没有新缓存或状态。返回 `Cow` 的生命周期绑定当前会话借用，调用方若要跨请求保存必须显式 `into_owned`。本片只声明 Rust 临时分配减少，不推导 SQLite、RSS 或端到端设备延迟收益。

