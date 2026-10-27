# 设置冒烟的只读快照不得干扰宿主原子替换；不依赖已编译的输入法或 WinUI。
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
. (Join-Path $PSScriptRoot 'settings_snapshot.ps1')

$root = Join-Path ([IO.Path]::GetTempPath()) ('msime-settings-snapshot-' + [guid]::NewGuid().ToString('N'))
$null = New-Item -ItemType Directory -Path $root
try {
    $path = Join-Path $root 'preferences.json'
    $expected = '{"preferences":{"candidate_page_size":9},"sample":"合成快照"}'
    foreach ($bom in @($false, $true)) {
        [IO.File]::WriteAllText($path, $expected, (New-Object Text.UTF8Encoding($bom)))
        # 管道下游开始运行前必须关闭文件，不能让 JSON 解析延长句柄寿命。
        Read-SettingsSnapshotText $path | ForEach-Object {
            if ($_ -ne $expected) { throw '快照读取没有保留 UTF-8 文本' }
            $exclusive = [IO.File]::Open($path, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::None)
            $exclusive.Dispose()
            if (($_ | ConvertFrom-Json).preferences.candidate_page_size -ne 9) { throw '快照 JSON 数值不符' }
        }
    }
    Write-Host '快照 UTF-8 与管道下游执行前关闭文件：通过'

    if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT) {
        Write-Host 'skipped: Windows 文件共享与原子替换对照需要 Windows'
    } else {
        Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class SettingsSnapshotReplacementProbe {
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern bool MoveFileEx(string source, string destination, uint flags);
    public static int Replace(string source, string destination) {
        return MoveFileEx(source, destination, 1) ? 0 : Marshal.GetLastWin32Error();
    }
}
'@
        $replacement = Join-Path $root 'replacement.json'
        [IO.File]::WriteAllText($path, 'synthetic-before')
        [IO.File]::WriteAllText($replacement, 'synthetic-after')
        $reader = [IO.File]::Open($path, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::ReadWrite)
        try {
            # 旧读取方式把句柄留给管道下游，确定性复现写入受到读取探针干扰。
            $errorCode = [SettingsSnapshotReplacementProbe]::Replace($replacement, $path)
            if ($errorCode -notin @(5, 32)) { throw "读取句柄未关闭的对照应拒绝替换（5 或 32），实际为 $errorCode" }
            if ((Read-SettingsSnapshotText $path) -ne 'synthetic-before') { throw '失败替换改变了原文件' }
            Write-Host "Windows 原子替换对照：读取句柄未关闭，错误码=$errorCode"
        } finally { $reader.Dispose() }
        # 有 Delete 共享标志也不能保证 MoveFileEx 覆盖打开的目标；验收真实 helper 在返回前关闭句柄。
        Read-SettingsSnapshotText $path | ForEach-Object {
            if ($_ -ne 'synthetic-before') { throw '替换前的快照内容不符' }
            $errorCode = [SettingsSnapshotReplacementProbe]::Replace($replacement, $path)
            if ($errorCode -ne 0) { throw "快照读取已返回但仍阻止原子替换：$errorCode" }
            if ((Read-SettingsSnapshotText $path) -ne 'synthetic-after') { throw '原子替换没有发布新内容' }
            Write-Host "Windows 原子替换对照：真实快照 helper 已返回，错误码=$errorCode，新内容验证通过"
        }
    }
} finally { Remove-Item -LiteralPath $root -Recurse -Force }
