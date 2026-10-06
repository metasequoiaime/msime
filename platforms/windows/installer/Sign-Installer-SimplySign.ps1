[CmdletBinding()]
param(
    [string]$InstallerPath = (Join-Path $PSScriptRoot 'Output\MetasequoiaIME-Full_Setup.exe'),
    [string]$CertificateThumbprint,
    [ValidateNotNullOrEmpty()]
    [string]$TimestampUrl = 'http://time.certum.pl',
    [string]$SignToolPath
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$CodeSigningEku = '1.3.6.1.5.5.7.3.3'

function Find-SignTool {
    param([string]$ExplicitPath)
    if ($ExplicitPath) {
        if (-not (Test-Path -LiteralPath $ExplicitPath -PathType Leaf)) { throw "signtool.exe 不存在：$ExplicitPath" }
        return (Resolve-Path -LiteralPath $ExplicitPath).Path
    }
    $root = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits\10\bin'
    $found = Get-ChildItem -LiteralPath $root -Directory -ErrorAction SilentlyContinue |
        Sort-Object { try { [version]$_.Name } catch { [version]'0.0' } } -Descending |
        ForEach-Object { Join-Path $_.FullName 'x64\signtool.exe' } |
        Where-Object { Test-Path -LiteralPath $_ -PathType Leaf } | Select-Object -First 1
    if (-not $found) { throw '找不到 Windows SDK signtool.exe。' }
    return $found
}

function Test-CodeSigningCertificate {
    param([Parameter(Mandatory)]$Certificate)
    foreach ($usage in @($Certificate.EnhancedKeyUsageList)) {
        $value = $usage.PSObject.Properties['Value']
        if ($value -and $value.Value -eq $CodeSigningEku) { return $true }
        $objectId = $usage.PSObject.Properties['ObjectId']
        if ($objectId) {
            $id = $objectId.Value
            if (($id -is [System.Security.Cryptography.Oid] -and $id.Value -eq $CodeSigningEku) -or $id -eq $CodeSigningEku) { return $true }
        }
    }
    return $false
}

function Select-SimplySignCertificate {
    param([string]$Thumbprint)
    $normalized = $Thumbprint -replace '\s', ''
    $candidates = @(Get-ChildItem -Path 'Cert:\CurrentUser\My' | Where-Object {
        $_.HasPrivateKey -and $_.NotBefore -le (Get-Date) -and $_.NotAfter -gt (Get-Date) -and
        (Test-CodeSigningCertificate $_) -and $_.Issuer -match '(?i)Certum'
    })
    if ($normalized) { $candidates = @($candidates | Where-Object { ($_.Thumbprint -replace '\s', '') -ieq $normalized }) }
    if ($candidates.Count -eq 0) { throw '没有发现有效的 Certum 代码签名证书；请先连接 SimplySign 虚拟卡。' }
    if ($candidates.Count -gt 1) { throw '发现多个 Certum 代码签名证书，请用 -CertificateThumbprint 指定。' }
    return $candidates[0]
}

$path = (Resolve-Path -LiteralPath $InstallerPath -ErrorAction Stop).Path
$certificate = Select-SimplySignCertificate -Thumbprint $CertificateThumbprint
$signTool = Find-SignTool -ExplicitPath $SignToolPath
$thumbprint = $certificate.Thumbprint -replace '\s', ''
& $signTool sign /sha1 $thumbprint /s My /fd sha256 /tr $TimestampUrl /td sha256 /v $path
if ($LASTEXITCODE -ne 0) { throw "安装包签名失败，退出码：$LASTEXITCODE" }
& $signTool verify /pa /all /v $path
if ($LASTEXITCODE -ne 0) { throw "安装包签名校验失败：$path" }
$signature = Get-AuthenticodeSignature -LiteralPath $path
if (-not $signature.SignerCertificate -or (($signature.SignerCertificate.Thumbprint -replace '\s', '') -ine $thumbprint)) {
    throw "安装包签名证书不匹配：$path"
}
if (-not $signature.TimeStamperCertificate) { throw "安装包缺少可信时间戳：$path" }
Write-Host "SimplySign 安装包签名完成：$path"
$global:LASTEXITCODE = 0
