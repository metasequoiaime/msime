# Agent Note: macOS 共享账号页补上设置同步

Status: implemented

## Problem

#10747 的报告者在 macOS 0.57.0 上找不到「同步设置」。macOS 的账号入口从 41231ad88a（随 macos-v0.51.0 发布）起走共享设置页的 `settings:account` 路由，`InputController.mm` 的 `showAccount` 只有在设置应用起不来时才打开原生的 SwiftUI 账号窗口。设置同步却仍按旧分工只接在 Windows 和 Linux 上：`apps/desktop/src-tauri/src/platform/desktop.rs` 只在这两个平台编译 `desktop_settings_sync`，`apps/desktop/src/main.tsx` 只在这两个平台给账号客户端挂 `settingsSync`，注释写的是「macOS 由输入法自己的账号窗口同步它的原生设置」。入口改走共享页之后这个前提已经不成立，正常路径上的 macOS 用户既不能上传、应用设置，账号页的登录说明也不许诺同步设置。

client-services 的 `DesktopHost` 只有 Windows 和 Linux 两种，`CURRENT` 在 macOS 上读作 Linux，注释说「macOS 不走桌面导出，这里读作 Linux 不会被用到」。直接打开 cfg 能工作，但 macOS 会悄悄用 Linux 的键集，以后两边的设置页一分开就会出错。

维护者决定（#10747 的分拆）：A 部分当缺陷修，在 `packages/ui`、`apps/desktop` 里补上，与 Linux、Windows 的行为一致；B（本机设置文件导出、导入）随 macOS 原生设置迁移做；C（网盘同步设置和主题、改了马上同步）拆到 #10787。

## Decision

- `DesktopHost` 加 `MacOS`，`CURRENT` 在 macOS 上取它。导出和应用的键逐项核对过：各平台共有的 `input.*`，加上语法模型、快捷短语混输（两键）、符号模式、日期菜单、第二三候选（两键）和 `translation.ai`，与 Linux 相同。依据是 macOS 的共享设置页对这几项都有控件（快捷短语混输要 `HostCapabilities::quick_phrase_candidates`，macOS 声明了；符号模式、日期菜单在快捷模式列表里；第二三候选在「选词与翻页」组；语法模型在 `SentenceAssociationSection`；AI 候选翻译在翻译服务里，`ai_generation` 在 macOS 上为真），输入法运行时也都读（`platforms/macos/tests/settings/preference_coverage.py` 要求每个共享偏好要么被 macOS 读、要么登记为不适用，这几项都不在不适用表里）。共享层的宿主判断收进 `DesktopHost::shared_settings_page`，Linux 和 macOS 为真。
- 两个思考强度（`ai_assistant.thinking`、`voice_input.polish_thinking`）和三个切换热键（`keybindings.toggle_*`）macOS 能改也会读，这次仍不同步，见下面的备选。
- `platform.macos.*` 不读也不写。这是原生回退窗口的快照（`MacSettingsAccess`），旧版本上传的这一份留在账号文档里，由合并上传原样保留；候选窗口、主题皮肤、翻页快捷键这些设置仍不随共享页的设置同步。
- Tauri 在 macOS 上编译 `desktop_settings_sync` 并注册四个 `account_preferences_*` 命令；`main.tsx` 对三个桌面宿主都挂 `settingsSync`。卡片说明按宿主列出实际同步的项：Windows 只列各平台共有的几项和语法模型，Linux 和 macOS 再列快捷短语混输、符号模式、日期菜单、第二三候选和 AI 候选翻译，因为点「应用」时这些也会被云端值覆盖，说明不能比实际少（原先三个宿主共用一句只列共有项的说明，Linux 早就少写了这几项）。「已应用云端设置。」沿用桌面的写法：应用走与设置页保存相同的 `save_preferences_impl`，macOS 输入法像读设置页的保存一样读到新设置，没有要重新打开的键盘。
- 出厂默认值不变；Windows 和 Linux 的导出、应用分支不变。
- `docs/ARCHITECTURE.md` 里「macOS 上的新设置只进共享 React 设置页」与 11-03 的迁移约定冲突，改成按迁移约定：规则先进共享 Rust，界面随 macOS 原生设置迁移实现，迁移前共享页只修缺陷。B 部分按此归入迁移。

## Alternatives considered

- **只打开 cfg 和 `main.tsx` 的平台判断，`CURRENT` 在 macOS 上继续读作 Linux**：两行改动，今天导出的键也完全相同。否定：键集相同是两个设置页恰好同为共享页的结果，不是约定；macOS 迁到原生设置时会先改掉一部分控件，那时 Linux 的键集会让 macOS 上传它改不了的默认值、应用它没有开关的值。单独一个 `MacOS` 变体让以后改 macOS 的键集只动一处，并有自己的测试。
- **把 macOS 能改的 13 项全部导出**：macOS 是唯一能改思考强度和切换热键的桌面，同步它们最完整。否定：思考强度只在 DeepSeek 接口上生效，接口地址和凭据都不随设置同步，单独同步档位换到另一台机器上不一定有意义；切换热键应用时要和本机未同步的开关（Shift+空格、Ctrl+空格切换、语音热键等）查冲突，按键写进去后 `Preferences::validate` 可能拒绝整份文档，需要另写按键跳过的应用逻辑，超出这个缺陷修复的范围。
- **让共享账号页调原生窗口的 `platform.macos.*` 同步**：能连主题、候选窗口一起同步，对报告者「主题和各项设置都要同步」更近。否定：那套映射在 Swift 和 AppKit 回退窗口里（`cloudSettingsSnapshot`），ARCHITECTURE 规定回退窗口冻结、不加新功能，迁移期 `packages/ui` 只修缺陷；从共享页去调它要新开一条 Tauri 到输入法进程的通道，不是修缺陷的量。主题与皮肤的同步归 #10787 一起设计。

## Consequences

收益：macOS 的正常路径上又有设置同步卡片，登录说明也恢复「同步设置和词库」；macOS 与 Windows、Linux、Android、鸿蒙、iOS 之间的输入方案、繁简、双拼五笔版本、学习、调频和标点开始互通，与 Linux 之间还多出快捷短语混输等七项。

代价与缺口：旧版 macOS 原生窗口上传的 `platform.macos.*` 在共享页上不应用，用户以前用原生窗口备份的主题、候选窗口布局、翻页快捷键不会经这张卡片恢复；思考强度和切换热键不同步。账号文档里如果只有旧原生窗口上传的 `platform.macos.*`、没有任何 `input.*`，卡片的应用什么也不改。B（本机设置文件导出、导入）没做，随 macOS 原生设置迁移；C 在 #10787。

验证：`cargo test -p msime-client-services --lib settings_sync` 覆盖 macOS 的键集与 Linux 相同且不含 `platform.*`、macOS 往返、文档里的 `platform.macos.*`、思考强度和切换热键不动本机。真机验收（登录后看到卡片、上传后在另一台设备应用、应用后输入法行为跟着变）由 PR 的清单交给设备轮。

## Related notes

- [字段表新增的共享偏好只由能改它的宿主导出和应用](../architecture/2026-10-27-settings-sync-new-shared-keys-per-host.md)：部分重叠。本篇沿用它「只导出本机能改、运行时也读的键」的规则，给 macOS 加上自己的一档；它写的「macOS 上传 `platform.macos.*`，不经过这里」只对原生回退窗口仍成立。
- [macOS 原生账号页与云端窗口改调 Rust](../architecture/2026-10-18-macos-account-through-rust.md)：部分重叠。原生账号窗口的 `platform.macos.*` 同步不变，现在只是回退路径。
- [Linux 打开账号云功能](../feature/2026-10-20-linux-account-features-and-input-parity.md)：部分重叠。Linux 当时把 Windows 的设置同步搬到 `platform/desktop/`，本篇把同一个模块扩到 macOS。
- [全部界面改为原生，所有平台不再使用 WebView](../../proposed/architecture/2026-11-03-all-native-ui.md)：本篇按它的迁移期规则只修缺陷，B 部分归入它的 macOS 步骤。
