# Agent Note: 字段表新增的共享偏好只由能改它的宿主导出和应用

Status: implemented

## Problem

metasequoiaime/msime-cloud#1277 给账号设置字段表加了 15 个共享偏好键。其中 `input.enumeration_comma_*` 两个客户端早有映射，其余 13 个（`mixed_input.quick_phrase`、`mixed_input.quick_phrase_frequency`、`sentence_association.grammar_model`、`local_modes.symbol`、`local_modes.date_time_menu`、`second_third_candidate.enabled`、`second_third_candidate.keys`、`translation.ai`、`ai_assistant.thinking`、`voice_input.polish_thinking` 和三个 `keybindings.toggle_*`）没有任何宿主导出或应用，加进字段表也不会同步。

`insert_input_settings` 由 Android、鸿蒙和桌面（Windows、Linux）共用。上传是合并进账号文档的：一个设置页改不了某项设置的宿主若导出它，每次同步都会把本机默认值盖到别的设备的选择上；应用一个本机没有开关的键，用户既看不到也改不回来。所以不能把 13 个键直接加进共用函数。

逐项核对各宿主（2026-10-27 的 develop）：Android 原生设置一个都改不了；鸿蒙的共享设置页能改符号模式、日期菜单和第二三候选；Windows 原生设置只提供语法模型（`translation_ai` 运行时会读，设置里却没有能关掉它的选项）；Linux 的共享设置页能改快捷短语混输、语法模型、符号模式、日期菜单、第二三候选和 AI 候选翻译，思考强度的两个选项页面上有，但它的服务提供程序固定不思考；三个切换热键只有 macOS 认（`toggle_hotkey_chords`）。macOS 13 项都能改，但它上传的是 `platform.macos.*` 快照，不经过这里。

## Decision

- 共用的 `insert_input_settings` / `apply_input_settings` 不变，Android 照旧不导出这 13 个键。
- `export_desktop_settings` / `apply_desktop_settings` 按 `DesktopHost` 区分：Windows 导出和应用语法模型；Linux 再加快捷短语混输、符号模式、日期菜单、第二三候选和 `translation.ai`。宿主由编译目标决定（`cfg!(target_os = "windows")`），client-services 在宿主本机编译，调用方签名不变。
- 鸿蒙导出和应用符号模式、日期菜单和第二三候选（`apply_touch_settings` 的平台附加应用函数）。
- 两个思考强度和三个切换热键，共享层的宿主都不导出也不应用。
- 第二三候选的键位目前只有 `semicolon_quote`，随开关一起导出；不认识的键位和语法模型只跳过那一个键。

## Alternatives considered

- **13 个键全部加进 `insert_input_settings`**：改动最小。否定：Android 改不了任何一项，鸿蒙、Windows 也只能改其中一部分，它们每次同步都会用默认值盖掉 Linux、macOS 上用户的选择；Windows 收到 `translation.ai = true` 会在设置显示「不使用在线服务」时照样用 AI 翻译，而且关不掉。
- **按 `HostCapabilities` 统一判断**：能力表本来就是宿主差异的来源。否定：只有快捷短语混输（`quick_phrase_candidates`）和热键（`toggle_hotkey_chords`）有对应的能力位，其余几项的差异来自各宿主原生设置页的取舍（Windows 有意不放第二三候选），不是能力；为同步新增能力位会把设置页的展示问题和同步混在一起。
- **新增一个宿主参数给导出、应用函数**：比按编译目标判断更直接可测。否定：调用方（Tauri、Windows 原生设置经 host-api）都只在本平台编译，再传一次平台只是重复；测试改用 `export_desktop_settings_for` / `apply_desktop_settings_for` 显式指定宿主。

## Consequences

Linux、Windows、鸿蒙之间这几项设置开始同步，Android 和这些宿主互不影响。收到别的设备选的语法模型而本机没装资源包时，运行时照常不用语法模型，设置页显示该模型待下载，不会自动下载。

没有处理的：macOS 原生快照还不带这 13 个键，macOS 与其他宿主之间这些设置仍不同步，需要在 `platform.macos.*` 快照的构造、应用、校验和下载过滤里各接一遍，另做。Linux 页面上显示的思考强度选项运行时不读，是设置页的问题，这里只是不同步它。

## Verification

- `cargo test -p msime-client-services --lib account::`：398 项通过。新增用例覆盖各宿主导出的键（Android 无、Windows 只有语法模型、Linux 八项、鸿蒙四项）、Linux 往返、Windows 只应用语法模型、鸿蒙往返且不应用别的新增键、不认识的语法模型和键位只跳过自身；原有「鸿蒙只写 `input.*` 与 `platform.harmony.*`」的用例改为同时允许这四个共享键。
- `cargo test -p msime-client-services -p msime-host-api`：host-api 有两项失败（`listing_reports_every_pack_and_checks_the_state_root`、`a_command_gets_the_local_time_to_the_minute`），在不带本次改动的 develop 上同样失败，与本次无关。
- `cargo clippy -p msime-client-services --all-targets -- -D warnings`：唯一的报错是 develop 上已有的 `settings_sync/tests.rs` `field_reassign_with_default`（caac03ed45 引入）；放开这一项后无警告。

## Related notes

检索了活跃笔记里的账号设置同步、`export_desktop_settings`、`insert_input_settings` 与字段表：[Windows 原生设置经 host-api 使用账号](2026-10-11-windows-settings-account-host-api.md) 部分重叠，本篇沿用它「原生设置窗口与共享应用调用同一份 client-services」的结构，只扩展导出的键；没有被本篇吸收或需要否决的旧笔记。
