# Agent Note: #11390 macOS 系统剪贴板历史调查

Status: proposed

## Problem

#11390 报告 macOS 26.6 切换水杉后 Spotlight 系统剪贴板历史为空。当前代码核查显示输入源切换监听只读取 `NSPasteboard` 的 `changeCount`、类型和字符串，不调用写入或清空 API。现有证据不足以把系统历史为空归因于输入法。

## Proposal

将本 issue 作为 Draft PR 跟踪合成复现。需要在同一应用复制合成文本，再分别在切换水杉前后打开 Spotlight 剪贴板历史，并记录其他应用条目是否也消失、水杉本地剪贴板历史开关和系统版本。确认输入法存在写入路径后，在本 PR 加入修复和回归；若监听保持只读，则保留调查结论。

## Alternatives considered

- 修改公共剪贴板监听或输入源切换清空逻辑：当前没有写入证据，可能破坏系统剪贴板和 Spotlight 历史。

## Acceptance criteria

- 复现时确认 Spotlight 历史是否只缺水杉条目或整体为空。
- 确认输入法是否调用系统剪贴板写入/清空 API。
- 有产品责任证据后再加入修复，否则保持 Draft。

## Risks

未确认写入者时修改监听可能影响所有 macOS 应用的剪贴板历史。
