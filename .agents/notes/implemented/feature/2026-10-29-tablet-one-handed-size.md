# Agent Note: 大屏上的单手模式缩到手机大小并贴到屏幕边

Status: implemented

## Problem

#10202：用户反馈平板上开单手模式，键盘还是太大，单手够不着。

Android 和鸿蒙的单手模式不分手机和平板，一律是「键区 85%、侧栏 15%」：

- Android：`ImeFrame.applyOneHanded` 按 `OneHandGutterView.GUTTER_FRACTION` 切分。平板的键盘表面又是居中、最宽 720dp（`KeyboardFormFactorPolicy.EXPANDED_SURFACE_MAX_WIDTH_DP`），单手时键区约 612dp，而且停在屏幕中间，横屏时离屏幕边还有几百 dp。
- 鸿蒙：`KeyboardView.handedKeys()` 的侧栏是 `RAIL_PERCENT = 15`。平板上键盘铺满面板，1280vp 宽时键区约 1088vp。

手机 411dp 宽时 85% 约 349dp，正合适。问题是这个比例没有按屏幕大小区分。

iOS 的单手模式只在 iPhone 上提供（`KeyboardOneHandLayout.effective`，设计稿限定 `mob && !pad`），不受影响。

## Decision

大屏（最短边 ≥ 600dp/vp，Android 的 `KeyboardFormFactorPolicy.expanded`，鸿蒙新加的 `KeyboardFormFactorPolicy.largeScreen` 用同一条界线）上单手模式生效时：

- 「侧栏 + 键区」这一块按键区 360dp/vp 算（Android `ONE_HANDED_KEYS_MAX_WIDTH_DP`、鸿蒙 `OneHandedPolicy.KEYS_MAX_WIDTH_VP`），侧栏仍占这一块的 15%。Android 是 424dp（64 + 360），鸿蒙多一道 6vp 的侧栏间距，是 431vp。窗口窄到放不下时整行都给它。
- 这一块贴到键盘所在的那一侧，另一侧空着，露出键盘底色。
- Android 的键盘表面在大屏单手时铺满窗口（`surfaceWidthDp` 新增 `oneHanded` 参数），和分离式键盘一样，这一块才能贴到屏幕边。单手模式切换时 `applyOneHanded` 顺带重排表面。
- 工具栏、候选条和替换按键的面板保持全宽，沿用两端原来「只有按键变窄」的规则。
- 手机上的行为完全不变：`oneHandedBlockWidthDp` / `blockWidthVp` 返回 0，走原来的按比例切分。

## Alternatives considered

- **把整块键盘表面（含工具栏、候选条）缩到手机大小贴边**：看起来就是一台手机键盘放在旁边，候选也在拇指范围内。不采用：两端都写明了单手模式只缩窄按键，鸿蒙也没有可以收窄的键盘表面，要另做一套面板宽度；而且两端的结构会不一样。
- **加一个单手宽度比例的滑杆**：用户原话里提到「调整比例」。暂不做：各平台刚精简过设置项，先看固定在手机大小够不够；不够再加。
- **左边数字键盘 / 文本编辑面板、右边九键的单手布局**：用户的第二个建议，是一种新的平板布局，和分离式、浮动键盘的分工要先定，单独记在 #10203。

## Consequences

平板上单手模式空出来的一侧是一大块键盘底色，没有按键。#10203 在这块地方放数字键盘，iPad 也补上单手模式，见[大屏单手模式空出来的一侧放数字键盘](2026-10-29-tablet-one-hand-side-number-pad.md)。

## Verification

- Android：`platforms/android/check-host.sh` 通过，`KeyboardFormFactorPolicySmoke` 覆盖平板 424dp（360 + 64）、手机返回 0、窗口窄于这一块、宽度未知，以及大屏单手时表面铺满、关掉时回到 720dp。
- 鸿蒙：`platforms/harmony/tests/run.sh` 通过，新增一组覆盖 `largeScreen` 的界线和两个方向、手机返回 0、平板 431vp 且键区约 360vp；`hvigorw assembleHap` 编译 ArkTS。
- 没有在平板模拟器或真机上看过实际效果。
