# Agent Note: 插件残留扫描绑定目录句柄

Status: implemented

## Problem

插件残留清理在 Unix 上虽然用已打开的目录句柄删除，但仍通过路径 `read_dir` 枚举。目录被替换成外部链接后，扫描结果可能来自外部目录，并据此删除原目录中同名残留。

## Decision

Unix 直接通过目录句柄的 `rustix::fs::Dir` 枚举，并用 `openat` 获取目录项修改时间，再以同一目录句柄执行递归删除；非 Unix 保留路径实现。

## Alternatives considered

- 只在删除前重新检查目录链接：枚举结果仍可能来自错误目录，且检查和删除之间有竞态。
- 继续用路径 `read_dir`、只把删除改成 `unlinkat`：无法保证扫描对象与删除对象属于同一目录。

## Consequences

Unix 残留扫描和删除绑定到同一个目录对象，目录替换期间最多跳过清理，不会依据外部目录项修改可信目录；年龄阈值和命名规则保持不变。

## Verification

- `cargo test -p msime-client-core plugins::tests:: --lib`
- `cargo fmt --all`
- `cargo clippy -p msime-client-core --lib --locked -- -D warnings`
- `git diff --check`
- `npm run verify-notes`
- `bash scripts/verify-local.sh --quick`
