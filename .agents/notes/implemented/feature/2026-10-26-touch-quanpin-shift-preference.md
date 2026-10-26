# Agent Note: 触屏全拼 26 键组字时保留 Shift 的共享偏好

Status: implemented

## Problem

[全拼 26 键用分词键替代触屏 Shift](../../implemented/feature/2026-10-15-quanpin-touch-separator.md) 落地后，组字中第三排左端不再是 Shift，触屏上就没有办法臂辅助码（Shift+字母），需要辅助码的用户没有退路。当时没有留切换配置。

## Decision

共享偏好新增 `touch_quanpin_shift`（`crates/client-core/src/preferences.rs` 的 `Preferences`），默认关：关着就是原有分词行为，老用户手感不变。打开后 Android、iOS 与 HarmonyOS 的全拼 26 键在组字中保留 ⇧，点一下臂辅助码（Shift+字母），不替换成分词键。九键、14 键、18 键和九键双拼自带的分词键不受它影响。双拼 26 键本来就不替换 Shift，也不受影响。

偏好只在组字分支处生效：Android 的 `usesQuanpinSeparator()`（`MSIMEInputService.java`）、iOS 的 `usesQuanpinSeparator`（`KeyboardViewController.swift`）与鸿蒙的 `KeyboardView.usesQuanpinSeparator()`（偏好经 `KeyboardScheme.usesQuanpinSeparator` 的 `shiftKept` 参数传入，键盘判定保持纯静态）都先看它，其余条件（中文、26 键字母页、非本地模式、非密码框）不变。键面、读屏和点击随同一分支恢复 Shift 语义，[就地改键面](../../implemented/bug-fix/2026-10-11-android-shift-relabel-in-place.md) 的约束照旧。鸿蒙的键面进了 `faceKey()`（`run.sh` 门禁钉住双拼提示的同一条），开关变化会重画字母键。

偏好随账号同步：Android 走 `platform.android.quanpin_shift`，鸿蒙走同构键 `platform.harmony.quanpin_shift`；iOS 键盘经 App Group 镜像 `keyboard.quanpin.shift` 读取，模式与 `touch_shuangpin_key_hints` 完全相同（`KeyboardLayoutPreference.swift`）。三端宿主都读这个偏好：Android 的「键盘 › 组字时保留 Shift」（`KeyboardOptionsPage.java`），iOS 的「键盘 › 布局 › 组字时保留 Shift」（`KeyboardLayoutSettingsView.swift`），鸿蒙走共享 React 设置页「屏幕键盘 › 布局 › 组字时保留 Shift」（`screen-keyboard-page.tsx` → `touch-keyboard-geometry-section.tsx`，键盘侧 `KeyboardSession.ets` 以 `=== true` 读文档、缺省关）。

## Alternatives considered

- 默认改回 Shift、分词做成另一个键：恢复旧键位会让依赖分词断音节的用户失去现有入口，且 26 键底行没有空位再放一个分词键；保留默认、把选择交给用户是双向都能活的方案。
- 把开关做成平台本地设置（Android `platform.android.*` 本地、iOS 只存 App Group）：改动更小，但同一账号的 Android 手机和 iPad 要各设一遍；辅助码习惯是跨端的，共享偏好照 `touch_shuangpin_key_hints` 的现成先例走。

## Consequences

用户打开开关后在触屏全拼组字中重新获得辅助码入口；代价是打开后组字中无法再点这个位置断音节，需要音节分界的用户保持默认。共享文档缺这个键或取值不是布尔时按关读，旧版本与桌面宿主不受影响。macOS 的偏好覆盖门禁（`preference_coverage.py`）为它登记了不适用理由。

## Verification

- `cargo test -p msime-client-core` 与 `-p msime-client-services`：preferences、settings_sync、harmony_settings 相关测试全部通过（导出键清单的两处快照已更新）。`skin::catalog` 与 `host_surface` 各有一个失败，在干净工作区上同样失败，属本机 Windows 环境的既有问题。
- `python platforms/macos/tests/settings/preference_coverage.py .` 通过（370/418）。
- Android `check-host.sh` 与 iOS XCTest 本机缺 Android SDK 和 Swift 工具链，未跑；编译与 JVM 冒烟由 CI 的 android job 补跑，iOS 验收（键盘 XCTest、设置页开关、组字中 Shift 臂辅助码的真实上屏）待模拟器执行。
