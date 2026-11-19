# Agent Note: 调查 Notepad-- 兼容性

Status: proposed

## Problem

`#11626` 报告 Windows 11 26H2 的 Notepad-- v3.9.2 plugin/Qt6 安装包在使用水杉时卡死。报告者补充了 Windows x86 环境和 `Notepad--.exe` 在 `KERNELBASE.dll` 抛出 `c06d007e` 的事件摘要。切换系统输入法不能恢复，当前证据还不能区分第三方插件、Qt 运行库和 TSF 宿主初始化路径。

## Proposal

建立独立 Draft PR 跟踪包含 `#11601` 的 Windows x86 构建上的复测。复测使用合成文本，记录 Notepad-- 完整架构、切换水杉后的启动结果以及新的脱敏模块名、异常码和调用栈摘要。只有在水杉路径仍能稳定触发时，才在本 PR 加入最小宿主兼容修复和回归测试。

## Alternatives considered

- **直接复用 `#11601` 的静态 CRT 修复并宣称解决**：最强理由是异常发生在第三方宿主进程，且 `#11601` 已处理同类运行库复用风险，但没有 Notepad-- 直接复测证据，因此不采用。
- **在 TSF 宿主中加入通用异常吞掉或延迟初始化**：最强理由是可能绕开单一第三方进程的启动崩溃，但会隐藏真实宿主错误并影响所有编辑器，因此不采用。

## Acceptance criteria

- 在包含 `#11601` 的 Windows x86 构建中完成 Notepad-- 最小复测。
- 若问题仍归因于水杉，加入对应代码、回归测试和 Windows 验收记录。

## Risks

当前 PR 只有调查记录，不宣称已修复第三方编辑器。未经复现的 TSF 生命周期改动可能影响其他宿主。
