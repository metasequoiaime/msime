# Win7 的核心和原生宿主来自不同构建，按架构保存实际配套的模块和 PDB，不复用现代版的 ARM64X 布局。
[CmdletBinding()]
param(
    [Parameter(Mandatory)][ValidatePattern('^[a-z][a-z0-9]*$')][string]$Edition,
    [Parameter(Mandatory)][ValidatePattern('^\d+\.\d+\.\d+$')][string]$Version,
    [Parameter(Mandatory)][string]$OutputDirectory,
    [string]$RepoRoot = (Split-Path -Parent (Split-Path -Parent (Split-Path -Parent $PSScriptRoot)))
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$RepoRoot = (Resolve-Path -LiteralPath $RepoRoot).Path
$revision = (& git -C $RepoRoot rev-parse HEAD).Trim()
if ($LASTEXITCODE -ne 0) { throw '无法读取符号对应的源码提交' }
$run = if ($env:GITHUB_RUN_ID) { $env:GITHUB_RUN_ID } else { 'local' }
$attempt = if ($env:GITHUB_RUN_ATTEMPT) { $env:GITHUB_RUN_ATTEMPT } else { '1' }
$scratch = Join-Path ([IO.Path]::GetTempPath()) ('msime-win7-symbols-' + [Guid]::NewGuid().ToString('N'))
try {
    $files = @()
    foreach ($arch in @('x64', 'x86')) {
        $destination = Join-Path $scratch $arch
        New-Item -ItemType Directory -Force -Path $destination | Out-Null
        $native = Join-Path $RepoRoot "target/windows7-$Edition/$arch/bin"
        $core = Join-Path $RepoRoot "target/windows7-core/$arch"
        $modules = @(
            @($core, 'msime_host_api.dll', 'msime_host_api.pdb'),
            @($core, 'msime-mcp.exe', 'msime-mcp.pdb'),
            @($native, 'MetasequoiaImeServer.exe', 'MetasequoiaImeServer.pdb'),
            @($native, 'MetasequoiaImeWatchdog.exe', 'MetasequoiaImeWatchdog.pdb'),
            @($native, 'msime-client-prepare.exe', 'msime-client-prepare.pdb'),
            @($native, 'MetasequoiaImeTsf.dll', 'MetasequoiaImeTsf.pdb'),
            @($native, 'MetasequoiaImeCrashWer.dll', 'MetasequoiaImeCrashWer.pdb'),
            @($native, 'msime-client-settings-win7.exe', 'msime-client-settings-win7.pdb'),
            @($native, 'msime-client-panels-win7.exe', 'msime-client-panels-win7.pdb')
        )
        foreach ($module in $modules) {
            foreach ($name in @($module[1], $module[2])) {
                $source = Get-Item -LiteralPath (Join-Path $module[0] $name)
                if ($source.Length -eq 0) { throw "符号或模块为空：$arch/$name" }
                Copy-Item -LiteralPath $source.FullName -Destination $destination
                $files += [ordered]@{ file = "$arch/$name"; sha256 = (Get-FileHash -LiteralPath $source.FullName -Algorithm SHA256).Hash.ToLowerInvariant() }
            }
        }
    }
    [ordered]@{ format = 1; platform = 'windows7'; version = $Version; edition = $Edition; revision = $revision; run = $run; attempt = $attempt; files = $files } |
        ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $scratch 'manifest.json') -Encoding utf8
    New-Item -ItemType Directory -Force -Path $OutputDirectory | Out-Null
    $archive = Join-Path $OutputDirectory "windows7-$Version-$Edition-$run-$attempt-symbols.zip"
    Compress-Archive -Path (Join-Path $scratch '*') -DestinationPath $archive -CompressionLevel Optimal -Force
    Write-Output $archive
} finally {
    Remove-Item -LiteralPath $scratch -Recurse -Force -ErrorAction SilentlyContinue
}
