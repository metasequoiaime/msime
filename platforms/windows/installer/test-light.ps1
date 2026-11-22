# 轻量本机一键编译安装：流程与 test.ps1 相同，安装包不带词库和桌面资源。
#
# 适用：只改了 TSF、Server 或设置窗口，本机已经装过完整包（词库已在数据目录）。不要用它做首次安装。
# 依赖前缀的设法、-NoInstall、-RustOutputs 见 test.ps1 和 Invoke-LocalInstall.ps1。
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
& (Join-Path $PSScriptRoot 'Invoke-LocalInstall.ps1') @PSBoundParameters -Light
