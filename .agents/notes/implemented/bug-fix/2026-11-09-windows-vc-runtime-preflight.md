# Agent Note: Windows 安装前拒绝过旧 VC 运行库

Status: implemented

## Problem

#11126 在 Windows Server 2022 上复现了输入法切换导致 `explorer.exe` 和 `Taskmgr.exe` 退出。事件记录显示故障涉及 `MSVCP140.dll`，系统运行库为 14.29.30133.0；安装 Microsoft Visual C++ 最新 x64 运行库后变为 14.51.36247.0，同一操作不再崩溃。

安装器原先只要求注册表中的 VC 运行库主次版本至少为 14.20。这个门槛会接受报告现场的 14.29，安装成功后仍把不兼容的 Host API 放进 Explorer 等宿主进程。

## Decision

安装器把发布构建使用的 Visual C++ v14.50 作为 x64 VC 运行库最低 minor 版本。注册表检查仍同时读取 HKLM32 和 HKLM64，并要求 `Installed=1`；随后用原生 64 位 PowerShell 检查实际加载路径 `System32\\vcruntime140.dll` 的文件版本，避免注册表残留较新版本而实际 DLL 被旧文件覆盖时误放行。低于 v14.50 或缺失时记录“缺失或过旧”，显示阻止安装的错误对话框，交互模式打开微软最新运行库下载页，静默模式直接返回失败，不启动浏览器；无法启动 Windows PowerShell 的精简系统回退到注册表结果。

WebView2 仍沿用原来的可选继续策略，因为它不造成已确认的宿主崩溃；只有 VC 运行库走硬阻止。包管理器文档和 Chocolatey 依赖同步到 v14.50。最低版本依据当前发布工作流在 Visual Studio 2026/v14.50 工具链上构建，以及微软文档关于运行库版本不得低于构建工具版本的约束：[Visual C++ 生命周期 FAQ](https://learn.microsoft.com/en-us/lifecycle/faq/visual-c-faq)、[最新支持的 VC++ 可再发行组件](https://learn.microsoft.com/en-us/cpp/windows/latest-supported-vc-redist)。

## Alternatives considered

- **继续接受 14.20，只在提示中建议更新**：不能阻止 #11126 的 14.29 现场安装后继续崩溃，因此不采用。
- **把 VC 运行库随安装包私有分发**：可以绕过系统版本，但会扩大安装包、签名和更新责任，也改变现有系统级依赖约定；本次先采用已有安装前检查链。
- **只检查文件版本或只查一个注册表视图**：单独检查文件版本会受 Setup.exe 的 WOW64 重定向影响，单独查注册表会放过实际 DLL 被覆盖的机器；现在保留双视图注册表读取，并通过原生 PowerShell 读取真实 System32 文件。

## Consequences

- 旧版 VC 运行库机器会在安装前明确失败，用户先更新运行库后才能注册输入法，避免把崩溃带进 Explorer。
- 静默部署遇到过旧运行库会得到非零退出码；包管理器可据此停止并提示依赖问题。
- 当前仅在本机脚本层验证了 Inno Pascal 源码契约，尚未在 Windows Server 2022 实机用 14.29/14.51 两个运行库版本编译并运行安装器；该实机复验是合并前的剩余条件。

