$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$script = Get-Content -LiteralPath (Join-Path $PSScriptRoot '../msime_setup.iss') -Raw
$script = $script -replace '\\\r?\n\s*', ' '
$records = [regex]::Matches($script, '(?m)^Source:[^\r\n]*')
$registrations = @($records | Where-Object { $_.Value -match '\bregserver\b' })
if ($registrations.Count -ne 3) { throw 'Expected exactly three TSF registrations' }
# 安装到哪个 Program Files，以及在什么机器上装：x64 TIP 只在不是 Windows on Arm 的机器上装，Arm64X TIP 只在 Windows on Arm 上装，两者落在同一个版本目录、注册同一个 CLSID。
foreach ($tsf in @(@('32', '32', ''), @('64', '64', 'Check: not IsArm64'), @('arm64', '64', 'Check: IsArm64'))) {
    $arch, $folder, $check = $tsf
    $tip = @($registrations | Where-Object { $_.Value.Contains("\tsf_dll\$arch\MetasequoiaImeTsf.dll") })
    $hostDll = @($records | Where-Object { $_.Value.Contains("\tsf_dll\$arch\*.dll") -and $_.Value.Contains('{code:GetVersionDir}') })
    if ($tip.Count -ne 1 -or $hostDll.Count -ne 1 -or $hostDll[0].Value -match '\bregserver\b' -or
        $hostDll[0].Index -gt $tip[0].Index -or
        -not $hostDll[0].Value.Contains('Excludes: "MetasequoiaImeTsf.dll"') -or
        -not $hostDll[0].Value.Contains("{commonpf$folder}\{#MyEditionInstallDir}\{code:GetVersionDir}") -or
        -not $tip[0].Value.Contains("{commonpf$folder}\{#MyEditionInstallDir}\{code:GetVersionDir}")) {
        throw "TSF dependency installation/registration contract mismatch for $arch"
    }
    if ($check -and (-not $tip[0].Value.EndsWith($check) -or ($arch -eq 'arm64' -and -not $hostDll[0].Value.EndsWith($check)))) {
        throw "TSF $arch is not limited to its machines ($check)"
    }
    if (-not $check -and $tip[0].Value -match '\bCheck:') { throw "TSF $arch is limited to some machines" }
}
# Server 目录的 x64 宿主 DLL 和运行时 DLL 取自 TIP 所用的 tsf_dll\64 源文件，包里只存一份；server_exe 不再自带一份。
$serverShared = @($records | Where-Object { $_.Value.Contains('\tsf_dll\64\*.dll') -and $_.Value.Contains('{commonpf64}\{#MyEditionInstallDir}\server"') })
if ($serverShared.Count -ne 1 -or $serverShared[0].Value -match '\bregserver\b' -or
    -not $serverShared[0].Value.Contains('Excludes: "MetasequoiaImeTsf.dll"')) {
    throw 'Server folder does not install the shared x64 host and runtime DLLs from tsf_dll\64'
}
# 符号作为单独的发布资产发布，从不安装。
if (@($records | Where-Object { $_.Value -match '\.pdb"' }).Count -ne 0) {
    throw 'Installer installs PDB files'
}
Write-Output 'TSF dependencies install without COM registration before matching TIPs'
