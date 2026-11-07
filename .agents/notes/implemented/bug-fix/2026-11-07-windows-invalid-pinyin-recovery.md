# Agent Note: Windows 无法切分的全拼回车后恢复组字

Status: implemented

## Problem

Windows 输入 `wi` 这类无法切分成音节的全拼后，旧版本曾出现输入法不再响应后续按键、必须切换输入法才能恢复的报告（#11035）。引擎对无候选的全拼会保留原文字母；Windows 宿主的回车提交路径必须使用同一段原文完成提交并清空会话。

## Decision

在 Engine 会话回归中固定以下契约：无效全拼仍显示为可编辑原文；`CommitRaw` 提交原文并清空编辑状态；下一次输入可以重新组字。Windows `session_smoke` 通过真实 `ServerSession` 和 `ReplyComposer` 补同一条宿主路径，使用合成的 `wi`、回车和 `ni` 数据验证原文提交、会话清空及后续恢复。产品逻辑没有为单个字母串增加特殊分支，避免把无效拼音变成不可编辑状态。

## Existing notes audit

按 `wi`、无效全拼、`CommitRaw`、原文回车和宿主会话检索活跃笔记。已有全拼候选与原文提交笔记分别持有 Engine 命令和 Windows Enter 观测的通用契约；本篇只补充无法切分全拼在真实 Windows 会话中的端到端回归，不改变其它 scheme 的提交规则。

## Alternatives considered

- **在 Windows 宿主里为 `wi` 增加专门恢复分支**：可以针对报告字符串绕过现象，但会让宿主和 Engine 的编辑状态分叉，也无法覆盖其他无法切分的字母串，因此不采用。
- **只增加 Engine 单测**：能证明核心命令可恢复，却不能验证 Windows `ReplyComposer` 的原文提交和确认送达路径，因此同时保留宿主回归。

## Verification

- `rbuild cargo test -p msime-engine session::tests::invalid_pinyin::invalid_quanpin_input_stays_editable_and_commits_raw -- --exact --nocapture` 通过。
- `rbuild bash scripts/verify-local.sh --quick` 通过；其中 Windows 本机可移植测试 191 项通过，Rust workspace、Android host Java、共享 Apple bridge、Windows contracts 和 notes 校验通过。Windows MinGW 原生编译在远端环境未配置，交给 PR 的 Windows 构建 job 编译并运行 `windows-session-smoke`。
- 未改变界面、资源或文件读写，不需要截图或设备验收。

## Consequences

当前实现继续由 Engine 的通用原文提交规则处理无效全拼；新增回归会在 Windows 宿主路径被改坏时阻止合入。无法在本机远端环境执行 Windows 真机/原生宿主安装验收，PR 保持 draft，待 Windows CI 完成后再决定是否关闭 issue。
