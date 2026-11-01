# Agent Note: 拼音九键 ！ 滑出 0 的方向跟「九键滑动输入数字」

Status: implemented

## Problem

拼音九键字母层右列的 ！（右侧大回车外框时是接到符号栏末尾的那个 ！）上滑 14 dp 输入 0，这是 [Android 叹号键提供数字与上下角标快捷手势](../bug-fix/2026-10-16-android-exclamation-superscript-shortcuts.md) 定下的固定手势，当时明确独立于九键数字滑动方向的偏好。后来有了「九键滑动输入数字」（[九键滑动输入数字](2026-10-29-nine-key-swipe-digits.md)，`touch_nine_key_swipe_digits`），选「下滑输入数字」的用户在同一块键盘上 1–9 下滑出数字，0 却要上滑（#10441）。

## Decision

拼音九键字母层的 ！ 滑出 0 的方向跟这一项走：`SuperscriptShortcutPolicy.zeroDirection(mode)` 在 `down` 时返回下滑，`up`、`off` 和不认识的写法返回上滑。阈值由 `SuperscriptShortcutPolicy.zeroThresholdDp(mode, distance)` 给出：选下滑时与 1–9 键共用「九键滑动距离」，上滑和关闭时保持原来的 14 dp。反方向在 ！ 上什么也不做（这个键没有字母可弹）。

绑定在 `ImeLetterRows.bindNineKeySuperscriptShortcut`，只由 `ImeLayoutRows` 的两处九键 ！ 调用（标准外框右列、右侧大回车符号栏末尾）；方向和阈值在每次 `ACTION_DOWN` 时从服务的 `nineKeySwipeDigits` 和本机设置读，与 `ImeLayoutRows.bindNineKeySwipe` 相同，设置页改了下一次触摸就生效，读屏描述也在那时换成「下滑输入 0」或「上滑输入 0」。123 / `#+=` 符号页的 ！ 仍走 `bindSuperscriptShortcut`，固定上滑 14 dp。`check-host.sh` 钉住两处调用和按下时读取。

只改 Android：iOS 和鸿蒙的九键 ！ 没有滑出 0 的手势，不存在方向不一致。

## Alternatives considered

- **关闭时也用「九键滑动距离」**：同一排键手感统一，九键的键高，24 dp 默认值更不容易误触。但关闭时设置页把这个滑块置灰，用户调不了，默认用户的手感会无提示地从 14 dp 变成 24 dp；关闭时保持原状更稳。
- **上滑时也用「九键滑动距离」**（本 PR 第一版的做法）：上滑用户的 1–9 键已经按这个距离判定，统一起来手感一致。但 #10441 只涉及方向，上滑用户的 ！ 方向本来就对，跟着改阈值会让他们原本 14–24 dp 之间的上滑从输入 0 变成落回轻点、输入 ！，这是没有提示的行为变化；只有方向已经改变的下滑用户才顺带换阈值。
- **123 / #+= 符号页的 ！ 也跟这一项**：那一页不是九键，「九键滑动输入数字」管不到它，跟着变会让 26 键用户的符号页手势因为一个九键设置而改变。
- **只在建键时读一次方向**：写法最简单，但设置改了以后要等键行重建才生效，与 1–9 键按下时读的行为不一致。

## Consequences

选了下滑的用户 0–9 都是下滑，阈值都是「九键滑动距离」；上滑和关闭的用户行为不变，！ 仍是上滑 14 dp。代价是上滑用户的 ！ 与 1–9 键阈值不一致（14 dp 对「九键滑动距离」），与改动前相同。代价是 ！ 的绑定多了一个九键变体和一次按下时的读取。手势只在 JVM 冒烟（`SuperscriptShortcutPolicySmoke`）和源码检查里覆盖，真实触摸仍需设备验收：`SuperscriptShortcutDeviceSmoke`、`TallReturnDeviceChecks` 按默认（关闭）找「上滑输入 0」的描述，下滑模式没有设备用例。
