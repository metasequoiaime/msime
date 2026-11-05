# Agent Note: 删除从未发布的 Tauri 移动壳

Status: implemented

## Problem

`apps/desktop/src-tauri` 除了 macOS、Linux、Windows 的设置外壳，还带着一套 Android 和 iOS 的 Tauri 工程：生成工程 `gen/android`、`gen/apple`，平台配置 `tauri.android.conf.json`、`tauri.ios.conf.json`，两个只为它们存在的 crate（`crates/tauri-mobile-platform` 注入平台能力，`crates/ios-native-ffi` 包住 Swift 的 `@_cdecl` 导出），`src/platform/{android,ios,mobile}` 三个目录里约三千行账号、社区和平台适配代码，以及 `lib.rs`、`voice.rs`、`clipboard_history.rs` 里成片的 `#[cfg(target_os = "android" | "ios")]` 分支。构建入口是 `platforms/android/build-client-apk.sh` 和 `platforms/ios/build-app.sh` 的 `MSIME_IOS_TAURI_COMPONENT=1` 分支。

这两个产物从来没有分发过（维护者 2026-11-03 确认）。发布的 Android APK 由 `platforms/android/build-apk.sh` 经 `platforms/android/gradle-app` 构建，iOS App 是原生的 `MSIMEApp`（`platforms/ios/project.yml`、`upload-testflight.sh`），两者都不含 Tauri，[设置界面按平台归属](../architecture/2026-10-15-settings-ui-ownership-by-platform.md)里早已写明。这一步也是[全部界面改为原生](../../proposed/architecture/2026-11-03-all-native-ui.md)的第一步共享清理。可这套工程一直要人维护：每次改 `lib.rs` 的命令都要顾及两个移动目标；`verify-local.sh` 专门有一段对 `aarch64-linux-android` 检查 `msime-desktop`；`default-members` 里的 `tauri-mobile-platform` 让裸 `cargo test` 也要拉起整套 Tauri 依赖；Android 宿主里留着两个没有任何门禁编译的 Kotlin 插件和只给它们用的 `KeyboardTryoutActivity`；几十处文档要在「原生包」和「合包」之间来回解释。更麻烦的是，原生的 `gradle-app` 一直借 `gen/android` 里的 Gradle wrapper 构建，CI、发版和 OCR 模块发布都依赖这份不属于自己的 wrapper。

## Decision

Tauri 只服务三个桌面平台；Android 和 iOS 的 Tauri 工程连同只为它们存在的代码一并删除，桌面外壳的行为不变。

- **删除**：`apps/desktop/src-tauri/gen/{android,apple}`、`tauri.{android,ios}.conf.json`、`crates/tauri-mobile-platform`、`crates/ios-native-ffi`、`src/platform/{android,ios,mobile}` 与对应的模块文件、`src/shared/mobile_ai.rs`（只有 iOS 用）、`platforms/android/build-client-apk.sh`、`AccountPlugin.kt`、`VoicePlugin.kt`（两者只由 Tauri 插件注册，`gradle-app` 不编 Kotlin）、`KeyboardTryoutActivity` 及其清单项、`build-app.sh` 的 Tauri 分支、`build.rs` 里链接 iOS SwiftRs 运行时导出的那一步、`verify-local.sh` 的「compile: android target」阶段。
- **Cargo**：两个 crate 退出 `members` 和 `default-members`；`msime-desktop` 去掉移动目标的依赖和开发依赖，`crate-type` 只留 `rlib`（`staticlib`、`cdylib` 只给 Xcode 和 Gradle 工程链接用）。`lib.rs` 等文件里只对移动目标成立的分支删掉，混合条件里去掉 `target_os = "android"`、`target_os = "ios"` 和 Tauri 的 `mobile` 别名，桌面三平台上求值结果不变。
- **Gradle wrapper**：`platforms/android/gradle-app` 有了自己的 `gradlew`、`gradlew.bat` 和 `gradle/wrapper/*`，逐字节取自原 `gen/android`（Gradle 8.14.3，分发包按官方 SHA-256 固定），Apache-2.0 文本和来源说明一并搬到 `gradle-app/gradle/`。`build-apk.sh`、`ci-platforms.yml`、`publish-android-ocr-module.yml`、两个 OCR 设备脚本和发版 skill 都改用它。
- **Android v1 会话分支**：`AccountSessionProvider` 原来在没有 v2 登录会话时去读 `msime_account_session_v1`，那份存储只有 `AccountPlugin.kt`（Tauri 合包）写过；原生 Java 从来只读不写（0c86d450f3 把唯一的读者也改走了）。插件删掉后这条分支在原生 APK 里不可达，`AccountSessionRoutingPolicy.Source`、`source()`、`legacyToken()` 和对应的 JVM 冒烟一起删掉，provider 只回答原生会话或「未登录」。
- **测试搬家**：`msime-desktop` 里三条验证 `mobile_voice_provider_configuration` 的用例测的是 `client-providers` 的共享逻辑（Android 键盘经 host-api 的 C 接口在用），搬进 `crates/client-providers/src/voice/provider.rs` 的测试模块，去掉 `ios_` 前缀；只测 iOS 专用函数的用例随函数删除。

### 留下没动的

- `packages/ui` 和 `apps/desktop/src` 里面向移动宿主的分支（`isMobileHost`、`mobile-host-services.ts`、`relay_unsupported` 文案等）不在这次删除范围，界面代码另行清理。
- 原生 Android 里依赖合包是否存在的入口（`home/ManagementUi.java` 和 `LexiconPage`、`AboutPage`、`AccountFragment` 里的 P21 入口）在原生 APK 里本来就不显示，`ChatFragment.standalone()` 也已没有调用方；它们是原生代码，留给单独的清理。
- 设备测试里操作 Tauri WebView 的套件（`SettingsDeviceSmoke` 默认模式、`SettingsLifecycleSmoke`、`TypingStatisticsDeviceSmoke` 的 React 部分）需要能跑设备的人确认去留，这次不动。
- `apps/desktop/src-tauri/icons/{android,ios}` 只被删掉的生成工程引用，属于图标资源，不在这次删除范围。
- iOS 原生 App 里的 `App/Sources/dictionary/TauriDictionarySnapshotBridge.swift`、`TauriPersonalDictionaryBridge.swift` 导出的四个 `@_cdecl` 符号原来只有 `ios-native-ffi` 调用，`.cargo/config.toml` 里为 Tauri 的 iOS cdylib 延后解析这四个符号的 `-U` 链接参数也只为它存在；它们现在都没有用处但也无害，删掉要连带改提交进仓库的 Xcode 工程，留给单独的清理。

## Alternatives considered

- **留着工程，只在文档里标「不发布」** — 最强的理由是零风险，将来想要移动端的 React 设置页还能直接打包。不用的原因是这正是现状：标了「只用于开发验证」之后它照样要维护、照样拖慢 `verify-local.sh` 和裸 `cargo test`，而[设置界面按平台归属](../architecture/2026-10-15-settings-ui-ownership-by-platform.md)已经定下 iOS、Android 用原生设置，这套工程没有要服务的产品。真要恢复，git 历史里是完整的。
- **只删生成工程，保留 `tauri-mobile-platform` 和 `lib.rs` 的移动分支** — 改动最小，`msime-desktop` 一行不动。不用的原因是没有生成工程这些代码就编不出任何产物，`cfg(target_os = "android")` 分支以后只会烂掉——`verify-local.sh` 的那段 Android 检查当初就是因为这些分支攒了十个编译错误才加上的。
- **gradle-app 改用系统安装的 Gradle，不提交 wrapper** — 少提交一个二进制 jar。不用的原因是 CI、发版和本机构建都假定有固定版本的 wrapper，换成系统 Gradle 要改所有调用处并在每台机器上装同一版本；搬过来的 wrapper 逐字节就是今天在用的那一份。

## Consequences

- **收益**：`msime-desktop` 只剩三个桌面目标，`lib.rs` 净少约七百五十行，`generate_handler!` 少了 66 个移动命令；裸 `cargo test` 的 `default-members` 不再拉起 Tauri；`verify-local.sh --quick` 少一个需要 NDK 的阶段；Android 只剩一个 APK 入口，文档不必再区分两个同名同路径的产物。
- **代价**：想在手机上看共享 React 设置页的开发验证路径没有了；需要时从 git 历史恢复。`relay_unsupported` 这类只有移动壳会返回的错误码，界面代码里暂时还认，等界面清理时一起去掉。
- **验证**：`rbuild` 在 Mac Studio 上跑了 `cargo check --workspace --all-targets --locked`（为 `msime-desktop` 放了占位的前端产物和 macOS 资源）、`msime-desktop` 与 `msime-client-providers` 的 clippy 和单测、`scripts/verify-local.sh --quick`，并经新 wrapper 跑了 CI 同款的六个版本 Java 编译和 `lintFullRelease`；`scripts/run-checks.sh` 的全部契约在本机跑过。`msime-desktop` 在 Linux 和 Windows 目标上的编译这次没有在本地验证：Studio 上没有 Docker 和 MinGW，`verify-local.sh` 的这两段都跳过了，留给 CI。
- **当场审计**：`.agents/notes/` 里另有三十多篇笔记提到合包、`tauri-mobile-platform` 或 `gen/apple`，都是当时的事实记录，决定本身不受影响，不改；只有两篇现行事实需要同步：[设置界面按平台归属](../architecture/2026-10-15-settings-ui-ownership-by-platform.md)的 Android 一行和 [Tauri CLI 只能由平台构建脚本驱动](../process/2026-10-16-tauri-cli-platform-gate.md)列出的平台脚本，已就地更新并链回这里。
