# Agent Note: Windows 本机一键编译安装

Status: implemented

## Problem

`installer/test.ps1` 和 `test-light.ps1` 原来每次都要手写依赖前缀、第三方声明目录、CMake 生成器和版本号，还必须在 VS 开发者终端里运行；少一样就在编到一半时失败。在开发机上实际跑时依次碰到：

- 用 `setx` 设了依赖前缀，但已经开着的终端（包括 Windows Terminal 新开的标签页）看不到，仍报缺少 `-X64Dependencies`。
- 普通终端里 cargo 跑了八分钟后，`Build-Client.ps1` 生成宿主 DLL 导入库时找不到 `lib`。
- 全新机器上完整包缺 `target/desktop-resources`。
- 只改了 TSF / Server / WinUI 设置时，入口不透传 `-RustOutputs`，每次都要把 Rust 那一半重编一遍。

改完代码想快速装上核对功能，这些都挡在前面。

## Decision

两个入口都改成 `Invoke-LocalInstall.ps1` 的薄包装，参数全部可选、原样透传：

- 依赖前缀取 `MSIME_WINDOWS_X64_DEPS` / `MSIME_WINDOWS_X86_DEPS`，进程环境没有时再读用户和系统环境变量；都没有就在编译前报错，并提示 `setx` 的写法。
- 第三方声明目录默认 `target/windows-notices`；版本仍取 `platforms/windows/version.txt`。
- CMake 生成器沿用 `target/windows-full/x64/CMakeCache.txt` 里已有的那个（不同的生成器会让 configure 失败），没有缓存时按 vswhere 找到的 Visual Studio 主版本选 2026 或 2022。
- 找不到 `lib.exe` 时用 vswhere 找到的 Visual Studio 执行 `Enter-VsDevShell`，只改本次会话的环境变量。
- 完整包缺桌面资源时，照发布工作流用 `install_resources` 下载一次；暂存阶段仍按锁文件逐个校验。
- 新增 `-NoInstall`（只打出签好名的安装包）和 `-RustOutputs`（转成绝对路径交给 `Build-Client.ps1`）。

`tests/installer_entry.ps1` 覆盖环境变量默认值、持久化变量回退、`-NoInstall` 停在签安装包、`-RustOutputs` 透传以及生成器总有值。`AGENTS.md` 的 Worktree 一节原来写着本机入口不透传 `-RustOutputs`，同步改掉。

## Alternatives considered

- **要求在 VS 开发者终端里运行。** 这正是开发机上实际踩到的坑：cargo 跑完八分钟才报错，代价太大。
- **固定写死 Visual Studio 2022 生成器。** 装的是 VS 2026、且已有 2026 生成器缓存的机器上 configure 直接失败。
- **在入口里自动删掉 `target/windows-full/*/bin` 中与 vcpkg 依赖不同的旧运行时 DLL。** `Copy-RuntimeDependencies.ps1` 拒绝覆盖是有意的保护，静默删除会掩盖依赖前缀指错的情况；先保留报错，由人确认后删除。

## Consequences

开发机设一次两个环境变量后，`.\platforms\windows\installer\test.ps1` 或 `test-light.ps1` 不带参数就能从编译走到安装。`-RustOutputs` 只在 Rust 侧没有改动、产物与当前源码同源时使用，否则 Server、TSF 与宿主 DLL 的协议对不上；这一点写在 `LocalInstall.md` 和入口头注释里。
