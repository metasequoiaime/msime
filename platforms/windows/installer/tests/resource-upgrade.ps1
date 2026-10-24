# 验证已加载 TIP 持有只读词库时的升级；仅创建合成文件，兼容 Win7 PowerShell 2。
param(
    [string]$IsccPath,
    [string]$ArtifactDirectory,
    [switch]$CompileOnly,
    [switch]$RunOnly
)
$ErrorActionPreference = 'Stop'
if (!$ArtifactDirectory) { $ArtifactDirectory = Join-Path ([IO.Path]::GetTempPath()) ('msime-resource-upgrade-' + [Guid]::NewGuid()) }
[void][IO.Directory]::CreateDirectory($ArtifactDirectory)
$setup = Join-Path $ArtifactDirectory 'resource-upgrade.exe'
if (!$RunOnly) {
    if (!$IsccPath) { $IsccPath = (Get-Command ISCC.exe -ErrorAction Stop).Definition }
    $source = [IO.File]::ReadAllText((Join-Path $PSScriptRoot '../msime_setup.iss'))
    $function = [regex]::Match($source, '(?s)function ResourceNeedsReplacement\(.*?(?=function InitializeUninstall)')
    if (!$function.Success) { throw 'Resource replacement function missing' }
    $fixture = Join-Path $ArtifactDirectory 'source.txt'
    [IO.File]::WriteAllText($fixture, 'synthetic pinned resource')
    $script = @'
#define SourceFile "SOURCE_FIXTURE"
[Setup]
AppName=MSIME resource upgrade test
AppVersion=1.0
DefaultDirName={tmp}\msime-resource-upgrade-unused
PrivilegesRequired=lowest
MinVersion=6.1sp1
Uninstallable=no
CreateAppDir=yes
CloseApplications=no
OutputBaseFilename=resource-upgrade
OutputDir=.
[Files]
Source: "{#SourceFile}"; DestDir: "{app}"; DestName: "same.db"; Flags: ignoreversion; Check: ResourceNeedsReplacement('{#GetSHA256OfFile(SourceFile)}')
Source: "{#SourceFile}"; DestDir: "{app}"; DestName: "changed.db"; Flags: ignoreversion; Check: ResourceNeedsReplacement('{#GetSHA256OfFile(SourceFile)}')
Source: "{#SourceFile}"; DestDir: "{app}"; DestName: "missing.db"; Flags: ignoreversion; Check: ResourceNeedsReplacement('{#GetSHA256OfFile(SourceFile)}')
[Code]
'@
    $script = $script.Replace('SOURCE_FIXTURE', $fixture) + "`r`n" + $function.Value
    $iss = Join-Path $ArtifactDirectory 'resource-upgrade.iss'
    [IO.File]::WriteAllText($iss, $script, [Text.Encoding]::UTF8)
    & $IsccPath $iss
    if ($LASTEXITCODE -ne 0) { throw 'Resource upgrade fixture compile failed' }
}
if ($CompileOnly) { 'RESOURCE_UPGRADE_FIXTURE_COMPILED'; exit 0 }
$sandbox = Join-Path $ArtifactDirectory ('installed-' + [Guid]::NewGuid())
[void][IO.Directory]::CreateDirectory($sandbox)
$same = Join-Path $sandbox 'same.db'
$changed = Join-Path $sandbox 'changed.db'
$missing = Join-Path $sandbox 'missing.db'
function Invoke-Fixture {
    $arguments = '/VERYSILENT /SUPPRESSMSGBOXES /NORESTART /DIR="' + $sandbox + '"'
    $process = Start-Process -FilePath $setup -ArgumentList $arguments -PassThru
    $heldHandle = $process.Handle
    $process.WaitForExit()
    return $process.ExitCode
}
try {
    [IO.File]::WriteAllText($same, 'synthetic pinned resource')
    [IO.File]::WriteAllText($changed, 'synthetic previous resource')
    $lock = [IO.File]::Open($same, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
    try {
        if ((Invoke-Fixture) -ne 0) { throw 'Identical locked resource prevented upgrade' }
        foreach ($path in @($same, $changed, $missing)) {
            if ([IO.File]::ReadAllText($path) -ne 'synthetic pinned resource') { throw 'Changed or missing resource was not installed' }
        }
        'IDENTICAL_LOCKED_RESOURCE_UPGRADE_PASSED'
    } finally { $lock.Dispose() }
    [IO.File]::WriteAllText($same, 'synthetic different locked resource')
    $lock = [IO.File]::Open($same, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
    try {
        if ((Invoke-Fixture) -eq 0) { throw 'Different locked resource was silently skipped' }
        if ([IO.File]::ReadAllText($same) -ne 'synthetic different locked resource') { throw 'Failed update damaged existing resource' }
        'DIFFERENT_LOCKED_RESOURCE_NOT_SKIPPED'
    } finally { $lock.Dispose() }
    'RESOURCE_UPGRADE_PASSED'
} finally { Remove-Item -LiteralPath $sandbox -Recurse -Force }
