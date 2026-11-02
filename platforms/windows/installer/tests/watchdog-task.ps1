$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$script = Get-Content -LiteralPath (Join-Path $PSScriptRoot '../msime_setup.iss') -Raw -Encoding UTF8
$script = $script -replace '\\\r?\n\s*', ' '

$create = [regex]::Match(
    $script,
    "(?s)function CreateWatchdogLogonTask: Boolean;.*?end;\r?\n\r?\n\{"
).Value
if (-not $create) { throw 'Watchdog task creation function is missing' }
if ($create -notmatch "/SC ONLOGON") { throw 'Watchdog task is not a logon task' }
if ($create -notmatch "/RL LIMITED") { throw 'Watchdog task must use a non-elevated run level' }
if ($create -notmatch "/IT") { throw 'Watchdog task must run only in an interactive session' }
# schtasks splits an unquoted /TR value at its first space, so the Program Files path has to carry its own quotes inside the /TR argument.
if (-not $create.Contains('/TR "\"'' + WatchdogPath + ''\""')) {
    throw 'Watchdog task action does not quote the executable path inside /TR'
}

# 登录任务默认 Priority 7、电池上不启动/切换即停止、72 小时后强制结束，创建后必须补上。
$configure = [regex]::Match(
    $script,
    "(?s)procedure ConfigureWatchdogLogonTask;.*?end;\r?\n\r?\nprocedure TryDeleteTree"
).Value
if (-not $configure) { throw 'Watchdog task priority configuration is missing' }
foreach ($setting in @('Priority', 'DisallowStartIfOnBatteries', 'StopIfGoingOnBatteries', 'ExecutionTimeLimit')) {
    if (-not $configure.Contains("Set-Setting `$s ''$setting''")) {
        throw "Watchdog task priority configuration never sets $setting"
    }
}
if ($configure -notmatch 'schtasks\.exe /Create /F /TN') {
    throw 'Watchdog task priority configuration does not re-register the task from its XML'
}
if ($configure -notmatch 'exit 1') {
    throw 'Watchdog task priority configuration does not fail when the settings do not stick'
}
# 只在创建成功之后配置；创建失败时任务不存在，配置只会再报一次错。
if ($script -notmatch '(?s)if CreateWatchdogLogonTask then\r?\n\s*ConfigureWatchdogLogonTask;') {
    throw 'Watchdog task priority configuration is not conditional on creating the task'
}

$delete = [regex]::Match(
    $script,
    "(?s)procedure DeleteWatchdogLogonTask;.*?end;\r?\n\r?\nprocedure EnsureImeUserDataDir"
).Value
if ($delete -notmatch '/Delete /F /TN') { throw 'Uninstall does not remove the watchdog task' }

# ---- what Task Scheduler actually stores ----
# Rebuild the exact argument string the installer hands to schtasks.exe from the Pascal expression, register it under a throwaway name, and read the action back. The static checks above only pin the text; this is the part that shows how schtasks parses it.
$expression = [regex]::Match($create, "(?s)Params :=(.*?);\s*\r?\n\s*if").Groups[1].Value
if (-not $expression) { throw 'Cannot find the schtasks argument expression' }
$tokens = [regex]::Matches($expression, "'(?:[^']|'')*'|\bWatchdogPath\b")
if (($expression -replace "'(?:[^']|'')*'|\bWatchdogPath\b|\+|\s", '') -ne '') {
    throw 'The schtasks argument expression uses something this probe cannot evaluate'
}
$watchdogPath = 'C:\Program Files\metasequoiaime\server\MetasequoiaImeWatchdog.exe'
$taskName = 'MSIME Watchdog Quote Probe ' + [Guid]::NewGuid().ToString('N')
$arguments = -join @($tokens | ForEach-Object {
    if ($_.Value -eq 'WatchdogPath') { $watchdogPath } else { $_.Value.Substring(1, $_.Value.Length - 2).Replace("''", "'") }
})
$arguments = $arguments.Replace('{#MyWatchdogTaskName}', $taskName)

$onWindows = [Environment]::OSVersion.Platform -eq [PlatformID]::Win32NT
$elevated = $onWindows -and ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole(
    [Security.Principal.WindowsBuiltInRole]::Administrator)
if (-not $elevated) {
    Write-Output 'SKIPPED: the Task Scheduler round trip needs an elevated Windows session'
    Write-Output 'Watchdog task registration and removal command contracts passed'
    exit 0
}

$schtasks = Join-Path $env:SystemRoot 'System32\schtasks.exe'
function Invoke-Schtasks([string]$Arguments) {
    # ProcessStartInfo.Arguments reaches the command line verbatim, like Inno's Exec; PowerShell's own argument passing would re-quote it.
    $info = [Diagnostics.ProcessStartInfo]::new($schtasks, $Arguments)
    $info.UseShellExecute = $false
    $info.RedirectStandardOutput = $true
    $info.RedirectStandardError = $true
    $process = [Diagnostics.Process]::Start($info)
    $output = $process.StandardOutput.ReadToEnd() + $process.StandardError.ReadToEnd()
    $process.WaitForExit()
    [pscustomobject]@{ ExitCode = $process.ExitCode; Output = $output }
}
function Get-TaskAction([string]$Name) {
    $query = Invoke-Schtasks "/Query /TN `"$Name`" /XML"
    if ($query.ExitCode -ne 0) { throw "schtasks /Query failed: $($query.Output)" }
    $exec = ([xml]$query.Output).Task.Actions.Exec
    [pscustomobject]@{
        Command = $exec['Command'].InnerText
        Arguments = if ($exec['Arguments']) { $exec['Arguments'].InnerText } else { '' }
    }
}

try {
    $created = Invoke-Schtasks $arguments
    if ($created.ExitCode -ne 0) { throw "schtasks /Create failed ($($created.ExitCode)): $($created.Output)" }
    $action = Get-TaskAction $taskName
    Write-Output "installer form stored Command=[$($action.Command)] Arguments=[$($action.Arguments)]"
    if ($action.Command.Trim('"') -ne $watchdogPath -or $action.Arguments -ne '') {
        throw 'Task Scheduler did not store the Watchdog path as one program without arguments'
    }

    # 把安装器配置优先级的 PowerShell 原样取出来跑一遍，读回任务自己存下的设置：脚本
    # 里写对了字段名不等于 Task Scheduler 收下了它们。
    $scriptExpression = [regex]::Match($configure, "(?s)Script :=(.*?);\s*\r?\n\s*Params :=").Groups[1].Value
    if (-not $scriptExpression) { throw 'Cannot find the priority configuration script expression' }
    if (($scriptExpression -replace "'(?:[^']|'')*'|\+|\s", '') -ne '') {
        throw 'The priority configuration script uses something this probe cannot evaluate'
    }
    $configureScript = -join @([regex]::Matches($scriptExpression, "'(?:[^']|'')*'") | ForEach-Object {
        $_.Value.Substring(1, $_.Value.Length - 2).Replace("''", "'")
    })
    $configureScript = $configureScript.Replace('{#MyWatchdogTaskName}', $taskName)
    $encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($configureScript))
    $shell = (Get-Process -Id $PID).Path
    $configured = Start-Process -FilePath $shell -ArgumentList @(
        '-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass', '-EncodedCommand', $encoded) -Wait -PassThru
    if ($configured.ExitCode -ne 0) {
        throw "Priority configuration failed with exit code $($configured.ExitCode)"
    }
    $stored = (Get-ScheduledTask -TaskName $taskName).Settings
    Write-Output ("stored priority=$($stored.Priority) limit=$($stored.ExecutionTimeLimit) " +
                  "startOnBatteries=$(-not $stored.DisallowStartIfOnBatteries) " +
                  "keepOnBatteries=$(-not $stored.StopIfGoingOnBatteries)")
    if ($stored.Priority -ne 4 -or $stored.ExecutionTimeLimit -ne 'PT0S' -or
        $stored.DisallowStartIfOnBatteries -or $stored.StopIfGoingOnBatteries) {
        throw 'Priority configuration did not survive round-tripping through the task XML'
    }

    # For the record only: the unquoted form this replaced.
    $unquoted = $arguments.Replace('/TR "\"', '/TR "').Replace('\""', '"')
    if ((Invoke-Schtasks $unquoted).ExitCode -eq 0) {
        $action = Get-TaskAction $taskName
        Write-Output "unquoted form stored Command=[$($action.Command)] Arguments=[$($action.Arguments)]"
    }
} finally {
    $null = Invoke-Schtasks "/Delete /F /TN `"$taskName`""
}

Write-Output 'Watchdog task registration and removal command contracts passed'
