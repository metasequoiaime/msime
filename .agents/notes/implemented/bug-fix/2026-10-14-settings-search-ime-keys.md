# Agent Note: 设置搜索的快捷键先让输入法完成组词

Status: implemented

## Problem

共享设置外壳的搜索框把任何回车都用于打开首个筛选页，任何 Esc 都用于清空查询。用户还在中文组词时确认或取消候选，也会跳页或丢掉查询。触屏首页的搜索框还会对组词 Esc 调用 `preventDefault()` 和 `stopPropagation()`，阻断输入法的默认处理。

## Decision

桌面外壳与触屏首页的搜索键盘入口先检查 `nativeEvent.isComposing` 和 `keyCode === 229`，组合事件直接返回，保留当前查询和页面，不取消默认行为。普通回车仍打开首个匹配页，普通 Esc 仍清空搜索；搜索过滤与点击导航保持原有契约。

共享外壳的一个入口分别显示在 macOS 和 HarmonyOS 2-in-1 侧栏、Windows 的共享设置标题栏或侧栏。本次约束的是共享组件，不是 Windows 原生 WinUI 搜索实现。

## Alternatives considered

只保护回车能避免误跳页，改动最少；但 Esc 取消组词仍会清空查询，首页也仍会阻断默认处理，所以同一入口保护所有组合事件。

只判断 `isComposing` 可以不用兼容字段；但组合结束可能早于最后一个键盘事件，`isComposing` 已是 `false` 时仍需用 `keyCode=229` 识别，依据 [MDN 的键盘事件说明](https://developer.mozilla.org/en-US/docs/Web/API/Element/keydown_event)。沿用仓库其他输入框的判断，不新增组合事件状态机。

## Testing

14 个组件用例覆盖三个共享桌面布局的回车和 Esc，以及首页 Esc 的两类组合标记。旧实现均失败；修复后保留查询、当前页和默认行为，独立普通按键仍能跳页或清空。

本机运行搜索回归、首页与共享外壳三个测试文件，共 82 项通过；桌面前端类型检查与构建、改动文件 lint 和格式检查、`pnpm run verify-notes`、`git diff --check` 与 `bash scripts/verify-local.sh --quick` 均通过。quick 的 Rust 工作区、Android 目标和 Apple 桥接编译通过；Linux Docker、Windows MinGW 与 HarmonyOS 原生工具链未配置，相关阶段跳过。这些结果属于共享组件及编译验证，本次没有做原生宿主设备验收。

## Consequences

用户需先结束组词，再用一次独立快捷键跳页或清空。组合确认引发的正常输入变更仍会刷新搜索筛选结果。

## 旧笔记归属

检索搜索、侧栏、`TouchHomePage` 与事件函数后：手机快捷键页隐藏、Windows 社区资源、HarmonyOS 皮肤全屏笔记讨论导航项目和子视图，不持有搜索按键契约；试用键盘及首页对话跳转笔记讨论首页的试键盘入口，也与搜索事件无关。没有现成搜索事件笔记可就地更新。
