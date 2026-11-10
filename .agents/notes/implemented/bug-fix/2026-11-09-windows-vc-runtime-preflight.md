# Agent Note: Windows 安装前拒绝过旧 VC 运行库

Status: implemented

## Problem

#11126 在 Windows Server 2022 上复现了输入法切换导致 `explorer.exe` 和 `Taskmgr.exe` 退出。事件记录显示故障涉及 `MSVCP140.dll`，系统运行库为 14.29.30133.0；安装 Microsoft Visual C++ 最新 x64 运行库后变为 14.51.36247.0，同一操作不再崩溃。

安装器原先只要求注册表中的 VC 运行库主次版本至少为 14.20。这个门槛会接受报告现场的 14.29，安装成功后仍把不兼容的 Host API 放进 Explorer 等宿主进程。

## Decision

安装器直接用 Inno Setup 的 `GetVersionNumbers` 读取 `System32` 中的 `msvcp140.dll`、`vcruntime140.dll` 和 `vcruntime140_1.dll`。任一缺失、无法读取或版本低于 14.51.36231.0 都阻止安装，不再启动 PowerShell，也不再用可能残留、缺失的注册表登记作兜底。Inno 6 的内置文件函数遵守 64 位安装模式；Inno 7 使用 `ApplyPathRedirRulesForCurrentProcess(True, ...)` 选择原生系统目录，同时支持 32 位和 64 位 Setup。日志读取同一组文件版本。

最低支持版本的依据修正为报告者实际使用的 [nightly 构建 42416145111 / pinyin](https://github.com/metasequoiaime/msime/actions/runs/42416145111/job/128538407793)：日志明确给出工具集目录 `14.51.36231`，编译器识别为 `19.51.36260.0`。此前仅由 Visual Studio 2026 名称推断 v14.50 是错误的。按照[微软的运行库兼容性约定](https://learn.microsoft.com/en-us/lifecycle/faq/visual-c-faq)，运行库不得早于构建工具集。但工具集目录与 CRT 补丁版本可能不同，当前 14.51.36231.0 仍是候选门槛，不能仅由目录证明。Windows 原生探针会读取当前工具链 `crtversion.h` 的四段版本并拒绝低于它的门槛，必须取得这个结果后再确定下限。报告者实际验证的通过版本为 14.51.36247.0；未对每个历史 redist 做二分，不能将支持下限称作“最低不崩溃版本”。后续工具链升级时还需同步核对门槛，尚未加入自动生成工具链版本的机制。

WebView2 保持原来的可选继续策略。VC 缺失或过旧时交互安装显示阻止原因并打开下载页，静默安装直接失败；包管理器说明和 Chocolatey 依赖同步完整版本下限。新逻辑抽成 `VcRuntimePrerequisite.iss`，原安装器只保留接入。

## Alternatives considered

- **继续接受 14.20，只在提示中建议更新**：不能阻止 #11126 的 14.29 现场安装后继续崩溃，因此不采用。
- **把 VC 运行库随安装包私有分发**：可以绕过系统版本，但会扩大安装包、签名和更新责任，也改变现有系统级依赖约定；本次先采用已有安装前检查链。
- **PowerShell 读文件，注册表兜底**：跨架构访问方便，但报告者指出安装过程启动 PowerShell 会被 HIPS 拦截；注册表也不能证明实际 DLL 可用，因此改用 Inno 内置读取和显式系统目录规则。

## Consequences

- 旧版 VC 运行库机器会在安装前明确失败，用户先更新运行库后才能注册输入法，避免把崩溃带进 Explorer。
- 静默部署遇到过旧运行库会得到非零退出码；包管理器可据此停止并提示依赖问题。
- 当前仅在本机脚本层验证了 Inno Pascal 源码契约，尚未在 Windows Server 2022 实机用 14.29/14.51 两个运行库版本编译并运行安装器；该实机复验是合并前的剩余条件。



新增 Windows 原生探针直接包含产品同一份 Pascal 逻辑，用带固定文件版本的合成 DLL 验证 14.29、14.50、下限前一个版本、精确下限、报告者修复版本和未来 minor，以及缺失和无版本文件。Inno 6 的 32 位安装器、Inno 7 的 32/64 位安装器还需分别读取系统 `kernel32.dll`，与外部 64 位进程算出的 SHA-256 比较，证明检查的是原生 System32。探针在 `InitializeSetup` 返回 False，只读夹具和系统文件，不注册产品、不修改运行库。该测试接入 Windows release script tests；尚待 Windows 实际结果，不以源码契约通过代替。
