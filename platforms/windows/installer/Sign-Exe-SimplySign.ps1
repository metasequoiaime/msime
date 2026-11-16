<#
.SYNOPSIS
    用 SimplySign 的 TOTP seed 自动计算动态码并签名一个或多个 EXE。

.DESCRIPTION
    这个入口给 SSH/无桌面的 Windows runner 使用。它调用外部的 ssign.exe，
    从 CERTUM_EMAIL 和 CERTUM_OTP 环境变量读取账号与 TOTP seed，不把 seed
    放进命令行参数；签名完成后用 Windows SDK 的 signtool 和 Authenticode
    再核对签名与 RFC 3161 时间戳。

    ssign 是独立的社区客户端，不属于 Certum。正式发布前应固定并审核
    ssign.exe 的来源；本脚本不会自动下载或替换它。
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
    [string]$SignToolPath
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
            if ([IO.Path]::GetExtension($resolved) -ine '.exe') {
                throw "此入口只接受 .exe：$item"
            }
            $resolved
        }
    )
    if ($targets.Count -eq 0) { throw '没有待签名的 EXE。' }
    return @($targets | Sort-Object -Unique)
}

function Assert-SignedExecutable {
    param(
        [Parameter(Mandatory)][string]$Executable,
        [Parameter(Mandatory)][string]$SignTool
    )
    & $SignTool verify /pa /all /v $Executable
    if ($LASTEXITCODE -ne 0) { throw "EXE 签名校验失败：$Executable" }
    $signature = Get-AuthenticodeSignature -LiteralPath $Executable
    if ($signature.Status -ne 'Valid' -or -not $signature.SignerCertificate) {
        throw "EXE 的 Authenticode 状态不是 Valid：$Executable ($($signature.Status))"
    }
    if (-not $signature.TimeStamperCertificate) {
        throw "EXE 缺少 RFC 3161 时间戳：$Executable"
    }
}

if ([string]::IsNullOrWhiteSpace($env:CERTUM_EMAIL)) {
    throw '必须通过 CERTUM_EMAIL 环境变量提供 SimplySign 账号邮箱。'
}
if ([string]::IsNullOrWhiteSpace($env:CERTUM_OTP)) {
    throw '必须通过 CERTUM_OTP 环境变量提供 TOTP seed；不要把 seed 写进命令行。'
}

$targets = Resolve-Targets -InputPath $Path
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
        throw '输出目录中存在同名 EXE，拒绝覆盖。'
    }
} else {
    foreach ($target in $targets) { $outputs[$target] = $target }
}
$ssignArgs += $targets

try {
    Write-Host "使用 ssign 为 $($targets.Count) 个 EXE 计算动态码并签名。"
    & $ssign @ssignArgs
    if ($LASTEXITCODE -ne 0) { throw "ssign 签名失败，退出码：$LASTEXITCODE" }
    foreach ($target in $targets) {
        $output = $outputs[$target]
        if (-not (Test-Path -LiteralPath $output -PathType Leaf)) {
            throw "ssign 没有生成签名文件：$output"
        }
        Assert-SignedExecutable -Executable $output -SignTool $signTool
        Write-Host "签名并校验通过：$output"
    }
} finally {
    Remove-Item Env:CERTUM_OTP -ErrorAction SilentlyContinue
}
$global:LASTEXITCODE = 0
