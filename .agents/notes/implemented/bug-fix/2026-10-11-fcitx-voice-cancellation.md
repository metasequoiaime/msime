# Agent Note: Fcitx5 语音 worker 取消传播

Status: implemented

## Problem

Fcitx5 的语音请求运行在插件级 detached worker 中。关闭会话时宿主会请求 provider 取消，但 Unix socket stream 之前没有收到本地取消信号；provider 最长可能继续等待约 730 秒，插件卸载前的安全等待因此会被无界拖住。

## Decision

在 Fcitx 语音 mailbox 中加入原子取消标志，并在关闭或取消时先置位。Host API 增加保留旧 ABI 行为的可取消 stream 入口，把 C 回调转成 input-runtime 的取消探针；socket 读取每次 100ms 轮询时检查该探针。旧的 IBus 及其它调用继续使用原入口，Fcitx 仍由 detached job tracker 等待 worker 完成后再卸载插件。

## Alternatives considered

- 给插件卸载等待增加固定超时：超时后仍可能 `dlclose`，后台线程会执行失效的插件代码。
- 只依赖 provider 的 `voice_cancel` socket 请求：provider 不响应或响应慢时，stream 读取仍可占满完整 deadline。
- 在 Fcitx 主线程等待 future：会把 provider 延迟带回输入事件循环。

## Verification

`python3 scripts/test-linux-fcitx-detached-job-lifecycle.py` 通过。`cargo test -p msime-input-runtime` 与 `cargo test -p msime-host-api --lib` 通过；新增的 callback 取消测试覆盖连接前和等待读取中的取消。Linux 原生容器门禁通过（79/79 tests）。`bash scripts/verify-local.sh --quick` 的 Android Java 阶段仍因基线中的 `ViewPolicy.java` 缺少 `ThemeColorPolicy` 符号失败，其余阶段完成。

## Consequences

Fcitx 取消和关闭会在下一次 provider 读取轮询内结束语音 worker，插件卸载仍等待 worker 完整退出，因此保留动态库安全不变量且不阻塞输入事件循环。
