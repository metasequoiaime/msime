# Agent Note: Android 空格复制提示使用键盘内浮层

Status: rejected — 键盘内的位置不符合 #11112 标注的屏幕顶部区域

## Problem

复制失败通知占用候选区诊断行，工具栏高度短暂变化。初稿把手势中的键面标签与失败后的诊断通知视作同一条提示路径。

## Proposal

将两者都移到 `keyboardSurface` 顶边的浮层，靠独立子视图避免测量变化。

## Alternatives considered

独立的屏幕顶部提示窗口需要兼容旧版 Android；键盘内浮层实现更简单，也不需要创建子窗口。但 issue 绿色标注位于应用内容上方，键盘内浮层仍覆盖工具栏，因此这个方案没有实现要求。最终采用[屏幕顶部失败提示](../../implemented/bug-fix/2026-11-10-android-space-copy-failure-phone-top.md)。

## Acceptance criteria

失败提示位于屏幕顶部中央，工具栏可见且键盘边界稳定；键盘内浮层不满足前一项。

## Risks

只验证高度不变会漏掉位置错误。验收须先读报告附件，并验证提示在键盘之外的屏幕顶部区域。
