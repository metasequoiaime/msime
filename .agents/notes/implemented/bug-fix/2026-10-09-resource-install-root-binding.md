# Agent Note: 资源安装锁与返回路径绑定根目录

Status: implemented

## Problem

资源安装先按路径打开 `resources.lock`，再另行打开资源根目录。两步之间根目录被替换时，锁可能保护另一棵目录树。下载期间根目录被替换时，安装器仍会在原目录句柄下发布资源，却返回已经指向新目录的路径，调用方随后可能找不到刚安装的文件。

## Decision
资源安装作用域结束时的释放语义见[资源安装锁释放](2026-10-15-resource-install-lock-release.md)，该决定与本篇部分重叠，保留各自的操作边界。


Unix 安装先打开资源根目录，再相对同一目录句柄打开并校验锁文件。发布前和返回路径前比较原目录句柄与当前路径所指目录的设备号和 inode；目录已替换则报存储错误。回归测试在下载回调中替换根目录，确认安装报错、原目录不发布资源，新目录不收到资源字节。

## Alternatives considered

- 仅重新检查根路径是否存在：新目录可占用同一路径，无法证明它仍是持锁目录。
- 只把锁改成相对目录打开：发布后仍可能返回指向另一目录的路径。
- 直接返回原目录的路径：目录已改名时，旧路径无法定位已发布的资源。

## Consequences

Unix 上安装锁与 staging、发布操作从同一个根目录句柄出发；安装期间根目录被替换会失败并清理未发布的 staging 文件。安装返回路径仍是路径值，返回之后的外部目录变更不在本次操作的控制范围内。

## Verification

- `cargo test -p msime-client-core resources::tests:: --locked --lib`
- `cargo clippy -p msime-client-core --all-targets --locked -- -D warnings`
- `cargo fmt --all`
- `npm run verify-notes`
- `git diff --check`
- `bash scripts/verify-local.sh --quick`
