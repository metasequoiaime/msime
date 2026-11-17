param([string]$IssPath = (Join-Path $PSScriptRoot '../msime_setup.iss'))
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
# 执行安装器真正拼出的 PowerShell 参数，不在测试里另写一份清理规则。
$installer = [IO.File]::ReadAllText($IssPath)
$start = $installer.IndexOf('procedure TryDeleteSystemVersionDirs')
$finish = $installer.IndexOf('procedure TryDeleteOldVersionDirs', $start)
$block = $installer.Substring($start, $finish - $start)
$start = $block.IndexOf("    '-NoProfile")
$finish = $block.IndexOf(",`n    '',", $start)
if ($finish -lt 0) { $finish = $block.IndexOf(",`r`n    '',", $start) }
if ($start -lt 0 -or $finish -lt $start) { throw '无法读取安装器清理命令参数' }
$expression = $block.Substring($start, $finish - $start)
function Invoke-InstallerCleanup([string]$Root, [string]$Keep = '') {
    $arguments = foreach ($token in [regex]::Matches($expression, "ExpandConstant\('(?:[^']|'')*'\)|\bKeep\b|'(?:[^']|'')*'")) {
        if ($token.Value.StartsWith('ExpandConstant(')) { $Root }
        elseif ($token.Value -eq 'Keep') { $Keep }
        else { $token.Value.Substring(1, $token.Length - 2).Replace("''", "'") }
    }
    $powershell = Join-Path ([Environment]::SystemDirectory) 'WindowsPowerShell/v1.0/powershell.exe'
    $process = Start-Process -FilePath $powershell -ArgumentList ($arguments -join '') -Wait -PassThru
    if ($process.ExitCode -ne 0) { throw '安装器清理命令失败' }
}
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class VersionCleanupFixture {
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    public static extern IntPtr LoadLibrary(string path);
    [DllImport("kernel32.dll")]
    public static extern bool FreeLibrary(IntPtr module);
}
'@
$root = Join-Path ([IO.Path]::GetTempPath()) ('msime-version-cleanup-' + [Guid]::NewGuid())
$loaded = [IntPtr]::Zero
try {
    $busy = Join-Path $root 'msime_v1.0.0'
    $idle = Join-Path $root 'msime_v1.0.1'
    $current = Join-Path $root 'msime_v1.0.2'
    $unrelated = Join-Path $root 'other'
    foreach ($directory in @($busy, $idle, $current, $unrelated)) {
        New-Item -ItemType Directory -Path $directory -Force | Out-Null
        [IO.File]::WriteAllText((Join-Path $directory 'msime_host_api.dll'), 'synthetic dependency')
    }
    # 使用系统 DLL 的独立副本模拟被应用映射的旧 TIP，不注册输入法或修改系统文件。
    $tip = Join-Path $busy 'MetasequoiaImeTsf.dll'
    Copy-Item (Join-Path ([Environment]::SystemDirectory) 'version.dll') $tip
    $loaded = [VersionCleanupFixture]::LoadLibrary($tip)
    if ($loaded -eq [IntPtr]::Zero) { throw '无法映射合成旧 TIP' }
    Invoke-InstallerCleanup -Root $root -Keep 'MSIME_V1.0.2'
    if (-not (Test-Path -LiteralPath $tip) -or
        -not (Test-Path -LiteralPath (Join-Path $busy 'msime_host_api.dll'))) {
        throw '旧 TIP 被映射时必须保留整组依赖'
    }
    if (Test-Path -LiteralPath $idle) { throw '未占用的旧版本应被清理' }
    if (-not (Test-Path -LiteralPath $current) -or -not (Test-Path -LiteralPath $unrelated)) {
        throw '当前版本及其他目录不能被清理'
    }
    if (-not [VersionCleanupFixture]::FreeLibrary($loaded)) { throw '无法解除合成映射' }
    $loaded = [IntPtr]::Zero
    Invoke-InstallerCleanup -Root $root -Keep 'msime_v1.0.2'
    if (Test-Path -LiteralPath $busy) { throw '解除映射后必须允许清理旧版本' }
    Invoke-InstallerCleanup -Root $root
    if (Test-Path -LiteralPath $current) { throw '卸载时应清理未占用版本' }
    Remove-Item -LiteralPath $unrelated -Recurse -Force
    Invoke-InstallerCleanup -Root $root
    if (Test-Path -LiteralPath $root) { throw '卸载后的空根目录应被移除' }
    Write-Output '旧版本目录清理行为回归通过'
} finally {
    if ($loaded -ne [IntPtr]::Zero) { [void][VersionCleanupFixture]::FreeLibrary($loaded) }
    if (Test-Path -LiteralPath $root) { Remove-Item -LiteralPath $root -Recurse -Force }
}
