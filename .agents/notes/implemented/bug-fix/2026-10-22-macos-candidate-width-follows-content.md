# Agent Note: macOS 候选窗按当前页内容恢复收窄

Status: implemented

## Problem

Issue #9198 报告 macOS 0.54.2 beta 中，向后翻页后即使候选变短，窗口仍保持之前的宽度。此前为减少打字时的伸缩而引入的宽度记忆跨越了所有可见候选页，把历史最大值作为排版的最小宽度；它不是 AppKit 未刷新窗口尺寸，而是主动阻止收窄。

## Decision

macOS 普通候选窗只按当前页候选和首行内容计算宽度，不再将历史最大宽度传入 `candidatePageGeometry:`。删除控制器中的横排宽度记忆及外观、皮肤变化时的重置分支。皮肤和装饰的最小宽度、屏幕边界、紧凑展开键以及竖排定位高度记忆保持不变。

这项决定取代[先前笔记](2026-10-20-macos-steady-candidate-width.md)中 macOS 的只增不减策略。旧笔记关于减少抖动的动机仍有参考价值，但不能以禁止窗口收窄实现。共享 `WidestCandidateWidth` 及 Linux 调用不变；[Linux 原生画布决定](../architecture/2026-10-21-linux-x11-native-candidate-default.md)不属于本次平台回归修复范围。

检索到的两篇相关活跃笔记均保留：macOS 旧决定由本篇部分取代并互链；Linux 原生画布笔记仅共享工具函数，不改变其行为。

## Alternatives considered

- **仅在翻页时清空宽度记忆。** 能保留连续打字时的稳定性，同时修复 issue 的翻页步骤；但同一页删除字符、候选或释义变短时仍不能收窄，保留了同一缺陷的另一条路径，因此不采用。
- **保留只增不减并增加设置开关。** 能兼容两种偏好，但会增加配置与各界面的维护成本；本次恢复内容自适应，不为回归引入新配置。

工厂皮肤布局专项也通过真实长候选切回短候选验证收窄；不再通过 KVC 写入已删除的历史最大宽度字段，普通、收紧与均分布局各自保留原有几何断言。

## Consequences

- 面板不隐藏时，翻到短候选页、同页候选变短都能收窄，长候选再次出现时仍能变宽；皮肤规定的最小宽度仍可能保留空白。
- 连续打字时窗口可以再次伸缩，这是恢复内容自适应的代价。若后续需要降低抖动，应独立设计可收窄的策略，不能重新引入无界历史最大值。
- `ShortcutTest.mm` 使用真实控制器和离屏面板验证普通皮肤及非 `fitContent` 外部皮肤的短→长→短序列，并检查翻页控件没有越界。该证据属于原生测试，不等同于安装后在真实编辑器中的系统输入源验收。

## Testing

- `cmake --build target/macos-isolated --target shortcut-test candidate-placement-test --parallel 6`：通过。
- `ctest --test-dir target/macos-isolated -R '^(shortcut|shortcut-translations|candidate-placement)$' --output-on-failure`：三项通过。
- `pnpm run verify-notes`、`git diff --check`：通过。
- `DEVELOPER_DIR=/Applications/Xcode.app/Contents/Developer MSIME_SPARKLE_ROOT="$HOME/Developer/Dependencies/Sparkle-2.9.6" bash scripts/verify-local.sh --quick`：通过，含完整 macOS 原生宿主构建。默认选中的 Command Line Tools 缺少 SwiftUI 宏插件和 XCTest，首次门禁因此失败；仅为本次命令指定完整 Xcode，不修改机器的全局工具链选择。
