# Agent Note: Android 拼音显示位置

Status: implemented

## Problem

Android 原先固定把全拼、双拼、五笔和粤拼的拼写写入输入框，并在候选栏隐藏拼写。共享偏好 `candidate_preedit_style` 在触屏宿主上被忽略，用户无法在输入框和候选栏之间选择显示位置；九键和 14 键还必须避免把数字串或组码字母写入输入框。

## Decision

Android 设置页「键盘」增加「拼音显示位置」，读新增的共享偏好 `touch_spelling_position`：`editor`（缺省）表示拼写作为组字写入输入框，`candidate_bar` 表示拼写只显示在候选栏。缺失或非法值回退到 `editor`，保持旧版 Android 行为。

最初的实现复用了 `candidate_preedit_style`（`empty` 为输入框、`pinyin` 为候选栏），并以为缺失时会回退到输入框。这个前提不成立：client-core 总是把这个字段序列化进文档，默认值是 `pinyin`（桌面候选窗画拼音行），所以 Android 永远读到 `pinyin`，所有没动过设置的用户都被切到了候选栏，合入后设备冒烟的 phrase composition 在 API 28 和 API 35 上同时失败。两个键的含义也不同：iOS 和 HarmonyOS 把 `candidate_preedit_style` 读作「候选栏画不画拼写」，与「拼写放在哪」不是同一件事。因此改为独立字段，`candidate_preedit_style` 在 Android 上不再被读取，`check-host.sh` 守住这一点。

`CandidatePreeditStylePolicy` 统一规范化偏好并决定两件事：组字是否写入编辑器，以及候选栏是否绘制拼写。盛京银行的应用兼容例外仍然把拼音留在候选栏；14 键、编辑器截短和非拼写类方案仍按原有规则处理。候选栏模式下九键继续用 `nine_key_reading` 显示拼音，绝不写入按下的数字；已选的 `phrase_prefix` 始终独立保留在候选栏。

这项决定取代触屏宿主永久忽略 `candidate_preedit_style` 的旧约定，见 [触屏组字笔记](2026-10-15-touch-composition-in-field.md)。桌面和 HarmonyOS 的既有语义不变。

## Alternatives considered

- **继续固定写入输入框**：实现最小，但无法满足 Android 设置中切换显示位置的需求。
- **新增 Android 本地开关**：可以隔离 Android 行为，但会让共享设置与 Android 脱节，跨设备同步时产生两个来源；因此用共享字段。
- **复用 `candidate_preedit_style`**：最初的选择，见上。它的默认值服务桌面候选窗，无法区分「用户选了候选栏」和「从没设置过」，含义也和其他触屏宿主不一致。
- **把 `candidate_preedit_style` 的默认值改成 `empty`**：会让 macOS、Windows、Linux 候选窗默认不再画拼音行，代价远大于新增一个字段。

## Consequences

新增字段只有触屏宿主读取，桌面宿主原样保留（macOS 偏好覆盖清单记了原因）；账号同步字段表尚未收录它，跨设备同步要等字段表加上对应的键。设置变更经共享偏好热重载后立即影响当前会话，且不需要重启输入法。默认缺字段仍保持旧行为，用户选择候选栏后，拼音从编辑器组字移到候选栏，提交候选的结果不变。策略冒烟测试覆盖非法值、两种模式、应用例外和九键无数字串；Android 设备验证仍需覆盖 API 28 与 API 35 的两种位置及九键提交路径。
