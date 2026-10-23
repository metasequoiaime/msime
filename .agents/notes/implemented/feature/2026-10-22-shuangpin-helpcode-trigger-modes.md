# Agent Note: 双拼辅助码三种触发方式全部接入引擎

Status: implemented

## Problem

PR #9243 把 `ShuangpinHelpcodeModes` 结构、默认值、写入端互斥归一和设置页控件做完了，但引擎不读 `trigger_backtick`/`trigger_semicolon`/`trigger_uppercase`/`mid_sentence*`/`direct_helpcode*`，只有 `direct`（末尾直接）接了门；设置页只渲染 `direct` 一行，其余开关「不伪装成可用功能」。用户打开开关没有任何效果。Windows v0.11.1 的三种触发方式是：末尾间接（触发键 + 1~2 辅码）、句中间接（触发键段绑定前一个音节进整句解码）、万象式直接（无引导键逐音节解析）。

## Decision

分三个独立提交把三组开关全部接入 `crates/engine`，并让设置页放行全部控件。

**第 1 步：末尾间接（`trigger_*`）。** 输入串契约引入两个标记字符：`|` 记末尾段、`` ` `` 记句中段。`ShuangpinScheme` 按请求里的模式开关决定敲下的键是触发键还是原路径字符；`;` 先让给 `ing` 韵母（`accepts_semicolon_final_at`），两侧顺序与 Windows 一致。段解析进 `QueryRequest.end_helpcodes`，`query_standard` 走一码重排、两码筛选。段不在输入串末尾时只当分隔符。

**第 2 步：句中间接（`mid_sentence*`）。** `` ` ``/`;`/大写码在光标处（含串中间）开一段，段绑定前一个完整音节，解析成 `QueryRequest.syllable_helpcodes` 逐音节约束。词典层 `set_syllable_helpcodes` 把签名拼进系列/单码/双码/在线写入的全部缓存键；`filter_by_syllable_helpcodes` 只筛整词汉字的拼音行，约束音节越出词长不拦（前缀行保留）；`generate_constrained_sentences` 的 `accepts` 回调用同一套约束进 lattice 解码，受约束整句插回候选前（同词留词库行）。没有辅助码表时约束作废而不是把候选全筛没。

**第 3 步：万象式直接（`direct_helpcode*`）。** `direct_helpcode_paths` 从恒开改为受 `direct_helpcode` 主开关（出厂关，与 Windows 一致）。`direct_helpcode_slash` 开着时 `/` 落进输入串当一位/两位辅码后的段终止符（`uia/`、`uiab/`）；`direct_helpcode_uppercase` 开着时第二码大写结束一段（`uiaB`）。`has_literal_input` 的 `direct_helpcodes_own_capitals` 判定同样受这两个开关门控。存量辅助码测试写于恒开版本：`shuangpin/tests.rs` 的 `request()` 按 helpcode 同步打开直接开关，会话侧用例显式开 `direct_helpcode`。

**输入串契约改动的函数**：`segment_input`/`segment_raw_boundaries`/`remove_manual_delimiters`（查询串剥段）、`has_active_helpcode`（末尾段 + 直接态）、`apply_shuangpin_helpcode_segmentation`（预编辑回填原标记）、`insert_at_caret`/`shuangpin_helpcode_marker`（标记键接受与映射）、`has_literal_input`（大写判定看去段后的查询串）、`advance_composition_after_selection`（选词后剩余串的段坐标重映射）。

## Existing notes

[偏好结构与默认值](2026-10-21-shuangpin-helpcode-trigger-modes.md)部分重叠：结构、互斥归一和默认值继续由旧篇持有，本篇接管旧篇明确保留的引擎触发缺口。[未完成尾音节候选顺序](../bug-fix/2026-10-22-shuangpin-incomplete-tail-candidate-order.md)是独立排序修复，合并时保留它的较长前缀优先规则。

## Alternatives considered

- **在 session 层把标记串直接拆成拼音查询，词典不感知约束**：缓存键不带签名会让同串不同约束共用旧答案，且 lattice 约束句无法进解码，被否。
- **`/` 走 `|`/`` ` `` 同一套标记映射**：`/ ` 是万象式解析自己的终止符，不是通用段标记；统一映射会让剥段后的查询串丢掉 `/`，直接路径拿不到它，被否。

## Consequences

- `ShuangpinHelpcodeModes` 全部字段生效：设置页开关与引擎行为一一对应，「开关惰性」消除；`direct_helpcode` 出厂关使万象式直接不再恒开，行为与 Windows 出厂一致，原来依赖恒开的存量测试显式开门。
- 系列/辅助码/在线写入缓存键全部带上逐音节约束签名，同串不同约束不共用答案；约束为空时签名串为空、缓存键形状不变，旧路径零成本。
- 输入串里可能出现 `|`、`` ` ``、`/` 三个标记字符：查询串在请求层剥净（词典看不到），预编辑/编辑/选词剩余串都按原标记串走；后续任何读 `raw_input` 的新代码必须分清它要的是原串还是查询串。

## Evidence

- 先失败后通过：第 1 步红测 `step1_red_probe_semicolon_trigger`（旧代码 `;` 不处理）；第 2/3 步各两个新测试在实现前断言为空/未筛、实现后命中（`shuangpin_mid_sentence_*`、`shuangpin_direct_helpcode_*`）。
- `cargo test -p msime-engine`：2475 passed / 0 failed / 51 ignored；`msime-client-core`、`msime-input-runtime`、`msime-host-api`、`msime-mcp-server` check/test 全绿；`cargo fmt --all --check` 干净。
- `pnpm --dir apps/desktop exec tsc --noEmit` 干净；`vitest run tests/settings/helpcode-page.test.tsx` 8/8。

## Follow-ups

- `insert_at_caret` 的 `/` 只覆盖光标在末尾的路径；光标落在串中间敲 `/` 仍按未处理键交还宿主（Windows 同），需要时另评。
- 万象式直接的句中编辑（删掉一个音节键后剩余串的段重挂）未逐例核对 Windows 的 `RemapMidSentenceRest` 全部分支；当前按剩余串重新解析约束，已覆盖选词后重挂。


## Integration verification

合并 `develop` 时保留较长前缀排序、共享触发方式字段和全部设置开关；去掉自动合并重复加入的 `set_shuangpin_helpcode_modes`，由会重绑方案触发规则的版本承接。51 项双拼定向引擎测试、8 项设置页测试、前端类型检查和生产构建通过。按当前工具链清除不必要的闭包 `mut`、用 `is_multiple_of` 表达偶数判定，并把直接辅助码的两种结束标记作为一组传入递归，Clippy 的全部 target 在 `-D warnings` 下通过。复审前整合状态的完整引擎回归为 2494 项通过、55 项忽略；复审后补齐直接辅助码扫描器对两种结束标记的一致处理，以及关闭直接模式时大写路径的归属回归，53 项双拼定向用例通过。首次 quick 因语音语言目录导出冷编译超时失败；预编译成功后完整重跑退出码 0，报告 `quick check passed`，仓库契约、Rust workspace、Android/wasm 目标、Android Java 宿主、Linux 设置壳与原生宿主、macOS 原生宿主及共享 Apple 桥接层通过。MinGW 与 HarmonyOS 打包前置缺失的阶段按脚本明确跳过。

提交钩子还检出整合进来的四个 TypeScript 文件不符合当前 oxfmt 输出；仅对钩子列出的共享候选样式组件、HarmonyOS 两个标点策略和键盘测试做格式化，未改行为。
