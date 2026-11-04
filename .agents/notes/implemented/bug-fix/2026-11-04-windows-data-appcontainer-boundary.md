# Agent Note: Windows 用户数据目录不向全部应用包开放

Status: implemented

## Problem

#10827 的 DLL 查找修复之外，还递归给两个应用包组授予整个用户数据目录的读取、执行权限。报告者的对照实验只能证明运行时选项读取有权限缺口，不能证明该目录内全部用户状态都应向这些组开放。

原方案见[沉浸式进程读取用户数据目录](../../rejected/bug-fix/2026-11-04-windows-immersive-data-acl.md)。本篇替代其中的整目录授权决定。

## Decision

保留 TSF 同目录加载宿主库的修复，安装器停止给 `S-1-15-2-1` 和 `S-1-15-2-2` 授予整个数据目录读取权限。安装时递归移除这两个组的显式授权，覆盖曾安装相关预览包的目录；桌面用户的修改权限和中完整性标签沿用原有逻辑。

安装冒烟改为检查数据目录及合成子文件没有这两个组的允许项，生命周期测试检查清理命令。AppContainer 所需的读取接口仍需要独立设计最小数据边界；当前不新增文件格式，也不宣称解决设置搜索读取问题。

## Alternatives considered

- **继续递归授权**：不需要更改宿主读取契约，报告者的实验也支持其功能效果；但授权覆盖整个用户状态目录，超过已证实所需范围，因此撤回。
- **只删掉新增授权命令**：对从未安装相关预览包的用户足够，回退实现也最小；已经落盘的 ACL 不会随代码删除而消失，因此安装时还要清理已有显式授权。
- **只开放现有运行时选项文件**：范围小于整个目录；该文件仍承载完整偏好，不能直接视为可公开数据，需要另行确定脱敏契约。

## Consequences

安装器恢复用户数据的原有应用包读取边界。AppContainer 的读取缺口和开始菜单组合异常继续在 #10629 跟踪。没有改出厂默认值、用户数据布局或 DLL 查找策略。

## Verification

- 安装器权限命令审计：原实现触发授权范围断言失败，修正后通过；保留桌面用户修改权限和完整性命令。
- `pwsh -NoLogo -NoProfile -File platforms/windows/installer/tests/lifecycle.ps1` 通过。
- PowerShell 解析器检查 `install-smoke.ps1` 通过，`git diff --check` 和笔记校验通过。
- Windows 安装冒烟需要实际 Windows 安装包，本机没有执行；新增用例覆盖初装、旧预览授权的重装清理，以及迁移后的目录与子文件。完整本地 quick 待复用现有缓存后运行。

官方命令语义见 [Microsoft icacls 文档](https://learn.microsoft.com/en-us/windows-server/administration/windows-commands/icacls)：`/remove:g` 移除指定 SID 的允许项，`/T` 递归处理目录内容；不使用重置整份 ACL 的 `/reset`。
