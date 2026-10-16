# Agent Note: 插件表的文件戳带上 inode

Status: implemented

## Problem

`crates/host-api/src/plugin_tables.rs` 用 `FileStamp`（修改时间、长度）判断指令表、短语表、辅助码表和 `mentions.json` 有没有变，输入框获得焦点时只在戳变了才重新读。重新导入一个同 id 的包时，`plugins::import` 经 `replace_directory` 把整个包目录换掉，新清单是暂存目录里新写的文件。新旧清单长度相同、修改时间又落在同一个计时粒度里（HFS+ 的 1 秒、FAT/exFAT 的 2 秒，以及部分网络文件系统）时，戳完全相同，会话一直用旧表，直到偏好变化或重开会话。音效包的 `key_sound::PackStamp` 早已因为同一原因带上了 inode，插件表没有跟上。

## Decision

`FileStamp` 是（修改时间、长度、inode），Unix 上取 `MetadataExt::ino`，其他平台记 0，与 `PackStamp` 的取法一致。换目录导入时新清单在旧清单还没删掉前就已建好，两者 inode 必然不同，所以同长度、同时间的重新导入也会被看到。辅助码表包的目录戳用的是同一个 `FileStamp`，一并受益。

## Alternatives considered

- **比较清单内容的哈希**：不依赖文件系统的时间粒度，Windows 上也有效。但每次获得焦点都要读并哈希每个启用的清单，而这条路径的设计前提是「没变动时只做几次 stat」；指令表和短语表可以有几千行。

## Consequences

Unix 上重新导入的表在下一次获得焦点时一定会被换上。代价是 Windows 仍只看修改时间和长度（NTFS 的时间戳是 100 纳秒精度，实际撞上的概率很低），以及原地改写同一个文件不会改变 inode，这种情况只靠修改时间和长度。

## Verification

`crates/host-api/src/tests.rs` 的 `a_reimported_table_of_the_same_length_and_time_is_picked_up`（仅 Unix）用 `msime_client_services::plugins::import` 先后导入两张同长度的 `work` 指令表，把新清单的修改时间设回旧值，获得焦点后断言候选换成新模板。若戳不含 inode，该测试拿到的仍是旧模板。
