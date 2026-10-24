# Agent Note: 设置界面按平台归属，macOS 的 AppKit 设置窗口冻结为回退

Status: implemented

## Problem

仓库里有两套设置界面实现在同时长大。共享 React 设置页 `packages/ui/src/settings` 有 381 个文件、36,726 行（按 2446934521 的树统计，下同）；Windows 的 WinUI 3 设置程序 `platforms/windows/settings` 有 21 个文件、25,620 行，其中 `main.cpp` 19,724 行；iOS 的原生 App `platforms/ios/App/Sources`（设置之外也含对话、键盘等页面）有 82 个文件、19,749 行；Android 发布 APK 的设置页 `platforms/android/java/app/msime/android/home` 有 99 个文件、21,477 行；macOS 的 AppKit 设置窗口 `platforms/macos/src/settings` 有 14 个文件、7,250 行，加上 AI、翻译、语音服务、支持、词库、账号等独立窗口合计约 1 万行。

文档却写着「六个平台都接入同一份 React 设置界面」「各平台共用同一套页面」「Tauri/React 由原生宿主按需承载」（根 README、`docs/ARCHITECTURE.md`、几份 parity 文档、Android 与 iOS 的 README）。实际把 React 设置页当作设置界面的只有 macOS、Linux、HarmonyOS 三个平台，其中 HarmonyOS 还没有发布（`release-harmony.yml` 产出的是未签名 HAP，零售设备装不上）：Windows 的 18 页全部是 `PageHost::Native`（`platforms/windows/settings/SettingsNavigation.h`），托盘、开始菜单和不带面板的请求都打开原生设置程序；React 设置页虽仍随 Tauri 的 `MSIME.exe` 安装，直接运行 `MSIME.exe` 能看到（`tauri.windows.conf.json` 的 `main` 窗口，`lib.rs` 的 `second_launch_route` 回落到 `SurfaceRoute::Settings(None)`），但没有设置入口指向它；iOS 的 App、`SharedUI` 和键盘扩展里没有 WebView，Android 发布的 `build-apk.sh` 包不含 Tauri/WebView/React，带 React 的合包 `build-client-apk.sh` 不在任何发布 workflow 里。

后果是同一个设置功能被写三到四遍，而且没人说清哪一份是该写的。单按 Caps Lock 切换中英文在 2026-10-11 同一天写了三遍：ed0abf979d（macOS AppKit 设置窗口）、1eef012f73（Windows 原生设置）、3d1ced4434（共享 React 设置页），见[单按 Caps Lock 切换中英文](../feature/2026-10-11-caps-lock-language-switch.md)。五笔查码写了 316213d1aa（共享 React）和 297772b297（iOS 与 Android 原生，#7124），见[五笔反查](../feature/2026-10-11-wubi-reverse-lookup.md)。macOS 的 AppKit 窗口只在设置应用打不开时出现，10 月仍有 49 个提交动了 `platforms/macos/src/settings`，例如[原生候选设置页的顶部四标签](../feature/2026-10-13-macos-candidate-settings-tabs.md)；[正常候选设置入口显示顶部四标签](../bug-fix/2026-10-13-macos-primary-settings-tabs.md)记下了这样走错的一次：标签先只加进了回退窗口，而菜单的正常入口打开的是共享 React 页，用户看到的仍是长列表。不把归属写下来，贡献者按文档会以为改了 `packages/ui` 就覆盖了六个平台，或者继续给 macOS 回退窗口加功能。

## Decision

设置界面按平台归属，与当前代码一致：

| 平台 | 设置界面 | 入口与承载 |
| --- | --- | --- |
| macOS | 共享 React 设置页 | 输入法菜单以 `--route=settings:<类别>` 拉起 Tauri 设置应用 `app.msime.macos`（`platforms/macos/src/core/DesktopSettingsLauncher.h`） |
| Linux | 共享 React 设置页 | IBus 与 Fcitx5 调 `msime-linux-settings`，由它启动 Tauri 外壳（`platforms/linux/data/msime-linux-settings.in`）；没有原生设置界面 |
| HarmonyOS | 共享 React 设置页 | 不经过 Tauri：`stage-settings.sh` 把 `apps/harmony` 构建成 `rawfile/settings/index.html`，`pages/Settings.ets` 用 ArkWeb 加载，ArkTS 一侧只做原生桥，经 NAPI 调 Rust |
| Windows | 原生 WinUI 3 | `msime-client-settings.exe` 绘制全部 17 页；托盘、开始菜单和不带面板的请求都打开它（`platforms/windows/src/system/ShellSurfaces.h`） |
| iOS | 原生 SwiftUI | `platforms/ios/App/Sources/`，App 里没有 WebView |
| Android | 原生 Java | 发布 APK 的 `home/`，另有 `settings/`、`account/`、`community/`、`statistics/`；React 只在不发布的 Tauri 合包里 |

Tauri 在三个桌面平台上还承载共享面板：表情与符号、手写、屏幕键盘、语音、云词库、云剪贴板，Linux 另有本地剪贴板面板（`SurfaceRoute::Clipboard`，`msime-linux-settings --panel clipboard`）。Windows 原生设置窗口里语音、屏幕键盘、手写、云词库和云剪贴板这 5 个面板的入口拉起 Tauri 的 `MSIME.exe`（`SettingsNavigation.h` 的 `shell_links::all`）；表情与符号面板由 Server 的托盘菜单和浮动工具栏拉起（`ShellSurfaces.h` 的 `shell_surface_request`），不经过设置窗口。

一个设置功能因此要在 `packages/ui` 实现一次，再在每个原生设置平台（Windows、iOS、Android）各实现一次。每个平台那一份只能是界面：设置项的校验、默认值、持久化和业务逻辑必须放在共享 Rust 层（偏好在 `crates/client-core`，账号、皮肤、词库等在 `crates/client-services`，语音与 AI 在 `crates/client-providers`，都经 `crates/host-api` 提供给原生宿主），原生界面调用它，不复制规则。Windows 原生设置窗口按页加的 C 接口就是这条规则的做法，例如[AI 页](2026-10-11-windows-settings-ai-host-api.md)、[账号页](2026-10-11-windows-settings-account-host-api.md)、[社区页](2026-10-11-windows-settings-community-host-api.md)、[语音页](2026-10-11-windows-settings-voice-host-api.md)用到的接口和[按路径读写偏好](2026-10-11-windows-settings-preference-paths.md)。

macOS 的 AppKit 设置窗口冻结为最小回退。`platforms/macos/src/settings/` 的 11 页原生设置窗口，以及 `AISettingsWindow.mm`、`TranslationSettingsWindow.mm`、`VoiceProviderSettings.mm`、`SupportWindowController.mm`、`DictionaryWindowController.mm`、SwiftUI 账号窗口等独立窗口，只在设置应用找不到、启动失败或 options 路径不是绝对路径时由 `DesktopSettingsLauncher.h` 的回退分支打开，另有以 `--preferences` 启动输入法 bundle 的独立设置窗口（`platforms/macos/src/input/input_method_main.mm`）。这些窗口不加新功能；macOS 上的新设置只进共享 React 设置页。冻结之前已经加进回退窗口的功能保留，不为冻结删除。

归属表和这两条规则的现行表述在 `docs/ARCHITECTURE.md` 的「设置界面按平台归属」一节，根 README、各语言 README、parity 文档和平台 README 都指向那里。

### iOS 昵称

iOS 的 `CommunityProfilePolicy` 只经既有 `msime_client_account_name` 取得规范化昵称、合法性与长度上限；昵称编辑页和保存入口消费同一份结果，不再用 Foundation 的空白集合、控制字符集合和本地常量重建规则。这个接口不创建会话、不访问文件和网络，可以同步用于输入提示；请求仍受共享 C 接口的 16 KiB 上限约束，失败时不能保存，不用 Swift 校验兜底。

这会统一既有差异：首尾 U+FEFF 与共享设置页一样去掉，含 U+200D 的组合表情与 Windows、Rust 一样允许；64 个 Unicode 标量的上限、C0/C1 控制字符拒绝由共享层决定。Swift 只为计数展示读取规范化结果的 `unicodeScalars.count`。iOS 产品已静态链接宿主库，适配器直接调用纯 C 接口并释放响应，不扩充只供 `shared/backend` 动态调用的函数表。

### Android 昵称

Android 的 `AccountNamePolicy` 经 `NativeClient.accountName` 和受 16 KiB 限制的 JNI 接口调用同一个 `msime_client_account_name`，只严格解析规范化结果、合法性与两个长度上限。`ProfilePage` 的提示和 UTF-16 输入过滤器取共享结果，`DeviceDataApi.rename` 再读取同一纯函数后提交 `normalized`；桥接失败不能保存，不保留 Java 昵称常量或控制字符校验兜底。

昵称编辑显式选择 `InputDialog.preserveWhitespace()`，让原文直接进入共享规则。通用对话框默认的 Java `trim()` 会吞掉 U+0000、U+001F 等首尾控制字符，若先做这一步，页面与保存入口就会验证不同的文本；其他输入框仍采用原有默认行为。规则测试放在产品进程的 `AccountNameChecks`，由既有 `SessionStoreDeviceSmoke` 调用，避免 JVM 的 `org.json` 桩和另一份 Java 规则掩盖 JNI 的实际行为。

### 共享 React 昵称编辑

共享 React 的 `useAccountRename` 通过必需的 `AccountClient.namePresentation` 读取规范化结果、合法性与两个长度上限。Tauri 的 `account_name` 直接调用 `client-services::account::name::present_name`，请求序列化后同样受 16 KiB 上限约束；Harmony 的 `accountName` 经 ArkWeb 注册和 NAPI 同步调用既有 `msime_client_account_name`。两条桥都不创建登录会话、不读文件或网络。

输入提示按输入原文和客户端实例隔离异步结果，等待或桥接失败时禁止保存，但保持输入框可编辑；同客户端已知的长度上限在下一次等待和失败期间保留。保存入口再次请求共享校验，只提交返回的 `normalized`；预检和改名响应都检查页面存活与客户端代次。输入控件使用 `maximum_utf16_units`，计数提示使用 `maximum_chars`，不再用 HTML 的 64 个 UTF-16 单元限制 Unicode 码点。界面只严格解析响应，不保留 JS 昵称规范化与合法性规则；同步的 `preferredAccountName` 展示回退仍属于后续迁移边界。

## Alternatives considered

- **React 保留同步 JS 校验。** 无须等待宿主回复，输入时能立即显示提示；但旧规则漏掉 C1 控制字符，HTML 的 64 单元限制还会截断允许的补充平面昵称。继续补字符表会保留另一份业务规则，因此接既有 Rust 纯函数，并明确等待和不可用状态。
- **Android 继续在 Java 维护昵称校验。** 不必为同步输入提示增加 JNI 调用；但旧实现只拒绝 C0 与 DEL，漏掉 C1 控制字符，ASCII 去空白也与共享 UI 集合不同。已有纯计算 C 接口可以直接复用，继续补字符表会制造新的规则归属。
- **iOS 继续用 Foundation 集合近似共享昵称规则。** 无须过 C 接口，页面代码较短；但 `controlCharacters` 会额外拒绝 U+200D，去空白集合也不等于共享设置页的集合。复用已服务 Windows 的纯函数，消除保存前后的平台分歧。
- **为昵称扩充 Apple 的 Rust 函数表。** 可以沿用后端动态桥和替身表；但这一片只消费已经静态链接 Rust 的 iOS 产品入口，`shared/backend` 的 macOS 动态库不使用该适配器，扩表会额外改变两个宿主的跨语言契约。保持适配器在 iOS 服务层，测试直接调用真实 Rust 规则。
- **全面原生：macOS、Linux、HarmonyOS 也各写原生设置。** 最强的理由是每个平台只剩一套设置实现，界面完全贴合系统，设置不依赖 WebView 或 Tauri 运行时；macOS 已经有 11 页、约 1 万行的 AppKit 设置，离全原生最近，改完还能顺带消掉 macOS 上 React 与 AppKit 并存的那一份重复。不选，因为 Linux 和 HarmonyOS 今天没有任何原生设置界面（fcitx5 插件声明 `Configurable=False`，`Settings.ets` 只是 ArkWeb 外壳），要为 IBus/Fcitx5 两个环境和 ArkTS 从零写全部页面；`packages/ui/src` 自 10-08 起有 451 个提交，是改动最多的设置代码，它服务的三个平台改成原生后，每个设置功能要写的份数从四份变成六份。
- **回归 React 统一：Windows、iOS、Android 也改回共享 React 设置页。** 最强的理由是每个设置功能只写一次，Caps Lock 那种一天写三遍的情况直接消失；`packages/ui` 已经有完整页面，仓库里也有 Tauri iOS 工程（`MSIME_IOS_TAURI_COMPONENT=1`）和 Android 合包（`build-client-apk.sh`）的构建脚本，看上去有现成的路；但两者都不在任何 CI 或发布 workflow 里，能不能构建、构建出来能不能用都未经 CI 验证。不选，因为三个平台的原生设置是刚做完的有意选择：Windows 09-24 用 WinUI 3 重建（41f15b5b22，#733），10-11 把剩下交给共享应用的页面全部搬成原生（#7129），10-15 又按 macOS 结构重做了框架（2d7ad264c3，#7978）；Android 10-06 重新设计了原生设置 App（56495eff8a，#3782）；iOS 一直是原生 SwiftUI。回归要放弃这三份原生代码（Windows 设置 25,620 行、iOS App 19,749 行、Android `home/` 21,477 行），并把 Tauri/WebView 带进两个现在都不含它的移动发布包。逐个功能的决定也一直落在原生这一边：[Windows 界面采用 macOS 的分组与选中样式](../feature/2026-10-15-windows-ui-macos-layout.md)考虑过让 Windows 承载共享 React 设置页，因为会动到已有的账号、社区和文件对话框路径而放弃；[Android 原生设置页可以选识别服务、填密钥](../feature/2026-10-12-android-voice-provider-settings.md)和[Android 原生设置页可以下载、选用和删除本地语音模型](../feature/2026-10-12-android-local-voice-models.md)都考虑过把共享 React 的对应部分嵌进 WebView，都因为原生应用的其余页面都是原生的、单独嵌一个 WebView 页要接上 Tauri 命令桥、工作量和范围远超补原生设置而放弃；[五笔反查](../feature/2026-10-11-wubi-reverse-lookup.md)因为 iOS 与 Android 的产品里没有 WebView，在两边各写了一个原生查码页。

## Consequences

- **收益**：文档与代码一致，贡献者看归属表就知道一个设置功能要改哪几处。原生设置平台保留各自系统的界面。macOS 回退窗口冻结后，macOS 上同一个设置只写一份（React），四份里少掉 AppKit 那一份。业务规则按这条规则下沉到 Rust 层后，四份界面读写的是同一份偏好与同一套校验；今天还有规则写在原生界面里，例如 Android 的润色方案（`VoicePolishSettingsPolicy.java`）；语音凭据预检已由 `client-providers` 经 `msime_client_voice_credential_issue` 提供，Java 只做 JNI 调用和严格结果解析；Windows 语音页的目录与换服务字段补丁已由 `client-providers` 经 C ABI 提供，账号页的昵称规范化、校验、默认显示名和长度上限由 `client-services` 经 `msime_client_account_name` 提供，原生页只转换字段、绘制并提交。Android 的 ASR、润色服务和豆包流式接口选择也由 `VoiceProviderCatalog` 经 JNI 读取同一共享目录，只在 ASR 列表前追加系统识别与本地模型；目录不可用时禁用选择入口，不复制一份默认列表兜底。Android 的语音接口预检也经 `msime_client_voice_endpoint_issue` 复用共享运行时 URL 规则和移动宿主输入限制，Java 不再自行判断地址。 Android 的凭据换槽和换服务时清空地址、模型也由 `provider_settings` 经 `msime_client_voice_provider_update` 的 JNI 适配提供；`clear` 为显式策略，现有宿主缺省的 `fill_defaults` 行为不变。
  共享凭据预检按 Rust 的 Unicode `White_Space` 规则处理“留空删除”，Android 原生页保存和展示凭据时经 `TextPolicy.trimUnicodeWhitespace` 使用同一字符集合归一化。Java `trim()` 漏掉非 ASCII 空白，`isWhitespace` 与 `isSpaceChar` 的并集又漏掉 U+0085、额外包含 U+001C–U+001F，因此不复用这两种近似规则；保留既有通用方法的其他调用者语义。JVM 回归覆盖差异字符及补充平面正文，设备回归用真实 JNI 核对预检接受的空白和带空白豆包密钥，与当前凭据、旧槽位的显示判断一致。
- **代价**：每个设置功能仍要写四份界面（`packages/ui`、Windows、iOS、Android），评审要逐个平台核对有没有漏。Rust 层没有承接的设置规则会在各平台界面里各写一份，开始漂移；把规则下沉到 `client-core` 是每个新设置功能的前置工作。macOS 用户落到回退窗口时看不到冻结之后新加的设置。[Windows 界面采用 macOS 的分组与选中样式](../feature/2026-10-15-windows-ui-macos-layout.md)以 macOS 的 `SettingsLayout.h`、`AppearancePreferences.mm` 为 Windows 设置的布局和信息结构依据，而这些 AppKit 窗口现在冻结了：之后 macOS 的新设置只进 React，Windows 再拿 AppKit 窗口当参照时，新设置项在那里找不到对应，参照物要改看共享 React 设置页的同一页。
- **已知上限与重访信号**：冻结和「每平台只写界面」都没有脚本门禁，靠评审守住。出现以下任一情况时重访：同一设置在不同平台的行为因为规则写在界面层而出了不一致的缺陷；Tauri iOS 工程或 Android 合包开始随产品发布；macOS 设置应用启动失败、用户长期停在回退窗口的报告变多；某个原生设置平台的维护跟不上新功能，四份界面里经常缺一份。

## Verification

- Windows：`platforms/windows/settings/SettingsNavigation.h` 的 `pages` 全部是 `PageHost::Native`，`shell_links::all` 只有 5 个面板；`ShellSurfaces.h` 的 `shell_executable_names` 对空面板返回 `msime-client-settings.exe`，其余返回 `MSIME.exe`。`tests/ui/settings_navigation.cpp` 核对页面模型。
- iOS：`git grep -l -e "import WebKit" -e WKWebView -- platforms/ios/App platforms/ios/SharedUI platforms/ios/KeyboardExtension` 没有输出。
- iOS 昵称：旧 Swift 规则的基线探针复现组合表情被拒绝、首尾 U+FEFF 未清除。`SkinCommunityAccountTests` 在 Xcode 27 / iOS 26.5 的临时干净模拟器上 15 项通过，其中新增三项直接调用真实 Rust 昵称接口，覆盖共享空白与控制字符、64 个 Unicode 标量（含补充平面与组合字符）、规范化后的提交和 16 KiB 请求上限；账号请求继续使用合成替身，不访问真实账号。`MSIMEApp` 原生模拟器构建通过；测试必须按 iOS README 保留 ad-hoc 签名与 App Group entitlement。
- Android 昵称：旧 JVM 规则的 C1 控制字符回归先失败；设备 `AccountNameChecks` 通过产品真实 JNI 覆盖共享空白、C0/C1、64 个码点与补充平面字符、严格响应类型、传输上限，以及原生输入控件保留原文和保存入口拒绝无效昵称。API 35 的 `SessionStoreDeviceSmoke` 整套通过；原生 `build-apk.sh` 构建、JNI 导出、arm64 ELF 和 16 KiB 段对齐检查通过。设备测试沿用既有 instrumentation，新增适配类同时登记在产品依赖源码清单中。
- Android：`.github/workflows/release-android.yml` 只调用 `platforms/android/build-apk.sh`，`.github/` 下没有任何 workflow 调用 `build-client-apk.sh`。
- React 昵称：旧 JS 规则的 C1 控制字符回归先失败。组件测试验证共享规范化结果、动态码点/UTF-16 上限、检查与不可用提示、旧输入与旧客户端回复隔离、保存前再次预检和同刻重复保存；桌面 `account_name` 两项单测直接调用真实 Rust，覆盖 BOM/空白、C1、64/65 个补充平面码点和序列化后 16 KiB 请求上限；JS 替身不重建校验。macOS 原生宿主构建通过。Harmony `arm64-v8a` 原生库和完整未签名 HAP 构建通过，检查包内包含两个原生库和当前设置页，ArkWeb 注册清单由 bridge parity 检查覆盖；没有以真实账号做设备改名验收。
- HarmonyOS：`platforms/harmony/entry/src/main/ets/pages/Settings.ets` 用 `$rawfile('settings/index.html')` 加载设置页。
- macOS 冻结：`git log -- platforms/macos/src/settings platforms/macos/src/core/AISettingsWindow.mm platforms/macos/src/cloud/TranslationSettingsWindow.mm platforms/macos/src/voice/VoiceProviderSettings.mm` 在本篇之后只应出现缺陷修复，不出现新设置项。
