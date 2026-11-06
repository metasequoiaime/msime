# Cargo/libgit2 检出 llama.cpp 的长路径时不受 Git core.longpaths 控制；通过短目录联接复用缓存。
[CmdletBinding()]
param(
    [string]$ShortHome = (Join-Path $env:SystemDrive 'mc'),
    [string]$CargoHome = $(if ($env:CARGO_HOME) { $env:CARGO_HOME } else { Join-Path $env:USERPROFILE '.cargo' }),
    [string]$EnvironmentFile = $env:GITHUB_ENV
)
$ErrorActionPreference = 'Stop'
$source = [IO.Path]::GetFullPath($CargoHome).TrimEnd('\')
$destination = [IO.Path]::GetFullPath($ShortHome).TrimEnd('\')
if ($source -ne $destination) {
    $existing = Get-Item -LiteralPath $destination -Force -ErrorAction SilentlyContinue
    if ($existing) {
        if ($existing.LinkType -ne 'Junction' -or
            [IO.Path]::GetFullPath([string]$existing.Target).TrimEnd('\') -ne $source) {
            throw 'Short Cargo home already exists and does not refer to this cache'
        }
    } else {
        New-Item -ItemType Directory -Path $source -Force | Out-Null
        New-Item -ItemType Junction -Path $destination -Target $source | Out-Null
    }
}
$env:CARGO_HOME = $destination
if ($EnvironmentFile) {
    "CARGO_HOME=$destination" | Out-File -LiteralPath $EnvironmentFile -Encoding utf8 -Append
}
