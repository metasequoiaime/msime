# Agent Note: 共享语音服务目录由 Rust 导出

Status: implemented

## Problem

共享 React 设置页重复保存 Rust 语音目录已有的识别服务、润色服务和豆包流式接口元数据。默认地址、建议模型、文档链接和选择器标题因此需要分别维护。鸿蒙的润色器、HTTP 识别策略与初始语音配置还分别手写同一目录中的默认地址与模型，HTTP 识别另有一份服务到请求格式的映射，运行时与设置页可能漂移。Linux 语音脚本也重复保存两类服务集合、默认地址与模型，以及整句上传格式。macOS 的运行时与原生回退窗口共用另一份七项识别服务表，名称、默认地址和模型与共享目录相同，顺序是宿主策略。

## Decision

保留 `client-providers::voice::provider_catalog` 的常量为事实源，以 Cargo example 序列化现有公开结构并提交 JSON。React 同步投影 JSON，继续提供原有默认表、选择器与更新函数；系统识别和本地模型仍由 UI 追加。保留公开凭据服务数组的历史顺序、空模型列表省略字段的形状，以及自定义值优先规则。

导出器保留无参时向标准输出生成 JSON 的接口，新增 `--write` 同步更新 JSON 与鸿蒙的静态 TypeScript 类。`--check` 比较提交 JSON 的语义值（空白与键顺序交给前端格式化），并逐字检查 TypeScript；继续由 Linux 设置应用 CI 在既有 Rust 依赖构建后执行，避免轻量 Repository contracts 作业单独编译网络依赖。

鸿蒙的 `VoiceProviderCatalog.polishEndpoint` 和 `polishModel` 使用带显式类型的静态 switch，同步返回字符串，未知 id 返回空串。`HarmonyVoicePolisher` 继续先 trim provider，配置中的自定义地址和模型仍优先；`DEFAULT_VOICE_INPUT_CONFIGURATION` 只把原有 SiliconFlow 的地址、模型来源换为生成方法。初始 provider 的平台选择、凭据槽位适配、请求校验、取消和润色正文规则继续由宿主持有。生成类不暴露可变表，不增加 NAPI/JSON 初始化，也不在产品运行时访问文件、网络或偏好。

`VoiceProviderCatalog.httpAsrEndpoint`、`httpAsrModel` 和 `httpAsrRequestFormat` 同样使用静态 switch。导出器遍历 `ASR_PROVIDERS`，通过真实 `voice::provider::asr_request_format` 只保留 `multipart` 与 `chat_audio`；豆包流式、本地、系统和未知服务不进入 HTTP 路径。`HttpAsrConfigurationPolicy` 同步读取这三项，未知 id 的默认值仍为 `null`，每次返回新的默认对象，provider 的大小写与空白继续按原样匹配。自定义地址、模型的 trim 与优先级、凭据槽位、请求预检、语种和请求体继续由原策略负责。构建工具的五个方法共用同一个字符串转义与 switch 渲染函数。

Linux 的生成模块 `msime_voice_provider_catalog.py` 同步导出 `ASR_PROVIDERS`、`POLISH_PROVIDERS`、`DEFAULT_ENDPOINTS`、`DEFAULT_MODELS` 与 `REQUEST_FORMATS`。两类服务集合由 Rust 目录的默认模型表取键，保持原有 Python set/dict 形状；HTTP 格式使用同一个真实 `asr_request_format` 筛选结果，豆包仍在识别目录、但不在 HTTP 格式表，本地识别仍由宿主单独追加。消费者直接导入原有五个名称，私有配置、profile 校验、凭据、trim、自定义值优先和请求构造继续由宿主负责。生成模块与润色提示词模块一同安装在各版本的可执行文件目录，不增加 JSON 文件初始化或新的网络访问。

macOS 同步读取同一导出器生成的 `shared/voice/VoiceProviderCatalog.h`。共享表在 `msime::voice` 中以 C++17 `constexpr std::array` 持有识别服务的 id、标题、默认地址和模型，UTF-8 转为 `NSString` 时显式传入字节长度，沿用已有润色目录的静态头模式。宿主只保存历史 id 优先级，缺失服务不补回，新服务按共享目录顺序排在历史项之后，最后追加系统与本地识别；原有小写匹配、空白不 trim、未知值返回空串和云服务判断保持。目录在 `dispatch_once` 中投影为不可变数组，不增加 C ABI 初始化或 JSON 解析。首次润色仍按 macOS 的 DeepSeek 策略；冻结的 AppKit 回退窗口不增加功能。

## Alternatives considered

- 直接从宿主异步读取 C ABI：可以免除生成物，但共享 UI 的预设和更新接口目前同步，ArkWeb 与 Tauri 又走不同桥，改动会扩展到加载生命周期和调用方。
- 鸿蒙经已有 NAPI 动态读取目录：能复用真实 ABI，不用增加静态投影；但会让原来同步可靠的默认值读取依赖原生库加载和信封解析，纯键盘策略测试也不能直接消费。默认值随版本固定，沿用静态导出可以保持该路径的初始化与失败行为。
- Rust 和 TypeScript 共同读取 JSON：无需生成，但 Rust 的常量切片需要改成运行时解析和分配，引入新的失败行为，超出静态目录迁移。
- Linux 运行时读取共享 JSON：可以直接安装已有 JSON、免去 Python 投影；但会给原来只导入模块的初始化增加 JSON 路径、读取和解析失败，安装结构也要多维护一类数据文件。沿用已有润色提示词的普通模块导入和安装方式，可以保持配置与启动边界。
- macOS 经已有 C ABI 读取目录：可直接使用 Rust 的运行时结果、减少生成文件，iOS 和 Windows 已有此路径；但这里原来是同步静态值，改用 ABI 会引入字符串释放、信封解析与读取失败的生命周期。现有润色目录已采用共享 C++ 静态头，沿用它可以消除副本并保持初始化与失败边界。
- 文本脚本解析 Rust：无需编译，但会把 Rust 表达式与字符串语法的解释复制到脚本，不能可靠复用现有序列化。

## Consequences

共享 UI 只维护模型到现有 API 的投影，端点、默认模型、建议模型、文档和标题随 Rust 目录更新。提交的 JSON 和鸿蒙 TypeScript 增加生成物维护成本，以同一真实 Rust 导出检查阻止漂移。鸿蒙的四项润色与六项 HTTP 识别服务地址、模型、HTTP 格式映射和初始配置不再各自保存一份字面量；Linux 的两类服务集合、地址、模型与 HTTP 格式也由同一导出器更新。Python 模块必须随语音服务安装，full 与各独立版本不能漏装；配置选择仍按平台策略，凭据字段仍由宿主适配，新增服务需要同时接好对应槽位。兼容顺序只保存标识优先级，不复制端点、模型或标题；新增服务排在历史项之后，选择器仍遵循 Rust 顺序。macOS 的生成头也要随目录更新；原生回归直接对照共享数组与真实公开接口，固定宿主的九项历史顺序和查找边界。若以后共享 UI 接入统一的异步目录桥，可重访静态生成方案。

检索活跃笔记中的 `provider_catalog`、`ASR_PROVIDER_DEFAULTS` 和语音服务目录。[iOS 目录读取](2026-10-21-ios-voice-provider-catalog.md)拥有 Swift C ABI 读取，[Windows 语音页](../feature/2026-10-11-windows-settings-voice-page.md)拥有共享目录与原生页绑定；本篇持有 React、鸿蒙、Linux 与 macOS 默认值的静态投影，不改变那些决定；[macOS 语音设置精简](2026-10-16-macos-voice-settings-prune.md)继续持有平台行为与回退窗口冻结边界。[预设公共模型](2026-10-16-provider-preset-model.md)仍持有 `ProviderDefaults` 字段契约，投影继续复用它。[百炼请求格式](../feature/2026-10-09-bailian-asr-request-format.md)部分重叠于 HTTP 格式契约；这里消除鸿蒙的映射副本，上传格式和非 HTTP 的边界不变。[EveryAPI 默认模型修复](../bug-fix/2026-10-12-everyapi-asr-default-model.md)保留原修复理由，鸿蒙目前从共享目录读取当时固定的模型。

## Verification

- 合成目录的三项测试先在旧实现上失败，再接入 JSON 后通过，覆盖新增和移除服务、重排目录、默认模型不同于建议列表首项、空 `models` 的字段形状、三项流式接口和自定义值优先。
- `pnpm --filter @msime/desktop test tests/voice`：19 文件、121 项通过；独立审查另跑 7 文件、36 项通过，无实际阻断。临时基线对照验证六组公开元数据与重构前完全相等，临时测试与基线文件随后移除。
- 导出 `--check` 在当前 JSON 上通过，手工修改一项模型后非零退出，恢复后再次通过。检查在运行时读取提交文件，避免检查二进制内嵌的是旧 JSON；文件读取仅在构建工具，产品运行时不增文件访问。
- 前端类型检查、生产构建、定向 lint 和 Rust exporter clippy 通过。没有启动原生宿主或执行设备验收，本片只更换共享 UI 静态元数据来源。
- 完整 `bash scripts/verify-local.sh --quick` 通过：Rust workspace、Android 交叉检查与宿主契约、Wasm、Linux 设置应用、Linux 原生 108 项测试与双版本构建、macOS 原生构建和共享 Apple bridge。Windows 可移植测试 164 项通过；缺少 MinGW 的 Windows 原生交叉构建、缺少 staged 输入的 HarmonyOS 原生包按门禁明确跳过。复用无进程占用的既有构建缓存，按固定路径重配 CMake。

- 鸿蒙目录回归先因生成模块缺失而失败，生成后严格 TypeScript 编译和执行通过；固定四服务八项默认值、未知/空/空格/大写 id 回退和三个初始配置字段。迁移前后真实润色请求初始化的 40 组配置逐字一致，覆盖 provider trim、缺省/null、地址与模型的自定义优先和空白回退。
- 合成接线探针临时替换生成类的地址与含中文、非 BMP 字符的模型，确认实际润色初始化与初始配置都读取生成值；旧宿主在同一探针上失败，导出器也拒绝陈旧 TypeScript。探针结束后恢复生成物。无参导出仍为纯 JSON，与固定 base 语义相等；`--check`、前端格式检查、`cargo fmt --all -- --check` 和 exporter 严格 clippy 通过。
- `platforms/harmony/tests/run.sh` 全套通过，独立审查的 40 组基线对照与单独严格编译运行也通过。真实原生库构建和 `hvigorw assembleHap --no-daemon` 通过，最终 HAP 包含 `ets/modules.abc`、`libmsimeclient.so`、`libmsime_host_api.so` 与 `libc++_shared.so`。未安装鸿蒙设备或运行输入法界面，此验证覆盖真实 ArkTS 编译和打包。
- 鸿蒙润色默认值迁移的完整 `bash scripts/verify-local.sh --quick` 通过：仓库契约、Rust workspace、Android 交叉检查与宿主 Java、Wasm、真实鸿蒙 HAP、Linux 设置应用与原生 108 项测试、双版本构建、macOS 原生和共享 Apple bridge 均完成。Windows 可移植测试 176 项通过，2 项由宿主差异排除、64 项需要 Windows 构建；本机未配置 MinGW/Windows 原生构建，x86 语法与 pipe-only 检查明确跳过。
- HTTP 识别迁移先固定六项预设、七类非 HTTP/不规范 id 和真实策略解析的 104 组配置。合成生成目录在旧消费者上以“未读取生成请求格式”断言失败，接入后实际服务集合、格式、默认值与预检均读取合成值，结束后恢复生成物。迁移前后 104 组配置逐字一致；回归还检查自定义字段 trim 优先、空白回退和默认对象修改隔离。
- HTTP 生成物的 `--check` 在旧 TypeScript 上先失败，重新生成后通过；临时修改识别模型后也非零退出，恢复后通过。无参仍只输出与固定 base 语义一致的 JSON，原润色生成方法逐字相等；严格 TypeScript 编译与新 HTTP 回归、定向格式检查通过。
- HTTP 迁移的 `platforms/harmony/tests/run.sh` 全套为 `0 failure(s)`，`cargo fmt --all -- --check`、exporter 的 `clippy -D warnings` 与笔记门禁通过。独立审查严格编译并运行新增 HTTP 和既有润色回归、104 组真实配置对照，未发现实质问题。最终 `hvigorw assembleHap --no-daemon` 通过真实 `CompileArkTS`，28 秒完成；HAP 内核实 `ets/modules.abc` 与三项 arm64 原生库。未安装鸿蒙设备或运行输入法 UI。
- HTTP 迁移的完整 `bash scripts/verify-local.sh --quick` 通过：仓库契约、Rust workspace、Android 交叉检查和宿主 Java、Wasm、真实鸿蒙 HAP、Linux 设置应用、原生 108 项测试与 full/五笔双版本构建、macOS 原生和共享 Apple bridge 均完成。Windows 可移植测试 176 项通过；2 项由宿主差异排除、64 项需要 Windows 构建，未配置 MinGW 的原生交叉构建、x86 语法与 pipe-only 明确跳过。
- Linux 迁移前固定七项识别和四项润色的真实公开目录，并用真实 `load_config` 记录 484 组合法/非法、自定义/默认及 profile 配置结果。注入合成模块的消费者探针在旧实现上以“识别服务集合仍是宿主副本”失败，确认测试能识别未接入生成数据的宿主。
- Linux 消费者接入生成模块后，同一合成探针与两项持久回归通过：真实配置读取合成服务集合、默认地址/含中文与非 BMP 字符的模型、大小写 profile、本地 profile 与自定义值，实际请求按合成的 chat_audio 生成正文；移除的旧服务被真实校验拒绝。迁移前后 484 组配置解析逐字一致，原鸿蒙生成类逐字不变。
- Linux 原生容器门禁通过：109 项 CTest 中 108 项通过，`linux-ibus-demand-activation` 按环境条件跳过，零失败，full 与五笔版原生构建、两套安装路径隔离、辅助码、五笔宿主库链接和日文版登记检查完成。再用容器 Python `-I` 分别加载两套已安装的语音服务，两项目录与真实配置回归各自通过；新模块确实从各自安装目录导入。未启动真实 IBus/Fcitx5 输入会话。
- Linux 独立审查未发现实质问题，另跑九项定向测试并核对原有五个公开表的名称、set/dict 类型和值、无参 JSON 语义和鸿蒙输出字节。导出器缺少新模块或模型被篡改时均拒绝，恢复后通过；定向 Python 共 14 项、格式与严格 clippy 通过。
- Linux 迁移的完整 `bash scripts/verify-local.sh --quick` 以 `quick check passed` 结束：仓库契约、Rust workspace、Android 交叉检查和宿主 Java、Wasm、真实鸿蒙 HAP、Linux 设置应用、原生 108 项通过与 1 项条件跳过、双版本构建、macOS 原生和共享 Apple bridge 均完成。Windows 可移植测试 176 项通过，2 项由宿主差异排除、64 项需要 Windows 构建；未配置 MinGW 的原生交叉构建、x86 语法与 pipe-only 明确跳过。
- macOS 的真实生产源码在合成生成头上先以“仍读取手写服务集合”断言失败，接入后同一探针通过：服务删除/新增、宿主历史顺序、系统/本地追加、标题、默认地址与含中文及非 BMP 字符的模型都来自共享表，大小写、空白、未知和空值查找保持。探针在 `finally` 中恢复生成头。真实 CMake 生产对象与固定基线的九项服务和七类查找结果完全相同。
- macOS 的 `voice-provider-catalog`、`voice-provider-settings-keys` 和 `voice-provider-settings-validation` 三项 CTest 通过；独立只读审查重复这三项、导出检查和固定基线对照，未发现实质问题。生成头缺失或被篡改时 `--check` 均拒绝，恢复后通过；无参 JSON 语义、鸿蒙 TypeScript 和 Linux Python 原字节保持。Rust 格式、exporter 严格 clippy 与笔记门禁通过。
- macOS 迁移的完整 `bash scripts/verify-local.sh --quick` 以 `quick check passed` 结束：仓库契约、Rust workspace、Android 交叉检查与宿主 Java、Wasm、真实鸿蒙 HAP、Linux 设置应用、原生 108 项通过与 1 项条件跳过、双版本构建、macOS 原生和共享 Apple bridge 均完成。Windows 可移植测试 176 项通过，2 项由宿主差异排除、64 项需要 Windows 构建；未配置 MinGW 的原生交叉构建、x86 语法与 pipe-only 明确跳过。没有安装或启动真实 macOS 输入法 UI。
- 首轮 PR 的语音回归通过，期间合入的 KeyChord 契约让 macOS 偏好覆盖出现三项未登记，另有偏好测试缺少函数闭合和 `#[test]`。同步最新 develop 后按[快捷键契约原笔记](../feature/2026-10-21-rebindable-toggle-hotkeys.md)补全阶段说明与测试结构；先复现覆盖失败和 Rust 测试编译失败，修复后覆盖脚本及真实 CTest、六项快捷键与既有三态循环回归、Rust 格式检查通过。更新基线后的完整 quick 再次以 `quick check passed` 结束，各平台结果及跳过边界与上项一致。
