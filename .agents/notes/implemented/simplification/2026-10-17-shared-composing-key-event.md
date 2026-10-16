# Agent Note: 共享输入法组词按键判断

Status: implemented

## Problem

设置搜索、账号面板、字体选择、聊天、社区发布、词库编辑和共享面板重复检查 `nativeEvent.isComposing || keyCode === 229`。各入口已经有不同的导航和提交规则，但相同的输入法兼容判断分散，后续修改容易漏掉其中一处。

## Decision

在 `packages/ui/src/core/ime-key-event.ts` 提供并公开导出 `isComposingKeyEvent`，接受 React 键盘事件需要的最小结构，只返回组词标记或兼容键码是否命中。12 个模块的 15 处双条件入口复用这个谓词，快捷键、修饰键、默认行为和事件传播规则仍由调用方决定。

单独使用原生 `event.isComposing` 的语音取消监听具有不同的事件结构和判断范围，保留其现有条件。公共谓词不引入组合状态机，也不负责提交、关闭或阻止默认行为。

## Alternatives considered

- 各入口继续写两个条件：每处代码短、无需导入，但相同兼容规则仍需逐处维护；公共谓词可以保持入口规则显式，同时只维护一份判断。
- 抽取统一的回车或 Escape 处理器：可以连默认行为一起复用，但搜索、字体选择、发布、聊天和面板的按键与修饰键规则不同，统一处理器会把这些业务分支混入公共层。

## Verification

- 对照基线检查全部 React 双条件入口，公共谓词保持原有求值顺序，调用方保留原有外围条件。
- 复用已有的字体选择、体验键盘、首页搜索、聊天、社区发布、云词库、设置搜索、应用规则、词库编辑、账号面板、手写和表情测试；其中组词与普通快捷键用例直接检查是否提交、导航、关闭及阻止默认行为。
- 桌面构建、HarmonyOS 类型检查、格式、lint、笔记和 quick 门禁覆盖公共出口与宿主编译。

## Consequences

`keyCode` 已弃用，但组词首尾按键可能位于组合生命周期之外，因此继续保留兼容条件。依据 [MDN 的 IME 键盘事件说明](https://developer.mozilla.org/en-US/docs/Web/API/Element/keydown_event)。

15 处入口共享同一条兼容判断，公共层不依赖具体控件或业务状态；代价是各入口增加一次模块导入。现有业务回归验证抽取前后的行为，避免新增与布尔表达式逐项同构的测试。

## Existing notes audit

检索 `isComposing`、`keyCode`、组词按键及组合事件。以下既有笔记持有各业务入口的行为，属于部分重叠；本篇只记录共享谓词的归属，不取代它们：

- [词库编辑弹窗](../../implemented/bug-fix/2026-10-14-dictionary-dialog-ime-keys.md)
- [社区发布回车](../../implemented/bug-fix/2026-10-14-community-publish-ime-submit.md)
- [账号面板 Escape](../../implemented/bug-fix/2026-10-14-account-sheet-ime-escape.md)
- [设置搜索按键](../../implemented/bug-fix/2026-10-14-settings-search-ime-keys.md)
- [字体选择导航](../../implemented/bug-fix/2026-10-14-font-family-ime-navigation.md)
- [云词库回车](../../implemented/bug-fix/2026-10-14-cloud-dictionary-ime-submit.md)
