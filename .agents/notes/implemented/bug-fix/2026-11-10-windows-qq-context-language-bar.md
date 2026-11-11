# Agent Note: QQ 异步 context 压入后恢复 TSF 语言栏

Status: implemented

## Problem

QQ 9.9.37 的输入框可能先通知 TSF 文档获得焦点，再异步把第一个 `ITfContext` 压入文档。`OnSetFocus` 在没有 context 时会把水杉语言栏按钮标成禁用；随后 `OnPushContext` 原先用两个 COM 接口返回的裸指针比较文档归属，可能把同一个文档的不同接口包装误判为不相等并提前返回，因此按钮一直禁用。任务栏仍显示“双”，但右键水杉菜单无法打开，输入框也只能直出英文。

## Decision

`_HandleFocusedContextStackChange` 用 `_IsSameComObject` 比较 `GetDocumentMgr` 和 `GetFocus` 得到的文档，也用 COM identity 比较顶层 context。只要 context 栈变化属于当前焦点文档，就在处理栈变化时重新调用 `_UpdateLanguageBarOnSetFocus`；顶层 context 发生变化时再继续重绑 `ITfTextEditSink` 和重置本地会话。这样焦点通知早于 context 压入时，后续压入事件会恢复按钮状态，同时保留后台文档变化不得抢占当前语言栏的边界。

`ContextStackChangePolicy.h` 把“忽略、只刷新语言栏、刷新并重绑编辑 sink”三种结果写成纯策略，回归测试覆盖 QQ 的先焦点后压入、不同 COM 包装、后台文档和顶层未变化四种时序。

## Alternatives considered

- **只在 `OnSetFocus` 延迟一次语言栏刷新**：可以覆盖部分异步宿主，但无法知道 context 是否已经压入，定时器也会给所有应用增加竞态；context 栈回调已经是更准确的完成信号，因此不采用。
- **继续用裸指针比较，只在 `OnPushContext` 无条件刷新**：无条件刷新会让后台文档的 context 变化改写当前语言栏，而且裸指针比较仍会丢掉属于当前文档的事件；先确认 COM identity 再刷新能同时收窄范围和覆盖包装指针。
- **把整个焦点处理搬到 Server**：语言栏状态、TSF context 和 `ITfTextEditSink` 都属于注入宿主进程的 TSF DLL，跨进程搬移会破坏 TSF 线程边界，因此不采用。

## Consequences

当前焦点文档异步获得 context 后，语言栏按钮会恢复可用，右键菜单可以重新进入 Aux pipe；顶层 context 变化仍会清理旧编辑会话，避免旧文档回包进入新 context。新增策略头和一个可移植 C++ 行为测试，Windows 真机上的 QQ 输入与任务栏菜单仍需报告者在 QQ 9.9.37 环境复验；本地交叉编译和 Wine 不能替代这一验收。

## Verification

`msime-tsf-context-stack-change-policy` 行为测试已用本机 C++ 编译运行，覆盖四种焦点与 context 栈时序；`python3 scripts/test-windows-native-run.py` 报告 195 项通过、2 项按宿主差异排除、116 项需要 Windows 构建、20 项非测试；本地 `bash scripts/verify-local.sh --quick`、`pnpm run verify-notes` 和 `git diff --check` 通过，本地 quick 的完整 Windows host 阶段因未配置 `target/windows-full` 而跳过，pipe-only 两架构通过。此前的 `rbuild` quick 在 Windows/pipe-only 阶段因远端共享构建盘无空间失败，已有 macOS 链接阶段也因同一远端环境失败，未将这两项环境失败归因于本次改动。未在本机 macOS 上声称真实 TSF/QQ 验收，PR 保留 QQ 9.9.37 实机复验缺口。
