# Agent Note: Android 按键布局选择不再被旧键覆盖

Status: implemented

## Problem

Android 0.7.3 的「按键布局」页把 26 键样式和顶部数字行保存到共享偏好，但升级迁移留下旧的 `platform.android.long_space_layout` 或 `platform.android.number_row` 时，用户主动选择共享默认值「标准」或「关闭」会让 client-core 省略字段。下一次迁移把旧值当成未选择，再次写回共享偏好，键盘看起来一直保留旧布局。

## Decision

设置页显式保存 `touch_twenty_six_key_style` 时先删除 `platform.android.long_space_layout`，显式保存 `touch_number_row` 时先删除 `platform.android.number_row`，然后再写共享偏好。清理只按当前共享字段映射对应旧键，不触碰九键旧表、九键滑动或其他 Android 本机设置。这样即使共享值是默认值、在磁盘上没有字段，旧迁移来源也不会再次覆盖用户选择。

迁移规则和现有默认值不变：升级时仍会把旧键迁进共享字段；本次只处理设置页已经明确选择后的旧键残留。清理和共享保存位于同一个后台任务中，清理失败则不写共享值并让页面回读。

## Alternatives considered

- 只修改 Rust 迁移条件，不再把旧值覆盖到共享默认值：会让尚未迁移的老用户丢失旧的长空格或数字行选择，因此没有采用。
- 保存共享偏好后再异步删除旧键：共享写入与删除之间仍可能被键盘进程迁移，且默认值被省略后会再次出现同一覆盖窗口，因此没有采用。

## Consequences

旧键清理不具备跨文件事务性；如果共享保存失败，旧键仍会被删除，但共享文档保留原有选择，下一次打开不会再被旧值覆盖。新增的纯 Java 冒烟断言固定两项共享字段与旧键的一一映射，并确认 14 键、九键样式不会误删无关旧键。

验证受本机条件限制：已运行源码级检查和 Android JVM 冒烟；当前工作机磁盘约 7.7 GiB 且未配置 Android SDK，未启动模拟器、未做设备或真实编辑器验收。

相关设计：[触屏按键布局样式与顶部数字行](../feature/2026-10-27-touch-key-layout-styles.md)。
