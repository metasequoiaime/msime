# Agent Note: iOS 快照队列状态复用有界读取器

Status: implemented

## Problem

`DictionarySnapshotQueue` 读取小型 `state.json` 时自行打开文件、检查普通文件类型并读取最多 65537 字节；相同的特殊文件防护与上限读取已由 `BoundedFileReader` 提供。两处安全选项独立维护容易漂移。

## Decision

队列状态读取调用 `BoundedFileReader.read(from:maximumBytes:)`，上限仍为 65536 字节。超过上限继续报 `.invalid`，打开或读取失败继续报 `.unavailable`；状态格式和请求字段校验仍由队列负责。大快照源文件的 `openSnapshotSource` 保留独立句柄和流式复制，避免将最多 512 MiB 的文件读入内存。

## Alternatives considered

- 保留状态文件的本地读取代码：错误映射在一处，但安全打开和大小限制继续与共享 reader 重复。
- 把快照源文件也改用 `BoundedFileReader`：可减少更多代码，但会一次分配整份大快照，失去现有流式哈希与复制行为。

## Consequences

小型状态文件与其他 iOS 存储共用安全读取实现，队列保留自己的状态校验和错误语义。特殊文件契约检查验证队列对共享 reader 的调用，并继续单独检查大快照源文件的非阻塞打开。
