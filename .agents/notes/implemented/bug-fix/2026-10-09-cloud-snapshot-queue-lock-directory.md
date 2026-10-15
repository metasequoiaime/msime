# Agent Note: 云端词库快照队列锁绑定目录句柄

Status: implemented

## Problem

快照队列先按路径解析队列根目录，再按路径打开状态锁；状态读取和部分清理也再次解析路径。锁持有期间队列目录被替换时，锁、状态文件和清理操作可能落在不同目录，跨进程互斥和状态一致性会失效。工作租约虽保留了目录句柄，但原校验只比较规范路径；原路径出现新队列时，旧租约仍可认领、激活或失败化新请求。激活回调中替换目录还会使终态快照清理落到替换目录。

## Decision

状态锁通过已打开的私有目录句柄创建，状态 JSON 的读取、写入及 `take_state` 的终态清理都使用同一目录句柄。工作租约保留目录句柄；认领、完成与失败化请求都在实际状态锁内核对规范路径，Unix 还比较租约目录与状态锁目录的设备号和 inode。`claim`、`complete` 与 `fail` 的终态清理使用状态更新时保留的目录句柄，在状态成功落盘后删除该目录内的快照。增加 Unix 回归测试，在状态更新持锁期间替换目录，确认状态仍写入原目录对象。

目录身份依据 [Rust `MetadataExt`](https://doc.rust-lang.org/std/os/unix/fs/trait.MetadataExt.html) 的 `dev` 与 `ino`，复用同模块快照入列的比较方式。磁盘状态格式与公开接口不变；非 Unix 继续使用既有路径归属与目录存储实现。

## Alternatives considered

- 只在状态锁之前比较目录身份：调用方改动少，也能拒绝已经完成的目录替换；但校验与状态锁取得之间仍可替换目录，必须对实际持锁的目录做比较。

- 只在锁前后检查队列路径：检查和状态读写之间仍存在目录替换竞态。
- 只绑定状态文件：锁文件可能来自另一个目录对象，跨进程互斥仍不可靠。
- 每次操作重新 canonicalize 队列根目录：无法保持锁、读取和写入属于同一个目录。

## Consequences

快照队列状态更新使用同一个已打开目录对象完成锁定、读取和写入；队列路径被替换时不会把状态写入新目录或外部目标。目录元数据访问通过共享 `PrivateDirectory` 接口复用跨平台实现。

## Verification

2026-10-15 的四项目录替换回归在旧实现全部失败；修复后模块 19 项通过。回归覆盖旧租约的认领、完成与失败化拒绝，状态与快照字节保持，新租约仍能完成工作，以及激活回调内替换目录后的原目录清理。另保留跨队列路径归属拒绝。macOS 完整 client-core 为 1442 项单测与 6 项集成测试通过，2 项忽略；固定 Linux 容器为 1441 项通过、2 项忽略；host-api 为 631 项单测与 15 项集成测试通过、2 项忽略。严格 Clippy、格式与笔记检查通过。

Android API 28/35 的完整核心复测各 1388 项通过、13 项忽略，设备脚本过滤 7 项硬链接用例。两级首轮均在未改动的模型删除用例触发 100 毫秒超时；未修改超时或跳过用例，主要冷构建结束后，同一个程序在两级设备原样完整复测通过，首次超时的原因仍未确定。两级专用模拟器的原生 APK 验收通过：API 28 运行 `smoke.sh emulator-5566 --core`，API 35 运行完整 `smoke.sh emulator-5580`，均从首次资源准备开始，未跳过键盘套件。

- `cargo test -p msime-client-core cloud::snapshot_queue --locked`
- `cargo clippy -p msime-client-core --all-targets --locked -- -D warnings`
- `cargo test -p msime-client-core --locked`
- `npm run verify-notes`
- `git diff --check`
- `bash scripts/verify-local.sh --quick`
- `bash platforms/android/check-host.sh`
