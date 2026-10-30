# Agent Note: 鸿蒙键盘的皮肤选择器最后一格「更多皮肤」打开设置的「皮肤」页

Status: implemented

## Problem

仓库负责人问键盘的皮肤选择器为什么只有 4 个主题。两个原因：

- 主题区分明暗（#7108），浅色模式下不列夜青、墨、水杉，只剩跟随系统、浅色、纸白和自定义。
- 下载的社区皮肤和保存的设计在设置应用的文件里，键盘进程读不到：两边没有共享沙箱，要华为在签名 Profile 里给 data-group-id（`HarmonySharedSandboxPolicy`，工单 D645548，尚无回复）。

第二点在华为批下来之前绕不过去，但选择器里连一个去看全部皮肤的入口都没有。

## Decision

- 选择器最后多一格「更多皮肤」（`MoreSkinsTile`，和皮肤缩略图同样大小的虚线框，中间「+」）。点了收起面板，调用 `KeyboardSession.openSettings('skin')`，设置应用打开「皮肤」页。分页按皮肤数加一算。
- 键盘能打开哪些设置页由两份名单决定：键盘和 `EntryAbility` 共用的 `SettingsPageLink.PAGES`，以及网页 `apps/harmony/src/settings-link.ts` 的 `LINKED_PAGES`（原在 `main.tsx`，2026-10-30 随[链接子目标](2026-10-30-harmony-settings-link-target.md)挪出）。两份都加上 `skin`。
- 第一次只改了 `SettingsPageLink.PAGES`：键盘发出请求、`onNewWant` 也收到了，网页按自己的名单把 `skin` 丢掉，面板收起后什么也没发生。新增 `scripts/test-harmony-settings-links.py`，核对两份名单一致、而且都是设置注册表里的页面。

## Alternatives considered

- **在键盘里直接列社区皮肤。** 键盘读不到那些文件，这正是问题所在。
- **把深色主题也列进浅色模式。** 那是 #7108 有意去掉的，键盘和周围界面颜色反着。

## Consequences

- **收益**：在键盘里想换别的皮肤，有一步可走的入口。
- **代价**：「更多皮肤」会离开当前输入的应用；华为批下共享沙箱后，应该把社区皮肤和保存的设计直接列进选择器，这一格可以保留作为浏览全部的入口。

## Verification

- `platforms/harmony/tests/kb-session.test.ts`：`SettingsPageLink` 接受 `skin`。
- `scripts/test-harmony-settings-links.py`：两份名单一致；把网页名单换回 develop 的版本时报出缺少 `skin`。
- HarmonyOS 6.0.2 phone 模拟器：在设置应用里「试用键盘」弹出水杉键盘，点工具栏的皮肤图标，最后一格是「更多皮肤」；点它，面板收起，设置应用跳到「皮肤」页，社区皮肤都在。
