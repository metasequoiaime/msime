# Agent Note: iOS 识别服务预设读取共享语音目录

Status: implemented

## Problem

iOS 的 `VoiceProviderPreset` 自己保存联网识别服务的默认地址、模型列表和文档链接，以及豆包流式接口的地址、标题。共享 Rust 的 `client-providers::voice::provider_catalog` 已经持有同一组数据，Android 和 Windows 通过现有 C ABI 消费。Swift 再维护一份会让原生选择器与共享运行时的默认值继续漂移。

## Decision

- `App/Services/VoiceProviderCatalog.swift` 通过已有 `msime_client_voice_provider_catalog` 读取静态目录，严格解码标准信封、识别服务的标识、地址、模型列表与文档链接，以及流式接口的标识、标题和地址。目录数据不涉及偏好、文件、网络或设备。
- 豆包默认地址按 `asr` 中的 `doubao` 标识查找，不依赖服务数组下标；流式接口保持共享目录的顺序和数量，不固定为两项。Swift 字符串持有解码结果，C 字符串在返回前通过 `msime_client_string_free` 释放。
- 静态快照在进程中只读一次。目录无法取得或解码失败时不复制宿主默认表兜底；默认地址为空、预设列表为空。已保存的地址仍由现有配置读取，不因目录加载、绘制或选项更新改写。
- `VoiceProviderPreset` 的联网服务地址，以及 EveryAPI、OpenAI、SiliconFlow、Groq、Mistral、百炼的模型列表和文档链接转发该快照。现有枚举的大小写原始值继续用于 UserDefaults，只在查询目录时转成共享小写标识；iOS 的服务顺序和显示名称、本地、系统与自定义服务保持。豆包现有模型提示和不显示文档链接的约定保持，流式 `Picker` 继续以地址作为选择值和标签。
- iOS 新预设仍选模型列表首项，不改用共享条目的 `model` 字段：OpenAI 的列表首项是 `gpt-4o-mini-transcribe`，共享默认模型是 `whisper-1`。已保存的地址和任意自定义模型继续优先恢复，EveryAPI 旧模型的只读退役逻辑不变。
- App 自动编入服务目录；`project.yml` 为两个显式列出 `CustomService.swift` 的测试 target 加入读取层，避免仅 App 编得过。新测试归现有 `ServiceTests`，由 iOS CI 的服务测试运行。

## Alternatives considered

- **继续同步 Swift 常量并增加逐字比较**：不改运行时调用，纯 Swift 测试简单；但两套表继续存在，每次共享目录变化仍需修改宿主，因此读取已存在的纯 C ABI。
- **把全部 iOS 服务选择器一起重写**：可以一次消除更多目录，但本机服务、自定义服务、模型首选项和 iOS 的服务名称各有既有契约；这里仅替换共享数据的来源，保留原生枚举、保存和选择流程，AI 服务目录另行迁移。

## Consequences

共享目录对识别服务地址、模型建议、文档链接以及豆包流式接口顺序与标题的变更会直接反映到 iOS 原生选择器。没有新增 ABI、依赖、偏好字段或网络请求；目录不可用时不会以另一份默认表掩盖问题。加载失败的进程不会自动重试，因为这个目录来自同包内的静态共享库。

检索 `VoiceProviderPreset`、`CustomService.swift`、豆包、流式接口和 iOS 活跃笔记后，没有已有的 Swift 目录读取决定；[Windows 原生语音页](../feature/2026-10-11-windows-settings-voice-page.md) 持有共享 C ABI 目录及 Windows 绑定，本片复用接口。[iOS 按住说话](../feature/2026-10-21-ios-hold-to-talk-voice-session.md) 持有录音会话与键盘交接，不改变其职责。

## Verification

- 初始读取层测试先因类型尚不存在而编译失败，再实现；元数据扩展的五类错误类型与缺字段输入先被旧读取层错误接受，五处断言失败，实现严格解码后通过。
- macOS 原生 XCTest 静态链接当前 Rust Host API，31 项现有 `CustomServiceTests` 加 8 项目录测试共 39 项通过。覆盖真实 C ABI 的释放与快照保有、三项合成流式目录、重排服务、元数据与模型顺序、未知服务、六种 HTTP 服务映射和已存地址/自定义模型恢复；OpenAI 首项默认值、豆包特例、本地/系统/自定义服务及 EveryAPI 旧模型迁移均保持。本机动态库出现 `LINKEDIT` 链接错误，改用 iOS 采用的静态链接方式执行这些测试，没有改变产品链接配置。
- Swift 6 严格并发类型检查、25 项工程配置测试、`pnpm run verify-notes`、`git diff --check` 通过，独立只读审查无阻断。既有 App 和两个服务测试 target 的接线保持，不新增工程文件。
- `bash scripts/verify-local.sh --quick` 完整通过，含契约、Rust workspace、Android 交叉检查、Linux 共享桌面检查与共享 Apple bridge。验证复用无进程占用的既有缓存，没有新建完整构建树。本机未启动 iOS 原生 App 或模拟器，不以 macOS 上的 Swift 测试代替设备结果；iOS 工程编译与模拟器测试由 PR CI 覆盖。
