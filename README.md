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

项目主页：[msime.app](https://msime.app/) · [GitHub 仓库](https://github.com/metasequoiaime/msime) · [贡献者](https://github.com/metasequoiaime/msime/graphs/contributors) · [问题反馈](https://github.com/metasequoiaime/msime/issues)

## Star History

[![Star History Chart](https://api.star-history.com/svg?repos=metasequoiaime/msime&type=Date)](https://star-history.com/#metasequoiaime/msime&Date)

> **关于名称**：MSIME 是 Metasequoia IME（水杉输入法）的缩写，与 Microsoft IME 无关，也与微软没有任何关联。代码、包名和仓库名中的 `msime` 一律是这个含义。设置中的 `shuangpin_profile: microsoft` 是「微软双拼」方案，与小鹤、自然码、首道并列的四个键位方案之一，供习惯该键位的用户选择，同样不代表任何关联。

**输入法能看到你输入的一切，所以这个问题应该有一个能逐条核对的答案：[哪些数据会离开设备](PRIVACY.md)。**简短版本：默认配置下只有云联想一个功能会把正在组的拼音发出去（发给 Google 输入工具，可关闭），其余联网功能都要你自己填凭据才会工作。另有一条不携带输入内容的自有上报：macOS、iOS、Android、Linux、HarmonyOS 会在启动时向 `api.msime.app` 发一次安装计数，其中除 HarmonyOS 外还会在崩溃时发送异常信息，默认开启且目前没有开关；Windows 上同一条上报默认关闭，要在设置页「关于」里打开「匿名使用统计」才会发送——字段、逐平台差异和落盘位置逐条列在 [PRIVACY.md](PRIVACY.md#安装与崩溃上报)。仓库不接入任何第三方统计或崩溃上报 SDK。

## 模块边界

- `crates/client-core`：宿主无关的客户端业务——本地配置、固定资源分代安装、账号与会话、云与 AI、皮肤、词库、剪贴板、语音、翻译、打字统计；不依赖 Tauri、UI 或平台宿主。
- `crates/input-runtime`：会话编排、焦点取消、候选分页和带代次的选择；不复制 Engine 组词状态机。
- `crates/engine-bridge`：通过 CXX 调用固定上游 C++ Engine 的公共 Session。
- `crates/host-api`：版本化 C 接口、线程绑定的会话句柄和显式响应释放，六个平台的宿主都链接它。
- `packages/ui`、`apps/desktop`：共享 React 设置页与 Tauri 承载层，各平台使用同一个 Rust 入口库、commands 与 React 页面；目录名沿用 desktop。Rust 入口按 `platform/{android,ios,linux,macos,windows,desktop}`、`shared/`、`tests/` 分层。**Tauri 是公共组件，不是任何平台的产品本体**：最终安装、启动、被系统识别为输入法的，始终是 `platforms/<os>` 下的原生宿主，Tauri/React 由它按需承载。
- `shared/apple/`：macOS 与 iOS 共用的 Foundation / Objective-C++ 桥接，不包含系统输入法入口。
- `platforms/`：各系统入口和适配层。Android、iOS、macOS、Linux、Windows 与 HarmonyOS 六个平台各自维护宿主边界，每个宿主都以该系统的原生方式被识别为输入法：Android 的输入法服务、iOS 的键盘扩展、macOS 的 InputMethodKit bundle、Linux 的 IBus 与 Fcitx5 入口、Windows 的 TSF DLL 与 Server、HarmonyOS 的 InputMethodExtensionAbility。共享输入算法和组合状态由 C++ Engine 持有，宿主不复制。

## 平台目录与宿主形态

| 平台 | 入口目录 | 宿主形态与集成方式 |
| --- | --- | --- |
| [Android](platforms/android/README.md) | `platforms/android/` | 输入法服务 `app.msime.client.MSIMEInputService` 跑在 `:ime` 独立进程，Java 宿主经 `platforms/android/native/client_jni.cpp` 调 host-api；Tauri/React 设置与输入法同包不同进程，共享私有 files/bootstrap/state；手写走 ML Kit Digital Ink |
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

贡献代码前请阅读 [架构说明](ARCHITECTURE.md)、[贡献指南](CONTRIBUTING.md)、[安全策略](SECURITY.md)、[网络请求与数据流向](PRIVACY.md) 和 [行为准则](CODE_OF_CONDUCT.md)。准备公开源代码或平台构建物时，再阅读 [开源发布清单](docs/open-source-release.md) 和[第三方组件清单](docs/third-party.md)；它们列出第三方通知、资源许可、敏感文件检查和验证边界。各平台宿主的构建、测试与安装细节以对应的 `platforms/<os>/README.md` 为准。

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

桌面设置的应用标识和默认应用数据目录在 Windows 与 Linux 上是 `app.msime.client`，macOS 上是 `app.msime.macos`，偏好保存在其中的 `preferences.json`。macOS 首次用正式标识启动时会完成默认状态初始化并重建其中的绝对路径；也可用绝对路径环境变量 `MSIME_CLIENT_STATE_DIR` 指向隔离开发目录。macOS 原生宿主读取同一份配置并在当前组词结束后应用更新；多个设置窗口同时保存时通过 revision 检测冲突，用户须显式重新读取后决定是否覆盖。

Android 合包构建和设备测试见 [Android 宿主](platforms/android/README.md#tauri--react-共享设置合包)。Tauri 设置与原生 `:ime` 服务同包、不同进程，共享私有 files/bootstrap/state；关闭设置窗口不结束输入法进程。iOS 的产品宿主是 `platforms/ios` 的原生 App，它嵌入原生键盘扩展并通过 App Group 共享状态；Tauri/React 在 iOS 上只作为共享功能与界面的公共组件，不作为独立 App 启动。签名与设备安装步骤见 [iOS 宿主](platforms/ios/README.md)。

共享设置支持 shuangpin_profile：xiaohe（小鹤）、ziranma（自然码）、shoudao（首道）、microsoft（微软）。旧配置缺省按小鹤读取且不自动改写，未知值拒绝；设置页在非双拼方案下禁用此选择但保留已选值。方案更改沿用组词结束后替换 Engine 的规则。新宿主会写出此字段，旧版本严格解析器可能拒绝新配置，设置端和宿主应成套更新，不得通过删除未知字段强行降级。

Linux 本地构建、隔离 D-Bus / IBus 测试和安装后的首次配置见 [Linux 宿主](platforms/linux/README.md)。宿主支持设置文件自动重读，安装后由随装的 `msime-linux-setup` 备齐词库并准备运行配置；`msime-linux-setup --download` 按 `resources/desktop-dictionary.lock.json` 取回缺失词库，图形入口在缺少 `runtime-options.json` 时会打开同一套首次配置页。

功能对齐以 MSIME-Windows 的完整功能为行为基线：公共业务和界面逐项接入共享层与 Tauri，Windows TSF DLL / Server 的进程和协议边界原样保留，Android、iOS、macOS、Linux 与 HarmonyOS 按各自系统能力适配。`scripts/test-reference-*.py` 把这条基线固化成可复跑的门禁——参考实现的出厂配置键、四个界面的机器可读能力清单、changelog 的每条特性、以及 `windows/`、`server/`、`ui/src` 下的每个源文件，都必须对应到本仓库的实现或一条写明理由的缺席记录。

## Engine 桥接

```sh
# 构建时自动准备；也可手动跑一次
python3 scripts/fetch_engine.py
# 安装 Engine 的 Boost、fmt、spdlog、SQLite3 和 CMake 依赖后：
cargo test -p msime-engine-bridge --locked
# macOS Homebrew 环境可能需要：
CMAKE_PREFIX_PATH="$(brew --prefix)" cargo test -p msime-engine-bridge --locked
```

Engine 由 `engine-lock.json` 固定：锁文件同时记录 Engine 及其第三方源码归档的 commit 和 SHA-256，`scripts/fetch_engine.py` 校验并展开这些归档，不使用 gitlink、`.gitmodules` 或递归 Git checkout；`crates/engine-bridge` 的 build.rs 在编译前调用它。离线构建可用 `MSIME_SKIP_ENGINE_FETCH=1` 跳过。CXX 生成互操作代码，CMake 构建原有 C++ 引擎，Cargo 链接静态引擎与系统 SQLite。会话不实现 Send/Sync，C++ 异常在桥接边界转成 Result。路径由宿主明确提供，字符输入是 Engine 支持的 ASCII 动作。桥接与运行时测试使用真实 Engine 而非替身，其中本地 Unicode 模式那组用例覆盖裸数字键归 Engine 还是归候选选择的判定。

## 原生宿主接口

`cargo build -p msime-host-api --locked` 产出静态库和动态库。头文件为 `crates/host-api/include/msime_client.h`，C 消费示例为 `crates/host-api/native/native_smoke.c`。调用链为 C 宿主 → host-api → input-runtime → CXX → C++ Engine，无 Tauri 运行时依赖。

宿主创建会话后须显式传入焦点状态，将 `handled` 映射为系统吃键，将 `commit` 通过系统 API 上屏，将值快照渲染为候选。候选选择携带返回的 generation 和全局 index。全部会话操作在创建线程执行；过期、销毁或错误线程句柄返回错误。每个 UTF-8 JSON 响应必须通过 `msime_client_string_free` 释放一次。逐键延迟由 `crates/input-runtime` 的 `rerank_latency` 示例按帧预算测量，`scripts/verify-local.sh` 把它作为一个门禁阶段运行。

`msime_client_select_edge(session, generation, index, edge)` 是 ABI 1 附加接口，适配器与宿主库须成套更新。方向使用 `MSIME_FIRST_HAN` / `MSIME_LAST_HAN`；共享层核对候选所属会话、代次和当前页，C++ Engine 提取首／尾汉字并在成功后清空整个组合。候选没有汉字时返回未处理并保留组合，不自动选词或追加标点；这类有效调用仍更新视图代次，宿主后续操作必须使用新快照。非法方向和失效身份在状态推进前拒绝。Windows 适配库的配置键路由、无汉字回退与 TSF 回复编码都已接进生产 KeyHandler，对应的 `msime-tsf-client-key-router`、`msime-tsf-character-result` 和 `msime-tsf-engine-response` 等测试在 `platforms/windows/tsf` 的 ctest 里覆盖这条路径。

## 固定词库资源

`resources/desktop-dictionary.lock.json` 固定已发布 `dict-v2.0.1` 的来源、长度和 SHA-256。其中 `mozc_dictionary_oss_README.txt` 是日文词库的许可证全文，IPAdic 与 ICOT 的条款都要求它随词库一同分发，重新打包时不可省略；详见[第三方组件清单](docs/third-party.md#日文词库的分发义务)。首次下载约 170 MB。**词库发布与 `engine-lock.json` 配对**：词库锁的 `source_commit` 记录它是哪次 Engine 提交产出的，其中 `bigram.bin` 与 `trigram.bin` 是整句词格仲裁的语言模型表。表缺失时 Engine 不报错，只是整句路径不加权——候选照出，顺序变差，所以提 Engine 锁必须同时提词库锁。开发准备命令：

```sh
cargo run -p msime-client-core --example install_resources -- target/resources
```

安装器通过注入的流读取资源，限制长度并校验摘要；全部成功后才发布到内容标识目录。再次使用时检查缓存字节；损坏缓存报错，失败安装不替换旧代。这里只准备不可变发布资源，不激活现有输入法，不迁移用户学习数据。资源按独立文件下载，不整包解压 Engine 归档。

`engine-bridge::prepare_options` 调用 Engine 权威的工作词库准备与学习回放入口，调用方必须先验证资源并暂停相关会话。以下探针使用临时用户目录和缓存，验证已发布词库中的 `nihao` 查询、选词提交及首／尾汉字选择：

```sh
cargo run -p msime-engine-bridge --example query_dictionary -- <上一步返回的资源目录>
```

macOS 原生 IMK bundle 的构建、隔离状态目录与安装见 [macOS 宿主](platforms/macos/README.md)。`platforms/macos/scripts/install.sh` 用 Developer ID 重签并原子替换到 `~/Library/Input Methods`，失败回滚；`platforms/macos/scripts/check_input_source.swift` 核查输入源注册结果。

## 许可证

源码为 **GPL-3.0-only**，全文见 [LICENSE](LICENSE)。

上游代码、词库、模型和各平台 SDK 以各自许可证和通知为准，其中 Android 与 iOS 的手写识别使用 Google ML Kit，按其服务条款授权而非开源许可证。完整对照见[第三方组件清单](docs/third-party.md)。
