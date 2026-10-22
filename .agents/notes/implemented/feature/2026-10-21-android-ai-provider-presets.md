# Agent Note: Android AI 设置读取共享服务商预设

Status: implemented

## Problem

#8758 的原生 Android「AI 润色与回复」只有端点、模型和凭据输入框，用户必须自己找到完整的 Chat Completions 地址和模型 id。共享设置页已有服务商预设与模型选择，但发布 APK 不承载该页面。AI 设计皮肤使用账号任务服务，与此配置独立，补齐设置入口不能作为皮肤生成成功的证明。

## Decision

由 `client-providers::ai` 提供公开服务商目录与切换补丁，经 Host API 和 JNI 供 Android 原生页读取。预设沿用共享设置页的公开数据快照；Java 不持有端点、默认模型或切换规则。目录包含模型建议和接入文档；用户仍可填写自定义地址、模型，以及显式获取服务返回的模型列表。

切换只替换空值或原预设默认值，保留自定义地址与模型，不复制或修改 origin 凭据槽。目标地址为空时关闭 AI 入口，补齐地址后可重新启用。默认选中的 DeepSeek 也允许再次选取以填好首次安装的空值。Ollama 沿用现有移动端限制，不提供为新的选择；导入设置已选择时保留显示。

模型列表请求只在用户点击后执行，沿用共享 `ai::settings::fetch_models` 的端点校验、超时、响应上限和禁重定向；返回时重新读取配置，服务商、地址或凭据变化则丢弃。加载失败保留预设、当前模型与手填入口。启用开关不再挡在配置步骤之前。

## 历史检索

- [AI 服务配置顺序](../../implemented/bug-fix/2026-10-19-ai-service-setup-order.md)与本篇部分重叠：原先只约束共享页，本篇在 Android 按服务商、地址、凭据、模型排列，不翻转其决定。
- [原生 AI Host API](../../implemented/architecture/2026-10-11-windows-settings-ai-host-api.md)与本篇部分重叠：复用模型列表接口，新增纯计算目录和补丁，不重写网络实现。
- [Ollama](../../implemented/feature/2026-10-20-ai-ollama-provider.md)继续约束移动端可选范围。
- 服务商预设模型、AI 偏好模型、凭据契约与图标笔记管理 TypeScript 类型或其他平台展示，不拥有 Android 的实现；保留原决定。

## Alternatives considered

- 在 Java 复制共享页的预设和切换逻辑：改动集中且接入快，但会新增一份宿主业务规则，违背原生设置归属约束，所以共享层持有新增规则。
- 只提供在线模型目录：模型选择总能反映当前账号，但配置前无法选择默认模型，且有些服务不提供标准模型目录；保留预设和自定义作为离线入口。
- 将各平台预设表全部迁移到一份 JSON：可消除现存的 TypeScript、Windows 与 Rust 数据重复，但涉及多个平台既有读取与校验契约；本篇只增加 Android 所需共享能力，后续可独立迁移。

## Consequences

- 服务商选择填入共享默认地址和模型，自定义值与其他 origin 的合成凭据保留。
- 预设模型、当前自定义值和服务模型可选，网络请求返回过时结果时不覆盖当前配置。
- 现存跨平台目录仍需同步维护；后续修改预设时要核对各目录，涉及多个宿主同步调整时重访统一数据源。

## Verification

- `cargo test --locked -p msime-client-providers --test ai_provider_settings`：7 项通过；`cargo test --locked -p msime-host-api ai_provider_`：3 项通过。两个 crate 的 `cargo clippy --all-targets --no-deps -- -D warnings` 均通过。
- 六个 Android edition 的 Java 编译、设备测试包编译、JNI/ELF 导出和原生 full APK 签名校验通过。
- API 28 与 35 各通过 `AiEndpointCheckDeviceSmoke`、`VoiceProviderCatalogDeviceSmoke`、`VoiceProviderSettingsDeviceSmoke`。AI 页经真实导航与控件操作，覆盖首次预设填充、自定义模型保存、origin 凭据保留、回环服务模型选择、服务商或凭据或外部端点变化后丢弃延迟结果，以及 503 失败后保留手填模型。两个批次均正常退出，安装 APK 的 SHA-256 与构建产物一致。
- `bash scripts/verify-local.sh --quick`、`cargo fmt --all -- --check`、`pnpm run verify-notes` 通过。quick 包含 Linux 108 项测试、macOS 原生与 Apple 桥接编译；Windows 完整交叉构建和 HarmonyOS ArkTS 工具链按脚本跳过。

## 验证边界

预设模型是公开目录快照，不保证账号都有权限；界面保留获取模型、文档和手填。既有跨平台目录暂时仍有重复。模拟器使用合成凭据与本地服务，不消耗真实服务额度，也不能代替报告人的实际 AI 皮肤流程验收。
