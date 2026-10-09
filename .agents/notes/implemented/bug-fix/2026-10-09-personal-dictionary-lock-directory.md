# Agent Note: 个人词典同步锁绑定目录句柄

Status: implemented

## Problem

个人词典更新先按路径打开 `sync.lock`，随后又按路径读取和写入 `sync.json`。锁持有期间目录被替换时，锁和状态文件可能落在不同目录，跨进程互斥失效，状态也可能写入替换后的目录。

## Decision

更新流程先打开个人词典目录句柄，再通过该句柄打开并独占锁文件；状态读取和原子写入都复用同一目录句柄。目录锁和状态读写抽成内部辅助方法，Unix 回归测试在持锁后替换目录，确认状态仍写入原目录对象。

## Alternatives considered

- 只重新检查目录路径：检查和锁、状态读写之间仍存在替换竞态。
- 只绑定锁文件：状态文件仍可能解析到被替换后的目录。
- 在每次读写前重新打开路径：不能保证锁、读取和写入属于同一个目录对象。

## Consequences

个人词典更新的互斥、读取和写入都固定在同一个已打开目录上；目录路径被替换不会把状态写入新目录或外部目标。

## Verification

- `cargo test -p msime-client-core dictionary::personal --locked`
- `cargo clippy -p msime-client-core --all-targets --locked -- -D warnings`
- `cargo test -p msime-client-core --locked`
- `npm run verify-notes`
- `git diff --check`
- `bash scripts/verify-local.sh --quick`
- `bash platforms/android/check-host.sh`
