# Agent Note: client-core 共用私有新文件入口

Status: implemented

## Problem

社区插件归档、资源暂存和本地语音模型安装各自复制创建新文件时的符号链接防护与平台打开标志。安全选项调整时，三处可能产生不同的私有文件边界。

## Decision

在 `crates/client-base/src/file_lock.rs` 增加共用的 `create_private_file`，集中 `create_new`、Unix 的 `O_NOFOLLOW | O_CLOEXEC` 和 Windows 的 reparse point 防护。社区归档继续自行写入并 `sync_all`；资源在 Unix 上继续使用目录句柄 `openat`，只有非 Unix/测试路径调用共用入口；语音模型的所有路径创建调用共用入口。

## Alternatives considered

- 让资源的 Unix `openat` 也走路径入口：会丢失已打开父目录绑定，扩大目录替换风险。
- 把写入和同步也封装进去：社区归档、模型文件和资源暂存的缓冲及同步时机不同，共用打开边界即可。

## Consequences

路径创建的跨平台安全标志只有一个维护点，目录句柄路径和调用方的写入语义保持不变。`file_lock` 测试及 client-core 编译覆盖入口仍可用。
