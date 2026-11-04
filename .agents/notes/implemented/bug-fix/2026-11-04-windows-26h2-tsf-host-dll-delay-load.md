# Agent Note: Windows 26H2 系统进程加载 TSF 宿主 DLL

Status: implemented

## Problem

Windows 11 26H2 的开始菜单和搜索框加载 `MetasequoiaImeTsf.dll` 时，系统能找到 TSF 及其 Visual C++ 运行库，却不会把 TSF 所在安装目录用于普通导入 DLL 的搜索。TSF 对版本专属 `msime_host_api*.dll` 的静态导入因此失败；`Procmon` 中只有 `NAME NOT FOUND`，没有宿主 DLL 的 `Load Image`。桌面进程因为工作目录或其他搜索路径不同，不能代表系统进程的行为。

## Decision

MSVC 构建的 TSF 使用 delay-load 导入宿主 DLL。延迟导入钩子只接受版本表生成的 x64/ARM64 宿主 DLL 名称，并从 `Global::dllInstanceHandle` 得到 TSF 自身路径，用 `LoadLibraryExW` 和 `LOAD_WITH_ALTERED_SEARCH_PATH` 按完整路径加载宿主及其同目录依赖。加载失败时抛出模块缺失异常，避免回到不受控的当前目录或 `PATH` 搜索。

普通 x64/x86 TSF 和 Windows on Arm 的 ARM64X TSF 都使用这条规则。ARM64X 的 ARM64 半边导入带 `_arm64` 后缀的宿主，ARM64EC 半边导入 x64 宿主；两半各自编译同一钩子后合并成一个 DLL，链接阶段同时登记两种 delay-import。MinGW 构建保持原有普通导入，因为该工具链没有复用 MSVC delay-import 的链接契约。

版本表生成器同时输出宽窄两种宿主文件名宏，CMake 从同一份版本表取得 `/DELAYLOAD` 的模块名，避免生成头文件、导入库和链接选项各自维护一份名字。

## Alternatives considered

- **把宿主 DLL 复制到 `System32` 或修改系统 `PATH`**：最强的理由是能让旧的静态导入继续工作；不采用，因为会污染系统目录或全局进程环境，多个版本还会发生同名覆盖，安装器也无法安全回滚。
- **让 TSF 在加载时全量使用 `GetProcAddress` 重写宿主 ABI**：最强的理由是完全绕过导入表；不采用，因为要手工复制全部导出函数和调用约定，容易漏导出，且会把现有导入库契约变成另一套未验证的 ABI。
- **继续依赖普通导入并要求调用方设置 DLL 搜索路径**：最强的理由是代码改动最小；不采用，因为开始菜单、搜索框等系统宿主不受输入法控制，问题正是它们的搜索路径不包含安装目录。

## Consequences

TSF 在系统进程中的宿主解析不再依赖当前进程的 `PATH`、当前目录或包外搜索路径，多个 edition 仍按各自的 DLL 名称隔离。宿主 DLL 的同目录依赖也由 altered search path 解析。代价是 MSVC 链接增加 `delayimp.lib` 和每个 edition 的 `/DELAYLOAD`，ARM64X 的构建必须继续保持 ARM64/ARM64EC 两套宿主名字一致；MinGW 和 Windows 26H2 真机行为仍需各自验证。

已运行版本表生成一致性、CMake/静态检查、`git diff --check` 和笔记门禁。当前环境没有 Windows 26H2 系统进程，开始菜单/搜索框验收交给 Windows CI 和报告者；在该验收完成前，issue #10629 不能关闭。
