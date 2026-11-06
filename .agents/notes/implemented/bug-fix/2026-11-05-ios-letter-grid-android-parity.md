# Agent Note: iOS 14/18 键对齐 Android：拖空格先上屏、输入页补全拼 18 键、长按字母跟随全角、预览画「符」

Status: implemented

## Problem

一次对照 Android 的 14/18 键审计在 iOS 上找出四处不一致，逐条在当前代码上复核过：

1. **拖动空格把引擎光标挪进看不见的组码。** 全拼、双拼的 14 键和 18 键打 nihao（BN UI GH AS OP）后拖空格，空格写「移动拼音光标」，引擎光标在 `editing_text` 里走。14/18 键的 `editing_text` 是各组的代表字母，读音行平时显示引擎给的读音（`ni'hao`），光标一离开末尾，读音行就换成 `bug|ao` 这样的组码。Android 拖空格先发 `MSIME_COMMIT_RAW`（网格下就是结束组字、上屏首选），再移动输入框光标；iOS 只在五笔 14 键（`typesWubiGrid`）这样做。
2. **「输入」页的全拼一行没有全拼 18 键。** #9643 把 `.eighteenKey` 加进全拼的方案，#9653 按方案拆分输入设置时把它漏了。键盘在 18 键时，`InputLanguage.of(.eighteenKey)` 靠兜底才落到全拼，全拼一行的面板里没有 18 键这一项，勾打在别的方案上，所有行都不标「默认」；移除全拼时 `.eighteenKey` 也不跟着停用，键盘留在 18 键上不回落。
3. **长按出来的字母和数字永远是半角。** 14/18 键长按选字母、九键长按或滑出的数字都经 `commitNineKeyHoldOption` 调 `insertOwnText` 原样上屏，不看全角开关。Android（`s.fullWidthOutput(text)`）和 HarmonyOS 都跟着开关走。
4. **设置里的键盘预览把拼音 14/18 键第三排左键画成「分词」。** 键盘空闲时这个键是「符」，组字时才换成「分词」（`updateFourteenKeySeparator`）。Android 和 HarmonyOS 的预览画空闲时的「符」，iOS 的 `KeyboardPreviewFace.gridSplitsSyllables` 却按「不是五笔」画「分词」，18 键的两处预览干脆写死「分词」。

## Decision

- **字母网格一律先结束组字再移文档光标。** `handleSpaceCursorMovement` 开始拖动时，`spaceDragEditsComposition` 的条件从 `!typesWubiGrid` 改为 `!typesLetterGrid`：全拼、双拼、五笔的 14 键和 18 键都先 `finishComposition()` 上屏首选，空格写「移动光标」，之后的步进交给 `moveCursor` 移文档光标。26 键和九键照旧在拼写里走光标。按音节走的那一项（`bySegment`）原来另外排除了字母网格，现在字母网格根本进不到这一支，那个条件删掉。
- **拼写编辑条的光标不在本次范围。** `render` 里的 `spellingCaret` 没有排除字母网格：14/18 键的 `editableSpelling` 仍然给出，#10894 的放大拼写编辑条要用它按组码换算读音位置。本次只改空格拖动这一个入口，不碰编辑条。
- **全拼一行的方案与 Android 的 `Language.QUANPIN` 同序**：`[.quanpin, .fourteenKey, .eighteenKey, .nineKey]`。行上标「默认」、面板打勾和移除都按 `InputLanguage.schemes` 走，补上这一项后三处一起恢复。
- **长按和滑出的上屏走 `insertDirectText`。** 全角开关打开时写全角（`w` 写成 U+FF57，`5` 写成 U+FF15）；韩语、越南语和藏文照 `insertDirectText` 已有的规则始终半角。
- **预览一律画空闲时的「符」。** `KeyboardPreviewFace.gridSplitsSyllables` 删掉，`KeyboardSkinPreview` 和「按键布局」页的 `KeyLayoutKeyboardPreview` 在 14 键、18 键第三排左端都画「符」。

## Alternatives considered

- **拖空格时保留组字，只把光标限制在音节边界上走**：用户在 14 键上也能改中间某个音节。不用的原因是光标停在组码中间时读音行仍然只能画组码，iOS 没有把组码位置换算成读音位置的现成办法（这正是 #10894 编辑条单独做换算的原因）；Android 也没有这样做，两端会不一致。
- **把 `spellingCaret` 一起对字母网格关掉**：读音行就再也画不出组码光标。不用的原因是 #10894 的编辑条依赖 `editableSpelling`，关掉会让它在 14/18 键上打不开；空格拖动改完以后，平常打字的路径已经不会把光标移进组码。
- **保留 `gridSplitsSyllables`，改成恒为 `false`**：改动最少。不用的原因是一个永远为假的字段只会让后来的人以为预览还区分拼音和五笔，删掉更清楚。

## Consequences

- 收益：四处行为与 Android、HarmonyOS 一致。14/18 键拖空格不会再露出组码；键盘在 18 键时「输入」页能看到、能选、能移除它；全角用户长按得到的字母和数字与点按一致；设置预览与空闲键盘对得上。
- 代价：14/18 键组字中拖空格不能再回到拼写中间改字，只能先上屏首选。这和 Android 相同，五笔 14 键原来就是这样。

## Testing

- `FourteenKeyKeyboardTests.testDraggingSpaceFinishesTheGridComposition`：经生产入口 `handleSpacePan:` 拖空格，断言空格写「移动光标」，组字结束、首选上屏，读音行没有出现组码，输入框光标收到步进。
- `KeyboardExtensionEditorUITests.testDraggingSpaceOnFourteenKeysCommitsBeforeMovingTheCaret`：iOS 27 模拟器上由系统加载的扩展里打 BN UI GH AS OP 再左拖空格，输入框里有了首选。改之前同一步读音行停在 `bu|gao`，输入框是空的。
- `FourteenKeyKeyboardTests.testHeldLettersFollowTheFullWidthSwitch` 和 `NineKeySwipeDigitsTests.testSwipedDigitsFollowTheFullWidthSwitch`：全角开着时长按 QW 选 w 写入 U+FF57，九键滑出 5 写入 U+FF15，长按 3 选 e 写入 U+FF45。
- `InputLanguageTests` 与 `scripts/test-ios-input-language.py`：`InputLanguage.of(.eighteenKey) == .quanpin`，全拼的方案顺序与 Android 相同，每个触屏方案恰好属于一门语言，键盘在 18 键时全拼一行代表的就是 18 键。
- `OnboardingUITests.testQuanpinRowOffersEighteenKeyAndRemovesIt`：在「输入」页选全拼 18 键，全拼一行标「默认」、面板里 18 键打勾；移除全拼后双拼一行成为默认。
- 以上新用例都先在改之前的代码上跑过、确认失败，再在改之后跑通。改前改后的截图见 PR。

相关笔记：[iOS 18 键](../feature/2026-10-25-ios-eighteen-key.md)、[五笔 14 键](../feature/2026-10-25-wubi-fourteen-key.md)、[按键布局页重做](../feature/2026-10-29-key-layout-page-redesign.md)。
