# Agent Note: macOS 输入源切换结束 Engine 焦点上下文

Status: implemented

## Problem

#11370 报告 macOS 使用小鹤双拼时，从 ABC/ABC Extended 通过 fn 切回水杉，首个 `fuvu` 会错误触发全拼纠错；取消组字或退格后再次输入才恢复。macOS 的输入源切换通知可能早于或替代 IMK 的 `deactivateServer:`，仅清理外观状态时，Engine 仍可能保留旧焦点上下文。

## Decision

输入源监视器确认离开水杉后，先让当前聚焦控制器调用现有会话的 `setFocused:NO`，通过既有 `apply:` 路径接收清理后的视图，再标记 `_focusPending`。会话对象保留，切回后的首个按键沿现有 `prepareSession`/`setFocused:YES` 路径重新建立焦点上下文。原始组字提交仍由 #11326 的钩子负责，焦点重置紧随其后执行。

该入口只操作 `MSIMEFocusedController`、活动客户端和活动会话；缺少任一对象或 Engine 返回失败时不改变状态。行为测试验证 `focus:0`、视图失焦和 `_focusPending` 标记；与 #11326 集成后额外断言 `MSIME_COMMIT_RAW` 发生在 `focus:0` 之前、编辑器只收到一次原文，避免两个修复互相吞字。测试不使用源码字符串守卫。

## Alternatives considered

- **只在 `deactivateServer:` 中依赖现有失焦逻辑**：代码改动更小，但无法覆盖报告中可能只送输入源通知、未完整调用 IMK deactivate 的 macOS 时序。
- **在首个 `fuvu` 前按方案强制切换或关闭全拼纠错**：不能证明纠错状态就是根因，会改变双拼用户的正常候选行为，因此不采用。
- **销毁并重建整个 `MSIMEClientSession`**：能清理更多状态，但会增加词库会话和资源重建成本；现有 `setFocused:NO/YES` 已定义跨客户端上下文清理，复用该契约即可。

## Consequences

输入源切换多一次同步的 Engine focus-out；切回首键会按既有 focus-in 路径清空旧上下文，避免旧纠错上下文影响双拼。会话和资源保持复用，未改变默认方案、纠错规则或用户设置。仍需在报告者 macOS 27.0.1、ABC Extended、真实第三方编辑器和 fn 切换路径复验；本地 CI 只能证明宿主回归和编译行为，尚未复现报告中的 `fuvu` 纠错结果，不能把焦点重置当作已证实的完整根因。
