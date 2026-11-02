# Agent Note: Android 与 HarmonyOS 的粤语双拼入口

Status: implemented

## Problem

#10523 希望 iOS 以外的平台也能显式选用粤语双拼。[iOS 的独立入口](2026-10-27-ios-cantonese-shuangpin-selection.md)已经把 `cantonese_shuangpin` 追加进共享触屏方案枚举，host-api 在触屏宿主上按 `touch_keyboard_schemes.selected == cantonese_shuangpin` 让会话从第一键起按两键码解释；但 Android 和 HarmonyOS 的方案表里没有这一项，用户无从选中。移动端键盘改动按仓库约定要三端一起做，iOS 已有，这次补齐另外两端。桌面三端的入口另由 PR #10586 完成（偏好 `cantonese.input_method`）。

## Decision

- Android `KeyboardScheme` 和 HarmonyOS `KeyboardScheme.ts` 各追加 `CANTONESE_SHUANGPIN`（`cantonese_shuangpin`，Engine 方案 `cantonese`，布局 `twenty_six_key`，标题「粤语双拼」，徽标「粤」「双」），排在末尾，与共享 `TouchKeyboardScheme::ALL` 的追加顺序一致；默认不启用（Android `optIn`、HarmonyOS `defaultEnabled`），和粤拼一样要 `msime-cantonese.db`，属于语言词库资源包。
- 选中粤语双拼写的偏好是 `scheme = cantonese`、`last_chinese_scheme = cantonese`、`touch_keyboard_layout = twenty_six_key`，再加 `touch_keyboard_schemes.selected = cantonese_shuangpin`。两个入口只靠 `selected` 区分：按方案和布局反查（`fromPreferences`）时落到普通粤拼，所以没有 `selected` 的旧文档行为不变。切换时宿主先写偏好、再让会话按新偏好重建，host-api 据此设置 `force_cantonese_shuangpin`，Engine 不改。
- 第二排 L 右边的 `;` 键：微软双拼原有的这个键现在也在粤语双拼下显示（Android `MicrosoftShuangpinKeyPolicy.showsSemicolonFinal`，HarmonyOS `KeyboardScheme.showsSemicolonFinal`），位置、宽度、第二排不缩进的规则和组字中把 `;` 交给 Engine 的路由都沿用微软双拼。读屏标签为「分号，粤语双拼 ing 和 ik」，与 iOS 相同；微软双拼的标签不变。普通粤拼不显示这个键，理由见 iOS 笔记的「分号键」一节。
- 设置页：Android 输入页「粤拼」一行列出粤拼和粤语双拼两个方案；HarmonyOS 用共享设置页，`touch-keyboard-scheme-helpers.ts` 的方案联合类型、选项表、默认列表、`selectTouchKeyboardScheme` 和「语言与方案」卡片的粤拼分组都加上 `cantonese_shuangpin`，宿主不提供粤拼（没有词库）时它和粤拼一起隐藏。
- 统计记到粤拼（Android `TypingSource`、HarmonyOS 按 Engine 方案名），空格键面写「粤语双拼」。

## Alternatives considered

- **在粤拼键盘上加一个「双拼」开关，而不是独立入口**：入口少一个，切换不用进方案列表。不采用：iOS 已经把粤语双拼做成独立的触屏方案，共享偏好也只认 `selected`；三端各做一种形态，同一份文档在不同手机上就会是不同的键盘。
- **粤语双拼不显示 `;` 键，让用户去符号层打分号**：第二排保持九个键、手感不变。不采用：ing、ik 只在 `;` 上，没有这个键方案就缺两个韵母，iOS 的笔记也是因此否决了把它做成开关。
- **双拼键位提示一起做**：不采用，属于 #10438。

## Consequences

- 三个手机平台都能选粤语双拼；出厂默认和已有用户的键盘不变。
- 粤语双拼与普通粤拼共享词库下载、简繁处理和候选语义；两者的差别只在 host-api 设置的会话模式和 `;` 键。
- 以后再给共享触屏方案追加入口时，Android、HarmonyOS 的方案表、共享设置页的选项表和这些平台的测试里写死的方案数都要一起改。

## Verification

- `platforms/harmony/tests/run.sh`：新增「粤语双拼是追加在最后的一张卡」一组，覆盖顺序、默认不启用、词典、`selected` 区分、偏好映射、`;` 键和读屏标签。
- `apps/desktop` vitest：`touch-keyboard-scheme-selection` 新增粤语双拼的选择、推断、标题和分组用例。
- Android JVM 冒烟（`platforms/android/check-host.sh`）：`KeyboardSchemeSmoke`、`MicrosoftShuangpinKeyPolicySmoke`、`TypingSourceSmoke`、`ms_w3_kcb_KeyboardFacesSmoke`。
- 没有在 Android 真机或模拟器、HarmonyOS 真机上看过方案选择器、`;` 键和实际打字；HarmonyOS 的 ArkTS 编译只在 CI 上跑。
