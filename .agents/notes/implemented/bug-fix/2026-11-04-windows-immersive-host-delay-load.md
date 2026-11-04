# Agent Note: Windows 沉浸式进程加载 TSF 宿主库

Status: implemented

## Problem

Windows 11 26H2 的开始菜单、搜索和设置进程能够加载 `MetasequoiaImeTsf.dll`，但不会把 TSF 所在的 IME 目录加入普通 DLL 搜索路径。`MetasequoiaImeTsf.dll` 对 `msime_host_api*.dll` 的静态依赖因此解析失败，TSF 初始化后立即卸载；Explorer 等已有其它加载路径的进程不受影响。#10629 的 Procmon 记录确认了主 DLL 加载成功、宿主库查找全部 `NAME NOT FOUND`。

## Decision

把 TSF 对版本宿主库的导入改为 MSVC 延迟加载，并在延迟加载通知中用当前 TSF 模块的绝对路径拼出同目录宿主库路径，再以 `LOAD_WITH_ALTERED_SEARCH_PATH` 加载。版本表提供宿主库文件名，所有版本和 ARM64X 原生半边沿用各自的名称；没有路径时不回退到当前目录或 `PATH`。路径拼接单独提供可测试函数。

## Alternatives considered

- **把宿主库复制到 `System32` 或加入全局 `PATH`**：污染系统范围并扩大 DLL 劫持面，也不能解决多版本并装的命名隔离，因此不采用。
- **静态链接宿主库**：会显著增大 TSF，破坏现有共享宿主生命周期和版本隔离；同时需要重做构建与安装布局，因此不采用。
- **在 `DllMain` 中预加载宿主库**：静态导入在进入 `DllMain` 前已经解析，无法修复当前失败路径；延迟加载通知才位于正确的解析时机。

## Consequences

沉浸式进程可从 TSF 安装目录解析宿主库，默认搜索路径不再决定产品行为。新增路径策略测试；Windows 11 26H2 开始菜单、搜索和设置的真机验收仍需在 PR 上由 Windows 环境完成。
