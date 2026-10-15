# Agent Note: Fcitx5 语音 worker 取消传播

Status: implemented

## Problem

Fcitx5 的语音请求运行在插件级 detached worker 中。关闭会话时宿主会请求 provider 取消，但 Unix socket stream 之前没有收到本地取消信号；provider 最长可能继续等待约 730 秒，插件卸载前的安全等待因此会被无界拖住。

## Decision

在 Fcitx 语音 mailbox 中加入原子取消标志，并在关闭或取消时先置位。Host API 增加保留旧 ABI 行为的可取消 stream 入口，把 C 回调转成 input-runtime 的取消探针；socket 读取每次 100ms 轮询时检查该探针。旧的 IBus 及其它调用继续使用原入口，Fcitx 仍由 detached job tracker 等待 worker 完成后再卸载插件。

共享语音行读取遇到 `ErrorKind::Interrupted` 时继续原来的轮询，保留未完成的 UTF-8/JSON 字节，重新检查取消与同一个总截止时间，不把系统调用中断当成语音失败。Unix socket 入口负责设置每次读取的超时；配置失败时原先立即结束，现在由[对端关闭后的补读](2026-10-15-voice-buffered-response-after-close.md)接管，保留非阻塞与取消边界。行读取循环接收一次读取的函数，便于独立核对分片和中断序列；对外接口、消息格式和 100ms 取消预算保持原样。错误分类依据 [Rust 标准库说明](https://doc.rust-lang.org/std/io/enum.ErrorKind.html#variant.Interrupted)。

## Alternatives considered

- 给插件卸载等待增加固定超时：超时后仍可能 `dlclose`，后台线程会执行失效的插件代码。
- 只依赖 provider 的 `voice_cancel` socket 请求：provider 不响应或响应慢时，stream 读取仍可占满完整 deadline。
- 在 Fcitx 主线程等待 future：会把 provider 延迟带回输入事件循环。
- 用真实线程信号制造 `EINTR`，能覆盖内核与 socket 的完整路径；但进程级信号处理器和投递时机使并行测试难以隔离，平台差异也较大。通过读取函数注入确定的中断和 UTF-8 分片，保留现有真实 Unix socket 测试核对适配器。

## Verification

最初取消传播改动的历史验证：`python3 scripts/test-linux-fcitx-detached-job-lifecycle.py` 通过。`cargo test -p msime-input-runtime` 与 `cargo test -p msime-host-api --lib` 通过；新增的 callback 取消测试覆盖连接前和等待读取中的取消。Linux 原生容器门禁通过（79/79 tests）。`bash scripts/verify-local.sh --quick` 的 Android Java 阶段当时因基线中的 `ViewPolicy.java` 缺少 `ThemeColorPolicy` 符号失败，其余阶段完成。

2026-10-14 中断重试修复：保留旧错误分类时，首字节前中断、UTF-8 分片中断和取消重检三项回归失败，两项既有失败语义检查通过。修复后六项回归通过，完整 `cargo test -p msime-input-runtime` 266 项通过；`cargo clippy -p msime-input-runtime --all-targets -- -D warnings` 和 `cargo fmt --all --check` 通过；`pnpm run verify-notes`、`git diff --check` 和最终源码的 `bash scripts/verify-local.sh --quick` 通过。本机缺少 Docker、MinGW 等工具的阶段按脚本跳过，本轮未运行 Linux 容器或原生设备验收。

## Consequences

Fcitx 取消和关闭会在下一次 provider 读取轮询内结束语音 worker，插件卸载仍等待 worker 完整退出，因此保留动态库安全不变量且不阻塞输入事件循环。
