# Agent Note: iOS 豆包接口预设读取共享语音目录

Status: implemented

## Problem

iOS 的 `VoiceProviderPreset` 自己保存豆包默认 WebSocket 地址和两项流式接口的地址、标题。共享 Rust 的 `client-providers::voice::provider_catalog` 已经持有同一组数据，Android 和 Windows 通过现有 C ABI 消费。Swift 再维护一份会让原生选择器与共享运行时的默认值继续漂移。

## Decision

- `App/Services/VoiceProviderCatalog.swift` 通过已有 `msime_client_voice_provider_catalog` 读取静态目录，严格解码标准信封、识别服务的标识与地址，以及流式接口的标识、标题和地址。目录数据不涉及偏好、文件、网络或设备。
- 豆包默认地址按 `asr` 中的 `doubao` 标识查找，不依赖服务数组下标；流式接口保持共享目录的顺序和数量，不固定为两项。Swift 字符串持有解码结果，C 字符串在返回前通过 `msime_client_string_free` 释放。
- 静态快照在进程中只读一次。目录无法取得或解码失败时不复制宿主默认表兜底；默认地址为空、预设列表为空。已保存的地址仍由现有配置读取，不因目录加载、绘制或选项更新改写。
- `VoiceProviderPreset` 的豆包默认地址和接口列表转发该快照；原生 `Picker` 继续以地址作为选择值和标签，原有按服务保存与恢复地址的行为保持。其他服务的元数据在后续切片迁移。
- App 自动编入服务目录；`project.yml` 为两个显式列出 `CustomService.swift` 的测试 target 加入读取层，避免仅 App 编得过。新测试归现有 `ServiceTests`，由 iOS CI 的服务测试运行。

## Alternatives considered

- **继续同步 Swift 常量并增加逐字比较**：不改运行时调用，纯 Swift 测试简单；但两套表继续存在，每次共享目录变化仍需修改宿主，因此读取已存在的纯 C ABI。
- **把全部 iOS 服务选择器一起重写**：可以一次消除更多目录，但本机服务、自定义服务、模型首选项和 iOS 的服务名称各有既有契约；本片先迁移豆包接口数据，保留保存和选择流程，其他迁移逐片验证。

## Consequences

共享目录对豆包地址、顺序与标题的变更会直接反映到 iOS 原生选择器。没有新增 ABI、依赖、偏好字段或网络请求；目录不可用时不会以另一份默认表掩盖问题。加载失败的进程不会自动重试，因为这个目录来自同包内的静态共享库。

检索 `VoiceProviderPreset`、`CustomService.swift`、豆包、流式接口和 iOS 活跃笔记后，没有已有的 Swift 目录读取决定；[Windows 原生语音页](../feature/2026-10-11-windows-settings-voice-page.md) 持有共享 C ABI 目录及 Windows 绑定，本片复用接口。[iOS 按住说话](../feature/2026-10-21-ios-hold-to-talk-voice-session.md) 持有录音会话与键盘交接，不改变其职责。

## Verification

- 新增合成目录测试先因读取层尚不存在而编译失败，再实现后编译；三项重排预设与非首位豆包服务证明数据不依赖原生硬编码的数量、标识、标题或默认项。
- 原生 XCTest 通过真实 C ABI 读取并释放两份目录，核对快照仍可用；坏信封、缺失服务与字段类型错误被拒绝。macOS 上 31 项现有 `CustomServiceTests` 加 4 项目录测试共 35 项通过；Swift 6 严格并发类型检查、25 项工程配置测试、正式脚本的固定资源校验暂存和 XcodeGen 工程生成均通过。本机未启动 iOS 原生 App 或模拟器，不以 macOS 上的 Swift 测试代替设备结果。
- `bash scripts/verify-local.sh --quick` 完整通过，含契约、Rust workspace、Android 交叉检查、Linux 共享桌面检查与共享 Apple bridge；`pnpm run verify-notes` 和 `git diff --check` 通过。迁移缓存遗留的 Tauri 绝对路径通过定向重建修复，Linux 复用已合并且无进程占用的既有缓存，未新建完整构建树。
