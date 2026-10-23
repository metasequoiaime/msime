# Agent Note: 服务商目录补齐到 Windows v0.11.1

Status: implemented

## Problem

Windows v0.11.1 的润色 token 槽 11 家、ASR token 槽含 zhipu/minimax/custom、AI token 槽含 minimax/volcengine/jev（parity-gaps B2）。新架构润色目录只有 4 家、ASR 目录 7 家、AI 目录 13 家，token map 结构本身支持任意 provider id，缺的是目录项、偏好白名单与凭据测试路由。

## Decision

目录事实源 `crates/client-providers/src/{voice,ai}/provider_catalog.rs` 各补全到与 Windows 相同的可选服务集合：

- ASR 追加 zhipu（`open.bigmodel.cn/api/paas/v4/audio/transcriptions`，`glm-asr`）、minimax（`api.minimax.cn/v1/speech_to_text`，`asr-1.0`）、custom（空地址/模型，用户自填）；三家在 `asr_request_format` 均按 `multipart` 上传，与 Windows 对所有非豆包服务走同一 `make_transcription_request` 一致。智谱（docs.bigmodel.cn `glm-asr` 文档）与 MiniMax（platform.minimax.cn `speech-to-text` 文档）官方文档确认同为 `file`+`model` 的 OpenAI 兼容表单。
- 润色追加 minimax、kimi、zhipu、qwen、volcengine、openrouter、custom，端点逐字取 Windows `DefaultPolishEndpoint`/`DefaultPolishModel`。
- AI 追加 minimax、volcengine，端点取 Windows `ime_config.h` endpoints 表；`legacy_provider_origin` 同步两家 origin 供按 origin 存取的凭据回退。
- `AI_PROVIDERS`/`ASR_PROVIDERS`/`POLISH_PROVIDERS` 白名单同步扩容（偏好 `load()` 会跑 `validate()`，不加会整份文档拒载）；`shared/voice/VoiceProviders.cpp` 的手写映射同步；导出器再生成 UI JSON、C++ 头、iOS Swift、鸿蒙 ETS、Linux Python 产物。
- minimax 默认模型用官方文档现行名 `MiniMax-M3.1-Flash`（Windows 写的 `MiniMax-M3.1-Flash-Preview` 是预览期名字，列入建议之外的用户自填范围）。

**id 收敛不双列**：dashscope/moonshot 沿用 AI 目录已定的改名，在润色目录写作 qwen/kimi，不把同一服务用两个 id 重复列出（会让 token 槽分裂）。`polish_provider_defaults` 为两个旧 id 提供只解析不进目录的回退，与 everyapi/mistral 的既有兼容方式相同。

**jev 不接**：它是 TypeSafe Choice API（`api.typesafe.ai/v1/systemone`），请求体是 `state`+`questions.candidate.criteria` 的候选选择协议、回答取 `answers.candidate.choice`，与全目录统一的 OpenAI Chat Completions 形状不兼容；Windows 还把它硬编码在固定端点、200ms 轮询延迟上跑。接入需要新的请求/解析协议与候选列表入参，不属于目录补齐范围；端点与模型已记录在此备查。

## Existing notes

B2 出处 `_refs/parity-gaps.md`；共享目录机制见 #9231/#9239 的提交（`export_*_provider_catalog` 导出器与各平台投影）。

## Alternatives considered

- 润色目录沿用 Windows 的 dashscope/moonshot id：会与 AI 目录同一服务两个名字，token 槽分裂；否决。
- jev 按普通目录项接入：请求协议不匹配，放进去的服务商永远发不出请求；否决。

## Verification

新目录项先以 stash 摘掉实现复现 4 项失败（目录断言、端点断言、ASR 凭据路由），恢复后 `cargo test -p msime-client-providers` 249 全过、`-p msime-client-core` 485 全过；`asr_request_format` 三家断言 multipart；两个导出器 `--check` 通过；Linux `provider_catalog.py`/`provider_request_format.py` pin 更新后通过；`cargo fmt --all --check` 通过。目录合入后 Windows UI 冒烟仍保留旧的 13 项断言，已同步为 15 项并允许新增服务暂不带品牌图标。

移动桥的 ASR 请求矩阵同步覆盖三家新增的 `multipart` 服务商，并保留 `system`、`whisper`、未知标识和空标识的拒绝断言；回归通过 `MobileVoiceTranscriptionRequest::is_valid` 检查实际委托共享 `asr_request_format` 的请求校验，不在生产桥接层新增白名单。

## Consequences

设置页识别/润色/AI 三个下拉按目录顺序直接出现新服务商，token 槽沿用 per-provider map 自然成立。jev 与 thinking 强度（A7）仍是独立后续项。
