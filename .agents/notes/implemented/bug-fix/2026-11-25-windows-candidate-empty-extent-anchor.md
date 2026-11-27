# Agent Note: Windows 候选锚点不再因空 extent 收起候选，并补齐锚点诊断

Status: implemented

## Problem

#11787（同一条线的前一次报告 #11483 已关闭）报告 Windows 11 + TSF 下 Firefox 系的 Nodeseek 评论框里，第一段输入正常，接下来的输入候选窗完全不出现，按一次退格再输入又回来，如此循环；同一页换 Edge 正常。报告者用的是 Iceweasel 160.0.1（Firefox 160 的第三方编译），页面缩放 150%，构建是 nightly `3f651989f0cd`，日志只有 `[issue47]` 按键轨迹、`[deferred-recovery]` 和一行 `[game]`，没有任何锚点或隐藏记录。

代码里能写出屏幕外哨兵值（`Global::INVALID_Y`）的位置有三处，其中两处是按键期的测量：

- `tsf/Edit/GetTextExtentEditSession.cpp` 的按键测量在「被裁掉且矩形为空」时无条件写 `INVALID_Y`，不管这次组字是不是已经量到过可用锚点；
- `tsf/Tf/TfTextLayoutSink.cpp` 的 `_GetTextExt` 同样只对 `TS_E_NOLAYOUT` 做了「已有有效锚点就沿用」的豁免，空 extent 那一支还是打哨兵值。

Server 收到哨兵值就收起候选（`src/candidate/CandidateWindow.cpp` 的 `policy_hide`），要等一次带着真实锚点的 Move 才回来，而 Move 只在候选 UI 会话已激活时才发。Firefox 系的 textarea 在内部滚动、换行时会用「`isClipped=TRUE` + 空矩形」而不是 `TS_E_NOLAYOUT` 回答这一次测量，于是一次空 extent 就把候选收起来，后面的按键继续量到空 extent 就继续收起，直到某次测量碰巧成功（退格让文字缩回可见区正好会造成这种成功）才恢复。

证据只到这一步：哪一支真正收起了报告者的候选窗，现有日志看不出来，本机也没有 Windows 宿主可以复现。

## Decision

两件事一起做，都不改变组字开始时的既有语义。

1. 空 extent 不再清掉已经量到的锚点。两个按键测量位置都按 `_hasValidAnchor` 分流：这次 layout 会话（同一次组字）已经量到过可用锚点，就保留 `Global::Point` 不动（`_GetTextExt` 返回失败，与上面 `TS_E_NOLAYOUT` 一支的写法一致）；还没有量到过可用锚点时照旧写哨兵值，组字开始仍然可以先收起，不沿用另一个文档里的旧位置。
2. 补齐 `[candidate-layout]` 诊断。带 `Global::TsfDiagnosticLogEnabled` 开关、不含任何按键内容，每处都只在开关打开时写：
   - 组字开始时的测量结果（锚点与是否可用）与失败码；
   - 每个按键的空 extent（保留还是收起，附原始矩形）与测量失败（错误码与矩形）；
   - `_GetTextExt` 的失败分支与空 extent 分支（保留还是收起）；
   - 每一次 Show/Move 真正报出去的锚点、写入标志、是否在组字、候选 UI 会话是否已激活；
   - 被丢掉的布局通知与 Move，以及丢掉的原因（异步清理中、会话未激活、非显示模式）。

下一次复现的 `server.log` 因此能直接指出候选窗是在哪一步被收起、收起时报了什么锚点。

## Alternatives considered

- **只加诊断、不动行为**：能确定分支，但报告者要再多等一轮，而且这一类「宿主还没排好版」的隐藏路径本身是已知的缺口。
- **跨组字沿用同一个文档的旧锚点**：更可能覆盖「提交之后下一段输入就不显示」这一形态，但 2026-10-13 那篇笔记和 `_GetTextExt` 里的注释都明确定了「组字开始退回哨兵、不沿用另一个文档的旧位置」。走到这一步需要报告者日志确认隐藏发生在组字开始，这次不改。
- **把 `firefox.exe` 这类宿主也加进 `ZEN_BROWSER` 的布局通知节流**：那处节流推迟的是 Move 通知（每三次测量连发一次），而这次的现象是窗口完全收起，不是位置滞后；没有证据支持，故不顺手改。

## Consequences

- 同一次组字里量到空 extent（宿主还没排好版）不再收起候选；持续输入时不会整段输入都看不到候选。
- 代价：组字文字确实滚出可见区时，候选窗会留在上一次量到的位置，而不是干脆隐藏。真正结束组字、失焦、`_LayoutDestroyNotification` 这些路径仍然收起窗口。
- 组字开始量不到坐标时仍按原规则收起，焦点换到另一个输入框时不会先用旧框的位置画一次候选。
- 没有 Windows 宿主，这一条无法在本机验证；TSF 的编译由 PR 上的 `Windows TSF system WER` 负责，行为要报告者在含这个构建的版本里复验，日志里要带上复现前后的 `[candidate-layout]` 行。
- 诊断打开时每键会写若干行，队列满时丢最旧的记录（`tsf/IPC/Ipc.cpp` 的 `QueueTsfDiagnosticLog`），所以复现要短、并且尽量在复现前清空日志。

## Related notes audit

[Windows Firefox 候选焦点按 COM 身份判断](../bug-fix/2026-11-16-windows-firefox-candidate-focus-identity.md) 修的是同一现象的另一条原因（同一文档管理器返回不同 COM 包装被判成失焦），隐藏走的是 `OnKillThreadFocus`；本篇走的是锚点与 `policy_hide`，两条都保留。[Windows 按键投递沿用同一租约里报过的候选窗位置](../bug-fix/2026-10-13-windows-candidate-position-across-keys.md) 定义了按键包里的 `INVALID_Y` 语义和「有了有效 Show/Move 才能回来」的时序，本篇只改谁有资格写哨兵值，不改那套沿用规则。检索 proposed、implemented、rejected，没有其他关于 `GetTextExt` 空 extent 的笔记；`TS_E_NOLAYOUT` 的沿用规则只写在代码注释里，本篇与它放在同一处。
