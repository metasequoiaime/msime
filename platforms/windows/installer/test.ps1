# Full local build/sign/install. Use Build-Client.ps1 for build-only work.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$X64Dependencies,
    [Parameter(Mandatory)][string]$X86Dependencies,
    [Parameter(Mandatory)][string]$NoticesDirectory,
    [string]$DesktopResourcesDirectory = 'target/desktop-resources',
    [string]$TargetVersion = (Get-Content -LiteralPath (Join-Path (Split-Path -Parent $PSScriptRoot) 'version.txt') -TotalCount 1).Trim(),
    [string]$RepoRoot,
    [string]$Generator = 'Visual Studio 17 2022'
)
& (Join-Path $PSScriptRoot 'Invoke-LocalInstall.ps1') @PSBoundParameters
