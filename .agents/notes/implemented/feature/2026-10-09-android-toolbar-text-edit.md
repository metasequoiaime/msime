# Agent Note: Android 工具栏上的「文本编辑」按钮

Status: implemented

## Problem

#6351：文本编辑面板（[2026-10-08-android-text-edit-panel](2026-10-08-android-text-edit-panel.md)）只能从功能面板第 3 页打开，要先点工具栏的「更多」、翻到第 3 页再点。用户希望工具栏上直接有一个入口。那篇笔记当时把「工具栏常驻按钮」列为备选方案，理由是要新增一项本地设置和设置页开关，那一轮只放进了功能面板；工具栏的可选按钮里一直没有它。

## Decision

- 工具栏加一个可选按钮「文本编辑」，由本地设置 `platform.android.toolbar_text_edit` 控制，布尔，默认关（不改变现有工具栏），只在本机。与「浮动键盘」按钮（`platform.android.toolbar_floating`）同样处理：服务端的同步字段表和 `crates/client-core` 的 `ANDROID_LOCAL_SETTINGS` 都没有这个键，所以先不随账号同步。
- 开关在设置「键盘」页「键盘工具栏」里，和表情、常用语、剪贴板、皮肤、输入方式、浮动键盘排在一起（`KeyboardOptionsPage.TOOLBAR_BUTTONS`），工具栏预览跟着显示，设置搜索能搜到。
- 按钮用 `ImeToolbar.panelToggle` 开关同一个 `ImeTextEditPanel`：面板关着时打开（先关掉别的工具栏面板，完成组字），开着时再点关上，回到键盘；面板开着时按钮画成选中，和表情、剪贴板按钮一致。没有输入连接时面板打不开，按钮灰掉。
- 图标沿用功能面板那一格的 Lucide text-cursor-input（`KeyboardIconPaths.Icon.TEXT_EDIT`），新增 `KeyboardShortcutIconPolicy.Icon.TEXT_EDIT` 映射到它，用户在两处看到的是同一个图标。

## Alternatives considered

- **默认显示这个按钮**：用户不用去设置里找。但工具栏一行等分宽度，每多一个按钮其余按钮都变窄；文本编辑是偶尔用的功能，默认关与「浮动键盘」按钮一致。
- **随账号同步**：换设备后工具栏一样。但同步字段表在服务端，这次改不到；「浮动键盘」按钮也是先只在本机，等字段表加上后两个一起进 `ANDROID_LOCAL_SETTINGS`。
- **为工具栏另画一个 Material 实心图标**：和工具栏其他按钮的实心风格一致。但需要新的图稿，功能面板的描边图标已经能被认出是同一个功能，先复用。

## Consequences

- **收益**：打开文本编辑面板从三次点按（更多、翻页、文本编辑）变成一次。
- **代价**：工具栏上这一个按钮是描边图标，其他是实心图标，风格略有差别；多了一项只在本机的设置，换设备要重新打开。
- **验证**：`KeyboardShortcutIconPolicySmoke` 钉住图标映射，`AndroidLocalSettingsSmoke` 钉住默认关、不同步，`SettingsSearchIndexSmoke` 核对设置搜索，均由 `platforms/android/check-host.sh` 运行；API 35 模拟器上打开设置里的开关后工具栏出现按钮，点按打开面板、再点关上。
