# Agent Note: 云词库文件面板状态集中到公共 hook

Status: implemented

## Problem

`CloudDictionaryFilesPanel` 同时持有个人词库导入、导出、完整快照备份与恢复的状态和异步生命周期，呈现函数还直接管理文件读取、宿主保存和浏览器下载。这些状态无法独立复用和验证；确认恢复或导入期间替换客户端也必须拒绝旧宿主操作。

## Decision

提供 `useCloudDictionaryFilesPanel(client)` 与派生的 `CloudDictionaryFilesPanelState`，迁移现有状态、文件传输动作、快照预览和恢复确认。面板保留完整 JSX、文件 input、下载呈现、窗口导航和宿主能力条件，继续注入已有 `readDictionaryFile`、`downloadBlob`、`saveExport` 与原生快照选择能力。

保留 64 KiB 词库文件和 512 MiB 快照边界、先预览再确认恢复、原生与 Web 快照协议、客户端生命周期代次、恢复取消 cleanup 以及导出保存顺序。确认回答后增加代次检查，避免失效客户端接收导入或恢复请求。

## Existing notes audit

按 `CloudDictionaryFilesPanel`、快照导出恢复、文件生命周期与客户端装配检索活跃笔记。

- [桌面云词库客户端装配](../../implemented/simplification/2026-10-18-desktop-cloud-dictionary-client.md)持有宿主能力组合，本片只抽取页面状态。
- [云词库应用面板状态](../../implemented/simplification/2026-10-19-cloud-dictionary-apply-panel-state.md)持有本机应用队列和轮询，本片保留云端文件传输协议。
- [云词库查询栏](../../implemented/bug-fix/2026-10-14-cloud-dictionary-ime-submit.md)与[完整目录状态](../../implemented/simplification/2026-10-19-cloud-dictionary-catalog-panel-state.md)属于其他面板状态，不合并。
- 文件解码、快照大小和平台能力的既有笔记命中属于底层工具或宿主契约，与本片 hook 边界无冲突。

## Alternatives considered

- 保留状态在呈现组件内最少改动，但无法独立复用完整文件传输交互，客户端生命周期测试只能依赖庞大 JSX。
- 把文件读写下沉到 hook 的新抽象会改变宿主注入边界；继续使用现有工具和客户端可选能力，hook 只编排状态与协议。

## Verification

新 hook 测试先因缺少公开导出失败；抽取后 hook、文件面板、快照、客户端生命周期和设置页消费者共 25 项通过。完整 JSX、文件边界、协议字段、恢复确认与 cleanup 顺序保持；快照导出客户端切换旧回复不再改变新面板。桌面构建、HarmonyOS typecheck、改动文件 lint、格式和差异检查通过；独立审查与 quick 门禁随后执行。

## Consequences

文件读取和快照恢复是异步操作，必须同时检查 mounted 与客户端代次；原生快照 token 不能泄露到 Web 文本协议。组件和编译检查不代表真实云账号或原生设备验收。
