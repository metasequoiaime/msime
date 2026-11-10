# Agent Note: Android 折叠屏按内外屏分别保存键盘高度

Status: implemented

## Problem

折叠屏的外屏和展开屏使用不同的键盘高度更合适，但 Android 目前只有一个 `platform.android.keyboard_height_adjustment`，来回开合时必须反复修改设置。#11094 要求在 `smallestScreenWidthDp >= 600` 的展开形态保存独立高度，并保持老用户升级后的原有手感。

## Decision

新增本机键 `platform.android.keyboard_height_adjustment_large`，取值范围与旧键相同，默认不显式写入且不参与同步。`KeyboardHeightSettingPolicy` 统一按 `KeyboardFormFactorPolicy.expanded` 选择读写键：窄屏只用旧键，展开屏优先使用新键；新键缺失时回退旧键，再回退共享偏好里的高度。

设置页和首页摘要使用同一策略，展开屏标题显示「键盘高度（展开屏）」。键盘调整确认写入当前形态的键；窄屏重置删除旧键，展开屏重置写入显式的 0，避免仍回退到窄屏已保存的高度。配置跨过 600dp 边界时立即刷新本机设置并重排几何；若正在拖动高度，先丢弃未确认预览，避免把一块屏的值保存到另一块屏。

## Alternatives considered

- **继续共用一个本机键**：改动最少，但无法满足内外屏分别保持用户手感的需求。
- **大屏缺失时使用固定默认值**：逻辑简单，但升级后第一次展开会改变已有用户的高度；因此回退旧键。
- **跨形态时保存当前拖动预览**：可以保留用户最后一次拖动，但预览属于旧屏坐标和旧形态，切屏时保存会污染另一块屏；因此取消本次预览。

## Consequences

- 老用户没有新键时行为不变；首次在展开屏调整后，两种形态各自独立保存。
- 高度设置键数量增加一个本机字段，`AndroidLocalSettingsSmoke` 和策略 JVM 冒烟覆盖默认值、边界和回退规则。
- 本机无法在当前磁盘余量下启动完整 Android 构建，也没有 Fold6 真机可供验收；设备和真实折叠切换仍需 CI 或报告者复验。
