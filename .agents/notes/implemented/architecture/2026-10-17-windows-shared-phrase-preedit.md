# Agent Note: Windows 的部分选词交给共享 Runtime 暂存

Status: implemented

## Problem

#7572 的共享偏好已支持关闭退格回退，但 Windows 的 TIP 与 Server 创建会话时没有启用 `phrase_preedit`。TIP 的旧造词历史、Server 的回复前缀各自持有选词状态，原生设置也没有开关。只加设置控件无法改变退格行为。

## Decision

Windows 的两个真实引擎会话都启用共享暂存。`phrase_prefix` 由 Runtime 拥有，TIP 只绘制前缀及光标，Server 的 ReplyComposer 只投影回复与记录送达。普通退格、分段退格、取消、实时切换复用共享实现，不再为新路径重放 C++ 选词历史。旧无宿主回退保留原路径。

原生输入设置提供现有共享偏好。删除剩余拼写时如 Runtime 提交已选文字，TIP 必须先替换组字文本再结束，Server 只记一次提交，不再拼接一次前缀。仅剩词段时标点仍结束当前组字，不能让斜杠重新打开命令模式。鼠标部分选词沿用已协商的候选继续协议，在各自会话中独立选择并核对前后状态。

## Historical note audit

[退格设置](../../implemented/feature/2026-10-16-backspace-selected-phrase.md) 记录共享规则和 Windows 未接入的缺口。本篇接续其 Windows 范围，不翻转开关默认值与普通退格规则。`CandidateContinuation.h` 已把 `phrase_prefix` 纳入两会话状态核对，可复用而不增协议。其余命中的整句改字、原样上屏与词段幂等笔记是相关独立行为，需要回归。

## Alternatives considered

- **在 TIP 和 Server 的旧选词历史上各接一个开关**：改动局部且无需改变回复投影，但会第三次实现 Runtime 已有的删空、撤回、取消与中途切换规则；鼠标和键盘选择仍容易漂移。统一持有者才能让设置在两会话里一致生效。
- **只在关闭开关时启用共享暂存**：默认用户不受影响，但组字中切换无法把旧历史无损交给 Runtime，违反实时生效的既有契约。

## Verification

- `cargo build --locked -p msime-host-api -j 2` 产出实际宿主库；新用例在接入前失败于部分选择仍产生提交，接入后通过。
- `windows-backspace-selected-phrase` 使用三个合成词条和两份独立 C ABI 会话，覆盖开关两值及组字中双向切换、鼠标继续协议与重复帧拒绝、数字部分选词、多段选词、两次分段删除、仅剩词段时追加斜杠、取消、原样提交和最终候选无重复前缀。
- TIP 的 `phrase_prefix` 解析、仅前缀状态的绘制回调与按键快照另有单测；延迟分类回归直接编译生产方法，176 项检查通过，包含仅前缀状态和退格后的等待分类。
- Windows 可移植原生套件链接同批实际动态宿主库、限制两路编译：167 项通过，2 项主机差异排除，57 项需 Windows 构建，1 项为非测试 fixture。静态库目标回归单独运行也通过；动态链接用于避免在磁盘紧张时为每个测试复制整份 Rust 静态代码。
- MinGW x64 对 7 个改动的生产/集成测试翻译单元运行语法检查通过，包括 TIP 的 `KeyHandler.cpp` 和 `KeyEventSink.cpp`。完整 `verify-local.sh --quick` 已尝试：契约、工作区 Rust 检查通过，Android/wasm 目标和 Windows 完整链接等阶段遭遇 `No space left on device`，没有完成；不能报告门禁通过。
- macOS 主机回归和 MinGW 语法检查不能替代 Windows TSF 编辑器验收，原生 WinUI 设置页也需 Windows CI 编译。

## Consequences

真实 Windows 会话复用共享设置和撤回状态，组字中改开关无需搬运历史。代价是两个进程的展示都要识别“原文为空但前缀非空”的组字；连续退格可能恢复读音，队列必须等实际结果再分类。旧无宿主与旧回复投影仍保留兼容代码，只有真实新会话不再重放历史。Windows 编辑器验收仍待有目标环境时完成。
