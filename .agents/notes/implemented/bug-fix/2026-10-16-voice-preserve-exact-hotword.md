# Agent Note: 语音热词纠错保留同长度的正确词形

Status: implemented

## Problem

`voice::hotwords::correct` 按词长与词库权重依次尝试拼音匹配。词库同时包含合成词「明天」与「名天」时，即使识别文本已经是「名天」，仍会被排在前面的「明天」覆盖；模糊拼音也有同样的问题。这违背该函数保留已有正确热词的约定，宿主经 `msime_client_voice_hotword_correct` 收到的是改坏的最终文本。

## Decision

保留最长热词优先、从左到右且不重叠的匹配；选中某个长度的匹配后，优先使用该长度中与原文完全一致的有效热词。没有正确词形时继续采用该长度中权重最高的拼音匹配。

## Historical audit

检索活跃笔记中的 `hotword`、热词：Android 备份学习状态笔记仅提及备份数据范围，与纠错优先级无关；未发现持有该约定的旧笔记。源码与既有测试约定最长词优先及同长度按输入顺序选取，继续保留。

## Alternatives considered

- 全局先保护任何长度的正确热词：最能减少替换，但较短的正确词会阻断长热词纠错，改变已有最长词优先的行为，因此只在已选长度内保护正确词形。
- 取消所有同音词纠错：最保守，但识别出的词不在热词表时仍需要按权重消歧；仅保护表中已有的正确词形。

## Verification

新增同音与模糊拼音两项共享层回归和一项 C ABI 回归，旧实现下均失败。回归同时覆盖热词反向排列、没有完全一致热词时按输入顺序纠错，以及较长匹配优先于较短正确热词的边界。

完整 `msime-client-providers` 与 `msime-host-api` 测试共 848 项通过、3 项忽略；独立审查复验 12 项共享热词测试及 3 项 C ABI 热词测试通过，无阻断意见。

`cargo fmt --all -- --check`、两 crate 的 Clippy（`--all-targets --locked -- -D warnings`）、笔记门禁、差异检查与 quick 通过。quick 覆盖工作区、Android、WASM、Android Java、macOS 原生宿主及共享 Apple 桥接层；Linux 容器、Windows 交叉编译及 Harmony 真编译因本机工具或依赖不齐跳过。

## Consequences

每次找到拼音匹配后额外检查同长度热词，增加一次有界扫描；不增加文件访问或网络调用，不修改接口格式。
