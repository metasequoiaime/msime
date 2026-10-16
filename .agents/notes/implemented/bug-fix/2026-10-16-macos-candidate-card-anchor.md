# Agent Note: macOS 带装饰的皮肤，候选卡片贴着光标摆放

Status: implemented

## Problem

皮肤包写了 `[candidate_window.decoration]`（比如 `top_inset_dip = 104`，一张人物图站在卡片上面）时，macOS 候选卡片出现在内嵌预编辑下方差不多一整条装饰带的位置，组字的文字和卡片之间空出一大块。

原因在 `platforms/macos/src/candidate/CandidatePlacement.h` 的 `MSIMECandidateOrigin`：它把窗口顶边放在光标下方 4pt（`y = NSMinY(caret) - size.height - 4`）。可是窗口顶上还有一条透明带，高度等于装饰高度（`InputController.mm` 排版时算出 `decorationHeight`，写进 `MSIMECandidateChromeView.cardTopInset`，卡片的底色、背景图和描边只画在带下面），卡片本身就从窗口顶边往下再退一条带才开始。翻不翻到光标上方也按含带的整个窗口高度判断，光标下方明明放得下卡片也会翻上去。旧的 `CandidatePanel.mm`（只有 `candidate-panel-test` 还编它）抄了一份同样的算法。

## Decision

- `MSIMECandidateOrigin` 多一个参数 `cardTopInset`，默认 0。`size` 和 `flipDecisionHeight` 照旧是含带的窗口高度，函数里先把带减掉。
  - **放在光标下方**：卡片顶边在光标下方 4pt，也就是窗口顶边在 `NSMinY(caret) - 4 + cardTopInset`。透明带朝上伸，盖在光标这一行和上面几行上。
  - **翻到光标上方**：窗口底边就是卡片底边，仍是 `NSMaxY(caret) + 4`。带在最上面，本来就没有空隙。
  - **翻不翻只看卡片**：`MAX(size.height, flipDecisionHeight) - cardTopInset` 放得下就留在下方。透明带朝上伸，不占光标下方的地方。竖排的翻转记忆（`MSIMETallestCandidateHeight`）照旧记含带的窗口高度，同一个皮肤的带高不变，判断时减掉同一条带即可，记忆和夹取的行为不变。
  - **夹进可见区域**：最后仍把整个窗口（连同透明带）夹进 `visibleFrame`。光标贴近屏幕上边、带伸出上边时，窗口被往下压，卡片和光标之间留出的距离等于 4pt 加上伸出去的那一段，至多一条带，比以前整条带的空隙小。
  - **左右**：水平方向的夹取不变。
  - `cardTopInset` 为 0 或负数时，结果和以前逐点相同，没有装饰的皮肤摆放不变。
- `InputController.mm` 的三处摆放（展开网格、只改释义的原地重绘、整页重建）都传 `[self candidateCardTopInset]`。这个值从面板当前的内容视图读，和 `_panel.frame.size` 来源相同：`refreshCandidateSkin` 发现明暗换了包时会整页重排，局部变量 `decorationHeight` 可能已经不是面板上那一张的值。
- 旧的 `CandidatePanel.mm` 不再自己算，改调 `MSIMECandidateOrigin`，把它在卡片顶上留的 `decorationTopDip` 当作带高传进去。

## 点击穿透

透明带不会挡住下面应用的点击，不用改 `hitTest:`。候选面板 `_panel.opaque = NO`、`backgroundColor = clearColor`，`MSIMECandidateChromeView` 是图层宿主、`isOpaque` 为 NO，带里什么都不画。WindowServer 判定鼠标落在哪个窗口时看窗口像素的透明度，完全透明的地方直接落到后面的窗口，事件根本不会进候选面板，`hitTest:` 也就轮不到。

本机用同样配置的面板实测过：无边框非激活 `NSPanel`，不透明设为 NO，背景透明，有阴影，内容视图是图层宿主、顶上留 150pt 透明带，带里放一张不透明的 `NSImageView`。`+[NSWindow windowNumberAtPoint:belowWindowWithWindowNumber:]` 按文档返回「鼠标按下会落到的最前面的窗口」，结果如下：

- 带里透明的点：返回后面的窗口。
- 卡片上的点：返回面板。
- 装饰图上的点：返回面板。

滚轮事件按同样的规则分发，所以带里的滚轮也落到后面的应用。卡片上的悬停（候选按钮各自的跟踪区域）和滚轮翻页（`MSIMECandidatePanel scrollWheel:`）不受影响。

装饰图不透明的像素仍会接住点击和滚轮。点在图上不会落到下面的文字，在图上滚轮会翻候选页。这和改动前一样，只是图现在可能盖在组字这一行上。在 `hitTest:` 里返回 nil 改变不了这一点：WindowServer 已经把事件交给了这个窗口，返回 nil 只是没有视图处理，并不会转给后面的窗口。

## Alternatives considered

- **带也算进光标和卡片之间的距离，只改翻转判断。** 装饰图永远不盖住光标所在的行，但用户报告的空隙原样保留，这正是要修的问题。
- **贴近屏幕上边时让带伸出可见区域，卡片照样贴着光标。** 卡片位置始终一致。但装饰图会被菜单栏或屏幕边缘切掉一截，而人物图的头部通常就在最上面；被压下来的空隙只出现在光标离屏幕上边不到一条带的时候，最多一条带。两害相权，取不裁图。
- **翻转判断仍按整个窗口高度。** 改动更小。但窗口下方放得下卡片、放不下整个窗口时会翻上去，翻上去以后带在顶上、卡片底边贴着光标上方，也算贴着，可是同一个光标位置下有装饰和没装饰的皮肤会在不同的一侧，而带朝上伸本来就不占光标下方的地方。
- **在 `MSIMECandidateChromeView` 里重写 `hitTest:`，让带返回 nil。** 任务里列出过这个方案，前提是带会截住点击。实测带本身就是穿透的，这个改动没有作用；装饰图上的点击也拦不回去，理由见上。

## Consequences

- **收益**：带装饰的皮肤，卡片贴着组字的文字出现，和没有装饰的皮肤一样。光标下方放得下卡片就不翻上去。
- **代价**：放在光标下方时，装饰图不透明的部分盖在光标这一行和上面几行上。卡片左边对齐组字的起点（`attributesForCharacterIndex:0`），装饰图默认靠卡片右边，组字很长或装饰图靠左对齐时，会挡住一部分正在打的字，点在图上也点不到下面的文字。光标贴近屏幕上边时，卡片和光标之间仍会留出一段，至多一条带。
- **双拼键位图**：`updateKeymapPanel` 让开候选窗时本来就只按卡片高度算（不含装饰带），现在卡片贴着光标，这个让开的距离和实际卡片一致了，不用改。
- **Windows 后续**：Windows 有同样的空隙，这次没有改。`platforms/windows/src/candidate/CandidateWindow.cpp` 把 `decoration_top_` 加进 `content_height`，`CandidateShadow.h` 的 `candidate_shadow_bounds` 再按含装饰的内容高度交给 `candidate_card_placement`，贴着光标的是装饰带的顶边；翻转判断（`decision_height`、`tallest_`）也含装饰。修法相同：放在光标下方时让内容顶边上移一条带，翻转判断减掉带。
- **验证**：
  - `candidate-placement`（`tests/candidate/CandidatePlacementTest.mm`）覆盖以下情形，新断言在改动前的实现上失败：
    - 带装饰时卡片顶边在光标下方 4pt，窗口顶边高过光标。
    - 带高 0 或负数时，在各个光标位置、两块屏幕、有无翻转记忆下都与旧结果逐点相同。
    - 光标下方放不下卡片时翻到上方，窗口底边贴着光标上方 4pt。
    - 放得下卡片、放不下整个窗口时留在下方，以前会翻上去。
    - 贴近屏幕上边时整个窗口夹回可见区域，留出的空隙等于 4pt 加伸出的部分。
    - 左右边缘和负坐标副屏的夹取不变。
    - 屏幕下边翻上去后仍在可见区域里。
    - 竖排翻转记忆含带时按卡片高度判断。
  - `shortcut` 的 `TestExternalSkin` 在实际控制器上，用 48pt 装饰带的皮肤包检查横排、竖排、浅色、深色四种组合：卡片顶边在光标下方 4pt，窗口顶边高过光标。
  - `candidate-panel` 检查旧面板改调共享函数后的翻转和夹取。
  - 点击穿透用上一节的独立程序在本机验证，没有写成自动测试：`shortcut` 里的面板不上屏（`HiddenCandidatePanel`），WindowServer 的判定要真窗口在屏幕上。
  - 装好的输入法里配真实装饰皮肤的观感还没有看过。
