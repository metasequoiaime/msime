# Run from a Windows MSVC build environment with the MSVC ARM64 and ARM64EC build tools and prebuilt native dependency prefixes. Unless -RustOutputs names the Rust half already built by Build-RustOutputs.ps1, it builds that first, which also needs the x64, x86 and ARM64 Rust MSVC targets, pnpm and clang on PATH (ring builds its ARM64 assembly with it). This script never signs or installs.
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$X64Dependencies,
    [Parameter(Mandatory)][string]$X86Dependencies,
    [string]$RepoRoot = (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)),
    [string]$Generator = 'Visual Studio 17 2022',
    [string]$TargetVersion = '',
    # 产品版本（shared/contracts/editions.json 里有 Windows 段的 id）。TSF DLL、Server、看门狗、prepare 工具和 WinUI 设置窗口在编译期绑定到这个版本；full 的输出在 target/windows-full，其他版本在 target/windows-<id>，可以一个接一个地构建而不互相覆盖。
    [ValidatePattern('^[a-z][a-z0-9]*$')][string]$Edition = 'full',
    # Build-RustOutputs.ps1's output directory with its x64, x86 and arm64 subdirectories. None of it depends on the edition, so a release builds it once for every edition; without it this script builds it under target/windows-rust.
    [string]$RustOutputs = ''
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

if ($RustOutputs -ne '' -and (-not [IO.Path]::IsPathRooted($RustOutputs) -or -not (Test-Path -LiteralPath $RustOutputs -PathType Container))) {
    throw 'RustOutputs must be an absolute, existing directory'
}
foreach ($prefix in @($X64Dependencies, $X86Dependencies)) {
    if (-not [IO.Path]::IsPathRooted($prefix) -or -not (Test-Path -LiteralPath $prefix -PathType Container)) {
        throw 'Provide absolute, existing x64 and x86 native dependency prefixes'
    }
}
$RepoRoot = (Resolve-Path -LiteralPath $RepoRoot).Path
$editionTable = Get-Content -LiteralPath (Join-Path $RepoRoot 'shared/contracts/editions.json') -Raw -Encoding UTF8 | ConvertFrom-Json
$editionEntry = @($editionTable.editions | Where-Object { $_.id -ceq $Edition -and $null -ne $_.platforms.windows })
if ($editionEntry.Count -ne 1) { throw "Edition $Edition has no Windows identifiers in shared/contracts/editions.json" }
# 两个版本的 TIP 被同一个应用加载时，按导入表找 msime_host_api.dll 会拿到先加载的那一个，所以不是 full 的版本把它改成自己的名字（版本表 host_dll），并生成同名的导入库给 TSF DLL、Server 和设置窗口链接。
$hostDll = [string]$editionEntry[0].platforms.windows.host_dll
$buildRoot = Join-Path $RepoRoot "target/windows-$Edition"
foreach ($relative in @('Cargo.toml', 'crates/engine/Cargo.toml',
                         'platforms/windows/CMakeLists.txt', 'platforms/windows/tsf/CMakeLists.txt',
                         'platforms/windows/settings/MSIME.Settings.vcxproj',
                         'apps/desktop/package.json', 'scripts/fetch_voice_runtime.py')) {
    if (-not (Test-Path -LiteralPath (Join-Path $RepoRoot $relative) -PathType Leaf)) {
        throw "Missing Client build source: $relative"
    }
}
# The on-device speech runtime staged beside the Server; Prepare-PackageFiles.ps1 and install-smoke.ps1 name the same three files.
$voiceRuntimeLibraries = @('sherpa-onnx-c-api.dll', 'onnxruntime.dll', 'onnxruntime_providers_shared.dll')
$previousPrefix = $env:CMAKE_PREFIX_PATH
# The ARM64 host DLL that the native half of the Arm64X TIP imports (edition_windows.py arm64_host_dll): it sits beside the x64 host in the same version directory.
$arm64HostDll = [IO.Path]::GetFileNameWithoutExtension($hostDll) + '_arm64.dll'
Push-Location $RepoRoot
try {
    $rust = $RustOutputs
    if ($rust -eq '') {
        $rust = Join-Path $RepoRoot 'target/windows-rust'
        foreach ($arch in @('x64', 'x86', 'arm64')) {
            & (Join-Path $PSScriptRoot 'Build-RustOutputs.ps1') -Architecture $arch -OutputDirectory $rust -RepoRoot $RepoRoot -TargetVersion $TargetVersion
        }
    }
    foreach ($arch in @('x64', 'x86')) {
        $platform = if ($arch -eq 'x64') { 'x64' } else { 'Win32' }
        $env:CMAKE_PREFIX_PATH = if ($arch -eq 'x64') { $X64Dependencies } else { $X86Dependencies }
        $release = Join-Path $rust $arch
        $output = Join-Path $buildRoot $arch
        $bin = Join-Path $output 'bin'
        $hostLibrary = Join-Path $release 'msime_host_api.dll.lib'
        if ($Edition -ne 'full') {
            New-Item -ItemType Directory -Force -Path $output | Out-Null
            $hostDefinition = Join-Path $output ([IO.Path]::ChangeExtension($hostDll, '.def'))
            Invoke-ClientBuild python @((Join-Path $RepoRoot 'platforms/windows/scripts/edition_windows.py'), 'host-def',
                '--edition', $Edition, '--dll', (Join-Path $release 'msime_host_api.dll'), '--output', $hostDefinition)
            $hostLibrary = Join-Path $output "$hostDll.lib"
            $machine = if ($arch -eq 'x64') { 'X64' } else { 'X86' }
            Invoke-ClientBuild lib @('/NOLOGO', "/DEF:$hostDefinition", "/OUT:$hostLibrary", "/MACHINE:$machine")
        }
        $source = if ($arch -eq 'x64') { 'platforms/windows' } else { 'platforms/windows/tsf' }
        $configure = @('-S', (Join-Path $RepoRoot $source), '-B', $output,
            '-G', $Generator, '-A', $platform,
            "-DCMAKE_PREFIX_PATH=$($env:CMAKE_PREFIX_PATH)",
            "-DMSIME_HOST_LIBRARY=$hostLibrary", "-DMSIME_EDITION=$Edition",
            "-DCMAKE_RUNTIME_OUTPUT_DIRECTORY_RELWITHDEBINFO=$bin",
            '-DMSIMEUI_BUILD_HANDWRITING_DEMO=OFF')
        if ($arch -eq 'x64') { $configure += '-DMSIME_SERVER_UIACCESS=ON' }
        if ($TargetVersion -ne '') { $configure += "-DMSIME_WINDOWS_VERSION=$TargetVersion" }
        Invoke-ClientBuild cmake $configure
        $targets = if ($arch -eq 'x64') {
            @('msime-client-server', 'msime-client-watchdog', 'msime-client-prepare', 'msime-tsf')
        } else { @('msime-tsf') }
        Invoke-ClientBuild cmake (@('--build', $output, '--config', 'RelWithDebInfo', '--parallel', '4', '--target') + $targets)
        # 宿主 DLL 的 PDB 保留 DLL 内嵌的文件名；Collect-Symbols.ps1 把它打进符号包。
        Invoke-ClientBuild cmake @('-E', 'copy_if_different', (Join-Path $release 'msime_host_api.dll'), (Join-Path $bin $hostDll))
        Invoke-ClientBuild cmake @('-E', 'copy_if_different', (Join-Path $release 'msime_host_api.pdb'), (Join-Path $bin 'msime_host_api.pdb'))
        if ($arch -eq 'x64') {
            $x64HostLibrary = $hostLibrary
            # The Rust outputs: the MCP server and the shared Tauri panel shell, each with its PDB under the name the package wants.
            Invoke-ClientBuild cmake (@('-E', 'copy_if_different') +
                @('msime-mcp.exe', 'msime-mcp.pdb', 'MSIME.exe', 'MSIME.pdb' | ForEach-Object { Join-Path $release $_ }) + @($bin))

            # Windows settings are a native WinUI 3 app. Keep the shared Tauri
            # shell below for the emoji/handwriting/keyboard panels, but do not
            # use it as the settings product anymore.
            $settingsProject = Join-Path $RepoRoot 'platforms/windows/settings/MSIME.Settings.vcxproj'
            $settingsIntermediate = Join-Path $output 'settings-obj'
            # -restore rather than /t:Restore,Build: restore writes obj\*.nuget.g.targets, where the CppWinRT and Windows App SDK build logic lives, and only a separate evaluation after it imports them. In one evaluation the build skips header generation, which only a previously restored obj directory hides.
            # Built under the name the Server launcher, PE checks and packaging expect. Renaming the output afterwards would leave its resource index behind as MSIME.Settings.pri, which MRT Core looks up by the executable's name.
            Invoke-ClientBuild msbuild @($settingsProject, '-restore', '/t:Build',
                '/p:Configuration=RelWithDebInfo', '/p:Platform=x64',
                '/p:TargetName=msime-client-settings',
                "/p:HostApiLibrary=$hostLibrary", "/p:MsimeEdition=$Edition",
                "/p:OutDir=$bin\", "/p:IntDir=$settingsIntermediate\")
            $settingsPdb = Join-Path $bin 'msime-client-settings.pdb'
            if (-not (Test-Path -LiteralPath $settingsPdb -PathType Leaf)) {
                throw "Expected one WinUI settings PDB output: $settingsPdb"
            }
        }
    }
    $env:CMAKE_PREFIX_PATH = $X64Dependencies
    # The Server recognizes speech on-device through the pinned sherpa-onnx runtime (resources/voice-runtime.lock.json), which it loads with LoadLibrary from its own directory; onnxruntime.dll resolves beside sherpa-onnx-c-api.dll. The fetch verifies the archive's SHA-256 before extracting and reuses a verified copy on later runs. These are upstream MSVC /MD builds, so they need the same VC runtime the installer already requires.
    $voiceRuntime = Join-Path $RepoRoot 'target/voice-runtime/windows-x64'
    Invoke-ClientBuild python @((Join-Path $RepoRoot 'scripts/fetch_voice_runtime.py'),
        '--platform', 'windows-x64', '--out', $voiceRuntime)
    Invoke-ClientBuild cmake (@('-E', 'copy_if_different') +
        @($voiceRuntimeLibraries | ForEach-Object { Join-Path $voiceRuntime $_ }) +
        @((Join-Path $buildRoot 'x64/bin')))
    # Windows on Arm: the 64-bit TIP there is Arm64X (tsf/CMakeLists.txt, MSIME_TSF_ARM64X), linked from an ARM64 and an ARM64EC build of the same sources. Its ARM64 half imports the ARM64 host (Build-RustOutputs.ps1, with a static C runtime like the TIP's, so it needs no ARM64 Visual C++ runtime) under its own name; its ARM64EC half runs in emulated x64 processes and imports the x64 host. Everything else stays x64 and runs emulated. The TIP takes only header-only libraries from its prefix, so both passes use the x64 one.
    $arm64Release = Join-Path $rust 'arm64'
    $arm64Output = Join-Path $buildRoot 'arm64'
    $arm64Bin = Join-Path $arm64Output 'bin'
    New-Item -ItemType Directory -Force -Path $arm64Output | Out-Null
    $arm64HostDefinition = Join-Path $arm64Output ([IO.Path]::ChangeExtension($arm64HostDll, '.def'))
    Invoke-ClientBuild python @((Join-Path $RepoRoot 'platforms/windows/scripts/edition_windows.py'), 'host-def',
        '--edition', $Edition, '--arm64', '--dll', (Join-Path $arm64Release 'msime_host_api.dll'), '--output', $arm64HostDefinition)
    $arm64HostLibrary = Join-Path $arm64Output "$arm64HostDll.lib"
    Invoke-ClientBuild lib @('/NOLOGO', "/DEF:$arm64HostDefinition", "/OUT:$arm64HostLibrary", '/MACHINE:ARM64')
    $arm64Response = Join-Path $arm64Output 'msime-tsf-arm64.rsp'
    foreach ($pass in @('ARM64', 'ARM64EC')) {
        $passOutput = Join-Path $arm64Output $pass.ToLowerInvariant()
        # The ARM64 pass's own DLL is only an input to the Arm64X link, so it stays in its build tree.
        $passBin = if ($pass -eq 'ARM64EC') { $arm64Bin } else { Join-Path $passOutput 'bin' }
        $passHost = if ($pass -eq 'ARM64EC') { $x64HostLibrary } else { $arm64HostLibrary }
        $configure = @('-S', (Join-Path $RepoRoot 'platforms/windows/tsf'), '-B', $passOutput,
            '-G', $Generator, '-A', $pass,
            "-DCMAKE_PREFIX_PATH=$X64Dependencies",
            "-DMSIME_HOST_LIBRARY=$passHost", "-DMSIME_EDITION=$Edition",
            "-DMSIME_TSF_ARM64X=$pass", "-DMSIME_TSF_ARM64X_RESPONSE=$arm64Response",
            "-DCMAKE_RUNTIME_OUTPUT_DIRECTORY_RELWITHDEBINFO=$passBin")
        if ($TargetVersion -ne '') { $configure += "-DMSIME_WINDOWS_VERSION=$TargetVersion" }
        Invoke-ClientBuild cmake $configure
        Invoke-ClientBuild cmake @('--build', $passOutput, '--config', 'RelWithDebInfo', '--parallel', '4', '--target', 'msime-tsf')
    }
    Invoke-ClientBuild cmake @('-E', 'copy_if_different', (Join-Path $arm64Release 'msime_host_api.dll'), (Join-Path $arm64Bin $arm64HostDll))
    Invoke-ClientBuild cmake @('-E', 'copy_if_different', (Join-Path $arm64Release 'msime_host_api.pdb'), (Join-Path $arm64Bin 'msime_host_api.pdb'))
    foreach ($arch in @('x64', 'x86')) {
        $bin = Join-Path $buildRoot "$arch/bin"
        $prefix = if ($arch -eq 'x64') { $X64Dependencies } else { $X86Dependencies }
        & (Join-Path $PSScriptRoot 'Copy-RuntimeDependencies.ps1') `
            -DependencyPrefix $prefix -Destination $bin -Architecture $arch
        foreach ($dll in @('MetasequoiaImeTsf.dll', $hostDll)) {
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
    & (Join-Path $PSScriptRoot 'Test-PortableExecutable.ps1') -LiteralPath (Join-Path $arm64Bin 'MetasequoiaImeTsf.dll') -Architecture arm64x -Kind dll
    & (Join-Path $PSScriptRoot 'Test-PortableExecutable.ps1') -LiteralPath (Join-Path $arm64Bin $arm64HostDll) -Architecture arm64 -Kind dll
    Write-Output 'Client build commands and PE architecture checks completed; no signing, packaging or installation performed.'
} finally {
    $env:CMAKE_PREFIX_PATH = $previousPrefix
    Pop-Location
}
