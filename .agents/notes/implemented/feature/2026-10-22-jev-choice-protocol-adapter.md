# Agent Note: Jev TypeSafe Choice API 接入协议适配层

Status: implemented

## Problem

Windows v0.11.1 的 AI token 槽含 `jev`（parity-gaps B2），它是 TypeSafe 的 Choice API 而不是 OpenAI 兼容服务：请求是 `state`（前文 + 拼音分段）加 `questions.candidate.criteria`（`c0`..`c31` 本地候选 + `none` 弃权项），回答取 `answers.candidate.choice` 的槽位号映射回候选。目录补齐时判定它「不进普通目录」（[服务商目录补齐](2026-10-22-provider-catalog-expand.md)），但协议差异本身可控，适配层可以先行落地。

## Decision

判定**接，但只接到协议适配层**。新模块 `crates/client-providers/src/ai/jev.rs` 复刻 Windows `server/src/ai/jev_client.cpp` 的三个职责：

- `build_request`：逐字构造 `systemone` 请求体（`model`/`state`/`questions.candidate.{type,instructions,criteria}`，`criteria` 上限 32 加 `none` 键）。
- `parse_choice`：取 `answers.candidate.choice`，`c` 前缀槽位号映射回候选；弃权、未知槽位、非 `choice` 类型、损坏 JSON 一律 `None`，不会变成输入文本。
- `jev_http_request`：产出与 `chat_completion_http_request` 完全同形状的 HTTP 描述符（`url`/`method`/`headers`/`body`/`timeout_ms`/`connect_timeout_ms`/`max_response_bytes`），宿主 transport 原样复用；凭据走 `credential_for_endpoint` 的 origin 解析，`legacy_provider_origin` 补 `jev` arm 让 Windows `token_jev` 对应的 `tokens["jev"]` 槽可解析。

与 Windows `Fetch` 提前返回对齐的门禁：禁用、非拼音模式、候选不足两个返回 `Ok(None)`（一个候选无可选择，前文不值得离机）；端点不等于官方 `https://api.typesafe.ai/v1/systemone` 返回 `Err(InvalidConfiguration)`——凭据只发给 api.typesafe.ai，Windows 的 `endpoint != kEndpoint` 拒发行为在新架构表达为配置错误。超时按 Windows 预算：连接 1500ms、总 2500ms、响应 1 MB。

**不进 `AI_PROVIDERS` 与 provider 目录**：目录项会让设置页能选它，而 `chat_completion_http_request` 只会发 Chat Completions 形状——可选但永远发不出请求，比不可选更坏。`jev` 进入白名单的时机是端到端链路打通时。

**端到端还缺的层**（本笔记的重开条件）：`OnlineQuery` 不携带本地候选池，`apply_online_candidates` 把 AI 回答追加成新行而不是提升已有行——Jev 需要「候选池进 + 提升出」两个新语义，跨线格式（`OnlineQuery`）、各宿主 transport worker 和应用路径，且〔Jev〕徽标与提升到第三位属于产品决策，不在本层范围。

## Alternatives considered

- **jev 按普通目录项接入**：最小改动，只需目录项加白名单；但 `chat_completion_http_request` 发出的 Chat Completions 请求体 TypeSafe 必然拒绝，服务商可选却永远无回答。与目录补齐时否决它的理由相同。
- **在 `chat_completion_http_request` 内按 provider 分发到 Choice 形状**：改动集中、目录可照旧收 jev；但 `AiSuggestionRequest` 不携带候选列表，分发分支只能发空 `criteria`——请求形状对了、语义仍然发不出有效回答，否决。
- **本轮直接端到端接入**：一次到位但范围失控——要动 `OnlineQuery` 线格式、全部宿主的 worker 取池方式、应用路径的提升语义和设置页，把协议适配（纯函数）与产品决策（提升、徽标）混在一个 PR 里，审查与回滚都分不开，否决。

## Verification

先失败再实现：`tests/ai_jev.rs` 六个用例对空 stub 全部断言失败（`build_request` 返回 `Null`、`parse_choice` 返回 `None`、`jev_http_request` 返回 `Ok(None)`），实现后 6/6 通过；覆盖请求体 `state`+`criteria` 形状、32 上限、choice 映射、弃权/未知/损坏回答、描述符字段与超时、禁用/错端点/薄候选池/英文模式/无凭据门禁、`tokens["jev"]` 槽解析。`cargo test -p msime-client-providers` 251 全过、`-p msime-client-core` 493 全过；`cargo check -p msime-host-api -p msime-input-runtime -p msime-mcp-server --all-targets` 通过；`bash scripts/run-checks.sh` exit 0；改动文件 `cargo fmt` 通过（`--all --check` 另有一处基线 `crates/engine/src/session/tests.rs` 的既存差异，与本改动无关）。全部合成 fixture，不打真实网络。

## Consequences

- 协议层真实可用且行为与 Windows 逐字对齐；端到端接入只剩「给查询喂候选池、应用路径提升选中行」两层接线，每步都可独立评审。
- 新增代码暂无调用方（适配层先行），行为由 `tests/ai_jev.rs` 钉住；未接线的公开 API 不产生运行时效果，也不改变任何既有 provider 的路径。
- `legacy_provider_origin` 新增 `jev` arm 只在 `provider == "jev"` 时命中，其他 provider 的凭据解析不变。
