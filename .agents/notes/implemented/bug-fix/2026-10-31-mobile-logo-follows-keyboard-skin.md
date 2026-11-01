# Agent Note: 手机键盘品牌键跟随当前皮肤

Status: implemented

## Problem

Android、iOS 和鸿蒙键盘的品牌键使用应用主题的季节强调色和卡片底色。用户选择具名或自定义键盘皮肤后，标志仍是季节色，与周围键盘不协调。

## Decision

品牌键的标志使用当前皮肤强调色的 82% 与黑色混合，圆盘使用当前皮肤强调色的 14%（浅色）或 22%（深色）与键盘背景混合。已有的品牌键预览与真实键盘复用同一配色；iOS 原生皮肤保留其已有系统 token，仍取自身柔和水杉绿。应用内的品牌标识、皮肤选择和默认值不变。

本决定部分取代 [iOS 与鸿蒙采用设计稿](../../implemented/feature/2026-10-07-ios-harmony-all-platform-design.md) 中品牌键统一取季节色的规则；[默认跟随系统皮肤](../../implemented/feature/2026-10-08-touch-keyboard-default-follows-app-theme.md) 的默认值与季节背景规则保持。

历史检索：设计稿笔记部分重叠，互链记录本次替代；[Android 皮肤重读重画循环](2026-10-11-android-skin-reload-rerender-loop.md) 把品牌键季节色列为 `displayKey` 拼入种子的理由之一，[鸿蒙键盘设计的功能键](2026-10-08-harmony-design-function-keys.md) 记录季节给品牌标着色，两篇的品牌键部分被本次取代，已就地改写并互链，其余决定保留（种子仍因回车键和开启态底色留在 `displayKey` 里）。默认跟随系统和 iOS 原生皮肤笔记属于保留的规则，不改其决定。

## Alternatives considered

- 保留季节配色能让品牌键和设置应用标识一致。但键盘主要出现在其他应用里，用户所选皮肤才是周围颜色的来源，统一应用品牌色正是此次突兀的原因。
- 直接用强调色填标志最贴近皮肤原色。但保留现有混黑方式能延续白色笔画与实色外框的层次，只更换颜色来源。

## Consequences

- 具名与自定义皮肤的品牌键不再受应用主题季节影响，皮肤选择、配置格式和出厂默认值不变。
- 跟随系统皮肤的圆盘混合基底也改为键盘背景，因此颜色比旧的应用卡片底色略深。标志的混黑比例和白色笔画保持。
- 鸿蒙 `bash platforms/harmony/tests/run.sh` 通过，覆盖系统、具名与自定义皮肤，含应用主题变化时品牌键保持自身配色。iOS 更新已有皮肤测试，并用 `swiftc -frontend -parse` 检查改动文件；未运行 UIKit 测试或模拟器验收。
- Android 的公式在 `KeyboardSkin.logoDisc` / `logoMark`，`ImeToolbar.styleTopRow` 只调用它们；`KeyboardSkinSmoke` 按鸿蒙同一组期望值断言跟随系统、具名与自定义皮肤的品牌键，含换应用主题时具名与自定义皮肤保持自身配色。Android 宿主检查与仓库契约检查记录在 PR。完整 quick 门禁受磁盘限制：本机约 42 GB、远端构建卷约 13 GB 可用，不新建完整构建树；本次没有设备外观验收。
