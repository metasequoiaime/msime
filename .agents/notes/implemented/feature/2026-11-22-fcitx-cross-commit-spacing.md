# Agent Note: Fcitx5 跨次上屏的中英文空格

Status: implemented

## Problem

[#11790](https://github.com/metasequoiaime/msime/issues/11790) 在 Arch Linux 的 Fcitx5 中分次上屏中英文时，需要手动补接缝。现有「中英文之间加空格」只处理同一次上屏的文本。

## Decision

Fcitx5 复用[跨次空格策略](2026-10-24-cross-commit-spacing-post-commit.md)的共享偏好、默认关闭、应用排除和 Host API。用户可用 `msime config set cross_commit_spacing=true` 明确开启。Fcitx5 通过当前输入上下文取得前一个字符，在发送同一次 `commitString` 前完成格式化。它按上屏时的引擎模式请求共享策略，英文直通键只有在需要补接缝时才由插件代为提交，其他键照常交给应用。

只使用客户端报告的折叠光标。前文无效、存在选区、输入框受限或隐私、全角输出，以及有方向键等待转发时保留原文。把新提交当成一次编辑，不触碰此前的正文，统计仍只记录原输入。已有[单次上屏空格](2026-10-12-chinese-english-spacing.md)继续独立启用，macOS 仍使用自己的文档时序。

## Alternatives considered

- 复用智能标点的前文影子可以覆盖不提供前文的客户端，但鼠标移动和应用编辑并不总会使影子及时失效，因此只使用当前客户端发布的前文。
- 先提交再替换能与 macOS 同步做法一致，但 Fcitx5 会使旧前文失效，Wayland 和异步客户端通常无法在此次按键回调内交回新文档。提前读取已发布的前文并提交一次，可避免删除和重插。

## Consequences

开关关闭时不读取前文或调用跨次策略。开启后需要编辑器提供有效的 surrounding text 和程序标识。覆盖 Fcitx5，IBus 保持既有行为。回归使用真实的 Fcitx5 输入上下文和 Host API，检查默认、双向接缝、已有空格、选区、排除名单以及无前文的退路。

## Verification

在 Ubuntu 容器中构建 Fcitx5 插件和测试程序，使用发布锁校验的词库运行 `fcitx5-native-test <资源目录> --punctuation-context`，退出码为 0；原生构建同时运行 134 项测试。报告者的 Arch Linux 编辑器尚未实测。
