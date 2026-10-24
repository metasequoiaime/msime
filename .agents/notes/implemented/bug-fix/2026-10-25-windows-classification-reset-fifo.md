# Agent Note: Windows 分类期间重置保留尚未发送的尾键

Status: implemented

## Problem

#9519 报告快速连续输入的回包超时、重试积压和候选消失。已有[本地重置诊断](../process/2026-10-24-windows-local-session-reset-diagnostics.md)没有改变行为；它提出的完整自锁链尚未由真实 Windows 日志证明。取消组字专用编辑会话绕过普通 worker Ready 检查，并主动完成本地重置，因此不能假定必须再处理普通按键才能清除重置。

独立确认的缺陷更窄：延迟分类读取 compartment 和查询焦点会进入 COM，期间同焦点本地重置可重入。分类返回后，`_IsDeferredKeyOwnerCurrent` 将重置等同于焦点失效，`_ResolveDeferredKeyClassification` 随即清空整队，丢掉尚未发送的尾键并推进焦点代次。直接执行生产分类、所有权检查和恢复方法的夹具重现此行为。

## Decision

分类的两次 owner 查询允许在本地重置期间核验真实线程焦点、顶层 context、前台窗口和焦点代次。内部参数 `allowPendingReset` 默认关闭；应用文字写入与应用键回放继续使用严格重置屏障。

确认仍为原 owner 后，重置中的分类键返回 FIFO 首位，保持未分类状态、捕获的修饰键和 context 引用；归还本次尚未发送的尝试次数，退休在途 token。随后复用 `_ArmDeferredRecoveryForTransport`，将已应用检查点移到原首键和其他尾键之前。检查点源队列随搬移清空，不重复恢复。组码状态仍由现有恢复方法整队作废。

归还路径不调用 `_RetryDeferredKeyReplay`，不再次标脏传输。现有本地重置成功路径负责唤醒 drain；重置阻塞时继续等待，不能提前向旧会话派发。真实失焦、禁用或前台变化仍优先作废整队。

## Historical audit

- [本地重置诊断](../process/2026-10-24-windows-local-session-reset-diagnostics.md)：部分重叠，保留诊断决定，补充未证实闭环的事实修正，并互链本修复。
- [网址延迟分类](../feature/2026-10-15-engine-email-url-completions.md)：部分重叠，保持选择结果未定时延迟分类的契约，不改变网址或邮箱语义。
- [应用键回放顺序](2026-10-16-windows-application-key-replay-order.md)：部分重叠，保留实际写入和注入的严格 owner 检查，默认参数不放宽该入口。
- [五笔提交所有权](2026-10-23-windows-wubi-commit-owner.md)：部分重叠，保留 worker 提交及组码失效边界，不把未确认的组码恢复成字母。
- 其他检索命中属于设置页、词库和移动端流程，与本切片无关；没有吸收或删除旧决定。

## Alternatives considered

- **用普通失败重试保留键。** 现有重试已经会前移检查点并有有界退避，复用成本低；但它无条件标脏传输，会在已有重置中再触发重连，并消耗尚未发送请求的预算，因此不用。
- **只移除分类末尾的 pending 条件。** 改动最少；但真实 owner 方法仍会因 pending 返回 false，依然清队列。测试必须执行真实 owner，不能用始终成功的替身遮住这一层。
- **放宽所有 owner 检查。** 可以不增加调用参数；但应用文字写入和导航注入不能越过本地重置屏障，因此仅分类显式允许，默认保持严格。

## Verification

真实方法夹具在修复前出现 FIFO、前缀和恢复断言失败。首次手造检查点沿用了 `ApplicationText` 类型，已按真实检查点改为 `KeyDown`；仅有分类方法绿色仍不充分，改为提取真实 owner 后再次得到 17 项行为断言失败，补齐分类专用 owner 边界后通过。最终 `python3 scripts/test-windows-deferred-classification.py` 为 265 项、0 失败；应用键回放为 191 项、0 失败，恢复检查点为 0 失败，五笔 worker 生命周期为 32 项、0 失败。`pnpm run verify-notes` 通过。独立代码复审无阻断项。本 worktree 完整 `bash scripts/verify-local.sh --quick` 退出 0：Windows 可移植用例 165 项通过，Rust 工作区、Android Rust、Linux Tauri 外壳与 Apple 桥接编译通过；按改动范围跳过无关平台。本机没有 MinGW，未运行 Windows 原生构建；macOS 原生不在该切片改动范围。

覆盖同焦点重置、两次 owner 查询期间重置、检查点顺序、重复挂起不消耗发送重试预算、引用与 token 退休，以及真实线程失焦、顶层 context 改变、前台变化、禁用、组码作废。测试输入均为合成值。

Win7 设置夹具同时补齐当前 develop 新资料契约要求的 `providerNames`，覆盖 profile、rename、upload_avatar、remove_avatar；原产品校验与断言不变。补丁来自固定提交 `36c7624c8b` 的已复审测试修正，其 Windows 对照在仅同步当前账号头文件时先重现旧夹具断言失败，仅替换夹具后 34.61 秒通过。本工作树尚未在 Windows 重跑该设置专项，不能混淆证据归属。

## Consequences

本切片修复已确认的分类重置丢键，不改变 50ms 回包预算或 30ms 候选渲染屏障，不证明报告中的整项 6 秒卡顿已经消除。真实 Windows TIP/Server 编辑器快速输入与新版诊断日志仍需验收，PR 使用 `Refs #9519`。
