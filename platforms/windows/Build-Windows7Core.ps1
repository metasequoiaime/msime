# 在同一源码上编译 Win7 核心及 MCP；完整产品由 Build-Windows7Client.ps1 收集。
[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidateSet('x64', 'x86')][string]$Architecture,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [string]$RepoRoot = ''
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if (-not [IO.Path]::IsPathRooted($OutputDirectory)) { throw 'OutputDirectory must be an absolute path' }
if (-not (Get-Command dumpbin -CommandType Application -ErrorAction SilentlyContinue)) {
    throw 'Win7 core needs the MSVC developer environment (vcvarsall.bat x64 or x64_x86); dumpbin was not found'
}

function Invoke-CoreCommand {
    param([string]$Command, [string[]]$Arguments)
    & $Command @Arguments
    if ($LASTEXITCODE -ne 0) { throw "Win7 core build failed: $Command ($LASTEXITCODE)" }
}

if ($RepoRoot -eq '') { $RepoRoot = Split-Path -Parent (Split-Path -Parent $PSScriptRoot) }
$RepoRoot = (Resolve-Path -LiteralPath $RepoRoot).Path
$triple = if ($Architecture -eq 'x64') { 'x86_64-win7-windows-msvc' } else { 'i686-win7-windows-msvc' }
$cmakeArch = if ($Architecture -eq 'x64') { 'x64' } else { 'Win32' }
$nativeBuild = Join-Path $RepoRoot "target/windows7-compat/$Architecture"
$release = Join-Path $RepoRoot "target/$triple/release"
$output = Join-Path $OutputDirectory $Architecture
$previousBootstrap = $env:RUSTC_BOOTSTRAP
$previousTarget = $env:CARGO_TARGET_DIR
$previousDebug = $env:CARGO_PROFILE_RELEASE_DEBUG
$previousEncodedFlags = $env:CARGO_ENCODED_RUSTFLAGS
Push-Location $RepoRoot
try {
    $sysroot = & rustc --print sysroot
    if ($LASTEXITCODE -ne 0) { throw 'Cannot read the pinned Rust toolchain' }
    if (-not (Test-Path -LiteralPath (Join-Path $sysroot 'lib/rustlib/src/rust/library/std/Cargo.toml'))) {
        throw 'Win7 Tier 3 needs standard library sources; run rustup component add rust-src first'
    }
    # 仅在此进程内启用固定编译器的 build-std，不更改普通构建的目标或工具链。
    $env:RUSTC_BOOTSTRAP = '1'
    $env:CARGO_TARGET_DIR = Join-Path $RepoRoot 'target'
    $env:CARGO_PROFILE_RELEASE_DEBUG = '2'
    # TSF 把核心作为插件载入任意应用；Win7 无法从插件目录解析随包 UCRT 转发。
    # 对标准库及原生依赖统一静态链接 CRT，不能只给最终 DLL 补链接参数。
    $flags = if ($null -ne $previousEncodedFlags) { @($previousEncodedFlags.Split([char]31)) }
        elseif ($env:RUSTFLAGS) { @($env:RUSTFLAGS.Split([char[]]" `t`r`n", [StringSplitOptions]::RemoveEmptyEntries)) }
        else { @() }
    $env:CARGO_ENCODED_RUSTFLAGS = (@($flags) + @('-C', 'target-feature=+crt-static')) -join [char]31
    Invoke-CoreCommand cmake @('-S', (Join-Path $PSScriptRoot 'compat'), '-B', $nativeBuild,
        '-G', 'Visual Studio 17 2022', '-A', $cmakeArch)
    Invoke-CoreCommand cmake @('--build', $nativeBuild, '--config', 'Release', '--parallel', '4')
    Invoke-CoreCommand ctest @('--test-dir', $nativeBuild, '-C', 'Release', '--output-on-failure')
    $object = [IO.File]::ReadAllText((Join-Path $nativeBuild 'windows7-delay-object-Release.txt')).Trim()
    if (-not (Test-Path -LiteralPath $object -PathType Leaf)) { throw 'Missing Win7 delay-load object' }
    $link = @('--', '-C', "link-arg=$object", '-C', 'link-arg=delayimp.lib',
        '-C', 'link-arg=ole32.lib', '-C', 'link-arg=oleaut32.lib',
        '-C', 'link-arg=/DELAYLOAD:combase.dll',
        '-C', 'link-arg=/DELAYLOAD:api-ms-win-core-winrt-error-l1-1-0.dll',
        '-C', 'link-arg=/DELAYLOAD:mmdevapi.dll', '-C', 'link-arg=/DELAY:NOBIND')
    $cargo = @('-Z', 'build-std=std,panic_unwind', 'rustc', '--locked', '--release', '--target', $triple)
    Invoke-CoreCommand cargo ($cargo + @('-p', 'msime-host-api', '--lib') + $link)
    & (Join-Path $PSScriptRoot 'Test-PortableExecutable.ps1') -LiteralPath (Join-Path $release 'msime_host_api.dll') -Architecture $Architecture -Kind dll
    $audit = @(& (Join-Path $PSScriptRoot 'Test-Windows7CoreImports.ps1') -LiteralPath (Join-Path $release 'msime_host_api.dll'))
    if (@($audit[0].Imports | Where-Object { $_.Library -match '^(api-ms-win-crt-.*|ucrtbase|vcruntime[0-9_]*|msvcp[0-9_]*)\.dll$' }).Count) {
        throw 'Win7 TSF core must not import a dynamic CRT from the editor process'
    }
    $files = @('msime_host_api.dll', 'msime_host_api.dll.lib', 'msime_host_api.pdb')
    New-Item -ItemType Directory -Force -Path $output | Out-Null
    # MCP 会把 host-api 当依赖重新链接，立即保存这一份带兼容钩子的 DLL 与符号。
    foreach ($name in $files) {
        Copy-Item -LiteralPath (Join-Path $release $name) -Destination $output
    }
    Invoke-CoreCommand (Join-Path $nativeBuild 'Release/windows7-core-load.exe') @((Join-Path $output 'msime_host_api.dll'))
    Invoke-CoreCommand cargo ($cargo + @('-p', 'msime-mcp-server', '--bin', 'msime-mcp') + $link)
    & (Join-Path $PSScriptRoot 'Test-PortableExecutable.ps1') -LiteralPath (Join-Path $release 'msime-mcp.exe') -Architecture $Architecture -Kind exe
    $audit += & (Join-Path $PSScriptRoot 'Test-Windows7CoreImports.ps1') -LiteralPath (Join-Path $release 'msime-mcp.exe')
    Copy-Item -LiteralPath (Join-Path $release 'msime-mcp.exe') -Destination $output
    $mcpPdbs = @('msime_mcp.pdb', 'msime-mcp.pdb' | ForEach-Object { Join-Path $release $_ } | Where-Object { Test-Path -LiteralPath $_ -PathType Leaf })
    if ($mcpPdbs.Count -ne 1) { throw 'Expected one MCP server PDB output' }
    Copy-Item -LiteralPath $mcpPdbs[0] -Destination (Join-Path $output 'msime-mcp.pdb')
    # 清单在全部构建完成后写入；仅供 Win7 产品收集，不改变现代发布流程。
    $audit | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $output 'windows7-core-imports.json') -Encoding UTF8
} finally {
    $env:RUSTC_BOOTSTRAP = $previousBootstrap
    $env:CARGO_TARGET_DIR = $previousTarget
    $env:CARGO_PROFILE_RELEASE_DEBUG = $previousDebug
    $env:CARGO_ENCODED_RUSTFLAGS = $previousEncodedFlags
    Pop-Location
}
