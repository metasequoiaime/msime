<#
.SYNOPSIS
    签名暂存包中的 EXE/DLL，再编译并签名外层安装器。
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$PackageRoot,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [string]$SsignPath,
    [string]$SignToolPath,
    [string]$IsccPath,
    [ValidatePattern('^$|^[a-fA-F0-9]{40}$')][string]$CertificateThumbprint,
    [ValidatePattern('^[a-z][a-z0-9]*$')][string]$Edition = 'full',
    [string]$SignatureCacheDirectory
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$root = (Resolve-Path -LiteralPath $PackageRoot).Path
$iss = Join-Path $root 'msime_setup.iss'
if (-not (Test-Path -LiteralPath $iss -PathType Leaf)) { throw '暂存包缺少 msime_setup.iss' }
if (Test-Path -LiteralPath $OutputDirectory) { throw '输出目录必须是新目录，避免混入旧安装器。' }
$files = @(
    foreach ($name in @('server_exe', 'tsf_dll')) {
        $directory = Join-Path $root $name
        if (-not (Test-Path -LiteralPath $directory -PathType Container)) { throw "暂存包缺少 $name" }
        Get-ChildItem -LiteralPath $directory -Recurse -File |
            Where-Object { $_.Extension -in @('.exe', '.dll') }
    }
)
if ($files.Count -eq 0) { throw '暂存包中没有 EXE/DLL。' }
$unsigned = @(
    foreach ($file in $files) {
        $signature = Get-AuthenticodeSignature -LiteralPath $file.FullName
        if ($signature.Status -eq 'NotSigned') { $file.FullName }
        elseif ($signature.Status -ne 'Valid') { throw "已有签名无效，拒绝覆盖：$($file.Name)" }
    }
)
$oldRuntime = [Environment]::GetEnvironmentVariable('XDG_RUNTIME_DIR', 'Process')
$sessionRoot = Join-Path $env:TEMP ('msime-signing-' + [Guid]::NewGuid())
New-Item -ItemType Directory -Path $sessionRoot | Out-Null
try {
    # 两次 ssign 调用共享本轮登录；外层签名完成后删除会话，不交给后续 runner 作业。
    $env:XDG_RUNTIME_DIR = $sessionRoot
    $signArgs = @{ SsignPath = $SsignPath; SignToolPath = $SignToolPath; CertificateThumbprint = $CertificateThumbprint; SignatureCacheDirectory = $SignatureCacheDirectory }
    if ($unsigned.Count -gt 0) {
        & (Join-Path $PSScriptRoot 'Sign-Files-SimplySign.ps1') -Path $unsigned @signArgs
        if ($LASTEXITCODE -ne 0) { throw '包内 EXE/DLL 签名失败。' }
    }
    New-Item -ItemType Directory -Path $OutputDirectory | Out-Null
    $outputRoot = (Resolve-Path -LiteralPath $OutputDirectory).Path
    & (Join-Path $PSScriptRoot 'Compile-Installer.ps1') -IssPath $iss -SourceDirectory $root -OutputDirectory $outputRoot -Edition $Edition -IsccPath $IsccPath
    if ($LASTEXITCODE -ne 0) { throw '安装器编译失败。' }
    $installers = @(Get-ChildItem -LiteralPath $outputRoot -File -Filter '*.exe')
    if ($installers.Count -ne 1) { throw '没有生成唯一的安装器。' }
    & (Join-Path $PSScriptRoot 'Sign-Files-SimplySign.ps1') -Path $installers[0].FullName @signArgs
    if ($LASTEXITCODE -ne 0) { throw '外层安装器签名失败。' }
    foreach ($file in $files) {
        if ((Get-AuthenticodeSignature -LiteralPath $file.FullName).Status -ne 'Valid') {
            throw "包内签名复核失败：$($file.Name)"
        }
    }
    $installer = $installers[0]
    $hash = (Get-FileHash -LiteralPath $installer.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
    $digestLine = "$hash  $($installer.Name)`n"
    [IO.File]::WriteAllText((Join-Path $outputRoot ($installer.Name + '.sha256')), $digestLine)
    [IO.File]::WriteAllText((Join-Path $outputRoot 'SHA256SUMS'), $digestLine)
    Write-Host "签名完成：$($files.Count) 个包内文件及外层安装器。"
} finally {
    if ($null -eq $oldRuntime) {
        Remove-Item Env:XDG_RUNTIME_DIR -ErrorAction SilentlyContinue
    } else {
        [Environment]::SetEnvironmentVariable('XDG_RUNTIME_DIR', $oldRuntime, 'Process')
    }
    Remove-Item -LiteralPath $sessionRoot -Recurse -Force
    Remove-Item Env:CERTUM_OTP -ErrorAction SilentlyContinue
    Remove-Item Env:CERTUM_TOKEN -ErrorAction SilentlyContinue
}
$global:LASTEXITCODE = 0
