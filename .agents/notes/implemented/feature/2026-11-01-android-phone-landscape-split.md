# Agent Note: Android 手机横屏分离式键盘用独立本机开关，按键盘实际宽度判断

Status: implemented

## Problem

Android 的分离式键盘只在大屏（`smallestScreenWidthDp` ≥ 分离启用宽度）上生效，普通手机横屏既不会分，设置页也没有入口（#10440）。iPhone 横屏已经支持（[iPhone 横屏与鸿蒙大屏的分离式键盘](2026-10-30-split-keyboard-iphone-harmony.md)），而 [iPhone 的宽度门槛](../bug-fix/2026-10-30-iphone-split-keyboard-view-width.md) 说明按屏幕宽度判断在真机上会算错：系统留白让键盘实际宽度远小于屏幕。

要定三件事：开关放哪、什么时候分、宽度从哪里取。

## Decision

- **独立的本机开关**：新增本地设置 `platform.android.split_keyboard_phone_landscape`，布尔，默认关，不同步（不进 `crates/client-services` 的 `ANDROID_LOCAL_SETTINGS`）。设置页只在 `!showLargeScreenOptions` 时显示一行「手机横屏分离式键盘」；大屏那组分离设置照旧只在大屏显示。
- **手机分支**（`SplitKeyboardPolicy.phoneActive`）：开关打开、横屏、`smallestScreenWidthDp` 低于分离启用宽度、分开后第一排字母键的可见宽度不小于 36 dp。键宽 = 可用宽度 ×（1 − 中缝比例）/ 10 − 按键间距，中缝比例沿用 `gapFraction(width, gapPercent)`，与实际建行时用的中缝一致。36 dp 与 iPhone 的 36 pt 相同：不比手机竖屏的字母键窄。默认设置下 560 dp 是边界，常见手机横屏（扣掉侧边导航栏后 750 dp 以上）都分。
- **宽度取键盘列的实际宽度**：`MSIMEInputService` 在键盘列的布局回调里记下「宽度 − 左右内边距」（内边距就是系统栏 insets，分屏、侧边导航栏都已算进去）和当时的 `screenWidthDp`。判断时只有配置没变才用它，否则（刚旋转、还没按新配置布局）先用 `screenWidthDp` 估；布局完宽度变了、键行又与新判断不一致时，`post` 一次 `render()` 重建键行。只在手机开关打开时这样重画，没开的用户行为不变。
- **外框不变**：`KeyboardFormFactorPolicy.surfaceWidthDp` 在分离时和非大屏上都返回 0（铺满），所以手机分离不会套上大屏的 720 dp 居中外框，`ImeStyler` 不用改逻辑。断行、重复 G/V、拆空格、死区、与单手模式互斥全部复用大屏实现。

## Alternatives considered

- **复用大屏主开关 `platform.android.split_keyboard`**：只放宽 `active` 的屏幕条件。没有采用：这个开关随账号同步，手机会继承平板上的开启状态，用户在平板上打开后手机横屏也突然分开（issue 里明确提到）。
- **按 `screenWidthDp` 判断**：省掉布局回调和重画。没有采用：targetSdk 36 下 Android 15+ 的 `screenWidthDp` 包含系统栏，旧系统不包含，口径随系统版本变；侧边导航栏和刘海留白也不在里面。iPhone 已经因为按屏幕宽度判断在真机上完全不生效，这里不重犯。
- **在布局回调里直接重建键行**：没有采用，布局过程中改视图树会触发新的布局请求；排到下一帧更稳，宽度不再变化时第二次回调直接返回，不会循环。
- **为手机提供竖屏分离、阈值和空隙滑块**：没有采用。手机竖屏分开后键宽不到 20 dp；阈值和空隙对手机没有调的必要，保持设置页简短。空隙仍读同一个本机值（默认 25%）。

## Consequences

- **收益**：普通 Android 手机横屏可以双手拇指输入，默认关，现有用户不受影响。
- **代价**：开关打开时，旋转后可能先画一次完整键盘、布局完再重建成分离式（或反过来），多一次键行重建；不动 Engine session 和当前组合。
- **单手模式**：分离画着时单手模式不生效，与大屏一致。
- **未在真机验证**：没有可用的设备和模拟器，侧边导航栏、手势导航、刘海、分屏下的实际宽度和旋转后的重画都还需要真机横屏验收。
- **测试**：`SplitKeyboardPolicySmoke.phoneLandscape` 覆盖默认关、竖屏不分、560/559 dp 边界、被压窄的键盘不分、键距与空隙的影响、大屏不走手机分支、只分 26 键和韩文、手机分离铺满外框；`AndroidLocalSettingsSmoke` 锁住默认关和不同步。
