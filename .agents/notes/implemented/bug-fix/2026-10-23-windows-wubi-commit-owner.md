# Agent Note: Windows 五笔自动上屏保留 Server 最终文本并只写入一次

Status: implemented

## Problem

#9452 报告 Windows 输入部分四码词组出现两份。固定公开词库的原始条目没有重复字面。生产 TIP 通过 `ApplyHostCharacterResult` 写入本地 Runtime 的自动上屏；Server 的 `ReplyComposer::edit` 又把同一按键的五笔提交路由到 `AutoCommitAndContinue`，发送 worker 提交。需要以两份真实会话复现重复的写入所有权，再决定修复。

## Decision

保留 Server 的 `AutoCommitAndContinue`、繁体投影与脚本改写。在线 TIP 的五笔自动提交只推进本地 Runtime，等待 Server 的最终文本；worker 写入一次后依据本地实时 host view 重建余码，不按旧缓冲计数裁剪已经推进的 Runtime。旧无 host 路径保留 consumed 缓冲协议。需覆盖字母提前输入和多次提交的 worker 代次，防止等待期间推进或结束组字造成迟到帧丢失。

## Historical audit

[五笔四码碰撞续输](../../implemented/bug-fix/2026-10-18-wubi-mixed-pinyin-continuation.md)与[顶字尾部](../../implemented/bug-fix/2026-10-10-wubi-top-commit-tail.md)部分重叠，继续持有引擎续输和提交文本，不改这些规则。[四码唯一偏好](../../implemented/feature/2026-10-08-wubi-auto-commit-unique-preference.md)持有跨平台开关，不改默认值。[脚本插件提交](../../implemented/architecture/2026-10-12-script-plugins-modify-commit-and-candidates.md)持有提交变换与观察边界，后续需确认计数没有丢失。其余命中为文案、查码、长度和生命周期，与本次文档写入归属无关。

## Alternatives considered

- 让 TIP 原文提交、Server 仅观察：可以复用 `SyllableCommit`，并减少一次 worker 编辑；但 Server 才执行繁体投影和脚本 `before_commit`，抑制 worker 会丢掉这些变换，不能采用。
- 为 worker 另建独立队列：能单独管理提交载荷，但还要协调已经持有物理按键的 FIFO 与重试代次，容易形成双重所有权。延长现有 replay token 的完成阶段即可串行化两条路径。
- 只在本地 `has_commit` 后设等待标志：状态最少，但 worker 可在 IPC 或 COM 重入期间先到，必须在发送按键前注册本地阶段，早到消息只暂存。

## 合并已有修复

#9458 的[已放弃本地提交方案](../../rejected/bug-fix/2026-10-23-windows-wubi-local-commit.md)最初以可选 `LocalCharacterCommit` 标记让 TIP 写入本地文本、抑制 Server worker。固定提交 `17befb822a1b4cccb6a5df80e2dfbb0e5c52db81` 经真实双会话繁体输出和已授权插件复现，Server 最终文本与 TIP 实际文本不同；该路径不能保留。将本篇的最终文本所有权方案整合到同一 PR，移除不再使用的协商位与逐键标记。原 PR 的 86/98、关闭自动提交、暂存词段和普通选词矩阵保留并改为核对 Server worker 最终文本，不另开重复 PR。

## Verification

固定基线 `53df4a89c1db06d264eadf4e57cb0b54758d61fa` 的共享库在本 worktree 构建。原始双会话回归在“同一码只写一次”断言失败；修复后的 72 个双会话场景退出 0，覆盖预编辑三种样式、UI-less、简繁、worker 早到与晚到、连续四码唯一提交、Shift 大写尾部及分段顶字保留。测试执行生产 `WubiCommitOwner`、结果应用器、`ServerWubiCommit`、两份真实会话和 `ReplyComposer`；文档与系统调度边界为合成观察器。改写载荷保留测试不等于真实插件加载的双进程验收。

现代 Windows 两侧会强制开启 `phrase_preedit` 能力，不能用传入 false 假装关闭它。非唯一 `abcdx` 在此能力下把前词保留在 `phrase_prefix`、把 x 留作下一段，尚不写入文档；回归明确核对这个边界，不把它当成自动提交失败。

提取实际窗口分支、worker 编辑类、收尾、清队列和超时方法的回归在 macOS 与 Linux 容器均 32 项通过。恢复“只清 owner 不清 in-flight”“先改 owner 再校验消息”“现代无 owner 仍允许 token 0 写入”三个缺陷后，相应用例分别出现 1、2、1 项失败；恢复修复后通过。真实 Windows 系统输入框验收尚缺，不据此关闭 issue。

生命周期夹具直接提取生产头文件中的 `_IsDeferredKeyReplayCurrent` 声明与实现，不再由替身给上下文参数增加默认值。这样 worker 窗口分支漏传第三参时，本机也会像 Windows GNU 构建一样编译失败。窗口派发尚未捕获 context，先显式传 `nullptr` 核验 token 与焦点代次，再取得原槽 context；后续编辑会话仍校验捕获的 context。该夹具在修复前复现“需要三个参数、只传两个”的编译失败，补齐调用后 32 项检查通过。

参数补丁的首轮 quick 在 Linux 的 `linux-setup-registration`（30 秒）和 `linux-clipboard-tool-parity`（3 秒）遇到子进程超时，原日志保留在 `target/issue-9452-arity-quick.log` 与 `target/linux-native-gate.log`，不计为通过。运行中的主题测试采样停在 `fsync` 的 `wait_log_commit`；相同安装命令在同一容器的磁盘临时目录耗时 2.855 秒，在可执行的 tmpfs 临时目录耗时 0.947 秒，均退出 0。临时文件系统上的完整安装注册测试也通过。这些对照定位本次同步写入等待，不修改产品逻辑、测试超时或失败基线。完整复验通过命令级 Docker 包装器，仅给 Linux 原生测试容器添加可执行的 512 MiB tmpfs `/tmp`；包装器打印实际环境，编译仍使用原有 `target/linux-build-gate`，正常 pre-push 运行全部门禁。完整结果以该次推送日志为准，上述定向对照不代替整条检查。

光标已经移出组字范围而尚未调用本地 `character` 时，输入处理返回 `S_FALSE`，保留原回放失败恢复；不能以初始 `S_OK` 提前完成 FIFO token。提取真实早退块的回归在修复前失败，修复后通过，且早到 worker 不会夺走未执行字符的恢复归属。

应用键回放、延迟分类与恢复三个现有提取夹具直接包含生产 `WubiCommitOwner`；恢复夹具也提取真实 `_FinishWubiWorkerEdit` 与 `_ClearDeferredKeyDowns`，不能用空方法跳过已经推进字符的失败收尾。三项分别通过 191、220 项检查和恢复检查点回归，覆盖等待期间保留 FIFO 槽与引用、阻止尾键、释放后仅派发一次、旧 token 不影响新 owner、未推进时正常恢复、已推进且有或没有提交文本时禁止重放，以及成功 worker 收尾释放。仅在内存中移除相应 owner guard 后，三个专项分别出现 2、1、2 项失败，仓库生产代码保持不变。首轮完整 quick 的三项夹具编译失败与 Linux 超时日志保留；修正后的整条 `bash scripts/verify-local.sh --quick` 重跑退出 0，其中本机 Windows 测试 178 项通过，Linux 111 项全部通过。Windows 原生交叉编译因本机没有 MinGW 跳过，不能替代真实 Windows 双进程验收。

## Consequences

在线五笔只由 Server 最终文本写入文档一次。本地 Runtime 先推进，worker 依据实时 host view 恢复余码，不裁剪已停止更新的旧缓冲。旧无 host 路径保留 consumed 协议；现代路径没有 retained token 时拒绝 worker 写入。

每个在线五笔输入在 IPC 前绑定现有 replay token，本地提交成功后保留 in-flight，worker 写入并恢复组字后才真正释放。焦点、代次和原 context 继续校验；旧窗口消息先核验载荷再触碰阶段状态。等待期间不更新宿主偏好，避免余码视图被切换。

worker 丢失、异步编辑拒绝、部分写后失败或 2 秒超时清理原队列并请求 dirty/reset，禁止重放已经推进的字符或重复写入。清 IPC 请求在本地已经推进时同时释放等待中的 in-flight 与 context；尚未推进时保留原发送/编辑失败回调的重试所有权。代价是故障批次不能自动重放；这避免不确定写入后再次提交同一文字。
