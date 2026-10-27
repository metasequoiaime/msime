# 共享层通过原子替换发布快照；读取时允许替换，并在交给管道下游之前关闭句柄。
function Read-SettingsSnapshotText([string]$Path) {
    $sharing = [IO.FileShare]::ReadWrite -bor [IO.FileShare]::Delete
    $stream = [IO.File]::Open($Path, [IO.FileMode]::Open, [IO.FileAccess]::Read, $sharing)
    $reader = $null
    try {
        $reader = [IO.StreamReader]::new($stream, [Text.Encoding]::UTF8, $true)
        $contents = $reader.ReadToEnd()
    } finally {
        if ($reader) { $reader.Dispose() } else { $stream.Dispose() }
    }
    return $contents
}
