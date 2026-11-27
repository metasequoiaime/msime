# Agent Note: macOS 声明 `TICapsLockLanguageSwitchCapable`，让「使用大写锁定键切换 ABC」作用到水杉

Status: implemented

## Problem

#11800：macOS 系统设置 → 键盘 → 文字输入 → 输入法 → 编辑 里的「使用大写锁定键切换 ABC」只对声明了 `TICapsLockLanguageSwitchCapable` 的输入源生效。水杉的 `Info.plist.in` 没有这个键，TIS 把它读成 `kTISPropertyInputSourceDoesCapsLockSwitch = false`（本机用 TIS 实测：水杉为假，同机安装的 AIME、豆包、清简为真），于是系统这层切换完全不认水杉：在水杉里短按 Caps Lock 只会打开大写锁定，切到 ABC 之后按 Caps Lock 也切不回来。

用户把水杉自己的「单按 Caps Lock 切换中英文」（`keybindings.switch_language_caps_lock`，见[单按 Caps Lock 切换中英文](../feature/2026-10-11-caps-lock-language-switch.md)）关掉、想交给系统时，正好撞上这一格：两条路径都不生效。

## Decision

- `platforms/macos/Info.plist.in` 顶层加 `<key>TICapsLockLanguageSwitchCapable</key><true/>`。这是 TIS 的属性来源，苹果自家中文、日文输入法和鼠须管、豆包都这么写。
- 新增 `platforms/macos/tests/settings/info_plist_caps_lock.py` 与 ctest 目标 `info-plist-caps-lock`：这个键必须存在且是布尔真，`<false/>` 或字符串 `"true"` 在系统看来都等于没打开，而 plist 语法、安装、注册和现有检查都不会报。
- `platforms/macos/README.md` 里原本写「打开了系统的『使用大写锁定键切换 ABC』时……需要关掉它」，补成两种情况：关掉水杉单项后 Caps Lock 交给系统；两项都开时系统这层仍优先。

## Alternatives considered

- **只在运行时用 `TISSetInputSourceProperty` 写这个属性**：TIS 没有公开的 setter 覆盖这一项，且属性来自 bundle 声明，运行时写入不在设计里。
- **不声明，改成水杉自己监听 Caps Lock 并调用 `TISSelectInputSource` 切到 ABC**：需要枚举上一个输入源、维护自己的「上一次选的是谁」，而且用户开不开这条系统设置都不再受控，等于把系统的开关复制一份到宿主里。声明属性是系统本来就提供的接口。
- **连水杉内置的 `switch_language_caps_lock` 一起去掉**：两者语义不同（内置切的是水杉自己的中/英文模式，系统切的是输入源），去掉内置会改变现有用户的手感，需要单独授权。

## Consequences

- 未打开系统「使用大写锁定键切换 ABC」时行为逐字节不变。
- 打开它、且水杉内置切换关闭时，Caps Lock 按系统设置在水杉与 ABC 之间切换，不再只打开大写锁定。
- 两项都打开时系统这层优先，水杉看到的是轻点已被截走；想只用内置切换仍要关掉系统那一项，README 已写明。
- 未验证：拉丁模式（英文模式）下的表现、系统属性在已安装 bundle 上的刷新时机（需要重装或重新登录才能读到新 Info.plist）、macOS 27 真机上的实际切换。本机磁盘约 24 GB，未起 macOS 构建，也没有重装。

## Verification

- `python3 platforms/macos/tests/settings/info_plist_caps_lock.py platforms/macos/Info.plist.in` 通过；`info_plist_names.py` 在同一模板上仍通过。
- 未跑的：完整 `ctest`（新增目标只在 macOS 构建树里注册）、装机后系统设置里「使用大写锁定键切换 ABC」的实际切换。合并后请报告者在自己机器上重装并复验。
