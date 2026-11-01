# Agent Note: 桌面设置宿主与 MCP 崩溃采集

Status: implemented

## Problem

共享崩溃记录核心已经能保存诊断身份，但桌面设置宿主和 `msime-mcp` 仍未安装 native capture，也没有把辅助进程的 fatal crash 绑定到持续授权代次。这样用户看到的崩溃只覆盖输入法宿主，桌面设置与 MCP 的故障无法进入同一条诊断队列。

## Decision

桌面设置宿主在启动、偏好变化和监视器刷新时维护 native capture，并把捕获文件、系统目录和授权代次交给共享 telemetry。`msime-mcp` 在真正执行命令前安装同一套捕获器；`--help`、`--version` 等无需运行时的命令不初始化遥测。两者都使用独立的版本目录和持久队列，发送由隔离 loopback 入口验证，不在 fatal 回调中启动网络。

Linux 设置应用沿用已初始化宿主的安装 id：IBus 根目录已有遥测状态时用根目录；只有 Fcitx5 子目录有状态时改用 Fcitx5 目录，不在根目录另建身份，避免只用 Fcitx5 的机器被计为两次安装和两份日活；两者都没有时仍落在根目录。改用 Fcitx5 目录后不再对同一目录做第二次代收。

MCP 的跨进程写入冷却行为不变：开始时间原本就在取得文件锁、读取共享状态之后确定，拒绝也不会延长窗口。本 PR 只把时钟改为可注入，用固定时钟的真实文件单测覆盖这两条，并在 handler 层补一条进程内限流拒绝的测试。桌面和 MCP 的采集身份必须通过持续授权代次校验，授权撤回或换代后旧捕获不能被认领。

## Verification

PR CI 中：`ci-platforms.yml` 的 Linux settings app 作业跑 `msime-desktop` 与 `msime-mcp-server` 的 clippy 和全部测试（含 `capture_consent_lifecycle`、`crash_capture_lifecycle`、stdio 集成测试和写入窗口测试）；Windows native settings 作业 `cargo check -p msime-desktop --all-targets` 并跑 `msime-mcp-server` 的测试，覆盖 MSVC 编译 `CrashRecorder.cpp` 与 Windows 路径。macOS 没有 PR 作业编译这两个 crate，`msime-desktop` 与 `msime-mcp-server` 的 macOS 编译由维护者用 Mac Studio 的 `cargo check` 验证。`tools/crash/verify_telemetry_loopback.py` 不在 PR CI 中运行。真实安装后的桌面设置与 MCP 辅助进程致命崩溃采集尚未在设备上验收。

## Alternatives considered

- 只在输入法宿主采集：改动小，但桌面设置和 MCP 的崩溃仍然丢失，因此不采用。
- 在 fatal 回调里直接上传：实现简单，但回调上下文不安全且会把网络失败变成二次崩溃，因此不采用。
- 每次被拒绝的 MCP 写入也重新计时：能限制频率，但并发拒绝会不断推迟其他请求，因此保持原有的不延长规则，只补固定时钟测试。
- Linux 设置应用始终使用 IBus 根目录：实现简单，但只用 Fcitx5 的机器会多出一个安装 id 和每日一次的 `active`，因此改为沿用已初始化宿主的目录。

## Consequences

桌面设置和 MCP 的崩溃可与其他宿主按同一身份和授权规则收集，MCP 的跨进程写入窗口由固定时钟测试稳定覆盖。代价是桌面/MCP 构建必须链接共享 native capture，macOS 还要经 `msime-host-macos` 的 Foundation 查询解析 Application Support；真实安装后的桌面应用和 MCP 辅助进程验收仍待维护者环境完成。
