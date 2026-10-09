<#
.SYNOPSIS
    把暂存包里本项目自己的二进制交给 SignPath 签名，再把签好的放回原处。

.DESCRIPTION
    release-windows.yml 的 SignPath 路径调用这里，是 Sign-PackageBinaries-SimplySign.ps1 的 CI 对应物，但签的范围不同：SignPath Foundation 只允许项目签自己的代码，所以 Windows App SDK、ONNX Runtime、sherpa-onnx 和 vcpkg 构建的 DLL 保持上游发布时的样子，不提交。

    本项目的二进制按两条规则认定：server_exe 下有同名 PDB 的 EXE（Prepare-PackageFiles.ps1 要求除 Windows App SDK 自带的可执行文件以外，每个 Server EXE 都带 PDB，并把 PDB 一起暂存进来，所以有 PDB 就是这次构建编出来的），以及 tsf_dll 下各架构的 TIP 和本版本的宿主 DLL（名字取自 shared/contracts/editions.json）。server_exe 里没有本项目的 DLL。

    Stage 把这些文件按相对 installer 目录的路径复制到 <Directory>/<Edition>/ 下，各版本的暂存目录合起来就是一次签名请求的 artifact，路径与 signpath/msime-payload.xml 对应。Restore 在编译安装包的 job 里运行，那里的暂存包不带 PDB，所以它不再按 PDB 认定，而是取回 SignPath 为本版本返回的每个文件：签名状态不是 Valid 就失败，TIP、宿主 DLL 和几个 Server 入口缺一个也失败，再覆盖暂存包里的原文件。

    -TestCertificate 用于 test-signing 策略：SignPath 的测试证书链到一个系统不信任的根，Get-AuthenticodeSignature 对它报 UnknownError 而不是 Valid。这时只要求文件确实带签名（有签名证书），不要求链可信。release-signing 的运行不传它。
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidateSet('Stage', 'Restore')][string]$Mode,
    [Parameter(Mandatory)][ValidatePattern('^[a-z][a-z0-9]*$')][string]$Edition,
    [Parameter(Mandatory)][string]$Directory,
    [string]$PackageRoot = $PSScriptRoot,
    [string]$RepoRoot = (Split-Path -Parent (Split-Path -Parent (Split-Path -Parent $PSScriptRoot))),
    [switch]$TestCertificate
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$PackageRoot = (Resolve-Path -LiteralPath $PackageRoot).Path
$editionTable = Get-Content -LiteralPath (Join-Path $RepoRoot 'shared/contracts/editions.json') -Raw | ConvertFrom-Json
$editionEntry = @($editionTable.editions | Where-Object { $_.id -ceq $Edition -and $null -ne $_.platforms.windows })
if ($editionEntry.Count -ne 1) { throw "版本 $Edition 在 shared/contracts/editions.json 里没有 Windows 标识" }
$hostDllName = [string]$editionEntry[0].platforms.windows.host_dll
$arm64HostDllName = [IO.Path]::GetFileNameWithoutExtension($hostDllName) + '_arm64.dll'

$serverDirectory = Join-Path $PackageRoot 'server_exe'
$tsfDirectory = Join-Path $PackageRoot 'tsf_dll'
foreach ($required in @($serverDirectory, $tsfDirectory)) {
    if (-not (Test-Path -LiteralPath $required -PathType Container)) { throw "暂存目录不存在，请先运行 Prepare-PackageFiles.ps1：$required" }
}
$editionRoot = Join-Path $Directory $Edition
$required = @(
    'server_exe/MetasequoiaImeServer.exe', 'server_exe/MetasequoiaImeWatchdog.exe', 'server_exe/msime-mcp.exe', 'server_exe/msime-client-settings.exe'
    foreach ($arch in @('32', '64')) { "tsf_dll/$arch/MetasequoiaImeTsf.dll"; "tsf_dll/$arch/$hostDllName" }
    'tsf_dll/arm64/MetasequoiaImeTsf.dll'
    "tsf_dll/arm64/$arm64HostDllName"
)
$files = if ($Mode -eq 'Stage') {
    @(
        Get-ChildItem -LiteralPath $serverDirectory -File -Filter '*.exe' |
            Where-Object { Test-Path -LiteralPath (Join-Path $_.DirectoryName "$($_.BaseName).pdb") -PathType Leaf } |
            ForEach-Object { "server_exe/$($_.Name)" }
        $required | Where-Object { $_ -like 'tsf_dll/*' }
    ) | Sort-Object -Unique
} else {
    if (-not (Test-Path -LiteralPath $editionRoot -PathType Container)) { throw "SignPath 返回的 artifact 里没有 $Edition" }
    $signedRoot = (Resolve-Path -LiteralPath $editionRoot).Path
    @(Get-ChildItem -LiteralPath $signedRoot -Recurse -File | ForEach-Object { $_.FullName.Substring($signedRoot.Length + 1) -replace '\\', '/' }) | Sort-Object -Unique
}
# Prepare-PackageFiles.ps1 已断言这几个入口存在。Stage 时确认它们都被认成了本项目的二进制，PDB 规则失效时（例如 PDB 不再暂存）不至于悄悄只签 TIP；Restore 时确认 SignPath 把它们都送了回来。
$missing = @($required | Where-Object { $_ -notin $files })
if ($missing.Count -gt 0) { throw "$Mode 缺少本项目的二进制（Stage 时多半是缺同名 PDB）：$($missing -join ', ')" }

if ($Mode -eq 'Stage') {
    if (Test-Path -LiteralPath $editionRoot) { Remove-Item -LiteralPath $editionRoot -Recurse -Force }
    foreach ($file in $files) {
        $source = Join-Path $PackageRoot $file
        if (-not (Test-Path -LiteralPath $source -PathType Leaf)) { throw "暂存包里没有 $file" }
        $target = Join-Path $editionRoot $file
        New-Item -ItemType Directory -Force -Path (Split-Path -Parent $target) | Out-Null
        Copy-Item -LiteralPath $source -Destination $target
    }
    Write-Host "提交 SignPath 的 $Edition 二进制（$($files.Count) 个）："
    $files | ForEach-Object { Write-Host "  $_" }
    # 其余 PE 文件保持上游原样，列出来连同它们现有的签名状态留作发布记录。
    $submitted = @($files | ForEach-Object { (Join-Path $PackageRoot $_) -replace '/', '\' })
    Write-Host '不提交、保持上游原样的 PE 文件：'
    Get-ChildItem -LiteralPath $serverDirectory, $tsfDirectory -Recurse -File -Include '*.exe', '*.dll' |
        Where-Object { ($_.FullName -replace '/', '\') -notin $submitted } |
        ForEach-Object { Write-Host ('  {0}  [{1}]' -f $_.FullName.Substring($PackageRoot.Length + 1), (Get-AuthenticodeSignature -LiteralPath $_.FullName).Status) }
} else {
    foreach ($file in $files) {
        $signed = Join-Path $editionRoot $file
        if (-not (Test-Path -LiteralPath $signed -PathType Leaf)) { throw "SignPath 没有返回 $Edition/$file" }
        $signature = Get-AuthenticodeSignature -LiteralPath $signed
        $accepted = $signature.Status -eq 'Valid' -or ($TestCertificate -and $signature.Status -eq 'UnknownError' -and $null -ne $signature.SignerCertificate)
        if (-not $accepted) { throw "SignPath 返回的 $Edition/$file 签名状态是 $($signature.Status)：$($signature.StatusMessage)" }
        Copy-Item -LiteralPath $signed -Destination (Join-Path $PackageRoot $file) -Force
        Write-Host "$file 已签名：$($signature.SignerCertificate.Subject)"
    }
}
$global:LASTEXITCODE = 0
