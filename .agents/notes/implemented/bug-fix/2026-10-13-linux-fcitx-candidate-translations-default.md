# Agent Note: Fcitx5 缺失候选翻译偏好时沿用共享默认值

Status: implemented

## Problem

共享偏好契约规定 `candidate_translations` 缺失时默认开启，IBus 和 Host API 都按这个规则处理。Fcitx5 宿主的状态栏动作、翻译请求和 Ctrl+Enter 快捷键却用 `false` 作为缺省值。旧 runtime options 或偏好文档缺少该字段时，Fcitx5 会把候选翻译显示为关闭，且第一次切换的方向与其他宿主不同。

## Decision

Fcitx5 的四个读取点统一使用共享契约的缺省值 `true`。显式 `false` 仍关闭，显式 `true` 仍开启；读取逻辑留在 Fcitx5 宿主，避免给被 macOS 测试复用的候选策略头文件增加平台专属 JSON 依赖。

## Alternatives considered

- **只修改状态栏勾选状态**：快捷键和实际 provider 请求仍会把缺失字段当成关闭，行为会继续分裂。
- **把共享默认值改成关闭**：会改变 IBus、Host API 和已存在配置的兼容行为，也不符合当前偏好契约。
- **启动时补写字段**：会把只读兼容逻辑变成配置迁移，增加写入和并发冲突；读取时使用契约默认值即可。

## Consequences

旧配置在 Fcitx5 下恢复与共享契约一致的开启行为；用户明确保存的 `false` 不受影响。新增策略单测和 native 测试覆盖缺字段状态栏切换。

## Verification

- `bash platforms/linux/build-container.sh`：构建通过，83/83 ctest 通过。
- `git diff --check` 通过。
