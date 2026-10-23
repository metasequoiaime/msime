# Agent Note: 插件设置状态与导航独立维护

Status: implemented

## Problem

`PluginsSection` 的列表、详情、声音效果和名单视图已经独立，但入口仍同时持有目录加载、导入删除、脚本授权、名单编辑、筛选和 DOM 焦点。修改视图需要穿过异步代次与副作用，状态契约也无法独立测试。

## Decision

`use-plugins-section.ts` 的 `usePluginsSection` 负责目录、授权、名单、筛选、派生值和操作，继续复用 `useAsyncGeneration` 与 `useAsyncActionRunner`。两者的 owners 均保持 `active`、`client`、`mentionsEditable`、`scriptsShown`，目录成功后重读失败仍保留原目录，未保存名单不被重访读取覆盖。

`use-plugin-navigation.ts` 的 `usePluginNavigation` 持有以类型和 id 表示的视图、根节点和焦点恢复；返回列表时优先原入口，其次同类型首行，最后列表首行。删除成功也调用同一个返回动作。入口只组合 hook 并分发 JSX，详情仍从最新目录按键查找。

## 旧笔记审计

检索活跃笔记中的 `PluginsSection`、`plugins-section`、`pendingFocus` 和 `openedFrom`，没有持有整个 React 插件状态边界的笔记。[插件导航行](../../implemented/simplification/2026-10-17-plugin-nav-row.md)、[插件操作块](../../implemented/simplification/2026-10-17-plugin-action-block.md)部分重叠，继续约束各自的展示与焦点标识。[macOS 插件库](../../implemented/feature/2026-10-19-macos-plugin-library.md)和 [Windows 插件库](../../implemented/feature/2026-10-20-windows-settings-plugin-library.md)部分重叠，仅同步共享筛选所有者的路径事实，布局和保留筛选的决定不变。Windows 原生插件页、选包打开播放、农历关键词和光标特效笔记属于平台功能或运行时规则，与状态边界无关。没有需要吸收、删除或否决的旧提案。

## Alternatives considered

- **一次性把状态和 JSX 移到另一个大组件**：接线最少且易于逐行对照，但目录与异步操作仍不能脱离渲染组合，因此不采用。
- **将导航、目录、授权和名单都放进一个 hook**：调用方只需一个入口，减少模块数；但焦点恢复只依赖 DOM 与视图，拆开后可以独立维护，且避免删除操作直接修改焦点 ref。业务状态 hook 组合导航，调用方仍只有一个入口。
- **为所有设置页引入通用状态机**：可统一加载与导航，但各页的确认、重访和焦点语义不同，会扩大本次迁移；保留已有公共异步机制并只抽取插件页边界。

## Verification

- 迁移前 90 项插件与脚本行为测试通过；状态边界检查先按预期失败，迁移后 7 个相关文件、107 项测试通过，独立审查重跑同样 107 项通过且无必须修复项。
- 源码门禁确认状态独立且继续复用公共代次和操作机制；新增 6 项测试覆盖重访保留未保存名单与筛选、StrictMode 下客户端替换隔离旧目录、授权及名单结果、重读失败保留目录。
- 桌面类型检查与构建、Harmony 类型检查、变更文件 lint 与格式、笔记校验和本地 quick 门禁通过，包含 Linux 设置外壳容器编译；原生 Windows 交叉编译和 HarmonyOS ArkTS 因本机工具链或构建输入缺少而跳过，未声明设备验收。

## Risks

移动 effect 可能改变代次先后，抽取时保留公共代次与动作 hook 先于读取 effect 的顺序。页面失活只重置导航，不清筛选或名单编辑；`onError` 身份变化不触发读取。此次只改共享 React 边界，不涉及原生宿主、文件读写或设备验收。

## Consequences

状态与操作可以脱离完整设置页单独验证，入口继续保留原属性、默认值、视图结构和文案。导航模块独立维护 DOM 焦点协议，业务状态只调用打开与返回动作；两个模块保留在共享 UI 的内部，不增加包入口 API，也不引入新依赖或持久化状态。
