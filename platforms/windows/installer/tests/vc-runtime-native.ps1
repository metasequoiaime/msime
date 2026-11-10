[CmdletBinding()]
param([switch]$InstallCompilers)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$repo = (Resolve-Path (Join-Path $PSScriptRoot '../../../..')).Path
$root = Join-Path $repo 'target/windows-installer-tests/vc-runtime'
New-Item -ItemType Directory -Force -Path $root | Out-Null
$compilers = @(
    @{ Major = 6; Tag = 'is-6_7_1'; File = 'innosetup-6.7.1.exe'; Hash = '4d11e8050b6185e0d49bd9e8cc661a7a59f44959a621d31d11033124c4e8a7b0' },
    @{ Major = 7; Tag = 'is-7_1_1'; File = 'innosetup-7.1.1-x64.exe'; Hash = '8a5a6711c3766a6e5fb74f280ba0e3551e719ce94538d901fdbd1f8f9920b3f0' }
)

foreach ($compiler in $compilers) {
    $directory = Join-Path $root "inno$($compiler.Major)"
    $compiler.Path = Join-Path $directory 'ISCC.exe'
    if (-not (Test-Path -LiteralPath $compiler.Path)) {
        if (-not $InstallCompilers) { throw "缺少测试编译器 $($compiler.Path)，用 -InstallCompilers 安装固定版本" }
        $download = Join-Path $root $compiler.File
        Invoke-WebRequest "https://github.com/jrsoftware/issrc/releases/download/$($compiler.Tag)/$($compiler.File)" -OutFile $download
        if ((Get-FileHash $download -Algorithm SHA256).Hash.ToLowerInvariant() -ne $compiler.Hash) {
            throw 'Inno Setup 下载摘要不匹配'
        }
        $install = Start-Process -FilePath $download -ArgumentList @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', "/DIR=`"$directory`"") -Wait -PassThru
        if ($install.ExitCode -ne 0) { throw "Inno Setup 安装失败：$($install.ExitCode)" }
    }
}

# 夹具只携带固定版本资源，不加载 DLL，也不改系统运行库或注册表。
$versions = [ordered]@{
    'old' = '14.29.30133.0'
    'old-minor' = '14.50.35719.0'
    'before-minimum' = '14.51.36230.65535'
    'minimum' = '14.51.36231.0'
    'reporter-fixed' = '14.51.36247.0'
    'new-minor' = '14.52.0.0'
}
foreach ($entry in $versions.GetEnumerator()) {
    $file = Join-Path $root "$($entry.Key).dll"
    if (Test-Path -LiteralPath $file) { Remove-Item -LiteralPath $file }
    $class = 'SyntheticRuntime' + $entry.Key.Replace('-', '')
    Add-Type -TypeDefinition "[assembly: System.Reflection.AssemblyFileVersion(`"$($entry.Value)`")] public class $class {}" -OutputAssembly $file
}
[IO.File]::WriteAllText((Join-Path $root 'plain.txt'), 'synthetic unversioned fixture')
if (-not [Environment]::Is64BitProcess) { throw '测试必须从 64 位 PowerShell 运行，才能独立读取原生 System32 作为对照' }
$kernelHash = (Get-FileHash (Join-Path $env:windir 'System32/kernel32.dll') -Algorithm SHA256).Hash.ToLowerInvariant()

foreach ($compiler in $compilers) {
    $modes = if ($compiler.Major -eq 7) { @('x86', 'x64') } else { @('x86') }
    foreach ($mode in $modes) {
        $arguments = @("/DFixtureRoot=$root", "/DExpectedKernelHash=$kernelHash", "/O$root")
        if ($mode -eq 'x64') { $arguments += '/DNativeProbe=1' }
        & $compiler.Path @arguments (Join-Path $PSScriptRoot 'vc-runtime-probe.iss')
        if ($LASTEXITCODE -ne 0) { throw "Inno $($compiler.Major) $mode 编译失败" }
        $result = Join-Path $root 'result.txt'
        if (Test-Path -LiteralPath $result) { Remove-Item -LiteralPath $result }
        $probe = Start-Process (Join-Path $root 'vc-runtime-probe.exe') -ArgumentList @('/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART') -PassThru
        if (-not $probe.WaitForExit(60000)) { $probe.Kill(); throw '运行库探针超时' }
        if (-not (Test-Path -LiteralPath $result) -or (Get-Content -LiteralPath $result -Raw) -ne 'PASSED') {
            throw "Inno $($compiler.Major) $mode 运行库探针失败；结果位于 $result"
        }
        Write-Host "Inno $($compiler.Major) $mode：版本边界、缺失文件、无版本文件和原生系统目录通过"
    }
}

# 编译器的目录版本与 CRT 的补丁版本可能不同；读取实际头文件，禁止把过低门槛当作完成。
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
$headers = @(& $vswhere -latest -products * -find 'VC/Tools/MSVC/*/include/crtversion.h')
if ($LASTEXITCODE -ne 0 -or $headers.Count -eq 0) { throw '没有找到发布工具链的 crtversion.h' }
$declaredText = Get-Content -LiteralPath (Join-Path $PSScriptRoot '../VcRuntimePrerequisite.iss') -Raw
$declared = [version][regex]::Match($declaredText, "VCRuntimeMinimumVersion = '(.*?)'").Groups[1].Value
foreach ($header in $headers) {
    $content = Get-Content -LiteralPath $header -Raw
    $parts = foreach ($part in 'MAJOR', 'MINOR', 'BUILD', 'RBUILD') {
        $match = [regex]::Match($content, "(?m)^\s*#define\s+_VC_CRT_$($part)_VERSION\s+(\d+)")
        if (-not $match.Success) { throw "crtversion.h 缺少 $part" }
        $match.Groups[1].Value
    }
    $required = [version]($parts -join '.')
    Write-Host "工具链 CRT 文件版本：$required；安装器声明下限：$declared"
    if ($declared -lt $required) { throw '安装器下限早于构建工具链的 CRT；必须按此实际版本更新并重新验证' }
}
