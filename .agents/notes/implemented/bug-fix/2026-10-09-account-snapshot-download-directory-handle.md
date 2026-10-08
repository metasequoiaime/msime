# Agent Note: 账户快照下载发布绑定目录句柄

Status: implemented

## Problem

账户词库快照下载虽然限制了响应大小并先拒绝符号链接父目录，但 Unix 上仍用路径型临时文件和 `persist` 写出最多 512 MiB 的流式内容。父目录在检查后被替换时，下载内容可能被写入外部目录。

## Decision

Unix 下载先打开目标父目录，在目录句柄相对的 0600 临时文件中直接接收 HTTP 响应；流式大小检查、flush 和同步完成后，用共享目录句柄 writer helper 原子 rename 发布。仍然只保留流式缓冲，不把快照整体载入内存。非 Unix 保留原有临时文件实现。

## Alternatives considered

- 只重复 `reject_symlink`：检查与临时文件创建之间仍有竞态。
- 使用 `dictionary_snapshot` 先读入内存再写入：512 MiB 上限下会产生不必要的高峰内存占用。
- 通过新的路径型临时文件 API 修补：仍不能绑定检查时的父目录对象。

## Consequences

Unix 下载目标始终相对于已打开父目录发布，目录替换不会重定向快照；响应上限、空文件拒绝、原子替换和非 Unix 行为保持不变。

## Verification

`cargo test -p msime-client-core --lib account::tests --locked --quiet`：51 passed；其中包含正常快照下载和符号链接父目录拒绝用例。`cargo clippy -p msime-client-core --lib --locked -- -D warnings`、`cargo fmt --all` 和 `git diff --check` 通过。
