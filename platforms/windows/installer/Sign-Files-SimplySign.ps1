<#
.SYNOPSIS
    用 SimplySign 的 TOTP seed 自动计算动态码并签名一个或多个 EXE/DLL。

.DESCRIPTION
    这个入口给 SSH/无桌面的 Windows runner 使用。它调用外部的 ssign.exe，从 CERTUM_EMAIL 和 CERTUM_OTP/CERTUM_TOKEN 环境变量读取账号与验证码，不把秘密放进命令行参数；签名完成后用 Windows SDK 的 signtool 和 Authenticode 再核对签名与 RFC 3161 时间戳。

    ssign 是独立的社区客户端，不属于 Certum。正式发布前应固定并审核 ssign.exe 的来源；本脚本不会自动下载或替换它。
#>
[CmdletBinding()]
param(
    [Parameter(Mandatory, Position = 0)]
    [ValidateNotNullOrEmpty()]
    [string[]]$Path,
    [string]$SsignPath,
    [string]$OutputDirectory,
    [ValidateNotNullOrEmpty()]
    [string]$TimestampUrl = 'http://time.certum.pl',
    [string]$Description,
    [string]$Url,
    [string]$SignToolPath,
    [ValidatePattern('^$|^[a-fA-F0-9]{40}$')][string]$CertificateThumbprint,
    [string]$SignatureCacheDirectory
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

function Resolve-Ssign {
    param([string]$ExplicitPath)
    if ($ExplicitPath) {
        if (-not (Test-Path -LiteralPath $ExplicitPath -PathType Leaf)) {
            throw "ssign.exe 不存在：$ExplicitPath"
        }
        return (Resolve-Path -LiteralPath $ExplicitPath).Path
    }
    foreach ($name in @('ssign.exe', 'ssign')) {
        $command = Get-Command $name -ErrorAction SilentlyContinue | Select-Object -First 1
        if ($command) { return $command.Source }
    }
    throw '找不到 ssign.exe；请先在 runner 上安装并通过 -SsignPath 或 PATH 提供它。'
}

function Find-SignTool {
    param([string]$ExplicitPath)
    if ($ExplicitPath) {
        if (-not (Test-Path -LiteralPath $ExplicitPath -PathType Leaf)) {
            throw "signtool.exe 不存在：$ExplicitPath"
        }
        return (Resolve-Path -LiteralPath $ExplicitPath).Path
    }
    $kitsBin = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits\10\bin'
    $candidate = Get-ChildItem -LiteralPath $kitsBin -Directory -ErrorAction SilentlyContinue |
        Sort-Object { try { [version]$_.Name } catch { [version]'0.0' } } -Descending |
        ForEach-Object { Join-Path $_.FullName 'x64\signtool.exe' } |
        Where-Object { Test-Path -LiteralPath $_ -PathType Leaf } |
        Select-Object -First 1
    if (-not $candidate) { throw '找不到 Windows SDK signtool.exe。' }
    return $candidate
}

function Resolve-Targets {
    param([string[]]$InputPath)
    $targets = @(
        foreach ($item in $InputPath) {
            if (-not (Test-Path -LiteralPath $item -PathType Leaf)) {
                throw "待签名文件不存在：$item"
            }
            $resolved = (Resolve-Path -LiteralPath $item).Path
            if ([IO.Path]::GetExtension($resolved) -inotmatch '^\.(exe|dll)$') {
                throw "此入口只接受 .exe/.dll：$item"
            }
            $resolved
        }
    )
    if ($targets.Count -eq 0) { throw '没有待签名的 EXE/DLL。' }
    return @($targets | Sort-Object -Unique)
}

function Assert-SignedExecutable {
    param(
        [Parameter(Mandatory)][string]$Executable,
        [Parameter(Mandatory)][string]$SignTool
    )
    & $SignTool verify /pa /all /v $Executable
    if ($LASTEXITCODE -ne 0) { throw "二进制签名校验失败：$Executable" }
    $signature = Get-AuthenticodeSignature -LiteralPath $Executable
    if ($signature.Status -ne 'Valid' -or -not $signature.SignerCertificate) {
        throw "二进制的 Authenticode 状态不是 Valid：$Executable ($($signature.Status))"
    }
    if (-not $signature.TimeStamperCertificate) {
        throw "二进制缺少可信时间戳：$Executable"
    }
    if ($CertificateThumbprint -and $signature.SignerCertificate.Thumbprint -ine $CertificateThumbprint) {
        throw "二进制签名证书与发布配置不符：$Executable"
    }
}

if ([string]::IsNullOrWhiteSpace($env:CERTUM_EMAIL)) {
    throw '必须通过 CERTUM_EMAIL 环境变量提供 SimplySign 账号邮箱。'
}
$hasOtp = -not [string]::IsNullOrWhiteSpace($env:CERTUM_OTP)
$hasToken = -not [string]::IsNullOrWhiteSpace($env:CERTUM_TOKEN)
if ($hasOtp -and $hasToken) {
    throw 'CERTUM_OTP 和 CERTUM_TOKEN 不能同时设置。'
}
if (-not $hasOtp -and -not $hasToken) {
    throw '必须通过 CERTUM_OTP 提供 TOTP seed，或通过 CERTUM_TOKEN 提供当前 6 位动态码；不要把秘密写进命令行。'
}

$targets = @(Resolve-Targets -InputPath $Path)
$ssign = Resolve-Ssign -ExplicitPath $SsignPath
$signTool = Find-SignTool -ExplicitPath $SignToolPath
$ssignArgs = @('--timestamp-url', $TimestampUrl)
if ($Description) { $ssignArgs += @('-n', $Description) }
if ($Url) { $ssignArgs += @('-u', $Url) }
$outputs = @{}
if ($OutputDirectory) {
    New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
    $outputRoot = (Resolve-Path -LiteralPath $OutputDirectory).Path
    $ssignArgs += @('-o', $outputRoot)
    foreach ($target in $targets) {
        $outputs[$target] = Join-Path $outputRoot ([IO.Path]::GetFileName($target))
    }
    $uniqueOutputs = @($outputs.Values | Sort-Object -Unique)
    if ($uniqueOutputs.Count -ne $targets.Count) {
        throw '输出目录中存在同名文件，拒绝覆盖。'
    }
} else {
    foreach ($target in $targets) { $outputs[$target] = $target }
}
$cacheLock = $null
try {
    $cacheRoot = $null
    $cacheContext = $null
    if ($SignatureCacheDirectory) {
        if (-not $CertificateThumbprint) { throw '复用签名缓存必须指定发布证书指纹。' }
        Add-Type -AssemblyName System.Security
        New-Item -ItemType Directory -Force -Path $SignatureCacheDirectory | Out-Null
        $cacheRoot = (Resolve-Path -LiteralPath $SignatureCacheDirectory).Path
        $cacheLock = [IO.File]::Open((Join-Path $cacheRoot '.lock'), [IO.FileMode]::OpenOrCreate, [IO.FileAccess]::ReadWrite, [IO.FileShare]::None)
        $cacheContext = ConvertTo-Json -Compress -InputObject @($CertificateThumbprint.ToUpperInvariant(), $TimestampUrl, $Description, $Url)
    }
    # 先取所有未签名字节的摘要，再改写任何文件；文件名和所在版本不影响复用。
    $groups = [ordered]@{}
    foreach ($target in $targets) {
        $sourceHash = (Get-FileHash -LiteralPath $target -Algorithm SHA256).Hash
        if (-not $groups.Contains($sourceHash)) { $groups[$sourceHash] = @() }
        $groups[$sourceHash] += $target
    }
    $pending = @()
    $signedByHash = @{}
    foreach ($sourceHash in $groups.Keys) {
        if ($cacheRoot) {
            $cached = Join-Path $cacheRoot "$sourceHash.exe"
            $seal = Join-Path $cacheRoot "$sourceHash.dpapi"
            if ((Test-Path -LiteralPath $cached) -or (Test-Path -LiteralPath $seal)) {
                if (-not (Test-Path -LiteralPath $cached -PathType Leaf) -or -not (Test-Path -LiteralPath $seal -PathType Leaf)) { throw '签名缓存不完整。' }
                # DPAPI 将签后摘要绑定到原文件摘要、证书和签名参数，不能把另一份合法签名文件冒充缓存命中。
                $binding = [Text.Encoding]::UTF8.GetBytes("$sourceHash`n$cacheContext")
                $expectedHash = [Text.Encoding]::UTF8.GetString([Security.Cryptography.ProtectedData]::Unprotect([IO.File]::ReadAllBytes($seal), $binding, [Security.Cryptography.DataProtectionScope]::CurrentUser))
                if ($expectedHash -cne (Get-FileHash -LiteralPath $cached -Algorithm SHA256).Hash) { throw '签名缓存摘要不匹配。' }
                Assert-SignedExecutable -Executable $cached -SignTool $signTool
                $signedByHash[$sourceHash] = $cached
                continue
            }
        }
        $pending += $groups[$sourceHash][0]
    }
    if ($pending.Count -gt 0) {
        $ssignArgs += $pending
        Write-Host "使用 ssign 为 $($pending.Count) 个唯一 EXE/DLL 签名。"
        $clientOutput = & $ssign @ssignArgs 2>&1
        if ($LASTEXITCODE -ne 0) { throw "ssign 签名失败，退出码：$LASTEXITCODE" }
    }
    foreach ($sourceHash in $groups.Keys) {
        if (-not $signedByHash.ContainsKey($sourceHash)) {
            $output = $outputs[$groups[$sourceHash][0]]
            if (-not (Test-Path -LiteralPath $output -PathType Leaf)) { throw "ssign 没有生成签名文件：$output" }
            Assert-SignedExecutable -Executable $output -SignTool $signTool
            $signedByHash[$sourceHash] = $output
            if ($cacheRoot) {
                $cached = Join-Path $cacheRoot "$sourceHash.exe"
                $seal = Join-Path $cacheRoot "$sourceHash.dpapi"
                Copy-Item -LiteralPath $output -Destination $cached
                $binding = [Text.Encoding]::UTF8.GetBytes("$sourceHash`n$cacheContext")
                $signedHash = [Text.Encoding]::UTF8.GetBytes((Get-FileHash -LiteralPath $cached -Algorithm SHA256).Hash)
                [IO.File]::WriteAllBytes($seal, [Security.Cryptography.ProtectedData]::Protect($signedHash, $binding, [Security.Cryptography.DataProtectionScope]::CurrentUser))
            }
        }
        foreach ($target in $groups[$sourceHash]) {
            $output = $outputs[$target]
            if ($output -ine $signedByHash[$sourceHash]) { Copy-Item -LiteralPath $signedByHash[$sourceHash] -Destination $output -Force }
            Assert-SignedExecutable -Executable $output -SignTool $signTool
            Write-Host "签名并校验通过：$output"
        }
    }
    Write-Host "签名统计：新增 $($pending.Count) 个唯一文件，复用 $($targets.Count - $pending.Count) 个文件。"
    if ($env:GITHUB_STEP_SUMMARY) {
        Add-Content -LiteralPath $env:GITHUB_STEP_SUMMARY -Encoding utf8 -Value "- SimplySign：新增 $($pending.Count) 个唯一文件签名，复用 $($targets.Count - $pending.Count) 个文件。"
    }
} finally {
    if ($null -ne $cacheLock) { $cacheLock.Dispose() }
    $clientOutput = $null
}
$global:LASTEXITCODE = 0
