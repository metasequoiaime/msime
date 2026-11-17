# 验证 TOTP 签名入口在没有凭据或没有 ssign 时尽早失败，不把秘密变成命令行参数。
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$script = (Resolve-Path (Join-Path $PSScriptRoot '../Sign-Files-SimplySign.ps1')).Path
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
    Remove-Item Env:CERTUM_TOKEN -ErrorAction SilentlyContinue
    Assert-Throws '缺少 SimplySign 邮箱' { & $script -Path $inputFile } 'CERTUM_EMAIL'

    $env:CERTUM_EMAIL = 'synthetic@example.invalid'
    Assert-Throws '缺少 TOTP seed' { & $script -Path $inputFile } 'CERTUM_OTP'

    $env:CERTUM_TOKEN = 'synthetic-token'
    Assert-Throws '一次性动态码走 ssign' { & $script -Path $inputFile -SsignPath (Join-Path $root 'missing-ssign.exe') } 'ssign.exe 不存在'
    Remove-Item Env:CERTUM_TOKEN -ErrorAction SilentlyContinue

    $env:CERTUM_OTP = 'synthetic-seed'
    $env:CERTUM_TOKEN = 'synthetic-token'
    Assert-Throws '两种凭据互斥' { & $script -Path $inputFile } '不能同时设置'
    Remove-Item Env:CERTUM_TOKEN -ErrorAction SilentlyContinue
    Assert-Throws '缺少 ssign 客户端' { & $script -Path $inputFile -SsignPath (Join-Path $root 'missing-ssign.exe') } 'ssign.exe 不存在'

    $dll = Join-Path $root 'input.txt'
    [IO.File]::WriteAllBytes($dll, [byte[]](0x4d, 0x5a))
    Assert-Throws '非 PE 扩展名输入' { & $script -Path $dll } '此入口只接受 .exe/.dll'

    # 合成客户端只记参数并标记文件，不联网；覆盖真实批量传参与签后校验的调用路径。
    $fakeClient = Join-Path $root 'ssign.ps1'
    $fakeVerifier = Join-Path $root 'signtool.ps1'
    $env:MSIME_SYNTHETIC_ARGUMENTS = Join-Path $root 'arguments.json'
    [IO.File]::WriteAllText($fakeClient, @'
param([Parameter(ValueFromRemainingArguments=$true)][string[]]$Arguments)
[IO.File]::WriteAllText($env:MSIME_SYNTHETIC_ARGUMENTS, (ConvertTo-Json -InputObject $Arguments))
if ($env:MSIME_SYNTHETIC_CALLS) { [IO.File]::AppendAllText($env:MSIME_SYNTHETIC_CALLS, "$env:XDG_RUNTIME_DIR`n") }
if ($env:MSIME_SYNTHETIC_SIGN_FAILURE) { $global:LASTEXITCODE = 7; return }
$outputRoot = ''
for ($i = 0; $i -lt $Arguments.Count; $i++) {
    if ($Arguments[$i] -eq '-o') { $outputRoot = $Arguments[$i + 1] }
}
foreach ($argument in $Arguments) {
    if ([IO.Path]::GetExtension($argument) -in @('.exe', '.dll') -and (Test-Path -LiteralPath $argument -PathType Leaf)) {
        $output = if ($outputRoot) { Join-Path $outputRoot ([IO.Path]::GetFileName($argument)) } else { $argument }
        [IO.File]::WriteAllText($output, 'synthetic-signed')
    }
}
$global:LASTEXITCODE = 0
'@)
    [IO.File]::WriteAllText($fakeVerifier, @'
param([Parameter(ValueFromRemainingArguments=$true)][string[]]$Arguments)
$global:LASTEXITCODE = if ($env:MSIME_SYNTHETIC_VERIFY_FAILURE) { 1 } else { 0 }
'@)
    function global:Get-AuthenticodeSignature {
        param([string]$LiteralPath)
        $content = [IO.File]::ReadAllText($LiteralPath)
        if ($content -eq 'synthetic-unsigned') { return [pscustomobject]@{ Status = 'NotSigned' } }
        if ($content -notin @('synthetic-signed', 'synthetic-vendor')) { throw '未检查签名输出文件' }
        [pscustomobject]@{
            Status = 'Valid'
            SignerCertificate = [pscustomobject]@{ Subject = 'CN=Synthetic'; Thumbprint = ('A' * 40) }
            TimeStamperCertificate = if ($env:MSIME_SYNTHETIC_NO_TIMESTAMP) { $null } else { [pscustomobject]@{} }
        }
    }
    $library = Join-Path $root 'library.dll'
    [IO.File]::WriteAllText($library, 'synthetic-unsigned')
    $outputDirectory = Join-Path $root 'signed'
    & $script -Path $inputFile,$library -SsignPath $fakeClient -SignToolPath $fakeVerifier -OutputDirectory $outputDirectory
    $arguments = @(Get-Content -LiteralPath $env:MSIME_SYNTHETIC_ARGUMENTS -Raw | ConvertFrom-Json)
    foreach ($file in @($inputFile, $library)) {
        if ($arguments -cnotcontains $file) { throw '批量调用漏掉 EXE/DLL' }
    }
    if (($arguments -join ' ') -match 'synthetic-seed|synthetic-token') { throw '秘密进入了命令行参数' }
    if ($env:CERTUM_OTP -ne 'synthetic-seed') { throw '脚本破坏了调用方凭据，后续安装器将无法签名' }
    $env:MSIME_SYNTHETIC_SIGN_FAILURE = '1'
    Assert-Throws '签名客户端失败' { & $script -Path $inputFile -SsignPath $fakeClient -SignToolPath $fakeVerifier } '退出码：7'
    Remove-Item Env:MSIME_SYNTHETIC_SIGN_FAILURE
    $env:MSIME_SYNTHETIC_VERIFY_FAILURE = '1'
    Assert-Throws '签名校验失败' { & $script -Path $inputFile -SsignPath $fakeClient -SignToolPath $fakeVerifier } '签名校验失败'
    Remove-Item Env:MSIME_SYNTHETIC_VERIFY_FAILURE
    $env:MSIME_SYNTHETIC_NO_TIMESTAMP = '1'
    Assert-Throws '缺少时间戳' { & $script -Path $inputFile -SsignPath $fakeClient -SignToolPath $fakeVerifier } '缺少可信时间戳'
    Remove-Item Env:MSIME_SYNTHETIC_NO_TIMESTAMP
    Assert-Throws '错误的发布证书' { & $script -Path $inputFile -SsignPath $fakeClient -SignToolPath $fakeVerifier -CertificateThumbprint ('B' * 40) } '签名证书与发布配置不符'
    & $script -Path $inputFile -SsignPath $fakeClient -SignToolPath $fakeVerifier -CertificateThumbprint ('A' * 40)

    # 编译器替身在包内文件全部带签名后才生成安装器；验证两次调用复用会话并最终清理。
    Copy-Item -LiteralPath $script -Destination (Join-Path $root 'Sign-Files-SimplySign.ps1')
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot '../Sign-StagedPackage-SimplySign.ps1') -Destination (Join-Path $root 'Sign-StagedPackage-SimplySign.ps1')
    [IO.File]::WriteAllText((Join-Path $root 'Compile-Installer.ps1'), @'
param($IssPath, $SourceDirectory, $OutputDirectory, $Edition, $IsccPath)
foreach ($relative in @('server_exe/app.exe', 'tsf_dll/host.dll')) {
    if ([IO.File]::ReadAllText((Join-Path $SourceDirectory $relative)) -ne 'synthetic-signed') { throw '编译发生在包内签名之前' }
}
[IO.File]::WriteAllText((Join-Path $OutputDirectory 'setup.exe'), 'synthetic-unsigned')
$global:LASTEXITCODE = 0
'@)
    $stage = Join-Path $root 'stage'
    foreach ($name in @('server_exe', 'tsf_dll')) { New-Item -ItemType Directory -Path (Join-Path $stage $name) -Force | Out-Null }
    foreach ($relative in @('server_exe/app.exe', 'tsf_dll/host.dll')) { [IO.File]::WriteAllText((Join-Path $stage $relative), 'synthetic-unsigned') }
    [IO.File]::WriteAllText((Join-Path $stage 'server_exe/vendor.dll'), 'synthetic-vendor')
    [IO.File]::WriteAllText((Join-Path $stage 'msime_setup.iss'), 'synthetic')
    $env:MSIME_SYNTHETIC_CALLS = Join-Path $root 'calls.txt'
    $oldRuntime = [Environment]::GetEnvironmentVariable('XDG_RUNTIME_DIR', 'Process')
    $packageOutput = Join-Path $root 'package-output'
    & (Join-Path $root 'Sign-StagedPackage-SimplySign.ps1') -PackageRoot $stage -OutputDirectory $packageOutput -SsignPath $fakeClient -SignToolPath $fakeVerifier
    $calls = @(Get-Content -LiteralPath $env:MSIME_SYNTHETIC_CALLS)
    if ($calls.Count -ne 2 -or $calls[0] -cne $calls[1] -or -not $calls[0]) { throw '包内与安装器未复用同一会话目录' }
    if (Test-Path -LiteralPath $calls[0]) { throw '签名会话没有清理' }
    if ([Environment]::GetEnvironmentVariable('XDG_RUNTIME_DIR', 'Process') -cne $oldRuntime) { throw '调用方运行目录未还原' }
    if ($env:CERTUM_OTP -or $env:CERTUM_TOKEN) { throw '整包签名后仍保留凭据' }
    if ([IO.File]::ReadAllText((Join-Path $stage 'server_exe/vendor.dll')) -ne 'synthetic-vendor') { throw '上游可信签名被覆盖' }
    $expectedHash = (Get-FileHash -LiteralPath (Join-Path $packageOutput 'setup.exe') -Algorithm SHA256).Hash.ToLowerInvariant()
    if ((Get-Content -LiteralPath (Join-Path $packageOutput 'SHA256SUMS') -Raw).Trim() -cne "$expectedHash  setup.exe") { throw '摘要没有基于最终签名安装器计算' }
    $global:LASTEXITCODE = 0
    Write-Host 'SimplySign EXE/DLL 签名行为测试通过'
} finally {
    Remove-Item Function:\Get-AuthenticodeSignature -ErrorAction SilentlyContinue
    foreach ($name in @('MSIME_SYNTHETIC_ARGUMENTS', 'MSIME_SYNTHETIC_CALLS', 'MSIME_SYNTHETIC_SIGN_FAILURE', 'MSIME_SYNTHETIC_VERIFY_FAILURE', 'MSIME_SYNTHETIC_NO_TIMESTAMP')) {
        Remove-Item "Env:$name" -ErrorAction SilentlyContinue
    }
    Remove-Item Env:CERTUM_EMAIL -ErrorAction SilentlyContinue
    Remove-Item Env:CERTUM_OTP -ErrorAction SilentlyContinue
    Remove-Item Env:CERTUM_TOKEN -ErrorAction SilentlyContinue
    Remove-Item -LiteralPath $root -Recurse -Force -ErrorAction SilentlyContinue
}
