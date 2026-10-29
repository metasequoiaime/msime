# Agent Note: 安卓剪贴板搜索框固定

Status: implemented

## Problem

剪贴板历史搜索页把 `SearchPill` 放在 `NestedScrollView` 的内容列中。用户向下浏览记录时，搜索框会随结果一起滚走，返回顶部后才能继续改查询。

## Decision

`ClipboardSearchPage` 将搜索框放入 `DetailPage.fixedNavigation()` 返回的固定导航区域，结果卡片继续留在详情页滚动列中。查询文字仍由 `Query` `ViewModel` 保存，重建页面时恢复文本；查询变化只重绘结果卡片，不重建固定搜索框。

## Alternatives considered

直接在结果列表滚动回调中复制或吸顶搜索框：需要额外维护滚动位置和遮挡关系，且容易与详情页已有固定导航和 inset 逻辑冲突，因此复用 `DetailPage` 的固定导航契约。

## Consequences

搜索框在滚动历史记录时持续可见，修改查询不必返回顶部；详情页滚动区会自动让出固定导航的高度。固定导航仍占用顶部空间。进页不再自动聚焦搜索框、弹键盘，见 [安卓剪贴板历史页进页不自动弹键盘](2026-10-29-android-clipboard-search-no-autofocus.md)。宿主检查新增契约，防止搜索框重新放回滚动内容列。
