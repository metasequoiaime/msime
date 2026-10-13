# Agent Note: 后端会话文件共用有界读取

Status: implemented

## Problem

桌面账号会话和 App Group 本地账号存储各自实现有界文件读取。两者都按 64 KiB 分块，并在打开时拒绝最终路径的符号链接。相同边界的两套代码会让后续安全修复只落在其中一处。

## Decision

新增 `BackendBoundedFileReader` 统一打开、分块、上限和关闭文件。两处存储保留各自的路径、权限、锁和 JSON 校验，并把 reader 错误映射到原有账号错误。桌面存储的 `readBounded` 方法保留为转发入口，供现有调用和测试使用。

## Alternatives considered

- 复用 iOS App 的 `BoundedFileReader`：它不属于跨平台 backend 包，桌面账号存储不能依赖 iOS App target。
- 把两种存储合成一类：桌面文件需要所有者和权限检查，App Group 存储需要跨进程锁；合并会扩大边界并改变存储语义。

## Consequences

两种会话文件共享同一处有界读取实现。reader 的合成测试覆盖恰好达到上限、超限和符号链接；现有存储测试继续覆盖各自的文件规则。
