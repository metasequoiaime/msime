# Agent Note: 设置页路由状态 hook

Status: implemented

## Problem

`packages/ui/src/index.tsx` 的设置页模型同时维护移动端标签页导航栈、桌面合并页映射、账号登录返回页、社区目的地和内容滚动重置。这组状态与设置页外壳耦合在一个大 hook 里，后续页面逻辑抽取必须重复穿过同一段路由代码。

## Decision

设置页路由状态集中在 `packages/ui/src/settings/use-settings-page-routing.ts` 的 `useSettingsPageRouting`。它接收初始页面、移动端标志和首页能力，统一处理历史记录恢复、请求页面解析、桌面承载页映射、各移动标签页最近页面、账号登录返回页、社区目的地以及页面内容滚动重置。设置页模型继续消费同一组返回值，路由行为和状态字段保持不变。

## Alternatives considered

- **继续留在 `useSettingsPageModel`**：改动最少，但每次拆出设置页状态都要重新理解路由栈和历史恢复的隐含约束。
- **把整个设置页模型移到新文件**：边界更大，必须同时搬运所有页面业务状态和宿主能力组合；本切片先只移动独立的路由状态，降低冲突面。

## Consequences

路由状态现在有独立的公共 hook，可由后续设置页壳拆分复用；设置页主文件减少了历史恢复和导航栈的内联逻辑。hook 仍依赖设置页注册表和滚动重置 hook，路由字段的语义没有改变，新增入口仍应通过该 hook 返回的 setter 进入导航栈。
