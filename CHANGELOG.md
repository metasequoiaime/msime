# 变更记录

本文件记录共享客户端的公开变更。平台细节以对应平台 README 及 [实施记录](docs/implementation.md) 为准。

## [Unreleased]

首个版本的完整变更集。共享 Rust 层、六个平台宿主（Android / iOS / macOS / Linux / Windows / HarmonyOS）和共享设置界面均已实现，尚未正式发版。

### 新增

#### 共享层与宿主接口

- 不依赖 Tauri、React 或任何平台宿主的 Rust 共享层，覆盖偏好设置、资源安装与校验、输入运行时、账号与云服务、语音、翻译、皮肤、社区资源、剪贴板、个人词库、打字统计和候选释义。
- 输入引擎 `crates/engine`（`msime-engine`）是纯 Rust 实现，由 C++ MSIME-Engine 移植而来，输入算法、组合状态和学习数据归它管理，共享层只做编排；已有的用户词库日志、个人词库与学习数据按原格式读写，无需迁移。
- 版本化 C ABI（`crates/host-api`）作为所有原生宿主的唯一入口，以 `cdylib`/`staticlib`/`rlib` 三种形态提供，覆盖会话生命周期、候选与代次选择、词库快照、云词典、云剪贴板、系统字体目录、翻译提供方、语音提供方与打字统计。
- 输入运行时统一处理焦点、候选翻页、代次选择与全半角转换，并接入固定版本的整句重排模型。
- macOS 与 Windows 各有一层宿主支持 crate：macOS 提供面板会话、云剪贴板与云词典桥接；Windows 提供语音控制器、语音上屏策略与 Windows Ink 手写。桌面 shell 禁用 `unsafe`，平台 API 调用集中在这两个 crate 里做安全封装。
- Rust workspace 统一 GPL-3.0-only、`edition 2021` 与 `unsafe_code = "deny"`，工具链由 `rust-toolchain.toml` 钉死。

#### 共享设置界面

- `packages/ui` 提供跨平台共享的 React 设置页，桌面、Android、iOS 与 HarmonyOS 宿主共用同一份实现，包含首页、账号、AI 对话、社区、打字统计、外观、输入、辅助码、快捷键、词库、皮肤、语音、屏幕键盘、手写、实用功能、AI 辅助、悬浮工具栏、帮助、关于与反馈等页面，并带移动端底部 tab 映射。
- 桌面 shell（Tauri）提供跨平台命令层，并按 Windows / macOS / Linux / Android / iOS 分目录接入各自的账号、凭据、数据目录、输入源与音频能力。
- HarmonyOS 宿主消费同一份设置页的单文件构建产物，由 `platforms/harmony/stage-settings.sh` 在每次打 HAP 前从 `packages/ui` 构建、不提交进仓库，`entry/hvigorfile.ts` 在它缺失时拒绝打包。
- 外观页新增「候选窗」分组，集中设置候选主题、候选字体预设（默认 / 宋体 / 黑体 / 楷体 / 圆体）、整体大小、不透明度、背景 / 焦点高亮 / 文字 / 序号四种颜色和圆角大小。共享偏好新增 `candidate_scale_percent`（50–200%，字号与窗口几何一并缩放）、`candidate_opacity_percent`（50–100%，只作用于卡片底色、边框和皮肤背景图，文字保持不透明）和 `candidate_corner_radius`（0–32pt，优先于皮肤包的 `corner_radius_dip`）；宿主能力新增 `candidate_window_scale`、`candidate_window_opacity`、`candidate_corner_radius` 三个开关，Windows 与 macOS 全部支持，Linux（Fcitx5）只支持圆角，HarmonyOS 2in1 支持圆角与不透明度，iOS 与 Android 不提供。可见变化：设置了焦点高亮颜色后，选中候选的文字改为按该底色明暗自动取黑或白（序号随之取同色半透明），不再沿用强调色。兼容性：三项保持默认时不写入偏好文档，旧版本照常读取；一旦调整过其中任一项，尚未升级的旧版本宿主（例如更新后尚未重启的输入法进程，或经同步共用同一份文档的另一台设备）会因不认识这些键而整份加载失败，需要一并升级。

#### macOS

- InputMethodKit 输入法宿主，支持候选面板（定位、分页、悬停、行宽适配、释义布局）、内置与外部皮肤包、候选释义与翻译、云联想、悬浮工具栏、中英模式 HUD、双拼键位提示、屏幕键盘、手写、表情与符号面板、剪贴板历史面板。
- 语音输入覆盖豆包 WSS 与 HTTP 提供方、波形面板、录音设备选择、系统静音与恢复、润色和提示音；macOS 另可选用系统识别，音频不出设备。
- 词库安装、运行时挂载、快照与云词典，账号与云剪贴板，打字统计、诊断日志、成对标点、智能标点空格、五笔上屏策略与辅助码。
- Sparkle 自动更新；`platforms/macos/scripts/install.sh` 完成重签名、原子替换与失败回滚，`platforms/macos/scripts/check_input_source.swift` 核查输入源注册。
- 首次安装窗口：本机从未装过输入法时，设置应用先以一个小的安装窗口打开（图标、名称、「立即安装」），点击后按钮变成进度条，装好后「进入设置」恢复设置页窗口；已装过的机器不出现这个窗口，启动时照旧静默刷新。

#### Windows

- TSF 进程内 DLL 与进程外服务分列为两个目标，另有守护进程与运行时准备工具；命名管道 IPC 分主通道、辅助通道与诊断通道三种角色，并校验客户端 PID、登录会话与 TokenUser SID。
- 基于 Direct2D/DirectWrite 的候选窗口、候选浮出窗口、托盘菜单与悬浮工具栏，皮肤、调色板、字体、阴影、滚轮翻页与卡片尺寸各有独立策略。
- 云候选、AI 候选与翻译各自独立工作线程；语音覆盖豆包 ASR、语音热键、提示音、系统静音、波形浮层与独立的语音控制器端点；剪贴板历史落 SQLite。
- 设置热重载、首次运行引导、焦点路由、繁简转换、打字统计与维护热键。
- `platforms/windows/build-cross.sh` 提供 MinGW 交叉构建，`run-tests-wine.sh` 在 Wine 下运行交叉产物的全部套件；`Build-Client.ps1` 提供 Windows 上的 MSVC 全量构建并对产物做 PE 架构门禁。
- Inno Setup 安装器注册 32/64 位 TIP、分离配置与词库、升级不覆盖用户配置，并随包携带 `LICENSE.txt` 与第三方通知；`Package-SimplySign.ps1` 编排双架构构建、载荷签名、安装器编译与安装器签名。

#### Linux

- IBus 与 Fcitx5 是并列的两个系统入口，链接同一份宿主 ABI；Fcitx5 插件按 Fcitx5Core 版本启用原生候选 Action 与自定义输入法信息。
- 候选调色板、Fcitx 主题、字体策略、滚轮翻页、候选动作（固定、取消固定、删除）、翻译策略、成对标点与双拼方案名；九键、九键歧义拼音、本地输入模式、数字选词、小键盘标点映射、繁简与日文转换、辅助码、模糊音、默认开启且不在设置页或输入法菜单显示的全拼纠错、全角半角、智能标点空格与退格长按。
- 模式徽章与语音波形浮层各自提供 X11 与 Wayland（layer-shell）后端，缺少依赖时退回面板文字。
- 在线候选（含 AI 与翻译）、语音、剪贴板、手写、表情、词库、云词典与云剪贴板各有独立的 provider 入口；在线候选与语音配套 socket 激活的 systemd 用户单元，剪贴板配套常驻用户单元。云词典与云剪贴板目前只有请求入口，仓库没有附带应答它们的 provider 服务，Fcitx5 菜单里的云剪贴板条目需要用户自备 provider；设置窗口里的云剪贴板直接使用已登录账号。桌面入口提供九个 Desktop Action。
- `msime-linux-setup` 按随装的词库锁校验或取回词库、准备用户状态目录，并按当前运行的是 fcitx5 还是 ibus 说明下一步；缺少运行时配置时图形设置页会直接进入同一套首次配置流程。卸载时先停用并禁用 setup 启用过的用户单元。
- 升级后由宿主在建立会话前比对词库锁代次，原子替换运行时配置中的资源与词库路径。
- CPack 提供 TGZ 与 DEB 打包，Debian 包声明 IBus、Python 与（启用时）Fcitx5 依赖，并随包携带许可证与平台 README。

#### Android

- 独立进程运行的 IME 服务，26 键与符号层、全拼九键、日语九键、四套双拼与微软双拼分词键、86 五笔、手写（ML Kit Digital Ink）以及 AI 回复键盘。
- 候选条与展开面板支持跨代次选择，候选长按提供优先、固定与删除；离线英文释义、在线候选翻译、云联想与 AI 联想（组字停顿后单次请求）。
- 剪贴板历史、云剪贴板、表情浏览器、符号面板、本地输入模式、AI 润色、语音输入、打字统计、内置与自定义皮肤库、社区皮肤与词库资源。
- 账号会话经 Keystore 加密落盘，支持 Google 登录；硬件键盘按共享命令表映射，含数字行选词与模式快捷键；大屏居中外框与无障碍键盘尺寸调整。
- `check-host.sh` 把 Android 侧与共享 ABI 的命令编号、键位映射和禁止复制的表格做成契约守卫，并编译运行 JVM 冒烟套件；设备套件覆盖候选、键盘、设置、统计与手写。
- 两条打包路径：原生 IME APK 与 Tauri 设置合包，各自的用途与同名产物冲突写在平台 README 中。

#### iOS

- 主 App 与键盘扩展共享 App Group 与设置实现，另有独立的豆包传输框架 target。
- 键盘扩展提供候选栏与展开候选面板、方案与布局选择、皮肤选择、符号面板、表情与颜文字、剪贴板、日语九宫、手写、云候选、候选翻译、词库快照后台线程、英文建议与大写策略、空格移光标与标点上下文，并适配不同形态设备。
- 主 App 提供引导、账号、社区皮肤画廊与自定义编辑器、AI 生成皮肤、云词典、个人词典、云剪贴板、AI 对话、平板分栏以及各设置页。
- 语音走豆包 WebSocket 传输与协调器，配合录音与 PCM 提取。
- Xcode 工程由 XcodeGen 从 `project.yml` 生成，`project.yml` 是工程配置的唯一权威来源；单元测试分键盘、服务、共享与传输四个 target，另有 UI 测试 target。

#### HarmonyOS

- 完整的 ArkTS 输入法宿主：`InputMethodExtensionAbility` 负责面板创建、光标跟随、选区与文本变更回调、HUD 面板、悬浮工具栏与硬件键路由；模块清单声明输入法类型并覆盖手机、平板与 2in1。
- NAPI 边界导出会话、候选管理、翻译、云与 AI 联想、语音、皮肤库、社区资源、个人词库与打字统计等能力。
- 手写走 Core Vision Kit 文本识别，语音覆盖系统离线识别、HTTP ASR 与豆包流式，配提示音与录音设备选择。
- 候选翻译、离线英文释义与补全、AI 润色、AI 对话、设置同步、具名皮肤库、社区皮肤与词库、AI 生成皮肤、个人词库文件导入、全拼与日语九键、无障碍标签、2in1 硬件键盘、悬浮工具栏与输入模式 HUD。
- 平台无对应 API 的能力（应用图标切换、主动拉起键盘、表情与剪贴板独立窗口）在平台 README 中逐条记录了查证结果与替代做法。

#### 本地语音识别

- 语音服务 `local` 改为基于 sherpa-onnx 的设备端识别，六个平台共用同一份模型目录与设置页。运行时 sherpa-onnx v1.13.8 按平台由 `resources/voice-runtime.lock.json` 固定 SHA-256，`scripts/fetch_voice_runtime.py` 校验后取回，宿主在首次识别时动态加载，缺少运行时的包照常启动、只把本地识别标为不可用。
- 模型目录 `resources/local-asr-models.json` 提供三个模型：默认的中英流式 X-ASR（边说边出字，自带标点），支持中英日韩粤的 SenseVoice-Small，以及仅桌面提供、约 1 GB 的 Fun-ASR-Nano。模型不随包分发，在设置页按需下载，逐文件校验长度与 SHA-256，完整就位后才写入 `msime-model.json`，下载可取消，也可配置 HTTPS 镜像（`voice_input.asr_model_mirror`）。
- 用户词库里自己添加的拼音词条作为热词：X-ASR 与 Fun-ASR-Nano 原生使用，SenseVoice 在识别后按拼音做近音替换（`client-core::voice::hotwords`，含 zh/z、n/l、an/ang 等模糊对）。
- `voice_input.asr_model_path` 接受已安装的模型目录，Unix、Windows 盘符、UNC 等绝对路径写法在任何系统上都能通过校验，同一份偏好文件跨平台读取不再被拒。
- 宿主接口新增 `msime_client_voice_hotwords`、`msime_client_voice_hotword_correct`、`msime_client_voice_local_models`、`msime_client_voice_local_model_install`、`msime_client_voice_local_model_cancel` 与 `msime_client_voice_local_model_remove`。
- `msime-voice-local` 辅助进程通过标准输入输出上的 JSON 行协议识别，macOS 与 Linux 的输入法进程由它加载模型，自身不常驻数百 MB 的模型；空闲 120 秒释放模型，空闲 10 分钟退出。协议见 `shared/voice/README.md`，许可证见[第三方组件清单](docs/third-party.md)。
- 识别全程不联网，只有下载模型时访问 GitHub Releases 或所配置的镜像，见[网络请求与数据流向](PRIVACY.md)。
- 各平台接入：Windows 在 `msime-client-server` 内边录边识别，浮窗与豆包一样显示实时文本；macOS 输入法经辅助进程识别；Linux 由用户级语音服务调用辅助进程，本地识别不再要求保存任何云端凭据，安装模型时顺带启用语音服务；Android 与 HarmonyOS（arm64）在应用内识别，HarmonyOS 设置页可下载与删除模型；iOS 主应用内识别（键盘扩展受内存上限所限不加载模型）。HarmonyOS 的 armeabi-v7a 包没有本地识别。
- 系统识别：macOS 26 与 iOS 26 起使用 SpeechAnalyzer，更早的系统在支持时要求 SFSpeechRecognizer 设备端识别；HarmonyOS 的 Core Speech Kit 改为写音频模式（`recognitionMode: 0`），识别器只听输入法写入的音频，末尾不足一帧的音频补静音后写入。
- 修正 macOS CMake 用 `FORCE` 覆盖用户缓存变量的问题。

#### 账号、云服务与联网行为

- 账号、云词典、云剪贴板与社区资源统一走 `https://api.msime.app`，凭据存放在各平台的系统密钥库。
- 跨设备剪贴板（#2451）：Android、iOS、HarmonyOS 键盘的剪贴板面板新增「云端」栏，点选即上屏；本地剪贴板历史可以「发到云剪贴板」（三个手机键盘、共享设置页与 macOS 原生面板）。macOS 输入法菜单新增「云剪贴板…」。Windows、Linux 与从设置窗口打开的 macOS 云剪贴板改为直接使用已登录账号，此前 Windows 上每个请求都返回不可用；Fcitx5 改读服务端返回的 `items` 字段。不自动读取或上传系统剪贴板，细节见[云剪贴板](PRIVACY.md#云剪贴板需要登录)。
- Android 原生安装包的候选翻译改用已登录账号的令牌：此前它读取的是 Tauri 合包才写入的会话存储，原生包登录后仍一直使用匿名账号。合包照旧使用其 Rust 会话的令牌。
- 云联想默认开启，把正在组的拼音发给 Google 输入工具；语音识别、语音润色、AI 联想默认凭据为空，不填就不发请求；候选翻译在 macOS 与 Linux 新装时默认用水杉账号，其他平台默认不联网，要在设置里选择一个翻译服务才发请求。
- 在线候选释义走水杉账号（`https://api.msime.app/v1/translate`）改为显式选择：新增偏好 `translation_account`（macOS 与 Linux 新装及恢复默认时为 `true`，其他平台为 `false`；配置文件里缺这个字段按 `false` 读，未选时不写入配置文件），只有在翻译服务里选了「水杉账号」、候选翻译开着、且没有启用你自己的小牛、自定义或凭据可用的腾讯服务时，macOS、iOS、Android 才把当前页的中文候选词发给它。此前这三个平台在没有配置自己的服务时会默认走这条路径。macOS 输入法也不再在首次激活时预先创建匿名账号，匿名账号改为在首次用水杉账号翻译时才创建。升级影响：没选过翻译服务的用户，包括已登录账号的用户，升级后都不再收到在线释义，要重新在设置里选择「水杉账号」或填入自己的服务；在此之前，非英语目标语言没有释义，英文释义只来自离线词典，iOS 上非英语的释义行（主语言和第二语言都算）会收起。细节见[网络请求与数据流向](PRIVACY.md#候选翻译macos-与-linux-新装默认用水杉账号)。
- 六个平台均在启动时上报一次安装事件；Android、iOS、macOS、Linux、Windows 还会在崩溃时上报（HarmonyOS 暂无崩溃上报）。字段、去重与离线重试行为按平台不同，逐条见[网络请求与数据流向](PRIVACY.md)，那里同时记录发送内容、目的地、代码位置和关闭方式。

#### 工程与文档

- 输入引擎从取回并打 overlay 的 C++ 归档改为仓库内的 Rust crate：`engine-lock.json`、`scripts/fetch_engine.py`、`scripts/relock_engine.py`、全部 `scripts/apply_engine_*.py` 与 `scripts/engine-overlays/`、每周重锁与自动合并的两个工作流、`crates/engine-bridge` 及其 Boost／fmt／spdlog 构建依赖一并删除，构建不再需要 C++ 工具链或 `vendor/MSIME-Engine`。行为基准从 C++ 参考实现录制在 `crates/engine/tests/golden/`，录制方法见 `tools/engine-golden/README.md`；平台仍用的 IPC 契约头文件、辅助码表与 miniaudio 改为随仓库提交（`shared/contracts/`、`resources/helpcodes/`、`platforms/windows/third_party/miniaudio/`）。Google 整句解码器及其 `dict_pinyin.dat`、Whisper 本地文件识别随之去掉。
- `resources/*.lock.json` 固定随包词库与模型的 URL、长度与 SHA-256。
- `scripts/verify-local.sh` 提供本地统一验证，分快速门禁与完整两档；长期失败集中记在 `scripts/known-failures.txt`，每条附完整取证记录，比对只对不在清单里的失败名报错。
- 静态与契约门禁以独立脚本形式进入本地验证，覆盖配置键覆盖率、界面动作覆盖率、源码清单与设置页产物一致性。
- 整句转换评测与重排延迟测量各有固定数据集与基线文件，可在本地复跑。
- Git 钩子提供亚秒级的 `pre-commit` 检查（冲突标记、Rust 与前端格式），`pre-merge-commit` 与 `pre-push` 运行快速验证。
- 前端接入 Oxlint 与 Oxfmt，与 Rust 侧 clippy／rustfmt 对应；`packages/ui/src/upstream` 与 `apps/desktop/src-tauri/gen` 保持上游原样。
- 文档：[架构说明](ARCHITECTURE.md)、[实施记录](docs/implementation.md)、[网络请求与数据流向](PRIVACY.md)、[第三方组件清单](docs/third-party.md)、[开源发布清单](docs/open-source-release.md)、贡献指南、安全策略、行为准则（Contributor Covenant 2.1）以及问题与拉取请求模板。

### 验证

- 共享 Rust/C++ 边界、资源校验、配置冲突与宿主接口由 workspace 测试覆盖，入口是 `bash scripts/verify-local.sh`。
- 六个平台各有自己的测试套件与构建入口，命令和范围记录在对应平台 README：macOS 与 Linux 走 CTest，Windows 在本机边界构建与 Wine 下运行交叉产物，Android 分 JVM 冒烟与设备套件，iOS 走 Xcode 测试 scheme，HarmonyOS 走 ArkTS 逻辑测试与 HAP 打包。
- GitHub Actions 在 Pull Request 上运行质量、依赖审查与仓库契约检查，macOS 与 iOS 有独立工作流，Android、Linux、HarmonyOS 与 Windows 由 Native Platform CI 在对应平台或共享层有改动时触发；CodeQL 每日扫描。

### 发布说明

- 源码使用 GPL-3.0-only；第三方依赖、固定 Engine 归档、词库、模型和平台 SDK 仍须遵守各自许可证与通知要求。
- 提交前请执行 `bash scripts/verify-local.sh --quick`，并按 [开源发布清单](docs/open-source-release.md) 检查敏感文件、来源与通知文件。
- 六个平台各有独立的发布工作流，手动触发并按各自的 `platforms/<平台>/version.txt` 取版本号。

[Unreleased]: https://github.com/metasequoiaime/msime/compare/develop...HEAD
