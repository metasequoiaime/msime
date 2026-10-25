[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidateSet('x64', 'x86')][string]$Architecture,
    [string]$RepoRoot = '',
    [ValidateRange(1, 16)][int]$Parallel = 4
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if ($RepoRoot -eq '') { $RepoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot) }
$RepoRoot = (Resolve-Path -LiteralPath $RepoRoot).Path

function Invoke-VoiceBuild {
    param([string]$Program, [string[]]$Arguments)
    & $Program @Arguments
    if ($LASTEXITCODE -ne 0) { throw "Win7 voice command failed: $Program ($LASTEXITCODE)" }
}

# 保持现代包的锁和产物不变；这里重编相同的 ONNX 版本，不减少模型运算支持。
$sourceCommit = '33ca9628233dc8f002435e868d4c2e9f82766ca1'
$source = Join-Path $RepoRoot 'target/tooling/onnxruntime-windows7-1.28.2'
$build = Join-Path $RepoRoot "target/windows7-voice/$Architecture/onnxruntime"
$output = Join-Path $RepoRoot "target/windows7-voice/$Architecture/bin"
$adapter = Join-Path $PSScriptRoot 'compat/Windows7Voice.cmake'
$drive = New-Object IO.DriveInfo ([IO.Path]::GetPathRoot($RepoRoot))
if ($drive.AvailableFreeSpace -lt 50GB) { throw 'Win7 voice build requires at least 50 GB free disk space' }
$completedManifest = Join-Path $output 'voice-runtime.build.json'
if (Test-Path -LiteralPath $completedManifest -PathType Leaf) { Remove-Item -LiteralPath $completedManifest }
New-Item -ItemType Directory -Force (Split-Path -Parent $source) | Out-Null
if (-not (Test-Path -LiteralPath $source)) {
    Invoke-VoiceBuild git @('-c', 'core.longpaths=true', 'clone', '--depth', '1', '--branch', 'v1.28.2',
        'https://github.com/microsoft/onnxruntime.git', $source)
}
$head = & git -C $source rev-parse HEAD
if ($LASTEXITCODE -ne 0 -or $head -cne $sourceCommit) { throw 'Win7 ONNX source is not at the pinned commit' }
$dirty = & git -C $source status --porcelain --untracked-files=no
if ($LASTEXITCODE -ne 0 -or $dirty) { throw 'Win7 ONNX source has tracked modifications; refusing to overwrite it' }
Invoke-VoiceBuild git @('-C', $source, '-c', 'core.longpaths=true', 'submodule', 'update', '--init', '--depth', '1', 'cmake/external/onnx')
$platform = if ($Architecture -eq 'x64') { 'x64' } else { 'Win32' }
Invoke-VoiceBuild cmake @('-S', (Join-Path $source 'cmake'), '-B', $build,
    '-G', 'Visual Studio 17 2022', '-A', $platform,
    "-DCMAKE_PROJECT_INCLUDE=$adapter", '-DCMAKE_BUILD_TYPE=Release',
    '-DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded', '-Donnxruntime_MSVC_STATIC_RUNTIME=ON',
    '-DABSL_MSVC_STATIC_RUNTIME=ON',
    '-Donnxruntime_BUILD_SHARED_LIB=ON', '-Donnxruntime_BUILD_UNIT_TESTS=OFF',
    '-Donnxruntime_DEV_MODE=OFF', '-Donnxruntime_ENABLE_LTO=OFF',
    '-Donnxruntime_ENABLE_PYTHON=OFF', '-Donnxruntime_ENABLE_TELEMETRY=OFF',
    '-Donnxruntime_USE_VCPKG=OFF', '-Donnxruntime_USE_CUDA=OFF', '-Donnxruntime_USE_DML=OFF')
Invoke-VoiceBuild cmake @('--build', $build, '--config', 'Release', '--target', 'onnxruntime',
    'onnxruntime_providers_shared', '--parallel', "$Parallel")
New-Item -ItemType Directory -Force $output | Out-Null
foreach ($name in @('onnxruntime.dll', 'onnxruntime_providers_shared.dll')) {
    $file = Join-Path $build "Release/$name"
    if (-not (Test-Path -LiteralPath $file -PathType Leaf)) { throw "Missing Win7 voice runtime: $name" }
    Copy-Item -LiteralPath $file -Destination (Join-Path $output $name)
    # 编译成功不等于兼容；发现未适配的静态导入立即失败，保留产物供排查。
    $imports = & (Join-Path $PSScriptRoot 'Test-Windows7CoreImports.ps1') -LiteralPath $file
    $imports | ConvertTo-Json -Depth 8 | Set-Content -Encoding UTF8 (Join-Path $output "$name.imports.json")
}
# sherpa 与 ONNX 保持当前锁定的版本，使用静态 CRT，避免额外引入未验收的动态运行库。
$sherpaCommit = '11afbd009a7f8c08f4bcf2fc1b265d0df4670fbf'
$sherpaSource = Join-Path $RepoRoot 'target/tooling/sherpa-onnx-windows7-1.13.8'
$sherpaBuild = Join-Path $RepoRoot "target/windows7-voice/$Architecture/sherpa"
if (-not (Test-Path -LiteralPath $sherpaSource)) {
    Invoke-VoiceBuild git @('-c', 'core.longpaths=true', 'clone', '--depth', '1', '--branch', 'v1.13.8',
        'https://github.com/k2-fsa/sherpa-onnx.git', $sherpaSource)
}
$head = & git -C $sherpaSource rev-parse HEAD
if ($LASTEXITCODE -ne 0 -or $head -cne $sherpaCommit) { throw 'Win7 sherpa source is not at the pinned commit' }
$dirty = & git -C $sherpaSource status --porcelain --untracked-files=no
if ($LASTEXITCODE -ne 0 -or $dirty) { throw 'Win7 sherpa source has tracked modifications; refusing to overwrite it' }
$sherpaAdapter = Join-Path $PSScriptRoot 'compat/Windows7Sherpa.cmake'
$previousInclude = [Environment]::GetEnvironmentVariable('SHERPA_ONNXRUNTIME_INCLUDE_DIR', 'Process')
$previousLib = [Environment]::GetEnvironmentVariable('SHERPA_ONNXRUNTIME_LIB_DIR', 'Process')
try {
    $env:SHERPA_ONNXRUNTIME_INCLUDE_DIR = Join-Path $source 'include/onnxruntime/core/session'
    $env:SHERPA_ONNXRUNTIME_LIB_DIR = Join-Path $build 'Release'
    Invoke-VoiceBuild cmake @('-S', $sherpaSource, '-B', $sherpaBuild,
        '-G', 'Visual Studio 17 2022', '-A', $platform, '-DCMAKE_BUILD_TYPE=Release',
        "-DCMAKE_PROJECT_INCLUDE=$sherpaAdapter",
        "-Dlocation_onnxruntime_lib2=$(Join-Path $build 'Release/onnxruntime.lib')",
        '-DCMAKE_MSVC_RUNTIME_LIBRARY=MultiThreaded', '-DSHERPA_ONNX_USE_STATIC_CRT=ON',
        '-DBUILD_SHARED_LIBS=ON', '-DSHERPA_ONNX_ENABLE_C_API=ON',
        '-DSHERPA_ONNX_USE_PRE_INSTALLED_ONNXRUNTIME_IF_AVAILABLE=ON',
        '-DSHERPA_ONNX_ENABLE_BINARY=OFF', '-DSHERPA_ONNX_BUILD_C_API_EXAMPLES=OFF',
        '-DSHERPA_ONNX_ENABLE_TESTS=OFF', '-DSHERPA_ONNX_ENABLE_PYTHON=OFF',
        '-DSHERPA_ONNX_ENABLE_GPU=OFF', '-DSHERPA_ONNX_ENABLE_DIRECTML=OFF',
        '-DSHERPA_ONNX_ENABLE_PORTAUDIO=OFF', '-DSHERPA_ONNX_ENABLE_WEBSOCKET=OFF',
        '-DSHERPA_ONNX_ENABLE_TTS=OFF', '-DSHERPA_ONNX_ENABLE_SPEAKER_DIARIZATION=ON')
    Invoke-VoiceBuild cmake @('--build', $sherpaBuild, '--config', 'Release',
        '--target', 'sherpa-onnx-c-api', '--parallel', "$Parallel")
} finally {
    [Environment]::SetEnvironmentVariable('SHERPA_ONNXRUNTIME_INCLUDE_DIR', $previousInclude, 'Process')
    [Environment]::SetEnvironmentVariable('SHERPA_ONNXRUNTIME_LIB_DIR', $previousLib, 'Process')
}
$sherpaDll = Join-Path $sherpaBuild 'bin/Release/sherpa-onnx-c-api.dll'
if (-not (Test-Path -LiteralPath $sherpaDll -PathType Leaf)) { throw 'Missing Win7 sherpa C API DLL' }
Copy-Item -LiteralPath $sherpaDll -Destination (Join-Path $output 'sherpa-onnx-c-api.dll')
$imports = & (Join-Path $PSScriptRoot 'Test-Windows7CoreImports.ps1') -LiteralPath $sherpaDll -DependencyDirectory $output
$imports | ConvertTo-Json -Depth 8 | Set-Content -Encoding UTF8 (Join-Path $output 'sherpa-onnx-c-api.dll.imports.json')
$manifest = [ordered]@{
    Version = '1.28.2'
    Source = $sourceCommit
    Architecture = $Architecture
    CpuOnly = $true
    StaticRuntime = $true
    SherpaTts = $false
    Adapter = (Get-FileHash -Algorithm SHA256 -LiteralPath $adapter).Hash.ToLowerInvariant()
    Api = (Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $PSScriptRoot 'compat/Windows7VoiceApi.h')).Hash.ToLowerInvariant()
    Redirect = (Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $PSScriptRoot 'compat/Windows7VoiceRedirect.h')).Hash.ToLowerInvariant()
    Runtime = (Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $output 'onnxruntime.dll')).Hash.ToLowerInvariant()
    Provider = (Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $output 'onnxruntime_providers_shared.dll')).Hash.ToLowerInvariant()
    SherpaVersion = '1.13.8'
    SherpaSource = $sherpaCommit
    SherpaAdapter = (Get-FileHash -Algorithm SHA256 -LiteralPath $sherpaAdapter).Hash.ToLowerInvariant()
    SherpaRuntime = (Get-FileHash -Algorithm SHA256 -LiteralPath (Join-Path $output 'sherpa-onnx-c-api.dll')).Hash.ToLowerInvariant()
}
$checks = Join-Path $RepoRoot "target/windows7-compat/$Architecture"
Invoke-VoiceBuild cmake @('-S', (Join-Path $PSScriptRoot 'compat'), '-B', $checks, '-A', $platform,
    "-DMSIME_ONNX_INCLUDE_DIR=$(Join-Path $source 'include/onnxruntime/core/session')",
    "-DMSIME_ONNX_RUNTIME=$(Join-Path $output 'onnxruntime.dll')",
    "-DMSIME_SHERPA_INCLUDE_DIR=$sherpaSource",
    "-DMSIME_SHERPA_RUNTIME=$(Join-Path $output 'sherpa-onnx-c-api.dll')")
Invoke-VoiceBuild cmake @('--build', $checks, '--config', 'Release', '--parallel', '2',
    '--target', 'windows7-voice-api', 'windows7-onnx-session', 'windows7-sherpa-runtime')
Invoke-VoiceBuild ctest @('--test-dir', $checks, '-C', 'Release', '--output-on-failure',
    '-R', '^windows7-(voice-api|onnx-session|sherpa-runtime)$')
$manifest | ConvertTo-Json | Set-Content -Encoding UTF8 $completedManifest
Write-Host "Win7 voice runtime built, known imports and actual C ABI operations checked: $output"
