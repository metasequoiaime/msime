# Agent Note: Windows 从共享偏好读 Vim 模式名单

Status: implemented

## Problem

旧 Windows 仓库从 v0.11.1 起有 Vim 模式：TSF 自己读数据目录 `config.toml` 的 `[input] vim_mode_apps`，名单命中的进程里，交给应用的无修饰 Esc 同时把水杉切到英文。[macOS 接入 Vim 模式](2026-10-21-macos-vim-mode.md) 把名单做成共享偏好 `vim_mode_apps`，并以「Windows 仍在 TSF 本地读自己的名单」为由，只给 macOS 打开能力位。

到 monorepo 的 `platforms/windows` 里，这个前提已经不成立：`grep -rni vim platforms/windows` 为空，TIP 没有任何 Esc 判定，也不再读 `config.toml` 的 `[input]` 段；`app_input_mode_rules` 等偏好都改从共享偏好快照（`msime::tsf::cached_host_configuration()`）读。结果是 Windows 用户既没有这个功能，也没有地方配置它，而 `host_surface.rs` 的注释还在说 Windows 有本地实现。

## Decision

Windows 和 macOS 用同一个共享键 `vim_mode_apps`，标识是进程的可执行文件基名，与 `app_input_mode_rules` 在 Windows 上的口径相同。

1. **按键判定。** 新增 `platforms/windows/common/VimMode.h`，只依赖标准库和 nlohmann：`ReadVimModeApps` 从共享偏好读名单（不是字符串或为空的项跳过，整段不是数组时当作没有名单），`VimModeListContains` 复用 `SameAppInputModeRuleId`（ASCII 不分大小写，空进程名不命中），`VimModeEscapeSwitchesToEnglish` 要求 `virtualKey == 0x1B && !eaten && !autoRepeat && !modifierHeld && !compositionActive && !deferredKeysPending && listed && chineseMode`，与旧 Windows 实现的 `ShouldSwitchToEnglish` 同一组条件。
2. **TIP 接线。** `tsf/Utils/FanyUtils.cpp` 新增 `ReadConfiguredVimModeApp`，按 `Global::current_process_name` 从共享偏好快照查名单。`KeyEventSink.cpp` 的 `_ApplyVimModeEscape` 在两个入口调用：`OnTestKeyDown` 在立即分类（`_IsKeyEaten`）之后、排队分支已返回的位置；`OnKeyDown` 在 `_DispatchKeyDown` 之后，并在派发前记下 `_HasDeferredKeyBarrier()`。有的宿主测试后不再调用 `OnKeyDown`，有的不先测试就直接交来按键，所以两处都要判定；同一次 Esc 第二次到达时已经是英文，`chineseMode` 为假，什么也不做。修饰键包括 `CaptureIpcModifiers()` 的 Shift/Ctrl/Alt 和 `VK_LWIN`/`VK_RWIN`；组字态是 `_IsCompositionActiveForKeyGuard()` 或候选窗还在。切换调用 `SetIMEMode(FALSE)`，走开关 compartment：compartment 回调按标点锁定同步标点、通知状态，Server 的 `ModeAuthority` 把它当作用户手动切换，`ime_mode_scope` 和应用例外的让位规则都沿用这条通路。
3. **能力位和设置。** `capabilities_for_platform` 对 macOS 和 Windows 都给 `vim_mode_apps`，Linux 仍为假，因为它的两个引擎还没有 Esc 判定。WinUI 原生设置的「输入 → 中英文」组在「应用例外」下面新增「Vim 模式」行：输入框、「添加」按钮、每个程序一行带「移除」，写入前用 `AppInputModeRuleList.h` 的 `normalize_app_input_mode_rule` / `validate_app_input_mode_rule` 规范化和校验，与偏好库 `valid_app_identifier_list` 同一口径（64 字节、32 个、ASCII 不分大小写去重，另外要求 `.exe` 结尾）。共享名单里 macOS 的 bundle id 原样保留并列出。MCP 的 `vim_mode_apps` 字段说明改成两个平台。

## 与旧 Windows 实现的差别

- 名单不再在 `config.toml` 里手改，而在设置页或 MCP 里改；偏好发布后 TIP 的快照随之更新，不再按文件 mtime 每秒轮询。
- 旧实现在名单格式非法时整项视为关闭；现在格式由偏好库 `PreferencesError::InvalidVimModeApps` 在写入时整份拒绝，TIP 读到的总是校验过的文档，`ReadVimModeApps` 只防手改坏的文件。
- 旧实现另外调用 `SetPunctuationMode(FALSE)`；现在只写开关 compartment，标点由 compartment 回调按 `ResolveImeModePunctuationOpen` 同步，与 Shift 切换同一条路，标点锁定也由那里处理。

## Alternatives considered

- **恢复旧实现：TIP 自己解析 `config.toml` 的 `[input] vim_mode_apps`。** 最强的理由是旧 Windows 用户的配置文件原样可用，代码可以逐行搬回来。不用它是因为 monorepo 的 Windows 已经不读 `config.toml` 的输入偏好，恢复它等于为一个键重新引入手写 TOML 解析和文件轮询，而且会出现两张名单：macOS 和设置页写共享文档，Windows 读本地文件。[macOS 那篇](2026-10-21-macos-vim-mode.md) 已经因为同一理由否掉了「每个平台各存一份」。
- **只做 TIP，不做设置界面。** 改动小，用户可以经 MCP 写名单。不用它是因为 Windows 的设置窗口是原生 WinUI，共享 Tauri 页不在 Windows 上展示；不加原生行，普通用户就没有入口，功能等于没有。
- **在 `OnKeyDown` 一处判定。** 更简单，但 TSF 宿主在 `OnTestKeyDown` 返回 FALSE 后可以不再调用 `OnKeyDown`，Esc 正是最常见的交还应用的键，只在一处判定会在这类宿主里漏掉；旧实现也是两处。

## Consequences

- Windows 上名单命中的应用里，无组字、无修饰键、非长按、没有在飞按键时的 Esc 让水杉切到英文，Esc 原样到达应用；进入 insert 模式不自动回中文。
- 组字或候选开着时第一次 Esc 只取消组字；长按、带修饰键、未在名单、有在飞按键时行为不变。
- 名单按宿主进程匹配：VS Code 的所有窗口都属于 `code.exe`，终端里的 Vim 要填终端程序，终端里的其他程序也会受影响。
- Windows 7 的 Win32 设置（`settings/win7`）没有这一行；TIP 的行为在 Windows 7 上同样生效，名单可以在 Windows 10/11 的设置或 MCP 里写，跟随共享文档同步。
- Linux 的能力位仍为假，设置页不显示名单。

## Verification

- `windows-vim-mode`（`platforms/windows/tests/input/vim_mode.cpp`）在 macOS 上以 `clang++ -std=c++20 -Wall -Wextra -Werror` 编译运行通过；把判定里的 `&& key.chineseMode` 去掉后在第 85 行失败，退出码 1。
- `msime-tsf-vim-mode-wiring`（`platforms/windows/tsf/tests/input/vim_mode_wiring.cpp`）核对两个入口和读取函数的接线，通过；撤掉 `KeyEventSink.cpp` 的改动后 10 项断言全部失败。
- `cargo test -p msime-client-core host_surface` 覆盖能力矩阵。
- CI 的 Windows GNU cross build 编译了 TIP DLL，Windows native settings 用 MSVC 编译了 WinUI 设置，并在 `settings_ui.ps1` 里断言「中英文」标签有 Vim 名单。
- 实机验收在 pve9 的 win-runner（Windows 11 Pro）上做：安装 release-windows 以 `publish=false` 构建的未签名 full 包（develop `05562d1686`），用计划任务在控制台会话里跑脚本。名单经设置页的「Vim 模式」行（UI Automation）增删；按键用 SendInput 发给新开的记事本窗口，打 `ni` + 空格后读回文档文本判断中英文。以下 8 个场景全部通过：
  - 不在名单时 Esc 不切；
  - 加进名单后无组字的 Esc 切到英文；
  - 组字时第一次 Esc 只取消、第二次才切；
  - Shift+Esc 不切；
  - 组字时按住 Esc，重复事件不切；
  - 上屏后按一次 Esc 切到英文；
  - 英文时按 Esc 保持英文；
  - 从名单移除后 Esc 不再切。
  名单增删都不用重启记事本。结果见 PR #11178 的评论。
- 仍未验证：真实 Vim 编辑器（VS Code Vim 插件、gvim、终端里的 Vim）里 Esc 进入 normal 模式这一步，以及 Windows 7。
