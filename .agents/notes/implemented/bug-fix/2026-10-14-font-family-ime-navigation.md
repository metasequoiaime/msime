# Agent Note: 字体下拉导航让组合输入先处理按键

Status: implemented

## Problem

共享候选设置的 `FontFamilyInput` 在处理方向键、回车和 Esc 前没有判断组合输入。用中文输入法填写字体名时，这些本应浏览、确认或取消组词的按键会改变字体高亮、选中字体或关闭菜单，同时 `preventDefault()` 阻止输入法默认处理。八个合成事件用例在旧实现上都复现了事件被取消。

## Decision

在字体输入框的键盘事件入口检查 `nativeEvent.isComposing` 和 `keyCode === 229`，组合事件直接返回，不取消默认行为、不改变菜单和字体选择。组词结束后的普通方向键、回车和 Esc 继续操作字体菜单。

## Alternatives considered

仅保护回车可以避免提前选中字体，改动范围最小；但方向键和 Esc 仍会截获输入法的候选浏览和取消，所以在共用事件入口保护全部组合按键。

关闭整个字体下拉菜单能避免同时展示两套候选；但会破坏已浏览的字体高亮，且需要额外管理组合开始和结束事件。保留菜单状态并把组合按键交给输入法，不引入新的状态机。组合结束边界的兼容判定依据 [MDN 的键盘事件说明](https://developer.mozilla.org/en-US/docs/Web/API/Element/keydown_event)。

## Testing

回归测试覆盖方向键、回车和 Esc 的 `isComposing` 与 `keyCode=229` 两种路径，事件默认行为、字体回调和菜单高亮均保持；随后普通按键继续导航、确认和关闭。两份相关字体测试共 11 项通过；前端类型检查与构建、改动文件 lint 与格式检查、笔记及 quick 门禁通过。quick 包含 Rust 工作区、Android 目标和 Apple 桥接编译；Linux 容器、Windows 交叉编译、HarmonyOS 原生打包因未配置工具链跳过。这是组件键盘事件验证，没有原生设备验收。

## Consequences

字体菜单显示期间输入法获得组合按键的优先处理权，用户必须先结束组词再操作字体菜单。

## 旧笔记归属

检索 `FontFamilyInput`、字体组合与下拉导航：Windows 原生候选字体笔记记录枚举和筛选规则，使用 WinUI 的 `AutoSuggestBox`，与共享 React 事件处理无关；macOS 候选设置标签笔记讨论入口、分区和预览，与组合事件无关；Android 符号目录笔记讨论符号字体资源，与此无关。本篇只约束共享字体输入框的按键入口。

[云词库回车查询](2026-10-14-cloud-dictionary-ime-submit.md) 部分重叠：共用组合标记的判断，但查询禁用状态与字体菜单导航各自属于对应控件。本篇约束字体菜单，不改变查询条件。
