# Agent Note: Android 14/18 键字母键按下画气泡

Status: implemented

## Problem

「按键弹出」（`AndroidLocalSettings.KEY_POPUP`）默认开着，Android 26 键和九键（#9349 修 #9338 之后）的字母键按下都画气泡。14/18 键却只有 18 键双拼的两字母键画：那是 #9593 横滑精确字母顺带加的，横滑要让用户看到选中的字母。14 键、18 键全拼和 18 键的单字母键按下都不画，同一排键有的有气泡有的没有。[全拼 14 键](2026-10-10-fourteen-key.md) 的规格写着「按键气泡：不画，同九键」，九键后来画了气泡，这一行在 Android 上已经和它的理由对不上。

## Decision

`ImeLetterRows.bindGridLetterGestures` 在 ACTION_DOWN 对每个 14/18 键字母键调 `updateKeyPreview`，不再只给 18 键双拼的两字母键。气泡的显示条件和 26 键、九键完全相同：「按键弹出」关掉时不画，密码框不画，松手和取消收起本键的气泡。气泡的字是键面（`QW`、双拼的 `L;`、18 键的 `Q`）。

- 角标滑动和 18 键双拼的横滑沿用 `gridGesturePreview`：按下时已经有气泡，触发后只换字；没有气泡（「按键弹出」关着）时也不因手势画出一个，与原来相同。
- 只改 Android。iOS 在三端对照时去掉了 14 键的按键气泡（见全拼 14 键的「对齐」一段），鸿蒙按同一份规格实现；两端要不要跟进另行决定。

## Alternatives considered

- **维持现状，只修规格文字**：理由是三端在 2026-10 对照时按多数一方定成「不画」，iOS 为此去掉了气泡，Android 再画就和另外两端不一样。但那次对齐的参照是「同九键」，Android 九键之后已经画气泡；而 Android 18 键双拼的两字母键一直在画，现状本身就和规格不一致。在同一个 Android 键盘上让 26 键、九键、18 键双拼画、其余 14/18 键不画，比跨平台的差异更难解释。
- **只给 18 键全拼和单字母键补上，14 键不画**：没有可说的理由，14 键的键更宽，按下时手指遮住的反而更多。

## Consequences

- Android 上所有字母键按「按键弹出」一致地画气泡；「按键弹出」默认开，所以 14 键、18 键全拼和 18 键单字母键的用户会看到原来没有的气泡，关掉设置即可回到原样。
- Android 与 iOS、鸿蒙的 14/18 键在这一点上不同，[全拼 14 键](2026-10-10-fourteen-key.md) 的规格表里注明 Android 由本篇取代；[Android 14/18 键角标符号](2026-11-07-android-fourteen-eighteen-key-corner-symbols.md) 里「按下时的气泡不变」一条改为指向本篇。
- 14 键两字母键的权重是 2，`KeyboardKeyPreview.bubbleWidth` 按宽键取 150% 的键宽，气泡比 18 键的宽。
- 设备验收：`OptionSheetDeviceSmoke -e key-feedback true`（`KeyboardFeedbackDeviceChecks`）在合成的输入法服务上重建 14 键和 18 键全拼的键行，核对两字母键和单字母键按下出现写着键面的气泡、取消后收起、关掉「按键弹出」时不画。这条套件不在 `smoke.sh` 里，要单独跑。
