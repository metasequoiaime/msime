# Agent Note: 保存 @ 名单时保留联系人手机号

Status: implemented

## Problem

`mentions.json` 里的每一条是 `{text, key, phone}`。`phone` 来自移动端经用户授权导入的电话本（见 [Android 联系人计数与姓名候选](2026-10-21-android-contact-candidates.md)），win7 设置页也能逐条填写。共享设置页（macOS、Linux、鸿蒙 2in1）和新版 Windows 设置页的「插件 → @ 名单」只读写 `text` 和 `key`：读到的 `phone` 被丢掉，保存时整份名单原样替换，于是只要在这两页点一次「保存名单」，名单里所有手机号都被清空，普通拼音里的「姓名 + 手机号」候选随之消失。

## Decision

两页把 `phone` 当作不显示、不编辑、但必须原样写回的字段。

- 共享页：`MentionEntry` 带可选的 `phone`；保存时只修剪 `text`，其余字段原样展开回去。页面上新加的条目没有 `phone`，client-services 按 `#[serde(default)]` 记为空串。
- 新版 Windows：`Mention` 增加 `phone`，`load_mentions` 读出、`save_mentions` 写回，修剪和相等比较都带上它，所以只改手机号以外的字段时「有未保存的修改」照常判断。
- `msime_client.h` 里 `load_mentions` 的说明改为 `[{text, key, phone}]`，并写明保存时要把读到的 `phone` 原样带回去。

同一个 PR 里顺带清理插件页的几处小问题：win7 去掉不生效的「打字效果强度」（Windows 编进的 host-api 按 `HostPlatform::fixed_effect_intensity` 固定为 50，新版设置页本来就不列这一项）；win7 插件库的类型筛选补上脚本插件；`docs/plugins.md` 开头、`lifecycle.rs` 和 Linux 两个入口里「release 构建不运行脚本」「各平台陆续接入」这类与代码不符的说法改为现状。

## Alternatives considered

- **在 client-services 的 `save` 里按 `text` 合并旧名单的 `phone`。** 最强的理由是一处修好所有宿主，将来的新宿主也不会再犯。不用：名单允许改名，改了 `text` 就对不上原来的手机号；删掉一条再加回同名一条时也说不清该不该恢复手机号。替换整份名单是现有契约，宿主带回自己读到的字段最直接，也和 win7 已有的做法一致。
- **在共享页和新版 Windows 上也显示、编辑手机号。** 不用：这两页是 @ 名单的编辑入口，手机号只服务普通拼音里的联系人候选，来源是电话本导入；在这里加一列会让「@ 名单」和「联系人」混在一起，超出这次修复的范围。

## Consequences

带手机号的名单在任何设置页保存后都保持不变。共享页新增一条回归测试（读到带手机号的名单、改名、保存，断言手机号原样写回），去掉修复时它失败；Windows 的 `plugin_page_model` 单测覆盖修剪保留手机号、手机号参与相等比较。win7 不再有特效强度控件，偏好里已存的 `effect_intensity` 保持原值，Windows 本来就不读它。
