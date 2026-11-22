# 本机一键编译安装（完整包，含词库）：编译 Rust 与原生组件 → 收集安装文件 → 本机自签名 →
# Inno Setup 打包 → 签安装包 → 启动安装程序。只构建不安装用 build-only 的 Build-Client.ps1，或加 -NoInstall。
#
# 第一次用之前设好依赖前缀（之后不带参数直接运行）：
#   setx MSIME_WINDOWS_X64_DEPS C:\deps\x64-windows
#   setx MSIME_WINDOWS_X86_DEPS C:\deps\x86-windows
# 只改了 TSF / Server / 设置、本机已有完整安装时，用 .\test-light.ps1。
# 实现与各参数的默认值见 Invoke-LocalInstall.ps1。
[CmdletBinding()]
param(
    [string]$X64Dependencies,
    [string]$X86Dependencies,
    [string]$NoticesDirectory,
    [string]$DesktopResourcesDirectory,
    [string]$TargetVersion,
    [string]$RepoRoot,
    [string]$Generator,
    [string]$RustOutputs,
    [switch]$NoInstall
)
& (Join-Path $PSScriptRoot 'Invoke-LocalInstall.ps1') @PSBoundParameters
