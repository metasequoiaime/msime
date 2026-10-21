# Agent Note: 共享设置页的语音目录由 Rust 导出

Status: implemented

## Problem

共享 React 设置页重复保存 Rust 语音目录已有的识别服务、润色服务和豆包流式接口元数据。默认地址、建议模型、文档链接和选择器标题因此需要分别维护。

## Decision

保留 `client-providers::voice::provider_catalog` 的常量为事实源，以 Cargo example 序列化现有公开结构并提交 JSON。React 同步投影 JSON，继续提供原有默认表、选择器与更新函数；系统识别和本地模型仍由 UI 追加。保留公开凭据服务数组的历史顺序、空模型列表省略字段的形状，以及自定义值优先规则。

导出器的 `--check` 比较生成结果和提交 JSON 的语义值（空白与键顺序交给现有前端格式化）；接入 Linux 设置应用 CI，在既有 Rust 依赖构建后运行，避免轻量 Repository contracts 作业单独编译网络依赖。正常生成输出到标准输出，不在产品运行时访问文件或网络。

## Alternatives considered

- 直接从宿主异步读取 C ABI：可以免除生成物，但共享 UI 的预设和更新接口目前同步，ArkWeb 与 Tauri 又走不同桥，改动会扩展到加载生命周期和调用方。
- Rust 和 TypeScript 共同读取 JSON：无需生成，但 Rust 的常量切片需要改成运行时解析和分配，引入新的失败行为，超出静态目录迁移。
- 文本脚本解析 Rust：无需编译，但会把 Rust 表达式与字符串语法的解释复制到脚本，不能可靠复用现有序列化。

## Consequences

共享 UI 只维护模型到现有 API 的投影，端点、默认模型、建议模型、文档和标题随 Rust 目录更新。提交的 JSON 增加生成物维护成本，以真实 Rust 导出检查阻止漂移。兼容顺序只保存标识优先级，不复制端点、模型或标题；新增服务排在历史项之后，选择器仍遵循 Rust 顺序。若以后共享 UI 接入统一的异步目录桥，可重访静态生成方案。

检索活跃笔记中的 `provider_catalog`、`ASR_PROVIDER_DEFAULTS` 和语音服务目录。[iOS 目录读取](2026-10-21-ios-voice-provider-catalog.md)拥有 Swift C ABI 读取，[Windows 语音页](../feature/2026-10-11-windows-settings-voice-page.md)拥有共享目录与原生页绑定；本片新增的是 React 的静态投影，不改变那些决定。[预设公共模型](2026-10-16-provider-preset-model.md)仍持有 `ProviderDefaults` 字段契约，投影继续复用它。

## Verification

- 合成目录的三项测试先在旧实现上失败，再接入 JSON 后通过，覆盖新增和移除服务、重排目录、默认模型不同于建议列表首项、空 `models` 的字段形状、三项流式接口和自定义值优先。
- `pnpm --filter @msime/desktop test tests/voice`：19 文件、121 项通过；独立审查另跑 7 文件、36 项通过，无实际阻断。临时基线对照验证六组公开元数据与重构前完全相等，临时测试与基线文件随后移除。
- 导出 `--check` 在当前 JSON 上通过，手工修改一项模型后非零退出，恢复后再次通过。检查在运行时读取提交文件，避免检查二进制内嵌的是旧 JSON；文件读取仅在构建工具，产品运行时不增文件访问。
- 前端类型检查、生产构建、定向 lint 和 Rust exporter clippy 通过。没有启动原生宿主或执行设备验收，本片只更换共享 UI 静态元数据来源。
- 完整 `bash scripts/verify-local.sh --quick` 通过：Rust workspace、Android 交叉检查与宿主契约、Wasm、Linux 设置应用、Linux 原生 108 项测试与双版本构建、macOS 原生构建和共享 Apple bridge。Windows 可移植测试 164 项通过；缺少 MinGW 的 Windows 原生交叉构建、缺少 staged 输入的 HarmonyOS 原生包按门禁明确跳过。复用无进程占用的既有构建缓存，按固定路径重配 CMake。
