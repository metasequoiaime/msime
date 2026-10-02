$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
# Upgrade and uninstall contracts in msime_setup.iss: which processes are stopped, which data directory the uninstaller removes, and which permissions the data directory is given. These read the script; they do not install anything.
$script = Get-Content -LiteralPath (Join-Path $PSScriptRoot '../msime_setup.iss') -Raw -Encoding utf8
$script = $script -replace '\\\r?\n\s*', ' '
function Get-Block([string]$Begin, [string]$End) {
    $first = $script.IndexOf($Begin)
    $last = if ($first -ge 0) { $script.IndexOf($End, $first + $Begin.Length) } else { -1 }
    if ($first -lt 0 -or $last -lt 0) { throw "Missing installer block: $Begin" }
    $script.Substring($first, $last - $first)
}

# ---- processes stopped before files are replaced or removed ----
# The WinUI 3 settings window and the shared Tauri panel shell are separate processes; both must be stopped before the Server directory is replaced.
$settings = [regex]::Match($script, '#define MySettingsExeName "([^"]+)"').Groups[1].Value
if ([regex]::Match($script, '#define MyMcpName +"([^"]+)"').Groups[1].Value -ne 'msime-mcp.exe') {
    throw 'The installer does not name the MCP server it stops'
}
$shell = Get-Content -LiteralPath (Join-Path $PSScriptRoot '../../src/system/ShellSurfaces.h') -Raw
$shellNames = [regex]::Match($shell, 'shell_executable_names\(\)\s*\{\s*return\s*\{L"([^"]+)"')
if (-not $settings -or -not $shellNames.Success -or $shellNames.Groups[1].Value -ne $settings) {
    throw 'The installer does not name the Tauri executable the Server launches'
}
$stop = Get-Block 'procedure StopImeProcesses;' 'procedure DeleteWatchdogLogonTask;'
$order = @("StopProcess('{#MyWatchdogName}')", "StopProcess('{#MyAppExeName}')", "StopProcess('{#MySettingsExeName}')",
    "StopProcess('MSIME.exe')", "StopProcess('{#MyMcpName}')" |
    ForEach-Object { $stop.IndexOf($_) })
if ($order -contains -1) { throw 'StopImeProcesses does not stop the Watchdog, the Server, the WinUI settings window, the Tauri shell and the MCP server' }
if ($order[0] -gt $order[1]) { throw 'The Watchdog must stop before the Server, or it restarts it' }
$prepare = Get-Block 'function PrepareToInstall' 'procedure CurStepChanged'
$uninstall = Get-Block 'procedure CurUninstallStepChanged' 'else if CurUninstallStep = usPostUninstall'
foreach ($block in @($prepare, $uninstall)) {
    if (-not $block.Contains('StopImeProcesses;') -or $block.Contains("StopProcess('")) {
        throw 'Upgrade or uninstall stops processes outside StopImeProcesses'
    }
}
$flatPrepare = $prepare -replace '\s+', ' '
$serverRemoval = $flatPrepare.IndexOf("TryDeleteTree(ExpandConstant( '{commonpf64}\metasequoiaime\server'))")
if ($serverRemoval -lt 0 -or $flatPrepare.IndexOf('StopImeProcesses;') -gt $serverRemoval) {
    throw 'Upgrade removes the server directory before stopping its processes'
}

# ---- the uninstaller removes the data directory it recorded ----
# DataDir carries uninsdeletevalue, so it is gone by usPostUninstall; reading it there falls back to the default and a custom directory is never removed.
$initialize = Get-Block 'function InitializeUninstall' 'procedure StopProcess'
if (-not $initialize.Contains('ResolvePreviousDataDir;')) {
    throw 'InitializeUninstall does not capture DataDir before the registry values are removed'
}
$postStart = $script.IndexOf('else if CurUninstallStep = usPostUninstall')
if ($postStart -lt 0) { throw 'Missing installer block: usPostUninstall' }
$post = $script.Substring($postStart)
if (-not $post.Contains('if OwnsDataDir(ResolvePreviousDataDir) then') -or
    -not $post.Contains('TryDeleteTree(ResolvePreviousDataDir)') -or $post.Contains('GetDataDir(')) {
    throw 'usPostUninstall does not remove the data directory captured at uninstall start'
}
if ($script -notmatch 'ValueName: "DataDir";[^\r\n]*Flags: uninsdeletevalue') {
    throw 'DataDir registry value is no longer removed on uninstall'
}
# A failed upgrade must not strip the marker that lets a retry or the uninstaller recognise the directory.
$preserved = Get-Block 'function IsPreservedAppDataItem' 'function InitializeUninstall'
if (-not $preserved.Contains('(CompareText(FileName, DataDirMarkerName) = 0)')) {
    throw 'Upgrade cleanup deletes the data-directory ownership marker'
}

# ---- the medium-integrity Server and settings window can write the data directory ----
if ($script -notmatch '(?m)^Name: "\{code:GetDataDir\}"; Permissions: users-modify') {
    throw 'The data directory is not created with Users modify permission'
}
$ensure = Get-Block 'procedure EnsureImeUserDataDir;' 'procedure CreateWatchdogLogonTask;'
foreach ($needle in @("AppDataPath := GetDataDir('')", '/grant *S-1-5-32-545:(OI)(CI)M /T /C /Q', '/setintegritylevel (OI)(CI)M /T /C /Q')) {
    if (-not $ensure.Contains($needle)) { throw "EnsureImeUserDataDir is missing $needle" }
}
$postInstall = Get-Block 'if CurStep = ssPostInstall then' 'procedure CurUninstallStepChanged'
$networkChoice = $postInstall.IndexOf('ApplyNetworkChoiceToUserConfig;')
$permissions = $postInstall.IndexOf('EnsureImeUserDataDir;')
if ($permissions -lt 0 -or $networkChoice -lt 0 -or $permissions -lt $networkChoice) {
    throw 'Data-directory permissions are not applied after everything the installer writes there'
}

Write-Output 'Installer stops every IME process, removes the recorded DataDir, and opens it to the user'
