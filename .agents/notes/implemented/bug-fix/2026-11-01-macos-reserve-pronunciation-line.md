# Agent Note: macOS 横排候选窗为读音预留一行

Status: implemented

## Problem

macOS 0.56.0 打开「显示读音」后，在贴着屏幕底边的输入框里打字，候选窗每按一个键都在光标下方和上方之间跳。录屏里能看出两种跳法：

- 同一个键先跳一次：`qa`、`qadb` 刚出现时候选窗在光标下方，首选下面只有释义 `clan, family; mister`；几帧之后音标 `/klæn/` 补上，窗口高一行，翻到光标上方。
- 连续几个键之间来回跳：`ad` 的首选「基」带读音（`Base /bers/`），窗口在上方；下一个键 `adq` 的首选「茂名」只有释义没有读音，窗口矮一行，又回到下方。

原因是横排的预留高度（`InputController.mm` 的 `glossHeightReservedForFont:`）只按目标语言数留释义行，不留读音行，这是 [macOS 候选读音单独一行](2026-10-16-macos-candidate-pronunciation-own-line.md) 当时有意的选择。读音由 `synchronizePronunciation` 在后台队列查本机表，查回来再 `renderCandidates`；横排的翻转判断（`MSIMECandidateOrigin` 传入的 `flipDecisionHeight` 横排是 0）只看当前高度。于是读音到达、或者换到一页有读音的候选时，卡片高出一行；光标下方的空间正好卡在两种高度之间时，这一行就决定了放上方还是下方。

那篇笔记否掉预留时的判断是「读音通常在下一帧就到，长高的那一下比常驻的空行代价小」。这个判断只考虑了卡片在原地长高，没考虑长高会让翻转判断换边：在屏幕底部打字时，用户看到的不是长高一行，而是整个窗口每键翻一次。

## Decision

**「显示读音」开着时，横排每种目标语言预留两行：释义一行、读音一行。** `glossHeightReservedForFont:` 在 `_glossPronunciation` 为真时把释义行数乘 2。支持的七种目标语言（英、法、日、西、俄、德、韩）都有读音来源（`MSIMEGlossLineLanguage`），所以不按语言区分。韩文汉字的 훈음 行不读，仍是一行。

这一行和预留释义行用同一个量法（`"X\n…"` 按释义字体量高），读音画在释义字体里、只是颜色淡一些（`CandidateChrome.h` 的 `readingColor`），两者行高相同。读音还没查回来、这一页没有读音、有读音，三种情况下横排卡片一样高，翻转判断也就每键一致。

竖排的高亮行预留（视图标了 `translation_reserved` 的那一行）走同一个函数，也跟着多留一行读音；竖排其他行不预留，和以前一样。

**预留看共享偏好，不看读音目标。** `_glossPronunciation` 在 `applySharedToolbarPreferences:` 里从共享偏好的 `candidate_pronunciation` 读出，和预留释义行所用的 `_glossTargetLanguages` 同一个来源、同一个时机。查读音和画读音仍用 `_pronunciationTargets`，但它不能定预留：它来自 `msime_client_translation_query`，没在组字时这个查询返回 `Null`（`translation_candidates()` 为空），而 `synchronizeCandidateServices` 在上屏那一键之后也会跑，于是每次上屏后它都被清成 nil；下一次组字的第一个键先画候选、后跑同步，按它预留的话这一键照样先矮一行、读音目标回来再长高，录屏里每次上屏后第一个键的那一跳就会留着。

## Alternatives considered

- **只改翻转判断：横排把读音行算进 `flipDecisionHeight`，但卡片仍按内容画** — 没有读音的页不多出空行，翻转判断也每键一致。但窗口翻到光标上方时是底边贴着光标，读音到达、卡片长高一行时往上长，首行和候选那一行整体上移一行；换到没有读音的页又下移回来。候选文字每键上下移一行，和这次要修的问题是同一类，只是幅度小一些。
- **横排也像竖排那样记住这次显示以来最高的一页（`TallestCandidateHeight` 对横排也累加）** — 改的是共享规则，Linux 的 X11 画布一起受益，也能防住读音以外的长高。但记忆在候选窗隐藏时清零，每次上屏后的第一个键照样先放下方、读音一到翻到上方，录屏里 `qa`、`qadb` 的那种跳法不会消失；而且它只管翻转，不管卡片在上方时的上下移动。
- **维持现状，不为读音预留** — 那篇笔记的理由：开关开着时多数页（中文短词、表里没有的词）没有读音，每页多一行空白。这个代价是真实的，但打开「显示读音」的用户本来就在看释义和读音，多一行空白换来窗口不翻，比每键翻一次好；释义行的预留当初也是同样的取舍（`reservedGlossHeightForFont:` 的注释）。

## Consequences

- **收益**：开着读音时，横排卡片高度不再随读音到达或换页变化，屏幕底部打字时候选窗固定在一边，不再每键上下翻。
- **代价**：开着读音时，横排每页多一行读音高度（12pt 释义字体约 15pt），没有读音的页这一行空着；两种目标语言时多两行。关着读音时完全不变。
- Windows 宿主同样不为读音预留（[Windows 候选读音单独一行](2026-11-01-windows-candidate-pronunciation-own-line.md) 的 `reserved_secondary_lines`），读音也是异步到的，同样的翻转在 Windows 上大概率也会出现；Linux 的原生画布也一样，`CandidateTranslationPolicy.h` 的 `translation_reserved_lines` 只数释义行，翻转用的是同一份 `CandidateOrigin`。这两个平台这次没有改。

## Verification

- `tests/candidate/CandidateRowLayoutTest.mm` 的 `TestPronunciationLine`：开着读音时，读音还没查回来的页、读音目标被上屏清掉时（上屏后的第一个键）的页、没有读音的页和有读音的页，候选格和卡片一样高；关掉读音回到原来的高度；其余断言（读音单独一行、格宽、原地重画换读音、竖排）不变。
- 用户录屏里的复现（贴近屏幕底边的输入框、五笔 86、英文释义加读音）在本机装包后核对，留给维护者。
