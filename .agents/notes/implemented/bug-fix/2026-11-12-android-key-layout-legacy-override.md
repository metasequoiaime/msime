# Agent Note: Android 按键布局显式保存时清除残留旧键

Status: implemented

## Problem

0.7.3 把 26 键样式和顶部数字行迁到共享偏好后，旧的 `platform.android.long_space_layout` 或 `platform.android.number_row` 若因迁移删除失败而留在 Android 本机设置，用户主动选择共享默认值「标准」或「关闭」时 client-core 会省略字段，下一次迁移就会把旧值当成未选择，再次写回共享偏好。

这只是一个理论上的覆盖窗口，不是 #11279 的已证实根因。`KeyLayoutStylesPage.reload()` 和键盘的 `withLivePreferences` 每次读偏好前都先跑迁移，迁移成功即删掉五个旧键，正常流程里用户点胶囊时旧键早已不在；迁移条件（`crates/client-core/src/preferences.rs`）也只会覆盖缺键或明写 `standard` 的 26 键样式，解释不了报告中「逗号句号」「表情键」同样不生效。#11279 的真实原因仍待复现，本变更只作纵深防御。

## Decision

设置页显式保存 `touch_twenty_six_key_style` 时先删除 `platform.android.long_space_layout`，显式保存 `touch_number_row` 时先删除 `platform.android.number_row`，然后再写共享偏好。清理只按当前共享字段映射对应旧键，不触碰九键旧表、九键滑动或其他 Android 本机设置。清理和共享保存位于同一个后台任务中，清理失败则不写共享值、提示保存失败并让页面回读。

迁移规则和现有默认值不变：升级时仍会把旧键迁进共享字段。

## Alternatives considered

- 只修改 Rust 迁移条件，不再把旧值覆盖到共享默认值：会让尚未迁移的老用户丢失旧的长空格或数字行选择，因此没有采用。
- 保存共享偏好后再异步删除旧键：共享写入与删除之间仍可能被键盘进程迁移，因此没有采用。

## Consequences

旧键清理不具备跨文件事务性；如果共享保存失败，旧键仍会被删除，但共享文档保留原有选择。旧键残留通常意味着本机设置文件写不进去，此时清理多半同样失败，用户会看到「保存失败」而不是静默被覆盖。新增的纯 Java 冒烟断言只固定两项共享字段与旧键的一一映射，并确认 14 键、九键样式不会误删无关旧键；它不是 #11279 的补丁前失败、补丁后通过回归。

#11279 后续排查方向：非标准样式（`comma_period`、`emoji`）保存后是否被持久化（`KeyboardSheets.write` 是否返回 null 或遇到版本冲突），以及运行中的键盘进程是否重读 `touch_twenty_six_key_style` 并刷新皮肤片段与布局缓存（`MSIMEInputService` 的 `SKIN_HINT_KEYS`、`touchKeyStylesFrom`、`TouchKeyStyle.Styles.forLayout`）。找到原因并补上补丁前失败的回归后才能关闭该 issue。

相关设计：[触屏按键布局样式与顶部数字行](../feature/2026-10-27-touch-key-layout-styles.md)。
