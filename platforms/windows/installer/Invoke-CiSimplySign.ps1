# 只在专用签名 runner 上读取当前用户 DPAPI 凭据，不向 GitHub 环境或输出文件写入秘密。
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$PackageRoot,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [Parameter(Mandatory)][ValidatePattern('^[a-fA-F0-9]{64}$')][string]$SsignSha256,
    [Parameter(Mandatory)][ValidatePattern('^[a-fA-F0-9]{40}$')][string]$CertificateThumbprint,
    [string]$CredentialPath = (Join-Path $env:LOCALAPPDATA 'CertumAutomation/credential.dpapi'),
    [string]$SsignPath = 'C:/Program Files/msime-tools/ssign/v0.1.8/ssign.exe',
    [ValidatePattern('^[a-z][a-z0-9]*$')][string]$Edition = 'full',
    [string]$IsccPath,
    [string]$SignToolPath
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if ((Get-FileHash -LiteralPath $SsignPath -Algorithm SHA256).Hash -ine $SsignSha256) { throw 'ssign 客户端摘要与固定配置不符。' }
if (-not (Test-Path -LiteralPath $CredentialPath -PathType Leaf)) { throw '缺少当前用户的 DPAPI 签名凭据。' }
$signatureCache = ''
$plain = $null
$uriText = $null
try {
    Add-Type -AssemblyName System.Security
    $plain = [Security.Cryptography.ProtectedData]::Unprotect([IO.File]::ReadAllBytes($CredentialPath), $null, [Security.Cryptography.DataProtectionScope]::CurrentUser)
    $uriText = [Text.Encoding]::UTF8.GetString($plain)
    $uri = [Uri]$uriText
    $query = [System.Web.HttpUtility]::ParseQueryString($uri.Query)
    if ($uri.Scheme -ne 'otpauth' -or $uri.Host -ne 'totp' -or $query['issuer'] -ne 'Certum' -or $query['algorithm'] -ne 'SHA256' -or $query['digits'] -ne '6' -or $query['period'] -ne '30') { throw '签名凭据必须是 Certum SHA256、6 位、30 秒 TOTP。' }
    $label = [Uri]::UnescapeDataString($uri.AbsolutePath.TrimStart('/'))
    if (-not $label.StartsWith('Certum:') -or -not $label.Substring(7).Contains('@') -or [string]::IsNullOrWhiteSpace($query['secret'])) { throw '签名凭据缺少账号或 seed。' }
    if ($env:GITHUB_RUN_ID) {
        if ($env:GITHUB_RUN_ID -notmatch '^\d+$') { throw '签名缓存需要有效的 GitHub 运行 ID。' }
        $cacheRoot = Join-Path $env:LOCALAPPDATA 'CertumAutomation/signature-cache'
        New-Item -ItemType Directory -Force -Path $cacheRoot | Out-Null
        # 发布并发组和签名矩阵均串行；只保留本轮缓存，失败重跑沿用，下一轮回收旧文件。
        Get-ChildItem -LiteralPath $cacheRoot -Directory |
            Where-Object { $_.Name -match '^\d+$' -and $_.Name -ne $env:GITHUB_RUN_ID } |
            Remove-Item -Recurse -Force
        $signatureCache = Join-Path $cacheRoot "$env:GITHUB_RUN_ID/$($CertificateThumbprint.ToUpperInvariant())"
    }
    $env:CERTUM_EMAIL = $label.Substring(7)
    $env:CERTUM_OTP = $query['secret']
    Remove-Item Env:CERTUM_TOKEN -ErrorAction SilentlyContinue
    # 每个版本使用新会话，等到下一个时间窗再登录，避免刚被上一版本或桌面登录消耗的验证码。
    $delay = 31 - ([DateTimeOffset]::UtcNow.ToUnixTimeSeconds() % 30)
    Start-Sleep -Seconds $delay
    & (Join-Path $PSScriptRoot 'Sign-StagedPackage-SimplySign.ps1') -PackageRoot $PackageRoot -OutputDirectory $OutputDirectory -Edition $Edition -SsignPath $SsignPath -SignToolPath $SignToolPath -IsccPath $IsccPath -CertificateThumbprint $CertificateThumbprint -SignatureCacheDirectory $signatureCache
    if ($LASTEXITCODE -ne 0) { throw 'SimplySign 安装包签名失败。' }
} catch {
    # 解密和 URI 解析异常可能包含原始凭据，CI 只输出固定错误。
    throw 'SimplySign CI 签名失败；请在签名机检查凭据、客户端、证书和网络。'
} finally {
    if ($null -ne $plain) { [Array]::Clear($plain, 0, $plain.Length) }
    $uriText = $null
    $uri = $null
    $query = $null
    $label = $null
    foreach ($name in 'CERTUM_EMAIL', 'CERTUM_OTP', 'CERTUM_TOKEN') { Remove-Item "Env:$name" -ErrorAction SilentlyContinue }
}
$global:LASTEXITCODE = 0
