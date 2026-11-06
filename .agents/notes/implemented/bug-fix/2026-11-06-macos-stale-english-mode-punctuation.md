# Agent Note: macOS 切到英文时不再把英文标点记成应用的手动切换

Status: implemented

## Problem

#10954 报告设置里勾了「中文标点」，在 VS Code 终端和网页对话框里打出的标点却是英文。排查中找到一条与症状一致的缺陷：`InputController.mm` 的 `setEnglishInputMode:showHUD:` 在切到英文时执行 `_appearance.runtimeChinesePunctuation = [punctuationLock isEqual:@"chinese"]`，等于给当前应用写下一个显式的按应用标点覆盖 NO。这个覆盖只有在同一个应用里再走一次 `setEnglishInputMode:NO` 才会清掉。模式经由别的途径回到中文时它一直留着，于是中文模式配英文标点：

- `ime_mode_scope=global` 下在应用 A 切英文，到应用 B 切回中文（只清了 B），回到 A；
- 英文模式下按 Ctrl+Shift+E 进英文候选模式再退出（`setDedicatedEnglishInputMode:` 直接写 `englishMode = NO`）；
- 应用例外规则为中文的应用，在里面切英文后离开再回来，规则让模式回到中文。

报告人具体走的是哪条路径还没确认，也可能只是智能标点在字母、数字后保留英文（预期行为）。

## Decision

中英切换在两个方向上都只调用 `resetRuntimePunctuationForActiveApplication`，不再写入显式值。[2026-10-22 的笔记](2026-10-22-macos-initial-english-punctuation.md) 已经让 `runtimeChinesePunctuation` 在没有覆盖时按锁定和模式推导：锁定中文返回 YES，锁定英文返回 NO，跟随时返回 `!englishMode && chinesePunctuation`。切到英文后读到的值和原来写进去的完全相同，区别只是它不再是一个会残留的状态。

由此确立一条不变量：按应用的运行态标点覆盖只代表用户手动按 Ctrl+.、工具栏或菜单做的切换，模式本身决定的标点一律从模式推导。英文模式里手动打开的中文标点仍按应用保留，直到下一次中英切换。

英文候选模式（Ctrl+Shift+E）的 `englishMode` 为 NO，标点按中文模式推导。从中文模式进入时本来就是这样，现在从英文模式进入也一样。

## Alternatives considered

- 像 Windows 那样在每次激活时按模式重新同步标点（`SyncPunctuationWithImeMode` 在 `InitializeMetasequoiaIMECompartment` 里调用）。能一次覆盖所有路径，但会清掉用户在同一个应用里手动选的标点，macOS 的按应用切换就没有意义了；2026-10-22 的笔记已经因为这个原因否掉过它。
- 在每条把模式改回中文的路径上（焦点切换后的全局模式、英文候选模式、应用规则）各补一次 reset。要逐个找全，以后再加一条路径又会漏；问题的根子是写进了不该写的状态，修写入点更彻底。

## Consequences

- 上面三条路径回到中文时都是中文标点；`TestPerApplicationPunctuationAndWidth`（`ctest -R '^shortcut-english-punctuation$'`）覆盖全局作用域跨应用切回和英文候选模式进出，修复前在这两段断言失败。
- 英文模式下改了标点锁定会立即生效：原来锁定中文时进英文写下的 YES 会在锁定改回跟随后残留，现在按模式推导为英文标点。
- 只改 macOS。Windows 每次激活都会重新同步，不受影响；Linux 的 `resync_punctuation_for_mode` 在中英切换和全局模式同步（`sync_global_input_mode`）时都按模式重新推导，也没有这种残留。
