# Agent Note: iOS 云同步状态复用有界读取器

Status: implemented

## Problem

`CloudSyncState` 读取 `state.json` 时自行打开句柄并多读一个字节判断 256 KiB 上限。iOS 其他小型共享 JSON 文件已经使用 `BoundedFileReader`，这处实现重复维护特殊文件防护和有界分配。

## Decision

云同步状态读取调用 `BoundedFileReader.read(from:maximumBytes:)`。超过上限仍按损坏状态返回全新的 `CloudSyncRecord`；其他底层读取失败仍映射为 `.unavailable`；JSON 解码和版本校验继续由 `CloudSyncState` 负责。状态文件仍保持原样，不因读取失败被改写。

## Alternatives considered

- 保留本地句柄读取：可以维持最小 diff，但会继续让状态文件和其他共享 JSON 文件拥有两套安全读取实现。
- 将损坏状态直接删除：能让下一次写入恢复，但会改变现有的无损自救语义，可能丢失待同步进度。

## Consequences

云同步状态与共享 reader 的非阻塞、普通文件和大小上限策略保持一致；超限回归测试确认状态被视为全新且原文件不变。读取器的安全调用关系由 iOS 特殊文件契约检查覆盖。
