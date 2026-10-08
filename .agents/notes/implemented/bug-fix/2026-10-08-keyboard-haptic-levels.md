# Agent Note: 振动强度三档手感一样，以及「跟随系统」

Status: implemented

## Problem

用户反馈振动强度设置无效，轻、中、强三档感觉一样，希望能跟随系统的强度或者无级调整。偏好本身在三端都存取正常，问题出在「档位怎样变成真实的振动」：

- Android：`VibrationEffect.createOneShot(10, amplitude)`，从不检查 `hasAmplitudeControl()`。没有振幅控制的马达把任何振幅都按满振幅播，三档完全相同；有振幅控制的，10 ms 也短到马达还没起振。设置页试听用的又是 18 ms，和键盘上的不是同一种振动。
- iOS：中、强两档强度都是 1.0，只有 `.medium` 和 `.heavy` 的样式不同，单次点按几乎分不出来；Tauri 插件的试听用的是另一套数值（0.45、0.75、1.0）。
- HarmonyOS：三档只是时长不同（10、20、35 ms），不带强度；档位只在键盘启动时读一次，设置页改了要等输入法进程重启才生效。

## Decision

- 三档保留，在每端改成真正有区别的振动方案，键盘按键和设置页试听共用同一个映射：
  - Android `keyboard/KeyboardHaptics`：按设备能力依次取，API 30 起设备支持时用 Composition 原语（轻是 TICK 0.7，中、强是 CLICK 0.6 和 1.0）；API 29 起用预定义效果 TICK、CLICK、HEAVY_CLICK；有振幅控制时用 12、20、30 ms 配三档振幅；都没有时只靠时长区分（8、16、28 ms）。
  - iOS `KeyboardFeedbackPreference`：轻是 `.light` 0.5，中是 `.medium` 0.8，强是 `.heavy` 1.0。键盘、引导页、皮肤编辑器和 Tauri 插件的试听都用它。
  - HarmonyOS `KeyboardFeedback.plan`：设备支持时用预置效果加强度，轻是 `haptic.effect.soft` 35，中是 `haptic.effect.sharp` 70，强是 `haptic.effect.hard` 100；支持与否用 `isSupportEffectSync` 查一次并缓存；不支持时按 8、20、40 ms 的时长振。每次聚焦都重读反馈文件，打开工具面板时也重读。
- 新增第四档 `system`（跟随系统）：
  - Android 交给 `View.performHapticFeedback(KEYBOARD_TAP)`，不带 `FLAG_IGNORE_GLOBAL_SETTING`。
  - HarmonyOS 用 `usage: 'touch'` 加 `haptic.clock.timer`，不带强度。
  - iOS 没有公开接口读系统的键盘触感设置，「跟随系统」是不带强度参数的 `.light` 冲击，即系统默认的那一下。
- 三档本身不带 Android 的 `USAGE_TOUCH` 用途：带上它，关掉系统触感反馈的用户在三档下就完全不振了。三档是给想自己定强度的人用的，跟系统走的是「跟随系统」。
- 取值 `system` 在所有校验处都接受：client-core 的 `valid_haptic_strength`、`crates/tauri-mobile-platform`、`apps/desktop/src-tauri` 的 `valid_mobile_haptic_strength`、HarmonyOS 的 `AccountPreferencePlan`、iOS 的 `IOSPreferencePlan`。默认值仍是 `medium`，不认识的值仍按 `medium`。
- iOS 暂不上传 `system`（`IOSCloudSettings`），选了「跟随系统」时云端保留原来的强度。原因是更早版本的 `IOSPreferencePlan` 遇到不认识的档位会拒绝整份文档，同一账号下还没升级的设备就会同步失败。Android 和 HarmonyOS 的旧版本遇到不认识的档位只跳过这一项，所以照常上传。

## Alternatives considered

- **改成 0–100 的无级滑杆**：用户原话里提了。但同步格式要从枚举改成数值并兼容旧值，而大量 Android 马达没有振幅控制，在这些设备上滑杆只是摆设，拖到哪都一样；三档加「跟随系统」能在所有设备上兑现。
- **只修三档、不加「跟随系统」**：改动更小。但 Android 和 HarmonyOS 都有现成的、尊重系统触感设置的接口，用户也明确问了能不能跟随系统。
- **iOS 也照常上传 `system`**：三端行为一致。代价是同一账号下未升级的 iOS 设备整份设置同步失败，这比少同步一项糟糕得多。

## Consequences

- **收益**：三档在三端都对应不同的系统效果，试听和按键一致；想跟系统走的用户有了明确的一档。
- **代价**：Android 和 HarmonyOS 各多一条按设备能力降级的分支要维护。iOS 的「跟随系统」只在本机生效，不随账号同步，直到放开上传。
- **未验证**：三端都没有在真机上逐档摸过；各档的强度数值可能还要按真机手感调。

## Verification

- Android：`bash platforms/android/check-host.sh`（`KeyboardFeedbackSmoke` 核对四档方案各不相同，`KeyboardFeedbackStoreSmoke` 核对 `system` 的存取）。
- iOS：`NineKeyKeyboardTests` 的档位映射用例；`swift test --package-path shared/backend --filter BackendPreferencesTests`。
- HarmonyOS：`bash platforms/harmony/tests/run.sh`。
- 共享层：`cargo test -p msime-client-core`、`cargo test -p msime-tauri-mobile-platform`、apps/desktop 的 `mobile-keyboard-feedback-section.test.tsx`。
