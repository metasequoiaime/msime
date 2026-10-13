# Agent Note: 快照待合并文件先校验再发布

Status: implemented

## Problem

Android 设置页排队输入记录或输入习惯时，先替换固定的 `pending` 文件，再从该路径重新读回校验。键盘可能恰好把这份文件改名认领；旧请求因此读不到文件并进入失败清理。如果另一请求此时发布了新的 `pending`，旧请求会删除这份较新的队列，导致待恢复的输入记录或习惯丢失。输入习惯还在校验源路径后用无界 `std::fs::read` 再次打开它；源文件被替换时，校验和读取可能不是同一份内容。

## Decision

输入记录与输入习惯都改为在内存中的实际发布字节上完整校验格式、条数和校验和，然后才原子替换 `pending`。输入习惯从安全打开的同一文件句柄有界读取。导出输入习惯也把发布后校验移到发布前，避免失败清理按固定路径删除后来者。

## Alternatives considered

- 校验失败后只比较文件时间或大小再删除：另一个队列仍可能具有相同属性，检查与删除之间还有竞态。
- 给设置页加进程内锁：键盘认领发生在另一进程，无法共用该锁。

## Consequences

格式无效或源文件读取失败时，已有待合并文件保持原样。校验使用同一套解析器，文件发布仍通过现有私有文件原子替换逻辑，不增加依赖。

## Verification

新增两项回归测试以有效的较新队列为初始状态，断言无效输入不会覆盖或删除它；测试先因缺少校验前发布入口失败，实现后通过。`cargo test -p msime-host-api --lib` 的 586 项测试通过，`cargo clippy -p msime-host-api --lib -- -D warnings`、`cargo fmt --check`、`git diff --check` 和 `bash scripts/verify-local.sh --quick` 通过。quick 门禁包括 Linux 原生宿主 83 项测试、Rust workspace、Android、wasm、macOS 和 Apple bridge 编译。
