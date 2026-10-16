# Agent Note: 社区皮肤库读取复用异步请求代次

Status: implemented

## Problem

社区皮肤页读取本地皮肤库时同时手写了局部 `let current` cleanup 和手动 `libraryGeneration`。手动代次用于保护“获取”完成后新写入的皮肤不被更早的读取覆盖，但局部布尔值只是重复处理视图切换和卸载。

## Decision

`packages/ui/src/community/use-community-skin-library.ts` 持有社区卡片的本机库状态、`pillState`、`take` 和 `apply`。页面注入客户端、网格可见性、偏好修改及提示回调；普通卡片与 Harmony 卡片由[公共皮肤卡片](2026-10-16-community-skin-cards.md)持有，原 JSX、按钮文案、详情试用与发布流程保持。胶囊状态类型继续由本 hook 持有并经公共包入口导出。hook 的客户端契约只需要 `download` 与 `finishTrial`，本机库只需要 `load`；独立使用不依赖整页、图库或 toast provider。

使用 `useAsyncGeneration(harmony, browsing, localSkinLibrary)` 处理加载请求的生命周期，并保留 `libraryGeneration` 处理本地库变更与读取之间的业务顺序。两种代次各自负责独立约束，不合并它们。

获取仍先下载并开始临时试用，再以 `finishTrial(id, false)` 恢复原皮肤，成功后才更新库和提示；未完成的恢复保留到客户端清理时重试。同步 `acting` 集合在重渲染前就阻挡同一 id 的重复操作，UUID 大小写保持等价，换装统计仍报告社区原始 id。详情页的可保留试用与卡片的即时恢复各自持有生命周期，不合并两种业务语义。

## Alternatives considered

- 只抽出按钮状态计算：改动更小，但库读取、获取恢复、重复点击和清理仍留在 JSX 文件，不能独立复用整套操作。
- 把详情试用一起抽成统一操作：可以集中所有下载入口，但详情允许保留设计，卡片必须即时恢复；共同抽象会扩大本片范围并掩盖不同的清理条件。
- 删除手动 `libraryGeneration`：会让较早的读取覆盖刚完成的获取结果。
- 保留局部布尔值：行为可行，但继续复制共享异步生命周期守卫。

## Consequences

社区皮肤页在关闭网格、切换皮肤库或卸载时忽略旧读取，同时保留获取结果优先级。独立 hook 负责状态生命周期，页面需要显式传入操作依赖；卡片界面和交互不变。独立 hook 测试覆盖重复点击、大小写 id、实例隔离、过期读取、网格重入、恢复失败重试与应用失败；既有社区皮肤界面测试和迁到真实持有者的源码契约测试覆盖页面委托。
