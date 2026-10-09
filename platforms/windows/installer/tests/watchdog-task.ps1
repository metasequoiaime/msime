$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest

$script = Get-Content -LiteralPath (Join-Path $PSScriptRoot '../msime_setup.iss') -Raw -Encoding UTF8
$script = $script -replace '\\\r?\n\s*', ' '

$create = [regex]::Match(
    $script,
    "(?s)procedure CreateWatchdogLogonTask;.*?end;\r?\n\r?\nprocedure TryDeleteTree"
).Value
if (-not $create) { throw 'Watchdog task creation procedure is missing' }
if ($create -notmatch "/SC ONLOGON") { throw 'Watchdog task is not a logon task' }
if ($create -notmatch "/RL LIMITED") { throw 'Watchdog task must use a non-elevated run level' }
if ($create -notmatch "/IT") { throw 'Watchdog task must run only in an interactive session' }
# schtasks splits an unquoted /TR value at its first space, so the Program Files path has to carry its own quotes inside the /TR argument.
if (-not $create.Contains('/TR "\"'' + WatchdogPath + ''\""')) {
    throw 'Watchdog task action does not quote the executable path inside /TR'
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
