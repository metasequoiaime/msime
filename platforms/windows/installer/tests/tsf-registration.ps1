$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$script = Get-Content -LiteralPath (Join-Path $PSScriptRoot '../msime_setup.iss') -Raw -Encoding UTF8
$script = $script -replace '\\\r?\n\s*', ' '
$records = [regex]::Matches($script, '(?m)^Source:[^\r\n]*')
$registrations = @($records | Where-Object { $_.Value -match '\bregserver\b' })
if ($registrations.Count -ne 3) { throw 'Expected exactly three TSF registrations' }
# 安装到哪里，以及在什么机器上装：32 位 TIP 装进 Program Files (x86)；64 位 TIP 装进 System32\IME 下本版本自己的目录，CS2 的 Trusted Mode 只放行系统目录里签了名的外来 DLL。x64 TIP 只在不是 Windows on Arm 的机器上装，Arm64X TIP 只在 Windows on Arm 上装，两者落在同一个版本目录、注册同一个 CLSID。
foreach ($tsf in @(@('32', '{commonpf32}\{#MyEditionInstallDir}\{code:GetVersionDir}', ''), @('64', '{#MySystemTipDir}\{code:GetVersionDir}', 'Check: not IsArm64'), @('arm64', '{#MySystemTipDir}\{code:GetVersionDir}', 'Check: IsArm64'))) {
    $arch, $destination, $check = $tsf
    $tip = @($registrations | Where-Object { $_.Value.Contains("\tsf_dll\$arch\MetasequoiaImeTsf.dll") })
    $hostDll = @($records | Where-Object { $_.Value.Contains("\tsf_dll\$arch\*.dll") -and $_.Value.Contains('{code:GetVersionDir}') })
    if ($tip.Count -ne 1 -or $hostDll.Count -ne 1 -or $hostDll[0].Value -match '\bregserver\b' -or
        $hostDll[0].Index -gt $tip[0].Index -or
        -not $hostDll[0].Value.Contains('Excludes: "MetasequoiaImeTsf.dll"') -or
        -not $hostDll[0].Value.Contains("DestDir: `"$destination`"") -or
        -not $tip[0].Value.Contains("DestDir: `"$destination`"")) {
        throw "TSF dependency installation/registration contract mismatch for $arch"
    }
    # 系统目录里的副本被占用时，卸载要把它排到重启后删除，否则会一直留在 System32 里。
    $inSystem = $destination.StartsWith('{#MySystemTipDir}')
    if ($inSystem -ne ($tip[0].Value -match '\buninsrestartdelete\b') -or $inSystem -ne ($hostDll[0].Value -match '\buninsrestartdelete\b')) {
        throw "TSF $arch system-directory copies are not (or Program Files copies are) deleted at restart when in use"
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
# 系统目录副本放在 System32\IME 下本版本自己的子目录里：各版本的 TIP 同名，运行时 DLL 是通用名字，直接放进 System32 会互相覆盖。
if (-not [regex]::IsMatch($script, '(?m)^#define MySystemTipDir "\{sys\}\\IME\\" \+ MyEditionInstallDir\s*$')) {
    throw 'The 64-bit TIP is not installed under System32\IME\<install_dir>'
}
Write-Output 'TSF dependencies install without COM registration before matching TIPs'
