# Agent Note: 撤掉「26 键按 123」和「九宫格数字顺序」两项设置，数字键面固定

Status: implemented

## Problem

两项设置是先后加进来的：

- [九宫格数字顺序](../../archived/feature/2026-10-08-number-keypad-order.md)（`touch_number_keypad_order`，`phone` / `calculator`）：用户建议数字键盘能像计算器那样 7 8 9 在上，三端只有九宫格数字层是 3×3，于是给它加了一个排列选项。
- [26 键按 123](../../archived/feature/2026-10-10-twenty-six-key-nine-key-digits.md)（`touch_twenty_six_key_number_layout`，`row` / `nine_key`）：用户希望 26 键切到数字时能用九键那样的 3×3，于是让 26 键按 123 可以改画九宫格数字层，排列跟着前一项走。

它们在 HarmonyOS 手机的「键盘」设置页上紧挨着「中文键盘」。手机上只有标题和取值、没有说明，仓库负责人先是看不懂（「26 键数字键盘：九宫格」「数字键盘顺序：电话」），三端改名成「26 键按 123：数字一行 / 九宫格」「九宫格数字顺序：123 在上 / 789 在上」之后，仍然判定这两项不要。两项加起来让三个宿主各多出一条键面分支（26 键借用九宫格数字层）、一种数字排列和若干形态例外，换来的是很少有人会去找的选择。

## Decision

- 三端都去掉这两行：共享设置页的 `TouchKeyboardGeometrySection`（HarmonyOS 等用共享页的宿主）、Android 的 `KeyboardOptionsPage` 与设置搜索关键词、iOS 的 `KeyboardLayoutSettingsView`。
- 键盘不再读这两个字段，行为固定为原来的默认值：26 键（以及 14 键、韩文两套式等画字母行的方案）按 123 是一行 1 到 0 的数字页；拼音九键的数字层是电话顺序，1 2 3 在上。
- 只为这两项存在的代码一并删除，不留走不到的分支：
  - 26 键借用九宫格数字层：Android `NineKeyLayout.twentySixKeyDigits`、`ImeLetterRows` / `ImeLayoutRows` / `ImeBottomRow` 的对应分支；iOS `KeyboardViewController.opensNineKeyDigitPad` 及借用时的标点、长按处理；HarmonyOS `KeyboardView.ets` 的 `twentySixKeyDigitGrid` 等。
  - 计算器顺序：Android `NineKeyLayout.rows(digits, calculator)` 的第二个参数，iOS 位置到数字的映射，HarmonyOS `NineKeyLayout.digits(order)` 的参数和 `CALCULATOR_DIGIT_ROWS`。
  - iOS `SharedUI` 里的 `NumberKeypadOrder`、`TwentySixKeyNumberLayout` 镜像和 App Group 写入；Android 皮肤片段 `SKIN_HINT_KEYS` 里的这两个键。
  - Android「九键数字键盘左侧符号」回到只在本版本有拼音九键时显示（它曾因 26 键可选九宫格而多出一个显示条件）。
- 偏好字段保留：client-core 的 `touch_number_keypad_order`、`touch_twenty_six_key_number_layout` 及其解析、默认值、账号同步（`platform.*.number_keypad_order`、`platform.*.twenty_six_key_number_layout`），HarmonyOS 的 `AccountPreferencePlan` 都不动。旧文档、旧版本同步上来的值照常读写，只是没有键盘再读它们。`platforms/macos/tests/settings/preference_coverage.py` 里两项的「不适用」理由仍然成立，不改。

## Alternatives considered

- **只藏起设置行，已存的值继续生效**：改动最小。不用它，是因为改过的人从此改不回来，而键盘里的两条分支照样要维护。
- **只在 HarmonyOS 去掉**：HarmonyOS 用的是共享页，改共享页和鸿蒙键盘就够。不用它，是因为三端的这两项本来就是一起加、一起改名的，只撤一端会让同一个偏好在三端含义不同，同步过去的值在一端生效、在另一端无处可改。
- **连 client-core 的字段一起删**：最干净。不用它，是因为会改配置格式，旧版本和服务端字段表还带着这两个键；保留字段、不读它，对新旧版本都无害。
- 原两篇的备选（为 26 键单独画 4×3 拨号盘、把 3×3 嵌进 123 页、存成宿主本机设置、数字格记 `Nine*` 键位）随功能一起作废，其中「数字记在 `Digit*` 上」的结论仍然成立：26 键的数字只有一行的 123 页一种，统计照旧记 `Digit*`。

## Consequences

- **收益**：设置页少两行看不懂的选项；三端键盘各少一条键面分支和一种数字排列，净删约 900 行；形态例外（iPad 全尺寸、Android 分离式不换，HarmonyOS 平板换）随之消失。
- **代价**：
  - 选过九宫格或 789 在上的用户，升级后会回到一行数字和 123 在上，且无处可改回。
  - 两个字段留在配置和同步里成为没人读的数据，日后若要彻底删除，需要连同服务端字段表一起处理。
  - iOS App Group 里旧的 `keyboard.numberKeypad.order`、`keyboard.twentySixKey.numberLayout` 两个键不再有人读，没有清理。

## Verification

- 共享设置页：渲染键盘页的 19 个测试文件 519 条通过，`voice-handwriting-harmony.test.tsx` 断言 HarmonyOS 手机键盘页不再出现这两行；`apps/desktop`、`apps/harmony` 的 `tsc`，`pnpm lint` 通过。
- HarmonyOS：`platforms/harmony/tests/run.sh` 键盘逻辑 4024 条断言 0 失败，`scripts/test-harmony-*.py` 全部通过，`hvigorw assembleHap` 编译 ArkTS 通过。
- Android：`platforms/android/check-host.sh`（编译 Java 并跑九键、14 键主机测试）通过，`scripts/test-android-*.py` 174 个通过；`FourteenKeyDeviceSmoke` 只编译过，没在设备上跑。
- iOS：键盘扩展和键盘测试两个 target 用 `xcrun swiftc -typecheck` 整体检查 0 个错误，`scripts/test-ios-*.py` 通过；XCTest 没有在模拟器上实际跑。

[历史快照：九宫格数字层的计算器排列](../../archived/feature/2026-10-08-number-keypad-order.md)

[历史快照：手机 26 键的「123」可以换成九宫格数字层](../../archived/feature/2026-10-10-twenty-six-key-nine-key-digits.md)
