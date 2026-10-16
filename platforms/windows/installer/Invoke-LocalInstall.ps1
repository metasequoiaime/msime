# Explicit local test-install workflow. Unlike Build-Client, this signs and
# launches an installer; automated tests must substitute every side-effect stage.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$X64Dependencies,
    [Parameter(Mandatory)][string]$X86Dependencies,
    [Parameter(Mandatory)][string]$NoticesDirectory,
    [string]$DesktopResourcesDirectory = 'target/desktop-resources',
    [string]$TargetVersion = (Get-Content -LiteralPath (Join-Path (Split-Path -Parent $PSScriptRoot) 'version.txt') -TotalCount 1).Trim(),
    [string]$RepoRoot = (Split-Path -Parent (Split-Path -Parent (Split-Path -Parent $PSScriptRoot))),
    [string]$Generator = 'Visual Studio 17 2022',
    [switch]$Light
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
function Invoke-InstallerStage {
    param([string]$Path, [hashtable]$Parameters)
    $global:LASTEXITCODE = 0
    & $Path @Parameters
    if ($global:LASTEXITCODE -ne 0) { throw "Installer stage failed ($global:LASTEXITCODE): $([IO.Path]::GetFileName($Path))" }
}
$RepoRoot = (Resolve-Path -LiteralPath $RepoRoot).Path
$noticeRoot = if ([IO.Path]::IsPathRooted($NoticesDirectory)) { $NoticesDirectory } else { Join-Path $RepoRoot $NoticesDirectory }
if (-not (Test-Path -LiteralPath (Join-Path $noticeRoot 'THIRD_PARTY_NOTICES.txt') -PathType Leaf)) {
    throw 'Supply the prepared third-party notice directory before local installation'
}
$build = Join-Path $RepoRoot 'platforms/windows/Build-Client.ps1'
foreach ($stage in @($build, (Join-Path $PSScriptRoot 'Prepare-PackageFiles.ps1'),
    (Join-Path $PSScriptRoot 'Sign-PackageBinaries-Local.ps1'), (Join-Path $PSScriptRoot 'Compile-Installer.ps1'),
    (Join-Path $PSScriptRoot 'Sign-Installer-Local.ps1'), (Join-Path $PSScriptRoot 'Install.ps1'))) {
    if (-not (Test-Path -LiteralPath $stage -PathType Leaf)) { throw 'Missing Client installation stage' }
}
Push-Location $PSScriptRoot
try {
    Invoke-InstallerStage $build @{ RepoRoot = $RepoRoot; X64Dependencies = $X64Dependencies; X86Dependencies = $X86Dependencies; Generator = $Generator; TargetVersion = $TargetVersion }
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
    Invoke-InstallerStage (Join-Path $PSScriptRoot 'Install.ps1') @{ Light = $Light }
} finally { Pop-Location }
