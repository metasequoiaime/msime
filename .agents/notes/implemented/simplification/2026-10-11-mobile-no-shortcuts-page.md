# Agent Note: 手机上不提供外接键盘快捷键页

Status: implemented

## Problem

HarmonyOS 手机的「设置」根页面在「输入 / 表达 / 词库」这一组里列着「外接键盘快捷键」，Android 手机在宿主路由了实体键盘快捷键时也有这一页。只有接了实体键盘的人才用得到它，对其余用户是噪音。用户看过之后的要求是手机上不要这一页（先试过把入口挪进「键盘」页，用户看过后仍不要）。

## Decision

- `settings-page-model-visibility.ts` 的 `mobileHiddenPageIds` 无条件包含 `shortcuts`：所有手机宿主（`mobilePlatform`）都不提供快捷键页，根页面、「全部设置」列表和侧栏里都没有它，`selectPage` 也不会去。以前它只在宿主一个快捷键都不路由（`mode_switch_shortcuts`、`panel_shortcuts`、`maintenance_shortcuts`/`panel_windows` 都为假）时隐藏。
- `keyboard/root-settings-list.tsx` 的 `rootGroupLayout` 去掉 `shortcuts`，`settings/mobile-tab-helpers.ts` 的 `mobilePageTitle` 去掉它的手机标题「外接键盘快捷键」。
- 桌面和 HarmonyOS 2-in-1（不是 `mobilePlatform`）的「快捷键」页不变。宿主读取快捷键的代码不变，手机上接实体键盘时按默认值和已保存的值处理这些按键。

这推翻了 [iOS 与 HarmonyOS 全平台设计](../../implemented/feature/2026-10-07-ios-harmony-all-platform-design.md) 里「鸿蒙首页另外保留……外接键盘快捷键……」中关于外接键盘快捷键的那一项；候选栏、剪贴板、AI 辅助仍留在根页面。

## Alternatives considered

- 把入口挪进「键盘」页底部的「外接键盘」一组，根页面搜索仍能找到它。理由最强的一条：接了实体键盘的手机用户仍能改中英切换、简繁切换、数字键选词和翻页键这些宿主确实读取的开关，根页面又清爽了。没采用：这样实现过，用户看过之后仍要求手机上不要它。
- 只在 HarmonyOS 手机上隐藏，Android 保持「宿主路由了快捷键就显示」。理由：以前隐藏条件特意不按平台名，是因为按平台隐藏曾让 Android 用户改不到这些开关。没采用：用户的要求是「手机不要」，不分平台；Android 手机接实体键盘同样少见。

## Consequences

- 手机上接了实体键盘的用户改不了中英切换键、简繁切换键、数字键选词和翻页键，只能用默认值或此前已保存的值；要改得在桌面端改后同步，或者等以后手机上重新提供入口。
- 手机根页面和列表少一行。
- 以后若要在手机上恢复，`mobileHiddenPageIds` 里去掉 `shortcuts` 并给它在根页面或某一页里放回入口，同时恢复 `mobilePageTitle` 里的手机标题。

## Verification

- `apps/desktop`：`tests/settings/mobile-app-chrome.test.tsx` 改为 Android 手机在宿主路由了快捷键时侧栏里也没有「外接键盘快捷键」和「快捷键」；`tests/core/home-page.test.tsx` 根页面的行和搜索结果里没有它；`tests/settings/mobile-tab-helpers.test.ts` 去掉它的手机标题。全部测试通过。
- HarmonyOS 模拟器上看过根页面和「键盘」页都没有它。没有在 Android 设备和真机上验证。
