# Agent Note: 本地语音模型残留清理绑定目录句柄

Status: implemented

## Problem

本地语音模型安装、删除和收编会清理崩溃遗留的 `.staging-*`、`.old-*` 目录。Unix 路径型 `read_dir`、`remove_dir_all` 和收编恢复改名在根目录被替换后可能跟随符号链接，访问或删除根目录外的文件。

## Decision

Unix 残留枚举改用已打开的根目录句柄，staging/model 和来源目录都通过 `openat` 且拒绝符号链接；恢复文件使用目录句柄相对的 no-clobber 改名，残留树删除复用 `remove_private_tree_at`。`Staging` 保存父目录句柄，析构清理和发布后的旧目录删除也绑定原目录。非 Unix 保留原有路径实现。

## Alternatives considered

- 清理前再次检查路径：检查和使用之间仍可发生目录替换。
- 只拒绝 staging 符号链接：根目录或来源目录替换仍可能重定向收编和删除。
- 让恢复覆盖来源同名文件：会破坏来源目录中新写入的文件，违反原有不覆盖语义。

## Consequences

根目录或 staging 被替换时，恢复和清理仍只作用于原先打开的目录；来源已有同名文件继续保留。非 Unix 行为和安装原子发布语义不变。

## Verification

`cargo test -p msime-client-core --lib voice::local_models::tests::`：55 passed，1 ignored。新增测试覆盖根目录替换后的残留恢复和 staging 析构清理。`cargo fmt --all`、`cargo clippy -p msime-client-core --lib --locked -- -D warnings` 和 `git diff --check` 通过。
