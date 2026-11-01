# Agent Note: 桌面设置宿主与 MCP 崩溃采集

Status: implemented

## Problem

共享崩溃记录核心已经能保存诊断身份，但桌面设置宿主和 `msime-mcp` 仍未安装 native capture，也没有把辅助进程的 fatal crash 绑定到持续授权代次。这样用户看到的崩溃只覆盖输入法宿主，桌面设置与 MCP 的故障无法进入同一条诊断队列。

## Decision

桌面设置宿主在启动、偏好变化和监视器刷新时维护 native capture，并把捕获文件、系统目录和授权代次交给共享 telemetry。`msime-mcp` 在真正执行命令前安装同一套捕获器；`--help`、`--version` 等无需运行时的命令不初始化遥测。两者都使用独立的版本目录和持久队列，发送由隔离 loopback 入口验证，不在 fatal 回调中启动网络。

MCP 的跨进程写入继续使用已有文件锁，但写入冷却时间记录“取得锁并确认状态之后”的开始时刻；拒绝写入不能延长下一次写入窗口。桌面和 MCP 的采集身份必须通过持续授权代次校验，授权撤回或换代后旧捕获不能被认领。

## Verification

Rust 格式检查、MCP 路径和 stdio 测试、桌面宿主测试、loopback 发送和 native capture 回归在 PR CI 中运行。本机因磁盘低于仓库规定的 50 GiB 构建门槛，不启动新的完整 Rust 构建树；本地只运行格式、静态契约和差异检查。

## Alternatives considered

- 只在输入法宿主采集：改动小，但桌面设置和 MCP 的崩溃仍然丢失，因此不采用。
- 在 fatal 回调里直接上传：实现简单，但回调上下文不安全且会把网络失败变成二次崩溃，因此不采用。
- 每次 MCP 写入都从冷却窗口重新计时：能限制频率，但并发拒绝会不断推迟其他请求，且难以稳定测试，因此不采用。

## Consequences

桌面设置和 MCP 的崩溃可与其他宿主按同一身份和授权规则收集，MCP 的跨进程写入窗口也能在 Windows 路径和隔离测试中稳定复现。代价是桌面/MCP 构建必须链接共享 native capture，并增加独立的 loopback 与运行时测试；真实安装后的桌面应用和 MCP 辅助进程验收由 CI/维护者环境完成。
