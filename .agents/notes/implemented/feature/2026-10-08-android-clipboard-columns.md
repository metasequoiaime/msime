# Agent Note: Android 剪贴板面板的双列排列

Status: implemented

## Problem

#5642：剪贴板历史多的时候，单列一条占一整行，要上下滑很远。用户希望能改成一行两条，作为一个选项。

## Decision

- 本地设置 `platform.android.clipboard_columns`（`AndroidLocalSettings.CLIPBOARD_COLUMNS`），取值 `one` / `two`，默认 `one`，只在本机、不随账号同步：同步字段表（`crates/client-core/src/account/settings_sync.rs` 的 `ANDROID_LOCAL_SETTINGS`）里没有它，加进去要改 Rust 和服务端字段表。
- 设置入口在「键盘」页新增的「剪贴板」一节：「排列」→ 单列 / 双列。键盘在 `applyLocalSettings` 里读成 `clipboardColumns`，和其他本地设置一样，下次弹出键盘时生效。
- `ImePanels.clipboardCard` 按列数排：双列时两条一行、等宽同高，最后一行只有一条时右边留空；本机和云端两个分段都这样排。列数与行号的换算在 `ClipboardLayoutPolicy`。
- 双列时长按一条打开的操作行跨在那一行下方，被长按的那条画成选中样式，看得出操作对着哪一条；单列时也一样标出。

## Alternatives considered

- **在剪贴板面板顶行放一个切换按钮** — 不用进设置就能换；但顶行已经有本机 / 云端分段和清空（以及清空的确认），窄屏上再加一个按钮会挤掉分段，而这个选择通常设一次就不再改。
- **按屏幕宽度自动决定列数** — 不用设置；但用户在手机上明确要双列，平板上也有人宁愿单列多露几个字，这是偏好而不是尺寸问题。

## Consequences

- **收益**：历史多时翻得少；默认仍是单列，不改变现有用户看到的样子。
- **代价**：双列时每条只有半行宽，最多三行的文字露出得更少；设置不随账号同步，换设备要重新选。

## Verification

`platforms/android/tests/clipboard/ClipboardLayoutPolicySmoke.java` 验证取值、默认值、只在本机和行号换算；`bash platforms/android/check-host.sh` 编译面板代码并运行它。设置页（`home/KeyboardOptionsPage.java`）只在 Gradle 构建里编译。双列在真机上的观感没有在设备上验证。
