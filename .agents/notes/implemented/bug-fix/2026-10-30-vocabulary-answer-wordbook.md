# Agent Note: 背词作答明确绑定卡片来源词书

Status: implemented

## Problem

共享会话收到只有 `word` 和 `known` 的作答后，读取全局当前选择的词书。另一窗口选了同词不同义的书后，旧卡片的答案会写到新书；若新书没有该词则报错，旧卡无法正常完成。

## Decision

`ReviewAction::Answer` 必须携带显示卡片时同一状态快照中的 `wordbook`。共享层只加载这本来源书并写入它的进度，返回状态仍属于当前选择的书。来源书为空、不存在或已删除则拒绝，缺少来源字段的请求在解析时拒绝。所有仓库宿主同步这个 JSON 契约；不改变 C 函数签名、学习计划、每日计数和产品默认值。

### 既有笔记审计

- [客户端代次](../../implemented/bug-fix/2026-10-15-vocabulary-action-owner.md)：部分重叠，保护界面客户端归属，不覆盖多个窗口共享词书选择；互补保留。
- [原子清空](../../implemented/bug-fix/2026-10-12-vocabulary-reset-settings-atomic.md)：部分重叠，存储锁内保留设置的决定保持；作答复用已有按明确书 id 写进度的存储接口。
- [内置词书布局](../../implemented/bug-fix/2026-10-20-bundled-wordbooks-packaging.md)：资源根和装包验收，与作答来源无关。
- [Win7 词书面板](../../implemented/feature/2026-10-24-windows7-vocabulary-panel.md)：部分重叠，复用表单的结构保持，同步其作答契约。
- Windows 设置对齐、iOS 与 Android 插件页：只涉及功能入口和插件目录，不决定卡片来源，保持。

## Alternatives considered

- 可选来源字段、缺省使用当前选择，能兼容旧请求；但旧宿主仍会静默把答案记错书，因此要求所有调用方明确来源，缺字段直接拒绝。
- 作答时要求来源仍是当前选择，能防止串账；但另一个窗口选书不应让仍然有效的旧卡无法记回原书，因此只验证来源书可读取，不把全局选择当作作答许可。

## Verification

共享层新增 3 项回归在旧实现失败，修复后整个 `vocabulary::` 的 109 项通过。共享 React 页旧实现 5 项失败，修复后 37 项通过；覆盖初次加载、切换词书、客户端替换后作答携带同一队列的来源书。桌面及鸿蒙前端类型检查和生产构建通过。

C API 回归检查缺来源、空来源、不存在来源和已删除书的请求不改进度；有效旧卡只写来源书，返回当前书的释义与队列。Android 设备专项与 iOS 存储回归同步相同场景。`bash scripts/verify-local.sh --quick` 通过，覆盖仓库契约、Rust workspace、Android 与 Wasm 目标、Android 宿主 Java、macOS 原生宿主与共享 Apple bridge，以及 Windows x64/x86 的 pipe-only 交叉编译。Linux 缺 Docker、Windows 完整原生依赖未配置、Harmony ArkTS 缺构建前置，相关阶段跳过。原生宿主实际设备验证范围与 CI 结果见 PR。

## Consequences

跨窗口换书不再把旧卡的学习结果记到新书。来源字段成为必填，所有随产品构建的宿主同步更新；旧二进制配新库会明确失败而不会猜测归属，不支持混装宿主与共享库。C 函数及结构布局保持，ABI 版本仍为 3，JSON 作答请求收紧。原书读取与删除的更深层事务边界保持现状，已经加载后并发删书的情况不由本次改动扩展。
