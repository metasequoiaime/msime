$ErrorActionPreference = 'Stop'
$root = Join-Path ([IO.Path]::GetTempPath()) ('msime-cargo-home-' + [Guid]::NewGuid())
$previous = $env:CARGO_HOME
$entry = Join-Path $PSScriptRoot '../../Use-ShortCargoHome.ps1'
try {
    $source = Join-Path $root 'cache with spaces'
    $short = Join-Path $root 'c'
    $environmentFile = Join-Path $root 'github-env'
    New-Item -ItemType Directory -Path $source -Force | Out-Null
    [IO.File]::WriteAllText((Join-Path $source 'synthetic-cache'), 'cached')
    & $entry -CargoHome $source -ShortHome $short -EnvironmentFile $environmentFile
    if ($env:CARGO_HOME -ne $short -or [IO.File]::ReadAllText((Join-Path $short 'synthetic-cache')) -ne 'cached') {
        throw 'Short home did not preserve the existing cache'
    }
    if (-not ([IO.File]::ReadAllText($environmentFile).Contains("CARGO_HOME=$short"))) { throw 'Missing CI environment export' }
    # 同一来源可重复调用；已生效的 CARGO_HOME 不会被重新联接到自身。
    & $entry -CargoHome $source -ShortHome $short -EnvironmentFile ''
    & $entry -ShortHome $short -EnvironmentFile ''
    $other = Join-Path $root 'other'
    New-Item -ItemType Directory -Path $other | Out-Null
    $rejected = $false
    try { & $entry -CargoHome $other -ShortHome $short -EnvironmentFile '' } catch { $rejected = $true }
    if (-not $rejected -or [IO.File]::ReadAllText((Join-Path $short 'synthetic-cache')) -ne 'cached') {
        throw 'Conflicting junction must remain untouched'
    }
    $rejected = $false
    try { & $entry -CargoHome $source -ShortHome $other -EnvironmentFile '' } catch { $rejected = $true }
    if (-not $rejected -or -not (Test-Path -LiteralPath $other)) { throw 'Existing directory must remain untouched' }
    'Short Cargo home cache reuse, idempotence and conflict preservation passed'
} finally {
    $env:CARGO_HOME = $previous
    # 先仅删除联接，避免递归清理触及其目标。
    if (Test-Path -LiteralPath (Join-Path $root 'c')) { [IO.Directory]::Delete((Join-Path $root 'c')) }
    Remove-Item -LiteralPath $root -Recurse -Force -ErrorAction SilentlyContinue
}
