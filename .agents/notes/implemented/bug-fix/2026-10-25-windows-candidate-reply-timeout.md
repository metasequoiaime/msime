# Agent Note: Windows 候选回包等待预算覆盖现场延迟

Status: implemented

## Problem

Windows TSF 在长输入后选词时，必须回包的请求固定只等待 50ms。#9518 的现场日志记录到 53–61ms 的候选回包；这类本来已经送达的回包被误判为传输故障，随后关闭管道并触发本地组字重置。恢复路径会把当前整串宿主组字摊回延迟队列，于是候选窗先消失、重连后重现，再次处理后才上屏。

## Decision

把必须回包路径的等待预算集中为 `ReplyWaitPolicy.h` 中的 `kServerReplyWaitTimeoutMs`，从 50ms 提高到 100ms。100ms 覆盖现场最高 61ms 的两倍余量，同时仍把单次同步等待限制在可感知延迟以内；软回包调用继续由 `abortTransportOnTimeout=false` 使用原有行为，不因缺少可选回包而重连。

## Alternatives considered

- 保持 50ms，只调整本地重置和恢复队列：只能减少重连后的丢键，仍会把正常的 53–61ms 回包当成断线，候选窗仍会闪退。
- 对所有请求改成软超时：候选选择和提交无法确认 Server 的权威结果，可能把空回包或旧候选误当成成功，破坏 TSF 与 Server 的一致性。
- 继续无限等待：会让 Server 或管道真的失效时永久卡住输入，违反有限 I/O 预算。

## Verification

新增 `msime-tsf-reply-wait-policy` 回归，要求预算至少覆盖现场 61ms 的两倍；改动前该测试按预期失败，改为 100ms 后通过。相关 Windows 延迟分类、恢复和应用键回放夹具也保持通过；尚待 Windows TIP、Server 和真实编辑器中的长输入选词验收。

## Consequences

候选选择、必须确认的提交和其他硬回包路径最多多等待 50ms，现场级的 53–61ms 延迟不再触发管道重置。真正超过 100ms 或读写失败仍走原有有限超时、重连和恢复流程；没有改变恢复队列、候选排序或协议格式。现场 53–61ms 里约 30ms 是 Server 的候选渲染屏障在空等候选窗轮询，[渲染屏障叫醒候选窗](2026-10-25-windows-candidate-render-wake.md) 去掉了这份等待，预算余量重新留给引擎处理。
