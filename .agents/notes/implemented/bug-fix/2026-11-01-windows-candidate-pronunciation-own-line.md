# Agent Note: Windows 候选读音单独一行

Status: implemented

## Problem

Windows 宿主打开「显示读音」后，`CandidateGlossReadings.h` 的 `candidate_gloss_display` 把读音用两个空格接在释义行尾（`input  /ˈɪnpʊt/`），和 macOS 在 [macOS 候选读音单独一行](2026-10-16-macos-candidate-pronunciation-own-line.md) 之前的版式一样，问题也一样：横排时格宽按「释义 + 读音」这条拼起来的长行量（`CandidateWindow::measure_items` 量的是 `candidate_secondary_text`），每格宽出一倍，读音的末尾紧挨下一格的释义，看上去压在相邻候选上；一页放不下、`candidate_single_line_columns` 把格子挤窄时，这一行又在音标中间折开。macOS 那次已经改成读音单独一行，Windows 留作后续。

## Decision

**读音单独一行。** `candidate_gloss_display` 把每行释义的读音放在它下面单独一行，和 macOS 的 `MSIMECandidateGlossDisplay` 一样；空的释义行不生读音行。读音范围照旧报告，候选窗照旧按释义色的 0.72 倍透明度画读音（选中底色不透明时不减淡）。

**排版不另加规则。** 读音多出来的一行就是释义那一段的又一行，走已有的多行释义路径：`candidate_secondary_lines` 数换行，交给 `CandidateItemWidths::translation_lines`；DirectWrite 量多行文字的宽度取最宽的一行，所以格宽是释义和读音里较宽的那一行。

- 横排：`candidate_page_layout` 让同一行的格子取最高一格的高度，一页里只要有一个候选有读音，每格都多出一行，候选和释义都在同一高度。没有读音的页、开关关着时高度和从前一样；横排预留的释义行（`reserved_secondary_lines`）按目标语言数算，不为读音预留。
- 竖排：多行释义按 Windows 已有的规则放在候选文字下面（`candidate_item_layout` 里 `translation_lines > 1` 的那一支，来自韩文汉字 훈음 加译文的两行释义），读音在释义下面。

**预选释义列按行起点找。** Tab 预选的释义列在高亮候选上画下划线，`candidate_gloss_column_range` 原来数第 N 个换行找第 N 列。读音单独占一行后，第一列有读音时第二列在第三行，数换行会把下划线画到读音上或者干脆找不到。现在 `candidate_gloss_display` 报出每行释义的起点，`CandidateSecondaryRun::translation_line_starts` 带上偏移，`candidate_gloss_column_range` 按它取。4096 字节的上限放不下读音、只画释义时，行起点跟着画出来的那份文字。

**截图工具带读音。** `tests/tools/candidate_skin_gallery` 的假候选页里前两个候选经 `attach_candidate_readings` 挂上读音，CI 的 `windows-skin-gallery` 产物里每款皮肤的横竖排截图都能看到读音单独一行、横排其他格子跟着留出这一行。

## Alternatives considered

- **竖排照搬 macOS：释义留在候选右边，读音在释义下面** — 竖排每行只多一行释义高，比放到候选文字下面矮一行。但 Windows 的规则是两行以上的释义一律放在候选文字下面（韩文 훈음 加译文、两种目标语言都是这样），只给读音开一条「两行也放右边」的例外，要在 `CandidateItemWidths` 里区分读音行和释义行，改自然宽度、单行最小宽度和右侧框的竖直对齐，几条规则一起变；读音和两种目标语言的释义在同一个宿主里排法不一样，也更难解释。这次只把读音挪到下一行，竖排的排法跟着 Windows 已有的多行规则走。
- **读音仍接在行尾，只把格宽量准或裁在格子里** — macOS 那次已经否掉：格宽本来就按整行量，问题出在版式；接在行尾每格都宽一倍，更容易被挤窄后在音标中间折开。
- **不改 `candidate_gloss_column_range`，改成在读音行前加别的分隔符** — 用换行以外的符号把读音和释义分在同一行，列号就还等于行号；但那就不是单独一行了，问题照旧。

## Consequences

- **收益**：横排有读音的格子宽度从「释义 + 读音」降到两者里较宽的一行，读音不再贴着相邻格的释义，挤窄时也不会在一条读音中间折开；和 macOS 的版式一致。预选释义列的下划线在有读音时仍然画在第二种目标语言那一行上。
- **代价**：横排有读音的页多一行释义高，同一行没有读音的格子也留着这一行；读音是异步到达的，到达时这一页会长高一行（和 macOS 一样不预留，理由见 macOS 那篇）。竖排有读音的行比从前高两行释义：释义和读音都在候选文字下面，和 macOS 竖排（释义在候选右侧）不同。
- **没有改的**：`CandidatePalette.h` 的 `candidate_row_translation_color` 在选中行仍画皮肤包的释义色，皮肤的选中底色和释义色同一色系时选中行的释义和读音对比度低，macOS 那篇用 `selectedTranslation` 修了，Windows 还没有。

## Verification

- `windows-candidate-gloss-readings`：读音在它那一行释义下面单独一行；两行都有读音时各自跟在下面；读音范围和每行释义的起点；空的释义行不生读音行。
- `windows-korean-hanja-presentation`：훈음、两种目标语言加读音时的次要文字、行数、读音范围和行起点；`candidate_gloss_column_range` 在第一列有读音时找到第二列、没有读音时第二列在第二行、超出列数和带 훈음 时不画；4096 字节上限按换行重新算。
- `windows-script-annotation-presentation`：插件注释排在读音那一行之后，起点和行数跟着变。
- `windows-candidate-skin-gallery-plan`：截图工具的假候选页前两个候选带读音，次要文字是释义加下一行读音。
- 截图：CI `windows-skin-gallery` 产物里横排、竖排、浅色、深色的基线和各款皮肤。真机（Windows 11）上打开「显示读音」核对横排和竖排，留给维护者。
