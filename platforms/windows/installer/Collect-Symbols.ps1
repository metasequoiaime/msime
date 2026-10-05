<#
.SYNOPSIS
    把本次构建的调试符号打成 msime-windows-<edition>-<version>-symbols.zip。

.DESCRIPTION
    安装包不带 PDB（msime_setup.iss），用户机器上的崩溃转储只能用同一次构建的符号解析，所以符号作为单独的发布资产随安装包发布。release-windows.yml 和 Package-SimplySign.ps1 都调用这里，两边的文件名和目录布局因此一致。
    Server、设置、MCP 和 TSF 的 PDB 取自 Prepare-PackageFiles.ps1 的暂存目录（它拒绝没有 PDB 的 Server 可执行文件）；宿主 DLL 的 PDB 不进暂存目录，取自 Build-Client.ps1 在拿走宿主 DLL 的那一刻复制到各架构 bin 目录里的那一份，因为同一 target 目录里后续的 cargo 构建可能重建 host-api、覆盖 cargo 输出里的 PDB。两种架构的 PDB 同名，所以分放在不同文件夹。
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidatePattern('^[a-z][a-z0-9]*$')][string]$Edition,
    [Parameter(Mandatory)][ValidatePattern('^(0|[1-9][0-9]{0,4})\.(0|[1-9][0-9]{0,4})\.(0|[1-9][0-9]{0,4})$')][string]$Version,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [string]$RepoRoot = (Split-Path -Parent (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)))
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$RepoRoot = (Resolve-Path -LiteralPath $RepoRoot).Path
$staging = $PSScriptRoot
$bin = Join-Path $RepoRoot "target/windows-$Edition"
$layout = [ordered]@{
    'server' = @(Get-ChildItem -LiteralPath (Join-Path $staging 'server_exe') -File -Filter '*.pdb')
    'tsf/x86' = @(Get-Item -LiteralPath (Join-Path $staging 'tsf_dll/32/MetasequoiaImeTsf.pdb'))
    'tsf/x64' = @(Get-Item -LiteralPath (Join-Path $staging 'tsf_dll/64/MetasequoiaImeTsf.pdb'))
    'host/x86' = @(Get-Item -LiteralPath (Join-Path $bin 'x86/bin/msime_host_api.pdb'))
    'host/x64' = @(Get-Item -LiteralPath (Join-Path $bin 'x64/bin/msime_host_api.pdb'))
}
if ($layout['server'].Count -eq 0) { throw '暂存目录里没有 Server 的 PDB' }
# 列出不含符号的安装包内容，让日志能看出每个包装了什么，例如 tsf_dll\64 只有 TIP、它的宿主 DLL 和它自己的运行时 DLL。
Get-ChildItem -LiteralPath (Join-Path $staging 'server_exe'), (Join-Path $staging 'tsf_dll') -Recurse -File |
    Where-Object Extension -notin @('.pdb', '.ilk') |
    ForEach-Object { '{0,12:N0}  {1}' -f $_.Length, $_.FullName.Substring($staging.Length + 1) }

$root = Join-Path ([IO.Path]::GetTempPath()) "msime-symbols-$Edition-$Version"
if (Test-Path -LiteralPath $root) { Remove-Item -LiteralPath $root -Recurse -Force }
try {
    foreach ($entry in $layout.GetEnumerator()) {
        $folder = Join-Path $root $entry.Key
        New-Item -ItemType Directory -Force -Path $folder | Out-Null
        $entry.Value | Copy-Item -Destination $folder
    }
    Get-ChildItem -LiteralPath $root -Recurse -File | ForEach-Object { '{0,12:N0}  {1}' -f $_.Length, $_.FullName.Substring($root.Length + 1) }
    New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
    $archive = Join-Path (Resolve-Path -LiteralPath $OutputDirectory).Path "msime-windows-$Edition-$Version-symbols.zip"
    Compress-Archive -Path (Join-Path $root '*') -DestinationPath $archive -CompressionLevel Optimal -Force
    Write-Host "符号包：$archive"
} finally {
    Remove-Item -LiteralPath $root -Recurse -Force -ErrorAction SilentlyContinue
}
