# Agent Note: 设置界面按平台归属，macOS 的 AppKit 设置窗口冻结为回退

Status: implemented

## Problem

仓库里有两套设置界面实现在同时长大。共享 React 设置页 `packages/ui/src/settings` 有 381 个文件、36,726 行（按 2446934521 的树统计，下同）；Windows 的 WinUI 3 设置程序 `platforms/windows/settings` 有 21 个文件、25,620 行，其中 `main.cpp` 19,724 行；iOS 的原生 App `platforms/ios/App/Sources`（设置之外也含对话、键盘等页面）有 82 个文件、19,749 行；Android 发布 APK 的设置页 `platforms/android/java/app/msime/android/home` 有 99 个文件、21,477 行；macOS 的 AppKit 设置窗口 `platforms/macos/src/settings` 有 14 个文件、7,250 行，加上 AI、翻译、语音服务、支持、词库、账号等独立窗口合计约 1 万行。

文档却写着「六个平台都接入同一份 React 设置界面」「各平台共用同一套页面」「Tauri/React 由原生宿主按需承载」（根 README、`docs/ARCHITECTURE.md`、几份 parity 文档、Android 与 iOS 的 README）。实际作为设置界面进入用户手里的 React 设置页只有 macOS、Linux、HarmonyOS 三个平台：Windows 的 18 页全部是 `PageHost::Native`（`platforms/windows/settings/SettingsNavigation.h`），托盘、开始菜单和不带面板的请求都打开原生设置程序；React 设置页虽仍随 Tauri 的 `MSIME.exe` 安装，直接运行 `MSIME.exe` 能看到（`tauri.windows.conf.json` 的 `main` 窗口，`lib.rs` 的 `second_launch_route` 回落到 `SurfaceRoute::Settings(None)`），但没有设置入口指向它；iOS 的 App、`SharedUI` 和键盘扩展里没有 WebView，Android 发布的 `build-apk.sh` 包不含 Tauri/WebView/React，带 React 的合包 `build-client-apk.sh` 不在任何发布 workflow 里。

后果是同一个设置功能被写三到四遍，而且没人说清哪一份是该写的。单按 Caps Lock 切换中英文在 2026-10-11 同一天写了三遍：ed0abf979d（macOS AppKit 设置窗口）、1eef012f73（Windows 原生设置）、3d1ced4434（共享 React 设置页），见[单按 Caps Lock 切换中英文](../feature/2026-10-11-caps-lock-language-switch.md)。五笔查码写了 316213d1aa（共享 React）和 297772b297（iOS 与 Android 原生，#7124），见[五笔反查](../feature/2026-10-11-wubi-reverse-lookup.md)。macOS 的 AppKit 窗口只在设置应用打不开时出现，10 月仍有 49 个提交动了 `platforms/macos/src/settings`，例如[原生候选设置页的顶部四标签](../feature/2026-10-13-macos-candidate-settings-tabs.md)。不把归属写下来，贡献者按文档会以为改了 `packages/ui` 就覆盖了六个平台，或者继续给 macOS 回退窗口加功能。

## Decision

设置界面按平台归属，与当前代码一致：

| 平台 | 设置界面 | 入口与承载 |
| --- | --- | --- |
| macOS | 共享 React 设置页 | 输入法菜单以 `--route=settings:<类别>` 拉起 Tauri 设置应用 `app.msime.macos`（`platforms/macos/src/core/DesktopSettingsLauncher.h`） |
| Linux | 共享 React 设置页 | IBus 与 Fcitx5 调 `msime-linux-settings`，由它启动 Tauri 外壳（`platforms/linux/data/msime-linux-settings.in`）；没有原生设置界面 |
| HarmonyOS | 共享 React 设置页 | `pages/Settings.ets` 用 ArkWeb 加载 `stage-settings.sh` 打包的 `rawfile/settings/index.html`，ArkTS 一侧只做原生桥 |
| Windows | 原生 WinUI 3 | `msime-client-settings.exe` 绘制全部 18 页；托盘、开始菜单和不带面板的请求都打开它（`platforms/windows/src/system/ShellSurfaces.h`） |
| iOS | 原生 SwiftUI | `platforms/ios/App/Sources/`，App 里没有 WebView |
| Android | 原生 Java | 发布 APK 的 `home/` 等目录；React 只在不发布的 Tauri 合包里 |

Tauri 在三个桌面平台上还承载共享面板：表情与符号、手写、屏幕键盘、语音、云词库、云剪贴板。Windows 原生设置窗口里这些面板的入口拉起 Tauri 的 `MSIME.exe`（`SettingsNavigation.h` 的 `shell_links`）。

一个设置功能因此要在 `packages/ui` 实现一次，再在每个原生设置平台（Windows、iOS、Android）各实现一次。每个平台那一份只能是界面：设置项的校验、默认值、持久化和业务逻辑必须放在共享 Rust 层（`crates/client-core`，经 `crates/host-api` 提供给原生宿主），原生界面调用它，不复制规则。Windows 原生设置窗口按页加的 C 接口就是这条规则的做法，例如[AI 页](2026-10-11-windows-settings-ai-host-api.md)、[账号页](2026-10-11-windows-settings-account-host-api.md)、[社区页](2026-10-11-windows-settings-community-host-api.md)、[语音页](2026-10-11-windows-settings-voice-host-api.md)用到的接口和[按路径读写偏好](2026-10-11-windows-settings-preference-paths.md)。

macOS 的 AppKit 设置窗口冻结为最小回退。`platforms/macos/src/settings/` 的 11 页原生设置窗口，以及 `AISettingsWindow.mm`、`TranslationSettingsWindow.mm`、`VoiceProviderSettings.mm`、`SupportWindowController.mm`、`DictionaryWindowController.mm`、SwiftUI 账号窗口等独立窗口，只在设置应用找不到、启动失败或 options 路径不是绝对路径时由 `DesktopSettingsLauncher.h` 的回退分支打开，另有以 `--preferences` 启动输入法 bundle 的独立设置窗口（`platforms/macos/src/input/input_method_main.mm`）。这些窗口不加新功能；macOS 上的新设置只进共享 React 设置页。冻结之前已经加进回退窗口的功能保留，不为冻结删除。

归属表和这两条规则的现行表述在 `docs/ARCHITECTURE.md` 的「设置界面按平台归属」一节，根 README、各语言 README、parity 文档和平台 README 都指向那里。

## Alternatives considered

- **全面原生：macOS、Linux、HarmonyOS 也各写原生设置。** 最强的理由是每个平台只剩一套设置实现，界面完全贴合系统，设置不依赖 WebView 或 Tauri 运行时；macOS 已经有 11 页、约 1 万行的 AppKit 设置，离全原生最近，改完还能顺带消掉 macOS 上 React 与 AppKit 并存的那一份重复。不选，因为 Linux 和 HarmonyOS 今天没有任何原生设置界面（fcitx5 插件声明 `Configurable=False`，`Settings.ets` 只是 ArkWeb 外壳），要为 IBus/Fcitx5 两个环境和 ArkTS 从零写全部页面；`packages/ui/src` 自 10-08 起有 451 个提交，是改动最多的设置代码，它服务的三个平台改成原生后，每个设置功能要写的份数从四份变成六份。
- **回归 React 统一：Windows、iOS、Android 也改回共享 React 设置页。** 最强的理由是每个设置功能只写一次，Caps Lock 那种一天写三遍的情况直接消失；`packages/ui` 已经有完整页面，Tauri iOS 工程和 Android 合包今天就能构建，技术上有现成的路。不选，因为三个平台的原生设置是刚做完的有意选择：Windows 09-24 用 WinUI 3 重建（41f15b5b22，#733），10-11 把剩下交给共享应用的页面全部搬成原生（#7129），10-15 又按 macOS 结构重做了框架（2d7ad264c3，#7978）；Android 10-06 重新设计了原生设置 App（56495eff8a，#3782）；iOS 一直是原生 SwiftUI。回归要放弃这三份原生代码（Windows 设置 25,620 行、iOS App 19,749 行、Android `home/` 21,477 行），并把 Tauri/WebView 带进两个现在都不含它的移动发布包。

## Consequences

- **收益**：文档与代码一致，贡献者看归属表就知道一个设置功能要改哪几处。原生设置平台保留各自系统的界面。macOS 回退窗口冻结后，macOS 上同一个设置只写一份（React），四份里少掉 AppKit 那一份。业务规则按这条规则下沉到 Rust 层后，四份界面读写的是同一份偏好与同一套校验；今天还有规则写在原生界面里，例如 Windows 语音页的服务列表与换服务规则（`VoiceSettings.h`）、账号页的昵称校验（`AccountPageModel.h`）和 Android 的语音服务列表（`VoiceProviderPolicy.java`）。
- **代价**：每个设置功能仍要写四份界面（`packages/ui`、Windows、iOS、Android），评审要逐个平台核对有没有漏。Rust 层没有承接的设置规则会在各平台界面里各写一份，开始漂移；把规则下沉到 `client-core` 是每个新设置功能的前置工作。macOS 用户落到回退窗口时看不到冻结之后新加的设置。
- **已知上限与重访信号**：冻结和「每平台只写界面」都没有脚本门禁，靠评审守住。出现以下任一情况时重访：同一设置在不同平台的行为因为规则写在界面层而出了不一致的缺陷；Tauri iOS 工程或 Android 合包开始随产品发布；macOS 设置应用启动失败、用户长期停在回退窗口的报告变多；某个原生设置平台的维护跟不上新功能，四份界面里经常缺一份。

## Verification

- Windows：`platforms/windows/settings/SettingsNavigation.h` 的 `pages` 全部是 `PageHost::Native`，`shell_links::all` 只有 5 个面板；`ShellSurfaces.h` 的 `shell_executable_names` 对空面板返回 `msime-client-settings.exe`，其余返回 `MSIME.exe`。`tests/ui/settings_navigation.cpp` 核对页面模型。
- iOS：`git grep -l -e "import WebKit" -e WKWebView -- platforms/ios/App platforms/ios/SharedUI platforms/ios/KeyboardExtension` 没有输出。
- Android：`.github/workflows/release-android.yml` 只调用 `platforms/android/build-apk.sh`，`.github/` 下没有任何 workflow 调用 `build-client-apk.sh`。
- HarmonyOS：`platforms/harmony/entry/src/main/ets/pages/Settings.ets` 用 `$rawfile('settings/index.html')` 加载设置页。
- macOS 冻结：`git log -- platforms/macos/src/settings platforms/macos/src/core/AISettingsWindow.mm platforms/macos/src/cloud/TranslationSettingsWindow.mm platforms/macos/src/voice/VoiceProviderSettings.mm` 在本篇之后只应出现缺陷修复，不出现新设置项。
