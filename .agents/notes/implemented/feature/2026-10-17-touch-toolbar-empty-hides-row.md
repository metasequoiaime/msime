# Agent Note: 触屏工具栏工具全关时整行不显示，长按工具栏进编辑

Status: implemented

## Problem

用户在 Android 上把工具栏的可选工具全部关掉后，空闲时那一行还在，只剩两端固定的品牌键和收起键，要求这时整行不显示。之后又看到从底栏打开剪贴板面板时，这一行又被带了出来。同时要求长按工具栏按钮能编辑显示哪些按钮，三个移动宿主一致。Android 已有长按进「编辑工具栏」（[Android 工具栏排序](2026-10-15-android-toolbar-order.md)），iOS 没有；鸿蒙的工具栏不可配置，另见[鸿蒙手机工具栏可选工具](2026-10-17-harmony-toolbar-tools.md)。

## Decision

工具全关按显示方式「隐藏」处理：Android `MSIMEInputService.toolbarRowHidden()` 是 `toolbarHidden` 或七个可选工具一个都没开，iOS `toolbarRowHidden` 是 `toolbarHidden` 或 `optionalShortcuts` 全部隐藏。只有决定空闲时这一行画不画的地方用它，「表情按钮在不在工具栏上」之类的判断照旧读各自的开关，全关时它们本来就是关的。组字时候选行照常显示。

这一行藏着时，打开的面板是否把它临时带出来，Android 由 `toolbarExitNeeded()` 决定：多数面板（表情等）的返回键就是工具栏上的收起键，必须带出来；只开着剪贴板面板、而底栏画着时不带，底栏的剪贴板键再点一次就关上面板。这条对「隐藏」和工具全关两种情况都生效。

设置页预览跟着键盘：Android 的「键盘工具栏」预览在工具全关时写「没有打开的工具按钮，工具栏不显示，只显示候选条」；iOS `KeyboardToolbarSettingsView` 的预览和「已隐藏」提示改按同一个 `rowHidden` 判断（现在由 `KeyboardToolbarPreviewRow.items` 算出，26 键长空格时这一行总带着 符，见[键盘预览跟随按键布局](../bug-fix/2026-10-29-keyboard-previews-follow-layout.md)），工具开关仍只在「隐藏」时置灰，全关之后还能逐个打开。

iOS 长按：工具栏上全部按钮（含品牌键和收起键）各挂 0.5 秒的 `UILongPressGestureRecognizer`，`cancelsTouchesInView` 让长按不再触发点按，播放按键反馈后经 `KeyboardAppLauncher.toolbarSettingsURL`（`<scheme>://settings/toolbar`）打开 App 的「键盘工具栏」页，App 端 `SettingsRouter` 原本就能解析这条路由。每个按钮另有 VoiceOver 自定义操作「编辑工具栏」。

## Alternatives considered

- **只隐藏品牌键和收起键之外的空白，保留一行高度。** 打字时键盘不会长高。但用户要的是「不要显示这个条」，留一条空白同样是那一条；键盘长高的问题「隐藏」模式早已接受，读音行加候选行与工具栏同高的约定只在工具栏显示时成立。
- **工具全关时仍保留品牌键，单独画成一个小按钮。** 保住了功能面板的入口，但还是一条横栏，正是用户要去掉的东西。
- **面板打开时一律不带出工具栏。** 规则更简单，但表情等面板只有工具栏上的收起键能回到键盘，打开就退不出来。

## Consequences

工具全关或「隐藏」时品牌键也不在，键盘上进不了功能面板，要回 App 打开按钮；这与原有「隐藏」模式一致。iOS 的功能菜单没有「编辑工具栏」一项（Android 有），全关之后在 iOS 上只能从 App 改回来。打字开始和结束时键盘高度会变，「隐藏」模式本来就是这样。

## Verification

- Android：`check-host.sh` 通过；发布签名 APK 装在 API 28 模拟器上，关掉全部工具后空闲时没有工具栏行、按笔画键候选行照常出现，「编辑工具栏」预览显示全关说明；同一 APK 覆盖安装到 Pixel 3 XL。底栏打开剪贴板面板的情形在这台模拟器上看不到底栏，没有在设备上点过。
- iOS：模拟器上 App 与键盘扩展编译通过；`NineKeyKeyboardTests` 新增全关时顶栏高度为 0、组字时候选行回来、打开常用语后这一行重新出现，以及 13 个按钮都挂 0.5 秒长按并取消触摸；`KeyboardAppLauncherTests` 固定链接格式。没有在真机或模拟器界面里实际长按。

后续 iOS 键盘高度稳定性修复见 [iOS 隐藏工具栏时保持键盘高度稳定](../bug-fix/2026-10-17-ios-keyboard-height-stability.md)；Android 和 HarmonyOS 仍按本篇的整行收起规则运行。
