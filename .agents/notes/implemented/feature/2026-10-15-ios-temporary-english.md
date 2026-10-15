# Agent Note: iOS Shift 临时英文在片段结束后恢复原方案

Status: implemented

## Problem

iOS 中文键盘轻点 Shift 会调用持久中英切换，输入一个字母或英文单词后仍留在英文；开启「沿用上次」还把这次临时输入记成下次键盘的默认语言。用户需要在中文句子里短暂输入英文，而非每次手动切回中文。

## Decision

中文方案的 Shift 在不承担辅助码、韩语双辅音、越南语或藏文大小写时，进入键盘进程内的临时英文。单击只大写下一字母，空格、标点、回车、Tab 或选定英文补全后恢复中文。双击锁定大写，保持到再按 Shift；外部编辑、换输入框或收起键盘会清掉临时状态。临时进入和恢复均不写中英文记忆；底行中/英和方案选择器的显式选择仍是持久选择。URL／邮箱字段的英文优先，离开后恢复进入临时英文前的中文状态。

## Related notes audit

[手机中英键位置](../../implemented/feature/2026-10-09-mobile-language-key-beside-return.md) 属于布局，另行记录隐藏开关；[拼音组合中保留 Shift 大写字母](../../implemented/bug-fix/2026-10-11-shifted-uppercase-composition.md) 和[大写拼音起始](../../implemented/bug-fix/2026-10-14-uppercase-pinyin-start.md) 属于 Engine 组合规则，保留原边界；[Caps Lock 语言切换](../../implemented/feature/2026-10-11-caps-lock-language-switch.md) 属于桌面实体键，不修改。检索其余 Shift、临时英文及中英记忆笔记，无已有 iOS 临时返回契约。

## Alternatives considered

- 下一字母打完立刻回中文：单个字母最快，但输入单词需要逐字按 Shift，用户已选择以空格／标点结束英文片段。
- 继续永久切英文、只增加设置文案：保留现有状态机最省实现，但无法消除每次手动切回中文，也会污染「沿用上次」。

## Verification

原生键盘模拟器回归 151 项执行、1 项因未内置笔画词库跳过、0 失败，包含 NineKeyKeyboardTests、KeyboardFormFactorTests、KeyboardGeometrySettingsTests、KoreanDubeolsikTests、CompositionBoundaryTests 和 HelpcodeSettingsTests。输入框切换边界及按键音去重补充后，临时英文及中英记忆 9 项最终再验证通过。XcodeGen 重生成工程无差异；`bash scripts/verify-local.sh --quick` 和 `pnpm run verify-notes` 通过。

## Consequences

临时英文与显式英文分别处理，片段结束不用再手动切回，且不会改写「沿用上次」；辅助码、韩语双辅音、越南语及藏文大小写沿用各自原规则。选择单词边界返回意味着连续英文要用中/英或方案选择器显式切换。双击大写锁定跨空格保持，解除时回原方案；外部编辑、焦点变化或收起键盘会清掉临时状态。原实现的 3 个测试先复现语言未恢复和记忆被改写，修复后通过。模拟器控制器测试不替代 TestFlight 真机系统入口验收。
