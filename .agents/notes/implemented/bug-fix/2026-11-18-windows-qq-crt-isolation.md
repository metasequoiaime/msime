# Agent Note: Windows TSF 宿主静态 CRT 隔离

Status: implemented

## Problem

Windows TSF DLL 会被加载进 QQ、Notepad-- 和其他编辑器的进程。现代 x64 与 x86 TSF 目前链接 Cargo 构建的动态 CRT 宿主 DLL，宿主进程若先加载同名但版本较旧的运行库，可能让宿主 API 使用错误的 CRT 状态并触发第三方进程崩溃。#11598、#11593 和 #11542 报告了这组现象。Server、设置窗口和 TSF 共用一份宿主 DLL，x64 宿主还编入按键重排用的 `/MD` ONNX Runtime，不能把现有 Server 宿主直接改成静态 CRT。

## Decision

为 TSF 单独构建一份静态 CRT 宿主 DLL。x64 TSF 变体关闭 `onnx-rerank`，因为 TSF 会明确把 `keyboard_reranker` 设为关闭，Server 继续使用现有动态 CRT 宿主和 ONNX Runtime。x86 TSF 变体同样使用静态 CRT。TSF 的 CMake 目标通过 `MSIME_TSF_HOST_LIBRARY` 接收这份导入库，Server、设置窗口和原生面板继续使用 `MSIME_HOST_LIBRARY`。现代安装包把两份宿主 DLL 都放进 TSF 版本目录和 Server 目录，按版本名隔离并纳入符号、PE 和安装器测试。Arm64X 的 ARM64EC 半部也改为导入静态 CRT x64 TSF 宿主。

`Build-RustOutputs.ps1` 先保存调用方的 x64、x86 和 ARM64 Rust flags，构建普通宿主后用 `+crt-static` 构建 TSF 宿主并按 `_tip` 名称收集。`edition_windows.py host-def` 支持显式模块名，非 full edition 的静态变体因此得到正确的导入库名称。安装器和 SignPath 入口把普通宿主与 `_tip` 宿主作为同一版本的一组二进制处理。

## Alternatives considered

- **直接把 x64 Server 宿主改成静态 CRT。** 这会让预构建的 `/MD` ONNX Runtime 与宿主混用运行库，破坏当前按键重排构建契约，因此不采用。
- **继续依赖安装器安装 VC++ 运行库。** 宿主进程可能已加载自己的同名模块，系统运行库存在不能保证插件取得正确 CRT 状态，因此不能解决进程内冲突。
- **把 TSF 会话全部移到 Server IPC。** 能完全避开插件进程中的 Rust CRT，但会改变现有 TSF 与 Host API 的进程边界，超出本次缺陷修复范围。

## Testing

Python Windows contract checks、edition schema、release page、release signing 和 staged runtime 测试通过。Build-Client 的 PowerShell 命令探针已覆盖普通宿主、x64/x86 静态 CRT TSF 宿主、Arm64EC 导入、版本 edition 导入库、失败阶段和 Rust flags 恢复。Windows MSVC 构建、安装暂存、签名暂存和 QQ/Notepad-- 真实宿主复测需由 Windows CI 与报告者环境完成。

## Consequences

Server、设置窗口和原生面板仍可使用 x64 ONNX Runtime 逐键重排。TSF 额外产出两个架构的静态 CRT 宿主，构建时间、符号包和安装包二进制数量增加。TSF 通过 `_tip` 模块名加载自己的宿主，避免与编辑器进程里同名动态 CRT 复用。真实 QQ/Notepad-- 复测通过前，相关 issue 仍需保持开放。
