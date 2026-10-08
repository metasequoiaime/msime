# Agent Note: 本地语音模型发布绑定根目录句柄

Status: implemented

## Problem

本地语音模型安装替换旧模型时先检查目标，再通过路径把旧目录移到旁路并把暂存目录改名为目标。根目录或暂存目录在检查后被替换时，路径解析可能重定向发布结果。

## Decision

Unix 上模型发布使用已打开的根目录句柄和暂存目录句柄执行两次 `renameat`：旧目标先移到旁路名，再把暂存目录条目发布为模型 id；失败回滚也使用同一根目录句柄。非 Unix 保留原有路径实现。

## Alternatives considered

- 继续使用 `fs::rename`：路径竞态仍可把旧目录移动或新模型发布到外部位置。
- 只再次检查根目录：检查与两个重命名之间仍有窗口。
- 让暂存目录也位于根目录后继续使用路径：根目录条目仍会被重新解析，不能绑定操作对象。

## Consequences

模型安装的替换、发布和失败回滚都绑定到当前根目录及实际暂存目录，旧文件句柄继续保持原有 Unix 语义。下载、校验、恢复和非 Unix 行为不变。

## Verification

`cargo test -p msime-client-core --lib voice::local_models --locked --quiet`：53 passed, 1 ignored。`cargo fmt --all`、`cargo clippy -p msime-client-core --lib --locked -- -D warnings` 和 `git diff --check` 均通过。
