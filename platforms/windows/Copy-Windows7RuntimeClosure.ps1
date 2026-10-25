[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$ProgramDirectory,
    [Parameter(Mandatory)][string[]]$EntryPoints,
    [Parameter(Mandatory)][string[]]$DependencyDirectories,
    [Parameter(Mandatory)][string]$Destination,
    [Parameter(Mandatory)][ValidateSet('x64', 'x86')][string]$Architecture
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$audit = Join-Path $PSScriptRoot 'Test-Windows7CoreImports.ps1'
$pe = Join-Path $PSScriptRoot 'Test-PortableExecutable.ps1'
foreach ($directory in @($ProgramDirectory) + $DependencyDirectories) {
    if (-not [IO.Path]::IsPathRooted($directory) -or -not (Test-Path -LiteralPath $directory -PathType Container)) {
        throw 'Win7 runtime sources must be absolute existing directories'
    }
}
if (-not [IO.Path]::IsPathRooted($Destination) -or (Test-Path -LiteralPath $Destination)) {
    throw 'Win7 runtime destination must be an absolute new directory'
}
$candidates = [Collections.Generic.Dictionary[string,object]]::new([StringComparer]::OrdinalIgnoreCase)
foreach ($directory in @($ProgramDirectory) + $DependencyDirectories) {
    foreach ($file in @(Get-ChildItem -LiteralPath $directory -File | Where-Object { $_.Extension -ieq '.dll' })) {
        if ($file.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Linked runtime dependency is not allowed' }
        if ($candidates.ContainsKey($file.Name)) {
            if ((Get-FileHash -LiteralPath $file.FullName).Hash -ne (Get-FileHash -LiteralPath $candidates[$file.Name].FullName).Hash) {
                throw "Conflicting Win7 runtime dependency: $($file.Name)"
            }
        } else { $candidates.Add($file.Name, $file) }
    }
}
$queue = [Collections.Generic.Queue[object]]::new()
$selected = [Collections.Generic.Dictionary[string,object]]::new([StringComparer]::OrdinalIgnoreCase)
$reports = [Collections.Generic.List[object]]::new()
$required = [Collections.Generic.Dictionary[string,object]]::new([StringComparer]::OrdinalIgnoreCase)
$processed = [Collections.Generic.Dictionary[string,int]]::new([StringComparer]::OrdinalIgnoreCase)
foreach ($name in $EntryPoints) {
    if ([IO.Path]::GetFileName($name) -cne $name -or [IO.Path]::GetExtension($name) -notin '.exe', '.dll') {
        throw 'Win7 entry points must be plain executable or DLL file names'
    }
    $file = Get-Item -LiteralPath (Join-Path $ProgramDirectory $name)
    if ($file.PSIsContainer) { throw 'Win7 entry point must be a file' }
    $queue.Enqueue($file)
}
if ($queue.Count -eq 0) { throw 'No Win7 program images to collect' }
while ($queue.Count -ne 0) {
    $file = $queue.Dequeue()
    $references = @(if ($required.ContainsKey($file.Name)) { $required[$file.Name] })
    if ($processed.ContainsKey($file.Name) -and $processed[$file.Name] -eq $references.Count) { continue }
    if ($file.Attributes -band [IO.FileAttributes]::ReparsePoint) { throw 'Linked program image is not allowed' }
    $kind = if ($file.Extension -ieq '.dll') { 'dll' } else { 'exe' }
    & $pe -LiteralPath $file.FullName -Architecture $Architecture -Kind $kind
    # 转发导出与序号导入均按实际候选目录核验。此阶段不执行任何待打包的 PE。
    $report = & $audit -LiteralPath $file.FullName -DependencyDirectory $file.DirectoryName -RequiredExports $references
    $selected[$file.Name] = $file
    $processed[$file.Name] = $references.Count
    foreach ($entry in $report.Imports) {
        # 已知延迟导入由 Win7 核心钩子处理，不能为它从 SDK 复制新版系统 DLL。
        if (-not $entry.Delayed -and $candidates.ContainsKey($entry.Library)) {
            if (-not $required.ContainsKey($entry.Library)) {
                $required[$entry.Library] = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
            }
            [void]$required[$entry.Library].Add($entry.Name)
            $queue.Enqueue($candidates[$entry.Library])
        }
    }
}
# 整个闭包核验成功后再落盘；失败撤去本次目录，不能残留“成功”的半包。
New-Item -ItemType Directory -Path $Destination | Out-Null
try {
    foreach ($file in $selected.Values) {
        [IO.File]::Copy($file.FullName, (Join-Path $Destination $file.Name), $false)
    }
    # 重新从实际目标目录检查，避免候选目录通过而装包时丢失转发目标。
    foreach ($file in $selected.Values) {
        $references = @(if ($required.ContainsKey($file.Name)) { $required[$file.Name] })
        $reports.Add((& $audit -LiteralPath (Join-Path $Destination $file.Name) -DependencyDirectory $Destination -RequiredExports $references))
    }
    $manifest = [ordered]@{
        architecture = $Architecture
        files = @($selected.Values | Sort-Object Name | ForEach-Object {
            [ordered]@{ name = $_.Name; sha256 = (Get-FileHash -LiteralPath (Join-Path $Destination $_.Name)).Hash.ToLowerInvariant() }
        })
        imports = @($reports.ToArray())
    }
    $manifest | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $Destination 'windows7-runtime-closure.json') -Encoding UTF8
    $rows = @($reports.ToArray() | ForEach-Object {
        $file = $_.File
        $_.Imports | ForEach-Object {
            $ordinal = if ($null -eq $_.Ordinal) { '-' } else { [string]$_.Ordinal }
            $delayed = if ($_.Delayed) { '1' } else { '0' }
            "$file|$($_.Library)|$($_.Name)|$ordinal|$delayed"
        }
    })
    $rows | Set-Content -LiteralPath (Join-Path $Destination 'windows7-imports.txt') -Encoding Ascii
} catch {
    Remove-Item -LiteralPath $Destination -Recurse -Force
    throw
}
Write-Output "Windows 7 runtime closure collected: $($selected.Count) images ($Architecture); system exports still require Win7 validation."
