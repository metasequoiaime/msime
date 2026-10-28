# 共享层在存储锁里以原子替换发布快照（Windows 上是 `NamedTempFile.persist` 的 `MoveFileExW`）。只要目标文件还有打开的句柄，Windows 的这种替换就返回访问拒绝 5，允许 `FileShare.Delete` 也不行，宿主随即把这次保存当成失败、重读旧值。所以读取和 Rust 侧的读者一样先取同一把存储锁，持锁期间打开、读完并关闭文件，写入方的替换不会落进读取窗口；交给管道下游之前两个句柄都已关闭。
function Get-SettingsSnapshotLockPath([string]$Path) {
    # 与 Rust 存储的锁文件一一对应：`PreferencesStore` 是同目录的 `preferences.lock`，`ClipboardHistoryStore` 是 `<文件名>.lock`。不认识的文件直接报错，不猜它的锁。
    $name = [IO.Path]::GetFileName($Path)
    switch ($name) {
        'preferences.json' { return (Join-Path ([IO.Path]::GetDirectoryName($Path)) 'preferences.lock') }
        'clipboard_history.json' { return "$Path.lock" }
        default { throw "未知的设置快照，找不到对应的存储锁：$name" }
    }
}
function Read-SettingsSnapshotText([string]$Path) {
    $sharing = [IO.FileShare]::ReadWrite -bor [IO.FileShare]::Delete
    $lock = $null
    # Unix 上的发布是 rename(2)，不受打开的读取句柄影响，也不与 `FileStream.Lock` 的 fcntl 锁互斥，只在 Windows 上取锁。
    if ([Environment]::OSVersion.Platform -eq [PlatformID]::Win32NT) {
        $lock = [IO.File]::Open((Get-SettingsSnapshotLockPath $Path), [IO.FileMode]::OpenOrCreate, [IO.FileAccess]::ReadWrite, $sharing)
    }
    try {
        if ($lock) {
            # `FileStream.Lock` 是不等待的 `LockFile` 字节区间锁，与 Rust `File::lock` 用 `LockFileEx` 独占的整段区间互斥；写入方持锁时稍后再试，超时就报错，不在锁外读。
            $deadline = [DateTime]::UtcNow.AddSeconds(10)
            while ($true) {
                try {
                    $lock.Lock(0, 1)
                    break
                } catch [IO.IOException] {
                    if ([DateTime]::UtcNow -ge $deadline) { throw "等待设置存储锁超时：$Path，$($_.Exception.Message)" }
                    Start-Sleep -Milliseconds 10
                }
            }
        }
        $stream = [IO.File]::Open($Path, [IO.FileMode]::Open, [IO.FileAccess]::Read, $sharing)
        $reader = $null
        try {
            $reader = [IO.StreamReader]::new($stream, [Text.Encoding]::UTF8, $true)
            $contents = $reader.ReadToEnd()
        } finally {
            if ($reader) { $reader.Dispose() } else { $stream.Dispose() }
        }
    } finally {
        # 关闭锁文件句柄即释放区间锁，与 Rust 侧 `ScopedFileLock` 的释放方式相同。
        if ($lock) { $lock.Dispose() }
    }
    return $contents
}
