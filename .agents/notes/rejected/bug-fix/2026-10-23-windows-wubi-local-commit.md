# Agent Note: Windows 五笔自动提交只写一次

Status: rejected — 真实双会话证明本地文本会丢失 Server 的繁体投影和插件改写，改用 Server 最终文本。

## Problem

#9452 报告部分五笔词组自动上屏两份。TIP 的字符路径已调用自己的 Host API，并经 `ApplyHostCharacterResult` 写入自动提交；Server 的 `ReplyComposer::edit` 却继续把同一次五笔提交投影成 `AutoCommitAndContinue` worker 消息。worker 入队时才捕获当前组字代次，本地提交已经完成时它会携带新的代次，代次检查不能保证消除重复。

## Proposal

TIP 与 Server 通过可选能力 `FanyImeProtocol::LocalCharacterCommit`（第 11 位）协商逐键提交归属。TIP 在最终 Main 写入边界确认当前存在有效的本地宿主会话，再给 `KeyEvent.modifiers_down` 加 `FanyImePipeFlags::LocalCharacterCommit`（`0x04000000`）；重连到旧 Server、失去本地会话或非按键包都会清掉这一位。它不是键盘修饰键，`PipeMetadata::key_modifiers` 统一剥离。

带归属声明的五笔字符自动提交使用现有 `SyllableCommit` 投影：TIP 独占文档写入，Server 只同步后续组字、清空前缀和计数。没有声明的旧 TIP 或无本地会话路径仍使用原 `AutoCommitAndContinue` worker。能力在产品和开发 Server 上都宣告，不进入必需能力；帧尺寸、候选选择、学习和引擎五笔算法保持不变。

## Historical audit

检索 `AutoCommitAndContinue`、`SyllableCommit`、`HostCharacterResult` 和五笔上屏。现有[五笔顶字保留后续按键](../../implemented/bug-fix/2026-10-10-wubi-top-commit-tail.md)定义引擎合并尾键，部分重叠但不持有 Windows 双进程投影；[Windows 组字细节](../../implemented/feature/2026-10-10-windows-input-compose-mac-parity.md)持有双拼、标点与日语组字行为，不涉及五笔自动提交，与本次无关。其余五笔偏好、查码、词库覆盖和桌面生命周期记录与写入归属无关，各自保留。

## Alternatives considered

- 不加能力协商，所有五笔自动提交都只由 Server 观察。它改动最少且正常本地会话的回归通过，但 TIP 明确保留了无本地宿主会话时的旧管道输入；在这种状态直接抑制 worker 会吞掉自动提交。逐键声明让旧客户端和回退路径仍能写入。

- 让 TIP 忽略自己的五笔自动提交，完全等待 Server worker。它可以复用原有 Server 提交路径，但会让五笔字符处理偏离已经使用真实 Host API 的通用提交路径，还要协调已推进的本地会话、剩余组字和 worker 时序。复用现有本地写入与 Server 观察路径更直接。
- 仅依赖组字代次丢弃 worker。它已经阻止失焦或旧组字消息写入，但消息是在 worker 收到时捕获当前代次，不能关联发起本次字符的旧代次；本地已提交后入队仍合法。

## Verification

`windows-wubi-auto-commit` 使用两份真实 Host API 会话和合成五笔表，经生产 `ApplyHostCharacterResult` 应用 TIP 提交、经 `ReplyComposer::basic_key` 处理 Server 回复。72 个双会话场景覆盖 86/98、三种预编辑样式、UI-less、四码唯一、连续重复输入同一词、大写顶字、关闭自动提交后空格选词。另外三个真实 Server 场景覆盖旧 Server 协商、旧 TIP 和本地会话不可用，验证 worker 仍提交一次；协议断言覆盖元数据剥离、失去能力或会话时清除旧标记、非按键包不带标记。小写顶字保留已选词段和下一组编码，测试明确确认其暂存行为，两端的编码和词段逐键一致。旧源码稳定失败于重复 worker 断言，修复后矩阵通过；C++ 严格语法检查通过。

## Consequences

声明本地归属的字符自动提交只有 TIP 写入，Server 保留统计和后续组字投影；普通候选选择仍走原来的选词回复。逐键元数据增加一项可选协议能力，兼容旧客户端与本地会话不可用的输入；旧 Server 协商不到能力时仍沿用原语义，完整修复依赖新版 TIP 和 Server 配对。

本机没有 Windows 桌面，真实 TSF 编辑器输入验收仍需 Windows；可移植回归验证生产会话、结果应用与回复投影，不声称完整 COM 系统验收。

## 替代决定

原方案的优点是减少一次 worker 编辑并复用 TIP 已有的结果应用，但 `17befb822a1b4cccb6a5df80e2dfbb0e5c52db81` 的真实双会话复现显示：繁体 Server 输出“測試詞組”，TIP 插入“测试词组”；实际导入并授权的 `before_commit` 插件也只改写 Server 文本。前述原验证未覆盖这两处差异，其通过不能支持最终提交文本一致。改用[Server 最终提交所有权](../../implemented/bug-fix/2026-10-23-windows-wubi-commit-owner.md)，本篇保留被放弃方案与原验证的历史记录。
