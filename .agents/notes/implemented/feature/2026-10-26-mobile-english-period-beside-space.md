# Agent Note: 英文底行在空格左边放句点、右边放逗号

Status: implemented

## Problem

维护者在 iOS 上用九键，切到英文后反馈：「左边少了个 . 按钮，空格左边 .，空格右边是 , 才对」，并说明指的是英文的时候。

iOS 和 Android 没有英文九键：九键按中/英切到英文，画的是 26 键（`NineKeyKeyboardTests.testNineKeyAndEnglishKeepTheSameKeyboardHeight`、Android `displayedTouchLayout` 在 `dedicatedEnglish` 时固定返回 26 键）。这一排底行按[手机底行保留逗号并以长按输入句号](2026-10-16-comma-key-secondary-period.md)是 123 | 符 | 空格 | , | 中/英 | 回车，句点只能长按逗号输入。英文按句子结尾要句点，长按这一步在英文里比中文频繁得多；空格右边的逗号已经在，缺的是空格左边的句点。

HarmonyOS 不同：九键切英文保留九宫格（输入方式面板里也有「英文 9 键」），左列是 `, . ?`，句点已经在左列，底行是 符 | 123 | 空格 | 中/英 | 回车，空格两边没有标点。

## Decision

三端英文模式的底行都在空格左边放句点、右边放逗号；中文底行不变。

- **英文 26 键**（iOS、Android、HarmonyOS 的手机 26 键字母层）：123 |（地球）| 符 | . | 空格 | , | 中/英 | 回车。句点占 1 份，这一份从空格里让出（空格 4 → 3），123、符、逗号、中/英、回车与中文 26 键同宽。逗号长按输入句号保留不变。常量：iOS `KeyboardViewController.englishPhoneSpaceWeight`、Android `KeyboardActionRow.DESIGN_ENGLISH_SPACE_WEIGHT`、HarmonyOS `ENGLISH_SPACE_SHARE`。
- **英文九键**（只有 HarmonyOS 有这个键面）：符 | 123 | . | 空格 | , | 中/英 | 回车。句点、逗号各 1 份，从空格里让出（4.2 → 2.2），其余键与中文九键同宽；左列 `, . ?` 和右列 `!` 不变，数字层不变（`NineKeyLayout.actionRow(digitLayer, english)`）。
- 句点和逗号都走各端已有的标点入口：iOS `handleSymbol`，Android `type('.')` 与快捷标点键，HarmonyOS `typePeriod` / `typeComma` 交给 Engine 的标点路由。英文下输出 ASCII，全角设置和「标点锁定为中文」照旧生效，与原来的逗号键一致。
- 判定英文的依据：iOS `!isChineseMode`，Android `dedicatedEnglish`（只影响 26 键一族的字母层，数字九宫格、九键、14 键、手写等底行不受影响），HarmonyOS `this.english`。网址、邮箱等临时英文也属于英文模式，同样多出句点。

## Alternatives considered

- **给 iOS 和 Android 新做一个英文九键键面，再在左列补句点**：这是对「九键左边少了个 .」最字面的读法，HarmonyOS 也确实有英文九键。但 iOS 和 Android 的英文从来是 26 键，维护者在 iOS 上看到的就是 26 键，那一排缺的正是空格左边的句点；新做一套英文九宫格（键面、组字、候选、数字层、统计）是另一项产品决定，这次没有被要求。
- **只在「从九键切过来的英文」上加句点**：最贴合报告的场景。但 iOS 和 Android 的英文只有一种 26 键，按来源方案改底行会让同一个英文键盘随上一次中文方案变样，不加。
- **保持现状，继续用逗号长按输入句号**：[逗号长按句号](2026-10-16-comma-key-secondary-period.md)当时放弃独立句号，理由是多一键会压缩空格。这次维护者明确要求英文时空格左边有句点；英文空格从 4 份让到 3 份，在 320pt 宽时仍有约 70pt，中文 26 键不受影响。
- **英文九键把左列的句点挪到底行**：能避免同一个句点出现两次。但左列 `, . ?` 是三端九键共用的设计，挪走会让英文九键的左列和中文九键对不齐，也偏离「保留左列现有项」的要求。

## Consequences

- 收益：英文模式下句点和逗号都能一次点到，位置三端一致；其余键的宽度不变，肌肉记忆不受影响。
- 代价：英文 26 键的空格窄了四分之一（4 → 3 份），英文九键的空格从 4.2 份让到 2.2 份；HarmonyOS 英文九键的句点和逗号在左列与底行各有一个。
- 打字统计的键位热力图仍按中文方案的底行画，不单独画英文底行；英文句点的按键记作 `Period`。
- 测试：iOS `NineKeyKeyboardTests.testEnglishBottomRowPutsPeriodLeftAndCommaRightOfSpace`（320pt、393pt，含渲染图）和 `testPhoneBottomRowFollowsTheDesignsWeights`；Android `KeyboardActionRowSmoke`；HarmonyOS `keyboard-logic.test.ts` 的「英文九键字母层」与「26 键底行」两组。
