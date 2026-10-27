# Agent Note: Linux 模式往返保留已保存的英文标点

Status: implemented

## Problem

#9882 报告保存默认英文标点后，重新加载输入法又得到中文标点。Linux 的 IBus `resync_punctuation_for_mode` 和 Fcitx5 `resyncPunctuationForMode` 在「跟随」锁定下直接把中文标点设成 `input_enabled`，因此只要切到英文再回到中文，保存的 `chinese_punctuation=false` 就被内存状态覆盖；设置文件未变，下一次读设置又可能暂时恢复，表现为重载或焦点切换时反复跳变。

## Decision

「跟随」时英文模式仍使用英文标点；回到中文模式时读取保存的 `chinese_punctuation`，不把会话快捷键的临时选择写回文件。固定中英文标点继续优先。IBus 与 Fcitx5 同步修复，并在各自真实宿主测试中覆盖默认英文标点的模式往返。macOS 的同类起始标点规则已有[独立修复](../../implemented/bug-fix/2026-10-22-macos-initial-english-punctuation.md)，其运行时覆盖保留范围不同，本篇只处理 Linux 两个宿主。

## Alternatives considered

维持「跟随」等于中文模式强制中文标点，能让模式指示和标点状态总是一致；但设置页明确允许「跟随、中文模式默认英文标点」，该解释让已保存的选择在模式往返后失效。

## Verification

IBus `engine_smoke` 和 Fcitx5 `native` 各补一条默认英文标点的模式往返断言；修复前两条都会在回到中文模式时看到中文标点。运行时构建和设备复验受本机 50 GiB 磁盘门槛限制，交由 PR CI 执行。

## Consequences

保存为英文标点的用户在中文模式里切换到英文再切回来，仍得到英文标点。保存为中文标点、固定锁定值以及英文模式里的临时 `Ctrl+.` 选择保持原有规则。报告未给平台和版本，Linux 的确定性缺陷得到修复，但报告者环境仍需合并后复验。
