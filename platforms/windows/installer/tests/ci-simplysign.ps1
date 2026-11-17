# 使用真实 DPAPI 和合成凭据验证 CI 边界；失败必须发生在联网签名前。
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$script = (Resolve-Path (Join-Path $PSScriptRoot '../Invoke-CiSimplySign.ps1')).Path
$root = Join-Path ([IO.Path]::GetTempPath()) ('msime-ci-signing-' + [Guid]::NewGuid())
$oldLocalAppData = $env:LOCALAPPDATA
$oldRunId = $env:GITHUB_RUN_ID
function Assert-Throws {
    param([scriptblock]$Action, [string]$Message)
    try { & $Action } catch {
        if ($_.Exception.Message -notlike "*$Message*" -or $_.Exception.Message.Contains('synthetic-private')) { throw '失败消息不符合边界约束。' }
        return
    }
    throw '应在联网前拒绝配置。'
}
try {
    New-Item -ItemType Directory $root | Out-Null
    $env:LOCALAPPDATA = $root
    $env:GITHUB_RUN_ID = '12345'
    $client = Join-Path $root 'ssign.exe'
    [IO.File]::WriteAllText($client, 'synthetic-non-executable')
    $credential = Join-Path $root 'credential.dpapi'
    $arguments = @{ PackageRoot = $root; OutputDirectory = (Join-Path $root 'output'); SsignPath = $client; SsignSha256 = ('A' * 64); CertificateThumbprint = ('B' * 40); CredentialPath = $credential }
    Assert-Throws { & $script @arguments } '客户端摘要'
    $arguments.SsignSha256 = (Get-FileHash $client).Hash
    Assert-Throws { & $script @arguments } 'DPAPI 签名凭据'
    Add-Type -AssemblyName System.Security
    foreach ($plain in @('synthetic-private-invalid-uri', 'otpauth://totp/Certum:synthetic@example.invalid?secret=synthetic-private&issuer=Certum&algorithm=SHA1&digits=6&period=30')) {
        $protected = [Security.Cryptography.ProtectedData]::Protect([Text.Encoding]::UTF8.GetBytes($plain), $null, [Security.Cryptography.DataProtectionScope]::CurrentUser)
        [IO.File]::WriteAllBytes($credential, $protected)
        $env:CERTUM_EMAIL = 'synthetic@example.invalid'
        $env:CERTUM_OTP = 'synthetic-private'
        Assert-Throws { & $script @arguments } 'SimplySign CI 签名失败'
        if ($env:CERTUM_EMAIL -or $env:CERTUM_OTP -or $env:CERTUM_TOKEN) { throw '失败后凭据未清理。' }
        if (Test-Path $arguments.OutputDirectory) { throw '非法配置启动了打包。' }
        if (Test-Path (Join-Path $root 'CertumAutomation/signature-cache')) { throw '非法凭据修改了签名缓存。' }
    }
    Write-Host 'SimplySign CI DPAPI 与配置边界通过'
    $global:LASTEXITCODE = 0
} finally {
    $env:LOCALAPPDATA = $oldLocalAppData
    $env:GITHUB_RUN_ID = $oldRunId
    Remove-Item $root -Recurse -Force
    foreach ($name in 'CERTUM_EMAIL', 'CERTUM_OTP', 'CERTUM_TOKEN') { Remove-Item "Env:$name" -ErrorAction SilentlyContinue }
}
