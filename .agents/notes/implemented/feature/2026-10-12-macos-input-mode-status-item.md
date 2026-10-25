# Agent Note: macOS 菜单栏输入模式状态项

Status: implemented

## Problem

#7066：macOS 用户用单按 Shift 切换水杉内部的中英文状态，想在菜单栏随时看到现在是「中」还是「英」，尤其在没有输入框焦点或开着多个窗口时。系统菜单栏的输入法图标停在「水杉输入法 · 中」，不跟着变；输入模式提示（HUD）只闪一下，悬浮工具栏是常驻浮框，报告人都不想要。

让系统图标跟着变的代码早就有了（PR #665）：中英文切换时 `syncSystemInputModeForClient:` 经 `MSIMESelectSystemInputMode` 对客户端发 `selectInputMode:`，系统报告的模式经 `setValue:forTag:client:` 反向同步。但它有前提：只有 `MSIMEInputSourceIsEnabled` 认为已启用的模式才会被请求。用户只在系统设置里加了「中」时，「英」不可选，控制器按设计不请求它，菜单栏一直是「中」。macOS 27.0 上进程又启用不了键盘输入模式（`TISEnableInputSource` 返回 noErr 而状态不变，见 `docs/macos-parity.md`《模式启用不了不是安装失败》），只能由用户自己去系统设置添加。报告人还不想多一条「英」输入源：它会带来 macOS 按应用记忆输入源的行为和系统级切换的提示。PR #665 自己写明「Not verified on an installed build」，加了「英」之后系统图标是否真的跟随，至今没有真机证据。

Windows 有托盘语言栏，Linux 有 Fcitx5/IBus 面板，触屏键盘的键面本身显示模式，缺这块的只有 macOS。切换后在光标旁闪一下的徽标在各桌面宿主上的对齐见 [2026-10-10-windows-input-mode-parity.md](2026-10-10-windows-input-mode-parity.md)，那是另一种反馈，与这里的常驻显示互补。

## Decision

**输入法进程自己在菜单栏放一个 `NSStatusItem`，由新的共享偏好 `input_mode_status_item` 控制，默认关闭。** 偏好在 `crates/client-core/src/preferences.rs`（`#[serde(default)]`，缺字段的旧文档读成关闭）；能力位 `HostCapabilities::input_mode_status_item` 只在 macOS 为真，共享设置页只在这个能力下于「输入 › 中英文」组显示「菜单栏显示中/英状态」，原生设置窗口候选窗口卡片里有同名开关，键 `MSIMEClientInputModeStatusItem` 在该分区的恢复列表里。

**状态项显示控制器算出的模式，而不是系统图标回退后的模式。** `syncSystemInputModeForClient:` 算出 `mode`（英文模式是 `.Roman`，否则按 `MSIMEEffectiveInputScheme` 的实际方案），先照旧调 `MSIMESelectSystemInputMode`，再把同一个 `mode` 交给状态项。「双」「五」等模式没有加成输入源时系统图标回退到「中」，状态项仍显示「双」「五」；⌃⇧E 的英文候选模式不是英文模式，两处都显示「中」。所有中英文切换（单按 Shift、Shift+空格、单按 Caps Lock、菜单、悬浮工具栏）、换方案、系统报告的模式、`activateServer:`、`appearanceChanged:` 和偏好轮询都经过这个方法，所以状态项不需要逐处接线。显式同步仍经现有的 `selectInputMode:` 路径；激活时立即摆放状态项，系统模式校准延后一轮主队列并核对实际输入源，见[恢复输入源时避免重复选择](../bug-fix/2026-10-21-macos-input-source-restore.md)。

**图标与系统输入法菜单是同一张。** `MSIMEInputModeMenuIconFileIn`（`InputModeIdentifiers.h`）沿 `MSIMEInputModeDeclaredIn` 的取值路径读 bundle 里该模式的 `tsInputModeMenuIconFileKey`；本版本没有声明该模式时（五笔版把五笔放在 `.Hans` 上）取 `.Hans` 的，再取不到时回退到 `tsInputMethodIconFileKey`，最后是写死的 `MSIMEClientInputMethodMenuIcon.tiff`（`info-plist-icons` 检查它仍是 plist 声明的那张）。图标作为模板图随菜单栏浅色、深色着色，悬停提示和读屏标签是 `MSIMEInputModeMenuName`。状态项设固定的 `autosaveName`，首版不挂菜单，点击没有动作。

**生命周期。** 共享文档里的开关在 `applySharedToolbarPreferences:` 里、也就是文档被载入的那一刻生效：关上时隐藏，打开时只有持有客户端的控制器按 `currentInputModeID` 显示，所以组字中推迟应用文档的分支也不必等下一次切换；输入法不在前台时收到的通知只会隐藏，不会把状态项放回菜单栏。但输入法只在有控制器持有客户端时才载入这份文档：每秒一次的轮询在 `deactivateServer:` 时停掉，`reloadPreferences` 和 `completePreferenceLoad:` 都要求 `_activeClient`，而在设置应用里拨开关时焦点在设置应用上，没有控制器持有客户端。所以在 Tauri 设置页或独立进程的原生设置窗口里拨开关，要等回到输入框、输入法重新获得焦点时才生效（`activateServer:` 之后的那次载入）。在输入法进程里打开的原生设置窗口（桌面设置应用缺失时的回退）不经过文档，开关直接改共享的外观偏好并发出 `MSIMEAppearanceDidChangeNotification`：`appearanceChanged:` 在关上时由每个控制器直接隐藏状态项，所以关上立即消失；打开同样等下一次获得焦点，没有客户端的控制器说不准该显示哪个方案（`currentInputModeID` 依赖各自会话的宿主选项）。切到 ABC 等别的输入源时，`input_method_main.mm` 里 `MSIMEInputSourceMonitor` 的切走回调把它移除，切回来时 `activateServer:` 的激活调度立即重新放上，不等待系统输入源校准。`deactivateServer:` 不隐藏它，焦点在客户端之间移动时它一直在。隐藏用 `removeStatusItem:` 整个移除，下次新建时设同一个 `autosaveName`，而不是保留对象只设 `visible = NO`：在 macOS 27.0.1 上用一个独立的小程序实测，把带 `autosaveName` 的状态项设成不可见会删掉 defaults 里的 `NSStatusItem Preferred Position <名字>`，`removeStatusItem:` 后它还在，用同一个名字新建的状态项读回同一个位置。

**测试进程不碰菜单栏。** 控制器经 `presentInputModeStatusItem:` 摆放状态项：设了 `_inputModeStatusItemPresenter`（测试经 KVC 注入的记录 block）就交给它；否则只有 Info.plist 声明了 `InputMethodConnectionName` 的输入法进程才真的建 `NSStatusItem`。ctest 经 SSH 跑在没有图形会话的机器上，建状态项既没有意义也可能失败。

## Alternatives considered

- **只靠系统输入源图标，引导用户把「英」加成输入源**：它最强的理由是零新代码、零新偏好，系统图标、Ctrl+空格切换条和输入源列表天然一致，而且代码已经在 develop 上。不选它：在 macOS 27 上必须由用户手动添加；报告人明确不想多一条输入源（按应用记忆输入源、系统切换提示）；这条路径从未在已安装的构建上验证过。它仍然保留，状态项与它并存。
- **由输入法进程自动启用「英」模式**：最强的理由是用户什么都不用做，系统图标就会跟随。不选它：macOS 27 上 `TISEnableInputSource` 对键盘输入模式无效，已有实测记录；即使在能启用的系统上，也会把报告人不想要的那条输入源硬塞进输入菜单。按需模式的启用规则见 [2026-10-08-macos-first-pick-opt-in-mode.md](../bug-fix/2026-10-08-macos-first-pick-opt-in-mode.md)。
- **状态项显示与系统图标相同的回退结果**：最强的理由是两处永远显示同一个字，不会让人困惑。不选它：状态项存在的意义就是反映真实状态，「双」「五」没有加成输入源时系统图标的「中」恰恰是不准确的那一个；维护者决定状态项显示真实方案。
- **状态项用文字标题而不是 TIFF**：最强的理由是不依赖 bundle 资源，字号可以随意调。不选它：十一张模式图标已经是为 16 点菜单栏绘制的模板图，复用它们才能和系统输入法图标、悬浮工具栏的角标长得一样，各版本的 bundle 由 `scripts/edition_bundle.py` 生成时图标也自动跟着变。
- **默认打开**：最强的理由是更多人能发现它。不选它：大多数人已经有系统的输入法图标，同时加了「英」输入源的人会看到两个相同的字，菜单栏空间在刘海屏上本来就紧；报告人也要求它可以关。

## Consequences

- **收益**：不需要任何额外输入源，单按 Shift 等全部中英文切换都能在菜单栏常驻看到；状态项与 HUD 互补，关掉 HUD 的人也有视觉反馈。共享偏好与能力位让 Tauri 设置页和原生设置窗口读写同一个值。账号设置同步只导出固定的一组键（`account/settings_sync`），不包括它，所以它不会被带到别的设备。
- **代价与已知上限**：`input_mode_status_item` 是 `Preferences` 的顶层字段，结构带 `deny_unknown_fields`，早于它的设置应用或 `msime-mcp` 会拒读写过这个字段的文档；正式包里输入法和设置应用版本一致，与加 `show_app_logo` 时的处理相同。`ime_mode_scope` 按应用记忆时，前台切到没有文本焦点的应用不会触发 `activateServer:`，状态项停在上一个应用的状态，这正是报告人提到的「没有输入框焦点」场景之一，设置页说明里写明了。`kTISNotifySelectedKeyboardInputSourceChanged` 是分布式通知，切到别的输入源后状态项的移除可能稍有延迟。macOS 26 起系统设置可以按应用隐藏菜单栏项，刘海屏菜单栏放不下时系统也会把它挤掉，这两种情况都只能在真机上确认。设置应用和输入法之间没有「偏好已保存」的跨进程通知，所以在设置应用里拨开关要等输入法下次获得焦点才生效；要做到立即生效，需要设置应用保存后发一条分布式通知（仿 `crates/host-macos` 里打字统计开关的 `notify_typing_statistics_enabled`），输入法在进程级收到后不依赖客户端读一次文档，这一步没有放进首版。首版没有点击菜单，MCP 也还没有暴露这个开关；要加时仿 `input_mode_hud` 在 `crates/mcp-server/src/preferences.rs` 里接。若日后证实加入「英」后系统图标在已安装构建上并不跟随，那是 #665 路径的缺陷，需要单独的 issue 修，与这个状态项无关。

## Verification

- Rust：`cargo test -p msime-client-core -p msime-host-api` 覆盖偏好默认值、缺字段读成关闭、往返，能力位只在 macOS 为真，以及经 C ABI 读到的能力值；`cargo clippy` 同两个包无警告。
- 前端：`apps/desktop/tests/settings/settings.test.tsx` 断言 macOS 下开关默认关闭、点开后保存 `input_mode_status_item: true`；`apps/desktop/tests/input/host-capabilities.test.tsx` 断言开关只在声明能力的宿主上出现。
- macOS 原生：`tests/input/InputModeIdentifiersTest.mm` 覆盖 full 和五笔版 info 的图标查找与各级回退；`tests/input/ShortcutTest.mm` 的 `TestInputModeStatusItem` 用替身断言关闭时只隐藏、打开立即显示、Shift 轻点在「中」「英」间切换、双拼显示「双」、⌃⇧E 显示「中」、关上立即隐藏、没有客户端时不显示，原生开关关上时没有客户端的控制器经 `appearanceChanged:` 立即隐藏、打开时不显示，以及原生开关与共享文档读写同一个值；`tests/settings/info_plist_icons.py` 检查最后的回退图标仍是 plist 声明的那张。
- 真机（图形会话）还没有做：浅色、深色菜单栏的外观，切到 ABC 后消失、切回出现，重启输入法进程后位置保持，以及加入「英」输入源后系统图标是否跟随。

[可编辑焦点限制](../feature/2026-10-24-macos-editable-shift.md) 只约束可选的单按 Shift，不改变本文的快捷键或状态归属规则。
