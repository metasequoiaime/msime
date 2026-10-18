# Agent Note: 偏好设置表单组件

Status: implemented

## Problem

`SettingsShell` 同时绘制外壳、社区图库和完整偏好表单。表单的页面集合、上下文与恢复默认和重新读取策略仍内联在外壳中；后续页面集合变化需要穿过无关的平台导航和图库代码。

## Decision

`packages/ui/src/settings/settings-preferences-form.tsx` 的 `SettingsPreferencesForm` 持有现有 `SettingsFormFrame`、`SettingsFormContext.Provider`、全部偏好页面及 `SettingsFormFooter`。外壳保留 `draft && isSettingsFormPage(page)` 挂载条件，把同一份 `{ ...model, draft }` 作为已加载的 `SettingsFormModel` 传给组件，组件直接将该对象提供给上下文。皮肤标签、图库覆盖标志、帮助动作、恢复默认和重新读取回调也由外壳传入；组件不创建模型、导航或持久化状态。

页面顺序、隐藏方式、手写并页判定、隐私页仅在打开时挂载、保存失败的提示归属和页脚策略保持不变。社区图库及无表单时的重新读取按钮继续位于表单之外。公共入口转导出组件和属性类型。

## Historical audit

按 `SettingsFormFrame`、`SettingsFormFooter`、`SettingsFormContext`、`SettingsFormModel` 和表单组装检索活跃笔记，没有既有完整表单组件提案。

- [设置页组件模块](../../implemented/simplification/2026-10-19-settings-page-component-module.md)部分重叠，继续负责外壳和其他页面组装，本次把其中偏好表单委托给子组件。
- [社区视图](../../implemented/simplification/2026-10-17-settings-community-views.md)部分重叠，继续持有图库覆盖状态；表单只消费该状态，图库保持在表单外。
- [工具栏与键盘手写精简](../../implemented/simplification/2026-10-16-toolbar-keyboard-handwriting-settings-prune.md)部分重叠，继续持有手写合并和无偏好页面不显示恢复默认的决定；本次只移动这些规则的原有渲染调用。
- [皮肤发布](../../implemented/simplification/2026-10-16-candidate-skin-publication.md)只共用表单嵌入场景，与本次无关。

## Alternatives considered

- **只抽页面列表**：组件输入更少，也能减少外壳中的页面导入。但表单上下文与页脚策略会继续跨两个模块维护，页面集合和它们共用的已加载模型没有完整归属，因此一起抽出完整表单。
- **新组件自己调用 `useSettingsPageModel`**：调用处更短。但会新建另一份草稿和持久化生命周期，与外壳状态分离；因此始终接收外壳现有的表单模型与动作。

## Consequences

完整表单有独立公共组件，外壳减少页面导入和表单策略细节；代价是多一层无状态组件与八个属性契约。组件接收外壳已经加载的模型，不另建草稿或持久化生命周期；新增边界仍需守住原挂载条件与图库位置。

临时结构验证确认完整表单 JSX 一致，唯一差别是 Provider 对象构造移到调用处；八个属性接线、外壳其余函数、原公共导出保持一致。组件及 522 个传递模块没有到公共入口的运行时依赖。抽取前十项聚焦基线通过；构建、类型检查和 lint 通过，lint 只有既有未使用声明警告。自动化前端验证不代表原生宿主装机验收。

quick 门禁通过，覆盖 Rust 工作区、Android 目标、Linux 桌面外壳、共享 Apple 桥和 HarmonyOS 设置包检查；缺少完整工程的 HarmonyOS ArkTS 编译按脚本跳过。独立审查没有阻塞项或重要问题。

完整设置页、保存及重新读取生命周期、社区视图和图库回归共七个文件、434 项通过。改动文件格式、笔记树与格式及归档封印、`git diff --check` 均通过。
