# Builds the Rust half of the Windows client for one architecture and collects it under fixed names in OutputDirectory/<arch>. None of it depends on the edition (editions rename the host DLL afterwards, Build-Client.ps1), so a release builds it once and hands the result to every edition's Build-Client.ps1 -RustOutputs. Without -RustOutputs, Build-Client.ps1 runs this itself for all three architectures. This script never signs or installs.
#
# x64: msime_host_api.dll with its import library and PDB, msime-mcp.exe and its PDB, and the Tauri panel shell as MSIME.exe and MSIME.pdb. The x64 host is built with `onnx-rerank`: the minimal ONNX Runtime static library pinned in resources/onnxruntime-minimal.lock.json (scripts/fetch_onnxruntime.py downloads and verifies it, so Python must be on PATH) is linked into it, and the Server's sessions run 整句联想「增强」's keystroke reranker on it when the installer put rerank-model and rerank-model-small beside the Server. The TIP loads the same DLL but its sessions never mount the reranker (tsf/HostConfiguration.cpp), which is why x86 and arm64, only ever loaded by the TIP, are built without it.
# x86: the host DLL, import library and PDB, for the 32-bit TIP.
# arm64: the host DLL and PDB, with the C runtime linked statically like the TIP, for the native half of the Arm64X TIP. ring assembles its ARM64 code with clang only, so clang must be on PATH.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidateSet('x64', 'x86', 'arm64')][string]$Architecture,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [string]$RepoRoot = (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)),
    [string]$TargetVersion = ''
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

if ($TargetVersion -ne '' -and
    ($TargetVersion -notmatch '^(0|[1-9][0-9]{0,4})\.(0|[1-9][0-9]{0,4})\.(0|[1-9][0-9]{0,4})$' -or
     @($TargetVersion.Split('.') | Where-Object { [int]$_ -gt 65535 }).Count -ne 0)) {
    throw 'TargetVersion must have three numeric components between 0 and 65535 without leading zeros'
}
if (-not [IO.Path]::IsPathRooted($OutputDirectory)) { throw 'OutputDirectory must be an absolute path' }

function Invoke-RustBuild {
    param([string]$Command, [string[]]$Arguments)
    & $Command @Arguments
    if ($LASTEXITCODE -ne 0) { throw "Client build command failed: $Command ($LASTEXITCODE)" }
}

# Rust and the toolchain can name a binary's PDB after the normalized crate name; take the one that exists and refuse stale or ambiguous symbols.
function Get-SinglePdb {
    param([string]$Release, [string[]]$Names, [string]$What)
    $found = @($Names | ForEach-Object { Join-Path $Release $_ } | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf })
    if ($found.Count -ne 1) { throw "Expected one $What PDB output" }
    $found[0]
}

$RepoRoot = (Resolve-Path -LiteralPath $RepoRoot).Path
$triple = switch ($Architecture) { 'x64' { 'x86_64-pc-windows-msvc' } 'x86' { 'i686-pc-windows-msvc' } 'arm64' { 'aarch64-pc-windows-msvc' } }
$output = Join-Path $OutputDirectory $Architecture
$previousTarget = $env:CARGO_TARGET_DIR
$previousDebug = $env:CARGO_PROFILE_RELEASE_DEBUG
$previousArm64Flags = $env:CARGO_TARGET_AARCH64_PC_WINDOWS_MSVC_RUSTFLAGS
$previousVersion = $env:MSIME_VERSION
$previousPlatformBuild = $env:MSIME_PLATFORM_BUILD
$previousOrtLocation = $env:ORT_LIB_LOCATION
Push-Location $RepoRoot
try {
    $env:CARGO_TARGET_DIR = Join-Path $RepoRoot 'target'
    $env:CARGO_PROFILE_RELEASE_DEBUG = '2'
    if ($Architecture -eq 'arm64') { $env:CARGO_TARGET_AARCH64_PC_WINDOWS_MSVC_RUSTFLAGS = '-C target-feature=+crt-static' }
    $release = Join-Path $env:CARGO_TARGET_DIR "$triple/release"
    New-Item -ItemType Directory -Force -Path $output | Out-Null
    $hostBuild = @('build', '--locked', '--release', '--target', $triple, '-p', 'msime-host-api')
    if ($Architecture -eq 'x64') {
        # The minimal ONNX Runtime for 整句联想「增强」's keystroke reranker (.agents/notes/implemented/architecture/2026-11-01-onnx-rerank-backend.md). The x64 library is built against the DLL C runtime (/MD) like this host. The script prints the directory ORT_LIB_LOCATION names; its progress goes to stderr.
        $ortLocation = @(& python (Join-Path $RepoRoot 'scripts/fetch_onnxruntime.py') --target $triple)
        if ($LASTEXITCODE -ne 0) { throw "Client build command failed: python ($LASTEXITCODE)" }
        if ($ortLocation.Count -lt 1 -or [string]::IsNullOrWhiteSpace([string]$ortLocation[-1])) { throw "fetch_onnxruntime.py printed no library directory for $triple" }
        $env:ORT_LIB_LOCATION = [string]$ortLocation[-1]
        # ort-sys copies the library into its own build directory and never copies it again when it changes; stamp the locked library digest and clean ort-sys when it moves.
        $ortLock = Get-Content -LiteralPath (Join-Path $RepoRoot 'resources/onnxruntime-minimal.lock.json') -Raw -Encoding UTF8 | ConvertFrom-Json
        $ortDigest = [string]$ortLock.targets.$triple.library_sha256
        if ($ortDigest -notmatch '^[0-9a-f]{64}$') { throw "No locked ONNX Runtime library for $triple" }
        $ortStamp = Join-Path $env:CARGO_TARGET_DIR "$triple/onnxruntime-library.sha256"
        $ortStamped = if (Test-Path -LiteralPath $ortStamp -PathType Leaf) { (Get-Content -LiteralPath $ortStamp -Raw).Trim() } else { '' }
        if ((Test-Path -LiteralPath $release -PathType Container) -and $ortStamped -ne $ortDigest) {
            Invoke-RustBuild cargo @('clean', '-p', 'ort-sys', '--release', '--target', $triple)
        }
        $hostBuild += @('--features', 'onnx-rerank,local-translation')
    }
    Invoke-RustBuild cargo $hostBuild
    if ($Architecture -eq 'x64') {
        [IO.File]::WriteAllText($ortStamp, "$ortDigest`n")
    }
    # Take the host and its PDB now: the MCP and desktop builds below share this target directory and can rebuild host-api, rewriting the PDB under the same name. The copy keeps the file name the DLL records; Collect-Symbols.ps1 packs it.
    $hostFiles = @('msime_host_api.dll', 'msime_host_api.pdb')
    if ($Architecture -ne 'arm64') { $hostFiles += 'msime_host_api.dll.lib' }
    Invoke-RustBuild cmake (@('-E', 'copy_if_different') + @($hostFiles | ForEach-Object { Join-Path $release $_ }) + @($output))
    if ($Architecture -eq 'x64') {
        # msime-mcp --version 和 MCP 握手报告的版本（crates/mcp-server/build.rs）；没给 TargetVersion 时它读 platforms/windows/version.txt。
        if ($TargetVersion -ne '') { $env:MSIME_VERSION = $TargetVersion }
        Invoke-RustBuild cargo @('build', '--locked', '--release', '--target', $triple, '-p', 'msime-mcp-server', '--bin', 'msime-mcp')
        Invoke-RustBuild cmake @('-E', 'copy_if_different', (Join-Path $release 'msime-mcp.exe'), $output)
        $mcpPdb = Get-SinglePdb $release @('msime_mcp.pdb', 'msime-mcp.pdb') 'MCP server'
        Invoke-RustBuild cmake @('-E', 'copy_if_different', $mcpPdb, (Join-Path $output 'msime-mcp.pdb'))

        Invoke-RustBuild pnpm @('install', '--frozen-lockfile')
        Invoke-RustBuild pnpm @('--filter', '@msime/desktop', 'typecheck')
        $desktopBuild = @('--filter', '@msime/desktop', 'tauri', 'build', '--no-bundle', '--target', $triple)
        if ($TargetVersion -ne '') {
            # Pass a file rather than inline JSON: pnpm is a .cmd shim on Windows, and PowerShell hands batch files their arguments without escaping the embedded quotes.
            $versionConfig = Join-Path $output 'tauri-version.json'
            [IO.File]::WriteAllText($versionConfig, (@{ version = $TargetVersion } | ConvertTo-Json -Compress))
            $desktopBuild += @('--config', $versionConfig)
        }
        # apps/desktop/scripts/require-platform-build.mjs refuses a Tauri build that no platform script asked for.
        $env:MSIME_PLATFORM_BUILD = '1'
        Invoke-RustBuild pnpm $desktopBuild
        Invoke-RustBuild cmake @('-E', 'copy_if_different', (Join-Path $release 'msime-desktop.exe'), (Join-Path $output 'MSIME.exe'))
        $desktopPdb = Get-SinglePdb $release @('msime_desktop.pdb', 'msime-desktop.pdb') 'Tauri desktop'
        Invoke-RustBuild cmake @('-E', 'copy_if_different', $desktopPdb, (Join-Path $output 'MSIME.pdb'))
    }
} finally {
    $env:CARGO_TARGET_DIR = $previousTarget
    $env:CARGO_PROFILE_RELEASE_DEBUG = $previousDebug
    $env:CARGO_TARGET_AARCH64_PC_WINDOWS_MSVC_RUSTFLAGS = $previousArm64Flags
    $env:MSIME_VERSION = $previousVersion
    $env:MSIME_PLATFORM_BUILD = $previousPlatformBuild
    $env:ORT_LIB_LOCATION = $previousOrtLocation
    Pop-Location
}
