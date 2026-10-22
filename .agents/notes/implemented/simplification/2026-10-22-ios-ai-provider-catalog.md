# Agent Note: iOS AI 预设复用共享静态目录

Status: implemented

## Problem

`AIProviderPreset` 在 Swift 中重复维护十二项 AI 服务的标题、地址、模型建议和文档。实际编译生产枚举取得的基线表明，除 Groq 的第二项模型和文档链接外，各字段与共享 Rust 目录一致；Ollama 不在 iOS 枚举中。枚举原始值和顺序还承担现有偏好槽位与选择器契约，不能直接改成共享小写标识。

## Decision

扩展现有 `export_ai_provider_catalog`，从 `PROVIDERS` 生成静态 Swift 元数据。iOS 原生枚举按 `rawValue.lowercased()` 精确查询，保留原有枚举集合、顺序、落盘值、Groq 两处平台差异和自定义服务的空模型及无文档策略。模型默认值仍取 iOS 建议列表首项，不转用共享 `model` 字段。已保存字段和凭据流程不改。

App 自动编入生成文件，两个显式列出 `CustomService.swift` 的测试 target 同时接入。CI 延续现有导出器漂移检查；合成目录探针编译生产枚举，验证字段来源、模型顺序、Unicode、缺失服务与平台差异。

## Existing notes audit

- [共享 AI 目录](../../implemented/simplification/2026-10-22-shared-ai-provider-catalog.md)持有 Rust owner 与生成模式，部分重叠，继续复用；iOS 作为新消费者单独记录平台差异。
- [iOS 语音目录](../../implemented/simplification/2026-10-21-ios-voice-provider-catalog.md)持有同一 Swift 文件中的语音枚举及读取协议，部分重叠，AI 本片不改语音读取或请求策略。
- [预设公共模型](../../implemented/simplification/2026-10-16-provider-preset-model.md)持有 TypeScript 换预设规则，本片不改其归属。

## Alternatives considered

- 直接读取已有 AI C ABI：能沿用语音目录的桥接模式，不提交生成文件；但 AI 枚举目前同步读取编译期数据，新增分配、解码和加载失败生命周期没有必要。静态投影保留现有读取时机与无运行时 I/O 的性质。
- 把 Groq 也改成共享建议：可以彻底删除平台常量；但两份现有建议和文档并不相同，统一会改变用户可见行为，超出本片所有权抽取范围。

## Verification

生产枚举编译后的十二项输出与固定运行基线完全一致，覆盖原始值、顺序、标题、地址、建议模型、模型首项和自定义项的 `nil` 文档。合成目录探针在旧实现上有八个预期失败，接入后通过；探针同时覆盖字段来源、服务缺失、Unicode、Groq 平台差异与枚举持久化标识。Swift 6 严格并发编译通过；生成器实际转义函数的六组合成值经 Rust 生成与 Swift 编译运行后逐字恢复，覆盖引号、反斜杠、插值前缀、控制字符、行分隔符与非 BMP 字符。

macOS 原生 XCTest 静态链接真实 Rust Host API，既有服务配置与语音目录测试加本片三项共 42 项通过；覆盖新预设取模型首项、已保存字段优先和 Codable 标识。26 项 iOS 工程配置检查通过。XcodeGen 重建现有工程时，同时补齐 `project.yml` 已声明但旧工程缺失的 `VoicePolishPresetCatalog.swift` 引用，没有改变语音逻辑。

完整本地 `--quick` 通过，包含 Android JVM 冒烟、HarmonyOS HAP 构建、Linux 设置外壳与原生宿主、macOS 原生宿主和共享 Apple 桥接编译；Linux 原生套件 111 项零失败，其中 IBus 按需激活一项按环境跳过。本机没有 MinGW，Windows 原生交叉编译与 pipe-only 阶段跳过；macOS Tauri 应用因未暂存产品 bundle 跳过。没有运行 iOS GUI 或设备验收。生成器对 Swift 文件缺失和过期都返回非零，恢复后 `--check` 通过。独立审查通过。

## Consequences

生成物需要与 Rust 同步；由已有导出器的 `--check` 检查。平台枚举的持久化标识继续归 iOS，新增共享服务不会自动成为 iOS 可选项。缺失元数据不恢复另一份手写表，目录覆盖由回归测试约束。
