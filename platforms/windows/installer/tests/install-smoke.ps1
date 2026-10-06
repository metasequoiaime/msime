param(
    [Parameter(Mandatory)][string]$Installer,
    # 安装包所属的版本（shared/contracts/editions.json 里有 Windows 段的 id）。CLSID、注册表键、看门狗任务名、安装目录和 host DLL 名都按它取。
    [ValidatePattern('^[a-z][a-z0-9]*$')][string]$Edition = 'full',
    # Windows on Arm only: an x64 pwsh.exe, which runs emulated, to activate the TIP from an x64 process as well. Windows on Arm ships no x64 PowerShell.
    [string]$X64PowerShell = ''
)
# Installs the built package silently on a disposable Windows machine, checks what it leaves on disk, in the registry and in Task Scheduler, then uninstalls it silently and checks the same places are clean. Requires an elevated session; the release runner is one.
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$Installer = (Resolve-Path -LiteralPath $Installer).Path

$editions = Get-Content -LiteralPath (Join-Path $PSScriptRoot '../../../../shared/contracts/editions.json') -Raw | ConvertFrom-Json
$identity = @($editions.editions | Where-Object { $_.id -ceq $Edition -and $null -ne $_.platforms.windows })
if ($identity.Count -ne 1) { throw "Edition $Edition has no Windows identifiers" }
$identity = $identity[0].platforms.windows
$clsid = $identity.clsid
$appKey = "HKLM:\$($identity.registry_key)"
$taskName = $identity.watchdog_task
$pf64 = Join-Path $env:ProgramFiles $identity.install_dir
$pf32 = Join-Path ${env:ProgramFiles(x86)} $identity.install_dir
# 数据目录所有权标记的文件名接版本的名字后缀（platforms/windows/scripts/edition_windows.py 的 data_dir_marker），full 是 .metasequoiaime-data.full。
$markerName = '.metasequoiaime-data' + $identity.name_suffix
$logs = Join-Path $env:RUNNER_TEMP "msime-install-smoke-$Edition"
New-Item -ItemType Directory -Force -Path $logs | Out-Null
$failures = [Collections.Generic.List[string]]::new()
function Check([bool]$Condition, [string]$What) {
    if ($Condition) { Write-Output "ok: $What" } else { Write-Output "FAIL: $What"; $failures.Add($What) }
}
function InprocServer([string]$ClassesRoot) {
    $key = "$ClassesRoot\CLSID\$clsid\InprocServer32"
    if (Test-Path -LiteralPath $key) { (Get-Item -LiteralPath $key).GetValue('') } else { $null }
}
function TaskExists { $null -ne (Get-ScheduledTask -TaskName $taskName -ErrorAction SilentlyContinue) }

# A custom data directory, not the default: the uninstaller used to re-read DataDir after its registry value was already gone and so only ever removed the default location.
$dataDir = Join-Path $env:RUNNER_TEMP "msime-smoke-data-$Edition"
if (Test-Path -LiteralPath $dataDir) { Remove-Item -LiteralPath $dataDir -Recurse -Force }

# ---- install ----
$process = Start-Process -FilePath $Installer -ArgumentList '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', "/DATADIR=`"$dataDir`"", "/LOG=`"$logs\install.log`"" -Wait -PassThru
if ($process.ExitCode -ne 0) { Get-Content -LiteralPath "$logs\install.log" -Tail 60; throw "installer exited with $($process.ExitCode)" }

$app = Get-ItemProperty -LiteralPath $appKey
$versionDir = $app.VersionDir
Check (-not [string]::IsNullOrWhiteSpace($versionDir)) 'VersionDir recorded in HKLM'
Check (Test-Path -LiteralPath $app.ServerPath -PathType Leaf) "ServerPath points at an installed file ($($app.ServerPath))"
Check (Test-Path -LiteralPath (Join-Path $app.DataDir 'config.toml') -PathType Leaf) 'user config.toml created in DataDir'
Check (Test-Path -LiteralPath (Join-Path $app.DataDir $markerName) -PathType Leaf) 'DataDir ownership marker written'
# 每个版本（包括 full）：所有权标记写着自己的版本 id，Server 目录里有版本声明。
$declaration = Join-Path $pf64 'server\edition.json'
$marker = Get-Content -LiteralPath (Join-Path $app.DataDir $markerName) -Raw
Check ($marker.Contains("(edition $Edition)")) 'DataDir ownership marker names the edition'
$declared = if (Test-Path -LiteralPath $declaration) { (Get-Content -LiteralPath $declaration -Raw | ConvertFrom-Json).edition } else { $null }
Check ($declared -eq $Edition) "server\edition.json declares $Edition"
# The three voice runtime libraries are what the Server loads for on-device speech recognition; Build-Client.ps1 stages them for every release package.
foreach ($name in 'MetasequoiaImeServer.exe', 'MetasequoiaImeWatchdog.exe', 'msime-client-settings.exe', 'MSIME.exe', 'msime-mcp.exe',
    'sherpa-onnx-c-api.dll', 'onnxruntime.dll', 'onnxruntime_providers_shared.dll') {
    Check (Test-Path -LiteralPath (Join-Path $pf64 "server\$name") -PathType Leaf) "server\$name installed"
}
# The MCP server an AI assistant starts from the install directory must run there, not only be copied; --version touches no state and prints to stderr.
$mcpVersion = (& (Join-Path $pf64 'server\msime-mcp.exe') --version 2>&1 | Out-String).Trim()
Check ($LASTEXITCODE -eq 0 -and $mcpVersion -like 'msime-mcp *') "installed msime-mcp.exe runs ($mcpVersion)"
$tip64 = Join-Path $pf64 "$versionDir\MetasequoiaImeTsf.dll"
$tip32 = Join-Path $pf32 "$versionDir\MetasequoiaImeTsf.dll"
Check (Test-Path -LiteralPath (Join-Path $pf64 "$versionDir\$($identity.host_dll)") -PathType Leaf) "64-bit $($identity.host_dll) installed beside the TSF DLL"
Check (Test-Path -LiteralPath (Join-Path $pf32 "$versionDir\$($identity.host_dll)") -PathType Leaf) "32-bit $($identity.host_dll) installed beside the TSF DLL"
# Server 目录的 x64 宿主 DLL 与 64 位 TIP 取自同一份暂存文件；包里不再在 server_exe 下另带一份。
Check (Test-Path -LiteralPath (Join-Path $pf64 "server\$($identity.host_dll)") -PathType Leaf) "server\$($identity.host_dll) installed"
Check (-not (Test-Path -LiteralPath (Join-Path $pf64 'server\MetasequoiaImeTsf.dll'))) 'no stray TSF DLL in the Server folder'
# TIP 会被加载进每个进程；它旁边只该有它的宿主 DLL 和运行时依赖，不该有设置程序的 Windows App SDK 或 Server 的语音运行时。
$tipNeighbours = @(Get-ChildItem -LiteralPath (Join-Path $pf64 $versionDir) -File -Include 'Microsoft.*', 'onnxruntime*', 'sherpa*' -Recurse -ErrorAction SilentlyContinue | ForEach-Object Name)
Check ($tipNeighbours.Count -eq 0) "64-bit TSF folder carries no Server-only DLLs ($($tipNeighbours -join ', '))"
# 符号是单独的发布资产。
$installedSymbols = @(Get-ChildItem -LiteralPath $pf64, $pf32 -Recurse -File -Include '*.pdb', '*.ilk' -ErrorAction SilentlyContinue | ForEach-Object FullName)
Check ($installedSymbols.Count -eq 0) "no PDB or .ilk installed ($($installedSymbols -join ', '))"
Check (Test-Path -LiteralPath $tip64 -PathType Leaf) '64-bit TSF DLL installed'
Check (Test-Path -LiteralPath $tip32 -PathType Leaf) '32-bit TSF DLL installed'
# Windows on Arm installs the Arm64X TIP in place of the x64 one, with the ARM64 host its native half imports beside it (msime_setup.iss).
$onArm = [Runtime.InteropServices.RuntimeInformation]::OSArchitecture -eq [Runtime.InteropServices.Architecture]::Arm64
$arm64Host = Join-Path $pf64 "$versionDir\$([IO.Path]::GetFileNameWithoutExtension($identity.host_dll))_arm64.dll"
function ImageIs([string]$Path, [string]$Architecture) {
    try { & (Join-Path $PSScriptRoot '../../Test-PortableExecutable.ps1') -LiteralPath $Path -Architecture $Architecture -Kind dll; $true } catch { $false }
}
if ($onArm) {
    Check (ImageIs $tip64 'arm64x') '64-bit TSF DLL is the Arm64X one'
    Check ((Test-Path -LiteralPath $arm64Host -PathType Leaf) -and (ImageIs $arm64Host 'arm64')) "ARM64 host installed beside the Arm64X TSF DLL ($arm64Host)"
} else {
    Check (ImageIs $tip64 'x64') '64-bit TSF DLL is the x64 one'
    Check (-not (Test-Path -LiteralPath $arm64Host)) 'no ARM64 host outside Windows on Arm'
}
# Create the TIP through its COM registration, as an application does, in a child process so that this one holds no DLL the uninstaller must remove. On Windows on Arm the native process loads the ARM64 half and its host, and an emulated x64 one the ARM64EC half and the x64 host.
function Activate([string]$PowerShell, [string]$What) {
    $output = (& $PowerShell -NoProfile -NonInteractive -Command "[void][Activator]::CreateInstance([Type]::GetTypeFromCLSID([Guid]'$clsid')); 'activated'" 2>&1 | Out-String).Trim()
    Check ($LASTEXITCODE -eq 0 -and $output -eq 'activated') "TIP activates in $What ($output)"
}
Activate (Get-Process -Id $PID).Path "a $([Runtime.InteropServices.RuntimeInformation]::ProcessArchitecture) process"
if ($onArm -and $X64PowerShell) { Activate $X64PowerShell 'an emulated x64 process' }
Check ((InprocServer 'HKLM:\SOFTWARE\Classes') -eq $tip64) '64-bit COM server registered to the installed DLL'
Check ((InprocServer 'HKLM:\SOFTWARE\WOW6432Node\Classes') -eq $tip32) '32-bit COM server registered to the installed DLL'
Check (Test-Path -LiteralPath "HKLM:\SOFTWARE\Microsoft\CTF\TIP\$clsid") 'TIP registered with the text services framework'
Check (TaskExists) 'watchdog logon task created'
Check ($app.DataDir -eq $dataDir) "DataDir recorded as the /DATADIR choice ($($app.DataDir))"
# schtasks splits an unquoted /TR at the first space; the stored action must be the whole Program Files path with no arguments.
$action = if (TaskExists) { @((Get-ScheduledTask -TaskName $taskName).Actions)[0] } else { $null }
$watchdog = Join-Path $pf64 'server\MetasequoiaImeWatchdog.exe'
Check ($null -ne $action -and $action.Execute.Trim('"') -eq $watchdog -and [string]::IsNullOrEmpty($action.Arguments)) "watchdog task runs the full Watchdog path ($(if ($action) { "$($action.Execute) | $($action.Arguments)" }))"
# The Server and settings window run at medium integrity and must be able to write what the elevated installer created.
$rules = (Get-Acl -LiteralPath $app.DataDir).GetAccessRules($true, $true, [Security.Principal.SecurityIdentifier])
$usersModify = @($rules | Where-Object {
    $_.AccessControlType -eq 'Allow' -and $_.IdentityReference.Value -eq 'S-1-5-32-545' -and
    ($_.FileSystemRights -band [Security.AccessControl.FileSystemRights]::Modify) -eq [Security.AccessControl.FileSystemRights]::Modify })
Check ($usersModify.Count -gt 0) 'DataDir grants Users modify'
$label = (& icacls $app.DataDir) -join "`n"
Check ($label.Contains('Mandatory Label\Medium Mandatory Level')) 'DataDir carries a medium integrity label'

# The notices must carry the supplemental Rust and npm sections that the release collects, not only the vcpkg prefixes and the repository notices.
$notices = Join-Path $pf64 'THIRD_PARTY_NOTICES.txt'
$text = if (Test-Path -LiteralPath $notices) { Get-Content -LiteralPath $notices -Raw -Encoding utf8 } else { '' }
Check ($text.Contains('Rust crates statically linked into the MSIME host library and binaries')) 'installed notices contain the Rust crate section'
Check ($text.Contains('npm packages bundled into the MSIME desktop settings frontend')) 'installed notices contain the npm package section'
Check (Test-Path -LiteralPath (Join-Path $pf64 'LICENSE.txt') -PathType Leaf) 'LICENSE.txt installed'

# ---- upgrade in place, then move the data directory ----
# DataDir is the Server state root, so a reinstall must keep everything that is not a package file, and choosing a new directory must move all of it, as the source installer does. The new directory is nested inside the old one on purpose: the source installer's recursive delete of the old directory would take the moved data with it.
function Install([string]$Directory, [string]$Log) {
    $run = Start-Process -FilePath $Installer -ArgumentList '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', "/DATADIR=`"$Directory`"", "/LOG=`"$logs\$Log`"" -Wait -PassThru
    if ($run.ExitCode -ne 0) { Get-Content -LiteralPath "$logs\$Log" -Tail 60; throw "installer exited with $($run.ExitCode)" }
}
$seeded = @{ 'preferences.json' = '{"smoke":"preferences"}'; 'user\smoke-user.txt' = 'user'; 'skins\smoke\skin.json' = 'skin' }
foreach ($item in $seeded.GetEnumerator()) {
    $path = Join-Path $dataDir $item.Key
    New-Item -ItemType Directory -Force -Path (Split-Path -Parent $path) | Out-Null
    Set-Content -LiteralPath $path -Value $item.Value -NoNewline -Encoding utf8
}
Set-Content -LiteralPath (Join-Path $dataDir 'runtime-options.json') -Value 'smoke-stale-runtime-options' -NoNewline -Encoding utf8
Install $dataDir 'upgrade.log'
foreach ($item in $seeded.GetEnumerator()) {
    $path = Join-Path $dataDir $item.Key
    Check ((Test-Path -LiteralPath $path -PathType Leaf) -and (Get-Content -LiteralPath $path -Raw) -eq $item.Value) "reinstall keeps $($item.Key)"
}
$movedDir = Join-Path $dataDir 'moved'
Install $movedDir 'move.log'
$app = Get-ItemProperty -LiteralPath $appKey
Check ($app.DataDir -eq $movedDir) "DataDir recorded as the new directory ($($app.DataDir))"
foreach ($item in $seeded.GetEnumerator()) {
    $path = Join-Path $movedDir $item.Key
    Check ((Test-Path -LiteralPath $path -PathType Leaf) -and (Get-Content -LiteralPath $path -Raw) -eq $item.Value) "move carries $($item.Key)"
}
$runtimeOptions = Join-Path $movedDir 'runtime-options.json'
Check (-not ((Test-Path -LiteralPath $runtimeOptions) -and (Get-Content -LiteralPath $runtimeOptions -Raw) -eq 'smoke-stale-runtime-options')) 'move leaves the old runtime-options.json behind'
# The package carries no dictionary in DataDir since #2830; its app_data items are the helpcodes, audio cues and built-in sound packs, so the default sound pack stands for them.
Check (Test-Path -LiteralPath (Join-Path $movedDir 'sound-packs\default\plugin.toml') -PathType Leaf) 'moved DataDir has the package app_data'
$left = @(Get-ChildItem -LiteralPath $dataDir -Force | ForEach-Object Name)
Check ($left.Count -eq 1 -and $left[0] -eq 'moved') "previous DataDir emptied around the nested new one ($($left -join ', '))"

# ---- uninstall ----
# The uninstaller relaunches itself from a temporary copy and returns at once, so wait for the program directory to go away instead of the process.
$uninstaller = Join-Path $pf64 'unins000.exe'
Check (Test-Path -LiteralPath $uninstaller -PathType Leaf) 'uninstaller present'
Start-Process -FilePath $uninstaller -ArgumentList '/VERYSILENT', '/SUPPRESSMSGBOXES', '/NORESTART', "/LOG=`"$logs\uninstall.log`"" -Wait
$deadline = (Get-Date).AddMinutes(3)
while ((Test-Path -LiteralPath $uninstaller) -and (Get-Date) -lt $deadline) { Start-Sleep -Seconds 2 }
Start-Sleep -Seconds 5

Check (-not (Test-Path -LiteralPath $pf64)) '64-bit program directory removed'
Check (-not (Test-Path -LiteralPath $pf32)) '32-bit program directory removed'
Check ($null -eq (InprocServer 'HKLM:\SOFTWARE\Classes')) '64-bit COM registration removed'
Check ($null -eq (InprocServer 'HKLM:\SOFTWARE\WOW6432Node\Classes')) '32-bit COM registration removed'
# DllUnregisterServer only removes the language profile and categories; the uninstaller then deletes the TIP key itself, so nothing of it may remain.
$tipKey = "HKLM:\SOFTWARE\Microsoft\CTF\TIP\$clsid"
Check (-not (Test-Path -LiteralPath $tipKey)) 'TIP registration removed'
if (Test-Path -LiteralPath $tipKey) { Get-ChildItem -LiteralPath $tipKey -Recurse | ForEach-Object { Write-Output "  left: $($_.Name)" } }
Check (-not (TaskExists)) 'watchdog logon task removed'
$remaining = @(if (Test-Path -LiteralPath $appKey) { (Get-Item -LiteralPath $appKey).GetValueNames() | Where-Object { $_ -in 'VersionDir', 'ServerPath', 'DataDir' } })
Check ($remaining.Count -eq 0) 'installer registry values removed'
Check (-not (Test-Path -LiteralPath $app.DataDir)) 'owned DataDir removed'

if ($failures.Count -gt 0) {
    foreach ($log in 'install.log', 'uninstall.log') { if (Test-Path -LiteralPath "$logs\$log") { Write-Output "---- $log (tail) ----"; Get-Content -LiteralPath "$logs\$log" -Tail 40 } }
    throw "$($failures.Count) install smoke check(s) failed"
}
Write-Output 'Installer installs, registers, and uninstalls cleanly'
