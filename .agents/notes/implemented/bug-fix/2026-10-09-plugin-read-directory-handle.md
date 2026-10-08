# Agent Note: 插件文件读取绑定目录句柄

Status: implemented

## Problem

插件包校验在检查文件类型后，音频签名和通知内容读取仍通过路径打开。包目录或其父目录被替换时，检查结果与实际读取对象可能不一致。

## Decision

音频签名读取和有界文件读取统一使用 `open_private_file_in`，让 Unix 读取相对于已确认的父目录句柄，并保留现有大小与类型校验。

## Alternatives considered

- 只重复 `symlink_metadata`：无法消除检查和打开之间的竞态。
- 在每个调用点自行使用系统调用：会重复 client-core 的共享存储适配。

## Consequences

插件包校验读取不会因目录替换而跟随新的路径目标，错误处理和包格式验证保持不变。

## Verification

`cargo test -p msime-client-core --lib plugins::tests --locked --quiet`：49 passed。`cargo fmt --all`、`cargo clippy -p msime-client-core --lib --locked -- -D warnings` 和 `git diff --check` 均通过。
