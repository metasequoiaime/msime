# 架构与工程约束

这份文档说明水杉输入法共享客户端的分层方式，以及哪些边界是有意为之、不能顺手打破的。修改代码前请先读这里；具体流程见[贡献指南](../.github/CONTRIBUTING.md)，公开发布前的检查见[开源发布清单](open-source-release.md)。

## 分层

仓库按「谁拥有什么状态」分层，而不是按语言或平台分。

```
平台宿主 (platforms/)          系统输入法入口，各自进程
    ↓  C ABI / JNI / Objective-C++
crates/host-api                版本化 C 接口，线程绑定会话句柄
    ↓
crates/input-runtime           会话编排、焦点、候选分页、带代次的选择
    ↓
crates/engine                  输入算法与组合状态（纯 Rust）

第三方输入法前端               嵌入引擎的其他输入法，各自进程
    ↓  C ABI（msime_engine.h）
crates/engine-capi             引擎的 C 接口：带版本的结构体、按下标取候选、带修订号的选择
    ↓
crates/engine                  同上，只经核心层 `Session`
```

`crates/engine-capi`（`msime-engine-capi`）是给其他输入法嵌入引擎用的 C 库，与上面那条产品链路平行：它直接包核心层 `msime_engine::Session`，不经过 `host-api` 和 `input-runtime`，也不用 `host` 门面，msime 自己的宿主不走它。它的代码是 MPL-2.0，用 cargo-c 构建出共享库、静态库、生成的头文件和 pkg-config 文件，约定见 [crates/engine-capi/README.md](../crates/engine-capi/README.md)。

`crates/engine` 的文档化 API 是 crate 根上的核心层；`host`（产品会话与选项、词库管理与同步、释义、文本工具）、`ordering`（msime 的候选排序）、`backend`（msime-cloud 协议）和 `web`（网页存储）是 msime 自己的产品胶水。核心层签名用到的类型和第三方用核心选项必需的函数（辅助码表、`/` `@` K 模式的表项、自定义双拼与九键双拼表的校验与取表）放在根上，设置页编辑这些表用的辅助函数留在 `host`，`input-runtime`、`host-api`、`engine-wasm`、`backend-engine` 也从根上导入它们，`host` 不再重新导出；边界与理由见[产品 crate 只经文档化的引擎 API](../.agents/notes/implemented/architecture/2026-10-29-engine-host-dogfood.md)。

`host-api` 的词库管理接口（个人词条、固定资源的校验与编辑）直接调用 `crates/engine`，不经过 `input-runtime`；按键与会话一律走上面这条链路。

`crates/client-base`、`crates/client-core`、`crates/client-services` 和 `crates/client-providers` 与上面这条链路平行，不参与按键处理：client-base 是存储、文件锁、文本谓词和简繁转换这些不联网的底层，client-core 负责本地配置、版本、主题和固定资源的分代安装，client-services 是账号、社区、皮肤、插件、词库和打字统计这些联网或处理用户内容的服务，client-providers 是语音、服务凭据、AI 和按需资源包。依赖方向是 base → core → services 与 core → providers，services 和 providers 互不引用；`input-runtime` 只依赖 client-core，`backend-engine` 只依赖 client-base，它们的编译图里没有 reqwest、tokio 这些网络栈。`packages/ui` 是共享的 React 设置页，`apps/desktop` 是在 macOS 与 Linux 上承载它们的 Tauri 层；HarmonyOS 不经过 Tauri，`platforms/harmony/stage-settings.sh` 把 `apps/harmony` 构建进 ArkWeb 加载的 rawfile，页面经 ArkTS 原生桥和 NAPI 调用 Rust；哪些平台的设置界面用这些页面，见下面的[设置界面按平台归属](#设置界面按平台归属)。

**产品本体是 `platforms/<os>` 的原生宿主。** Android、iOS、HarmonyOS、Linux、macOS、Windows 一律如此：最终安装、启动、被系统识别为输入法的都是原生宿主。Tauri/React 是跨平台共享功能与界面的公共组件，只在用到它的平台上由原生宿主承载，不单独作为某个平台的产品去启动或验收。`apps/desktop` 的目录名和 Tauri 生成的工程都不改变这一点。

### 设置界面按平台归属

设置界面按平台分属两套实现：macOS、Linux 与 HarmonyOS 用 `packages/ui` 的共享 React 设置页，Windows、iOS 与 Android 发布包用各自的原生设置界面。决定的理由和放弃的方案见[决策笔记](../.agents/notes/implemented/architecture/2026-10-15-settings-ui-ownership-by-platform.md)。

| 平台 | 设置界面 | 入口与承载 |
| --- | --- | --- |
| macOS | 共享 React 设置页 | 输入法菜单的设置项以 `--route=settings:<类别>` 拉起 Tauri 设置应用 `app.msime.macos`（`platforms/macos/src/core/DesktopSettingsLauncher.h`） |
| Linux | 共享 React 设置页 | IBus 与 Fcitx5 的「设置…」以 `--route=settings` 调用 `msime-linux-settings`，关于、帮助、反馈和词库四项以 `--route=settings:<类别>` 调用（`platforms/linux/src/core/ClientEngine.cpp`、`platforms/linux/fcitx5/FcitxEngine.cpp`），由它启动 Tauri 外壳（`platforms/linux/data/msime-linux-settings.in`）；没有原生设置界面 |
| HarmonyOS | 共享 React 设置页 | `pages/Settings.ets` 用 ArkWeb 加载 `stage-settings.sh` 打包的 `rawfile/settings/index.html`，ArkTS 一侧只做原生桥，不画设置表单 |
| Windows | 现代 Windows：原生 WinUI 3；Win7：原生 Win32 | `msime-client-settings.exe`（`platforms/windows/settings/`）原生绘制全部 16 页；托盘、开始菜单和不带面板的请求都打开它（`platforms/windows/src/system/ShellSurfaces.h`）；React 设置页仍随 Tauri 的 `MSIME.exe` 安装，直接运行 `MSIME.exe` 能看到，但没有设置入口指向它；Win7 构建由 `settings/win7` 的 `msime-client-settings-win7.exe` 承载，复用同一共享 Rust 业务与偏好默认值 |
| iOS | 原生 SwiftUI | `platforms/ios/App/Sources/`；App、`SharedUI` 和键盘扩展里都没有 WebView |
| Android | 原生 Java | 发布的 APK（`platforms/android/build-apk.sh`）的设置 App 各页在 `java/app/msime/android/home/`，另有 `settings/`、`account/`、`community/`、`statistics/` 下的页面，不含 Tauri/React；带 React 设置的 Tauri 合包（`build-client-apk.sh`）只用于开发验证，不发布 |

Tauri 在 macOS、Linux 和现代 Windows 上还承载共享面板：表情与符号、手写、屏幕键盘、语音、云词库和云剪贴板，Linux 另有本地剪贴板面板（`SurfaceRoute::Clipboard`，`msime-linux-settings --panel clipboard`）。现代 Windows 原生设置窗口里语音、屏幕键盘、手写、云词库和云剪贴板这 5 个面板的入口拉起 Tauri 的 `MSIME.exe`（`platforms/windows/settings/SettingsNavigation.h` 的 `shell_links::all`）；表情与符号面板不从设置窗口打开，由 Server 的托盘菜单和浮动工具栏拉起（`platforms/windows/src/system/ShellSurfaces.h` 的 `shell_surface_request`）。Win7 构建通过显式 `MSIME_WINDOWS7_CLIENT` 配置选择 `panels/win7` 原生面板，覆盖这些功能入口，不依赖 Tauri 或 WebView2；引擎、协议与共享业务归属保持。

由此，一个设置功能要在 `packages/ui` 实现一次，再在每个原生设置平台（Windows、iOS、Android）各实现一次。为了让每个平台的那一份只剩界面，设置项的校验、默认值、持久化和业务逻辑必须放在共享 Rust 层（偏好在 `crates/client-core`，账号、皮肤、词库等在 `crates/client-services`，语音与 AI 在 `crates/client-providers`，都经 `crates/host-api` 提供给原生宿主），原生设置界面不应复制这些规则。新设置功能要先把规则下沉到上面对应的共享 Rust crate。Windows 语音页的服务目录和换服务字段补丁已由 `client-providers::voice::provider_catalog`、`provider_settings` 经 C ABI 提供，原生页只转换字段并保存。Android 的 ASR、润色服务列表、润色状态、服务分类、鉴权模式与豆包流式接口预设也经同一目录和共享规则的 JNI 入口读取，Java 只在识别列表前追加系统识别与本地模型，并适配方案、自定义槽位、字段输入和测试标题；目录不可用时保留当前配置并禁用相关选择入口。 Android 的凭据换槽和换服务时清空地址、模型也由 `provider_settings` 经 `msime_client_voice_provider_update` 的 JNI 适配提供；`clear` 为显式策略，现有宿主缺省的 `fill_defaults` 行为不变。 Android 语音接口预检也通过 `msime_client_voice_endpoint_issue` 复用运行时 URL 校验，并保留移动传输既有的小写协议、无 `@` 输入限制；Java 只适配并严格解析结果，桥接失败不允许保存。Windows 账号页的昵称规范化、校验、默认显示名和长度上限也由 `client-services::account::name` 经 `msime_client_account_name` 提供。

云候选服务的 HTTP 配置、模板、响应路径和缓存由 `client-core::cloud::candidates` 持有，Host API 将请求描述符附在在线查询上，各宿主只执行传输；Linux 的在线服务返回原始 JSON 给共享层解析。完整配置和隐私边界见[云候选服务](cloud-candidates.md)。

**macOS 的 AppKit 设置窗口冻结为回退。** `platforms/macos/src/settings/` 的原生设置窗口，以及 `AISettingsWindow.mm`、`TranslationSettingsWindow.mm`、`VoiceProviderSettings.mm` 等独立窗口，只在设置应用找不到、启动失败或 options 路径不是绝对路径时由 `DesktopSettingsLauncher.h` 的回退分支打开，另有以 `--preferences` 启动输入法 bundle 的独立设置窗口。它们只保留为设置应用无法启动时的最小回退，不加新功能；macOS 上的新设置只进共享 React 设置页。

### 设置项说明收进「?」

这是全平台的设计语言，六个平台的设置界面（共享 React 设置页、Windows、iOS、Android 原生设置）一律遵守：设置页不常驻解释性说明。每个设置项的解释放在标题旁一个「?」里，桌面（macOS、Linux、Windows）悬停或键盘聚焦时弹出气泡、点一下固定，手机（iOS、Android、HarmonyOS）点「?」弹出、点别处收起；一组设置共用的解释段落放在组标题旁的「?」里。页面上能看到的只有标题、控件和下面这些必须一眼看到的内容。决定的理由和放弃的方案见[决策笔记](../.agents/notes/implemented/feature/2026-11-01-settings-descriptions-behind-info-tip.md)。

始终留在原处（标题下方或组内）、不收进「?」的：

- 状态和当前值：「已安装」「12pt」「跟随皮肤」、当前数据目录、「已同意，联网功能已开启」。
- 校验、错误和进度：表单问题、连接或下载失败、下载百分比、「已达 N 个上限」。
- 列表条目的元信息：版本、作者、条数、登记时间、核对码、程序 id、排位「第 N 位」。
- 前置条件：「登录后可用」。
- 宿主限制：当前平台或桌面下这一组设置不起作用的说明，它就是这一组的内容。
- 空列表和截断提示：「还没有应用例外」「只显示前 20 条」。
- 紧贴按钮、讲这个按钮后果的一句话（「改回水杉账号会清除…」「卸载后默认保留词库」）和风险警告。

说明文字对读屏仍然可读：它以视觉隐藏的形式留在原处，控件通过 `aria-describedby`（或各平台的等价属性）读到它，「?」本身只是给看得见的人用的入口。

落地到代码：共享设置页的 `Row` 及其包装组件（`SwitchRow`、`SelectRow`、`SliderRow` 等）把 `description` 画成标题旁的 `InfoTip`（`packages/ui/src/core/info-tip.tsx`），上面列的常驻内容走 `detail`；`SettingsGroupNote` 在有组名的 `GroupList` 里自动收进组名旁的「?」，是内容而不是解释的段落标 `inline`，`role="status"` 的反馈和 `SettingsManagerBlock` 里紧贴按钮的说明留在原处。Windows、iOS、Android 的原生设置界面各有一个对应的可复用「?」组件，新设置项都用它，不在页面上另写常驻说明段落。写说明之前先想这句话能不能不要；要的话写进 `description`，不要写成标题下面的一段灰字。

## 四条不能打破的边界

**输入算法归 Rust 引擎 crate `crates/engine`。** 组词状态机、候选排序、学习回放都在 engine 里。`input-runtime` 仍然不复制组词逻辑：它只维护宿主编排和展示状态——它知道当前是第几页、焦点在不在、这次选择属于哪一代快照，但它不知道「ni hao」应该出什么词。在 `input-runtime` 或宿主里再写一份组词规则，就等于让两份实现开始漂移。

**client-base、client-core、client-services 和 client-providers 不依赖 Tauri、React、Engine 或平台宿主。** 平台能力一律通过接口注入。这条保证了它们可以被链进任何宿主进程，包括没有 Tauri 运行时的键盘扩展。

**平台库不依赖桌面应用。** iOS 键盘扩展不依赖常驻桌面服务，Android 的 `:ime` 进程不需要设置窗口活着。设置窗口关闭不能结束输入法进程；跨进程的设置变更需要明确的持久化与通知机制，不能靠共享内存里的一个全局变量。

**Windows TSF DLL 与 Server 的进程和协议边界保持不变。** TSF DLL 被系统加载进每个宿主应用的进程，Server 是独立进程，两者之间是显式协议。把逻辑从一侧挪到另一侧之前，先确认它在新的一侧还能满足 TSF 的线程和生命周期要求。

## 上游 Engine

输入引擎是本仓库里的 `crates/engine`（`msime-engine`），随 workspace 一起构建和测试，没有锁文件，也不再从外部拉取源码归档。它由 C++ 版 MSIME-Engine 移植而来：移植时用参考实现（固定 commit 加当时的 overlay）录下了行为基准，提交在 `crates/engine/tests/golden/`，由 `crates/engine/tests/golden.rs` 对照；录制方法和参考实现的来源记在 `tools/engine-golden/README.md`，只在需要从归档的参考构建重新录制时才用得上。引擎行为的改动直接改 `crates/engine` 并更新对应的基准，不再有 overlay 这一层。

引擎在运行时读写的文件（主库、五笔库、英文库、n-gram 表、日文模型、语言词库、手写与神经模型、学习日志和代次目录）的格式、文件内版本、产出方、各方案的最小数据集和逐文件的数据许可证规定在 [data-formats.md](data-formats.md)。改动这些文件的结构时同一个提交里更新它；读端不认识的改动要把 `format::FORMAT_VERSION` 加一，引擎拒绝比自己新的版本，没有 `metadata` 表的旧发布按版本 0 照常读。

平台仍要用到的非引擎文件已随仓库提交：Windows 与各宿主共用的 IPC 契约头文件在 `shared/contracts/`，辅助码表在 `resources/helpcodes/`，Windows 提示音用的 miniaudio 在 `platforms/windows/third_party/miniaudio/`。

引入新的上游代码、字体、图标、模型或服务 SDK 时，同时提交来源提交、许可证文本、通知位置和分发限制。

OpenCC 七份简繁转换表归 `msime-dictionary/sources/opencc/`，由 `resources/opencc.lock.json` 固定词库仓提交、OpenCC 上游提交及逐文件 SHA-256 与大小。`client-base/build.rs` 在构建主机用 Python 3.10+ 调用 `scripts/fetch_opencc.py` 准备输入，之后仍由 `include_str!` 内嵌。首次构建需要网络；后续复用逐文件校验的 `target/opencc/<commit>/`。离线构建可先运行 `python3 scripts/fetch_opencc.py` 准备缓存，再设置 `MSIME_OPENCC_OFFLINE=1`；也可用 `MSIME_OPENCC_SOURCE_DIR=/path/to/msime-dictionary/sources/opencc` 指定离线检出，字节必须与锁一致。`MSIME_PYTHON` 可指定解释器。运行时转换不读外部文件、不访问网络；许可证仍随各平台打包。

符号数据同样归词库仓：打拼音出符号的名称表和 LaTeX 命令名表是 `msime-dictionary/sources/symbols/` 下的 `symbol-names.tsv`、`latex-symbols.tsv`，改符号名称改那里。`resources/symbol-tables.lock.json` 固定词库仓提交和逐文件 SHA-256 与大小，`scripts/sync_symbol_tables.py` 把它们逐字节同步成仓库里的三份副本：`crates/engine/src/local/symbol_candidates.tsv` 和 `crates/engine/src/local/latex_symbols.tsv`（引擎 `include_str!`，后者供符号模式按 LaTeX 命令名查符号，放在 crate 内是为了 `cargo package` 能带上），以及 `plugins/phrase_table/latex-symbols/symbols.tsv`（官方短语表生成器），`scripts/test-symbol-tables.py` 离线核对副本等于锁。和 OpenCC 不同，副本提交进仓库、编译不联网：引擎被 Linux 的 Nix、Debian、RPM 离线打包和网页引擎一起编译，构建期下载会让每条打包流程都要再备一份离线输入。

## 验证

GitHub Actions 在 Pull Request 上分几条线跑，各自只被相关改动叫起来。`ci.yml` 跑仓库级检查：actionlint 校验全部 workflow 和它们内嵌的 shell，dependency-review 按 high 阈值拦截依赖。`ci-macos.yml` 用 Xcode 26.3（与发版同一个版本）在 arm64（Depot 的 macOS 26）与 x86_64（GitHub 的 macos-15-intel）上构建原生宿主并跑 ctest，另有一个 ASan/UBSan 任务，构建完还会 ad-hoc 签名并断言 bundle 的架构与 Info.plist 输入法键；macOS runner 只有 5 台，所以发往 `develop` 的 PR 只跑 arm64，x86_64 与 ASan/UBSan 留给发往 `main` 的 PR、定时运行和手动触发，推到 `develop` 或 `main` 时 macOS 与 iOS 都不再构建（PR 上已经跑过，合并后的回归由每天的定时运行兜底）。`ci-ios.yml` 跑共享 Swift 后端的 `swift test`、键盘工程配置校验和 XcodeGen 工程生成，其中偏重的设置与皮肤分片、以及需要 ML Kit 的手写用例只在进 `main` 的 Pull Request 上跑。`ci-platforms.yml` 在对应平台或共享层有改动时检查 Android（宿主契约与 JVM 冒烟）、Linux（固定容器内构建并跑 ctest）、HarmonyOS（类型检查与键盘逻辑测试）和 Windows（MinGW x64 交叉构建，以及 Windows runner 上独立编译 WinUI 3 设置工程），共享层（`crates/`、工作区清单、工具链等）有改动时还以普通用户在 Linux 上跑 `msime-engine` 的单元测试和 golden 基准。纯文档改动走 `ci-docs.yml`，它跑同一组仓库级检查并补上平台 workflow 按路径跳过的那几个必需检查名，免得一个只改 README 的 Pull Request 永远等一个不会来的报告。`codeql.yml` 在发往 `main` 的 Pull Request、推送到 `main` 和每日定时时扫 Actions、C/C++、Python 和 Rust，其中 Rust 覆盖 clippy 与 cargo audit 看不到的 FFI；Swift 分析只在每日定时和手动触发时跑，理由与 macOS 在推送时不构建相同；发往 `develop` 的 Pull Request 不扫，`develop` 由每日定时和发版时进 `main` 的那个 Pull Request 覆盖。`cargo-audit.yml` 单独对 `Cargo.lock` 的公告把关，在 `Cargo.lock`、`.cargo/audit.toml` 或它自身 workflow 变化的 Pull Request、推送到 `main` 以及每日定时运行：新公告是外部事件，不该让每个 Pull Request 同时变红，所以它不做必需检查。

CI 之外的那一半由本地验证覆盖：上面这些作业没跑到的 Rust crate 的 `cargo test`、clippy、前端的类型检查与 vitest、Wine 下的 Windows 套件、句子转换评测和重排延迟预算都只在本地跑。它同时也是提交前的快速反馈：

```sh
bash scripts/verify-local.sh --quick   # 只编译，合并前的门禁
bash scripts/verify-local.sh           # 全量
```

Windows 的编译门禁不需要 Windows 机器。装好 MinGW（`x86_64-w64-mingw32-g++`）后，在主工作区跑一次 `platforms/windows/build-cross.sh x64` 把 vcpkg 引导到清单基线，整台机器就位了：之后 `verify-local.sh` 在任何一个 worktree 里都会自动接管这条路径，把 host DLL、TSF DLL、Server 与全部原生测试链接一遍（vcpkg 树从 `MSIME_VCPKG_ROOT`、本工作区的 `target/tooling/vcpkg`、主工作区的同名目录依次查找；编译好的依赖放在该 vcpkg 树旁边共用，不必每个 worktree 各自把 curl 再编一遍）。没有这套环境时该阶段仍然跳过，但会把这行命令打出来——它曾经在每台机器上都只打印「skipped」，而背后的原生构建同时坏了六处。

Linux 的 Tauri 外壳同理，只要机器上有 Docker 就不需要 Linux 机器：`verify-local.sh` 会在 `platforms/linux/tests/tools/Dockerfile.desktop-check` 构建的镜像（固定摘要的 `rust:1.97.1-bookworm` 预装 webkit2gtk/gtk3/libsoup，按 checkout 打 tag）里 `cargo check -p msime-desktop --all-targets`。这一阶段的由来和 Windows 那条一样——`cargo check --workspace` 只看宿主 target，而 macOS 上 `msime-desktop` 因为缺少它当资源列出的 app bundle 被整包排除，于是 Tauri 外壳里所有 `#[cfg(target_os = "linux")]` 分支从来没被任何东西编译过，攒到 36 个编译错误：Linux 的设置窗口、全部共享面板和账号界面根本构建不出来。没有 Docker 时该阶段跳过并打印命令。

Linux 的原生宿主也一样，由 `platforms/linux/build-container.sh` 在同一个容器里编译 IBus engine、Fcitx5 插件、全部 provider 入口和单测并跑 `ctest`。这一条同样是补洞：此前没有任何阶段构建过 `platforms/linux`，而它已经不能配置了——三个测试的相对 include 比源码移动后的层级少一级，其中一个连自己的 fixture 都引不到。这不是单元测试的覆盖问题，是这个平台的产品本体构建不出来。Linux 主机上直接用系统的 ibus 开发包跑，其它主机走容器；两者都没有时跳过并打印命令。这一阶段只编译并跑单测，`platforms/linux/tests/tools/check-container.sh` 仍是需要已验证词库和真实 IBus daemon 的验收运行。

关键在于基线，不在于通过率。几个测试套件有长期存在的失败，所以单看「几个失败」没有意义；合并前唯一要回答的问题是「这次改动有没有弄坏原本能用的东西」。脚本因此把每一阶段失败的**测试名**与 `scripts/known-failures.txt` 对照，只有不在基线里的名字才算回归。那个文件里的每一行都是债务而不是豁免：修好一个就删一行，不要为了让运行变绿而新增。

首次克隆后执行一次：

```sh
git config core.hooksPath .githooks
```

三个钩子分工明确。`pre-commit` 是亚秒级的，只检查冲突标记和暂存 Rust 文件的格式——编译得起来的检查放在后面两个里，因为耗时一分钟的提交钩子会被关掉，然后一个都不剩。`pre-merge-commit` 在合并提交产生的那一刻跑 `--quick`，`pre-push` 作为没走合并路径的提交的兜底。`--quick` 会按改动范围跳过碰不到的平台阶段（依据是整个分支相对 `origin/develop` 的改动加上工作区，合并时进来的文件也在其中），并为每个跳过的阶段打印原因；`MSIME_VERIFY_ALL=1` 强制全部运行。曾有六次编译中断因为「合并了但没构建合并结果」进入 `develop`，`--quick` 正是为此存在。

Rust 改动另需 `cargo fmt` 与 `cargo clippy`；UI 改动另需类型检查和构建。

## 各平台的验证覆盖面

六个宿主各有自己的自动化套件和自己的装机路径，入口不同但结构一致：先在不需要目标系统的层面跑通编译与单测，再在目标系统里跑需要真实输入法框架的那一层。报告验证结果时说清楚跑的是哪一层——交叉编译通过和系统输入法里真的出了候选不是同一件事。

- **macOS**：`platforms/macos` 的 CMake 工程注册了一百多项 ctest，覆盖候选面板、皮肤、语音、表情与剪贴板面板、词库安装与偏好持久化；`platforms/macos/scripts/install.sh` 用 Developer ID 重签后原子替换到 `~/Library/Input Methods`，`platforms/macos/scripts/check_input_source.swift` 核对输入源注册。
- **iOS**：工程由 XcodeGen 从 `platforms/ios/project.yml` 生成，`MSIMEClientTests` 聚合键盘、服务与共享三个单测 target，`MSIMEClientUITests` 跑引导流程和键盘扩展在编辑器里的界面用例；`platforms/ios/build-app.sh` 出模拟器或真机构建。
- **Android**：`platforms/android/check-host.sh` 在没有 Android 运行时的机器上校验 JNI 契约并跑 JVM 冒烟；`platforms/android/tests/device/smoke.sh` 在专用 AVD 上跑设备套件，设置、打字统计和手写各有开关；`platforms/android/build-apk.sh` 出原生 IME 包。
- **HarmonyOS**：`platforms/harmony/tests/run.sh` 做类型检查并跑键盘逻辑测试；HAP 由 DevEco 的 `hvigorw assembleHap` 打出，`ohpm install` 和真打包缺一不可——ArkTS 对 `@Builder` 体内声明、对象字面量类型的一批限制只有 `.ets` 真正编译时才报出来。
- **Linux**：`platforms/linux/build-container.sh` 在固定容器里构建 IBus engine、Fcitx5 插件和全部 provider 入口并跑 ctest；`MSIME_PACKAGE_FORMAT=rpm MSIME_PACKAGE_TESTS_ONLY=1 platforms/linux/package-container.sh` 在发版 `.rpm` 用的 Fedora 容器里以 Release 编 full 版并跑同一套 ctest，不出包，PR 上的「Linux native host (Fedora)」就是它；`platforms/linux/tests/tools/check-container.sh` 起独立 D-Bus 与 IBus/Fcitx5 daemon 做隔离验收。
- **Windows**：`platforms/windows/build-cross.sh` 用 MinGW 交叉构建 host DLL、TSF DLL、Server 和全部原生测试，`platforms/windows/run-tests-wine.sh` 在 Wine 下真跑这些产物；`platforms/windows/Build-Client.ps1` 是 Windows 上的 MSVC 全量构建，安装包由 `platforms/windows/installer/` 的 Inno Setup 脚本和双架构签名链产出。

各平台的构建前置、环境变量和装机步骤见对应的 `platforms/<os>/README.md`，目录与职责总览见 [README](../README.md)。
