$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$script = Get-Content -LiteralPath (Join-Path $PSScriptRoot '../msime_setup.iss') -Raw
$script = $script -replace '\\\r?\n\s*', ' '
$records = [regex]::Matches($script, '(?m)^Source:[^\r\n]*')
$registrations = @($records | Where-Object { $_.Value -match '\bregserver\b' })
if ($registrations.Count -ne 2) { throw 'Expected exactly two TSF registrations' }
foreach ($arch in @('32', '64')) {
    $tip = @($registrations | Where-Object { $_.Value.Contains("\tsf_dll\$arch\MetasequoiaImeTsf.dll") })
    $hostDll = @($records | Where-Object { $_.Value.Contains("\tsf_dll\$arch\*.dll") })
    if ($tip.Count -ne 1 -or $hostDll.Count -ne 1 -or $hostDll[0].Value -match '\bregserver\b' -or
        $hostDll[0].Index -gt $tip[0].Index -or
        -not $hostDll[0].Value.Contains('Excludes: "MetasequoiaImeTsf.dll"') -or
        -not $hostDll[0].Value.Contains("{commonpf$arch}\{#MyEditionInstallDir}\{code:GetVersionDir}")) {
        throw 'TSF dependency installation/registration contract mismatch'
    }
}
Write-Output 'TSF dependencies install without COM registration before matching TIPs'
