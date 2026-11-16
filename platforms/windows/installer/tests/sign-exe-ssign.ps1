# 验证 TOTP 签名入口在没有凭据或没有 ssign 时尽早失败，不把秘密变成命令行参数。
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$script = (Resolve-Path (Join-Path $PSScriptRoot '../Sign-Exe-SimplySign.ps1')).Path
$root = Join-Path ([IO.Path]::GetTempPath()) ('msime-sign-exe-ssign-' + [Guid]::NewGuid())
$inputFile = Join-Path $root 'input.exe'

function Assert-Throws {
    param([string]$Description, [scriptblock]$Action, [string]$Expected)
    try { & $Action } catch {
        if ($_.Exception.Message -notlike "*$Expected*") {
            throw "$Description 失败原因不对：$($_.Exception.Message)"
        }
        return
    }
    throw "$Description 没有失败"
}

try {
    New-Item -ItemType Directory -Force -Path $root | Out-Null
    [IO.File]::WriteAllBytes($inputFile, [byte[]](0x4d, 0x5a))
    Remove-Item Env:CERTUM_EMAIL -ErrorAction SilentlyContinue
    Remove-Item Env:CERTUM_OTP -ErrorAction SilentlyContinue
    Assert-Throws '缺少 SimplySign 邮箱' { & $script -Path $inputFile } 'CERTUM_EMAIL'

    $env:CERTUM_EMAIL = 'synthetic@example.invalid'
    Assert-Throws '缺少 TOTP seed' { & $script -Path $inputFile } 'CERTUM_OTP'

    $env:CERTUM_OTP = 'synthetic-seed'
    Assert-Throws '缺少 ssign 客户端' { & $script -Path $inputFile -SsignPath (Join-Path $root 'missing-ssign.exe') } 'ssign.exe 不存在'

    $dll = Join-Path $root 'input.dll'
    [IO.File]::WriteAllBytes($dll, [byte[]](0x4d, 0x5a))
    Assert-Throws '非 EXE 输入' { & $script -Path $dll } '此入口只接受 .exe'
    Write-Host 'SimplySign TOTP 签名入口的前置条件通过'
} finally {
    Remove-Item Env:CERTUM_EMAIL -ErrorAction SilentlyContinue
    Remove-Item Env:CERTUM_OTP -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $root -Recurse -Force -ErrorAction SilentlyContinue
}
