# Agent Note: 设置导航页归属投影

Status: implemented

## Problem

`packages/ui/src/index.tsx` 同时负责调用设置导航 hook 和计算 `navigationPage`。这让移动根标签页、移动子页以及桌面合并页的高亮规则留在设置页总入口，后续导航行为抽取仍要维护两处边界。

## Decision

`useSettingsNavigation` 现在返回 `navigationPage`，并在同一处根据 `mobilePlatform`、`subPageParents` 和 `desktopPage` 计算页面归属：移动根标签页保持自身，移动子页使用父页，桌面子页再应用桌面合并页映射。设置页总入口只消费 hook 返回的投影，页面归属规则与导航栈行为保持同一边界。

## Alternatives considered

- **继续在 `index.tsx` 内联计算**：改动最少，但导航 hook 的调用者必须同时知道页面注册表的父子关系和桌面合并规则，容易在新增导航入口时产生漂移。
- **抽成独立纯函数**：测试归属映射很直接，但调用者仍需在 hook 外拼接页面状态；本切片的目标是让导航相关派生值和导航行为一起返回，因此没有再增加一个仅供单处调用的模块。

## Consequences

导航 hook 的返回值增加 `navigationPage`，设置外壳和页面模型无需重复导入注册表映射；新增子页或桌面合并页时，导航行为和高亮投影共享同一套注册表。hook 也因此依赖 `desktopPage` 与 `subPageParents`，但这两个映射本来就是设置导航的现有契约。
