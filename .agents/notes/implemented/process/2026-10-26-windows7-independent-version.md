# Agent Note: Windows 7 独立版本源

Status: implemented

## Problem

Win7 安装包虽然使用 `windows7-v<版本>` 专用更新通道，但构建脚本、Win7 设置和 MCP 仍从 `platforms/windows/version.txt` 读取版本。这会把兼容系统的版本递增绑定到 Windows 10/11，无法独立发布 Win7 修复或按兼容包的节奏维护。

## Decision

Win7 使用 `platforms/windows/version-win7.txt` 作为独立版本源，现代 Windows 继续使用 `platforms/windows/version.txt`。Win7 核心通过 `MSIME_VERSION` 编译 MCP 版本，Win7 Server、设置和面板通过 `MSIME_WINDOWS_VERSION` 使用同一版本，Win7 发布 workflow 默认读取该文件；CMake 在 `MSIME_WINDOWS7_CLIENT` 下也把该文件列为配置依赖。初始文件值复制当前 `0.3.0`，不改变现有已验收包的显示版本；之后两条版本线可分别递增。Win7 包名和专用更新 tag 继续使用 `windows7-v<版本>`，现代包的命名与更新契约保持不变。

## Alternatives considered

- **继续共用 `platforms/windows/version.txt`**：改动最少，但每次现代 Windows 发版都会强制产生同号 Win7 构建，无法表达兼容版独立修复和维护节奏，因此不满足专用更新通道的边界。
- **给 Win7 使用完全不同的顶层平台目录**：版本边界清楚，但会重复 Windows 兼容构建脚本与资源路径，且现有更新平台名、安装器和 CMake 入口都在 `platforms/windows` 下；保留目录、拆出版本源可以隔离版本而不复制产品代码。
- **只在 workflow 输入里覆盖版本**：可以临时打包，但本地构建、关于页、MCP 握手和安装脚本仍会读普通 Windows 版本，容易生成内部版本不一致的包，因此默认值必须落在仓库内的独立文件。

## Consequences

Win7 和现代 Windows 可以独立升版，但版本发布流程必须分别维护两个文件；Win7 发布时仍须创建匹配的 `windows7-v<版本>` 专用发布，不能回退普通 `windows-v<版本>`。初始值与现有版本相同，因此本次不改变已生成包的版本显示。Win7 构建会多一个版本文件依赖，相关 CMake 配置和 MCP 构建必须保持同源读取。

## Verification

- `actionlint .github/workflows/release-windows7.yml` 通过。
- `git diff --check` 通过。
- 笔记树与格式校验通过。
- Windows 原生构建和 Win7 实机验收交由 GitHub Actions 与既有 Win7 验收流程执行；本机 macOS 不执行 Windows/MSVC 构建。
