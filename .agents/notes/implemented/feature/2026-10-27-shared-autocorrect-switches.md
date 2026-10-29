# Agent Note: 共享设置页拆分全拼纠错开关

Status: implemented

## Problem

共享输入设置页把全拼邻键纠错和全拼乱序纠错合并为一个开关。底层偏好本来就是两个独立字段，因此用户无法只关闭会把 `zhr` 判成 `zhe` 的邻键纠错，同时保留乱序纠错，或反过来。

## Decision

在 HarmonyOS 手机共享输入页分别显示“拼音邻键纠错”和“拼音乱序纠错”，各自只更新对应的 `quanpin` 字段，并保留默认开启行为。最初写的是“全拼…”；双拼纠错整句（[2026-10-09](2026-10-09-shuangpin-typo-sentence.md)）和触屏双拼纠错（#10018）也受这两个字段控制，所以标题改成“拼音…”，`apps/desktop/tests/settings/language-card.test.tsx` 按新标题找开关。macOS 原生设置里同名的两个开关仍写“全拼…”，但那张卡片是隐藏的，用户看不到，没有改。

## Alternatives considered

继续保留合并开关：实现简单且与 Android 页面一致，但无法满足已有独立偏好字段的可配置性，因此不采用。

## Consequences

用户可以独立关闭两类全拼纠错，设置保存后宿主仍沿用原有字段。桌面共享设置页的同一对开关放在「高级选项」的「全拼纠错」组，`qjn`、`zhr` 这类全辅音简拼被纠错抢首位的问题由引擎修复，见[全辅音简拼的字面整词排在全拼纠错前面](../bug-fix/2026-10-29-quanpin-jianpin-over-autocorrect.md)。

Windows 发布的设置窗口是原生 WinUI（`platforms/windows/settings/main.cpp`），不用共享页。#10210 在它的「高级选项」里模糊音之后加了同一组（`build_autocorrect_group`）：只在全拼、双拼方案下画，组名、标题和说明与桌面共享页 `input-page.tsx` 逐字相同，写的仍是 `quanpin.autocorrect_neighbor`、`quanpin.autocorrect_transposition`，默认开。文案在两处各写一份，没有抽成共享表，因为 WinUI 窗口的其他行也是直接写在 `main.cpp` 里，改共享页的说明时要同步改 `main.cpp`，`tests/tools/settings_ui.ps1` 的 `Assert-PinyinAutocorrect` 按乱序纠错的完整说明找行，漏改会在 CI 的「Windows native settings」任务里失败。Win7 原生设置（`win7/AdvancedInputForm.h`）不在 #10210 的范围内，仍没有这两个开关。
