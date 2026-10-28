# 设置冒烟的只读快照不得干扰宿主原子替换：读取要取与 Rust 存储同一把锁，并在返回前关闭句柄；不依赖已编译的输入法或 WinUI。
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
using System.IO;
using System.Runtime.InteropServices;
using Microsoft.Win32.SafeHandles;
public static class SettingsSnapshotReplacementProbe {
    [DllImport("kernel32.dll", CharSet = CharSet.Unicode, SetLastError = true)]
    private static extern bool MoveFileEx(string source, string destination, uint flags);
    public static int Replace(string source, string destination) {
        return MoveFileEx(source, destination, 1) ? 0 : Marshal.GetLastWin32Error();
    }
    [StructLayout(LayoutKind.Sequential)]
    private struct Overlapped { public IntPtr Internal, InternalHigh; public uint Offset, OffsetHigh; public IntPtr Event; }
    [DllImport("kernel32.dll", SetLastError = true)]
    private static extern bool LockFileEx(SafeFileHandle file, uint flags, uint reserved, uint low, uint high, ref Overlapped overlapped);
    // 与 Rust 标准库 `File::lock` 在 Windows 上的调用相同：`LOCKFILE_EXCLUSIVE_LOCK`，从偏移 0 起锁住整段区间。
    public static int LockLikeRust(FileStream file) {
        var overlapped = new Overlapped();
        return LockFileEx(file.SafeFileHandle, 2, 0, uint.MaxValue, uint.MaxValue, ref overlapped) ? 0 : Marshal.GetLastWin32Error();
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
        # 轮询读取即使返回前就关闭句柄，打开到关闭之间仍是一个读取窗口，宿主的替换落进去就会失败（#9915、#10115 两次候选数量「未保存」都是这样）。模拟写入方像 Rust 存储那样持锁、在锁里替换：读取必须等到锁释放后读到新内容，在锁外读会读到旧内容。
        [IO.File]::WriteAllText($path, 'synthetic-before')
        [IO.File]::WriteAllText($replacement, 'synthetic-locked-after')
        $held = New-Object Threading.ManualResetEvent($false)
        $writer = [PowerShell]::Create()
        $null = $writer.AddScript({
            param($lockPath, $source, $destination, $held)
            $sharing = [IO.FileShare]::ReadWrite -bor [IO.FileShare]::Delete
            $lock = [IO.File]::Open($lockPath, [IO.FileMode]::OpenOrCreate, [IO.FileAccess]::ReadWrite, $sharing)
            try {
                $lockError = [SettingsSnapshotReplacementProbe]::LockLikeRust($lock)
                if ($lockError -ne 0) { return "lock:$lockError" }
                $null = $held.Set()
                Start-Sleep -Milliseconds 500
                return "replace:$([SettingsSnapshotReplacementProbe]::Replace($source, $destination))"
            } finally { $lock.Dispose() }
        }).AddArgument((Join-Path $root 'preferences.lock')).AddArgument($replacement).AddArgument($path).AddArgument($held)
        $pending = $writer.BeginInvoke()
        try {
            if (-not $held.WaitOne(10000)) { throw "模拟写入方没有取得存储锁：$(@($writer.EndInvoke($pending)) -join ',')" }
            $contents = Read-SettingsSnapshotText $path
            $writerResult = @($writer.EndInvoke($pending)) -join ','
        } finally {
            $writer.Dispose()
            $held.Dispose()
        }
        if ($writerResult -ne 'replace:0') { throw "模拟写入方在锁内替换失败：$writerResult" }
        if ($contents -ne 'synthetic-locked-after') { throw "快照读取没有等待存储锁，读到了替换前的内容：$contents" }
        Write-Host '存储锁对照：写入方持锁替换成功，读取等到锁释放后读到新内容'
    }
} finally { Remove-Item -LiteralPath $root -Recurse -Force }
