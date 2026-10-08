# Agent Note: 诊断崩溃目录读取绑定目录句柄

Status: implemented

## Problem

目录型崩溃日志先用路径 `read_dir` 枚举，再按拼接路径打开记录。目录被替换成符号链接时，扫描可能进入外部目录；后续路径操作还会受目录替换影响。

## Decision

Unix 一次打开崩溃目录，用 `rustix::fs::Dir` 枚举文件名，并通过 `open_private_file_at` 保留已绑定目录的普通文件句柄，再读取和排序记录。非 Unix 保留原有路径实现。

## Alternatives considered

- 只在扫描前检查目录不是符号链接：检查和枚举之间仍有竞态。
- 继续拼接 `entry.path()` 后调用私有文件打开：不能绑定目录枚举结果。

## Consequences

目录替换或链接时诊断源读取失败并返回稳定的源错误；正常目录仍保留扩展名筛选、最近记录限制和清洗规则。

## Verification

- `cargo test -p msime-client-core diagnostics::tests:: --lib`
- `cargo fmt --all`
- `cargo clippy -p msime-client-core --lib --locked -- -D warnings`
- `git diff --check`
- `npm run verify-notes`
- `bash scripts/verify-local.sh --quick`
