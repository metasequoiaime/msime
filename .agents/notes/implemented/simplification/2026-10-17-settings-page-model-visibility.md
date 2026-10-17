# Agent Note: 设置页模型平台可见性判定

Status: implemented

## Problem

`useSettingsPageModel` 同时计算辅助码入口、维护与诊断页是否存在、手机根标签页是否需要大标题，以及手机侧栏隐藏页列表。这些判定只依赖宿主能力和平台，却和偏好读取、页面投影及导航状态混在入口 hook 中；新增平台或调整页面入口时容易只改一处。

## Decision

`packages/ui/src/settings/settings-page-model-visibility.ts` 提供纯函数 `settingsPageModelVisibility`，接收平台、宿主能力和辅助码方案能力，返回 `showHelpcode`、`showDeveloperPage` 和 `untitledOnPhone`。同文件导出只读的 `mobileHiddenPageIds`，继续由 `settingsPageProjections` 和 `useSettingsNavigation` 共用。桌面平台维护能力仍以 `settingsPageEnvironment` 计算出的平台布尔值传入，避免未知宿主平台被 `settingsPlatform` 的 Windows 回退值误判为 Windows。

## Alternatives considered

- **继续在 `useSettingsPageModel` 内联计算**：改动小，但页面可见性规则继续和异步状态、偏好模型耦合，平台组合没有独立测试。
- **只抽出 `mobileHiddenPageIds` 常量**：能减少一段重复来源，却把辅助码、维护页和手机标题规则留在入口，新增平台仍需要穿过大 hook。
- **让 helper 自己从 `settingsPlatform` 推导桌面平台**：参数更少，但未知宿主会回退成 Windows 外观，而现有维护页判定只在明确的 Linux、Windows、macOS 宿主上显示；因此保留 `settingsPageEnvironment` 的三个布尔能力作为输入。

## Consequences

设置页模型只组装平台可见性结果，辅助码、维护页和手机标题判定有独立组合测试。`mobileHiddenPageIds` 的内容与导航过滤保持不变，快捷键页面的既有决定见[手机上不提供外接键盘快捷键页](./2026-10-11-mobile-no-shortcuts-page.md)。helper 不读取客户端、不注册 effect，页面路由和偏好生命周期仍由原 hook 持有。
