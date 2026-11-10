# Agent Note: Fcitx5 面板回退遵守候选窗口预编辑开关

Status: implemented

## Problem

JetBrains 等 Linux 编辑器可能不声明 Fcitx5 的客户端预编辑能力。此时 Fcitx5 会把宿主提交的行内预编辑回退画到候选面板。候选窗口预编辑已经设为「不显示」时，旧实现仍按 `tsf_preedit_style` 把这段文字放进面板，导致设置与实际候选窗不一致。

## Decision

Fcitx5 在没有 `CapabilityFlag::Preedit` 时，只有候选窗口预编辑没有关闭才把普通方案的预编辑交给面板；候选窗口预编辑关闭后清掉这条回退。韩文等 `AlwaysInlinePreedit` 方案必须在面板中显示正在组成的文字，继续保留例外。声明了客户端预编辑能力的上下文仍只收到 `tsf_preedit_style` 控制的客户端预编辑，候选窗口辅助行继续由 `candidate_preedit_style` 控制。

## Alternatives considered

- **同时关闭所有行内预编辑。** 能隐藏候选弹窗中的文字，但会改变支持客户端预编辑的编辑器行为，也违背设置里把“候选窗口预编辑”和“行内预编辑”分开的契约。
- **按 JetBrains 程序名特殊处理。** 可以针对报告者环境绕过问题，但无法覆盖其它不支持客户端预编辑的 Linux 前端，也会把宿主行为绑定到易变的程序名。

## Consequences

不支持客户端预编辑的前端在用户隐藏候选窗口预编辑时不会再看到候选弹窗顶部的行内编码；支持客户端预编辑的前端和必须内嵌文字的方案保持原行为。原生 Fcitx5 回归测试覆盖关闭后清空和恢复开关后重新显示。真实 JetBrains/Wayland 验收仍需报告者按同版本组合复验。
