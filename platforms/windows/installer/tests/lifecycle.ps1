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
# The WinUI 3 settings window, the shared Tauri panel shell and the MCP server are separate processes outside the Server's process tree; all of them live in the server directory and must be stopped before it is replaced.
$settings = [regex]::Match($script, '#define MySettingsExeName "([^"]+)"').Groups[1].Value
$shell = Get-Content -LiteralPath (Join-Path $PSScriptRoot '../../src/system/ShellSurfaces.h') -Raw -Encoding UTF8
$shellNames = [regex]::Match($shell, 'shell_executable_names\(const ShellSurfaceRequest &request\)\s*\{\s*if \(request\.panel\.empty\(\)\)\s*return\s*\{L"([^"]+)"')
if (-not $settings -or -not $shellNames.Success -or $shellNames.Groups[1].Value -ne $settings) {
    throw 'The installer does not name the Tauri executable the Server launches'
}
# Every edition, full included, stops the processes whose executable is inside its own server directory, not by image name: the editions' processes share names, and taskkill /IM would also stop the other editions installed side by side. The Watchdog stops first, or it restarts the Server.
$stop = Get-Block 'procedure StopImeProcesses;' 'procedure DeleteWatchdogLogonTask;'
$watchdogFirst = $stop.IndexOf("StopProcessesUnder(ServerDir, 'MetasequoiaImeWatchdog');")
$rest = $stop.IndexOf("StopProcessesUnder(ServerDir, '');")
if ($watchdogFirst -lt 0 -or $rest -lt 0 -or $watchdogFirst -gt $rest) {
    throw 'StopImeProcesses must stop the Watchdog first and then everything else under the server directory'
}
if (-not $stop.Contains("ServerDir := ExpandConstant('{commonpf64}\{#MyEditionInstallDir}\server');")) {
    throw 'StopImeProcesses does not stop processes under its own server directory'
}
if ($script.Contains('taskkill.exe') -or $script.Contains('procedure StopProcess(')) {
    throw 'The installer stops processes by image name, which also stops the other installed editions'
}
$prepare = Get-Block 'function PrepareToInstall' 'procedure CurStepChanged'
$uninstall = Get-Block 'procedure CurUninstallStepChanged' 'else if CurUninstallStep = usPostUninstall'
# ---- the 64-bit TIP's copy in System32 ----
# A Setup compiled with Inno Setup 7 is a 32-bit process whose [Code] file functions and Exec are redirected to SysWOW64, so the System32 copy is probed and removed only through tools launched with ExecNativeSys (ExecWithNativeSysDir under 7), never DirExists or DelTree. An upgrade keeps only its own version directory; uninstall removes them all. GetVersionDir avoids a version directory still waiting for its restart-time deletion.
$systemCleanup = Get-Block 'procedure TryDeleteSystemVersionDirs' 'procedure TryDeleteOldVersionDirs'
$nativePowerShell = "\{#ExecNativeSys\}\(\s*ExpandConstant\('\{sys\}\\WindowsPowerShell\\v1\.0\\powershell\.exe'\)"
if ($systemCleanup -notmatch $nativePowerShell -or
    -not $systemCleanup.Contains('[Environment]::Is64BitProcess') -or
    $systemCleanup.Contains('DelTree(')) {
    throw 'System32 TIP cleanup does not run in a 64-bit PowerShell'
}
$stopUnder = Get-Block 'procedure StopProcessesUnder' 'procedure StopImeProcesses'
if ($stopUnder -notmatch $nativePowerShell) {
    throw 'StopProcessesUnder starts a 32-bit PowerShell under Inno Setup 7, which cannot read 64-bit process paths'
}
$systemProbe = Get-Block 'function SystemDirExists' 'function GetVersionDir'
if ($systemProbe -notmatch "\{#ExecNativeSys\}\(\s*ExpandConstant\('\{sys\}\\cmd\.exe'\)") {
    throw 'SystemDirExists does not reach the native System32'
}
if (-not $prepare.Contains('TryDeleteSystemVersionDirs(VersionDirName);')) {
    throw 'An upgrade does not remove the older System32 TIP directories'
}
$postUninstallStart = $script.IndexOf('else if CurUninstallStep = usPostUninstall')
if ($postUninstallStart -lt 0 -or -not $script.Substring($postUninstallStart).Contains("TryDeleteSystemVersionDirs('');")) {
    throw 'Uninstall does not remove the System32 TIP directories'
}
$versionDir = Get-Block 'function GetVersionDir' 'function IsUserConfigFile'
if (-not $versionDir.Contains('SystemDirExists(ExpandConstant(') -or -not $versionDir.Contains("'{#MySystemTipDir}\' + Candidate))")) {
    throw 'GetVersionDir does not skip a System32 version directory that still exists'
}
foreach ($block in @($prepare, $uninstall)) {
    if (-not $block.Contains('StopImeProcesses;') -or $block.Contains('StopProcessesUnder(')) {
        throw 'Upgrade or uninstall stops processes outside StopImeProcesses'
    }
}
$flatPrepare = $prepare -replace '\s+', ' '
$serverRemoval = $flatPrepare.IndexOf("TryDeleteTree(ExpandConstant( '{commonpf64}\{#MyEditionInstallDir}\server'))")
if ($serverRemoval -lt 0 -or $flatPrepare.IndexOf('StopImeProcesses;') -gt $serverRemoval) {
    throw 'Upgrade removes the server directory before stopping its processes'
}

# ---- the uninstaller removes the data directory it recorded ----
# DataDir carries uninsdeletevalue, so it is gone by usPostUninstall; reading it there falls back to the default and a custom directory is never removed.
$initialize = Get-Block 'function InitializeUninstall' 'procedure StopProcessesUnder'
if (-not $initialize.Contains('ResolvePreviousDataDir;')) {
    throw 'InitializeUninstall does not capture DataDir before the registry values are removed'
}
$postStart = $script.IndexOf('else if CurUninstallStep = usPostUninstall')
if ($postStart -lt 0) { throw 'Missing installer block: usPostUninstall' }
$post = $script.Substring($postStart)
if (-not $post.Contains('if OwnsDataDir(ResolvePreviousDataDir) then') -or
    -not $post.Contains("DeleteDataDir(ResolvePreviousDataDir, '')") -or $post.Contains('GetDataDir(')) {
    throw 'usPostUninstall does not remove the data directory captured at uninstall start'
}
if ($script -notmatch 'ValueName: "DataDir";[^\r\n]*Flags: uninsdeletevalue') {
    throw 'DataDir registry value is no longer removed on uninstall'
}
# 几个版本可以和 msime-windows 同时安装。每个版本（包括 full）走同一个 OwnsDataDir，不认带着别的版本或 msime-windows 标记的目录，即使那是它自己的默认数据目录，所以不会接管、清理或删除别人的数据。
$owns = Get-Block 'function OwnsDataDir' 'procedure WriteDataDirMarker'
if ($script -notmatch "DataDirMarkerPrefix = '\{#MyDataDirMarkerPrefix\}';" -or
    -not $script.Contains("FindFirst(AddBackslash(Directory) + DataDirMarkerPrefix + '*', FindRec)") -or
    ([regex]::Matches($owns, [regex]::Escape('(not HasOtherEditionDataDirMarker(Directory)) and'))).Count -ne 1 -or
    $owns.Contains('#if')) {
    throw 'An edition may own a data directory that carries another edition''s marker'
}
# The marker check only looks at a directory's top level. One edition's data directory can still sit inside another's, so the installer refuses a data directory that overlaps another edition's (registered or default), and removing a data directory leaves another edition's directory inside it alone.
$validation = Get-Block 'function DataDirRejectionReason' 'procedure DataDirBrowseClick'
$remove = Get-Block 'procedure DeleteDataDir' 'function IsMigratedDataItem'
if (-not $validation.Contains('Overlap := OtherEditionDataDirWithin(Directory);') -or
    -not $validation.Contains('Overlap := OtherEditionDataDirAround(Directory);') -or
    -not $remove.Contains("if OtherEditionDataDirAround(Directory) <> '' then") -or
    -not $remove.Contains("(OtherEditionDataDirWithin(ItemPath) = '')") -or
    -not $script.Contains("RegistryKeys := '{#MyOtherEditionRegistryKeys}';") -or
    -not $script.Contains("InstallDirs := '{#MyOtherEditionInstallDirs}';")) {
    throw 'A data directory may overlap another edition''s, and removing it may delete the other edition''s data'
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
