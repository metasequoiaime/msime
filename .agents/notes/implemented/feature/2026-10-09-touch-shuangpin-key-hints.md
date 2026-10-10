# Agent Note: 触屏双拼键位提示开关

Status: implemented

## Problem

触屏 26 键在双拼方案下会在字母键底部画这个键代表的声母/韵母（例如小鹤 `K` 下的 `ing uai`），读屏也会把它读出来。熟练的双拼用户不需要它，想把键面收干净（#5957）。三个触屏宿主原先只按「非独立英文、双拼方案、非本地模式」决定画不画：Android 的 `ShuangpinKeyHintPolicy.visible`、HarmonyOS 的 `ShuangpinKeyHintPolicy.visible`、iOS `KeyboardViewController.updateLetterCaseControls` 里的 `isChineseMode && !inLocalMode`，都不读任何用户偏好。

## Decision

- 新增共享偏好 `touch_shuangpin_key_hints: bool`（client-core `Preferences`），serde 缺省和 `Default` 都是 `true`：旧文档、没写过这个键的设备照旧画提示。桌面宿主原样保留、不使用（`preference_coverage.py` 记为不适用）。
- 开关只是叠加在原有三个条件上的第四个条件，关掉时提示为空，其余规则不变：全拼、独立英文和本地模式本来就不画，打开开关也不会让它们画。
  - Android：`ShuangpinKeyHintPolicy.visible/hint` 多一个 `enabled` 参数；`MSIMEInputService` 从偏好读出 `shuangpinKeyHintsEnabled`，空串交给 `KeyHintButton.setHintText` 后 11dp 下边距自然收回，`contentDescription` 也不再带「；双拼提示」。这个值进了 `touchGeometryKey()`，偏好快照改了它就触发 `render()` 重画（`check-host.sh` 锁住 `touchGeometryKey()` 含这一位）；也进了 `SKIN_HINT_KEYS`，冷启动的第一帧按皮肤片段画，不会先闪一帧提示（`check-host.sh` 锁住这一点）。
  - HarmonyOS：`ShuangpinKeyHintPolicy.visible/hint` 多一个 `enabled` 参数；`KeyboardPreferences.shuangpinKeyHints` 由 `touch_shuangpin_key_hints !== false` 得出，`KeyboardView` 的 `@State shuangpinKeyHints` 在 `applySettings` 和 `applyGeometry`（键盘内拖动几何时重读设置的另一处）里更新，并进了 `faceKey()`，字母键随之重建；`tests/run.sh` 锁住 `faceKey()` 含这一位。提示行是键面 `Column` 里的一个 `Text`，空串时不画，不占高度；HarmonyOS 的读屏文字本来就不含提示。
  - iOS：`KeyboardLayoutPreference` 加 `sharedShuangpinKeyHints(in:)`（缺省为开）、App Group 镜像 `keyboard.shuangpin.keyHints` 和 `saveShuangpinKeyHints`。键盘出现时 `synchronizeSharedTouchPreferences` 把文档的值抄进镜像，变了就调 `updateLetterCaseControls` 重画；`hint == nil` 同时驱动下边距和 `accessibilityValue`。
- 设置入口：共享设置页「屏幕键盘 › 布局」在移动宿主（`mobilePlatform`）上、且宿主的 `input_schemes` 含双拼时显示「双拼键位提示」开关（五笔、日文、越南文、藏文等不含双拼的版本经 `narrow_to_edition` 收窄后不列出，和原生设置页一致；数字键盘顺序不收窄，因为九键数字层每个版本都有），HarmonyOS 手机上跟在「数字键盘顺序」后面放进已经叫「布局」的尺寸组；Android 键盘页和 iOS 键盘设置页的「布局」组各加一行，本版本没有双拼方案时不显示。
- 账号同步沿用 `touch_number_keypad_order` 的做法：Android 经 client-core `settings_sync` 用 `platform.android.shuangpin_key_hints`，HarmonyOS 用 `platform.harmony.shuangpin_key_hints`。服务端字段表还没有这两个键，上传时会被按字段表过滤掉，目前只存在本机，等服务端登记后自动开始同步。iOS 的账号同步（`IOSCloudSettings`）本来就不带触屏键盘的几何和数字键盘顺序，这个开关也不进去。
- 「恢复默认」不重置这个开关：那一行明确列出它恢复的是高度、间距、顶部语音入口和工具栏按钮，数字键盘顺序同样不在其中。

## Alternatives considered

- **存成宿主本机设置（Android 的 `AndroidLocalSettings`、iOS 的 App Group）**：不用动 client-core，也不用碰同步字段表。但 HarmonyOS 的设置页就是共享设置页，读不到宿主本机设置；三端各存一份，以后要同步还得三端各接一次。它和 `touch_number_keypad_order`、`touch_voice_shortcut` 一样是触屏键盘的显示偏好，放共享文档最自然，见 [数字键盘顺序](2026-10-08-number-keypad-order.md)。
- **在宿主里把提示表过滤成空表，不改 `ShuangpinKeyHintPolicy`**：Android 和 HarmonyOS 的改动更小，但开关就只活在调用处，策略类的单测（`ShuangpinKeyHintPolicySmoke`、`keyboard-logic.test.ts`）覆盖不到它。把它做成策略的显式参数，「关掉时双拼也不画」和「打开时全拼也不画」都能在纯逻辑测试里锁住。
- **放进「恢复默认」的清单**：重置后提示一定回来，看起来更彻底。但这一行的说明和确认框都逐项列出恢复什么，加进去要同时改文案；而布局类的选择（数字键盘顺序）一直不在其中，用户也不会把「恢复键盘尺寸」理解成「把键面提示打开」。

## Consequences

- **收益**：不需要提示的双拼用户可以把键面收成纯字母，读屏也安静一些；三端读同一个键，设置页和同步只维护一份。
- **代价**：三端和共享设置页各多一个偏好要维护；Android 和 HarmonyOS 的 `ShuangpinKeyHintPolicy` 签名多了一个参数，以后的调用处都要传它。
- **待办**：服务端账号字段表登记 `platform.android.shuangpin_key_hints` 和 `platform.harmony.shuangpin_key_hints` 之后才会跨设备同步。

## Verification

- `cargo test -p msime-client-core`：旧文档缺省为开、保存 `false` 后读回、同步导出与导入往返（`settings_sync_export_is_exactly_the_shared_android_keys`、`settings_sync_round_trips_every_exported_key`）。
- apps/desktop vitest：`touch-keyboard-geometry-section.test.tsx`（只在给出时画、两种分组位置）、`settings.test.tsx`（桌面不画、Android 上缺省为开并写回 `false`、不含双拼的版本不画）。
- Android `ShuangpinKeyHintPolicySmoke` 与 `check-host.sh`；HarmonyOS `keyboard-logic.test.ts`（策略开关与同步映射）；iOS `KeyboardGeometrySettingsTests`、`NineKeyKeyboardTests.testTheShuangpinKeyHintSwitchDropsTheHintLineAndItsReading`（提示行隐藏、下边距为 0、`accessibilityValue` 为空）、`testTheShuangpinKeyHintSwitchReachesAKeyboardThatIsAlreadyShown`（键盘开着时只改共享文档，再次出现后同一组按键收回再画回提示）。
