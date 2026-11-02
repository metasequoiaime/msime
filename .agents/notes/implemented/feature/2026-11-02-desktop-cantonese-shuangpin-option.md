# Agent Note: 桌面粤拼方案下的粤语双拼选项

Status: implemented

## Problem

#10523 希望 iOS 以外的平台也能显式选用粤语双拼。[粤语双拼键位规则](2026-10-26-cantonese-shuangpin.md)在共享引擎里，各平台通用；[iOS 的独立入口](2026-10-27-ios-cantonese-shuangpin-selection.md)靠共享触屏偏好 `touch_keyboard_schemes.selected == cantonese_shuangpin` 让会话强制双拼，而桌面宿主（macOS、Windows、Linux）原样保留这个字段、不使用它，所以桌面上没有任何途径打开强制双拼。

桌面的普通粤拼已经能打一部分双拼：前两键不是完整粤拼音节、又能解成双拼码时自动按双拼读，`s;` 这类组合也接受。缺口是与粤拼同形的码（如 `oi`）仍按粤拼读，用户没有办法说「我打的就是双拼」。

## Decision

- 共享偏好新增 `cantonese.input_method`，取值 `jyutping`（默认）或 `shuangpin`，类型是 `CantonesePreferences` / `CantoneseInputMethod`（`crates/client-core/src/preferences.rs`）。等于默认值时不写进文档，没动过这一项的用户文档与以前的版本逐字节相同；读盘时未知取值按 `settle_newer_values_on_read` 回退到 `jyutping`，与韩语布局的做法相同。
- host-api 用 `force_cantonese_shuangpin_for(platform, scheme, preferences)` 统一决定会话的 `force_cantonese_shuangpin`：桌面宿主只看 `cantonese.input_method`，触屏宿主只看 `touch_keyboard_schemes.selected`。会话重建（偏好变化）和词库维护两处构造引擎选项的地方都走它。
- 桌面设置把原来只读的「粤拼方案」说明行换成两项选择「粤拼 / 粤语双拼」，说明随选项变化：
  - macOS 与 Linux 用共享页 `InputSchemeDetailsSection` 的 `SegmentedRow`，触屏宿主（`hasTouchKeyboardSchemes`）不显示这一行。
  - Windows 设置（WinUI）用 `segment_row` 写 `cantonese.input_method`。
  - Win7 设置在粤拼方案下加一个下拉框，整组写 `cantonese` 对象（文档在默认值时没有这一组，`PreferencesStore` 的 `optional_object` 白名单放行它）。
- 不新增系统输入源：macOS 继续用同一个粤拼 TIS 输入模式，Windows 的 InputMode、Linux 的方案列表都不变。`;` 在三端都已经经 `spelling_symbols` 交给引擎，宿主原生代码不用改。
- 引擎不改。选了粤语双拼后，会话从第一键起按两键码解释（`CantoneseScheme::set_force_shuangpin`），与 iOS 选中粤语双拼时的行为相同；选普通粤拼时保留自动识别。

## 两个字段互不影响

桌面以前不读触屏字段，但 host-api 的判断没有区分平台：iOS 选了粤语双拼、偏好又被带到桌面时，桌面的粤拼会被强制成双拼。现在按编译到的平台分开读，桌面的选择也不改变触屏键盘。这是一处行为收紧，只影响「触屏方案是粤语双拼的文档出现在桌面」这种情况；账号设置同步目前不同步粤拼相关的键，实际很少遇到。

## Alternatives considered

- **为粤语双拼新增独立的系统输入源**（macOS TIS 模式、Windows InputMode、Linux 方案项）：用户能直接在系统输入法列表里切换，是最直观的入口。不采用：要改 Info.plist、TSF 图标、FFI 方案编号和各平台方案映射，[iOS 笔记](2026-10-27-ios-cantonese-shuangpin-selection.md)已经否决过新增全局引擎方案编号；两种输入共用同一份词典和候选语义，只是按键解释不同，与普通话双拼作为一个方案加 `shuangpin_profile` 的结构一致。
- **桌面也读 `touch_keyboard_schemes.selected`**：不需要新字段。不采用：这个字段在桌面没有任何设置入口，只能来自别的设备的文档，用户在桌面上既看不到也改不了；而且会让触屏和桌面的选择互相牵连。
- **把选项做成开关「粤拼按双拼输入」**：形式更简单。不采用：韩语布局（两套式 / 罗马字）和越南语方案（Telex / VNI）都是分段选择，粤拼方案已有说明行，直接扩成两项最贴合现有页面。

## Consequences

- 桌面三端都能显式选粤语双拼，出厂默认和老用户的行为不变。
- 桌面的双拼键位提示（`shuangpin_keymap_hint`）不覆盖粤语双拼，键位提示另见 #10438。
- Android 和 HarmonyOS 的触屏入口（方案列表里的「粤语双拼」与 `;` 键）不在这次改动里，另起一个 PR 完成 #10523。
- 宿主只要沿用 host-api 的会话构造，就自动得到正确的强制双拼判断；以后新增读这个字段的地方要走 `force_cantonese_shuangpin`，不要再直接比较 `touch_keyboard_schemes.selected`。

## Verification

- `cargo test -p msime-client-core --locked --lib cantonese`：新字段在默认值时不写进文档、`shuangpin` 往返、切回 `jyutping` 后键消失、未知取值回退。
- `cargo test -p msime-host-api --locked --lib cantonese_shuangpin`：六个平台各自只认自己的字段，别的方案下不强制。
- `apps/desktop` 的 vitest：`input-scheme-details-section`、`input-scheme-options`、`input-scheme-capability-gating`。
- Windows WinUI 与 Win7 设置页只能在 Windows CI 上编译和跑 `windows7_settings` 界面测试；macOS、Linux、Windows 上实际打字（选双拼后 `oi` 按双拼出字、切回粤拼后按粤拼出字）尚未在真机上看过。
