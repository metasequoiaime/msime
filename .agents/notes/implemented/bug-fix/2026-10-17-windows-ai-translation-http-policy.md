# Agent Note: Windows AI 候选翻译复用本地接口策略

Status: implemented

## Problem

Windows 的 AI 候选请求已经按 `AiEndpointPolicy` 允许本机和局域网的 `http://`，但同一份 AI 配置进入候选翻译时却调用只允许 HTTPS 的通用翻译传输门禁。用户把 LM Studio、Ollama 或局域网模型配置成 HTTP 后，AI 候选可用，候选释义却静默失败；该路径还误用了 NiuTrans 的 2.5 秒超时，而共享 AI 描述器和 AI 候选请求的预算是 8 秒。

## Decision

Windows 候选翻译为 AI 描述器使用独立的 `HttpEndpointPolicy::Ai`：发请求前复用 `AiEndpointPolicy` 的 HTTPS、本机和局域网 HTTP 判断，局域网 HTTP 设置 `CURLOPT_NOPROXY "*"`，仍不跟随重定向。腾讯云、小牛、普通自定义服务和账号请求继续使用各自原有的 HTTPS 或回环 HTTP 边界。AI 候选翻译使用与共享描述器一致的 8 秒请求预算。

## Alternatives considered

- **继续强制 HTTPS**：边界简单，也符合部分旧翻译传输的历史规则；但它与同一 AI 配置的候选请求和 macOS 的 AI 翻译路径不一致，会让本地模型只缺候选释义功能，因此不采用。
- **把所有翻译服务都改为 AI 的局域网 HTTP 规则**：可以统一传输代码，但会扩大腾讯云、小牛和自定义翻译的明文范围，突破它们现有的共享契约；本次只收窄到 AI 描述器。
- **让 AI 翻译继续沿用小牛的 2.5 秒预算**：修改最少，但大模型首 token 和 JSON 回复可能超过该预算，和描述器的 `timeout_ms: 8000` 及 AI 候选行为不一致，因此不采用。

## Consequences

Windows 用户配置本机或局域网 AI 服务后，候选翻译与 AI 候选使用同一地址策略和直连规则；公网 HTTP 仍被拒绝，HTTPS 行为不变。AI 翻译最多等待 8 秒，翻译线程在模型较慢时会占用更久，但不会改变取消、无译文退避和响应上限。

## Verification

`windows-translation-worker` 测试新增本机 HTTP、私有网段 HTTP 和公网 HTTP 的 AI 地址边界用例。由于当前 macOS 主机没有 MinGW 工具链，无法在本机编译 Windows C++ 目标；`git diff --check` 和 Rust/UI 基线门禁不受本次 Windows-only 改动影响，Windows 交叉构建与 Wine 测试需由 CI 或 Windows 环境完成。
