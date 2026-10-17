# Agent Note: 抽取设置社区视图状态

Status: implemented

## Problem

`SettingsShell` 直接组合皮肤页的「内置 / 我的 / 社区」标签、手机皮肤图库是否覆盖页面，以及桌面候选皮肤和插件图库的表单覆盖判定。这几条判断都依赖同一组社区服务能力和外壳视图状态，却分散在页面拼装代码中，容易出现标签仍可选但服务不存在、或图库覆盖判断和实际挂载条件不一致的情况。

## Decision

`packages/ui/src/settings/use-settings-community-views.tsx` 集中管理皮肤标签状态和三种社区覆盖结果：`phoneSkinCommunityShown`、`skinCommunityShown`、`pluginCommunityShown`。它复用 `useSettingsTabs` 与 `skinGridTabs`，只让 HarmonyOS 触屏在有社区皮肤服务时启用皮肤分类标签；皮肤覆盖结果继续合并触屏分类和桌面候选皮肤视图，插件覆盖结果继续要求当前页、插件服务和社区视图同时成立。设置外壳只传入页面、平台、外壳视图和服务能力。

## Alternatives considered

- **继续在 `SettingsShell` 内联三组判定**：改动最小，但标签状态和覆盖结果仍散落在页面渲染函数中，新增社区入口时要重复核对条件。
- **只抽出两个纯布尔函数**：能减少条件重复，但皮肤标签的 React 状态仍留在外壳，标签选择和覆盖结果可能再次分离。
- **把皮肤与插件页面组件一起搬到新模块**：边界更大，但会同时移动图库渲染、表单上下文和发布动作；本切片只收拢状态和可见性契约。

## Consequences

社区视图的能力投影和覆盖判定有独立 hook 与测试，外壳继续负责页面内容和布局。皮肤标签选择生命周期、社区服务缺失时的标签过滤、候选皮肤/插件图库覆盖表单的行为保持不变。hook 仍依赖 React 标签状态，因此它属于共享设置 UI 层，不替代社区图库自身的状态 hook。
