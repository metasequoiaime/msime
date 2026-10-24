# 同仓构建 Win7 原生产品；不签名、不发布、不安装，普通 Windows 构建保持原入口。
[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$X64Dependencies,
    [Parameter(Mandatory)][string]$X86Dependencies,
    [Parameter(Mandatory)][string[]]$X64RuntimeDirectories,
    [Parameter(Mandatory)][string[]]$X86RuntimeDirectories,
    # vcpkg 的包查找包装（例如静态 zlib 的 zs.lib）；只读取已安装依赖，不运行清单安装。
    [string]$VcpkgRoot = '',
    [string]$RepoRoot = (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)),
    [ValidatePattern('^[a-z][a-z0-9]*$')][string]$Edition = 'full',
    # 只复用同一源码的兼容核心，目录含 x64 与 x86；未指定则调用专用核心构建。
    [string]$CoreOutputs = '',
    # 目录含两个架构的 bin/voice-runtime.build.json；摘要和兼容适配源码都必须相符。
    [string]$VoiceOutputs = '',
    # 与现代构建相同，只有随后受信任签名并安装到安全目录的包才显式开启。
    [switch]$ServerUiAccess
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
function Invoke-Windows7Build {
    param([string]$Program, [string[]]$Arguments)
    & $Program @Arguments
    if ($LASTEXITCODE -ne 0) { throw "Win7 client build failed: $Program ($LASTEXITCODE)" }
}
$RepoRoot = (Resolve-Path -LiteralPath $RepoRoot).Path
foreach ($directory in @($X64Dependencies,$X86Dependencies) + $X64RuntimeDirectories + $X86RuntimeDirectories) {
    if (-not [IO.Path]::IsPathRooted($directory) -or -not (Test-Path -LiteralPath $directory -PathType Container)) {
        throw 'Win7 dependency and redistributable paths must be absolute existing directories'
    }
}
if (-not (Get-Command dumpbin -ErrorAction SilentlyContinue)) { throw 'Win7 product build needs the MSVC developer environment' }
if ($VcpkgRoot -ne '' -and (-not [IO.Path]::IsPathRooted($VcpkgRoot) -or
    -not (Test-Path -LiteralPath (Join-Path $VcpkgRoot 'scripts/buildsystems/vcpkg.cmake') -PathType Leaf))) { throw 'VcpkgRoot must contain the official CMake toolchain' }
$drive = New-Object IO.DriveInfo ([IO.Path]::GetPathRoot($RepoRoot))
if ($drive.AvailableFreeSpace -lt 50GB) { throw 'Win7 product build requires at least 50 GB free disk space' }
$table = Get-Content -LiteralPath (Join-Path $RepoRoot 'shared/contracts/editions.json') -Raw -Encoding UTF8 | ConvertFrom-Json
$editionEntry = @($table.editions | Where-Object { $_.id -ceq $Edition -and $null -ne $_.platforms.windows })
if ($editionEntry.Count -ne 1) { throw 'Unknown Windows product edition' }
$hostName = [string]$editionEntry[0].platforms.windows.host_dll
$buildRoot = Join-Path $RepoRoot "target/windows7-$Edition"
$version = (Get-Content -LiteralPath (Join-Path $PSScriptRoot 'version.txt') -TotalCount 1).Trim()
Push-Location $RepoRoot
try {
    if ($CoreOutputs -eq '') {
        $CoreOutputs = Join-Path $RepoRoot 'target/windows7-core'
        foreach ($arch in @('x64','x86')) {
            & (Join-Path $PSScriptRoot 'Build-Windows7Core.ps1') -Architecture $arch -OutputDirectory $CoreOutputs -RepoRoot $RepoRoot
        }
    }
    if ($VoiceOutputs -eq '') {
        $VoiceOutputs = Join-Path $RepoRoot 'target/windows7-voice'
        foreach ($arch in @('x64','x86')) {
            & (Join-Path $PSScriptRoot 'Build-Windows7VoiceRuntime.ps1') -Architecture $arch -RepoRoot $RepoRoot
        }
    }
    foreach ($directory in @($CoreOutputs,$VoiceOutputs)) {
        if (-not [IO.Path]::IsPathRooted($directory) -or -not (Test-Path -LiteralPath $directory -PathType Container)) { throw 'Win7 output roots must be absolute existing directories' }
    }
    foreach ($arch in @('x64','x86')) {
        $prefix = if ($arch -eq 'x64') { $X64Dependencies } else { $X86Dependencies }
        $runtimes = if ($arch -eq 'x64') { $X64RuntimeDirectories } else { $X86RuntimeDirectories }
        $platform = if ($arch -eq 'x64') { 'x64' } else { 'Win32' }
        $output = Join-Path $buildRoot $arch
        $bin = Join-Path $output 'bin'
        $core = Join-Path $CoreOutputs $arch
        $voice = Join-Path $VoiceOutputs "$arch/bin"
        $manifest = Get-Content -LiteralPath (Join-Path $voice 'voice-runtime.build.json') -Raw -Encoding UTF8 | ConvertFrom-Json
        if ($manifest.Architecture -cne $arch -or $manifest.Version -cne '1.28.2' -or $manifest.SherpaVersion -cne '1.13.8') { throw 'Win7 voice runtime manifest has the wrong target or version' }
        foreach ($pair in @(@('Adapter',(Join-Path $PSScriptRoot 'compat/Windows7Voice.cmake')),
                @('Api',(Join-Path $PSScriptRoot 'compat/Windows7VoiceApi.h')),
                @('Redirect',(Join-Path $PSScriptRoot 'compat/Windows7VoiceRedirect.h')),
                @('SherpaAdapter',(Join-Path $PSScriptRoot 'compat/Windows7Sherpa.cmake')),
                @('Runtime',(Join-Path $voice 'onnxruntime.dll')),
                @('Provider',(Join-Path $voice 'onnxruntime_providers_shared.dll')),
                @('SherpaRuntime',(Join-Path $voice 'sherpa-onnx-c-api.dll')))) {
            if ((Get-FileHash -LiteralPath $pair[1]).Hash -ine $manifest.($pair[0])) { throw "Win7 voice runtime/source digest mismatch: $($pair[0])" }
        }
        New-Item -ItemType Directory -Path $bin -Force | Out-Null
        # 字体与许可证固定随程序分发，只供进程私有集合读取，不安装到系统。
        $fontSource = Join-Path $PSScriptRoot 'assets/fonts'
        $fontLock = Get-Content -LiteralPath (Join-Path $fontSource 'SOURCE.json') -Raw | ConvertFrom-Json
        foreach ($file in $fontLock.files.PSObject.Properties) {
            if ((Get-FileHash -LiteralPath (Join-Path $fontSource $file.Name) -Algorithm SHA256).Hash -ine $file.Value) { throw 'Bundled emoji font digest mismatch' }
        }
        Copy-Item -LiteralPath $fontSource -Destination $bin -Recurse -Force
        $hostLibrary = Join-Path $core 'msime_host_api.dll.lib'
        if ($Edition -ne 'full') {
            $definition = Join-Path $output "$hostName.def"
            Invoke-Windows7Build python @((Join-Path $PSScriptRoot 'scripts/edition_windows.py'),'host-def','--edition',$Edition,
                '--dll',(Join-Path $core 'msime_host_api.dll'),'--output',$definition)
            $hostLibrary = Join-Path $output "$hostName.lib"
            $machine = if ($arch -eq 'x64') { 'X64' } else { 'X86' }
            Invoke-Windows7Build lib @('/NOLOGO',"/DEF:$definition","/OUT:$hostLibrary","/MACHINE:$machine")
        }
        $common = @('-G','Visual Studio 17 2022','-A',$platform,"-DCMAKE_PREFIX_PATH=$prefix",
            "-DMSIME_HOST_LIBRARY=$hostLibrary","-DMSIME_EDITION=$Edition",
            "-DCMAKE_RUNTIME_OUTPUT_DIRECTORY_RELEASE=$bin",'-DBUILD_TESTING=ON')
        if ($VcpkgRoot -ne '') {
            $common += @("-DCMAKE_TOOLCHAIN_FILE=$(Join-Path $VcpkgRoot 'scripts/buildsystems/vcpkg.cmake')",
                "-DVCPKG_INSTALLED_DIR=$(Split-Path -Parent $prefix)","-DVCPKG_TARGET_TRIPLET=$(Split-Path -Leaf $prefix)",'-DVCPKG_MANIFEST_MODE=OFF')
        }
        Invoke-Windows7Build cmake (@('-S',$PSScriptRoot,'-B',$output,'-DMSIME_WINDOWS7_CLIENT=ON',
            "-DMSIME_SERVER_UIACCESS=$(if ($ServerUiAccess) { 'ON' } else { 'OFF' })",'-DMSIMEUI_BUILD_HANDWRITING_DEMO=OFF') + $common)
        Invoke-Windows7Build cmake @('--build',$output,'--config','Release','--parallel','4','--target',
            'msime-client-server','msime-client-watchdog','msime-client-prepare','msime-tsf','windows-shell-surfaces','windows7-shell-surfaces','windows-dpi-compat','windows7-websocket-runtime')
        foreach ($part in @('settings','panels')) {
            $native = Join-Path $output "win7-$part"
            Invoke-Windows7Build cmake (@('-S',(Join-Path $PSScriptRoot "$part/win7"),'-B',$native) + $common)
            $target = if ($part -eq 'settings') { 'msime-client-settings-win7' } else { 'msime-client-panels-win7' }
            Invoke-Windows7Build cmake @('--build',$native,'--config','Release','--parallel','4','--target',$target)
        }
        Invoke-Windows7Build ctest @('--test-dir',$output,'-C','Release','--output-on-failure','-R','^windows(7-shell-surfaces|7-websocket-runtime|-shell-surfaces|-dpi-compat)$')
        Copy-Item -LiteralPath (Join-Path $core 'msime_host_api.dll') -Destination (Join-Path $bin $hostName) -Force
        Copy-Item -LiteralPath (Join-Path $core 'msime-mcp.exe') -Destination $bin -Force
        foreach ($name in @('onnxruntime.dll','onnxruntime_providers_shared.dll','sherpa-onnx-c-api.dll')) { Copy-Item -LiteralPath (Join-Path $voice $name) -Destination $bin -Force }
        # 先建全新的闭包，旧成功产物只在本次全部审计成功后替换。
        $destination = Join-Path $output 'runtime.new'
        if (Test-Path -LiteralPath $destination) { throw 'Previous unfinished Win7 runtime.new exists; inspect it before rebuilding' }
        $entries = @('MetasequoiaImeServer.exe','MetasequoiaImeWatchdog.exe','msime-client-prepare.exe','MetasequoiaImeTsf.dll',
            'msime-client-settings-win7.exe','msime-client-panels-win7.exe','msime-mcp.exe','sherpa-onnx-c-api.dll','onnxruntime_providers_shared.dll')
        $dependencies = @((Join-Path $prefix 'bin') | Where-Object { Test-Path -LiteralPath $_ -PathType Container }) + @($runtimes)
        & (Join-Path $PSScriptRoot 'Copy-Windows7RuntimeClosure.ps1') -ProgramDirectory $bin -EntryPoints $entries -DependencyDirectories $dependencies -Destination $destination -Architecture $arch
        Copy-Item -LiteralPath $fontSource -Destination $destination -Recurse -Force
        [IO.File]::WriteAllText((Join-Path $destination 'edition.json'),("{`"edition`":`"$Edition`"}"),[Text.UTF8Encoding]::new($false))
        [ordered]@{ version = $version; edition = $Edition; architecture = $arch; minimum_windows = '6.1.7601'; settings = 'win32'; panels = 'win32' } |
            ConvertTo-Json | Set-Content -LiteralPath (Join-Path $destination 'windows7-product.json') -Encoding UTF8
        $runtime = Join-Path $output 'runtime'
        if (Test-Path -LiteralPath $runtime) { Remove-Item -LiteralPath $runtime -Recurse -Force }
        Move-Item -LiteralPath $destination -Destination $runtime
    }
    Write-Output "Win7 native products built and imports audited: $buildRoot; target-system installation and input validation remain required."
} finally { Pop-Location }
