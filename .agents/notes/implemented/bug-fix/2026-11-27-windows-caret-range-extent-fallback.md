# Agent Note: 组字范围量不到坐标时改用光标范围

Status: implemented

## Problem

#11787（前一次报告 #11483）：Windows 11 + TSF，Firefox 系宿主（报告者用 `Iceweasel.exe`）在 Nodeseek 评论框里输入，候选窗经常整段不出现，按一次退格再输入又回来。`#11876` 修过一次：同一次组字里已经量到过可用锚点时，`TS_E_NOLAYOUT` 和「`isClipped=TRUE` + 空矩形」都不再清掉锚点。报告者用带那次改动的构建复验，仍然不出现，并按提示打开了 `[candidate-layout]` 诊断。

日志里 18 段组字，**每一段在「组字开始」那次测量都没量到坐标**：

```
[candidate-layout] extent query failed hr=0x80040206 has_range=1 anchor_valid=0
[candidate-layout] composition start anchor=0,-100000 usable=0
[candidate-layout] per-key empty clipped extent, no usable anchor yet, hid rect=0,0,0,0
```

`0x80040206` 是 `TS_E_NOLAYOUT`。这 18 段里 11 段从头到尾一个有效锚点都没有（`payload` 一直是 `flag=63 anchor=0,-100000`，候选窗整段在屏幕外），7 段在第二键或更晚才碰巧量到。`#11876` 的保留分支只对「这一段已经量到过锚点」生效，组字开始时无锚点可留，所以覆盖不到失败点；随后每一次按键测量也答空矩形，窗口于是整段不出现。

## Decision

`platforms/windows/tsf/Tf/TfTextLayoutSink.cpp` 的 `_GetTextExt` 在「组字范围量失败或量到空矩形、而且这次 layout 会话还没有可用锚点」时，再按**光标范围**量一次：从 `ITfContext::GetStart` 取一个范围，`ShiftEndToRange(ec, _pRangeComposition, TF_ANCHOR_END)` 把终点移到组字末尾，再 `Collapse(TF_ANCHOR_END)` 成空范围，用同一个 `ITfContextView::GetTextExt` 取矩形。量到（只要求 `bottom > top`，光标范围宽度为 0 是正常形状）就把它的左下角当锚点并标记 `_hasValidAnchor = true`，后面的按键测量会照 `#11876` 的路径留在这个位置上；量不到才退回 `Global::INVALID_Y` 的哨兵值，行为与改动前一致。

这里**不用 `ITfRange::Clone`**：Windows SDK 的签名是 `Clone(TfEditCookie ec, ITfRange **ppClone)`，MinGW 的 `msctf.h` 把它声明成单参数的 `Clone(ITfRange **ppClone)`，照 MinGW 的声明调用会把 vtable 上的编辑 cookie 参数错位。`GetStart` 和 `ShiftEndToRange` 两边签名一致，这条路径在 MSVC 与 MinGW 下都是同一份代码。`Windows GNU cross build` 第一次就红在这个签名差异上。

诊断加一行 `[candidate-layout] caret range anchor=X,Y after <原因>`，把「组字范围失败、光标范围成功」与「两条都失败」分开。

依据是日志里的两件事：整段组字没有排版，但光标就在可见的输入框里；宿主不给整段组字的矩形，不等于不给光标处的矩形。这比沿用上一段组字的旧锚点安全——旧锚点可能来自另一个文档或另一个滚动位置，而光标范围是这次组字自己的位置。

## Alternatives considered

- **组字开始量不到就沿用上一段组字的锚点（跨 layout 会话保留 `Global::Point`）。** 最强理由是不用再向宿主测量，日志里上一段组字确实已经有 `465,718` 这样的坐标。但 `#11876` 明确否掉过跨会话沿用：窗口会出现在另一个文档或上一次滚动位置的光标处，错误位置比不显示更难排查，而且会盖掉「用户点了另一个输入框」这个事实。
- **只测光标范围，不测组字范围。** 会把候选窗固定成光标位置的下沿，失去「整段组字首行」的语义（多行 textarea 里组字换行时位置会差一行）。
- **等 `TF_LC_CHANGE` 再来一次真实测量。** 日志里这些组字连每次按键的测量都是空矩形，宿主在这段时间里不会再给出可用矩形，等不到。
- **只在 `TS_E_NOLAYOUT` 时回退。** 不够：这 18 段里随后按键答的是「`isClipped` + 空矩形」，两条失败路径都要走回退。

## Consequences

- 组字开始时多一次 `GetTextExt`（取范围 + 移终点 + 折叠 + 测量），只在组字范围量不到坐标时发生；正常宿主一次都不多测。
- 光标范围的矩形在某些宿主里可能比组字首行高一两像素，候选窗位置会有极小差别；与整段不显示相比可以接受。
- 本机是 macOS，没有 Windows 宿主可复现：编译与运行由 PR 上的 `Windows TSF system WER` 负责，行为真机验收仍要报告者在含本改动的 nightly 上复验（诊断会直接写出走的是哪条分支）。

## Testing

- 编译：本机用 `x86_64-w64-mingw32-g++ -std=c++17 -fsyntax-only` 按 TSF 目标的 include 与 `MSIME_EDITION_FULL` 编过这个翻译单元（MinGW 的 `msctf.h` 正是暴露 `Clone` 签名差异的那份头）；MSVC 一侧由 PR 的 `Windows TSF system WER` 作业（MSBuild）编，完整交叉编译由 `Windows GNU cross build` 作业覆盖。
- 行为：报告者用含本改动的 nightly 复现一次，把新的 `[candidate-layout]` 行发回来。出现 `caret range anchor=…` 表示走了回退；仍然只有 `extent query failed`/`empty clipped … hid` 表示光标范围也量不到，需要换别的锚点来源。
