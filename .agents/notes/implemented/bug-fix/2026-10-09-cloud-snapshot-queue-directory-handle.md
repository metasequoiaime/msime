# Agent Note: 云端快照队列发布绑定目录句柄

Status: implemented

## Problem

云端词库快照队列把最多 512 MiB 的源文件复制到 `NamedTempFile::new_in`，再在状态锁内用路径型 `persist` 发布。队列目录经过检查后若被替换，复制和发布可能跟随新路径；同时把大文件复制放在状态更新流程里会延长锁持有时间。

## Decision

Unix 新增目录句柄绑定的 `PrivateStagedFile`：在已打开的队目录中创建 0600 临时文件，锁外流式复制并校验大小和 SHA-256；进入状态锁后确认句柄仍指向当前队目录，再用相同句柄 `renameat` 发布。状态更新失败时暂存文件和已发布快照都由句柄或受保护路径清理。非 Unix 保留原有临时文件流程。

## Alternatives considered

- 只在 `persist` 前重复路径检查：复制和发布之间仍有目录替换竞态。
- 把 512 MiB 复制放在状态锁内：虽然能缩短暂存生命周期，却会阻塞取消、领取和状态读取。
- 先把快照整体读入内存：会制造不必要的高峰内存占用。

## Consequences

Unix 大文件复制不再占用状态锁，最终发布绑定到创建暂存文件时确认的队目录；目录替换不会把快照写到外部位置。哈希校验、版本冲突、原子发布和非 Unix 行为保持不变。

## Verification

`cargo test -p msime-client-core --lib cloud::snapshot_queue::tests --locked --quiet`：12 passed。

新增 storage 回归测试验证目录替换后暂存文件仍发布到原目录；完整 `cargo test -p msime-client-core --lib --locked --quiet`：1086 passed，1 ignored。`cargo clippy -p msime-client-core --lib --locked -- -D warnings`、`cargo fmt --all` 和 `git diff --check` 通过。
