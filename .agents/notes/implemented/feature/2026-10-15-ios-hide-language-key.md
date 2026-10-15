# Agent Note: iOS 可隐藏底行中英键并扩大空格

Status: implemented

## Problem

iPhone 底行右侧有句号、中英和回车，空格偏左，右拇指选词时易按到句号。用户希望可隐藏中英键，腾出空格宽度；已有用户仍需要原排列。

## Decision

在 App「键盘 → 布局」增加默认关闭的「隐藏中/英键」本机开关，保存在 App Group，与 iPad 排列和手势类本机偏好一致，不引入共享配置字段。底行隐藏中英键时，其宽度份额交给空格，保留逗号、句号、回车及原顺序；下次键盘出现时重读。设置预览同步，恢复默认时重新显示中英键。语言仍可从工具栏的输入方案选择器切换。

## Related notes audit

[手机中英键位置](../../implemented/feature/2026-10-09-mobile-language-key-beside-return.md) 决定默认排列，保留并补充可选隐藏入口；[三平台原生设计](../../implemented/feature/2026-10-07-ios-harmony-all-platform-design.md) 范围更大，本改动只涉及 iOS 本机布局。检索空格加宽、隐藏中英及 KeyboardGeometry 笔记，无重复决定。

## Alternatives considered

- 直接更改所有用户默认底行：无需新开关，但会改变肌肉记忆，且旧位置是此前明确选择；用户已选择保留默认排列、提供自定义。
- 删除句号或把它移到左侧：能更直接避免右侧误按，但用户仍需直接输入句号；保留标点、让空格向右延伸更适合此次反馈。

## Verification

原生键盘模拟器回归 151 项执行、1 项因未内置笔画词库跳过、0 失败，包含 NineKeyKeyboardTests、KeyboardFormFactorTests、KeyboardGeometrySettingsTests、KoreanDubeolsikTests、CompositionBoundaryTests 和 HelpcodeSettingsTests。原生 App 的 `OnboardingUITests/testHideLanguageKeySettingPersistsAcrossRelaunch` 在 iOS 27 模拟器通过：点击开关、重启保持、还原偏好均成功，并检查设置页截图。XcodeGen 重生成工程无差异；`bash scripts/verify-local.sh --quick` 和 `pnpm run verify-notes` 通过。

## Consequences

默认排列不变，隐藏中英键时空格向右扩展，逗号、句号与回车仍可直接输入。26／14／9 键在 320、414 点宽，以及 iPad 分离式键盘的实际约束均通过，后者保持两半空格等宽和原中缝。模拟器截图已检查。隐藏后语言选择依赖工具栏入口，本机开关不同步到其他设备；默认重置会恢复中英键。模拟器不替代 TestFlight 真机拇指输入验收。
