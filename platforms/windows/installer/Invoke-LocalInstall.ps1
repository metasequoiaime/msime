# Explicit local test-install workflow. Unlike Build-Client, this signs and
# launches an installer; automated tests must substitute every side-effect stage.
#
# 本机一键编译安装：test.ps1（完整包）和 test-light.ps1（轻量包）都是它的薄包装。
# 依赖前缀不传时取环境变量 MSIME_WINDOWS_X64_DEPS / MSIME_WINDOWS_X86_DEPS（用 setx 设一次即可），
# 第三方声明目录默认 target/windows-notices，CMake 生成器默认沿用已有构建缓存，没有缓存时按本机 Visual Studio 版本选。
# -NoInstall 只打包不启动安装程序；-RustOutputs 指向已编好的 Rust 产物时不再跑 cargo（只在 Rust 侧没改动时用）。
[CmdletBinding()]
param(
    [string]$X64Dependencies = $env:MSIME_WINDOWS_X64_DEPS,
    [string]$X86Dependencies = $env:MSIME_WINDOWS_X86_DEPS,
    [string]$NoticesDirectory = 'target/windows-notices',
    [string]$DesktopResourcesDirectory = 'target/desktop-resources',
    [string]$TargetVersion = (Get-Content -LiteralPath (Join-Path (Split-Path -Parent $PSScriptRoot) 'version.txt') -TotalCount 1).Trim(),
    [string]$RepoRoot = (Split-Path -Parent (Split-Path -Parent (Split-Path -Parent $PSScriptRoot))),
    [string]$Generator = '',
    [string]$RustOutputs = '',
    [switch]$Light,
    [switch]$NoInstall
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
function Invoke-InstallerStage {
    param([string]$Path, [hashtable]$Parameters)
    $global:LASTEXITCODE = 0
    & $Path @Parameters
    if ($global:LASTEXITCODE -ne 0) { throw "Installer stage failed ($global:LASTEXITCODE): $([IO.Path]::GetFileName($Path))" }
}
# 生成器必须与已有的 CMake 缓存一致，否则 configure 直接报错；没有缓存时取本机最新 Visual Studio 的主版本。
function Resolve-CMakeGenerator {
    param([string]$Root)
    $cache = Join-Path $Root 'target/windows-full/x64/CMakeCache.txt'
    if (Test-Path -LiteralPath $cache -PathType Leaf) {
        $line = Select-String -LiteralPath $cache -Pattern '^CMAKE_GENERATOR:INTERNAL=(.+)$' | Select-Object -First 1
        if ($line) { return $line.Matches[0].Groups[1].Value.Trim() }
    }
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    if (Test-Path -LiteralPath $vswhere -PathType Leaf) {
        $version = & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationVersion
        switch -Regex ("$version") {
            '^18\.' { return 'Visual Studio 18 2026' }
            '^17\.' { return 'Visual Studio 17 2022' }
        }
    }
    'Visual Studio 17 2022'
}
$RepoRoot = (Resolve-Path -LiteralPath $RepoRoot).Path
# setx 只写注册表，已经开着的终端（包括 Windows Terminal 新开的标签页，它们继承终端主进程启动时的环境）看不到；进程环境里没有时再读用户和系统环境变量。
function Get-PersistedVariable {
    param([string]$Name)
    foreach ($scope in 'User', 'Machine') {
        $value = [Environment]::GetEnvironmentVariable($Name, $scope)
        if ($value) { return $value }
    }
    ''
}
if (-not $X64Dependencies) { $X64Dependencies = Get-PersistedVariable 'MSIME_WINDOWS_X64_DEPS' }
if (-not $X86Dependencies) { $X86Dependencies = Get-PersistedVariable 'MSIME_WINDOWS_X86_DEPS' }
foreach ($dependency in @(@('X64Dependencies', $X64Dependencies, 'MSIME_WINDOWS_X64_DEPS'), @('X86Dependencies', $X86Dependencies, 'MSIME_WINDOWS_X86_DEPS'))) {
    if (-not $dependency[1]) {
        throw "缺少 -$($dependency[0])：传入 vcpkg 依赖前缀，或设置环境变量 $($dependency[2])（例如 setx $($dependency[2]) C:\deps\x64-windows）"
    }
}
# Build-Client.ps1 要用 MSVC 的 lib.exe 生成宿主 DLL 的导入库，它只在 VS 开发者环境的 PATH 上。普通终端里找不到时，用 vswhere 找到的 Visual Studio 进入 x64 开发者环境（只改本次 PowerShell 会话的环境变量，不切目录）；找不到 VS 就照旧交给 Build-Client 报错。
function Enter-MsvcEnvironment {
    if (Get-Command lib -CommandType Application -ErrorAction SilentlyContinue) { return }
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio\Installer\vswhere.exe'
    $installation = if (Test-Path -LiteralPath $vswhere -PathType Leaf) {
        & $vswhere -latest -products * -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    }
    $devShell = if ($installation) { Join-Path $installation 'Common7\Tools\Microsoft.VisualStudio.DevShell.dll' }
    if (-not $devShell -or -not (Test-Path -LiteralPath $devShell -PathType Leaf)) {
        Write-Warning '没有找到带 C++ 工具的 Visual Studio，无法自动进入 MSVC 开发者环境'
        return
    }
    Write-Host "进入 MSVC 开发者环境：$installation"
    # VsDevCmd 自己按 PATH 找 vswhere，找不到会打印一条无害的报错。
    $env:PATH = "$(Split-Path -Parent $vswhere);$env:PATH"
    Import-Module $devShell
    Enter-VsDevShell -VsInstallPath $installation -SkipAutomaticLocation -DevCmdArguments '-arch=x64 -host_arch=x64' | Out-Null
}
Enter-MsvcEnvironment
$noticeRoot = if ([IO.Path]::IsPathRooted($NoticesDirectory)) { $NoticesDirectory } else { Join-Path $RepoRoot $NoticesDirectory }
if (-not (Test-Path -LiteralPath (Join-Path $noticeRoot 'THIRD_PARTY_NOTICES.txt') -PathType Leaf)) {
    throw 'Supply the prepared third-party notice directory before local installation'
}
if (-not $Generator) { $Generator = Resolve-CMakeGenerator $RepoRoot }
# 完整包要锁定的桌面资源；本机还没有时照发布工作流的做法下载一次（Prepare-PackageFiles.ps1 仍按锁文件逐个校验长度和 SHA-256）。
$resourceRoot = if ([IO.Path]::IsPathRooted($DesktopResourcesDirectory)) { $DesktopResourcesDirectory } else { Join-Path $RepoRoot $DesktopResourcesDirectory }
if (-not $Light -and -not (Test-Path -LiteralPath $resourceRoot -PathType Container)) {
    Write-Host "本机没有桌面资源，下载到 $resourceRoot"
    $download = Join-Path $RepoRoot 'target/desktop-resources-download'
    Push-Location $RepoRoot
    try {
        $reported = cargo run --quiet --locked -p msime-client-core --example install_resources -- $download
        if ($LASTEXITCODE -ne 0) { throw '桌面资源下载失败' }
    } finally { Pop-Location }
    $generation = @($reported)[-1].Trim()
    if (-not (Test-Path -LiteralPath $generation -PathType Container) -or @(Get-ChildItem -LiteralPath $generation -File).Count -eq 0) {
        throw "桌面资源代次目录无效：$generation"
    }
    New-Item -ItemType Directory -Force -Path $resourceRoot | Out-Null
    Copy-Item -Path (Join-Path $generation '*') -Destination $resourceRoot -Recurse -Force
}
Write-Host "安装包版本：$TargetVersion；CMake 生成器：$Generator$(if ($Light) { '；轻量包' })"
$build = Join-Path $RepoRoot 'platforms/windows/Build-Client.ps1'
foreach ($stage in @($build, (Join-Path $PSScriptRoot 'Prepare-PackageFiles.ps1'),
    (Join-Path $PSScriptRoot 'Sign-PackageBinaries-Local.ps1'), (Join-Path $PSScriptRoot 'Compile-Installer.ps1'),
    (Join-Path $PSScriptRoot 'Sign-Installer-Local.ps1'), (Join-Path $PSScriptRoot 'Install.ps1'))) {
    if (-not (Test-Path -LiteralPath $stage -PathType Leaf)) { throw 'Missing Client installation stage' }
}
Push-Location $PSScriptRoot
try {
    $buildParameters = @{ RepoRoot = $RepoRoot; X64Dependencies = $X64Dependencies; X86Dependencies = $X86Dependencies; Generator = $Generator; TargetVersion = $TargetVersion; ServerUiAccess = $true }
    if ($RustOutputs) {
        $buildParameters.RustOutputs = if ([IO.Path]::IsPathRooted($RustOutputs)) { $RustOutputs } else { Join-Path $RepoRoot $RustOutputs }
    }
    Invoke-InstallerStage $build $buildParameters
    Invoke-InstallerStage (Join-Path $PSScriptRoot 'Prepare-PackageFiles.ps1') @{
        RepoRoot = $RepoRoot; TargetVersion = $TargetVersion; NoticesDirectory = $noticeRoot
        DesktopResourcesDirectory = $DesktopResourcesDirectory; Light = $Light
        ServerReleaseDirectory = 'target/windows-full/x64/bin'
        Tsf32ReleaseDirectory = 'target/windows-full/x86/bin'; Tsf64ReleaseDirectory = 'target/windows-full/x64/bin'
        DesktopExecutable = 'target/windows-full/x64/bin/msime-client-settings.exe'
    }
    Invoke-InstallerStage (Join-Path $PSScriptRoot 'Sign-PackageBinaries-Local.ps1') @{}
    Invoke-InstallerStage (Join-Path $PSScriptRoot 'Compile-Installer.ps1') @{ Light = $Light }
    Invoke-InstallerStage (Join-Path $PSScriptRoot 'Sign-Installer-Local.ps1') @{ Light = $Light }
    if ($NoInstall) {
        Write-Host '已按 -NoInstall 跳过安装，安装包在 platforms/windows/installer/Output'
    } else {
        Invoke-InstallerStage (Join-Path $PSScriptRoot 'Install.ps1') @{ Light = $Light }
    }
} finally { Pop-Location }
