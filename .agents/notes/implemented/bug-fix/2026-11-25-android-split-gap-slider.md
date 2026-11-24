# Agent Note: Android 分离键盘的「中间空隙」滑杆真正生效

Status: implemented

## Problem

#11812 报告安卓平板开了分离键盘后无法调节两半之间的间距。设置页有这个滑杆（`platform.android.split_keyboard_gap`，10–40%、步进 5%、默认 25%，`KeyboardOptionsPage`），但改完键盘看起来不变，原因有两处：

- **改比例不重建键行。** `ImeLetterRows.splitStale()` 只比较「该不该分离」和分离宽度，空隙比例不在键行签名里；`render()` 因此不会因为滑杆变化重建键行，要等旋转、切页或其他重建发生才按新值重画。
- **宽窗口上滑杆的上限被 720 dp 键区下限吃掉。** `SplitKeyboardPolicy.gapFraction` 取 `max(用户选的, 1 − 720/可用宽度)`。可用宽度 1200 dp 时下限正好是 40%（等于滑杆上限），再宽的窗口下限是 45%、50%，整段滑杆都落在下限以下，怎么拖都是同一个空隙。报告者的平板截图里两半之间约 38–40%，正是这条下限画出来的。

## Decision

两处一起修。

1. `splitStale()` 除了分离状态和宽度，还比较建行时的空隙比例，以及当时是不是用户显式设过（`builtSplitGapPercent` / `builtSplitGapExplicit`）。设置页保存后键盘下次弹出就按新比例重画，不用旋转。
2. `SplitKeyboardPolicy.gapFraction(width, percent, explicit)` 分两种情形：
   - **用户设过这一项**（`platform.android.split_keyboard_gap` 在 `android-settings.json` 里显式存在，`AndroidLocalSettings.Snapshot.has`）时，以用户选的比例为准，720 dp 键区上限不再抬高空隙，很宽的窗口上键区可以超过 720 dp。滑杆因此与画出来的空隙一一对应。
   - **用户没设过时**，沿用原规则：默认 25% 之外再保证两侧键区合计不超过 720 dp。

于是 720 dp 键区上限从硬规则变成「默认布局规则」：没碰过滑杆的用户手感不变，碰过的用户拿到自己选的比例。

空隙这一项只留在本机、不进账号同步（`ANDROID_LOCAL_SETTINGS` 里只有主开关），所以 `has()` 就是这台设备上「用户真的设过」的准确信号；设置页「恢复默认」把这一项写回 null、键被删掉，720 dp 上限随之回来。

## Related notes audit

[分离键盘启用条件与中间空隙可调](../feature/2026-10-27-android-split-keyboard-controls.md) 就是加这两个滑杆那篇，它的「仍保留当前屏幕宽度为键区提供的最低空隙」现在只对没设过的窗口成立，这篇收窄了它的适用范围；它的 Consequences 里「设置页保存后输入法重新布局」当时只在阈值的路径上成立，空隙这条由本篇补上。[平板分离键盘按窗口限制键区宽度](../bug-fix/2026-10-26-android-tablet-split-width.md) 的 720 dp 规则保留为默认值规则，与这篇互链。[手机横屏分离](../feature/2026-11-01-android-phone-landscape-split.md) 用同一个 `gapFraction` 估算键宽，估算仍走默认规则（显式设了更小空隙时只会低估键宽，宁可多退回整块键盘），实际画出来的空隙按用户的值。检索 proposed、implemented、rejected 没有发现其他涉及空隙滑杆的笔记。

## Alternatives considered

- **总是按用户选的比例画（无条件去掉 720 dp 下限）**：最简单，也是谷歌输入法的做法。不采用：没碰过滑杆的宽窗口用户会一起从 40% 以上掉回 25%，键区重新被拉宽，等于静默改掉 2026-10-26 那篇为宽屏调好的默认手感。
- **把滑杆上限提到键区下限之上**：滑杆仍然「能动」，但 10–40%、步进 5 这套取值 iOS 和鸿蒙的设置文档里也一样，为宽屏单独放宽会产生跨平台不同的同一个百分比。
- **只修重建、保留硬下限**：1200 dp 及以上的窗口滑杆依旧完全无效，正是这次报告的情形，不算修好。

## Consequences

- 键行重建这条路径没有自动断言：`splitStale()` 在依赖 Android 的 `ImeLetterRows` 里，JVM 冒烟测不到；策略计算由 `SplitKeyboardPolicySmoke` 新增的 `explicitGapOnWideScreens` 覆盖（各宽度下显式值生效、没设过时保留 720 dp 键区），设置页改完滑杆的实际重画由报告者复验，回归由 `android-device-pr` / 完整设备套件看守。
- 鸿蒙的 `SplitKeyboardPolicy.ts` 有同一条下限，那边设置页的滑杆在很宽的窗口上同样无效。这次只改安卓（#11812 是安卓的报告），鸿蒙若跟进需另开 issue 并按那边的产品决定处理。
- 数字层分离（#11812 的诉求 2，切到数字九宫格后回落到 720 dp 居中）这次不做，留在 issue 里等维护者选定方向。
