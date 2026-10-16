# Agent Note: macOS 候选窗即时显隐

Status: implemented

## Problem

#7760 报告 0.53.0 的候选窗出现动画拖慢快速输入。皮肤 v2 的默认淡入和选中弹簧进入两个 macOS 原生渲染路径，退出动画还需要异步回调维护窗口可见性。

## Decision

macOS 的 `InputController` 与兼容 `CandidatePanel` 同步显示、隐藏和切换选中项，并关闭 AppKit 窗口动画。皮肤动画数据仍能解析和同步，但 macOS 不执行它；其他平台保留各自行为。显示始终使用完整透明度，不再维护动画代次、缩放或迟到回调。

本决定被[只播放清单写出的动效](../feature/2026-10-16-macos-candidate-declared-motion.md)部分取代：皮肤包在 `[animation]` 里显式写出的预设重新在 macOS 播放，动画只加在图层上，退场在独立的退场窗里进行。下面这些仍然成立：显示、隐藏和选中状态同步变化；窗口始终完整不透明；不画包、内置主题和省略 `[animation]` 的 v2 包即时显隐。

本决定部分取代[皮肤 v2 动画契约](../architecture/2026-10-13-candidate-skin-v2-motion-contract.md)的 macOS 映射；该笔记的共享格式和其他平台决定仍成立。[原生宿主生命周期](../architecture/2026-10-13-candidate-skin-v2-native-host-lifecycle.md)与动画策略无关，保留其构建边界。

## Alternatives considered

只将默认皮肤的动画改成 `none` 可以保留自定义动效，但用户安装的 v2 皮肤仍会重现输入延迟，且两个原生渲染路径继续承担异步显隐状态。因此 macOS 在宿主层统一忽略动画。

## Consequences

快速输入不再等待候选窗动效，隐藏后立即重新显示也不会被旧回调收起。macOS 皮肤失去显隐和选中动画能力；视觉颜色和几何继续生效。原生面板测试验证完整透明度、立即隐藏、重新显示和无选中动画；控制器布局测试验证实际渲染路径没有缩放或选中动画。
