# Agent Note: Android 字母键「滑动距离」

Status: implemented

## Problem

用户反馈：14 键、单手模式、默认键盘高度，快速打字时常误出角标符号，问能不能调灵敏度。

Android 26 键和 14/18 键字母键的纵滑判定是固定的 14 dp（`SwipeHintPolicy.THRESHOLD_DP`，约 2 mm），没有时间或速度条件，也不随键大小、键盘高度变化；能调的只有「滑动输入符号」开关和「滑动方向」。组字中纵滑按点按处理，所以误触落在每个词的第一键：上屏后、标点后、联想显示时。14/18 键从 [#11116](2026-11-07-android-fourteen-eighteen-key-corner-symbols.md) 起才有纵滑，[三端规则](2026-11-07-fourteen-eighteen-key-corner-symbols.md) 定的是「触发方式跟随本端的 26 键，不新增设置」，老用户更新后点按时手指稍有上移就出符号。九键遇到过同样的问题，所以默认 24 dp 并有滑块（[九键数字键盘](2026-10-08-android-nine-key-number-pad.md)，现为 [九键四向 flick](2026-11-11-nine-key-four-way-flick.md) 的 `touch_nine_key_flick_threshold`）。#9256 当时也要过 26 键的滑动距离，只做了候选态禁用（#9319）。

单手模式只把键区收窄到 85%，行高不变；纵滑判定不读键宽，所以代码上单手模式不改变阈值。

## Decision

- 新增 Android 本机设置「滑动距离」`platform.android.swipe_symbols_distance`：14–48 dp、步进 2、**默认 14**，在「键盘 → 手势」的「滑动方向」下面，「滑动输入符号」关闭时置灰。常量在 `SwipeHintPolicy`（`MIN_THRESHOLD_DP`、`DEFAULT_THRESHOLD_DP`、`MAX_THRESHOLD_DP`、`THRESHOLD_STEP_DP`），校验在 `AndroidLocalSettings` 的整数规格里，越界或不合步进的值读成默认。
- 26 键（`ImeLetterRows.bindLetterGestures`）和 14/18 键（`GridKeyGesturePolicy`，按下时经 `Gesture.start` 传入）共用这个距离，沿设定方向出角标、反方向打开选项都按它判。14/18 键「跟随本端 26 键」的规则不变，只是 26 键的距离现在可调。
- 默认值不变：调到最小就是加这项设置之前的手感。改出厂值要维护者授权（AGENTS.md），这里只加设置。
- 只在本机，不随账号同步：服务端字段表还没有这个键，上传前按字段表过滤会去掉它（[字段表新增的共享偏好](../architecture/2026-10-27-settings-sync-new-shared-keys-per-host.md)）。字段表收录后再把规格改成同步、在 `settings_sync.rs` 的 `ANDROID_LOCAL_SETTINGS` 里登记。
- 不受这项设置影响：14/18 键横滑限定字母（`EighteenKeySwipePolicy`，24 dp）、123 / #+= 层和九键的叹号上滑出 0、逗号上下滑出句号（都固定 14 dp）、九键 flick 阈值。

### 没滑到距离的纵移

距离调大之后，手指纵向移出键帽（约半个键高加 touch slop）却没到距离时，按钮自己会取消点击。

- 14/18 键：`GridKeyGesturePolicy` 把「纵向为主、超过 14 dp 却没触发手势」的按压记为点按（`verticalTap`，`forcesTap()` 为真），松手由监听补一次 `performClick`，与关闭「滑动输入符号」时的纵滑相同。判定用固定的 `THRESHOLD_DP` 而不是「滑动距离」，所以关闭时的行为与加设置之前逐字相同。
- 26 键不补：它关闭滑动时纵移也交给按钮，移出键帽就没有输入；距离调大后没滑到时与关闭时一致。#11822 起 26 键也补，见 [Android 26 键与 14/18 键字母键滑动判定统一](2026-11-22-android-letter-swipe-detection.md)。

## Alternatives considered

- **提高出厂距离（如 20 或 24 dp）**：不用加设置，多数快打用户开箱就少误触。没采用：改变所有用户的手感要维护者授权；#10773 的用户反而嫌滑出符号太难；多少合适需要真机数据。设置加上之后维护者仍可单独改默认。
- **按键高比例算距离**：跟着键盘高度走。没采用：报告人用的是默认高度，单手模式也不改行高，这个办法帮不到他们；还要跟着键面尺寸重构一起改。
- **共享 Rust 偏好、三端同步**（照 `touch_nine_key_flick_threshold`）：三端一致、随账号同步。没采用：「滑动输入符号」的开关和方向本来就是各端本机设置，iOS、鸿蒙也各有自己的；放进共享偏好要改 schema、三端宿主和服务端字段表，这里只解决 Android 的反馈。iOS、鸿蒙需要时各自照做。
- **快打判定（按压很短且位移不大时算点按）或不让抬手那一点触发**：直接针对快打。没采用：需要真机数据调参，可能吃掉真正的快速滑动；先给用户一个能调的距离。
- **26 键没滑到距离时也补点按**：移出键帽不丢键。没采用：26 键关闭滑动时一直交给按钮，这里保持两种状态一致，不改 26 键的点按语义。

## Consequences

收益：误触多的用户可以把距离调大，不必关掉整个「滑动输入符号」；默认手感不变；14/18 键调大距离后没滑到的纵移仍输入这一组，不丢键。

代价：多一项设置，只在 Android、只在本机，换设备要重新调。26 键调大距离后，纵向移出键帽却没到距离的按压没有输入，与关闭滑动时相同（#11822 起补点按，见 [Android 26 键与 14/18 键字母键滑动判定统一](2026-11-22-android-letter-swipe-detection.md)）。

验证：`SwipeHintPolicySmoke`、`GridKeyGesturePolicySmoke`（距离 30、48 的边界，两个方向，没滑到时补点按，横滑仍是 24 dp）和 `AndroidLocalSettingsSmoke`（默认 14、越界和不合步进的值丢弃、不同步）；设备上 `GridLetterSymbolDeviceChecks` 的 14 键距离 40 和 `LetterHintPunctuationDeviceSmoke -e letter-swipe true` 的 26 键距离 30。
