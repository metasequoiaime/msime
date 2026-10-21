# Agent Note: AI 服务商新增不鉴权的 Ollama

Status: implemented

## Problem

本机跑 Ollama 的用户只能在 AI 辅助里选「自定义」，自己填 `http://localhost:11434/v1/chat/completions`，还得往 API Token 里随便塞一串字符：Rust 的请求构造、设置页、Windows 原生设置页、配置测试和 Linux provider 都把空 Token 当成配置不完整。Ollama 本身不做鉴权，这个必填项对它没有意义，只会让人以为要去哪里申请密钥。

## Decision

- `ollama` 是 `AI_PROVIDERS`（`crates/client-core/src/preferences.rs`）里的一个服务商，预设地址 `http://localhost:11434/v1/chat/completions`，没有默认模型：模型是用户自己拉到本机的，靠「获取模型」读 `/v1/models`。
- 是否用 Token 只看服务商：`ai::provider_uses_token`（Rust）、`aiProviderUsesToken`（`packages/ui/src/settings/ai-provider-options.ts`）、`ais::provider_uses_token`（`platforms/windows/settings/AiSettings.h`）三处同一条规则，只有 `ollama` 为假。
- 选 Ollama 时设置页整块不显示 Token（共享页的 `credentialSection`、Windows 原生页的 API Token 一行、Linux 的凭据区），请求 never 读这个地址下以前存过的 Token，也 never 带 `Authorization`：
  - 候选与翻译描述符由 `ai::request_headers` 构造，Ollama 只有 `Content-Type`；macOS 的 AI 翻译描述符检查因此接受只有一个头的形状，带了 `Authorization` 时仍必须是 `Bearer`。Windows 宿主逐个转发描述符里的头，不用改。
  - 「获取模型」和「测试」（`ai::settings`）以及「测试 AI 辅助配置」（`credential/probe.rs`）把空 Token 当成不鉴权，不带 Bearer；其他服务要不要 Token 由设置页把关。
  - `host-api` 判断 AI 翻译是否可用时改问 `ai::has_request_credential`。
  - Linux provider 遇到 `ollama` 不查私有的 `ai-provider.json`，直接用页面传来的地址和模型，按 `load_ai_config` 同样的规则校验，`fetch` 的 Token 为 `None` 时不带鉴权头。
- 地址规则不放宽：http 仍只能指向本机或局域网（[AI 辅助接口允许本机和局域网的 http 地址](2026-10-08-ai-endpoint-local-http.md)），公网的 http 地址选 Ollama 也拒绝。
- 只在桌面提供：`aiProviderOptionsFor` 在 iOS、Android 和 HarmonyOS 上去掉 Ollama（已经选着时保留，免得下拉显示不出当前值）。手机键盘各自还有一份 Token 判断，接入要三端一起改。
- 图标照 [AI 服务商图标](2026-10-19-ai-provider-icons.md) 的来源，取同一 Lobe Icons 提交里的 `ollama.svg`。

## Alternatives considered

- **只加预设，Token 照样必填** —— 改动最小，所有判断不动。但用户选了 Ollama 还要填一串假 Token，界面在暗示需要申请密钥，和「自定义」没有区别，等于没做。
- **按地址判断：本机和局域网的地址都免 Token** —— 一条规则同时覆盖 LM Studio 和自建服务。但局域网里也有要鉴权的网关（one-api、带密钥的 LM Studio），把它们的 Token 输入框一起藏掉会让这些配置没法填；而且 Token 输入框随地址出现消失，比随服务商变化更难预料。按服务商判断只影响主动选了 Ollama 的人。
- **选 Ollama 时 Token 改成选填，输入框仍然显示** —— 给 Ollama 前面套了鉴权反代的人留了口子。但这类用户可以选「自定义」；对绝大多数 Ollama 用户，一个可以不填的密钥框仍然在问他们要不要填，正是要去掉的困惑。

## Consequences

- **收益**：Windows、macOS、Linux 上选 Ollama 填好模型就能用，不再需要假 Token；设置页、Rust 请求构造和 Linux provider 对「哪个服务不鉴权」用的是同一条规则。
- **代价**：同一条规则在 Rust、TypeScript、Windows C++ 和 Linux Python 里各有一份，靠各自的测试保持一致；在 Ollama 前面套了鉴权的用户要改选「自定义」。Linux 上 Ollama 的地址和模型来自设置页传给 provider 的查询，不再经过私有配置文件绑定——没有凭据可泄露，地址仍按本机、局域网规则检查。
- **验证**：`cargo test -p msime-client-providers`（`ollama_requests_never_read_or_send_a_token`、`ollama_chat_test_sends_no_token`），`apps/desktop` 的 `ai-provider-option`、`use-ai-assistant`、`ai-credential-test-config` 测试，Linux 的 `platforms/linux/tests/provider/ai_service_contract.py`，macOS 的 `CloudCandidateRequestTest`，Windows 的 `tests/ui/ai_settings.cpp`。没有在真机上连过 Ollama。
