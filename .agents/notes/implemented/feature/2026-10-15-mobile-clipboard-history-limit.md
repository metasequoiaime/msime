# Agent Note: 移动端剪贴板历史扩展到 200 条

Status: implemented

## Problem

#7787 反馈 Android 剪贴板历史只能保存 50 条，希望超过 100 条并接近 200 条。移动端 Android、iOS 和 HarmonyOS 共享 `client-core` 的本机历史文件，但宿主各自复制了容量和满载文案；只改 Android 会让同一个文件在另外两端被截断或拒绝。

## Decision

移动端本机剪贴板历史统一保留最多 200 条。`crates/client-core/src/clipboard.rs` 的 `MAX_ENTRIES` 是存储和淘汰规则的唯一所有者；Android `ClipboardHistoryPolicy.LIMIT`、iOS `ClipboardHistoryStore.limit` 和 HarmonyOS `ClipboardHistoryPolicy.LIMIT` 只镜像这个契约，用于容量、计数和用户提示。云剪贴板仍受服务端 50 条上限约束，不随本机历史扩展。

共享文件的有界读取按 200 条最长移动端文本的 JSON 转义最坏情况计算。HarmonyOS 的本地解析器上限提高到 64 MiB，覆盖共享 Rust 存储的边界；其余宿主直接通过 C ABI 读取结构化结果。已有 50 条文件可以原样读取，新增记录按新的 200 条淘汰和固定规则工作。

## Alternatives considered

- **只把 Android 上限改成 200 条** — 改动最小，也能直接回应 issue；但 Android、iOS 和 HarmonyOS 读同一文件，另外两端会把合法的新记录当成越界文件或只显示前 50 条，不能接受。
- **把上限设为 100 条** — 能满足“大于 100 条”字面要求且占用更少；issue 明确给出约 200 条的目标，200 条仍保持有界并避免用户很快再次遇到同一限制，因此选择 200。
- **让 C ABI 每次返回动态上限** — 可以减少宿主常量镜像；但会扩大现有移动剪贴板协议，且宿主仍需本地文案和 UI 计数，当前固定契约没有必要增加一层协议复杂度。

## Consequences

本固定 200 条的决定已由[剪贴板历史容量可选档位](2026-10-17-clipboard-history-capacity.md)取代；共享存储仍沿用这里记录的固定优先、旧文件兼容和边界校验。

- **收益**：三种移动端宿主都能保存并显示最多 200 条本机历史，固定项、去重、淘汰和旧文件迁移规则保持一致；云端容量和本机容量的提示不再混淆。
- **代价**：极端情况下历史文件和一次 `load` 返回的结构化 JSON 会比原来大；Rust 仍按计算出的文件边界拒绝无界输入，HarmonyOS 解析器额外保留 64 MiB 上限。桌面 Windows 历史是独立存储，继续保持 50 条。

## Verification

共享 `client-core` 剪贴板单测覆盖 200 条去重、固定和淘汰；Android host 检查与 JVM 容量回归、HarmonyOS 键盘逻辑测试、iOS 剪贴板 XCTest 源文件语法解析通过，iOS 用例扩展到 210 条并断言保留 200 条。提交前还运行笔记门禁、差异检查、共享 Rust 格式与 clippy，以及 Android API 35 模拟器上的剪贴板保存生命周期回归。完整 iOS XcodeGen 构建未运行：该 worktree 没有 `target/ios/EngineResources`，生成工程在校验资源目录时停止。
