# Agent Note: 抽取设置社区视图状态

Status: implemented

## Problem

`SettingsShell` 直接组合皮肤页的「内置 / 我的 / 社区」标签、手机皮肤图库是否覆盖页面，以及桌面候选皮肤和插件图库的表单覆盖判定。这几条判断都依赖同一组社区服务能力和外壳视图状态，却分散在页面拼装代码中，容易出现标签仍可选但服务不存在、或图库覆盖判断和实际挂载条件不一致的情况。

## Decision

`packages/ui/src/settings/use-settings-community-views.tsx` 集中管理皮肤标签状态和三种社区覆盖结果：`phoneSkinCommunityShown`、`skinCommunityShown`、`pluginCommunityShown`。它复用 `useSettingsTabs`、`skinGridTabs` 与 `desktopSkinTabs`：HarmonyOS 触屏保留内置、我的和按服务过滤的社区分类，桌面提供本地皮肤和按服务过滤的社区皮肤分类；皮肤覆盖结果继续合并触屏分类和桌面候选皮肤视图，插件覆盖结果继续要求当前页、插件服务和社区视图同时成立。设置外壳只传入页面、平台、外壳视图、皮肤视图变更回调和服务能力。桌面分类合并的理由见[本地皮肤统一选择](../simplification/2026-10-18-local-skins-unified.md)。

## Alternatives considered

- **继续在 `SettingsShell` 内联三组判定**：改动最小，但标签状态和覆盖结果仍散落在页面渲染函数中，新增社区入口时要重复核对条件。
- **只抽出两个纯布尔函数**：能减少条件重复，但皮肤标签的 React 状态仍留在外壳，标签选择和覆盖结果可能再次分离。
- **把皮肤与插件页面组件一起搬到新模块**：边界更大，但会同时移动图库渲染、表单上下文和发布动作；本切片只收拢状态和可见性契约。

## Consequences

社区视图的能力投影和覆盖判定有独立 hook 与测试，外壳继续负责页面内容和布局。皮肤标签选择生命周期、社区服务缺失时的标签过滤、候选皮肤/插件图库覆盖表单的行为保持不变。hook 仍依赖 React 标签状态，因此它属于共享设置 UI 层，不替代社区图库自身的状态 hook。

社区视图 hook 和图库渲染位于[独立设置页组件](2026-10-19-settings-page-component-module.md)的调用层，图库仍在设置表单之外，覆盖判定与挂载规则保持不变。

覆盖结果与标签输入交给[偏好设置表单组件](2026-10-19-settings-preferences-form.md)，图库渲染由外壳组合的社区区域持有，不进入表单上下文。

皮肤页标签和两类皮肤图库的渲染由[设置皮肤社区区域](2026-10-19-settings-skin-community-section.md)消费同一组视图对象；hook 仍由外壳调用一次，本地表单继续使用相同标签和覆盖结果。插件来源标签与图库由[设置插件社区区域](2026-10-19-settings-plugin-community-section.md)消费同一组覆盖结果和外层分类状态，社区根页的渲染归属不变。
