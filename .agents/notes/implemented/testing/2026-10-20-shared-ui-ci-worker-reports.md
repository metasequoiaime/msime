# Agent Note: 共享 UI 门禁限制并发并保留测试结果

Status: implemented

## Problem

#8868 的共享 UI 作业多次在运行约十三分钟后返回退出码 1，日志只有启动、CSS 与导航的 jsdom 消息，没有具体失败测试或最终汇总。本机完整套件使用受限并发与逐项输出后，定位到 HarmonyOS 两处仍预期左右导航默认关闭的旧断言；修正后该文件 14 项通过。旧 CI 日志无法区分断言失败与进程异常，不能据此确认内存耗尽。

现有配置固定最多 8 个隔离 worker，CI 沿用此值且只用 dot reporter。进程未到最终报告时，已有测试进度也难以追溯。

## Decision

共享 UI CI 明确使用 2 个 worker，完整执行原套件、沿用既有测试与作业超时。verbose reporter 持续输出测试进度，JUnit reporter 将结构化结果写入根目录 `target/shared-ui-junit.xml`，作业无论成败均尝试上传该文件。测试命令的失败退出码继续决定作业失败，报告缺失会在上传步骤提示。桌面本机配置仍保持原有上限。

并发上限是可复验的 CI 资源预算，不将没有证据的退出码归因为 OOM；后续真实断言或进程失败仍需按新日志处理。

## Historical audit

- [插件选择播放开关](../../implemented/bug-fix/2026-10-10-plugin-selection-playback-gates.md)：无关，只有其验证命令使用过单 worker，不持有 CI 并发策略。
- `apps/desktop/vite.config.ts` 已说明无界并发会造成超时；保留本机原有 8 worker 上限，CI 单独收窄。

## Alternatives considered

- **继续原配置重跑。** 可排除一次性的 runner 故障，但重复失败仍没有失败用例或汇总，不能继续把重跑当成诊断。
- **延长作业超时。** 给慢 runner 更多执行时间，但当前约十三分钟的退出并未达到二十分钟作业上限，也无法补齐测试报告。

## Verification

HarmonyOS 候选快捷键测试 14 项通过。完整套件以相同受限并发与报告参数执行：784 个文件、4,838 项测试全部通过，JUnit 的 failures 与 errors 均为 0。类型检查、生产构建、actionlint、笔记校验及远程 quick 通过（运行 20261020-042505，exit 0）。

## Consequences

CI 可以定位具体失败测试并下载结构化结果，代价是并发降低可能延长套件运行时间。完整执行测试并保留退出码，不跳过用例或忽略错误。

进程被强制结束时可能来不及写 JUnit，逐项控制台进度仍提供已经完成的测试名称；尚无内核或 runner 证据能够说明此前异常退出的根因。
