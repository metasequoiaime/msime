# Agent Note: Windows 7 app-local CRT 转发依赖核验

Status: implemented

## Problem

暂存给 Win7 的 Windows SDK UCRT 包含一组只有转发导出的 `api-ms-win-crt-*.dll`。现有检查把零普通导入全部拒绝，无法完成这组 DLL 的审计；直接略过又无法发现相邻 `ucrtbase.dll` 缺失或目标符号错误。

## Decision

仅在普通导入为空、文件名属于 CRT 或 core API 桥接 DLL、调用方明确提供绝对依赖目录时解析转发导出。必须完整解析导出表，每项具名且模块在限定集合中；CRT 只允许相邻 `ucrtbase`，core 只允许 SDK 实际使用的 `kernel32`、`user32`、`advapi32`、`normaliz` 与 `ntdll`。符号区分大小写，解析数量与 PE 函数数相符。未知模块、序号转发、重复名与部分解析继续失败。

目标函数只沿产品实际引用的导出核对。SDK 的 core 桥同时导出 `CreateFile2` 等新版接口，未被产品或 CRT 引用时不构成 Win7 装载障碍；把整个导出表当作导入表会拒绝有效的 SDK 桥。调用方必须提供 `RequiredExports`，逐个核对入边、目标模块和具名导出，再将这些实际依赖交给现有兼容拒绝表。需要的函数缺失仍拒绝，不能以“可选导出”跳过真正的调用。

`Copy-Windows7RuntimeClosure.ps1` 从显式列出的程序入口收集实际依赖闭包，校验架构、同名文件摘要和全部导入，不整目录复制 SDK。已知延迟导入只由核心钩子处理，不从 SDK 带入新版系统模块。全部预检查通过才创建目标目录，复制后按实际目标再核对转发与序号，并生成摘要清单和供 Win7 装载测试读取的导入清单。失败删除本次半包，不覆盖既有产物。

`windows7-import-load <清单> <目录> <插件 DLL…>` 首先把静态 CRT 探针复制到独占空目录，对显式列出的 TSF 与核心 DLL 逐个启动全新进程，使用与注册器一致的 `LOAD_WITH_ALTERED_SEARCH_PATH` 装载；失败即中止，不先加载 UCRT 或其他产品 DLL。插件验收通过后才逐符号检查整包。普通 EXE 的语音和 CRT 依赖仍在应用本地布局中检查，不把插件目录规则错误套到仅由产品 EXE 使用的运行库。该区分针对微软明确的 Win7 UCRT 插件限制，不是容忍 TSF 装载失败。

`windows7-import-load` 使用静态 CRT，在两个架构下按实际包目录或系统目录加载每个普通依赖，核对具名导出及固定序号的地址相等。系统固定序号只列产品实际使用的 Winsock、OLE Automation、原生设置标签使用的三个 Common Controls 子类化函数、Direct2D 工厂与内存流，未知序号继续拒绝。

### 既有笔记审计

- [Win7 Rust 核心与延迟导入](../../implemented/feature/2026-10-23-windows7-rust-core-build.md)：部分重叠，既有规则处理普通和延迟导入，本篇补充无导入 CRT 桥接 DLL 的有限依赖校验。
- [完整 Windows 7 支持](../../implemented/feature/2026-10-23-windows7-support.md)：部分落地，全部依赖与真实系统装载继续按完整提案验收。

## Alternatives considered

- **将零导入的 CRT DLL 全部视为通过。** 可以立刻审完运行库目录，但把错误或缺失的转发目标一同隐藏，因此必须读取并核对实际导出。
- **把每个桥的全部导出视为产品导入。** 覆盖看似更严格，但 SDK 有意同时转发新旧系统接口，这会把从未被调用的新接口也判成必需。完整解析转发表，兼容检查沿真实引用闭包执行。
- **继续拒绝全部零导入 DLL。** 规则简单且默认失败，但 SDK 正式 UCRT 包也无法检查，因此只为已确定的 CRT 桥接形态增加严格验证，未知零导入 DLL 仍拒绝。

## Testing

`SetWindowSubclass`、`RemoveWindowSubclass` 与 `DefSubclassProc` 的 410、412、413 序号已在 Win7 SP1 的两个架构核对具名与序号地址相同；解析回归继续拒绝未使用的 411 与延迟序号导入。

合成解析回归覆盖精确符号、大小写、缺失依赖、未知模块、序号目标、部分解析、普通导入与未被实际引用的新导出。实际完整产品的 x64 与 x86 Release 依赖闭包均在 Win7 Ultimate SP1 x64 的交互桌面验证：x64 为 3491 个导入、69 个模块与 46 个包内 DLL；x86 为 3355 个导入、67 个模块与 44 个包内 DLL，具名导出、固定序号与全部 DLL 实际装载全部通过。WOW64 不替代 32 位 Win7 系统。

检查实际发现并阻止了 TSF 的每屏 DPI 转换、Server 的 UI Automation 清理与 WinHTTP WebSocket 静态导入。相关代码修复后重建完整产品再通过，不能用局部手工测试目录的结果代替完整产品。

SDK 10.0.22621.5040 的 x64 `api-ms-win-crt-private-l1-1-0.dll` 转发 `_o_fabsf`，而相邻 `ucrtbase.dll` 没有实现；它不在实际完整产品的依赖图中，因此不打包。没有修改 SDK、伪造缺失函数或放宽真实入边校验。

## Consequences

运行库从真实引用收集并留有 SHA-256 清单，既拒绝不存在的目标，也不会把未使用的新 API 当成必需。`dumpbin` 文本格式变化时检查明确失败，不能因为漏解析而缩小检查范围。安装签名、TLS 连接、真实语音模型与输入法交互仍分别验收；导出与 DLL 装载通过不等于这些功能都已完成。
