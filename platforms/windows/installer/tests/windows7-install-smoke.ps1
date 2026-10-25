# 仅用于可丢弃的 Win7 SP1 测试机；兼容系统自带 PowerShell 2，不读取真实用户配置。
[CmdletBinding()]
param(
    [Parameter(Mandatory=$true)][string]$Installer,
    # 构建机从 shared/contracts/editions.json 生成的 Windows 标识 XML，避免复制产品标识。
    [Parameter(Mandatory=$true)][string]$IdentityFile,
    [ValidateSet('x64','x86')][string]$Architecture = 'x64',
    [string]$TestRoot = 'C:\msime-win7-tests\install-smoke'
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2
$principal = New-Object Security.Principal.WindowsPrincipal ([Security.Principal.WindowsIdentity]::GetCurrent())
if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) { throw 'Installer test requires an elevated test session' }
if (($Architecture -eq 'x64') -ne ([IntPtr]::Size -eq 8)) { throw 'Run the test in the package architecture on the matching OS' }
if ($Architecture -eq 'x86' -and $env:PROCESSOR_ARCHITEW6432) { throw 'A WOW64 session does not validate a 32-bit Windows installation' }
[xml]$document = [IO.File]::ReadAllText($IdentityFile)
$identity = $document.windows
foreach ($name in @('registry_key','install_dir','clsid','watchdog_task','host_dll','name_suffix','user_data_dir')) {
    if (-not $identity.GetAttribute($name)) { throw "Missing Windows identity: $name" }
}
$appKey = 'HKLM:\' + $identity.registry_key
if (Test-Path -LiteralPath $appKey) { throw 'Test requires a machine without this edition installed' }
$account = Join-Path $env:LOCALAPPDATA ($identity.user_data_dir + '\account')
if (Test-Path -LiteralPath $account) { throw 'Refusing to remove an existing account directory in an installation test' }
if (Test-Path -LiteralPath $TestRoot) { throw 'TestRoot must be a new directory' }
[IO.Directory]::CreateDirectory($TestRoot) | Out-Null
$dataDir = Join-Path $TestRoot 'data'
$program = Join-Path $env:ProgramFiles $identity.install_dir
$program32 = if ($Architecture -eq 'x64') { Join-Path ${env:ProgramFiles(x86)} $identity.install_dir } else { $program }
$systemTip = Join-Path $env:WINDIR ('System32\IME\' + $identity.install_dir)
$script:failures = 0
function Check([bool]$Condition, [string]$What) {
    if ($Condition) { 'PASS ' + $What } else { 'FAIL ' + $What; $script:failures++ }
}
function Install([string]$Directory, [string]$Log) {
    $arguments = '/VERYSILENT /SUPPRESSMSGBOXES /NORESTART /DATADIR="' + $Directory + '" /LOG="' + (Join-Path $TestRoot $Log) + '"'
    $process = Start-Process -FilePath $Installer -ArgumentList $arguments -PassThru
    # PowerShell 2 必须先缓存句柄，否则退出后 ExitCode 可能为空。
    $heldHandle = $process.Handle
    $process.WaitForExit()
    if ($process.ExitCode -ne 0) { throw ('Installer failed: ' + $process.ExitCode) }
}
function Inproc([string]$Root) {
    $key = $Root + '\CLSID\' + $identity.clsid + '\InprocServer32'
    if (Test-Path -LiteralPath $key) { (Get-Item -LiteralPath $key).GetValue('') } else { '' }
}
function Task {
    $scheduler = New-Object -ComObject Schedule.Service
    $scheduler.Connect()
    try { $scheduler.GetFolder('\').GetTask($identity.watchdog_task) } catch { $null }
}
Install $dataDir 'install.log'
$app = Get-ItemProperty -LiteralPath $appKey
Check ($app.DataDir -eq $dataDir) 'custom data directory recorded'
Check (Test-Path -LiteralPath (Join-Path $dataDir ('.metasequoiaime-data' + $identity.name_suffix))) 'data ownership marker present'
foreach ($name in @('MetasequoiaImeServer.exe','MetasequoiaImeWatchdog.exe','msime-client-prepare.exe',
    'msime-client-settings-win7.exe','msime-client-panels-win7.exe','msime-mcp.exe',
    'sherpa-onnx-c-api.dll','onnxruntime.dll','onnxruntime_providers_shared.dll',$identity.host_dll)) {
    Check (Test-Path -LiteralPath (Join-Path $program ('server\' + $name))) ('installed ' + $name)
}
foreach ($name in @('msime-client-settings.exe','MSIME.exe','Microsoft.UI.Xaml.dll')) {
    Check (-not (Test-Path -LiteralPath (Join-Path $program ('server\' + $name)))) ('no modern-only ' + $name)
}
$tip32 = Join-Path $program32 ($app.VersionDir + '\MetasequoiaImeTsf.dll')
Check ((Inproc 'HKLM:\SOFTWARE\Classes') -eq $(if ($Architecture -eq 'x64') { Join-Path $systemTip ($app.VersionDir + '\MetasequoiaImeTsf.dll') } else { $tip32 })) 'native COM registration'
if ($Architecture -eq 'x64') { Check ((Inproc 'HKLM:\SOFTWARE\WOW6432Node\Classes') -eq $tip32) 'WOW64 COM registration' }
Check (Test-Path -LiteralPath ('HKLM:\SOFTWARE\Microsoft\CTF\TIP\' + $identity.clsid)) 'TSF framework registration'
foreach ($powerShell in @((Join-Path $env:WINDIR 'System32\WindowsPowerShell\v1.0\powershell.exe')) + @($(if ($Architecture -eq 'x64') { Join-Path $env:WINDIR 'SysWOW64\WindowsPowerShell\v1.0\powershell.exe' }))) {
    $command = "[void][Activator]::CreateInstance([Type]::GetTypeFromCLSID([Guid]'" + $identity.clsid + "')); 'activated'"
    $result = & $powerShell -NoProfile -NonInteractive -Command $command
    Check ($LASTEXITCODE -eq 0 -and ($result -join '') -eq 'activated') 'registered TIP activates in child process'
}
$task = Task
Check ($null -ne $task) 'watchdog task exists'
if ($task) {
    [xml]$xml = $task.Xml
    Check ($xml.Task.Actions.Exec.Command.Trim('"') -eq (Join-Path $program 'server\MetasequoiaImeWatchdog.exe')) 'watchdog task preserves full executable path'
}
$rules = ([IO.Directory]::GetAccessControl($dataDir)).GetAccessRules($true,$true,[Security.Principal.SecurityIdentifier])
$modify = @($rules | Where-Object { $_.AccessControlType -eq 'Allow' -and $_.IdentityReference.Value -eq 'S-1-5-32-545' -and
    ($_.FileSystemRights -band [Security.AccessControl.FileSystemRights]::Modify) -eq [Security.AccessControl.FileSystemRights]::Modify })
Check ($modify.Count -gt 0) 'Users can modify installed data directory'
Check ((Get-AuthenticodeSignature -FilePath $app.ServerPath).Status -eq 'Valid') 'installed Server has a trusted test signature'
$seeded = @{'preferences.json'='{"synthetic_install_test":true}'; 'user\synthetic.txt'='synthetic user data'; 'skins\synthetic\skin.json'='{"synthetic":true}'}
foreach ($entry in $seeded.GetEnumerator()) {
    $path = Join-Path $dataDir $entry.Key
    [IO.Directory]::CreateDirectory((Split-Path -Parent $path)) | Out-Null
    [IO.File]::WriteAllText($path,$entry.Value)
}
Install $dataDir 'upgrade.log'
foreach ($entry in $seeded.GetEnumerator()) { Check ([IO.File]::ReadAllText((Join-Path $dataDir $entry.Key)) -eq $entry.Value) ('upgrade preserves ' + $entry.Key) }
$moved = Join-Path $dataDir 'moved'
[IO.File]::WriteAllText((Join-Path $dataDir 'runtime-options.json'),'synthetic stale runtime path')
Install $moved 'move.log'
foreach ($entry in $seeded.GetEnumerator()) { Check ([IO.File]::ReadAllText((Join-Path $moved $entry.Key)) -eq $entry.Value) ('move preserves ' + $entry.Key) }
Check (-not (Test-Path -LiteralPath (Join-Path $moved 'runtime-options.json'))) 'move excludes stale runtime paths'
$app = Get-ItemProperty -LiteralPath $appKey
Check ($app.DataDir -eq $moved) 'moved data directory recorded'
$uninstaller = Join-Path $program 'unins000.exe'
$arguments = '/VERYSILENT /SUPPRESSMSGBOXES /NORESTART /REMOVEDATA /LOG="' + (Join-Path $TestRoot 'uninstall.log') + '"'
Start-Process -FilePath $uninstaller -ArgumentList $arguments -Wait | Out-Null
$deadline = (Get-Date).AddMinutes(3)
while ((Test-Path -LiteralPath $uninstaller) -and (Get-Date) -lt $deadline) { Start-Sleep -Seconds 1 }
Check (-not (Test-Path -LiteralPath $program)) 'native program directory removed'
Check (-not (Test-Path -LiteralPath $program32)) '32-bit program directory removed'
if ($Architecture -eq 'x64') { Check (-not (Test-Path -LiteralPath $systemTip)) 'system TIP directory removed' }
Check ((Inproc 'HKLM:\SOFTWARE\Classes') -eq '') 'native COM registration removed'
if ($Architecture -eq 'x64') { Check ((Inproc 'HKLM:\SOFTWARE\WOW6432Node\Classes') -eq '') 'WOW64 COM registration removed' }
Check (-not (Test-Path -LiteralPath ('HKLM:\SOFTWARE\Microsoft\CTF\TIP\' + $identity.clsid))) 'TSF framework registration removed'
Check ($null -eq (Task)) 'watchdog task removed'
Check (-not (Test-Path -LiteralPath $moved)) 'owned custom data removed by explicit switch'
'FAILED=' + $script:failures
exit $script:failures
