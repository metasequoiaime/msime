# Agent Note: IBus settled rerank 定时器生命周期

Status: implemented

## Problem

IBus 的 settled rerank 延迟回调把裸 `IBusEngine *` 作为 GLib source data。会话销毁会尝试移除定时器，但 source 本身没有持有 Engine 引用；如果销毁与主循环派发交错，回调可能在对象释放后解引用悬空指针。

## Decision

让 settled rerank source 像翻译和在线候选延迟 source 一样调用 `g_object_ref(engine)`，并在 destroy notify 中调用 `g_object_unref`。移除或执行 source 后引用都会成对释放，回调读取状态时 Engine 仍然有效。

## Alternatives considered

- 只在回调里检查 `state`：裸 `IBusEngine *` 已经可能悬空，无法安全读取状态。
- 只依赖 `State::close()` 移除 source：这不能让 source 在对象释放前拥有稳定的对象引用，且与同文件其它延迟回调的生命周期契约不一致。

## Verification

`python3 scripts/test-linux-ibus-rerank-lifecycle.py` 在修复前按预期失败，修复后通过；另运行 `git diff --check`。`bash platforms/linux/build-container.sh` 完成 IBus/Fcitx5 与 full/wubi 构建，79/79 个 Linux 测试通过。

## Consequences

settled rerank 延迟期间 Engine 的生命周期由 GLib source 保持；输入会话关闭时 source 仍会被移除并释放引用，正常输入和重排行为不变。
