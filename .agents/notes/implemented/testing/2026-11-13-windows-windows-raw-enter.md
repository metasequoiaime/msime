# Agent Note: Windows 无候选英文输入回车回归覆盖

Status: implemented

## Problem

Issue #11357 报告 Windows 11 预览版 0.4.1 输入 `windows` 没有候选，按 Enter 也不上屏。现有 develop 已通过 #11046 修复无效全拼的本地回车路径，但回归测试只覆盖了 `wi`，没有锁定报告者实际输入的完整字符串。

## Decision

在 `platforms/windows/tests/runtime/session_smoke.cpp` 的真实 `ServerSession` 与 `ReplyComposer::configured_key` 场景中新增 `windows` 回归用例。用例先清空前一段组字，再逐键输入 `windows`，确认视图没有候选且仍保留原文；随后发送带完整原文观察值的 Enter，确认本地提交文本为 `windows`、不产生编码回复，并清空 Engine 组字。

该改动只扩大测试输入覆盖面，不改变 Enter 路由、候选策略或原文学习行为。由于 #11035 已由 #11046 合入 develop，本 issue 的 PR 以测试防回归为目的，不能把它描述成新的生产修复。

## Alternatives considered

- **修改生产代码重新处理 `windows`**：报告版本早于 #11046，当前 develop 的 `CommitRaw` 路径已能原样提交并清空组字；没有新的失败证据，改动会把已修复问题误当成现行缺陷。
- **只保留已有 `wi` 用例**：可以验证同一分支的通用行为，但无法防止报告字符串在分词、候选为空判断或观察值校验上出现特例回归，因此补充完整输入。

## Consequences

回归测试会覆盖报告者给出的英文串，并在 Windows 原生测试中固定无候选 Enter 的行为。测试本身不提供 Windows 11 预览版实机验收，PR 验证段仍需明确记录本地或 CI 的原生测试结果及报告者复验状态。
