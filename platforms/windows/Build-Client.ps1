# Run from a Windows MSVC build environment with both Rust MSVC targets and
# prebuilt native dependency prefixes. This script never signs or installs.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$X64Dependencies,
    [Parameter(Mandatory)][string]$X86Dependencies,
    [string]$RepoRoot = (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)),
    [string]$Generator = 'Visual Studio 17 2022',
    [string]$TargetVersion = ''
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

# Keep direct development builds on the checked-in Tauri version. Installation
# builds supply one numeric release version for packaging and desktop metadata.
if ($TargetVersion -ne '' -and
    ($TargetVersion -notmatch '^(0|[1-9][0-9]{0,4})\.(0|[1-9][0-9]{0,4})\.(0|[1-9][0-9]{0,4})$' -or
     @($TargetVersion.Split('.') | Where-Object { [int]$_ -gt 65535 }).Count -ne 0)) {
    throw 'TargetVersion must have three numeric components between 0 and 65535 without leading zeros'
}

function Invoke-ClientBuild {
    param([string]$Command, [string[]]$Arguments)
    & $Command @Arguments
    if ($LASTEXITCODE -ne 0) { throw "Client build command failed: $Command ($LASTEXITCODE)" }
}

foreach ($prefix in @($X64Dependencies, $X86Dependencies)) {
    if (-not [IO.Path]::IsPathRooted($prefix) -or -not (Test-Path -LiteralPath $prefix -PathType Container)) {
        throw 'Provide absolute, existing x64 and x86 native dependency prefixes'
    }
}
$RepoRoot = (Resolve-Path -LiteralPath $RepoRoot).Path
foreach ($relative in @('Cargo.toml', 'crates/engine/Cargo.toml',
                         'platforms/windows/CMakeLists.txt', 'platforms/windows/tsf/CMakeLists.txt',
                         'platforms/windows/settings/MSIME.Settings.vcxproj',
                         'apps/desktop/package.json', 'scripts/fetch_voice_runtime.py',
                         'scripts/fetch_handwriting_model.py')) {
    if (-not (Test-Path -LiteralPath (Join-Path $RepoRoot $relative) -PathType Leaf)) {
        throw "Missing Client build source: $relative"
    }
}
# The on-device speech runtime staged beside the Server; Prepare-PackageFiles.ps1 and install-smoke.ps1 name the same three files.
$voiceRuntimeLibraries = @('sherpa-onnx-c-api.dll', 'onnxruntime.dll', 'onnxruntime_providers_shared.dll')
$previousPrefix = $env:CMAKE_PREFIX_PATH
$previousTarget = $env:CARGO_TARGET_DIR
$previousDebug = $env:CARGO_PROFILE_RELEASE_DEBUG
Push-Location $RepoRoot
try {
    $env:CARGO_TARGET_DIR = Join-Path $RepoRoot 'target'
    $env:CARGO_PROFILE_RELEASE_DEBUG = '2'
    foreach ($arch in @('x64', 'x86')) {
        $triple = if ($arch -eq 'x64') { 'x86_64-pc-windows-msvc' } else { 'i686-pc-windows-msvc' }
        $platform = if ($arch -eq 'x64') { 'x64' } else { 'Win32' }
        $env:CMAKE_PREFIX_PATH = if ($arch -eq 'x64') { $X64Dependencies } else { $X86Dependencies }
        $release = Join-Path $env:CARGO_TARGET_DIR "$triple/release"
        $output = Join-Path $RepoRoot "target/windows-full/$arch"
        $bin = Join-Path $output 'bin'
        Invoke-ClientBuild cargo @('build', '--locked', '--release', '--target', $triple, '-p', 'msime-host-api')
        $source = if ($arch -eq 'x64') { 'platforms/windows' } else { 'platforms/windows/tsf' }
        $configure = @('-S', (Join-Path $RepoRoot $source), '-B', $output,
            '-G', $Generator, '-A', $platform,
            "-DCMAKE_PREFIX_PATH=$($env:CMAKE_PREFIX_PATH)",
            "-DMSIME_HOST_LIBRARY=$(Join-Path $release 'msime_host_api.dll.lib')",
            "-DCMAKE_RUNTIME_OUTPUT_DIRECTORY_RELWITHDEBINFO=$bin",
            '-DMSIMEUI_BUILD_HANDWRITING_DEMO=OFF')
        if ($arch -eq 'x64') { $configure += '-DMSIME_SERVER_UIACCESS=ON' }
        if ($TargetVersion -ne '') { $configure += "-DMSIME_WINDOWS_VERSION=$TargetVersion" }
        Invoke-ClientBuild cmake $configure
        $targets = if ($arch -eq 'x64') {
            @('msime-client-server', 'msime-client-watchdog', 'msime-client-prepare', 'msime-tsf')
        } else { @('msime-tsf') }
        Invoke-ClientBuild cmake (@('--build', $output, '--config', 'RelWithDebInfo', '--parallel', '4', '--target') + $targets)
        Invoke-ClientBuild cmake @('-E', 'copy_if_different', (Join-Path $release 'msime_host_api.dll'), $bin)
        if ($arch -eq 'x64') {
            Invoke-ClientBuild cargo @('build', '--locked', '--release', '--target', $triple,
                '-p', 'msime-mcp-server', '--bin', 'msime-mcp')
            Invoke-ClientBuild cmake @('-E', 'copy_if_different', (Join-Path $release 'msime-mcp.exe'), $bin)
            # As for the desktop below, the PDB can carry the normalized crate name; the package wants it beside the executable under the same name.
            $mcpPdbs = @('msime_mcp.pdb', 'msime-mcp.pdb') |
                ForEach-Object { Join-Path $release $_ } |
                Where-Object { Test-Path -LiteralPath $_ -PathType Leaf }
            if (@($mcpPdbs).Count -ne 1) { throw 'Expected one MCP server PDB output' }
            Invoke-ClientBuild cmake @('-E', 'copy_if_different', @($mcpPdbs)[0], (Join-Path $bin 'msime-mcp.pdb'))

            # Windows settings are a native WinUI 3 app. Keep the shared Tauri
            # shell below for the emoji/handwriting/keyboard panels, but do not
            # use it as the settings product anymore.
            $settingsProject = Join-Path $RepoRoot 'platforms/windows/settings/MSIME.Settings.vcxproj'
            $settingsIntermediate = Join-Path $output 'settings-obj'
            # -restore rather than /t:Restore,Build: restore writes obj\*.nuget.g.targets, where the CppWinRT and Windows App SDK build logic lives, and only a separate evaluation after it imports them. In one evaluation the build skips header generation, which only a previously restored obj directory hides.
            Invoke-ClientBuild msbuild @($settingsProject, '-restore', '/t:Build',
                '/p:Configuration=RelWithDebInfo', '/p:Platform=x64',
                "/p:HostApiLibrary=$(Join-Path $release 'msime_host_api.dll.lib')",
                "/p:OutDir=$bin\", "/p:IntDir=$settingsIntermediate\")
            $settingsPdb = Join-Path $bin 'MSIME.Settings.pdb'
            if (-not (Test-Path -LiteralPath $settingsPdb -PathType Leaf)) {
                throw "Expected one WinUI settings PDB output: $settingsPdb"
            }
            Invoke-ClientBuild cmake @('-E', 'copy_if_different', $settingsPdb,
                (Join-Path $bin 'msime-client-settings.pdb'))
        }
    }
    $env:CMAKE_PREFIX_PATH = $X64Dependencies
    Invoke-ClientBuild pnpm @('install', '--frozen-lockfile')
    Invoke-ClientBuild pnpm @('--filter', '@msime/desktop', 'typecheck')
    $desktopBuild = @('--filter', '@msime/desktop', 'tauri', 'build', '--no-bundle',
        '--target', 'x86_64-pc-windows-msvc')
    if ($TargetVersion -ne '') {
        # Pass a file rather than inline JSON: pnpm is a .cmd shim on Windows, and PowerShell hands batch files their arguments without escaping the embedded quotes.
        $versionConfig = Join-Path $RepoRoot 'target/windows-full/tauri-version.json'
        [IO.File]::WriteAllText($versionConfig, (@{ version = $TargetVersion } | ConvertTo-Json -Compress))
        $desktopBuild += @('--config', $versionConfig)
    }
    Invoke-ClientBuild pnpm $desktopBuild
    Invoke-ClientBuild cmake @('-E', 'copy_if_different',
        (Join-Path $env:CARGO_TARGET_DIR 'x86_64-pc-windows-msvc/release/msime-desktop.exe'),
        (Join-Path $RepoRoot 'target/windows-full/x64/bin/MSIME.exe'))
    # Rust/toolchain output can use the normalized crate name for the PDB.
    # Require one unambiguous symbol file rather than accepting stale symbols.
    $desktopPdbs = @('msime_desktop.pdb', 'msime-desktop.pdb') |
        ForEach-Object { Join-Path $env:CARGO_TARGET_DIR "x86_64-pc-windows-msvc/release/$_" } |
        Where-Object { Test-Path -LiteralPath $_ -PathType Leaf }
    if (@($desktopPdbs).Count -ne 1) { throw 'Expected one Tauri desktop PDB output' }
    Invoke-ClientBuild cmake @('-E', 'copy_if_different', @($desktopPdbs)[0],
        (Join-Path $RepoRoot 'target/windows-full/x64/bin/MSIME.pdb'))
    # The Server recognizes speech on-device through the pinned sherpa-onnx runtime (resources/voice-runtime.lock.json), which it loads with LoadLibrary from its own directory; onnxruntime.dll resolves beside sherpa-onnx-c-api.dll. The fetch verifies the archive's SHA-256 before extracting and reuses a verified copy on later runs. These are upstream MSVC /MD builds, so they need the same VC runtime the installer already requires.
    $voiceRuntime = Join-Path $RepoRoot 'target/voice-runtime/windows-x64'
    Invoke-ClientBuild python @((Join-Path $RepoRoot 'scripts/fetch_voice_runtime.py'),
        '--platform', 'windows-x64', '--out', $voiceRuntime)
    Invoke-ClientBuild cmake (@('-E', 'copy_if_different') +
        @($voiceRuntimeLibraries | ForEach-Object { Join-Path $voiceRuntime $_ }) +
        @((Join-Path $RepoRoot 'target/windows-full/x64/bin')))
    # The offline handwriting model and its LGPL-2.1 licence, pinned by resources/handwriting-model.lock.json. Prepare-PackageFiles.ps1 stages both beside the Server from target/handwriting-model and Collect-Notices.ps1 reads the licence there. The fetch discards anything that does not match the lock and leaves a matching copy alone.
    Invoke-ClientBuild python @((Join-Path $RepoRoot 'scripts/fetch_handwriting_model.py'),
        '--out', (Join-Path $RepoRoot 'target/handwriting-model'))
    foreach ($arch in @('x64', 'x86')) {
        $bin = Join-Path $RepoRoot "target/windows-full/$arch/bin"
        $prefix = if ($arch -eq 'x64') { $X64Dependencies } else { $X86Dependencies }
        & (Join-Path $PSScriptRoot 'Copy-RuntimeDependencies.ps1') `
            -DependencyPrefix $prefix -Destination $bin -Architecture $arch
        foreach ($dll in @('MetasequoiaImeTsf.dll', 'msime_host_api.dll')) {
            & (Join-Path $PSScriptRoot 'Test-PortableExecutable.ps1') -LiteralPath (Join-Path $bin $dll) -Architecture $arch -Kind dll
        }
        if ($arch -eq 'x64') {
            foreach ($dll in $voiceRuntimeLibraries) {
                & (Join-Path $PSScriptRoot 'Test-PortableExecutable.ps1') -LiteralPath (Join-Path $bin $dll) -Architecture x64 -Kind dll
            }
            foreach ($exe in @('MetasequoiaImeServer.exe', 'MetasequoiaImeWatchdog.exe',
                'msime-client-prepare.exe', 'msime-mcp.exe',
                'msime-client-settings.exe', 'MSIME.exe')) {
                & (Join-Path $PSScriptRoot 'Test-PortableExecutable.ps1') -LiteralPath (Join-Path $bin $exe) -Architecture x64 -Kind exe
            }
        }
    }
    Write-Output 'Client build commands and PE architecture checks completed; no signing, packaging or installation performed.'
} finally {
    $env:CMAKE_PREFIX_PATH = $previousPrefix
    $env:CARGO_TARGET_DIR = $previousTarget
    $env:CARGO_PROFILE_RELEASE_DEBUG = $previousDebug
    Pop-Location
}
