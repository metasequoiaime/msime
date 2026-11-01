# Agent Note: macOS 候选读音单独一行，选中行释义按底色换色

Status: implemented

## Problem

打开「显示读音」后，macOS 候选窗把读音接在释义同一行的后面（`Animation protagonist  /.ænɪˈmeɪʃən prəʊˈtægənɪst/`）。横排时这一行比候选本身宽一倍以上，整格跟着它变宽，读音的末尾和下一格的释义在同一条基线上、同一色系里，只隔一个序号的距离，用户看到的是读音压在相邻候选 `东华主题 / Donghua theme` 上。一页候选加起来比屏幕宽时，`SingleLineColumns` 把格子挤窄，这一行又在音标中间折开（`/.ænɪ` 换行 `ˈmeɪʃən/`）。

用本机的偏好（蓝色大肥鱼皮肤、18pt、每页 6 个）和产品渲染路径复现，格宽是按画出来的整行量的（`InputController.mm` 的 `candidatePageGeometry` 把读音拼进释义再交给 `MSIMECandidateItemWidths`），按钮又在 `drawRect:` 里按自身边界裁剪、释义在自己的框里折行，所以没有复现出字形真正画进相邻格子；问题出在版式本身：读音和释义挤在一行，格宽、折行位置都由这条拼起来的长行决定。

同一段代码还有两个缺陷：

- 只换了释义的视图走原地重画（`renderCandidates` 里 `MSIMEOnlyCandidateTranslationsChanged` 那一支），按钮的 `translation`、排版都换成新的，`pronunciation` 却留着旧值，会把上一条释义的读音画在新释义后面。
- 皮肤包给了释义色（`translation`）时，选中行也画这个颜色。皮肤的选中底色不透明、又和释义色同一色系时（`selected = "#4D6BFE"`、`translation = "#2F5BD3"`），选中行的释义和读音是蓝底蓝字，对比度约 1.4:1，读不出来。选中文字和选中序号早就按底色推导（`DeriveSelectedForegrounds`），释义没有对应的选中色。

## Decision

**读音单独一行。** `CandidatePronunciation.h` 的 `MSIMECandidateGlossDisplay` 把每条释义行的读音放在它下面单独一行，原来是两个空格接在行尾。读音范围和释义行起点照旧报告，读音的淡色和按列上屏的下划线不受影响。排版不另加规则，复用已有的多行释义路径：

- 横排：释义框的宽度是释义和读音里较宽的一行，高度多一行；同一行的格子取最高一格的高度（`LayoutCandidatePage` 的 `closeLine`），所以这一页只要有一个候选有读音，每格都留出第三行，候选和释义都在同一高度，选中底色盖住整格。这一页没有读音、或者开关关着时，高度和从前一样；横排为还没到的释义预留的高度（`reservedGlossHeightForFont:`）不为读音预留。
- 竖排：释义仍在候选同一行，读音在释义下面，两行释义框比候选行高时行跟着长高，和两种目标语言各占一行的释义同一规则。
- 展开的候选网格用同一个按钮和同一份页面排版，跟着变。`CandidatePanel.mm` 和 `CandidateSkinPreviewView.mm` 不画释义和读音，不涉及。

**原地重画时读音跟着释义重算。** 原地重画那一支和整页重建一样用 `pronunciationForCandidate:` 给按钮设 `pronunciation`。

**选中行的释义色。** `SkinTokens` 新增 `selectedTranslation`，只在有 `translation` 时才有，由 `DeriveSelectedForegrounds` 推导：选中底色不透明（alpha ≥ 0.85，和 `ContrastingText` 同一门槛）时按底色挑白或近黑，不透明度 0.82，和选中序号一样，这也是设计稿里选中行释义的颜色（dc.html L1533 `candSelTr`）；底色半透明时就是 `translation` 本身。共享解析结果里将来出现 `selected_translation` 时 `SkinManifestBridge.mm` 照它画，不再推导；目前共享层只在清单里认这个键，不放进解析结果。`refreshCandidateSkin` 给选中行用它。选中底色不透明时，`MSIMECandidateButton` 画读音不再在释义色上乘 0.72：释义色已经是按底色挑出的次要色，再减淡在中等亮度的底色上会掉到 3:1 以下；读音单独一行，靠位置就能和释义分开。工具栏的 `ToolbarSkinTokens` 和清掉 `translation` 一样清掉它。

## Alternatives considered

- **读音仍接在行尾，只把格宽量准或在格子边界裁掉** — 改动最小。但复现下格宽本来就是按整行量的、绘制本来就裁在格子里，问题不在测量；行尾接读音让每一格都宽出一倍，页面更容易超出屏幕被挤窄，挤窄后又在音标中间折开，裁剪只会把读音截掉一半。
- **横排把读音预留进卡片高度，开关打开就留第三行** — 读音是异步到达的，预留能避免读音到达时卡片长高一下。但开关打开后大多数页（中文短词、没有英文表的词）根本没有读音，每页都多出一行空白；读音查的是本机表，通常在同一次按键的下一帧就到，长高的那一下比常驻的空行代价小。
- **选中释义色取 0.72 不透明度，比选中序号更淡** — 能把释义和候选文字分得更开。但在 `#4D6BFE` 上白色 0.72 的对比度只有约 3.0:1，读音若再减淡只剩约 2.2:1；0.82 与选中序号一致，也是设计稿给选中行释义的颜色。
- **选中行的释义直接用选中序号色（`selectedNumber`）** — 不用新字段。但选中序号色在皮肤包写了 `selected_number` 时是包的取值，和释义色无关；半透明底色上它退回序号色，而那时应该保留包自己的释义色。

## Consequences

- **收益**：横排有读音的格子宽度从「释义 + 读音」降到两者中较宽的一行（复现页从 843pt 收到 794pt，其中第 2 格从 318pt 收到 187pt），读音不再贴着相邻格的释义，挤窄时也不会在一条读音中间折到释义那一行；原地重画不再留下旧读音；释义色和选中底色同色系的皮肤，选中行的释义和读音按底色换成白或近黑。
- 「横排不为读音预留」这一条后来翻转了，见 [macOS 横排候选窗为读音预留一行](2026-11-01-macos-reserve-pronunciation-line.md)：读音到达时长高的那一行会让贴近屏幕底边的候选窗在光标上下来回翻。
- **代价**：有读音的横排页多一行高（12pt 的释义行加行距，约 15pt）；同一页里没有读音的格子也留着这一行。竖排有读音的行在 18pt 候选字号下正好放得下两行，释义的第一行不再和候选文字居中对齐。选中行的读音在不透明底色上和释义同色，只靠位置区分。
- Windows 宿主的读音已经同样改成单独一行，见 [Windows 候选读音单独一行](2026-11-01-windows-candidate-pronunciation-own-line.md)（竖排和这里一样，释义加读音两行留在候选右侧）；选中行的释义色见 [Windows 选中行释义跟随选中文字换色](2026-11-01-windows-selected-gloss-contrast.md)：从选中文字推导，皮肤没写选中文字时和这里得到同一个颜色，另外在选中文字够的底色上把不透明度往上加到对底色 4.5:1，默认皮肤也按这条调不透明度，这几点和 macOS 不同。[候选窗不再画整句候选的逐词拆解](../simplification/2026-10-16-drop-candidate-gloss-breakdown-line.md) 之后释义区只剩释义和读音，这次是在那之上把读音挪到下一行。

## Verification

- `tests/candidate/CandidatePronunciationTest.mm`：读音在释义下一行，读音范围和行起点不变，空目标行不生读音行。
- `tests/candidate/CandidateRowLayoutTest.mm` 的 `TestPronunciationLine`：经产品渲染路径，读音单独一行、释义行位置不变；释义框宽度等于释义和读音中较宽的一行、比拼在一起的宽度窄；两格不重叠、画出的框在格子内；同一行格子等高、释义同高；没有读音的页和开关关掉时高度不变；只换释义的原地重画（同一个按钮对象）换上新读音；竖排读音在释义下面。`TestSelectedGlossColour`：选中底色 `#4D6BFE` 配释义 `#2F5BD3`、`#23262F` 配 `#6B7180` 时选中行释义色和底色的对比度至少 3:1，未选中行仍是包的释义色；半透明底色 `#2F5BD329` 时选中行保留包的释义色。
- `tests/candidate/CandidateSkinTest.cpp`：`DeriveSelectedForegrounds` 在三种不透明底色上给出 0.82 不透明度、对比度至少 3:1 的选中释义色；半透明底色时等于释义色；没有释义色时没有它；指定了时不推导。
- `tests/input/ShortcutTest.mm`：带释义色的皮肤里选中行取 `selectedTranslation`，其他行取包的释义色。
- 本机截图核对（临时探针，未提交）：`动画主体 / Animation protagonist / /.ænɪˈmeɪʃən prəʊˈtægənɪst/` 和相邻的 `东华主题 / Donghua theme` 在默认皮肤、蓝色大肥鱼皮肤、横排和竖排下，读音都在释义下面单独一行、不压到相邻格；`#4D6BFE` 和 `#23262F` 选中底色上选中行的释义和读音清楚可读。
