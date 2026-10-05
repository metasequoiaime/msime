$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$script = Get-Content -LiteralPath (Join-Path $PSScriptRoot '../msime_setup.iss') -Raw
$script = $script -replace '\\\r?\n\s*', ' '
$records = [regex]::Matches($script, '(?m)^Source:[^\r\n]*')
$registrations = @($records | Where-Object { $_.Value -match '\bregserver\b' })
if ($registrations.Count -ne 2) { throw 'Expected exactly two TSF registrations' }
foreach ($arch in @('32', '64')) {
    $tip = @($registrations | Where-Object { $_.Value.Contains("\tsf_dll\$arch\MetasequoiaImeTsf.dll") })
    $hostDll = @($records | Where-Object { $_.Value.Contains("\tsf_dll\$arch\*.dll") -and $_.Value.Contains('{code:GetVersionDir}') })
    if ($tip.Count -ne 1 -or $hostDll.Count -ne 1 -or $hostDll[0].Value -match '\bregserver\b' -or
        $hostDll[0].Index -gt $tip[0].Index -or
        -not $hostDll[0].Value.Contains('Excludes: "MetasequoiaImeTsf.dll"') -or
        -not $hostDll[0].Value.Contains("{commonpf$arch}\{#MyEditionInstallDir}\{code:GetVersionDir}")) {
        throw 'TSF dependency installation/registration contract mismatch'
    }
}
# The Server folder takes the x64 host DLL and runtime DLLs from the tsf_dll\64 source the TIP uses, so the package stores them once; server_exe no longer carries its own copy.
$serverShared = @($records | Where-Object { $_.Value.Contains('\tsf_dll\64\*.dll') -and $_.Value.Contains('{commonpf64}\{#MyEditionInstallDir}\server"') })
if ($serverShared.Count -ne 1 -or $serverShared[0].Value -match '\bregserver\b' -or
    -not $serverShared[0].Value.Contains('Excludes: "MetasequoiaImeTsf.dll"')) {
    throw 'Server folder does not install the shared x64 host and runtime DLLs from tsf_dll\64'
}
# Symbols are published as a separate release asset, never installed.
if (@($records | Where-Object { $_.Value -match '\.pdb"' }).Count -ne 0) {
    throw 'Installer installs PDB files'
}
Write-Output 'TSF dependencies install without COM registration before matching TIPs'
