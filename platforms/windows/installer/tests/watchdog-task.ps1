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

# 登录任务默认 Priority 7、72 小时后强制结束，创建后必须补上；电池策略是单独的产品
# 决定，这次不动，脚本里也就不该出现那两个设置。
$configure = [regex]::Match(
    $script,
    "(?s)procedure ConfigureWatchdogLogonTask;.*?end;\r?\n\r?\nprocedure TryDeleteTree"
).Value
if (-not $configure) { throw 'Watchdog task priority configuration is missing' }
if ($configure -notmatch 'Settings\.Priority=4') {
    throw 'Watchdog task priority configuration never raises Priority to 4'
}
if ($configure -notmatch "Settings\.ExecutionTimeLimit=''PT0S''") {
    throw 'Watchdog task priority configuration never removes the execution time limit'
}
if ($configure -match 'Batteries') {
    throw 'Battery policy is a product decision and must not ride along with the priority fix'
}
if ($configure -notmatch 'RegisterTaskDefinition') {
    throw 'Watchdog task priority configuration does not re-register the task through its COM definition'
}
if ($configure -notmatch 'exit 1') {
    throw 'Watchdog task priority configuration does not fail when the settings do not stick'
}
# 注册失败后先看任务还在不在，只有真的没了才重建，避免安全软件把自启任务带走。
if ($configure -notmatch '\$null -eq \(Get-Definition \$n\)') {
    throw 'Watchdog task priority configuration does not check that the task survived a failed update'
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

    # 安装器用的是系统自带的 Windows PowerShell 加内联 -Command（Win7 上是 2.0，其它系统是
    # 5.1）。先按这条真实命令行跑一遍，确认脚本在安装器那条路径上也成立。
    if ($configureScript.Contains('"')) { throw 'The priority configuration script cannot be passed inline' }
    $systemShell = Join-Path $env:SystemRoot 'System32\WindowsPowerShell\v1.0\powershell.exe'
    $inline = '-NoProfile -NonInteractive -ExecutionPolicy Bypass -Command "' + $configureScript + '"'
    $configured = Start-Process -FilePath $systemShell -ArgumentList $inline -Wait -PassThru
    if ($configured.ExitCode -ne 0) {
        throw "Priority configuration through powershell.exe -Command failed with exit code $($configured.ExitCode)"
    }
    $configuredTask = Get-ScheduledTask -TaskName $taskName
    $stored = $configuredTask.Settings
    Write-Output ("stored priority=$($stored.Priority) limit=$($stored.ExecutionTimeLimit) " +
                  "runLevel=$($configuredTask.Principal.RunLevel) logon=$($configuredTask.Principal.LogonType)")
    if ($stored.Priority -ne 4 -or $stored.ExecutionTimeLimit -ne 'PT0S') {
        throw 'Priority configuration did not survive the COM update'
    }
    # COM 更新改的是同一条任务定义，触发器和主体必须原样保留。
    if ($configuredTask.Principal.LogonType -ne 'Interactive' -or
        $configuredTask.Principal.RunLevel -ne 'Limited' -or
        @($configuredTask.Triggers).Count -ne 1 -or
        @($configuredTask.Triggers)[0].CimClass.CimClassName -ne 'MSFT_TaskLogonTrigger') {
        throw 'Priority configuration did not preserve the logon trigger or the interactive principal'
    }

    # 再从默认 Priority 7 重建一次，用编码命令跑同一份脚本：两条调用路径都覆盖。
    $null = Invoke-Schtasks "/Delete /F /TN `"$taskName`""
    $recreated = Invoke-Schtasks $arguments
    if ($recreated.ExitCode -ne 0) { throw "schtasks /Create failed again ($($recreated.ExitCode)): $($recreated.Output)" }
    if ((Get-ScheduledTask -TaskName $taskName).Settings.Priority -ne 7) {
        throw 'The recreated task did not start from the default priority'
    }
    $encoded = [Convert]::ToBase64String([Text.Encoding]::Unicode.GetBytes($configureScript))
    $second = Start-Process -FilePath (Get-Process -Id $PID).Path -ArgumentList @(
        '-NoProfile', '-NonInteractive', '-ExecutionPolicy', 'Bypass', '-EncodedCommand', $encoded) -Wait -PassThru
    if ($second.ExitCode -ne 0) {
        throw "Priority configuration through -EncodedCommand failed with exit code $($second.ExitCode)"
    }
    if ((Get-ScheduledTask -TaskName $taskName).Settings.Priority -ne 4) {
        throw 'The encoded-command path did not raise the priority'
    }

    # 注册失败时任务必须还在：安装器的兜底只在它真的消失时才重建。这里用一个坏的
    # UserId 让第二次注册失败，确认失败不会带走任务或已生效的设置。
    $scheduler = New-Object -ComObject Schedule.Service
    $scheduler.Connect()
    $folder = $scheduler.GetFolder('\')
    $broken = $folder.GetTask($taskName).Definition
    $broken.Principal.UserId = 'S-1-5-21-0-0-0-9999'
    $rejected = $false
    try { $null = $folder.RegisterTaskDefinition($taskName, $broken, 6, $null, $null, 3) } catch { $rejected = $true }
    if (-not $rejected) { throw 'A broken principal was accepted; the failure path cannot be exercised' }
    $afterFailure = Get-ScheduledTask -TaskName $taskName
    if ($null -eq $afterFailure -or $afterFailure.Settings.Priority -ne 4) {
        throw 'A rejected registration lost the task or its settings'
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
