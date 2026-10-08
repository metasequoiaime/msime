# Agent Note: 资源代际发布绑定目录句柄

Status: implemented

## Problem

资源安装在 root 下创建并校验 incoming 暂存目录后，最后用路径型 `fs::rename` 发布代际目录。root 在检查后被替换时，完整资源树可能被发布到外部路径。

## Decision

Unix 安装在获取资源锁后打开 root 目录句柄，完成下载和校验后通过 `renameat` 将暂存目录发布到生成名；非 Unix 保留路径型 rename。新增回归测试在暂存后把 root 替换为符号链接，确认发布仍落在原目录。

## Alternatives considered

- 只在 rename 前重新检查 root：检查与 rename 之间仍有竞态。
- 复制暂存目录内容到目标：破坏目录级原子发布并扩大失败清理面。
- 把整个资源代际载入内存：资源文件可能很大，没有必要。

## Consequences

Unix 资源代际发布相对于已打开的 root 目录执行，目录替换不会把完整资源树重定向到外部位置；下载、摘要校验、已有代际验证和非 Unix 行为保持不变。

## Verification

新增 `generation_publish_stays_in_an_open_resource_directory` 回归测试通过；资源测试：25 passed。`cargo clippy -p msime-client-core --lib --locked -- -D warnings`、`cargo fmt --all` 和 `git diff --check` 通过。
