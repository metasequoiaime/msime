# Agent Note: macOS 接入 Vim 模式的无修饰 Esc

Status: implemented

## Problem

Windows 从 2026-10-09 起有 **Vim 模式**：`[input] vim_mode_apps` 列出的应用里，交给应用的无修饰 Esc 同时把水杉切到英文，Vim 用户可以立刻输入 `hjkl`。改动前 macOS 侧 **0 命中**：`grep -rni vim platforms/macos/` 为空，整个共享客户端也没有这个字段。

### Windows 行为规格（v0.11.1，检出 `target/ref/msime-windows`）

| 项 | Windows 实现位置 | 确切语义 |
|---|---|---|
| 偏好字段 | `installer/default_config/config.default.toml` `[input] vim_mode_apps` | 字符串数组，出厂 `[]`（关闭）；升级合并保留用户写的值 |
| 名单存储 | 数据目录 `config.toml` 的 `[input]` | 可执行文件**基名**（不带路径），ASCII 大小写不敏感；终端里的 Vim 要填终端宿主 |
| 名单解析 | `windows/src/Key/VimMode.cpp` `ParseStringArray` | 单层字符串数组，可跨行、带 `#` 注释、单双引号、`\\` 与 `\"` 转义；数字、嵌套数组、缺逗号、未闭合、不支持的转义一律**整表拒绝**，半开的配置不会悄悄生效 |
| 读取与缓存 | `windows/src/Utils/FanyUtils.cpp` `ReadConfiguredVimMode` | 最多每秒轮询；文件 mtime 或大小变化才重读；读失败保留重试标记；用当前宿主进程名匹配，空进程名视为不启用 |
| Esc 触发条件 | `windows/src/Key/VimMode.h` `ShouldSwitchToEnglish` | `key == 0x1B && !eaten && !repeat && modifiers == 0 && !winDown && !compositionActive && !deferredKeysPending` |
| 修饰键范围 | `CaptureIpcModifiers` + `GetAsyncKeyState` | 只认 Shift / Ctrl / Alt / Win；带任一修饰键的 Esc 保持原行为 |
| 触发动作 | `windows/src/Key/KeyEventSink.cpp` `ApplyVimModeEscape` | 仅当**当前是中文**（`GetIMEMode` 为真）才切：`SetIMEMode(FALSE)` 加 `SetPunctuationMode(FALSE)`。不改系统输入法，只改水杉自己的中英文与标点态 |
| Esc 去向 | `OnTestKeyDown`（`KeyEventSink.cpp:983`）与 `OnKeyDown`（`:1050`）两处调用 | Esc 原样交给应用，切英文是搭车；`OnTestKeyDown` 那处保证 `TestKeyDown` 返回 FALSE、TSF 省略 `OnKeyDown` 的宿主也能切 |
| 组字中 | `eaten` / `compositionActive` 两个入参 | 组字或候选开着时一律不切：Esc 先按现有逻辑取消组字，**取消后重新按一次**才切英文 |
| 长按 | `IsAutoRepeat(lParam)` | 长按 Esc 的重复事件不触发切换 |
| 在飞按键 | `_HasDeferredKeyBarrier()` | 有排队/在飞的异步按键时不抢先切，避免改变队列里按键的含义 |
| 冲突与作用域 | `ime_mode_scope` / 标点锁定 | 切英文后沿用现有 compartment 通路；`scope = global` 时英文态会同步到别的应用；进入 insert 模式不自动回中文 |
| 设置界面 | `server/src/settings/native/` **没有任何 vim 控件** | 这个版本只能手改 `config.toml`，唯一说明是 `docs/vim-mode.md`；`server/src` 全目录无 `vim` 命中 |

### 为什么 macOS 不能照抄

- **标识不同。** Windows 匹配前台进程的可执行文件基名；macOS 输入法宿主在独立进程里，进程名永远是水杉自己，只有前台应用的 `bundleIdentifier`（`InputController.mm` 的 `activateInputModeForApplication:`，存进 `AppearancePreferences._activeModeApplication`）能当标识。名单因此按平台分键，和 `app_input_mode_rules` 现在「macOS bundle id、Windows 进程基名」同款。
- **没有现成通路。** Windows 是 TSF DLL 自己把 `config.toml` 读成裸文本解析；macOS 宿主经 Rust 桥拿 `hostOptions[@"preferences"]`，字段必须先进 `crates/client-core/src/preferences.rs` 的 `Preferences`。改动前 `vim_mode` 只出现在 Windows 源码、Windows 测试、两份出厂模板和 `windows/CMakeLists.txt`，共享偏好契约里没有它。

## Decision

按应用例外的整套做法补齐，三层一起落地：

1. **共享偏好。** `Preferences` 新增 `vim_mode_apps: Vec<String>`：`#[serde(default, skip_serializing_if = "Vec::is_empty")]`，空名单不写进文档，没有这个键的旧版本照读。校验沿用应用例外的口径：每项非空、UTF-8 ≤ 64 字节、两侧无空白、无控制字符和 `\` `/`，按 ASCII 不分大小写去重，上限同一档 `MAX_APP_INPUT_MODE_RULES`（32），超限整份拒绝，对应 `PreferencesError::InvalidVimModeApps`。`host_surface.rs` 在 `app_input_mode_rules` 旁加同名能力位，`capabilities_for_platform` 只给 macOS：Windows 仍在 TSF 本地读它自己的名单，共享键不接它。
2. **macOS 宿主。** `AppearancePreferences.mm` 四处接上 `vim_mode_apps`：`SharedOverrideProperties()`、`SettingProbes()`、`-applySharedInputPreferences:`、`-sharedPreferencesByMerging:`（整张写回，空名单让文档里的名单消失）；`VimModeAppsKey` 注册进「应用例外」段，恢复该段默认时一起清空。新增 `-vimModeEnabledForApplication:`，把 `vimModeApps` 交给纯函数查。判定放在新 `platforms/macos/src/core/VimMode.h`，镜像 Windows 的 `VimMode::ShouldSwitchToEnglish`：`VimModeIdentifierEqual`（ASCII 不分大小写，与 `valid_app_input_mode_rules` 的 `to_ascii_lowercase` 和 `InputModeRuleIdentifiersEqual` 同一口径）、`VimModeListContains`（空标识不命中）、`VimModeEscapeSwitchesToEnglish`（`keyCode == 53 && !repeat && !modifierHeld && !compositionActive && listed && !englishMode`）。Windows 的 `eaten` / `deferredKeysPending` 没有对应物：Esc 会不会被引擎吃掉由 `compositionActive` 代答，组字三项（`editing_text` / `candidates` / `phrase_prefix`）全空时 `MSIME_CANCEL` 无事可取消，Esc 才确定回到应用；在飞按键由判定前的各早退分支覆盖。`InputController.mm` 的 `handleKeyEvent:client:` 在 `switch (event.keyCode)` 的 `case 53` 之前判定：sender 的 `bundleIdentifier` 命中名单时按当前 `_view` 组字态和 `_appearance.englishMode` 调 `setEnglishInputMode:YES`；判定在派发 `MSIME_CANCEL` 之前，组字开着时这次 Esc 只取消组字，不既取消又切。标点不额外处理：`setEnglishInputMode:` 已经按模式和 `punctuation_lock` 同步，等价于 Windows 的 `SetPunctuationMode(FALSE)`。
3. **设置界面与 MCP。** 共享设置页在「中英文」组的应用例外段旁新增「Vim 模式」名单段（`VimModeAppsSection` + `useVimModeApps`），复用 `app-input-mode-rules.ts` 的 `normalizeAppInputModeRuleId`、`appInputModeRuleProblem`（把名单看成全是 `chinese` 的规则表）和 `pickApplication` / `applicationNames` 契约，没有模式下拉；`preferences-model.ts` 加 `vim_mode_apps?: string[]`，`host-capabilities.ts` 加 `vim_mode_apps?: boolean`，`input-page.tsx` 按 `host?.vim_mode_apps === true` 挂入 `modeExtra`。MCP 的 `PreferencesView` / `PreferencesChange` 同 `app_input_mode_rules` 读写，整张替换、`[]` 清空、非法标识拒存。

macOS 的标识语义是**前台应用的 bundle id**，不是进程基名：共享设置页按用户键入或选应用得到的 bundle id 写入，查找与去重都按 ASCII 不分大小写。Windows 把 `config.toml` 的本地名单迁进共享文档与否留给维护者：迁则两边一份名单，不迁则 macOS 用共享键、Windows 用 `config.toml`，用户要维护两张表。

## Alternatives considered

- **macOS 本地 `NSUserDefaults` 键，不碰共享契约。** 最强的理由是改动最小、只动 macOS、不经过跨平台评审，Windows 那份现在也是本地键，看起来对称。不用它的原因是它会造出第二套「应用名单」：共享设置页看不到也改不了，Windows 写在 `config.toml`、macOS 写在本机默认值，用户要在两处维护两张表，两边的说法会很快不一致。[应用例外成为共享偏好](2026-10-10-windows-input-mode-parity.md) 已经因为同一理由否掉过「规则只放 Windows 本地」，这里沿用同一判断。
- **照抄 Windows 的 exe 基名匹配。** 逻辑可以逐行移植，但 macOS 上进程基名恒为水杉自己的输入法宿主，配了也永远不匹配，等于功能没做。
- **复用应用例外表：有 `app_input_mode_rules` 的应用按 Esc 就切英文。** 最省事，零新偏好、零新 UI。语义不同：应用例外决定「进入应用时的起始模式」，Vim 模式是「离开输入时切回英文」，把两者绑死会让只想固定起始模式的人也被改掉行为；而 Vim 用户要的正是同一个应用里中文和 normal 模式来回切换。
- **先只落 macOS，Windows 完全不动。** 可行，也是本次采用的范围；代价是长期保留两份名单/两种键，等维护者决定要不要把 Windows 一起迁进共享文档。

## Consequences

- 名单命中的应用里，无组字、无修饰键、非长按的 Esc 让 `_appearance.englishMode` 变为 YES，Esc 原样到达应用；切英文后再打中文需要用户自己切回或按既有中英文快捷键。
- 未在名单、带任一修饰键（Shift / Ctrl / Option / Command）、长按、组字或候选开着时按 Esc：行为与之前逐键一致，组字中的第一次 Esc 仍只取消组字。
- 名单为空、键缺失、类型非法、超出上限：一律不触发，不影响别的偏好；非法文档被 `InvalidVimModeApps` 整份拒绝。
- 「应用例外」段的恢复默认会同时清空 Vim 名单：两段都是按应用的输入行为规则，共用一个恢复入口。
- 共享设置页和 MCP 都能增删名单；macOS 原生设置窗口没有为 Vim 名单新增控件（与应用例外的表格分开，但它归在「应用例外」段恢复范围之内）。
- Windows 侧没有改动：它继续读 `config.toml` 的 `[input] vim_mode_apps`，能力位为 false，共享设置页不给它画这个段落。

## Verification

- `cargo fmt --all --check` 干净；`cargo test -p msime-client-core` 432 通过（含 `vim_mode_apps_stay_out_of_the_document_until_written`、`vim_mode_app_identifiers_are_validated`）；`cargo check -p msime-host-api -p msime-input-runtime -p msime-mcp-server --all-targets` 通过；`cargo test -p msime-mcp-server vim_mode` 通过（同包 `tests/stdio.rs` 的 `expand_and_config_print_lines_for_testing_by_hand` 在本机因词库 fixture 缺失失败，与本改动无关）。
- CTest `vim-mode`（`platforms/macos/tests/core/VimModeTest.cpp`，不依赖窗口）：先把 `VimModeEscapeSwitchesToEnglish` 打个 `&& false`，`Assertion failed` 于 `VimModeTest.cpp:26`、退出码 134；恢复后 `1/1 Test #50: vim-mode ... Passed`。
- `shortcut-test` 构建通过，`InputController.mm`（经 `ShortcutTest.mm` 的 `#import`）与 `AppearancePreferences.mm` 两处编译干净；`shortcut` 测试自身在本机已知因上游 `strokeColor` 断言失败，不作为本次判据。
- `pnpm --dir apps/desktop exec tsc --noEmit` 干净；vitest `tests/settings/vim-mode-apps.test.tsx` + `app-input-mode-rules.test.tsx` 共 23 个用例通过。
- `bash scripts/run-checks.sh` 退出码 0（396 项全过）；`pnpm run verify-notes` 通过。
