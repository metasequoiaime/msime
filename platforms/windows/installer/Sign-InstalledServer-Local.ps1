# 一键修复“已安装”的本地快速构建：停进程 → 本机自签名 → 补齐缺失的运行时 DLL。
# 解决两类只在本地快速部署（Build-Client.ps1 + 手动拷贝，跳过了 test.ps1 的签名/打包）
# 才会撞上的问题：
#   1. uiAccess=true 的 Server 未签名 → 系统拒绝启动（现象：只能打英文）。
#   2. Server 导入了某个 debug 命名的依赖（如 libcurl-d.dll）但安装目录只有 release 版，
#      导致“找不到 DLL”。CMakeLists 的 RelWithDebInfo→Release 映射已从源头修掉这点；
#      本脚本对“修复前构建出来的” Server 仍做一次 release→debug 别名兜底。
#
# 必须用“管理员 PowerShell”运行（要写本机受信任根，并改写 Program Files）：
#   pwsh -File .\Sign-InstalledServer-Local.ps1
#
# 签名复用与 Sign-PackageBinaries-Local.ps1 相同的本机自签测试证书（只在本机受信任，
# 不可对外分发）。幂等：已签过的文件、已存在的依赖都会跳过。

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

# full 的安装目录（版本表 platforms.windows.install_dir）。不带后缀的 metasequoiaime 是 msime-windows 的，这个脚本会按映像名停进程、改写目录里的文件，不能指向它。
$InstallDir = 'C:\Program Files\metasequoiaime-full\server'
$SignTargets = @('MetasequoiaImeServer.exe', 'MetasequoiaImeWatchdog.exe', 'MetasequoiaImeTsf.dll')
$StopProcesses = @('MetasequoiaImeServer', 'MetasequoiaImeWatchdog',
                   'MetasequoiaImeEmojiPanel', 'MetasequoiaImeKeyboardPanel')
$CertificateSubject = 'CN=Metasequoia IME Local Test Code Signing'
$CodeSigningEku = '1.3.6.1.5.5.7.3.3'
$TrustStores = @('Cert:\LocalMachine\Root', 'Cert:\LocalMachine\TrustedPublisher')

function Test-Administrator {
    $id = [System.Security.Principal.WindowsIdentity]::GetCurrent()
    return ([System.Security.Principal.WindowsPrincipal]::new($id)).IsInRole(
        [System.Security.Principal.WindowsBuiltInRole]::Administrator)
}
if (-not (Test-Administrator)) {
    throw '需要管理员 PowerShell：uiAccess 签名要装进本机受信任根，且要改写 Program Files。'
}
if (-not (Test-Path -LiteralPath $InstallDir)) { throw "安装目录不存在：$InstallDir" }

function Find-SignTool {
    $kitsBin = Join-Path ${env:ProgramFiles(x86)} 'Windows Kits\10\bin'
    $c = Get-ChildItem -LiteralPath $kitsBin -Directory -ErrorAction SilentlyContinue |
        Sort-Object { try { [version]$_.Name } catch { [version]'0.0' } } -Descending |
        ForEach-Object { Join-Path $_.FullName 'x64\signtool.exe' } |
        Where-Object { Test-Path -LiteralPath $_ -PathType Leaf } | Select-Object -First 1
    if (-not $c) { throw '找不到 signtool.exe；请安装 Windows SDK。' }
    return $c
}

function Get-OrCreateCert {
    $now = Get-Date
    $cert = Get-ChildItem 'Cert:\CurrentUser\My' | Where-Object {
        $_.Subject -eq $CertificateSubject -and $_.HasPrivateKey -and
        $_.NotBefore -le $now -and $_.NotAfter -gt $now -and
        ($_.EnhancedKeyUsageList.ObjectId -contains $CodeSigningEku)
    } | Sort-Object NotAfter -Descending | Select-Object -First 1
    if ($cert) { Write-Host "复用已有测试证书：$($cert.Thumbprint)"; return $cert }
    $cert = New-SelfSignedCertificate -Type CodeSigningCert -Subject $CertificateSubject `
        -FriendlyName 'Metasequoia IME Local Test Code Signing' -KeyAlgorithm RSA -KeyLength 3072 `
        -HashAlgorithm SHA256 -KeyExportPolicy Exportable -KeyUsage DigitalSignature `
        -CertStoreLocation 'Cert:\CurrentUser\My' -NotAfter (Get-Date).AddYears(5)
    Write-Host "已创建测试证书：$($cert.Thumbprint)"
    return $cert
}

function Add-CertificateTrust($cert) {
    $tmp = Join-Path ([System.IO.Path]::GetTempPath()) ("msime-{0}.cer" -f $cert.Thumbprint)
    Export-Certificate -Cert $cert -FilePath $tmp -Type CERT -Force | Out-Null
    try {
        foreach ($store in $TrustStores) {
            if (-not (Test-Path (Join-Path $store $cert.Thumbprint))) {
                Import-Certificate -FilePath $tmp -CertStoreLocation $store | Out-Null
                Write-Host "已信任：$store"
            }
        }
    } finally { Remove-Item $tmp -Force -ErrorAction SilentlyContinue }
}

# —— 1. 停进程（否则正在运行的 Watchdog/Server 会锁住文件，签名会失败）——
Write-Host '== 停止正在运行的 水杉输入法 进程 =='
foreach ($name in $StopProcesses) {
    Get-Process -Name $name -ErrorAction SilentlyContinue | ForEach-Object {
        Write-Host "  停止 $($_.ProcessName) (PID=$($_.Id))"
        Stop-Process -Id $_.Id -Force -ErrorAction SilentlyContinue
    }
}
Start-Sleep -Seconds 1

# —— 2. 补齐缺失的运行时依赖（release→debug 别名兜底）——
# 对每个 Server 导入、但安装目录缺失的 “<名>-d.dll”，若存在 release 版 “<名>.dll”，
# 就复制一份作为别名。两者 ABI 相同（同一库同一版本），仅内部构建差异。
Write-Host '== 补齐运行时依赖 =='
$serverExe = Join-Path $InstallDir 'MetasequoiaImeServer.exe'
if (Test-Path -LiteralPath $serverExe) {
    $ascii = [System.Text.Encoding]::ASCII.GetString([System.IO.File]::ReadAllBytes($serverExe))
    $referenced = [regex]::Matches($ascii, '[A-Za-z0-9_.\-]+\.dll') |
        ForEach-Object { $_.Value } | Sort-Object -Unique
    $patched = 0
    foreach ($dll in $referenced) {
        if ($dll -notmatch '-d\.dll$') { continue }
        if (Test-Path -LiteralPath (Join-Path $InstallDir $dll)) { continue }
        $release = ($dll -replace '-d\.dll$', '.dll')
        $releasePath = Join-Path $InstallDir $release
        if (Test-Path -LiteralPath $releasePath) {
            Copy-Item -LiteralPath $releasePath -Destination (Join-Path $InstallDir $dll) -Force
            Write-Warning "兜底：$release -> $dll（请重建以让 Server 直接链 $release）"
            $patched++
        } else {
            Write-Warning "缺失依赖 $dll，且无同名 release 版可兜底：$releasePath"
        }
    }
    if ($patched -eq 0) { Write-Host '  依赖齐全，无需兜底。' }
} else {
    Write-Warning "未找到 $serverExe，跳过依赖检查。"
}

# —— 3. 签名 ——
Write-Host '== 签名 =='
$targets = @($SignTargets | ForEach-Object { Join-Path $InstallDir $_ } |
    Where-Object { Test-Path -LiteralPath $_ })
if ($targets.Count -eq 0) { throw "安装目录里没找到待签名目标：$InstallDir" }
$cert = Get-OrCreateCert
Add-CertificateTrust $cert
$signTool = Find-SignTool
$targets | ForEach-Object { Write-Host "  $_" }
& $signTool sign /sha1 $cert.Thumbprint /s My /fd sha256 /v @($targets)
if ($LASTEXITCODE -ne 0) { throw "signtool 签名失败，退出码 $LASTEXITCODE" }

# —— 4. 验证 ——
Write-Host '== 验证签名链 =='
$allValid = $true
foreach ($t in $targets) {
    $s = Get-AuthenticodeSignature -LiteralPath $t
    Write-Host ("  {0,-30} {1}" -f (Split-Path $t -Leaf), $s.Status)
    if ($s.Status -ne 'Valid') { $allValid = $false }
}
Write-Host ''
if ($allValid) {
    Write-Host '完成。切到水杉输入法试打中文；若仍为英文，注销再登录让 TextInputHost 重载。'
} else {
    Write-Warning '有文件签名链未通过，请检查上面的输出。'
}
$global:LASTEXITCODE = 0
