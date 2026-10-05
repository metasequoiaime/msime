# 水杉输入法

[![Core CI](https://github.com/metasequoiaime/msime/actions/workflows/ci.yml/badge.svg?branch=develop)](https://github.com/metasequoiaime/msime/actions/workflows/ci.yml)
[![iOS CI](https://github.com/metasequoiaime/msime/actions/workflows/ci-ios.yml/badge.svg?branch=develop)](https://github.com/metasequoiaime/msime/actions/workflows/ci-ios.yml)
[![macOS CI](https://github.com/metasequoiaime/msime/actions/workflows/ci-macos.yml/badge.svg?branch=develop)](https://github.com/metasequoiaime/msime/actions/workflows/ci-macos.yml)
[![Android Release](https://github.com/metasequoiaime/msime/actions/workflows/release-android.yml/badge.svg)](https://github.com/metasequoiaime/msime/actions/workflows/release-android.yml)
[![iOS Release](https://github.com/metasequoiaime/msime/actions/workflows/release-ios.yml/badge.svg)](https://github.com/metasequoiaime/msime/actions/workflows/release-ios.yml)
[![macOS Release](https://github.com/metasequoiaime/msime/actions/workflows/release-macos.yml/badge.svg)](https://github.com/metasequoiaime/msime/actions/workflows/release-macos.yml)
[![Linux Release](https://github.com/metasequoiaime/msime/actions/workflows/release-linux.yml/badge.svg)](https://github.com/metasequoiaime/msime/actions/workflows/release-linux.yml)
[![Windows Release](https://github.com/metasequoiaime/msime/actions/workflows/release-windows.yml/badge.svg)](https://github.com/metasequoiaime/msime/actions/workflows/release-windows.yml)
[![HarmonyOS Release](https://github.com/metasequoiaime/msime/actions/workflows/release-harmony.yml/badge.svg)](https://github.com/metasequoiaime/msime/actions/workflows/release-harmony.yml)
[![License: GPL-3.0-only](https://img.shields.io/badge/license-GPL--3.0--only-blue.svg)](LICENSE)
[![GitHub stars](https://img.shields.io/github/stars/metasequoiaime/msime?style=flat)](https://github.com/metasequoiaime/msime/stargazers)
[![GitHub contributors](https://img.shields.io/github/contributors/metasequoiaime/msime?style=flat)](https://github.com/metasequoiaime/msime/graphs/contributors)

水杉输入法（MSIME）是面向 Android、iOS、macOS、Linux、Windows 与 HarmonyOS 的多平台中文输入法。六个平台各有自己的原生输入法宿主，都接入同一套共享输入运行时和同一份 React 设置界面；React 管理界面通过 Tauri 调用普通 Rust 业务库；输入算法由 msime-engine 提供。

项目主页：[msime.app](https://msime.app/) · [GitHub 仓库](https://github.com/metasequoiaime/msime) · [贡献者](https://github.com/metasequoiaime/msime/graphs/contributors) · [问题反馈](https://github.com/metasequoiaime/msime/issues) · [代码签名策略 / Code signing policy](https://github.com/metasequoiaime/msime-windows/blob/develop/docs/code-signing-policy.md)

## Star History

[![Star History Chart](https://api.star-history.com/svg?repos=metasequoiaime/msime&type=Date)](https://star-history.com/#metasequoiaime/msime&Date)

> **关于名称**：MSIME 是 Metasequoia IME（水杉输入法）的缩写，与 Microsoft IME 无关，也与微软没有任何关联。代码、包名和仓库名中的 `msime` 一律是这个含义。设置中的 `shuangpin_profile: microsoft` 是「微软双拼」方案，与小鹤、自然码、首道并列的四个键位方案之一，供习惯该键位的用户选择，同样不代表任何关联。

**输入法能看到你输入的一切，所以这个问题应该有一个能逐条核对的答案：[哪些数据会离开设备](PRIVACY.md)。**简短版本：默认配置下只有云联想一个功能会把正在组的拼音发出去（发给 Google 输入工具，可关闭），其余联网功能都要你自己填凭据才会工作。另有一条不携带输入内容的自有匿名使用统计：六个平台都会向 `api.msime.app` 发送每日活跃、会话和崩溃（错误摘要与去掉目录的调用栈）记录，默认开启，可以在设置的「匿名使用统计」里关闭——字段、逐平台差异和落盘位置逐条列在 [PRIVACY.md](PRIVACY.md#使用统计与崩溃上报默认开启可关闭)。仓库不接入任何第三方统计或崩溃上报 SDK。

## 模块边界

- `crates/client-core`：宿主无关的客户端业务——本地配置、固定资源分代安装、账号与会话、云与 AI、皮肤、词库、剪贴板、语音、翻译、打字统计；不依赖 Tauri、UI 或平台宿主。
- `crates/input-runtime`：会话编排、焦点取消、候选分页和带代次的选择；不复制 engine 的组词状态机。
- `crates/engine`（`msime-engine`）：纯 Rust 输入引擎——组合状态、各输入方案、词库查询与学习回放；由原 C++ MSIME-Engine 移植而来。
- `crates/host-api`：版本化 C 接口、线程绑定的会话句柄和显式响应释放，六个平台的宿主都链接它。
- `packages/ui`、`apps/desktop`：共享 React 设置页与 Tauri 承载层，各平台使用同一个 Rust 入口库、commands 与 React 页面；目录名沿用 desktop。Rust 入口按 `platform/{android,ios,linux,macos,windows,desktop}`、`shared/`、`tests/` 分层。**Tauri 是公共组件，不是任何平台的产品本体**：最终安装、启动、被系统识别为输入法的，始终是 `platforms/<os>` 下的原生宿主，Tauri/React 由它按需承载。
- `shared/apple/`：macOS 与 iOS 共用的 Foundation / Objective-C++ 桥接，不包含系统输入法入口。
- `platforms/`：各系统入口和适配层。Android、iOS、macOS、Linux、Windows 与 HarmonyOS 六个平台各自维护宿主边界，每个宿主都以该系统的原生方式被识别为输入法：Android 的输入法服务、iOS 的键盘扩展、macOS 的 InputMethodKit bundle、Linux 的 IBus 与 Fcitx5 入口、Windows 的 TSF DLL 与 Server、HarmonyOS 的 InputMethodExtensionAbility。共享输入算法和组合状态由 `crates/engine` 持有，宿主不复制。

## 平台目录与宿主形态

| 平台 | 入口目录 | 宿主形态与集成方式 |
| --- | --- | --- |
| [Android](platforms/android/README.md) | `platforms/android/` | 输入法服务 `app.msime.android.MSIMEInputService` 跑在 `:ime` 独立进程，Java 宿主经 `platforms/android/native/client_jni.cpp` 调 host-api；Tauri/React 设置与输入法同包不同进程，共享私有 files/bootstrap/state；手写走 ML Kit Digital Ink |
| [iOS](platforms/ios/README.md) | `platforms/ios/` | XcodeGen 从 `project.yml` 生成的原生 App 内嵌键盘扩展 `MSIMEKeyboardExtension`，两者通过 App Group `group.app.msime.ios` 共享状态；Swift 键盘直接调 host-api，手写走 ML Kit Digital Ink |
| [macOS](platforms/macos/README.md) | `platforms/macos/` | InputMethodKit bundle（产物名 `水杉输入法.app`，bundle id `app.msime.inputmethod.MetasequoiaIME`），Swift 后端编成 `MSIMEBackend.dylib` 随 bundle 分发，Sparkle 负责自动更新 |
| [Linux](platforms/linux/README.md) | `platforms/linux/` | IBus 与 Fcitx5 是两个并列的系统入口，链同一份 host-api ABI；在线联想、语音、剪贴板等能力由独立 provider 进程加 systemd 用户单元承载；CPack 出 TGZ 与 DEB |
| [Windows](platforms/windows/README.md) | `platforms/windows/`、`platforms/windows/tsf/` | 进程内 TSF DLL（`MetasequoiaImeTsf`）与进程外 `MetasequoiaImeServer` 经命名管道通信，另有 watchdog 与运行配置准备工具；候选窗口、悬浮工具栏与托盘菜单由 Direct2D/DirectWrite 的 `msimeui` 绘制；Inno Setup 安装器注册 TIP 并随包词库 |
| [HarmonyOS](platforms/harmony/README.md) | `platforms/harmony/` | ArkTS 的 `KeyboardExtensionAbility`（`module.json5` 声明 `type: "inputMethod"`）承载键盘，经 NAPI 模块 `libmsimeclient.so` 调 host-api；设置页是内联打包进 `entry/src/main/resources/rawfile/settings/index.html` 的同一套共享 React 页面 |

共享库可以加载进不同宿主进程；不要求启动 Tauri 才能输入。六个平台一致：产品形态是 `platforms/<os>` 的原生宿主，Tauri 只提供跨平台共享的功能与界面，不单独作为产品启动。跨进程的设置变更走显式的持久化与通知机制，不依赖进程内共享内存。

## CI 与发布

`develop` 上的基础检查由 `Core CI` 负责，实际执行的是 actionlint 的 workflow 校验和依赖审查；仓库的文本契约是 `scripts/` 下的那批 `test-*.py`，由 `scripts/run-checks.sh` 按文件名自动发现并执行，`Core CI` 的 contracts job 在 Linux 上跑它，`scripts/verify-local.sh` 在本地跑同一份脚本；缺少所需工具或参考仓库的检查在 CI 上打印 skipped 并通过。iOS 与 macOS 的编译与测试由各自的原生宿主 workflow 负责。Android、Linux、HarmonyOS 与 Windows 由 Native Platform CI 在对应平台或共享层有改动时运行，未改动时明确跳过；这一层覆盖宿主契约、JVM 冒烟、容器构建和交叉编译。设备级的输入验收由各平台 README 记录的设备套件和手动步骤承担，不由 CI 代替。

六个平台的发布完全独立：每个平台有自己的手动 release workflow、版本文件、并发组、构建产物和 GitHub Release tag。版本文件分别位于 `platforms/android/version.txt`、`platforms/ios/version.txt`、`platforms/macos/version.txt`、`platforms/linux/version.txt`、`platforms/windows/version.txt` 和 `platforms/harmony/version.txt`；tag 使用 `android-vX.Y.Z`、`ios-vX.Y.Z`、`macos-vX.Y.Z`、`linux-vX.Y.Z`、`windows-vX.Y.Z` 和 `harmony-vX.Y.Z`。发布 workflow 只创建 GitHub Release，不自动上传应用商店或使用签名凭据。

## 开发

贡献代码前请阅读 [架构说明](ARCHITECTURE.md)、[贡献指南](CONTRIBUTING.md)、[安全策略](SECURITY.md)、[网络请求与数据流向](PRIVACY.md) 和 [行为准则](CODE_OF_CONDUCT.md)。准备公开源代码或平台构建物时，再阅读 [开源发布清单](docs/open-source-release.md) 和[第三方组件清单](docs/third-party.md)；它们列出第三方通知、资源许可、敏感文件检查和验证边界。各平台宿主的构建、测试与安装细节以对应的 `platforms/<os>/README.md` 为准。制作音效包、音乐包、指令表、特效包、短语表、辅助码表、单词本或符号集，见[插件作者指南](docs/plugins.md)。

跨平台的本地验证入口是 `bash scripts/verify-local.sh`：`--quick` 只跑编译阶段，是合并前的门禁；无参数跑全量，包含 Rust 测试、fmt、clippy、依赖审计、前端 lint 与类型检查、各原生宿主的 ctest 以及整句转换与逐键延迟评测。每个阶段把失败的测试名与 `scripts/known-failures.txt` 比对，只对不在清单里的名字失败。`git config core.hooksPath .githooks` 可以把 `--quick` 挂到 pre-push 上。

```sh
cargo test -p msime-client-core --locked
cargo fmt --all --check
cargo clippy -p msime-client-core --all-targets --locked -- -D warnings
pnpm install --frozen-lockfile
pnpm --filter @msime/desktop test
pnpm build
pnpm tauri dev
```

桌面构建需要 [Tauri 平台依赖](https://tauri.app/start/prerequisites/)。`pnpm tauri build --debug --no-bundle` 构建不打包的开发二进制；面向用户的安装包由各平台自己的打包链产出——macOS 的 CMake bundle、Windows 的 `platforms/windows/installer/msime_setup.iss`、Linux 的 CPack（`-DMSIME_ENABLE_PACKAGING=ON`）、Android 的 `platforms/android/build-apk.sh`、HarmonyOS 的 `hvigorw assembleHap`、iOS 的 Xcode 工程。普通浏览器中只显示无法访问本地配置的提示，不模拟保存成功。

桌面设置的应用标识和默认应用数据目录按平台命名：macOS 是 `app.msime.macos`，Windows 是 `app.msime.windows`，Linux 是 `app.msime.linux`。偏好保存在其中的 `preferences.json`。也可用绝对路径环境变量 `MSIME_CLIENT_STATE_DIR` 指向隔离开发目录。macOS 原生宿主读取同一份配置并在当前组词结束后应用更新；多个设置窗口同时保存时通过 revision 检测冲突，用户须显式重新读取后决定是否覆盖。

Android 合包构建和设备测试见 [Android 宿主](platforms/android/README.md#tauri--react-共享设置合包)。Tauri 设置与原生 `:ime` 服务同包、不同进程，共享私有 files/bootstrap/state；关闭设置窗口不结束输入法进程。iOS 的产品宿主是 `platforms/ios` 的原生 App，它嵌入原生键盘扩展并通过 App Group 共享状态；Tauri/React 在 iOS 上只作为共享功能与界面的公共组件，不作为独立 App 启动。签名与设备安装步骤见 [iOS 宿主](platforms/ios/README.md)。

共享设置支持 shuangpin_profile：xiaohe（小鹤）、ziranma（自然码）、shoudao（首道）、microsoft（微软）。缺省按小鹤读取，未知值拒绝；设置页在非双拼方案下禁用此选择但保留已选值。方案更改沿用组词结束后替换 Engine 的规则。

Linux 本地构建、隔离 D-Bus / IBus 测试和安装后的首次配置见 [Linux 宿主](platforms/linux/README.md)。宿主支持设置文件自动重读，安装后由随装的 `msime-linux-setup` 备齐词库并准备运行配置；`msime-linux-setup --download` 按 `resources/desktop-dictionary.lock.json` 取回缺失词库，图形入口在缺少 `runtime-options.json` 时会打开同一套首次配置页。

功能对齐以 MSIME-Windows 的完整功能为行为基线：公共业务和界面逐项接入共享层与 Tauri，Windows 沿用 TSF DLL 与 Server 分进程的结构，但两者之间的协议只服务本仓库同版本构建，不再兼容 MSIME-Windows 的旧配置、共享内存、无版本握手和旧语音管道，Android、iOS、macOS、Linux 与 HarmonyOS 按各自系统能力适配。`scripts/test-reference-*.py` 把这条基线固化成可复跑的门禁——参考实现的出厂配置键、四个界面的机器可读能力清单、changelog 的每条特性、以及 `windows/`、`server/`、`ui/src` 下的每个源文件，都必须对应到本仓库的实现或一条写明理由的缺席记录。

## 输入引擎

```sh
cargo test -p msime-engine --locked
```

输入引擎是 workspace 里的 `crates/engine`，与其它 crate 一起构建，不需要额外拉取源码、也不需要 C++ 工具链；SQLite 由 `rusqlite` 的 `bundled` 特性编进去。行为基准是从原 C++ Engine 录下的 `crates/engine/tests/golden/`，录制方法见 `tools/engine-golden/README.md`。路径由宿主明确提供，字符输入是 engine 支持的 ASCII 动作。engine 与运行时测试使用真实 engine 而非替身，其中本地 Unicode 模式那组用例覆盖裸数字键归 engine 还是归候选选择的判定。

## 原生宿主接口

`cargo build -p msime-host-api --locked` 产出静态库和动态库。头文件为 `crates/host-api/include/msime_client.h`，C 消费示例为 `crates/host-api/native/native_smoke.c`。调用链为 C 宿主 → host-api → input-runtime → engine，无 Tauri 运行时依赖。

宿主创建会话后须显式传入焦点状态，将 `handled` 映射为系统吃键，将 `commit` 通过系统 API 上屏，将值快照渲染为候选。候选选择携带返回的 generation 和全局 index。全部会话操作在创建线程执行；过期、销毁或错误线程句柄返回错误。每个 UTF-8 JSON 响应必须通过 `msime_client_string_free` 释放一次。逐键延迟由 `crates/input-runtime` 的 `rerank_latency` 示例按帧预算测量，`scripts/verify-local.sh` 把它作为一个门禁阶段运行。

`msime_client_select_edge(session, generation, index, edge)` 是 ABI 1 附加接口，适配器与宿主库须成套更新。方向使用 `MSIME_FIRST_HAN` / `MSIME_LAST_HAN`；共享层核对候选所属会话、代次和当前页，engine 提取首／尾汉字并在成功后清空整个组合。候选没有汉字时返回未处理并保留组合，不自动选词或追加标点；这类有效调用仍更新视图代次，宿主后续操作必须使用新快照。非法方向和失效身份在状态推进前拒绝。Windows 适配库的配置键路由、无汉字回退与 TSF 回复编码都已接进生产 KeyHandler，对应的 `msime-tsf-client-key-router`、`msime-tsf-character-result` 和 `msime-tsf-engine-response` 等测试在 `platforms/windows/tsf` 的 ctest 里覆盖这条路径。

## 固定词库资源

`resources/desktop-dictionary.lock.json` 固定 `metasequoiaime/msime-dictionary` 已发布 `dict-v2.0.13` 的来源、长度和 SHA-256。其中 `msime-mozc_dictionary_oss_README.txt` 与 `msime-mozc_LICENSE.txt` 是日文词库的许可证全文（前者是 IPAdic 与 ICOT 的条款，后者是 Mozc 的三条款 BSD），`msime-scowl_Copyright.txt` 是 `msime-english.db` 里 SCOWL 词表的版权与许可声明，它们的条款都要求随数据一同分发，重新打包时不可省略；详见[第三方组件清单](docs/third-party.md#日文词库的分发义务)。首次下载约 189 MB。词库锁的 `source_commit` 记录这批词库是本仓库哪次提交用 `msime-dict-build` 产出的，其中 `msime-bigram.bin` 与 `msime-trigram.bin` 是整句词格仲裁的语言模型表。表缺失时 engine 不报错，只是整句路径不加权——候选照出，顺序变差，所以换词库版本时要确认 `crates/engine` 还读得了新表，并重跑句子转换评测。开发准备命令：

```sh
cargo run -p msime-client-core --example install_resources -- target/resources
```

安装器通过注入的流读取资源，限制长度并校验摘要；全部成功后才发布到内容标识目录。再次使用时检查缓存字节；损坏缓存报错，失败安装不替换旧代。这里只准备不可变发布资源，不激活现有输入法，不迁移用户学习数据。资源按独立文件下载，不整包解压归档。

`msime_engine::host::prepare_options` 是工作词库准备与学习回放的权威入口，调用方必须先验证资源并暂停相关会话。以下探针使用临时用户目录和缓存，验证已发布词库中的 `nihao` 查询、选词提交及首／尾汉字选择：

```sh
cargo run -p msime-engine --example query_dictionary -- <上一步返回的资源目录>
```

整句神经重排模型另有一份 `resources/neural-model.lock.json`，把键盘用的小模型和桌面落定时使用的大模型锁在同一发布版本。需要单独准备两份模型时运行：

```sh
python3 scripts/fetch_neural_model.py --out target/neural-model
```

下载只接受 HTTPS，先写入临时文件，再逐个核对锁定的长度和 SHA-256 后改名发布；已有摘要匹配的文件会跳过。发布的安装包都不带桌面模型，用户打开「桌面神经联想」时由设置应用按需下载（资源包 `settled-model`）；本地构建和发行版打包仍可以随包带上它，这时资源打包优先接受历史的 `target/settled-model` 目录，缺失时自动使用这个 `target/neural-model` 目录里的桌面模型，随包的那份优先于下载的。键盘模型仍由 `desktop-dictionary.lock.json` 的词库资源安装器校验并复制进 EngineResources，两个锁都保留作来源记录。

macOS 原生 IMK bundle 的构建、隔离状态目录与安装见 [macOS 宿主](platforms/macos/README.md)。`platforms/macos/scripts/install.sh` 用 Developer ID 重签并原子替换到 `~/Library/Input Methods`，失败回滚；`platforms/macos/scripts/check_input_source.swift` 核查输入源注册结果。

## 代码签名策略 / Code signing policy

Windows 正式版目前由 [msime-windows](https://github.com/metasequoiaime/msime-windows) 构建、签名和发布，签名适用那边的 [Code signing policy](https://github.com/metasequoiaime/msime-windows/blob/develop/docs/code-signing-policy.md)：

Free code signing provided by [SignPath.io](https://about.signpath.io/), certificate by [SignPath Foundation](https://signpath.org/).

本仓库的 `release-windows.yml` 只产出未签名的 Windows 安装包，不使用任何签名凭据。其余平台按各自平台的机制签名，不经过 SignPath。隐私说明见 [PRIVACY.md](PRIVACY.md)。

## 许可证

源码为 **GPL-3.0-only**，全文见 [LICENSE](LICENSE)。

上游代码、词库、模型和各平台 SDK 以各自许可证和通知为准，其中 Android 与 iOS 的手写识别使用 Google ML Kit，按其服务条款授权而非开源许可证。完整对照见[第三方组件清单](docs/third-party.md)。
