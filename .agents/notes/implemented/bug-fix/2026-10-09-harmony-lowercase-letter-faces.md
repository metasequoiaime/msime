# Agent Note: 鸿蒙字母键平时画小写，只在下一键真的输入大写时画大写

Status: implemented

## Problem

鸿蒙触屏键盘在中文模式下，26 个字母键一直画大写，Shift 关着也是。原因是 `LetterKeyFacePolicy` 还停在旧规则上：中文模式（本地输入模式除外）固定画大写，英文模式跟着 Shift。

Android 和 iOS 早就按新设计改了：字母键平时画小写，和设计稿 `in_default` 一致。只有鸿蒙还是大写键面，用户看到的是「小写状态下键盘却是大写」。

## Decision

键面画大写，当且仅当 Shift 亮着、并且下一键真的会输入大写：

- 英文：Shift 或大写锁定时画大写。
- 中文：只有组字中的辅助码会以大写交给 Engine（`ChineseHelpcodePolicy.entersHelpcode`），这时画大写。没有组字时，Shift 加字母仍然输入小写拼音，键面保持小写。
- 中文的本地输入模式：键面固定小写，Shift 也不变。

做法上，`LetterKeyFacePolicy.displaysUppercase` 改成和 Android 一样的 `shifted && !(chineseMode && localMode)`，读屏文字也跟着键面走。「下一键是否真的输入大写」由 `KeyboardView.typesUppercase` 算出来再传进去，判断方式和 `tapLetter` 发键时一致。它读的都是视图的 `@State`（`english`、`editing`、`engineScheme`、`localMode`），所以组字开始或结束时键面会跟着重绘。韩语、越南语、藏文的分支不变。

## Alternatives considered

- **照搬 Android：Shift 亮着就画大写，中文也一样。** 最强的理由是两端规则逐字一致。不用它，是因为鸿蒙中文没有组字时，Shift 加字母送的是小写拼音（`tapLetter`），这时画大写是在骗人。iOS 用的就是「下一键真的输入大写才画大写」，鸿蒙的发键逻辑和 iOS 相同，所以跟 iOS。
- **在 `LetterKeyFacePolicy` 里直接判断组字状态。** 规则可以集中在一处。不用它，是因为策略是纯函数、和 Android 的签名一一对应；组字状态属于视图，放在视图里算，策略只回答「给定是否输入大写，键面怎么画」。

## Consequences

- **收益**：鸿蒙字母键与设计稿、Android、iOS 一致，平时小写；辅助码 Shift 时键面和读屏都如实显示大写。
- **代价**：只在模拟器上验证过中文模式的三种情况（平时、无组字时按 Shift、组字中按 Shift）。英文模式和本地输入模式只有单元测试覆盖。
