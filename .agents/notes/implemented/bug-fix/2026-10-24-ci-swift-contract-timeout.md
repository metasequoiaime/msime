# Agent Note: 放宽 Swift 跨语言合同检查的编译超时

Status: implemented

## Problem

`scripts/test-polish-prompt-catalog.py` 在 Ubuntu 合同任务中用 `swiftc` 编译只输出内置方案 id 和名称的标准库探针。原来的 60 秒限制已经贴近 runner 的正常耗时：一次失败运行在 60 秒处超时，随后同一提交重跑通过；成功运行记录的编译耗时为 58.5 秒。这个时间预算让 runner 的短暂负载把有效的目录一致性检查变成偶发红灯。

## Decision

Swift 探针的编译超时设为 120 秒，并把该值用于启动日志，继续保留 `-v` 详细编译输出、单调时钟耗时记录、10 秒探针执行限制和 `check=True` 的真实编译失败语义。该调整只改变 CI 检查的等待预算，不改变检查的输入、输出或比较规则。它是对[共享润色目录迁移笔记](../simplification/2026-10-21-shared-polish-prompt-catalog.md)中原有 60 秒决定的后续修正。

## Alternatives considered

- 保持 60 秒并依赖重跑：失败后可以立即恢复，但会继续阻塞合并，并把 runner 负载误报成代码问题。
- 只在超时后重试一次：可以降低偶发红灯，但会掩盖持续变慢，且一次任务的最大等待时间和日志语义更复杂。
- 移除 Swift 编译检查或改为静态文本比较：能避免编译耗时，但失去真实 Swift 消费目录的编译验证，不符合跨语言合同的目的。

## Consequences

合同任务在慢 runner 上最多多等待 60 秒，换取 Swift 目录检查不再因正常冷编译抖动而失败；真正超过 120 秒或编译失败仍会明确失败。该修复与原有润色目录迁移笔记互链，后续若 Swift 工具链再次显著变慢，应依据日志重新评估预算，而不是无限放大超时。

## Verification

- 新提交 `4f5a7104` 的 `Core CI` 合同任务在原 60 秒限制下成功，但日志记录 Swift 编译用时 58.514 秒，证实余量不足。
- 修改后本地运行 `python3 scripts/test-polish-prompt-catalog.py`，验证共享 JSON、Linux Python、Windows C++、Swift 和 Android Java 目录检查通过或按平台能力跳过。
- 运行 `pnpm run verify-notes`，确认笔记树、格式和归档校验通过。
