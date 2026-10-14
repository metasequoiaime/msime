# Agent Note: 单条在线候选的来源边界校验

Status: implemented

## Problem

`msime_client_apply_online_candidate` 的 C ABI 入口没有像批量入口一样校验 `source`。调用方传入 0（云候选）或 1（AI 候选）之外的值时，入口仍返回成功响应，只把运行时的 `applied:false` 当作正常的过期或不适用结果，隐藏了宿主的契约错误。

## Root cause

单条入口在解析查询和候选文本后直接调用 `Runtime::apply_online_candidate_owned`；来源范围校验只存在于运行时的防御性检查和批量 C ABI 入口，入口层漏掉了统一的参数错误返回。

## Decision

单条入口在读取查询前拒绝大于 1 的来源值，并返回 `invalid online candidate source`。合法的 0 和 1 继续走原有的会话、查询新鲜度和候选内容校验；运行时保留自己的来源检查，作为跨层防线。

## Alternatives considered

- **继续把未知来源交给运行时并返回 `applied:false`**：不会改变当前列表，但会把宿主编程错误伪装成合法的过期结果，与批量入口的参数错误行为不一致。
- **只在运行时保留检查**：能阻止状态变化，却不能让 C ABI 调用方知道参数不符合接口契约，也无法和批量入口保持相同的错误边界。

## Consequences

未知来源不会再进入查询解析和会话处理路径，宿主可以区分参数错误与合法的未应用结果。合法在线候选和运行时防御检查的行为保持不变。

## Verification

新增 `single_online_candidate_rejects_unknown_source` 回归测试；修复前测试因收到 `ok:true`、`applied:false` 失败，修复后通过。另运行 `cargo test -p msime-host-api --lib`、`git diff --check`、`pnpm run verify-notes` 和仓库 quick 门禁。
