# Agent Note: 云剪贴板上传表单独立复用

Status: implemented

## Problem

云剪贴板历史视图与会话状态已可独立复用，上传表单仍内嵌在 `panels.tsx`。表单与 hook 各自硬编码 4000，而剪贴板设置页已有公开的 `CLOUD_CLIPBOARD_MAX_UTF16`，维护长度边界需要同时修改多个来源。

## Decision

公开受控的 `CloudClipboardComposer` 和 props 类型，接收草稿、启用及忙状态、草稿变更与上传回调。文本框、计数、按钮和原有 DOM、样式类、ARIA、占位文案保留。上传只调用零参数回调，输入原样转发字符串，hook 继续持有草稿修订、请求互斥及上传后的刷新和清空。

组件与 hook 都直接导入既有 `CLOUD_CLIPBOARD_MAX_UTF16`，不新建长度配置或 helper。计数按 JavaScript 字符串长度，即 UTF-16 单元；不改成 Unicode 字符数。未启用、忙、空白或超限时只禁上传，不禁草稿编辑，不加 `maxLength` 截断。

## Historical audit

按云剪贴板、上传表单、组件名和长度常量检索活跃笔记：

- [面板状态](../../implemented/simplification/2026-10-19-cloud-clipboard-panel-state.md)继续持有会话和草稿代次；同步视图归属。
- [历史视图](../../implemented/simplification/2026-10-19-cloud-clipboard-history.md)持有列表及条目动作，本片是相邻独立边界，互不吸收。
- [端到端加密](../../implemented/feature/2026-10-11-cloud-clipboard-e2e.md)持有服务端 4000 个 UTF-16 单元限制和协议；本片只复用相同限制。
- 宿主账号接入、设备生命周期和中继偏好笔记持有平台或传输边界，与表单无关。没有被完全取代的决定或失效提案。

## Alternatives considered

- 直接复用通用 textarea 加按钮：组件更通用，但云剪贴板计数、警告样式和提交许可仍留在面板，不能单独消费完整上传表单。
- 表单接收完整客户端并自行上传：调用方接线更少，但重复会话状态并破坏现有草稿修订归属，因此只转发草稿和动作。

## Verification

- 公共入口的 12 项组件测试先因组件缺失失败，覆盖受控编辑、原文转发、UTF-16 边界、空白、忙状态及未启用时的编辑、超限警告和零参数上传。

- 新增组件与历史视图、hook、设备加密、代次及宿主客户端测试共 8 个文件、54 项通过，设置页相关 6 项通过，合计 60 项。
- 桌面构建、HarmonyOS 类型检查、lint、修改文件格式与笔记门禁通过；lint 保留原有 4 条未使用声明警告。
- 对固定基线 `3ca43cb9be74871c575c12b148447961776557e3` 的上传 JSX 忽略排版对照一致，仅替换回调与长度常量来源。
- 独立审查无阻塞问题，另跑 12 项组件测试、桌面类型检查及差异检查，全部通过。
- 本地 quick 通过，包含契约、Rust workspace、Android 交叉编译、Linux 桌面容器和 Apple bridge；PR CI 是合并门禁。

## Consequences

上传视图可独立复用；长度上限沿用既有公共常量，设置页、面板 hook 和上传视图共用同一来源。代价是新增公共 props 契约。

文本框在上传过程中继续可编辑，不冻结或截断。成功上传只清空未被再次修改的草稿，这个归属由 hook 保证。前端验证不代表原生宿主装机或真实账号联调。
