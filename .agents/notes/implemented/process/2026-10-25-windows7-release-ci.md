# Agent Note: Windows 7 安装包发布构建 CI

Status: implemented

## Problem

仓库原有的 Native Platform CI 只编译 Win7 设置、面板和兼容核心，`Release Windows` 只覆盖现代 Windows 安装包。修复 Win7 原生设置后，没有自动流程把同一提交构建成 Win7 x64/x86 安装包，也没有在构建机上做安装器布局冒烟，导致 PR 只能依赖手工打包才能把修复装进 Win7 验收机。

## Decision

新增 `.github/workflows/release-windows7.yml`。它在涉及 Win7 产品、共享运行时或安装器的 pull request 上运行，也支持手动选择 Windows edition。工作流先在 Ubuntu runner 上构建并校验官方插件包，再在 Windows runner 上固定执行以下顺序：安装 Rust 和 vcpkg 的 Win7 静态依赖，构建 x64/x86 兼容核心与语音运行库，构建完整 Win7 原生产品，下载锁定的资源和词书，收集第三方声明，强制把同一份官方插件包装入暂存目录，分别准备并编译 x64 与 x86 安装包，使用本机测试证书签名后运行 x64 安装器冒烟，最后上传两个安装包、摘要和声明文件。

工作流只上传 CI 产物，不创建 GitHub Release，也不把本机测试证书当作正式发布签名。`windows7-install-smoke.ps1` 在 Windows runner 上验证安装、注册、升级、数据目录迁移和卸载；它不能替代 Win7 SP1 实机的系统 API、输入法上屏和音频验收，VM100 仍由产品验收流程执行。

本决定与[Win7 语音页修复](../bug-fix/2026-10-25-windows7-voice-page-loading.md)相接：语音页修复的安装验收依赖这条独立包流程；也与[Windows 原生设置窗口](../feature/2026-10-11-windows-settings-voice-page.md)保持宿主归属不变。现代 Windows 的 `Release Windows` 不改为复用这条流程，因为两者的 Rust、宿主和安装器输入不同。

## Alternatives considered

- **继续只跑 Native Platform CI**：成本较低，但它不生成可安装的 Win7 Server、设置、面板和 TSF 闭包，无法验证安装器实际拿到的文件，因此不能覆盖本次实机问题。
- **把 Win7 矩阵并入现代 `Release Windows`**：可以复用版本和发布逻辑，但现代流程的 `Build-Client.ps1`、WinUI/Tauri 产物和三架构矩阵与 Win7 专用核心、语音运行库、x64/x86 安装器不同，混在一起会让普通 Windows 发布依赖 Win7 工具链和更长的构建时间。
- **只在手工脚本中打包，不进 CI**：能临时生成 VM 安装包，但无法在 PR 上阻止安装器参数、运行库闭包或版本命名回归。

## Consequences

Win7 相关 PR 会获得可下载的 x64/x86 安装包和安装冒烟证据；依赖和语音运行库使用缓存后仍需要较长的 Windows runner 时间，并消耗较大的磁盘空间。产物由本机测试证书签名，只用于 CI 内部验证，正式发布仍需受信任签名和单独的 Win7 SP1 x64/x86 实机验收。

## Verification

- `actionlint .github/workflows/release-windows7.yml` 通过。
- `git diff --check` 通过。
- 工作流的 Windows runner 执行结果由 GitHub Actions 负责；本机 macOS 不执行 Windows 原生构建。
