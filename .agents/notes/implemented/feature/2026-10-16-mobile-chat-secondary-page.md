# Agent Note: 触屏 AI 对话改为二级页面并恢复稳定底部入口

Status: implemented

## Problem

小米 Android 14 上打开 AI 对话并弹出系统键盘时，底部主导航会被输入区域挤掉，用户看不到页面切换入口。这个行为由共享触屏设置壳和各原生宿主的一级聊天 Tab 共同造成。

## Decision

AI 对话在 Android、iOS 和共享触屏设置壳上从「AI 辅助」或「AI 润色与回复」页面的二级入口打开；「试用键盘」复用该二级页并自动聚焦。底部入口恢复为稳定设置入口：共享壳和 iOS 使用「设置 / 全部设置 / 统计 / 我的」，Android 保留已有「设置 / 社区 / 统计 / 我的」。聊天页保留返回栈，系统键盘弹出时底部导航继续显示。

## Alternatives considered

- 只改 Android 会让 iOS 和鸿蒙继续有同样的键盘遮挡问题。
- 输入时隐藏底部导航会让用户失去返回和切换入口。
- 三个平台都使用社区会在共享壳和 iOS 重复 Android 原生已有的社区根页。

## Consequences

用户从 AI 辅助页进入聊天时会看到明确的二级标题和返回按钮，输入框获得焦点后底部导航仍在。桌面端聊天入口保持不变。

## Verification

共享 UI 定向测试、桌面类型检查、Android 宿主 smoke、HarmonyOS 逻辑测试与 HAP 构建、iOS 模拟器测试覆盖导航入口和原生编译路径。HarmonyOS 的 UTF-8 编解码、组件快照和宿主上下文改用非弃用 API；剩余系统字体列表、系统输入法选择器和 ArkTS `Function may throw` 提示属于既有 API 兼容提示，未通过全局屏蔽掩盖。
