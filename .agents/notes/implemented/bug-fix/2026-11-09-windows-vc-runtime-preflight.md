# Agent Note: Windows 安装前拒绝过旧 VC 运行库

Status: implemented

## Problem

#11126 在 Windows Server 2022 上复现了输入法切换导致 `explorer.exe` 和 `Taskmgr.exe` 退出。事件记录显示故障涉及 `MSVCP140.dll`，系统运行库为 14.29.30133.0；安装 Microsoft Visual C++ 最新 x64 运行库后变为 14.51.36247.0，同一操作不再崩溃。

安装器原先只要求注册表中的 VC 运行库主次版本至少为 14.20。这个门槛会接受报告现场的 14.29，安装成功后仍把不兼容的 Host API 放进 Explorer 等宿主进程。

## Decision

安装器直接用 Inno Setup 的 `GetVersionNumbers` 读取原生 `System32` 中的 `msvcp140.dll`、`vcruntime140.dll` 和 `vcruntime140_1.dll`。任一缺失、无法读取或版本低于 **14.51.36244.0** 都阻止安装，不启动 PowerShell，也不依赖可能残留的注册表登记。Inno 6 的内置文件函数遵守 64 位安装模式；Inno 7 使用 `ApplyPathRedirRulesForCurrentProcess(True, ...)` 选择原生系统目录，覆盖 32 位和 64 位 Setup。日志读取同一组文件。

支持下限取实际 CRT 版本，不由 Visual Studio 名称或 MSVC 工具集目录推断。[报告 nightly 的构建日志](https://github.com/metasequoiaime/msime/actions/runs/42416145111/job/128538407793) 的工具集目录为 `14.51.36231`，但 [Windows 原生检查](https://github.com/metasequoiaime/msime/actions/runs/42805626142) 读取 `crtversion.h` 得到 **14.51.36244.0**，并正确拒绝了较低的目录候选值。CI 每次核对声明下限不得低于实际 CRT。报告者确认通过的运行库为 `14.51.36247.0`；没有逐一验证历史 redist，不能把支持下限称为最低不崩溃版本。

VC 缺失或过旧时交互安装显示阻止原因并打开官方下载页，静默安装直接失败。WebView2 保持可选继续策略。包管理器说明和 Chocolatey 依赖使用同一完整版本下限。检查放在 `VcRuntimePrerequisite.iss`，完整安装器只接入它。

## Verification

`vc-runtime-native.ps1` 直接编译包含生产 Pascal 逻辑的原生探针，覆盖 Inno 6 x86 和 Inno 7 x86/x64。合成 DLL 的版本包括报告现场 14.29、14.50、下限之前 `14.51.36243.65535`、精确下限、报告者修复版本及未来 minor；同时覆盖文件缺失和无版本资源。探针读取原生 `kernel32.dll` 的摘要，与外部 64 位进程的结果核对，防止误读 SysWOW64。它在安装前退出，不注册产品、不替换系统运行库。

夹具由系统 .NET Framework `csc.exe` 生成真正的文件版本资源；PowerShell `Add-Type` 的程序集属性不能替代原生版本资源。失败输出保留探针结果、退出码、实际文件版本和安装器日志，不删减边界断言。

[最终 Windows 检查](https://github.com/metasequoiaime/msime/actions/runs/42807616043) 在 `2580582c20` 上通过三组版本边界、原生系统目录及实际 CRT 下限检查。2026-11-16 在 pve9 的 Windows 11 交互桌面复跑相同三组原生探针通过；本机工具链 CRT 为 `14.44.35220.0`，仅核验当前声明不低于它，发布下限仍以上述 CI 的 `14.51.36244.0` 为依据。

交互提示对照从改前／改后完整安装器原样提取 `InitializeSetup`，仅注入“VC 缺失、WebView2 已安装”的依赖状态，禁用下载与后续安装，在 Windows 桌面运行原生 Inno 对话框。旧函数点“否”返回允许，修复函数点“确定”返回阻止，截图保留真实窗口大小和底色。此证据不等同于完整产品安装，也没有在 Windows Server 2022 上重装两代系统运行库或重现 Explorer 崩溃；这些报告者环境证据不能由探针替代。

## Alternatives considered

- **继续接受 14.20，只建议更新**：保留已有安装兼容范围，但会放行报告现场的 14.29，不能阻止安装后崩溃，因此不采用。
- **随安装包私有分发 VC 运行库**：可以绕过系统版本，但会扩大载荷、签名和更新责任，也改变现有系统级依赖约定；本次使用已有安装前检查链。
- **PowerShell 读文件，注册表兜底**：跨架构访问方便，但报告者的 HIPS 会阻止安装过程启动 PowerShell，注册表也不能证明实际 DLL 可用，因此使用 Inno 内置读取。

## Consequences

- 过旧运行库在注册输入法之前被拒绝，用户更新后再安装；静默部署会得到非零退出码。
- 不代替用户安装系统组件；WebView2 缺失时仍可按原策略继续。
- 工具链升级必须同步核对门槛，CI 的 CRT 一致性检查会阻止过低门槛；没有自动生成下限的机制。
- 探针与合成提示验收不代替报告者的完整安装和真实编辑器复验，PR 必须分别说明证据范围。
