# Agent Note: 笔记三线校验接入 CI

Status: implemented

## Problem

笔记是 AGENTS.md 写死的硬性要求（见[采用笔记](2026-10-07-adopt-agent-notes.md)），但唯一的执行点是可选的本地 pre-push 钩子：没设 `core.hooksPath` 的检出、或 `--no-verify` 的推送，格式损坏或缺失的笔记都能合进 develop。PR #5251 的评审把这一条列为合并前必改——采用笔记里「协作者变多再接 CI」的触发条件已经成立。

## Decision

三项检查（tree / format / archived）作为 `quality.yml` 的新 job `agent-notes` 接入 CI。`quality.yml` 是 workflow_call，被 `ci.yml` 与 `ci-docs.yml` 调用，文档类 PR 同样覆盖。归档封印的 append-only 基准：PR 事件取 `pull_request.base.sha`，push 事件取 `github.event.before`，workflow_dispatch 下两者皆空、脚本降级为磁盘封印自校验。Node 经 `setup-node` 按 `.nvmrc` 固定到 24.21——直接执行 `.ts` 需要默认开类型剥离的版本（22.18+ / 23.6+）。

本地 `scripts/verify-local.sh` 的 agent notes 阶段同批修正：`AGENT_NOTE_ROOT` 钉死到 `$root/.agents/notes`（防错误 CWD 与残留环境变量）、node 类型剥离能力探测（不够则跳过，不拦无关 push）、归档基准取 `git merge-base origin/develop HEAD`（脚本默认拿 HEAD 的 manifest 当基准，等于没查）；`.gitattributes` 给 `.agents/notes/**` 强制 LF，否则 Windows 的 autocrlf 会让归档失败、封印哈希跨机器不一致。

## Alternatives considered

- **维持仅本地 pre-push 检查** — 最省设施，单人仓库本就够用；但 hooksPath 是每台机器的手动配置，`--no-verify` 一律绕过，评审者作为仓库 member 明确要求不受自觉约束的强制点。
- **放进 `contracts.yml`** — 与 run-checks.sh 的契约检查同域，语义也说得通；但那些检查围着编译与平台转，笔记校验是纯仓库卫生，`quality.yml`（actionlint、依赖评审所在处）更贴，且两处都由 CI 调用，覆盖面无差。

## Consequences

- **收益**：笔记约定有了不受本地配置与 `--no-verify` 影响的强制点，本地 pre-push 降级为快速反馈。
- **代价与已知上限**：每次 push 多一个秒级 job；workflow_dispatch 路径下封印基准退化为自校验，与上游脚本设计一致；CI 与本地必须维持「Node ≥22.18 才能直跑 .ts」这一前提，`.nvmrc` 大版本降级时需重访。

## Verification

PR #5251 的 quality 检查出现 `Agent notes` job；本地 `bash scripts/verify-local.sh --quick` 的 agent notes 阶段在类型剥离可用的 node 上跑同一组校验，`AGENT_NOTE_ARCHIVE_BASE_REF` 指向与 `origin/develop` 的分叉点。
