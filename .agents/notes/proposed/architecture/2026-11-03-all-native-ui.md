# Agent Note: 全部界面改为原生，所有平台不再使用 WebView

Status: proposed

## Problem

[设置界面按平台归属](../../implemented/architecture/2026-10-15-settings-ui-ownership-by-platform.md)把现状写清楚了，但现状本身是一半原生、一半 WebView，而且两半都在长。2026-11-03 按 develop 697895e31b 盘点了六个平台的全部界面（每条结论都对照代码复核过）：

- 按键路径上没有 WebView。候选窗、模式提示、打字特效和双拼键位图在每个平台都是原生的：Windows 是 Server 里的 Win32 + Direct2D，macOS 是 IMK 里的 `MSIMECandidatePanel`，Linux 是 Fcitx5 classicui 和自绘 cairo 画布，Android 是 View，iOS 是 UIKit 键盘扩展，HarmonyOS 是 ArkUI 的 InputMethodExtensionAbility。
- 发布的 Android APK 和 iOS App 里没有一行 JS：Android 原生配套 App 约 4.1 万行，iOS SwiftUI 容器 App 约 4.8 万行。Tauri 的移动外壳从未分发。
- Windows 10/11 的设置是 WinUI 3（约 3.6 万行），但表情、手写、屏幕键盘、语音、云剪贴板、云词库、背单词 7 个面板路由拉起 Tauri 的 `MSIME.exe`（WebView2）。Win7 版的同一批面板已经是 `panels/win7` 的原生 Win32 实现。
- macOS 以 Tauri 应用 `app.msime.macos` 为设置和面板的主路径，IMK 进程里同时编译着一整套 AppKit/SwiftUI 回退（约 1.36 万行）。
- Linux 的设置和 8 个面板只有 Tauri（WebKitGTK），NVIDIA 上还要关合成和 DMABUF 才能用。
- HarmonyOS 的设置是 ArkWeb 加载的 React（`Settings.ets` 用 39 个同步方法的 `registerJavaScriptProxy` 桥接），输入法本体的面板是 ArkUI。

结果是同一个功能被写很多遍：设置界面 8 份、表情面板 8 份、本地剪贴板 8 份、语音界面 8 份、候选窗皮肤绘制 9 份、手写 7 份、屏幕键盘 6 份、账号 6 份、引导 6 份。WebView 没有把界面统一起来，只是在原生之外又多了一份。维护者要求二选一：要么全部 WebView，要么全部不用。

## Proposal

所有平台的所有界面都用原生实现，发布物里不再有 WebView 宿主。共享的只有 Rust：设置项的校验、默认值、持久化、页面可见性和业务规则放在 `crates/client-core`、`crates/client-services`、`crates/client-providers`，经 `crates/host-api` 的 C ABI（Android 经 JNI，HarmonyOS 经 NAPI）提供给各平台，各平台那一份只画界面。

2026-11-03 维护者拍板的事实与取舍：

1. Windows 设置保持 WinUI 3，不换成 WebView2。
2. Win7 产品继续发布。
3. Windows 的面板不用 WebView，改为原生。
4. HarmonyOS 已经发布，按正式平台对待。
5. Tauri 移动外壳从未对外分发，可以直接删除。
6. Windows 面板用 Win32 加 msimeui 的 Direct2D 自绘，Win10/11 与 Win7 共用一份，不再给 Win10/11 单写 WinUI 3 面板。`platforms/windows/src/panels` 和 `msimeui/demos` 里的 D2D 面板是起点，不删。
7. macOS 最低版本从 13 提到 14，以便直接复用 iOS 的 SwiftUI 代码（iOS 侧有 25 个文件用了 iOS 17 的双参数 `onChange`）。
8. 平台顺序：Windows → macOS → HarmonyOS → Linux。

### 迁移期规则

从本篇起，新的设置功能先把规则写进共享 Rust，再在已有原生设置的平台（Windows、iOS、Android）和已迁完的平台上实现界面；`packages/ui` 只修缺陷，不加新设置。不这样做，追赶的目标一直在移动：`packages/ui/src/settings` 近 30 天有八百多个提交。代价是 macOS、Linux、HarmonyOS 的用户在各自迁完之前拿不到新设置。

### 各平台的终态和步骤

工期是单人常规速度的估算，合计约 143.5 周，评审认为偏低；按仓库历史速度，日历时间可能是它的 1/3 到 1/5。

- **共享清理**（约 1 周）：删除 Tauri 移动外壳（`gen/android`、`gen/apple`、`crates/tauri-mobile-platform`、`crates/ios-native-ffi`、移动端 Rust 和只服务它的 Kotlin），先把 Gradle wrapper 迁到 `platforms/android/gradle-app`。
- **Windows**（约 11 周）：面板改走 `panels/win7` 那套原生宿主，在现代版构建里编成中性名字的 `msime-client-panels.exe`，`ShellSurfaces.h` 的 `shell_executable_names`、WinUI 设置的 `shell_executable()` 和托盘、工具栏的可用性判断都指向它。补齐与 React 面板的差距：18 键分组按键（`crates/host-windows/src/grouped_panel_key.rs` 先导出到 C ABI）、长按连发、表情网格与清除最近使用、手写重做与只复制模式、语音的识别语言与 Esc 取消、背单词朗读改用 SAPI、按显示器感知 DPI、深色模式、单实例复用窗口。只在新系统需要的能力用运行时加载隔离，保证 Win7 不受影响。完成后安装包不再带 `MSIME.exe`，也不再检测 WebView2。第一步（现代包带上 `msime-client-panels.exe`，用 `MSIME_WINDOWS_PANEL_HOST=native` 切换，缺省仍是 `MSIME.exe`）已落地，见[现代 Windows 包的原生面板宿主开关](../../implemented/feature/2026-11-03-windows-native-panel-host-switch.md)。
- **共享 Rust 下沉**（约 8 周）：把 `apps/desktop/src-tauri` 里与 Tauri 无关的后端抽成独立 crate（IMK 安装与修复、数据目录、资源包、Linux 首次配置、剪贴板历史、背单词、语音、更新检查等），把只写在 TS 里的规则（页面投影与可见性、引导默认值、AI 服务商目录、候选布局选项）移进 `client-services`，对应的 vitest 用例改写成 Rust 测试或共用 fixture。
- **macOS**（约 29.5 周）：先把 `MSIMEAppearancePreferences` 拆成纯模型和旧 UI（`InputController.mm` 有 382 处访问），再把表情、手写、云剪贴板、云词库面板改为 IMK 内的原生默认路径并补齐差距，然后抽出 iOS 与 macOS 共用的 SwiftUI 包，用沿用同一 bundle id、签名和 Sparkle appcast 的原生容器 App 接替 Tauri 版（先只发 beta 通道），最后逐页原生化设置。
- **HarmonyOS**（约 47 周）：先把 `SettingsBridge` 重构成带类型的 ArkTS 服务，搭 ArkUI 外壳，还没原生化的页暂时用 Web 组件加载原来的 rawfile，再逐页换成 ArkUI，每一页都能单独发布。
- **Linux**（约 43 周）：先用 gtk4-rs 做面板和进程外壳，再做首次配置页和设置窗口，在 GNOME、KDE、deepin、niri、Omarchy 上逐一验证。
- **收尾**（约 3 周）：迁走 `@msime/web-engine` 对 `packages/ui` 的文件依赖（`theme-catalog.json`、`global-theme.ts` 镜像、`custom-theme-parity.json`），再删除 Tauri、`packages/ui`、`apps/desktop`、`apps/harmony`。`@msime/web-engine` 是独立的 npm 产品，不在本篇范围内。

## Alternatives considered

- **全部改为 WebView。** 规则是输入法进程之外、与按键无关的界面都用 `packages/ui` 这一份 React，Android 和 iOS 照 HarmonyOS 的做法由原生 App 承载系统 WebView 并经 JS 桥调用现有能力代码。它最强的理由：三位评审（成本、质量、维护）合计 15.5 分对原生的 12.5 分，原生只在质量一项领先；单人工期约 58 周，原生约 143.5 周；设置界面从 8 份降到 2 份（React 和 Win7），而原生方案做完后每个功能的份数基本不降；`apps/desktop/tests` 的 12.1 万行 vitest 是各平台原生测试共用的规格来源，ArkUI 在 CI 里编译不了。不选，因为维护者不接受 Windows 设置从 WinUI 3 换成 WebView2（失去 Fluent 外观和原生 UIA 无障碍，设置从此硬依赖 WebView2 运行时），也不接受 Windows 面板用 WebView；而 Windows 一旦是原生，全部 WebView 的规则就在 Windows 上破了，剩下的只是按平台划分的另一种一半一半。
- **维持现状，按平台划分归属。** 这是[设置界面按平台归属](../../implemented/architecture/2026-10-15-settings-ui-ownership-by-platform.md)的决定，它的理由是三个平台的原生设置是刚做完的有意选择，不该放弃。本篇沿用这个理由，只是把方向从「按平台各选一种」改成「全部原生」：那篇的重访信号之一「某个原生设置平台的维护跟不上新功能，经常缺一份」已经出现，而且 WebView 那一半自己也在分叉（macOS 的 Tauri 主路径和 IMK 回退各一套）。

## Acceptance criteria

- Windows：安装包不含 `MSIME.exe`，`msime_setup.iss` 不再检测 WebView2；`ShellSurfaces.h` 的 `shell_executable_names` 对任何请求都不返回 Tauri 可执行文件。
- macOS：发布的 App 里没有 `WKWebView`（`git grep -e WKWebView -e "import WebKit"` 在 `platforms/macos` 和容器 App 里没有输出），`app.msime.macos` 的 bundle id、签名和 Sparkle 更新链连续，老用户能直接升级。
- Linux：七套打包配方都不依赖 WebKitGTK、libsoup、javascriptcoregtk。
- HarmonyOS：设置页没有 ArkWeb `Web` 组件（服务公告的 HTML 也改为原生渲染）。
- 仓库里不再有 `apps/desktop`、`apps/harmony`、`packages/ui` 和 Tauri 依赖，`@msime/web-engine` 的构建和发布照常通过。
- 每个设置项的规则只在共享 Rust 里有一份，各平台界面不复制校验和默认值。

## Risks

- macOS 容器 App 的连续性：`runtime-options.json` 放在设置 App bundle 的状态目录下，内置词书按设置 App bundle 定位，bundle id、签名或 appcast 任何一项断开，老用户的输入法就读不到配置、更新链也会断。App 只能整体切换，所以必须先在 beta 通道跑稳。
- 规则下沉改的是所有宿主共用的契约，包括 Windows 原生设置与 Win7 共用的页面模型头文件（Win7 以 `../` 引用了 44 个），必须做广度扫描。
- Windows 面板代码由 Win7 和 Win10/11 共用，每显示器 DPI、深色模式这类只在新系统需要的改动必须运行时加载或编译开关隔离，否则会弄坏 Win7。
- 富内容原生化成本最高：皮肤编辑器的实时预览要求各平台的候选渲染器脱离输入法进程运行，社区图片网格、公告 HTML 都要各写一份。
- HarmonyOS 的 `.ets` 只能在装了 SDK 的机器上编译，UI 在 CI 里跑不了，规模估计约 4.2 万行 ArkTS，误差 ±30%。
- Linux 屏幕键盘不抢焦点：GTK4 去掉了 accept-focus 和窗口移动，X11 靠 WM hints，wlroots/KWin 靠 layer-shell，GNOME Wayland 上做不到。
- 迁移期冻结 React 新功能，macOS、Linux、HarmonyOS 用户在迁完前拿不到新设置；冻结只靠评审守住，没有脚本门禁。
- 行数与行号来自 2026-11-01 的 develop，开工前要在当时的 develop 上重新核对。
