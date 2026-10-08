# Agent Note: 诊断压缩包发布绑定目录句柄

Status: implemented

## Problem

诊断 zip 导出在检查目标路径后仍通过 `NamedTempFile::new_in` 和 `persist` 按路径创建、发布。目标父目录被替换成符号链接时，压缩包可能被写到外部目录。

## Decision

Unix 导出先打开目标父目录，再在该句柄中创建 0600 临时文件。zip writer 继续流式写入临时文件，完成并同步后用同一目录句柄 `renameat` 发布，保留原子替换和 bounded 记录逻辑。非 Unix 保留原有临时文件流程。

共享 storage 增加流式 writer helper，不把压缩包整体读入内存；诊断测试覆盖符号链接父目录被拒绝且外部目录不产生文件。

## Alternatives considered

- 只在发布前重新检查父目录：临时文件创建和最终发布之间仍有竞态。
- 把 zip 先写入内存再调用私有字节写入：诊断内容可能接近上限，额外内存峰值没有必要。
- 改用路径型 `persist` 并重复检查：无法绑定到检查时的目录对象。

## Consequences

Unix 诊断导出过程中的临时文件和最终 zip 都相对于已打开的父目录执行，目录项替换不会把导出重定向到外部位置；压缩过程仍是流式的。非 Unix 行为保持不变。

## Verification

- 先运行新增 `diagnostic_zip_rejects_a_symlinked_destination_parent`，确认原实现错误地成功并使测试失败。
- 实现后 `cargo test -p msime-client-core --lib diagnostics::tests --locked --quiet`：10 passed。
- `cargo clippy -p msime-client-core --lib --locked -- -D warnings`、`cargo fmt --all` 和 `git diff --check` 通过。
