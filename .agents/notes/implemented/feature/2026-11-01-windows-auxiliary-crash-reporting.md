# Agent Note: Windows Server 与辅助进程崩溃采集

Status: implemented

## Problem

Windows TSF 的进程外 WER 采集覆盖了输入法 DLL，但 Server、watchdog、prepare-host、WinUI 设置和 Win7 面板/设置仍可能在自身崩溃时没有现场记录。它们还使用不同的启动路径和运行时目录，不能直接复制 TSF 的初始化方式。

## Decision

Windows 辅助进程统一通过 `AuxiliaryCrashCapture.h` 根据版本遥测目录和 `runtime-options.json` 解析实际偏好目录，再绑定持续授权代次。Server 继续负责常驻刷新；watchdog、prepare-host、WinUI 设置、Win7 设置和面板在各自入口安装 native handlers，并在 terminate 路径写入最小现场。prepare-host 只在准备好独占状态目录并完成版本初始化后创建捕获器，避免在失败前写入错误默认授权。

CMake 将共享 `Telemetry.cpp`/`CrashRecorder.cpp` 链接到这些辅助目标，原生设置工程显式接收 nlohmann-json include 路径；Windows 原生门禁新增 prepare-host、watchdog、Win7 设置/面板和 crash recorder 的构建入口。TSF WER 的 helper 与发布文件由独立 Windows TSF 切片提供，本切片只消费它们，不重复修改 TSF 代码。

## Verification

本地完成 CMake/PowerShell 变更的差异检查和笔记门禁。x64、x86、Windows7 构建、installer 测试、辅助进程合成崩溃夹具和真实 Windows 安装验收在 PR CI/维护者 Windows 环境运行；本机没有 Windows 工具链，不启动交叉完整构建。

## Alternatives considered

- 只在 Server 进程采集：实现简单，但 watchdog、设置和 Win7 面板的崩溃仍不可见，因此不采用。
- 让每个辅助进程复制一份偏好/目录解析：短期直观，但版本目录、授权代次和重定向规则会漂移，因此集中到 `AuxiliaryCrashCapture.h`。
- 让 prepare-host 在进程入口一开始就安装捕获：能记录更早的错误，但此时状态目录和版本身份尚未准备好，会把错误写到错误目录或使用错误授权，因此不采用。

## Consequences

Windows 的非 TSF 进程进入同一崩溃记录和授权链路，符号与运行时构建保持同源。代价是辅助目标需要链接共享 native capture 和 JSON 依赖，Windows/Win7 构建门禁的依赖图扩大；真实安装、watchdog 重启和编辑器输入验收必须在 Windows runner 或维护者机器完成。
