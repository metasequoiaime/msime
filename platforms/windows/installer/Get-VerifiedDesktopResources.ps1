[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$SourceDirectory,
    [Parameter(Mandatory)][string]$ManifestPath
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$manifest = Get-Content -LiteralPath $ManifestPath -Raw -Encoding UTF8 | ConvertFrom-Json
if ($manifest.source_commit -notmatch '^[a-f0-9]{40}$' -or @($manifest.artifacts).Count -eq 0) {
    throw 'Invalid pinned desktop resource manifest'
}
$names = [Collections.Generic.HashSet[string]]::new([StringComparer]::OrdinalIgnoreCase)
$verified = @(
    foreach ($artifact in $manifest.artifacts) {
        $name = [string]$artifact.name
        if ($name -notmatch '^[A-Za-z0-9][A-Za-z0-9_.-]*$' -or
            -not $names.Add($name) -or $artifact.sha256 -notmatch '^[a-f0-9]{64}$' -or
            $artifact.size -le 0) {
            throw 'Invalid pinned desktop resource entry'
        }
        $path = Join-Path $SourceDirectory $name
        $file = Get-Item -LiteralPath $path -Force
        if ($file.PSIsContainer -or ($file.Attributes -band [IO.FileAttributes]::ReparsePoint) -or
            $file.Length -ne $artifact.size -or
            (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash -ne $artifact.sha256) {
            throw "Desktop resource verification failed: $name"
        }
        $file.FullName
    }
)
# Emit nothing until every artifact passes; callers must not stage partial sets.
$verified
