# Agent Note: 共享 UI 与 Windows 共用 Rust AI 服务目录

Status: implemented

## Problem

共享设置页与 Windows 原生 AI 页分别维护十三项服务的标题、地址、默认模型、建议模型与文档。两份生产模块的实际运行结果逐项相等，UI 省略的可选字段也已固定基线；重复维护会让展示默认值漂移。语音目录的模型策略不同，iOS 的 Groq 建议模型也不同，不能仅因地址相同就混用。

## Decision

复用已合入 Android 的 `client-providers::ai::provider_catalog::PROVIDERS` 作为唯一纯公开静态目录，按原共享页顺序导出 JSON 与 C++17 宽字符串头。共享 UI 直接消费 JSON，保留 Ollama 无 `models`、自定义项无 `models/documentation` 的对象形状。Windows 沿用 `Provider` 字段和静态 `const vector` 引用生命周期，从生成表取得相同内容，不引入运行时文件、JSON 解析或 ABI 初始化。

`ui_provider_catalog` 投影省略空建议、空文档及 `desktop_only`，保留 UI 历史字段形状；Android C ABI 继续序列化完整目录，保留必需的 `models`、`documentation`、`desktop_only`，也保留服务切换补丁。两种协议不通过改全局序列化注解互相迁就。

精确 id 匹配、未知值回退、换服务时自定义字段优先、Ollama 桌面过滤与免 Token、图标资源号保持；偏好的首次默认值、iOS 和语音目录仍各自负责原有策略。

iOS 也消费同一导出器生成的静态 Swift 元数据，保留其枚举标识、模型首项默认策略及 Groq 差异，见 [iOS AI 预设](2026-10-22-ios-ai-provider-catalog.md)。

## Existing notes audit

- [预设公共模型](../../implemented/simplification/2026-10-16-provider-preset-model.md)持有 TypeScript 字段契约，部分重叠，继续复用类型。
- [Windows AI 页](../../implemented/feature/2026-10-11-windows-settings-ai-page.md)持有原生页行为，部分重叠，同步其目录来源。
- [Ollama](../../implemented/feature/2026-10-20-ai-ollama-provider.md)持有平台与鉴权策略，保留决定。
- [语音静态目录](../../implemented/simplification/2026-10-21-shared-ui-voice-catalog.md)提供已验证的生成模式，AI 数据归独立目录；[iOS 语音目录](../../implemented/simplification/2026-10-21-ios-voice-provider-catalog.md)不拥有 AI 服务预设。

## Alternatives considered

- 经 C ABI 异步读取：可以减少生成物；但现有两个消费者同步读取静态数据，改动会新增加载、释放、解析及失败生命周期。
- 直接共用语音服务表：现有字段结构相近，可少定义一个类型；但 AI 候选与语音润色的服务集合、模型和平台策略不同，合表会改变原有行为。
- Windows 运行时读取 JSON：可只生成一种文件；但会引入原来没有的路径、文件和解析失败，宽字符串静态表能沿用现有生命周期。

## Verification

合成 JSON 的两项消费者回归先在手写目录上失败，接入后通过；真实 Windows 接线探针同样先红后绿，验证集合删除、新服务、Unicode、默认模型不同于建议首项、自定义值优先和未知回退。UI 生产模块经 TypeScript 编译执行，Windows 生产头经 C++17 编译执行，十三项输出分别与原固定运行基线完全相同，包含 UI 可选字段形状。Rust 目录的唯一 id、偏好集合与自定义回退、可选字段回归通过。

UI 四文件回归共 412 项通过，类型检查、生产构建和定向 lint 通过。Rust 导出器与目录测试的严格 clippy、全仓 Rust 格式检查通过。生成物缺失或内容过期时，导出器检查拒绝继续；恢复后检查通过。独立审查未发现实质问题。

完整本地 quick 通过：Rust、Android 目标及宿主、Wasm、HarmonyOS HAP、Linux 桌面层和原生宿主、macOS 与共享 Apple 桥构建通过，Linux 原生测试 109 项通过。Windows 可移植宿主测试 176 项通过；完整 Windows 交叉构建及 pipe-only 配置因缺少 MinGW 跳过，x86 语法检查缺少 x64 构建参数，完整原生构建由 PR CI 覆盖。本片未进行设备或 GUI 验收。

合并 Android 目录后，目录与切换规则十项、C ABI 三项和 UI 五文件二十二项回归通过；导出器检查与定向严格 Clippy 通过，完整 quick 再次通过（Linux 原生 111 项、Windows 可移植宿主 176 项）。新增回归明确检查 Android 完整字段与 UI 缺省字段互不影响。

## Consequences

生成物需要与 Rust 同步，CI 使用真实导出器检查漂移。Windows 的未知服务依赖最后一项自定义服务；目录测试固定这一边界。平台过滤和免鉴权仍由现有策略负责，本片只收敛展示元数据。
