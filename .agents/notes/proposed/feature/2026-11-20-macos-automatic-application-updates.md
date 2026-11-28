# Agent Note: macOS 设置应用自动更新设计

Status: proposed

## Problem

#11681 希望 macOS 用户在设置应用内选择自动更新，应用发现新版后下载、校验并在用户允许的时机安装，避免手动打开发布页替换整套应用。当前仓库已经有 Sparkle 更新输入法本体和设置应用的能力，但两条更新路径的安装单位、用户确认和输入法占用状态不同，直接增加一个开关可能让设置应用和输入法 bundle 版本不一致，或在用户打字时重启。

## Existing notes audit

- [设置应用在后台检查并下载更新包](../../implemented/feature/2026-11-27-macos-settings-background-update-download.md)：部分实现本提案。按维护者在 #11681 的决定落地了第一步（默认关闭的 `update_auto_download`、主实例后台检查与下载、安装仍由用户触发并经管理员授权），开关界面、「安全时机」一段和 root 助手扩展仍留在本篇。
- [设置应用在应用内更新完整安装包](../../implemented/feature/2026-10-18-macos-settings-in-app-updates.md)：本提案部分翻转它。它的 Decision 规定「设置应用只在用户主动检查时启动更新器，避免多个共享面板进程各自检查更新」，本提案要让设置应用在后台检查与下载，下文「后台检查只在一个进程里跑」回答它的理由。实现 PR 要同步改写那篇的 Decision 并互链。
- [输入法本体的 Sparkle 更新](../../implemented/feature/2026-10-15-macos-sparkle-input-method-updates.md)：部分重叠，继续拥有已装输入法自己的后台下载与离开输入源时安装，本提案不改它的默认值。
- [macOS 的更新通道](../../implemented/feature/2026-10-25-macos-update-channels.md)：部分重叠，继续拥有通道选择与 appcast 命名，本提案沿用。
- [系统目录安装与 root 更新助手](../../implemented/bug-fix/2026-11-16-macos-system-directory-install-and-updater.md)：部分重叠。助手目前只替换 `/Library/Input Methods` 里已装的输入法，本提案是否扩展它取决于下文的产品决定。
- [macOS 只发 pkg](../../implemented/process/2026-11-02-macos-pkg-only.md)：相关事实来源，pkg 把 `MSIME.app` 装进 `/Applications`、归 root:wheel。
- [界面全部迁到原生](../architecture/2026-11-03-all-native-ui.md)：约束开关放在哪个界面，见下文「开关放在哪里」。

## Proposal

### 更新单元分三段

设置应用的自动更新不能只看 `MSIME.app` 一份 bundle。用户打字用的输入法不是 `MSIME.app/Contents/Resources` 里的嵌入副本，而是另装在 `~/Library/Input Methods` 或 `/Library/Input Methods` 的那一份（`apps/desktop/src-tauri/src/platform/macos/macos_input_source.rs` 的 `installed_bundle_path`、`SYSTEM_INPUT_METHODS`）。一次自动更新实际经过三段：

1. Sparkle 替换 `/Applications/MSIME.app`。这一步只换掉嵌入副本，不碰正在运行的输入法。设置应用与嵌入输入法同属一个发布单元，这一点由打包结构保证，不需要另做检查。
2. 新的设置应用下次启动时，`ensure_current` → `refresh_decision` 把嵌入副本同步到已装位置：用户目录走 `install_unlocked`，系统目录走 `refresh_system_install_silently` 并交给 root 更新助手。`refresh_decision` 从不降级，只在嵌入副本更新（或版本相同但源码修订不同）时替换。
3. 已装输入法自己的 Sparkle（`platforms/macos/CMakeLists.txt` 里 `SUEnableAutomaticChecks`、`SUAutomaticallyUpdate` 为 true）继续独立更新自己。

以已装输入法为准：它已被自己的 Sparkle 更新到比嵌入副本更新的版本时，第 2 段不能退回旧版本，这一点 `refresh_decision` 已经保证，实现 PR 要用测试锁住自动更新场景。验收看的是已装输入法（用户目录和系统目录各一次）在自动更新后的版本与替换时机，不是嵌入副本。

### 安装授权

`MSIME.app` 装在 `/Applications`、归 root:wheel（`platforms/macos/README.md` 的安装包一节），登录用户的设置应用写不了这个目录。仓库已经写明 Sparkle 遇到这种位置时会放弃静默安装、改成弹窗要管理员密码（`platforms/macos/src/core/SystemUpdateStaging.h`）。所以按现有安装路径，任何一次设置应用安装都要管理员授权，「下次启动时静默安装」做不到。

第一个实现切片只把检查和下载自动化：下载并验签完成后提示有新版，安装仍由用户点「现在安装」触发，并照常经过 Sparkle 的管理员授权。切片里不提供「下次启动自动安装」。如果产品要无授权安装，要沿用输入法的系统目录做法另开切片：Sparkle 以用户身份更新一份暂存副本，交给 root 助手（`crates/macos-system-updater`）核对团队 ID 和更大的构建号后替换 `/Applications/MSIME.app`。助手的 `Layout` 和 bundle 标识白名单都要扩展到设置应用，而这会扩大唯一特权部件的权限面，必须单独评审。

### 安全时机放在替换已装输入法的那一步

会打断输入的不是 Sparkle 替换 `MSIME.app`，而是第 2 段。`install_unlocked` 和 `refresh_system_install_silently` 都会调用 `stop_running_copies`，用 `/usr/bin/pkill -u <uid> -f <输入法可执行文件>` 直接结束当前用户的输入法进程，事先不检查组字或录音。现有的未完成工作检查（`platforms/macos/src/core/UpdateController.h` 的 `MetasequoiaUpdateUnfinishedWork`、`MSIMEUpdateWindowsHoldUserWork`）只存在于输入法进程，设置应用看不到这些状态。今天手动「检查更新」后的重启也走这条路径，自动更新只会让它更常发生。

因此「没有未完成工作」的门槛不放在设置应用安装之前，而是放在第 2 段：设置应用只负责把新 bundle 换到磁盘上，不再 `pkill` 正在运行的输入法；结束旧进程交给输入法自己，复用系统目录更新里已有的做法：发现磁盘上那份比正在运行的新，就在用户切走输入源、熄屏或闲置、且 `unfinishedWork` 返回 NO 时退出，让系统按需拉起新版本。输入法没有在运行时不需要结束任何进程。实现 PR 要同时改手动更新和自动更新这两条路径，并覆盖用户目录与系统目录两个分支。

### 后台检查只在一个进程里跑

[既有决定](../../implemented/feature/2026-10-18-macos-settings-in-app-updates.md)不让设置应用后台检查，理由是多个共享面板进程会各自检查。这一条仍然成立，所以后台更新器只允许在设置应用的主实例里启动，面板进程从不构造更新器。主实例启动后台检查前，先拿共享状态目录里的独占锁，拿不到就不检查；同一把锁覆盖检查、下载和安装之间的并发窗口，设置应用和输入法共用状态目录时也用它。Info.plist 里的 `SUEnableAutomaticChecks`、`SUAutomaticallyUpdate` 继续由 `platforms/macos/scripts/settings_updater.py` 写成 false，后台检查只由主实例在运行时按用户选择打开。

### 开关放在哪里

自动更新偏好、默认值和更新状态契约（检查、下载、待安装、失败状态的落盘位置）先写进共享 Rust（`client-core` / `client-services`，经 `host-api` 提供），各平台界面只读写这份契约。

开关的界面不放进 `packages/ui`：AGENTS.md 规定 `packages/ui` 只修缺陷、不加新设置，而 macOS 设置应用 `app.msime.macos` 的「关于」页目前正是 `packages/ui` 里的共享 React 页面。IMK 进程里虽然编译着一个原生关于页（`platforms/macos/src/settings/AppearancePreferences.mm` 的 `MSIMESettingsPageAbout`），但它属于输入法进程的 AppKit 回退，驱动的是输入法自己的 Sparkle，不能控制设置应用的更新器。所以开关跟随[全部原生](../architecture/2026-11-03-all-native-ui.md)里接替 Tauri 版的原生容器 App 一起做；在那之前要提前交付，必须先由维护者明确记录一次对 `packages/ui` 冻结的例外。

### 待产品决定

实现 PR 前要确定：开关默认值；首次启用时机；是否另开切片扩展 root 助手以支持无授权安装（不扩展时，每次安装都要管理员授权）；开关随原生容器 App 交付，还是申请 `packages/ui` 冻结例外提前交付。新的设置应用开关不能改变输入法后台更新的默认值，也不能把稳定版用户切到测试或 nightly 通道。

## Alternatives considered

- 只在现有「检查更新」按钮旁增加自动检查，不增加设置开关。优点是改动小，但不能满足用户选择自动下载安装的需求，也无法表达安装时机。
- 让输入法本体的 Sparkle 直接更新整个 `MSIME.app`。优点是沿用现有输入法更新链，缺点是输入法是 `LSBackgroundOnly` 进程，不能安全承载设置应用的安装确认和整包替换。
- 把更新迁移到 Tauri updater。优点是设置应用已有 Rust 命令和页面，缺点是会引入第二套签名与发布元数据，并破坏现有 Sparkle 版本比较和密钥连续性。
- 第一个切片就扩展 root 助手，让设置应用无授权安装。优点是体验最接近「自动安装」，缺点是扩大唯一特权部件的权限面，应等产品确认确实需要再单独做。
- 让设置应用经 IPC 询问输入法是否空闲，空闲才 `pkill`。优点是改动集中在设置应用，缺点是要新增一条跨进程查询，并且要定义输入法没回答或没在运行时怎么办；交给输入法自己在安全时机退出，复用的是已经验证过的系统目录更新路径。

## Acceptance criteria

- 设置应用能读取并持久化自动更新选择，偏好与默认值来自共享 Rust 契约，默认值经过单独产品确认。
- 只有设置应用主实例启动后台检查；同时打开多个面板进程时只发生一次检查。
- 自动检查遵循用户选择的更新通道和现有节流规则，下载包完成后能显示版本、签名和摘要状态。
- 安装由用户触发并经过管理员授权（扩展 root 助手的切片除外），取消授权后旧版本继续可用。
- 自动更新并重启设置应用后，已装输入法（用户目录和系统目录各验一次）升到新版本，且已被输入法自己的 Sparkle 更新到更新版本时不被降级。
- 设置应用安装时输入法正在组字或录音：组字和录音不丢失，旧输入法进程在用户切走输入源或闲置后才退出。
- 安装保留状态目录、词库、学习记录和个性化设置。
- 失败、取消、断网和关闭开关后，旧版本继续可用，重试入口明确。
- macOS 13 以上完成设置应用、输入法和真实编辑器验收，覆盖稳定版、测试版和 nightly 通道边界。

## Risks

- 设置应用、已装输入法和输入法自己的 Sparkle 三段更新时序不一致，会造成版本漂移或输入法加载旧资源；以已装输入法为准、从不降级是这里的约束。
- 启动同步若保留现有的 `pkill`，会在用户组字或录音时结束输入法，丢失用户工作。
- 每次安装都要管理员授权，用户可能把「自动更新」理解成无需确认的安装，文案要写清楚。
- 扩展 root 助手会扩大特权部件可替换的范围，白名单或构建号校验出错的后果比输入法更新更重。
- 更新开关和现有输入法自动更新策略含义相近，文案和状态若不区分会让用户误以为整套应用已经更新。
