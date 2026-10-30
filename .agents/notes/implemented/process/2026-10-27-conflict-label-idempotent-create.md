# Agent Note: 并发 PR 检查重复创建冲突标签时保持成功

Status: implemented

## Problem

`conflict-label.yml` 在每个 PR 更新时运行。两个检查可以同时查询到 `conflict` 标签不存在，随后其中一个创建成功，另一个在 `gh label create` 收到“标签已存在”而失败。2026-10-27 的 #9985 检查出现了这一种先查后建竞态，标签已存在且 PR 无合并冲突，仍显示 CI 失败。

## Decision

准备标签时直接执行 `gh label create --force`，把已存在与并发创建后的状态视为同一个成功结果。颜色和说明沿用工作流定义；不再依赖标签列表的查询时刻，也不受列表页数限制。

## Alternatives considered

- **创建失败后再查标签是否存在**：能避免对已存在标签的重复更新，但仍需处理查询失败、API 状态延迟和额外请求。这里的标签样式由该工作流定义，重复写入同一值可以接受，因此采用一次幂等命令。

## Consequences

同一标签被多个 PR 作业同时准备时不会因已存在而失败。每次运行会把 `conflict` 标签的颜色和说明设为工作流指定值；维护者若要改变其样式，应同时更新工作流。

## Verification

`gh label create --help` 确认 `--force` 在标签已存在时更新颜色和说明；`actionlint` 和 `pnpm run verify-notes` 检查工作流与笔记。PR CI 再验证真实 GitHub API 调用。
