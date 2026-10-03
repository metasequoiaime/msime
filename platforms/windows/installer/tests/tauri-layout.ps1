$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$script = Get-Content -LiteralPath (Join-Path $PSScriptRoot '../msime_setup.iss') -Raw
$script = $script -replace '\\\r?\n\s*', ' '
$records = [regex]::Matches($script, '(?m)^Source:[^\r\n]*')
$data = @($records | Where-Object { $_.Value.Contains('\app_data\*') })
if ($data.Count -ne 1 -or -not $data[0].Value.Contains('\config.toml')) {
    throw 'Full package does not exclude user configuration'
}
$server = @($records | Where-Object { $_.Value.Contains('\server_exe\*') })
if ($server.Count -ne 1 -or -not $server[0].Value.Contains('recursesubdirs')) {
    throw 'Missing native WinUI/Tauri executable installation rule'
}
# The voice runtime DLLs carry upstream version resources; an upgrade must replace them with the pinned build even when an older one reports a higher version.
if (-not $server[0].Value.Contains('ignoreversion')) {
    throw 'Server files, including the voice runtime, can be kept back on upgrade'
}
if (-not $script.Contains('#define MySettingsExeName "msime-client-settings.exe"') -or
    -not $script.Contains('{#MySettingsExeName}')) {
    throw 'Start Menu shortcut does not target the staged WinUI settings executable'
}
# 逐条点名，而不是数数：#2830 去掉单独安装 config.default.toml 的记录后，原来「至少 3 条」的计数就失效了；计数每少一条记录就要跟着改，也分辨不出少的是哪一条。
$dataDirRecords = @($records | Where-Object { $_.Value.Contains('{code:GetDataDir}') })
$appData = @($dataDirRecords | Where-Object { $_.Value.Contains('\app_data\*') })
$userConfig = @($dataDirRecords | Where-Object {
        $_.Value.Contains('\app_data\config.default.toml') -and
        $_.Value.Contains('DestName: "config.toml"') -and
        $_.Value.Contains('onlyifdoesntexist')
    })
if ($appData.Count -ne 1 -or $userConfig.Count -ne 1) {
    throw 'Installer resources do not follow the selected DataDir'
}
if (-not $script.Contains('ValueName: "DataDir"') -or
    -not $script.Contains('{param:DATADIR|}')) {
    throw 'Installer does not persist or accept the selected DataDir'
}
if (-not $script.Contains('function DataDirRejectionReason') -or
    -not $script.Contains('DataDirRejectionReason(GetDataDir')) {
    throw 'Installer does not validate the selected DataDir'
}
if (-not $script.Contains("DataDirMarkerName = '{#MyEditionDataDirMarker}'") -or
    -not $script.Contains('function OwnsDataDir') -or
    -not $script.Contains('WriteDataDirMarker(GetDataDir')) {
    throw 'Installer does not protect user-owned data directories with a marker'
}
if (-not $script.Contains('if not OwnsDataDir(AppDataPath) then')) {
    throw 'Installer cleanup is not guarded by data-directory ownership'
}
if (-not $script.Contains('function MigrateUserDataDir') -or
    -not $script.Contains('robocopy.exe') -or
    -not $script.Contains('MigrateUserDataDir(ResolvePreviousDataDir')) {
    throw 'Installer does not migrate user data when DataDir changes'
}
if ($script.Contains('/MOVE') -or
    -not $script.Contains("Log('Copying user data; the previous directory is kept until installation succeeds.')") -or
    -not $script.Contains('DataDirMigrated := True') -or
    -not $script.Contains('procedure FinishDataDirMove')) {
    throw 'Installer migration must retain the previous data directory for recovery'
}
if (-not $script.Contains('RobocopySucceeded') -or
    -not $script.Contains('原目录中的数据保持不变')) {
    throw 'Installer migration must fail closed when user data copy fails'
}
Write-Output 'Installer carries native WinUI/Tauri outputs'
