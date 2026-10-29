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

Windows 发布的设置窗口是原生 WinUI（`platforms/windows/settings/main.cpp`），不用共享页。#10210 在它的「高级选项」里模糊音之后加了同一组（`build_autocorrect_group`）：只在全拼、双拼方案下画，组名、标题和说明与桌面共享页 `input-page.tsx` 逐字相同，写的仍是 `quanpin.autocorrect_neighbor`、`quanpin.autocorrect_transposition`，默认开。文案在 `input-page.tsx` 和 `main.cpp` 各写一份，没有抽成共享表，因为 WinUI 窗口的其他行也是直接写在 `main.cpp` 里；`tests/tools/settings_ui.ps1` 的 `Assert-PinyinAutocorrect` 为了在真实窗口里找行又抄了一份标题和乱序说明。UI Automation 只能把窗口和 ps1 绑在一起，改共享页而漏改 `main.cpp` 时它照样通过，所以另加 `scripts/test-windows-autocorrect-settings-copy.py`（Core CI 的 contracts job 自动发现执行）：按出现顺序逐条比较 tsx 与 cpp 这一组的组名、标题和说明，要求 ps1 用到的每一句都在 cpp 里，偏好字段三处一致。`Assert-PinyinAutocorrect` 在全拼、双拼两个方案下都经实际开关各关一次再打开，关着时切走标签再切回来让整页重画，开关必须读回关；默认值和回退值都是开，只看默认态分不出开关读的是字段还是回退值。Win7 原生设置（`win7/AdvancedInputForm.h`）不在 #10210 的范围内，仍没有这两个开关。
