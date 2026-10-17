# Agent Note: 设置外壳平台展示判定

Status: implemented

## Problem

`packages/ui/src/index.tsx` 的 `SettingsShell` 同时计算平台标识、窗口标题栏、导航搜索位置、iPad 侧栏、移动端撑满内容和鸿蒙手机推入页面。这些判定只依赖宿主能力与当前路由，却和窗口交互、页面拼装及返回逻辑交错在同一入口，后续拆分外壳时容易复制平台组合规则。

## Decision

`packages/ui/src/settings/settings-shell-platform-presentation.ts` 提供纯函数 `settingsShellPlatformPresentation`，集中计算平台外壳展示结果。它接收平台、移动标签、当前页面、窗口能力、可用页面和移动端无标题页，返回原有的 HarmonyOS 外观与推入状态、内容撑满、窗口标题栏、页面标题、iPad 侧栏、搜索位置和品牌显示判定。`SettingsShell` 只消费结果；窗口拖动、页面历史和页面内容仍由外壳负责。

## Alternatives considered

- **继续在 `SettingsShell` 内联计算**：改动最小，但平台组合规则仍与渲染代码耦合，后续抽取会重复搬运并缺少组合测试。
- **把整个 `SettingsShell` 搬到新文件**：能一次减少入口文件长度，但会同时移动导航、图库、表单和宿主回调，扩大冲突面；本切片只抽取无副作用的展示判定。
- **按平台分别写多个 helper**：每个平台更短，但标题栏、搜索位置和移动外壳的组合关系会分散，新增平台时更容易漏改。

## Consequences

平台展示判定有单独的纯函数和组合测试，入口文件不再重复声明这些布尔值。返回行为和页面动画仍留在 `SettingsShell`，所以历史记录、窗口交互和表单挂载边界不变。helper 需要接收可用页面与移动端无标题页列表，调用方仍拥有这些页面模型。
