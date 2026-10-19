# Agent Note: 默认启用输入串两端的左右候选导航

Status: implemented

## Problem

已有 `navigation.left_right` 能在输入串光标到达端点时把左右键转换为候选移动，但默认值仍为关闭。macOS 用户按系统输入法的习惯在拼音末尾按右箭头时，水杉因此吞掉按键而不选择候选；#8847 报告的默认操作仍未被 #8346 覆盖。

## Decision

沿用 #8346 已实现的共享 runtime 规则，只把共享 Rust、共享设置页和 macOS 回退缺省改为默认开启。用户显式关闭后仍保留关闭状态；旧文档缺少该字段时按新默认开启，下一次保存会写出 `true`。

macOS 的共享偏好同步同时接受显式 `false`，避免用户关闭后旧的进程内覆盖继续保留。现有 #8346 的「编辑优先」、整句改字和特殊候选列表边界不变。

## Alternatives considered

- **继续保持默认关闭，只在文档中提示用户手动打开。** 这不能满足 #8847 的复现路径，用户仍会在默认安装上遇到无响应的右箭头。
- **在 macOS 宿主单独默认开启。** 这会让各平台对同一份 `navigation.left_right` 的缺省不一致，且把共享偏好规则复制回宿主。

## Consequences

默认安装在输入串两端按左右键会移动候选；输入串仍在中间时优先编辑。显式关闭会恢复旧行为。缺少该字段的旧配置在 Rust 和共享设置页加载后都按开启解释，但不被强制立刻重写；用户修改翻页设置并保存时会沿用开启值写回。

## Historical audit

此决定取代了默认关闭的决定，见 [2026-10-16-left-right-candidate-navigation.md](../feature/2026-10-16-left-right-candidate-navigation.md)；共享 runtime 的实现和边界保持不变。

## Verification

- `cargo test -p msime-client-core`：426 项通过。
- `cargo test -p msime-host-api left_right_navigation_preference_reaches_real_sessions_and_live_updates`：通过，覆盖默认开启和运行中显式关闭。
- `pnpm --filter @msime/desktop exec vitest run tests/settings/candidate-paging-options.test.ts tests/settings/candidate-paging-preferences.test.ts`：13 项通过。
- `pnpm --filter @msime/desktop exec vitest run tests/settings/settings.test.tsx -t "word-to-character and paging|paging defaults match Windows"`：2 项通过。
- `pnpm --filter @msime/desktop typecheck`：通过。
- 旧文档缺少 `navigation.left_right` 的共享设置模型、翻页选项和候选速查回归测试通过，且未补写可选的 `mouse_wheel` 字段。
