# Agent Note: iPhone 分离式键盘的宽度门槛按键盘视图算

Status: implemented

## Problem

[iPhone 横屏与鸿蒙大屏的分离式键盘](../feature/2026-10-30-split-keyboard-iphone-harmony.md)给 iPhone 定了 50pt 的门槛：分开后第一排字母键 `(0.75 × 键区宽 − 8 × 键距) / 10` 不到 50pt 就不分。那篇的机型表把键区宽当成屏幕宽度（XR 896pt、16 Pro Max 956pt），算出全面屏 iPhone 都分、SE 和 Plus 不分；验证也只在模拟器上把键盘视图按屏幕宽度画成截图。

2026-10-30 在 iPhone XR（iOS 18.7）真机上打开开关、横屏，键盘仍是整块。截图量出键盘视图只有约 662pt：全面屏 iPhone 横屏时系统在键盘两侧各留一栏放地球键和听写键，扩展拿到的宽度远小于屏幕。按 662pt 算，分开后字母键是 44.4pt，过不了 50pt。最宽的 16 Pro Max 照同样的两侧留白也到不了 50pt，所以这个功能在真机上对任何 iPhone 都不生效。

## Decision

`KeyboardSplitLayout.phoneMinimumLetterKeyWidth` 改成 36pt，计算公式和其余条件不变，键区宽仍取键盘视图的宽度（`view.bounds.width` 减去两侧 3pt）。

36pt 的依据是 iPhone 竖屏时的字母键：SE 约 31.5pt、XR 约 35.4pt。分开后的键不窄于用户每天竖屏打字的键，就不会更难按。默认 6pt 键距下所有 iPhone 横屏都分，SE 也分（没有刘海，按横屏键盘占满 667pt 推算是 44.775pt，没有真机量过）；门槛只挡住窄得不正常的键盘视图。设置页说明去掉「屏幕较窄的 iPhone（如 iPhone SE）保持完整键盘」这半句。

XR 真机上打开开关、横屏后键盘分成 qwert | yuiop、asdfg | hjkl、zxcv | bnm，底行两个空格；在 Safari 地址栏打了一串拼音（nihosqaaa），左右两半的字母键都能输入，左半边的空格把首选上屏。

## Alternatives considered

- **门槛保持 50pt，按屏幕宽度判断**：保留原先「SE、Plus 不分」的区分。没有采用：判断的目的是分开后的键够不够按，而键的宽度由键盘视图决定，不由屏幕决定；按屏幕宽度算会在全面屏机型上画出 44pt 的键却声称它有 62pt。
- **拿掉 iPhone 的宽度门槛**：iPhone 横屏的键盘视图都够宽，门槛已经挡不住任何真机。没有采用：留一个下限的代价是一个常量，能防住将来系统给出很窄的键盘视图（例如新的窗口形态）时画出没法按的键。
- **继续排除 SE**：按机型或有没有刘海区分。没有采用：SE 横屏的键盘视图（按占满屏宽推算为 667pt）不比 XR（约 662pt）窄，分开后的键也更宽，排除它没有尺寸上的理由；按机型判断也违反 `KeyboardFormFactor` 不按机型判断的约定（`platforms/ios/README.md`）。

## Consequences

- **收益**：iPhone 横屏的分离式键盘真正可用，打开开关的 iPhone 都会分。
- **代价**：SE 用户打开开关后也会分，键宽约 44.8pt，比竖屏宽，但底行功能键跟着变窄。默认关，现有用户不受影响。
- **只量过 XR**：其他全面屏机型横屏的键盘视图宽度按同样的两侧留白推算，没有逐台量过；36pt 离 XR 的 44.4pt 有 8pt 余量，推算有偏差也不会跨过门槛。
- **测试**：`KeyboardFormFactorTests` 的 iPhone 一节改为 543/544pt 的边界、XR 662pt 和 SE 667pt 都分、540pt 的键盘视图不分。
