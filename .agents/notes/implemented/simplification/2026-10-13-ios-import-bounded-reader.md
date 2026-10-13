# Agent Note: iOS 导入复用有界文件读取器

Status: implemented

## Problem

iOS 的词汇复习导入和个人词典导入各自实现了 `open`、`fstat`、`O_NOFOLLOW`、`O_NONBLOCK` 以及按上限分块读取。三处实现必须同时维护特殊文件防护和大小上限，容易出现安全选项或错误映射漂移。

## Decision

两个导入入口都调用 `BoundedFileReader.read(from:maximumBytes:)`。个人词典导入继续在 `NSFileCoordinator` 和 security-scoped resource 范围内读取，并把 reader 的超限或读取错误转换为原有中文错误；词汇复习导入继续把所有读取失败转换为 `unreadableWordbook`。共享 reader 仍负责打开句柄、拒绝符号链接和特殊文件，并限制分配量。

## Alternatives considered

- 保留两份本地实现：每个入口的错误处理更直观，但安全打开选项和读取循环会继续重复，后续修改必须同步多处。
- 把 `NSFileCoordinator` 也移入共享 reader：可以进一步减少代码，但会把文件提供商协调和 security-scoped 生命周期错误地耦合到普通本地文件读取器。

## Consequences

导入路径共享同一套特殊文件防护和大小上限实现，减少重复代码；入口仍保留各自的协调、权限和领域错误语义。源码契约检查改为验证共享 reader 的安全选项以及两个入口对它的调用。
