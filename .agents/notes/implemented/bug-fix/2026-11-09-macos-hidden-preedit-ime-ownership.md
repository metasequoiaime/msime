# Agent Note: macOS 隐藏行内预编辑仍保持输入法接管

Status: implemented

## Problem

macOS 把 `tsf_preedit_style=empty` 映射为零长度 marked text。文本桥在客户端本来没有 marked text 时会跳过这次更新；输入法虽然返回已处理，IMK 却没有收到任何 marked text，部分编辑器会把原始双拼编码重新交给应用，形成「`nimf`你们」这样的原文与候选同时上屏。

## Decision

共享 Apple 文本桥在行内预编辑隐藏但仍有组字、词段或整句改字时，发送一个空 marked text 来明确告知 IMK 这枚按键仍由输入法接管。宿主的 `clientHasMarkedText` 继续保持为假，因此后续空闲焦点更新不会重复清理客户端；没有组字且没有提交时仍跳过空清理，保留 #10980 之前为避免 IMK 焦点切换阻塞而建立的边界。

## Alternatives considered

- **继续完全跳过空 marked text。** 可以减少一次客户端回调，但无法向 IMK 表明隐藏预编辑仍在接管按键，正是报告问题的触发条件，因此放弃。
- **在控制器里为隐藏样式单独吞键或提交原始编码。** 这会把 IMK 所有权判断分散到宿主按键路径，且可能绕过共享文本桥的客户端生命周期与重入保护，因此不采用。

## Verification

`TextClientTest` 的跟踪 marked text 回归覆盖：普通组字、提交后的清理、隐藏样式下有组字时发送空标记，以及空闲时重复空清理仍被跳过。未在本机启动新的完整 macOS 构建：当前磁盘剩余 41 GB，按仓库规则保留现有构建目录并交由 PR 的 macOS job 编译和运行原生测试。

## Consequences

隐藏行内预编辑每个输入键会多一次 `setMarkedText:@""`，但不会在客户端保留 marked text，也不会改变文档内容；这换取了 IMK 对输入法按键所有权的明确确认。已有跟踪标记状态的焦点切换优化仍然生效，空闲状态不增加同步客户端调用。

## Existing notes audit

检索 `TextClient`、`setMarkedText`、`tsf_preedit_style`、隐藏预编辑和 IMK 焦点切换笔记。[macOS 行内组字显式声明预编辑样式](2026-10-19-macos-inline-preedit-caret.md)持有标记样式和光标位置，不涉及隐藏样式的按键所有权；[macOS 重入文本客户端上屏](2026-10-23-macos-reentrant-text-client-commit.md)持有转换代次和回调重入边界，本次沿用其调用接口，不改代次规则；[停止清理不存在的标记文本](2026-10-23-macos-reentrant-text-client-commit.md)的历史事实已由当前实现保留，本次只扩大“有组字时”的发送条件。
