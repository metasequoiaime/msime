# Agent Note: Windows 回车原文提交观测读取宿主会话

Status: implemented

## Problem

#9525 报告五笔回车提交编码后连续几次输入没有候选、无法上屏；#9527 正文完全相同。TIP 的回车处理在有效宿主会话中调用 `_HandleHostRawCommit`，而发送给 Server 的 `localCommitObservation` 仍读取旧 `GetKeystrokeBuffer` 与旧全局词段。有效宿主拥有的输入可能没有写进旧缓冲；Server 的 `ReplyComposer::dispatch` 会在 LocalCommit 派发前按当前词段加 `editing_text` 验证观测，不一致就拒绝请求。固定基点的生产观测代码在 17 项合成检查中有 9 处断言失败，证实这条来源缺口；它还不是报告者真实编辑器事件的复现。

## Decision

在现有宿主快照工具中增加回车原文观测读取：有效宿主只从同一次视图取得 `phrase_prefix + editing_text`，失败返回空且不复活旧缓冲；没有有效宿主时保留旧入口。按键发送入口使用该读取器，继续遵守现有小于 128 个 UTF-16 码元的载荷限制，不截断成另一段文字。Server 的观测校验、提交命令与恢复策略保持原规则。

历史检索：[宿主预编辑与光标](2026-10-22-windows-host-preedit-caret.md)覆盖同类旧缓冲来源缺口，部分相关并互链；[空格和点选同步宿主会话](2026-10-16-windows-space-selects-in-host-session.md)处理选词推进，原职责不变；[连续大写与本地回车](2026-10-15-uppercase-run-and-local-mode-enter.md)持有共享引擎的回车语义，不修改；插件指令架构只涉及插件提交观察权限，无本次观测载荷决定，保留。

## Alternatives considered

- **每次宿主按键后同步旧键击缓冲和全局词段**：可让旧调用方照常读取，但会增加第二份组字状态，部分选择、取消和方案切换都要同步，继续扩大漂移面。
- **Server 在观测缺失时直接按自身状态接受回车**：可以避免这次拒绝，但失去“TIP 交来的原文属于同一组字”的独立校验，不能证明文档上屏与 Server 状态一致。

## Verification

`scripts/test-windows-enter-observation.py` 编译执行生产按键入口的完整观测片段：基点 `3fc3fab9b4` 的 17 项检查中 9 处失败，修复后在 macOS 与 Windows 上均为 17 项全部通过。覆盖空或残留旧缓冲、宿主词段前缀、显示文本与原文不同、失败读取或解析、127/128 个 UTF-16 码元与扩展字符边界、无宿主、前缀单独存在、未组字及其他按键。模型只统计片段的结果，不等同真实 TSF 或报告者编辑器。既有生产预编辑、217 项延迟分类、恢复检查点和 183 项应用键回放回归通过。

Windows MSVC Debug 重建固定基点的 Host API 库并成功链接实际 TSF DLL。`msime-tsf-key-composition-state` 覆盖真实宿主 JSON 解析，`msime-tsf-raw-commit` 与新增 `windows-reply-composer-enter` 三项原生专项全部通过。后者使用真实 Server 会话，确认五笔和全拼方案确实生效，各连续四轮输入、拒绝空观测、核对状态未变、提交正确原文并立即续打；不依赖用户词库。

原 `windows-reply-composer` 在新增回车场景之前的 Home/End 断言失败；换回固定基点的未修改测试重新编译后同样失败。新增场景因此通过独立 CTest 参数运行，保留原测试及其失败，不修改断言凑绿。基点试跑后已恢复最终源码并强制刷新时间戳重编译，避免 Windows `copy` 保留时间戳而复用基点测试二进制。

Windows 上完整执行 `bash scripts/verify-local.sh --quick`，退出 1；新增观测检查在门禁中实际运行并通过。失败阶段共 19 个，与先前 Windows 完整 quick 的阶段名集合相同，未增加已知失败豁免。仍有可移植测试与宿主差异、POSIX/换行契约、`client-core` 测试缺字段、实验编辑器 `TextContainer.cpp` 的 `min/max` 宏冲突，以及 pipe-only 三个界面测试的警告错误。阶段名相同不证明阶段内所有用例都没有回归；本次修复依靠上述红例、观测检查与三项原生专项独立验证。缺少 SDK、Docker 或其他目标环境的阶段按脚本跳过，不算通过。

笔记三项门禁在 macOS 上实扫 1783 篇与 4 个归档封印通过，`git diff --check` 通过；当前尚未完成真实 Windows 编辑器安装验收。

## Consequences

观测在提交前读取，保留既有跨进程请求顺序与焦点令牌保护；此次不解决任意进程延迟造成的超时或完整重连队列问题。

观测仍可能对不上（超过 127 个 UTF-16 码元时清空、读宿主快照失败时为空）。被拒之后不再让整个 Server 停机：校验失败抛 `RequestRejected`，只拒绝这一个请求并断开这一个客户端，输入队列继续运行，见[单个请求校验失败不再停掉输入队列](2026-10-25-windows-rejected-request-keeps-input-queue.md)。已有日语转换、释义与整句改字的回车路由继续按各自原规则处理。
