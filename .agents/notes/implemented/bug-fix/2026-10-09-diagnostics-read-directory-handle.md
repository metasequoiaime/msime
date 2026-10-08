# Agent Note: 诊断日志读取绑定目录句柄

Status: implemented

## Problem

诊断导出在检查崩溃日志类型和长度后仍按路径打开文件。日志目录或父目录被替换时，检查对象与读取对象可能不一致。

## Decision

崩溃记录和诊断来源文件读取统一改用 `open_private_file_in`，保留现有边界读取、截断和清洗逻辑。

## Alternatives considered

- 只重复路径元数据检查：仍无法消除检查到打开之间的竞态。
- 改变诊断导出格式或日志清洗：与问题无关，扩大了变更范围。

## Consequences

诊断读取相对于已确认的父目录打开普通文件，异常和大小限制行为保持不变。

## Verification

`cargo test -p msime-client-core --lib diagnostics --locked --quiet`：9 passed。`cargo fmt --all`、`cargo clippy -p msime-client-core --lib --locked -- -D warnings` 和 `git diff --check` 均通过。
