# Launch the installer built in this directory's Output folder.
param([switch]$Light)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$issPath = Join-Path $PSScriptRoot 'msime_setup.iss'
if (-not (Test-Path -LiteralPath $issPath -PathType Leaf)) { throw "Installer script not found: $issPath" }
$issContent = Get-Content -LiteralPath $issPath -Raw -Encoding UTF8
if ($issContent -notmatch '(?m)^#define\s+MyAppVersion\s+"(?<version>[0-9][0-9A-Za-z.+-]*)"') {
    throw 'Could not find MyAppVersion in msime_setup.iss.'
}

$suffix = if ($Light) { '_light' } else { '' }
$installerPath = Join-Path $PSScriptRoot "Output\MetasequoiaIME-Full_Setup_v$($Matches.version)$suffix.exe"
if (-not (Test-Path -LiteralPath $installerPath -PathType Leaf)) {
    throw "Installer does not exist; run Compile-Installer.ps1 first: $installerPath"
}
& $installerPath
