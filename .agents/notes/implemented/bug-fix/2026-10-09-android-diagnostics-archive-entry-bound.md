# Agent Note: Android 诊断归档条目数上限

Status: implemented

## Problem

Android 诊断包读取只限制单个 ZIP 条目的展开大小，没有限制归档条目总数。攻击者可以提交包含大量重复条目的归档，让解析循环持续消耗 CPU，并在最终保留最近事件之前积累过多事件对象。

## Decision

`DiagnosticsApi.readBundle()` 将归档条目总数限制为 128，超过后立即抛出 `IOException`。事件解析和多条同名日志的合并过程都增量保留最近 8000 条，避免中间列表无界增长，同时保持上传前的最近事件语义。

## Alternatives considered

- 只在 `latest()` 截断：仍会让重复条目在解析期间持续消耗资源。
- 只限制单个条目大小：无法阻止大量空条目或重复日志条目造成遍历开销。
- 拒绝所有重复文件名：会改变诊断包读取已有的覆盖和合并行为，且不能替代总条目上限。

## Consequences

原生诊断包的固定条目数量远低于上限，正常上传行为不变；异常归档在有限的条目数内失败，事件内存占用保持有界。

## Verification

新增 `DiagnosticsApiSmoke` 回归测试，129 个 ZIP 条目必须被拒绝。`bash platforms/android/check-host.sh` 通过，包含 187 个 Android JVM 冒烟和 55 个设备契约源码编译；`git diff --check` 通过。
