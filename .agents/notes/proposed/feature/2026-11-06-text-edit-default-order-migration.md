# Agent Note: 文本编辑侧键默认顺序换成设计图的顺序，并迁移没排过顺序的用户

Status: proposed

## Problem

[无缝方向区与七个侧键](../../implemented/feature/2026-11-06-android-text-edit-seamless-panel.md)的设计图里，侧键是左列复制、剪切、粘贴、全选，右列退格、123、空格；共享默认排列 `default_text_edit_button_order` 是 `copy, paste, select_all, cut, space, number_pad, delete, select_line`，面板上是左列复制、粘贴、全选、剪切，右列空格、123、退格。要做出设计图的顺序，只能改共享默认值。

只改默认值不够。已经发出去的版本把当时的默认排列明文写进了偏好（Android 0.6.0、HarmonyOS 0.3.0-beta、macOS 0.55.0、Linux 0.14.0 写七个动作，Windows 7 0.3.0 写八个，见 `crates/client-core/tests/fixtures/released-preferences`）：

- 这些用户的文档里明文写着旧顺序，换了默认值也看不到新顺序，新顺序只到全新安装的用户；
- 新版本存一次盘，旧顺序补上 `select_line` 成了八项，不等于新默认就不会按[默认排列不写进文件](../../implemented/bug-fix/2026-10-28-preferences-forward-compat-runtime.md)省掉；退回只认识七个动作的已发布版本时整份偏好读不了，打不了字（`a_default_text_edit_order_is_left_out_of_the_file` 守的正是这件事）。

这改变既有用户的出厂和迁移行为。维护者在 #11019 要求它不能随布局 PR 一起悄悄翻转，需要单独授权、单独定迁移规则和旧偏好兼容测试。另外，侧键顺序目前只有 Android 画；等别的移动平台也有类似的面板再一起定，默认值才在各端一致。

## Proposal

- 共享默认排列改为 `copy, cut, paste, select_all, delete, number_pad, space, select_line`。
- 在 `deserialize_text_edit_button_order` 补齐排列之后，结果正好等于旧默认排列 `copy, paste, select_all, cut, space, number_pad, delete, select_line` 时，当作没排过顺序，换成新默认。已发布版本写下的七项、八项两种写法补齐后都等于它，所以没排过顺序的用户直接换到新顺序；存盘时按默认排列省掉这个键，退回已发布版本仍读得了。读盘、导入、同步走同一个反序列化，口径一致。
- 直接提交给 `Preferences::validate` 和 `Preferences::from_submission` 的排列不经过这一步，保持严格校验。
- 一个参考实现曾在 `nuthatadf/msime` 的 `feat/android-text-edit-panel-redesign` 分支上（提交 `8069d42d20` 的 `client-core` 部分，后由撤回提交拿掉），包括新增测试 `an_untouched_old_default_text_edit_order_moves_to_the_new_default`。

## Alternatives considered

- **只换默认值，不迁移**：改动只有一行。但上面两条后果都在：已发布版本的用户看不到新顺序，存盘后退回旧版本整份偏好读不了。
- **保留用户文档里的旧顺序，排列以选择行结尾时只写前七项**：有意排成旧顺序的用户不会被改，退回旧版本也读得了。但已发布版本把默认值明文写进了所有用户的文档，等于绝大多数用户留在旧顺序，和换默认值的目的相反。
- **只对全新安装生效**：改动小。但仍是改出厂行为，老用户和新用户从此是两种顺序。
- **共享默认值不动，Android 换一种摆法**：不碰共享层。但排序页显示的顺序和存下来的顺序对不上，等于把真正的默认值藏起来。
- **不改**：没有迁移风险，面板照样能用，只是侧键顺序和设计图不同。在维护者授权之前就是这个状态。

## Acceptance criteria

- 维护者在单独的 issue 里同意改默认值和迁移规则，并确认各移动平台的面板是否一起改。
- 迁移规则写清：哪些排列算「没排过顺序」，有意排成旧默认顺序的用户如何处理，退回已发布版本时读到什么。
- 兼容测试：`released-preferences` 里每个已发布版本的文档读成新默认、存一次后不再写出这个键；只差一个位置的排列原样保留；严格提交校验不变；导入和同步的期望值随新默认更新。
- 各平台涉及的设备验收（Android 的排序页、默认侧键位置）在 API 28/35 和真机上跑过。

## Risks

- 有意把侧键排成旧默认顺序的用户会被换成新默认，文档里分不出「没排过」和「排成了一样的顺序」。
- 默认值写在共享层，别的平台（导入、同步、设置页的展示）读到的默认也随之变化，要逐个确认。
- 迁移只在读偏好时发生；不经过 `deserialize_text_edit_button_order` 的路径（直接提交、严格校验）不会换，要确认没有遗漏的读法。
