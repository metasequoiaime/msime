# Agent Note: iOS 九键一族切英文时字母区与九宫格等高

Status: implemented

## Problem

[26 键排间距保持紧凑](2026-10-18-ios-letter-spacing-compactness.md)让手机竖屏 26 键用 44pt 键帽、11pt 排距，再在字母区上下各留 10pt，补足到九键的外框高度。外框不跳，但字母区实际覆盖的高度比九宫格短 20pt。维护者在九键双拼上切到英文时指出，内部键区要和九键保持一致：按键区上下都缩进一截，看起来像换了一块键盘。

Android 和 HarmonyOS 没有这个差别：Android 的字母行和九键行用同一个 `KeyboardGeometry.KEY_ROW_HEIGHT_DP`，HarmonyOS 的九宫格和字母行都从同一块键面高度里分出来，所以只改 iOS。

## Decision

当前中文方案属于九键一族（拼音九键、九键双拼、笔画，`TouchKeyLayoutOffering.nineKeySchemes`）时，手机竖屏画 26 键（英文或本地模式）不留上下 10pt：`KeyboardViewController.phoneLetterVerticalInset` 对这几个方案返回 0，按键列的 `layoutMargins` 和键区面板的底边都读它。键盘总高仍按原公式算，字母行与底行等高、由按键列均分，所以默认行距下键帽从 44pt 长到 49pt（四排 52pt 加三条 7pt 的 229pt，由四排 49pt 加三条 11pt 填满），排距保持紧凑的 11pt，字母区的上下边界与九宫格重合。

全拼、双拼 26 键等其他方案不变，仍是 44pt 键帽加上下留白：这些用户的中文和英文都是 26 键，没有九宫格可以对照，改了只会改变他们已经习惯的键帽高度。横屏和 iPad 也不变。

本决定在九键一族上取代[26 键排间距保持紧凑](2026-10-18-ios-letter-spacing-compactness.md)的上下留白，其他方案仍按那篇。

## Alternatives considered

- **把留白的 20pt 摊到三条排距**（[26 键内部键区与九键等高](2026-10-18-ios-letter-inner-height-alignment.md)的做法）：字母区同样与九宫格重合，键帽保持 44pt。但排距会从 11pt 涨到约 18pt，同一天已经因为字母键之间空得太松被撤回，不再重复。
- **恢复 52pt 键帽**：与九宫格逐排同高。但字母键又高又窄，排距只剩 7pt，两篇旧笔记都因此放弃。这里只多 5pt，排距不变，是两者之间的折中。
- **所有 26 键都改成 49pt**：规则更简单，三端更接近。但会改变全拼、双拼用户的手感，而他们没有提出过问题，所以只改有九宫格可以对照的九键一族。

## Consequences

九键一族用户切到英文时，字母区和底行与九宫格逐边对齐，键帽高 49pt；切回中文时按键区边界不动。键盘外框高度、高度调节条的百分比（按标准键区算）和其他方案的 26 键都不变。用户调过行距或键盘高度时，字母键按同样的均分随之变化，边界仍与九宫格重合。

`NineKeyKeyboardTests.testNineKeyAndEnglishKeepTheSameKeyboardHeight` 对拼音九键和九键双拼各跑一遍，检查外框高度不变、`字母 Q` 的上沿与 `nineKey1` 的上沿重合、英文底行回车的下沿与九键的重合、默认行距下键帽 49pt。全拼切英文仍是 44pt，由 `testSymbolLayerFollowsTheDesignsWeights` 覆盖。
