# Agent Note: 社区图库搜索与分页共用外观变体

Status: implemented

## Problem

鸿蒙社区搜索与加载更多控件定义在 `community-skins.tsx`，资源图库跨页面引用它们。桌面已有 `CommunitySearchForm` 和 `CommunityGalleryLoadMore`，输入长度限制、提交阻止默认行为，以及分页按钮/忙状态却又实现一次，只有外观不同。

## Decision

`community-search-form.tsx` 和 `community-gallery-load-more.tsx` 的公共控件接收可选 `look="harmony"`。缺省仍渲染原桌面样式；鸿蒙保持胶囊输入、搜索键提示、隐藏搜索按钮与原分页文字按钮样式。两种外观共用长度限制、事件和分页状态判断。

皮肤与资源图库直接引用公共控件，鸿蒙分支显式选择外观。原 `CommunityHarmonySearch`、`CommunityHarmonyLoadMore` 名称保留为公共模块内的薄包装，并从皮肤页继续兼容导出。包入口提供搜索控件与 props 类型，分页现有包出口保持。

## 旧笔记归属

- [社区皮肤库代次](../../implemented/simplification/2026-10-12-community-skin-library-generation.md)持有本机库读取与获取/使用操作，本次不修改状态或请求。
- [社区皮肤发布](../../implemented/simplification/2026-10-16-community-skin-publication.md)持有发布对话框与状态，本次不修改表单提交入口。
- [触屏社区融合](../../implemented/feature/2026-10-11-touch-community-merged-into-feature-pages.md)与[手机皮肤顶部分类](../../implemented/feature/2026-10-16-mobile-skin-page-top-tabs.md)持有社区入口和表单边界，本次保留页面结构与导航。
- 检索 `CommunityHarmonySearch`、`CommunityHarmonyLoadMore`、公共模块名、搜索表单和加载更多复用，未找到持有这些控件归属的既有笔记；其他平台功能笔记提到分页，约束各自原生图库，不属于本次 React 外观抽取。

## Alternatives considered

- 只把鸿蒙控件搬到新文件：能消除跨页引用，但继续维护两套长度、提交和分页判断，无法让共有逻辑只有一份。
- 给公共组件开放任意样式、是否画按钮和键盘提示等参数：适配最灵活，但把两套确定外观拆成多个可随意组合的选项；现有两个外观用单个 `look` 即可保持完整约束。

## Consequences

输入长度与提交只有一份实现，分页四种状态只由公共组件判断；页面只声明外观。旧名称保留一层兼容转发，新增的可选 `look` 是公开的 UI 契约，不改变宿主偏好或持久化格式。

新增测试覆盖两个外观的受控搜索、128 个字素上限、键盘提示、搜索按钮、阻止表单默认行为、零参数回调、分页四态、禁用点击与旧出口。源码契约保留原分页复用断言，并检查两个页面直接委托公共模块、旧出口继续转发。既有图库测试继续覆盖实际页面。

鸿蒙输入的键盘提示只在其外观出现；桌面分页通过 `undefined` 继续使用原按钮默认类名。样式、文案、页面结构和请求状态不变。
