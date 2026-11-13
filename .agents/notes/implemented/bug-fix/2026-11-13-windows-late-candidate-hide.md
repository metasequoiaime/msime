# Agent Note: Windows 延迟候选隐藏不取消组字

Status: implemented

## Problem

Windows 宿主在方向键翻候选时，候选视图的确认和 `HideCandidateWnd` 可能因输入队列拥塞交错。旧路径先在 `FocusedSession::hide_candidate` 里取消当前组字，再把事件标成 `internal_late_event` 交给候选信箱做 24 毫秒宽限。于是过期的隐藏通知会清空 Engine 会话，候选窗即使短暂保留也无法继续点选或输入。

## Decision

`SessionPump` 在处理事件任务开始时先根据队列等待时间判定延迟隐藏，并把这个事实传给 `InputState::hide_candidate`。`FocusedSession` 对仍有 `editing_text` 的延迟隐藏返回独立的 `Late` disposition，保留已确认的组字；候选信箱继续按原有 `internal_late_event` 规则延迟收起界面。正常隐藏仍立即取消组字，已结束的会话和上屏后联想沿用原有分支。

回归测试在 Windows Server 冒烟中直接验证：延迟隐藏保持合成文本，普通隐藏清空合成文本。

## Alternatives considered

- **只延长候选信箱的 24 毫秒宽限**：不能恢复已被 `FocusedSession` 取消的 Engine 会话，候选仍然无法点选，因此放弃。
- **所有隐藏都不取消组字**：会破坏焦点转移、Esc 和宿主正常收起候选的语义；只对已标记的延迟隐藏且仍在组字时保留会话。

## Consequences

延迟到达的旧宿主隐藏不再让方向键后的候选会话消失，候选窗仍可按现有宽限逻辑等待后续视图。队列等待时间现在在调用 `hide_candidate` 前计算，新增参数只影响候选隐藏路径；正常隐藏、预测联想和续接上屏的处理保持不变。Windows Server 冒烟增加一个真实 `FocusedSession` 行为断言；仍需在报告者的 Windows 11 预览版和真实编辑器中复验视频中的 Up/Down 与点击路径。
