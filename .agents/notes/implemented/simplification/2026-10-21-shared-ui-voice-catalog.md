# Agent Note: 共享语音服务目录由 Rust 导出

Status: implemented

## Problem

共享 React 设置页重复保存 Rust 语音目录已有的识别服务、润色服务和豆包流式接口元数据。默认地址、建议模型、文档链接和选择器标题因此需要分别维护。鸿蒙的润色器和初始语音配置还分别手写同一目录中的默认地址与模型，运行时与设置页可能漂移。

## Decision

保留 `client-providers::voice::provider_catalog` 的常量为事实源，以 Cargo example 序列化现有公开结构并提交 JSON。React 同步投影 JSON，继续提供原有默认表、选择器与更新函数；系统识别和本地模型仍由 UI 追加。保留公开凭据服务数组的历史顺序、空模型列表省略字段的形状，以及自定义值优先规则。

导出器保留无参时向标准输出生成 JSON 的接口，新增 `--write` 同步更新 JSON 与鸿蒙的静态 TypeScript 类。`--check` 比较提交 JSON 的语义值（空白与键顺序交给前端格式化），并逐字检查 TypeScript；继续由 Linux 设置应用 CI 在既有 Rust 依赖构建后执行，避免轻量 Repository contracts 作业单独编译网络依赖。

鸿蒙的 `VoiceProviderCatalog.polishEndpoint` 和 `polishModel` 使用带显式类型的静态 switch，同步返回字符串，未知 id 返回空串。`HarmonyVoicePolisher` 继续先 trim provider，配置中的自定义地址和模型仍优先；`DEFAULT_VOICE_INPUT_CONFIGURATION` 只把原有 SiliconFlow 的地址、模型来源换为生成方法。初始 provider 的平台选择、凭据槽位适配、请求校验、取消和润色正文规则继续由宿主持有。生成类不暴露可变表，不增加 NAPI/JSON 初始化，也不在产品运行时访问文件、网络或偏好。

## Alternatives considered

- 直接从宿主异步读取 C ABI：可以免除生成物，但共享 UI 的预设和更新接口目前同步，ArkWeb 与 Tauri 又走不同桥，改动会扩展到加载生命周期和调用方。
- 鸿蒙经已有 NAPI 动态读取目录：能复用真实 ABI，不用增加静态投影；但会让原来同步可靠的默认值读取依赖原生库加载和信封解析，纯键盘策略测试也不能直接消费。默认值随版本固定，沿用静态导出可以保持该路径的初始化与失败行为。
- Rust 和 TypeScript 共同读取 JSON：无需生成，但 Rust 的常量切片需要改成运行时解析和分配，引入新的失败行为，超出静态目录迁移。
- 文本脚本解析 Rust：无需编译，但会把 Rust 表达式与字符串语法的解释复制到脚本，不能可靠复用现有序列化。

## Consequences

共享 UI 只维护模型到现有 API 的投影，端点、默认模型、建议模型、文档和标题随 Rust 目录更新。提交的 JSON 和鸿蒙 TypeScript 增加生成物维护成本，以同一真实 Rust 导出检查阻止漂移。鸿蒙的四服务地址、模型与初始配置不再各自保存一份字面量；配置选择仍按平台策略，凭据字段仍由宿主适配，新增服务需要同时接好对应槽位。兼容顺序只保存标识优先级，不复制端点、模型或标题；新增服务排在历史项之后，选择器仍遵循 Rust 顺序。若以后共享 UI 接入统一的异步目录桥，可重访静态生成方案。

检索活跃笔记中的 `provider_catalog`、`ASR_PROVIDER_DEFAULTS` 和语音服务目录。[iOS 目录读取](2026-10-21-ios-voice-provider-catalog.md)拥有 Swift C ABI 读取，[Windows 语音页](../feature/2026-10-11-windows-settings-voice-page.md)拥有共享目录与原生页绑定；本片新增的是 React 与鸿蒙默认值的静态投影，不改变那些决定。[预设公共模型](2026-10-16-provider-preset-model.md)仍持有 `ProviderDefaults` 字段契约，投影继续复用它。

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
