# Agent Note: 新装默认关闭候选翻译并保留已有配置

Status: implemented

## Problem

#7599：macOS 新装直接出现候选译文，用户希望默认不打开。Windows 没有可用本机翻译源时虽看不到译文，共享默认值实际也为开启，造成平台体验差异。

## Decision

共享 `Preferences::default` 的 `candidate_translations` 改为关闭，让新装、各产品版本与用户主动恢复默认都采用关闭状态。只改初始默认值；已有文档的显式开关完整保留，旧文档缺失字段时仍按此前契约读取为开启。

[Fcitx5 缺字段默认值](../../implemented/bug-fix/2026-10-13-linux-fcitx-candidate-translations-default.md)部分相关且继续有效：该篇约束缺字段兼容读取，不能把它连同新装默认一起改掉。各宿主和共享页面继续接收共享层产生的完整快照，不各自增加首启判断。Android 的 `InputFeatureToggle` 初始显示表同步关闭，已完成反序列化的旧配置仍传入显式开启值。

## Alternatives considered

- **只在 macOS 关闭默认值**：影响范围最小，直接回应报告的平台；但 Windows 与其他平台仍保留相同开关的不同初始状态，设置页和引擎读到的共享默认值也会分歧。
- **把缺字段的 serde 默认也改成关闭**：初始化与反序列化表面更统一；但会在升级时替缺少新字段的旧用户改变行为，与新装的诉求无关。
- **新增首启功能向导**：可以同时解释多个可选功能；但该请求已经明确接受默认关闭，修改共享初值即可达成，不必增加一轮新交互。

## Verification

新增真实存储回归在旧实现上以新装 `full` 默认开启失败，修改后 140 项偏好测试通过。测试覆盖所有产品版本、第一份文档落盘再读取、缺字段旧文档、显式开关往返和主动恢复默认。`cargo clippy -p msime-client-core --all-targets -- -D warnings` 与 `cargo fmt --all --check` 通过。

本地 quick 门禁通过，包含 Android 的 231 项 JVM 冒烟、偏好表与共享默认值契约检查、Rust 与 Wasm 检查、Windows x64 原生链接、共享壳交叉检查、macOS 宿主和共享 Apple 桥接编译；Linux 容器和 HarmonyOS 构建依赖缺失的阶段按环境跳过。`pnpm run verify-notes` 与 `git diff --check` 通过。没有把这些配置存储回归当成 macOS 真实新装编辑器验收。

## Consequences

新装不会自动请求或显示候选翻译，用户可在设置中开启。恢复默认也会采用新的关闭值，这是用户主动重置的结果；已有明确开启用户和缺字段旧文档继续保留开启行为，没有配置迁移或格式变化。
