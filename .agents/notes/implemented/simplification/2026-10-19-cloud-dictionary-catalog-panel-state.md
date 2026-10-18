# Agent Note: 完整云词库目录状态集中到公共 hook

Status: implemented

## Problem

`CloudDictionaryCatalogPanel` 把查询草稿、确认参数、分页、编辑和删除交互放在呈现函数中，无法独立复用整套目录状态。真实组件回归还确认：删除确认期间更换客户端后，旧确认仍会向旧宿主发送 `edit_catalog`。

## Decision

提供 `useCloudDictionaryCatalogPanel(client)` 与派生的 `CloudDictionaryCatalogPanelState`，迁移现有状态、查询、表单及修改后刷新。面板继续持有 DOM、类型展示、禁用条件和窗口导航。原内联表单更新与取消分别命名为 `changeForm`、`cancelForm`。

保留只在明确查询时发送编码、分页和修改后刷新使用已确认参数、类型切换清理、全局目录修订号及错误文案。确认前捕获 `usePanelAction` 的操作代次，回答后检查 `isCurrent`，避免旧确认重新调用失效宿主；其余操作和 effect 顺序保持。

## Existing notes audit

按组件名、`catalog_entries`、`edit_catalog`、完整目录与确认交互检索活跃笔记。

- [单词条面板状态](../../implemented/simplification/2026-10-19-cloud-dictionary-panel-state.md)部分重叠；使用相同确认代次原则，单词条 ID 与完整目录全局修订号契约仍分别持有。
- [查询栏组合输入](../../implemented/bug-fix/2026-10-14-cloud-dictionary-ime-submit.md)部分重叠；保留共享工具栏的组词和禁用规则，不把键盘事件移进 hook。
- [桌面客户端装配](../../implemented/simplification/2026-10-18-desktop-cloud-dictionary-client.md)与 Windows 云账号笔记持有宿主能力与会话，本项不改变它们。
- 其它“目录”命中属于文件路径句柄、资源安装、皮肤目录、设置导航和运行时流测试，与云词库完整目录状态无关；`theme_catalog_entries` 属于主题菜单，与本项无关。

## Alternatives considered

- 保留逻辑在呈现组件中，模块数最少，但无法直接复用或独立验证状态和交互。
- 新增独立客户端 generation 可以只隔离客户端切换，但现有操作代次在客户端替换和卸载时已作废，也能拒绝被更新查询取代的旧确认，所以复用原守卫。

## Verification

抽取前相关基线 41 项、实际设置页目录消费者 1 项通过。旧确认真实组件回归在修复前因旧宿主收到 `edit_catalog` 失败；新 hook 首次 7 项均因缺少公开导出失败。抽取后相关套件 51 项、设置页目录消费者 1 项通过，合计 52 项；覆盖确认失效、正常删除确认与取消、StrictMode 无自动查询、已确认参数分页、快捷短语空编码、编辑参数与目录修订号、保存防重与刷新失败、旧查询成功/失败隔离及旧保存不清理新表单。

桌面类型检查与生产构建、HarmonyOS 类型检查、改动文件 lint 通过。

独立只读审查重跑新增 10 项全部通过，AST 对照确认除删除确认守卫外的原状态、查询、错误、修订号及语句顺序一致，完整 JSX 和表单动作等价；既有类型转导出保持，54 个传递模块无运行时入口循环。格式、笔记和差异检查通过。

`bash scripts/verify-local.sh --quick` 输出 `quick check passed`，覆盖仓库契约、Rust workspace、Android 目标、Linux 设置外壳和共享 Apple bridge 编译；HarmonyOS 原生构建缺少 hvigorw 与 staged 依赖、Windows 缺 MinGW，其余原生宿主按改动范围跳过。

## Consequences

目录状态可以脱离固定 JSX 复用，代价是增加公共 hook 与派生类型。旧确认不能在操作代次失效后重新调用宿主，已发出的操作只隔离结果，不取消底层请求。共享 React/jsdom 与编译不代表原生宿主装机或真实云账号验收。
