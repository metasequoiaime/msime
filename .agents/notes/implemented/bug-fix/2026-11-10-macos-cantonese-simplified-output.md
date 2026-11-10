# Agent Note: macOS 粤拼简体输出宿主接入

Status: implemented

## Problem

共享 Engine 已支持 `cantonese_simplified_output`，移动端、Linux 和 Windows 已接入；macOS 仍把粤拼当作只读繁体方案。#10435 的 macOS 用户无法从菜单或悬浮工具栏切换粤拼的简体输出，且宿主没有把这个偏好传给当前会话。

本笔记承接 [粤拼和粤语双拼输出简体](../feature/2026-11-02-cantonese-simplified-output.md) 的 Engine 和跨平台约定，只记录 macOS 宿主接线。

## Decision

macOS 在 `MSIMEAppearancePreferences` 中增加本机镜像键 `MSIMEClientCantoneseSimplifiedOutput`，对应共享偏好 `cantonese_simplified_output`，默认关闭且不改变既有 `traditional_chinese_output` 默认值。共享偏好存在时由 macOS 读入并在本机写回；未打开过该设置时继续不向共享文档写入。

当当前视图方案为粤拼（普通粤拼和粤语双拼在 Engine 中都使用 Cantonese 方案编号）时，菜单和悬浮工具栏的字符集入口读取、写入这个专用偏好，标题显示为「简体输出」，切换后立即把共享偏好重新应用到当前会话。其他方案继续使用全局 `traditional_chinese_output` 和原有宿主转换路径。

候选文字、候选释义和上屏不在 macOS 再做繁转简：粤拼简体转换由 Engine 完成，避免重复转换或改变候选去重结果。macOS 只在粤拼下显示当前专用开关状态；普通方案仍沿用 `script_conversion` 的宿主繁简转换。

## Alternatives considered

- **让粤拼直接复用 `traditional_chinese_output`**：实现更小，但全局开关默认值会改变既有粤拼用户的输出行为，因此不采用。
- **在 macOS 宿主单独调用繁转简**：会与 Engine 的候选去重和上屏文字不一致，也会让多个宿主各自维护转换逻辑，因此不采用。
- **只改菜单，不刷新当前会话**：设置要等下一次会话重建才生效，当前输入中的候选和上屏行为会滞后，因此切换后立即重新应用共享偏好。

## Consequences

macOS 粤拼用户可以从现有菜单和悬浮工具栏切换简体输出，默认仍为繁体；切回全拼、双拼或五笔时全局简繁设置不受影响。新增设置只在用户实际打开过时写入共享文档，暂不加入账号同步或 MCP 偏好工具。macOS 真实编辑器验收仍需报告者在其环境复验。

## Verification

新增字符集策略测试和快捷菜单回归，覆盖粤拼与普通方案的状态、标题、读写和非法值；macOS CMake 编译及远程 quick 门禁负责编译检查。
