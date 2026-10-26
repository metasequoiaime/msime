# Agent Note: Windows 7 安装包发布构建 CI

Status: implemented

## Problem

仓库原有的 Native Platform CI 只编译 Win7 设置、面板和兼容核心，`Release Windows` 只覆盖现代 Windows 安装包。修复 Win7 原生设置后，没有自动流程把同一提交构建成 Win7 x64/x86 安装包，也没有在构建机上做安装器布局冒烟，导致 PR 只能依赖手工打包才能把修复装进 Win7 验收机。

## Decision

新增 `.github/workflows/release-windows7.yml`。它只支持手动选择 Windows edition 和 dispatch，不在每个 pull request 上自动运行昂贵的发布打包。工作流先在 Ubuntu runner 上构建并校验官方插件包，再让 `windows-2022` 上的 x64、x86 两个运行库 job 并行构建 Win7 核心与语音运行库；两个架构都成功后，打包 job 安装 Rust 和 vcpkg 的 Win7 静态依赖，下载锁定的资源和词书，收集第三方声明，强制把同一份官方插件包及两个架构运行库放入暂存目录，分别准备并编译 x64 与 x86 安装包，使用本机测试证书签名后运行 x64 安装器冒烟，最后上传两个安装包、摘要和声明文件。选择 `windows-2022` 是因为 Win7 专用构建脚本固定使用 `Visual Studio 17 2022` 生成器；`windows-2025` 当前镜像不提供该实例。PR 的普通 Windows 验证继续由现有 CI 负责。

工作流只上传 CI 产物，不创建 GitHub Release，也不把本机测试证书当作正式发布签名。`windows7-install-smoke.ps1` 在 Windows runner 上验证安装、注册、升级、数据目录迁移和卸载；它不能替代 Win7 SP1 实机的系统 API、输入法上屏和音频验收，VM100 仍由产品验收流程执行。

运行库矩阵 job 使用按架构、Windows runner 和工具链输入加锁的 Actions cache，保存 Rust target、ONNX Runtime/Sherpa-ONNX 源码及 CMake 构建目录。缓存放在 runtime job 内，因为矩阵 job 使用彼此独立的 runner；打包 job 的 vcpkg cache 不能被前面的运行库 job 复用。

本决定与[Win7 语音页修复](../bug-fix/2026-10-25-windows7-voice-page-loading.md)相接：语音页修复的安装验收依赖这条独立包流程；也与[Windows 原生设置窗口](../feature/2026-10-11-windows-settings-voice-page.md)保持宿主归属不变。现代 Windows 的 `Release Windows` 不改为复用这条流程，因为两者的 Rust、宿主和安装器输入不同。

## Alternatives considered

- **继续只跑 Native Platform CI**：成本较低，但它不生成可安装的 Win7 Server、设置、面板和 TSF 闭包，无法验证安装器实际拿到的文件，因此不能覆盖本次实机问题。
- **把 Win7 矩阵并入现代 `Release Windows`**：可以复用版本和发布逻辑，但现代流程的 `Build-Client.ps1`、WinUI/Tauri 产物和三架构矩阵与 Win7 专用核心、语音运行库、x64/x86 安装器不同，混在一起会让普通 Windows 发布依赖 Win7 工具链和更长的构建时间。
- **在同一个 runner 内顺序构建两个架构**：实现简单且不需要传递中间产物，但 x64 和 x86 的 Rust、ONNX Runtime、Sherpa-ONNX 编译互相等待，墙钟时间由两个架构相加决定。改用架构矩阵后各 runner 独立构建并上传运行库，打包 job 在复制前检查两个架构的目录，保留相同的产物和安装验收边界。
- **只在手工脚本中打包，不进 CI**：能临时生成 VM 安装包，但无法在 PR 上阻止安装器参数、运行库闭包或版本命名回归。

## Consequences

Win7 相关手动运行会获得可下载的 x64/x86 安装包和安装冒烟证据。两个架构各占一个 Windows runner，runner 分钟数增加，但架构编译不再串行等待，墙钟时间由较慢的单个架构决定；每个运行库 artifact 在汇总前都必须存在。依赖和语音运行库仍需要较大的磁盘空间。产物由本机测试证书签名，只用于 CI 内部验证，正式发布仍需受信任签名和单独的 Win7 SP1 x64/x86 实机验收。

## Verification

- `actionlint .github/workflows/release-windows7.yml` 通过。
- `git diff --check` 通过。
- 运行库矩阵的每个 artifact 都包含 `windows7-core/<arch>` 和 `windows7-voice/<arch>/bin`；打包 job 在复制前检查两个目录，缺失即失败。语音目录保留 `bin` 层级，因为 `Build-Windows7Client.ps1` 从该位置读取运行库清单和 DLL。
- 运行库上传前只从 `windows7-core/<arch>` 和 `windows7-voice/<arch>/bin` 整理最终文件，ONNX Runtime 与 sherpa-onnx 的构建目录不会进入 artifact。
- runtime job 在构建前恢复、成功后保存按架构隔离的编译缓存；缓存键变化时只退回同架构同 runner 的旧缓存，CMake 和 Cargo 会自行重建受影响部分。
- 工作流的 Windows runner 执行结果由 GitHub Actions 负责；本机 macOS 不执行 Windows 原生构建。
- 2026-10-26 的首次重跑已证明 artifact 目录修复有效；随后导入审计发现 Windows SDK 的 `api-ms-win-core-kernel32-legacy-l1-1-1.dll` 名称含多个连字符，原正则未识别该 API 转发模块。审计正则现允许 API-set 名称中的连字符，并用合成转发测试覆盖该形态。
