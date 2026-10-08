# Agent Note: Android 剪贴板历史只按实时偏好清空

Status: implemented

## Problem

#5602：Android 键盘的剪贴板历史「丢三落四」。录屏里每点一个新的输入框、每切一次输入法再切回来，面板里就只剩最近复制的那一条。用户的诊断包里 `clipboard_history` 是开着的。

原因在键盘读偏好的两条路上。`MSIMEInputService.start`（`onStartInput`）先用 `runtime-options.json` 里的偏好副本调 `applyEditorPreferences(preferences, false)`，那份副本是首次安装时写下的，之后再没更新，`clipboard_history` 永远是出厂默认的关；`applyClipboardPreference` 照单全收，把开关置为关并调 `clearQuietly()`。`onCreateInputView` 又在实时偏好到来之前按字段初始值（关）清一次。实时偏好随后把开关改回开，打开面板时 `captureClipboard` 补读系统剪贴板，于是历史里只剩当前这一条。

## Decision

- `ClipboardHistoryRetentionPolicy`（`platforms/android/java/app/msime/android/clipboard/`）是唯一的判断：只有实时读到的偏好（`Source.LIVE`）能改 `clipboardHistoryEnabled`、能触发清空；`runtime-options.json` 的副本（`Source.RUNTIME_OPTIONS_COPY`）和读不到偏好（null）都不动开关、never 清空。
- 来源由调用处明说：`applyEditorPreferences(preferences, source)` 的第二个参数就是 `ClipboardHistoryRetentionPolicy.Source`，`start` 读完 `runtime-options.json` 传 `RUNTIME_OPTIONS_COPY`，单参数版本（`loadAppearanceWithoutSession`、`applyPreferencesSnapshot` 等实时路径）传 `LIVE`，`applyClipboardPreference(preferences, source)` 直接用它。这个参数原来是布尔量 `appearance`（是否重算皮肤），第一版把剪贴板也挂在它上面；两件事碰巧同真同假，但一个叫「重算皮肤」的参数决定要不要清空历史，日后有人为了皮肤改它的值就会把 #5602 带回来，所以换成写明来源的枚举，皮肤的判断也从同一个来源推出。
- `onCreateInputView` 不再清空历史。开关关着时由下一次实时读偏好清空：有引擎会话的输入框走 `applyPreferencesSnapshot`，没有会话的走 `loadAppearanceWithoutSession`，两条都是实时的。设置页说明里「关闭会立即清空」仍然成立。
- `check-host.sh` 守住三点：`onCreateInputView` 里没有 `clipboardHistory.clear`；`MSIMEInputService` 清空前问 `ClipboardHistoryRetentionPolicy.clearsHistory`；读 `runtime-options.json` 之后的那次 `applyEditorPreferences` 传的是 `Source.RUNTIME_OPTIONS_COPY`、不是 `LIVE` 或单参数版本，且 `applyClipboardPreference` 的参数就是这个来源。

## Alternatives considered

- **让 `runtime-options.json` 的副本跟着偏好更新** — 根上消除「副本是出厂默认」；但副本由 Bootstrap 在准备阶段写，它的其他字段（主题、方案）也依赖「永远是出厂默认」这一点被各处特殊处理，改它的影响面远大于这个 bug。
- **在键盘进程启动时同步读一次实时偏好再决定是否清空** — 开关立刻准确；但 `onCreateInputView` 在主线程上，`NativeClient.loadPreferences` 读盘加 JSON 解析会拖慢每次弹出键盘，而清空历史并不急：关着的开关本来就挡住了捕获和面板。

## Consequences

- **收益**：换输入框、切输入法、键盘进程被系统回收后重启，历史都不再被清空；清空只发生在用户确实关掉开关之后。
- **代价**：进程冷启动后、实时偏好到来之前（通常一两百毫秒），`clipboardHistoryEnabled` 仍是字段初始值关，这段时间里复制的内容监听不记；打开面板时的补读会把当时的系统剪贴板补上。用户在键盘进程不在时关掉开关，历史文件要等键盘下一次实时读偏好时才清空，这段时间里面板因开关关着而不可用，内容不会显示。

## Verification

`platforms/android/tests/clipboard/ClipboardHistoryRetentionPolicySmoke.java` 覆盖副本、读不到偏好和实时开关三种情况；`bash platforms/android/check-host.sh` 运行它并执行上面的三条源码守卫（第三条在一份把调用改成 `LIVE` 或单参数版本的副本上确认会失败）。真机上换输入框和切输入法的行为没有在设备上复现验证。
