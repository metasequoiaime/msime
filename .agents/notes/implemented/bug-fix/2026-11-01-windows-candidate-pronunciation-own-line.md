# Agent Note: Windows 候选读音单独一行

Status: implemented

## Problem

Windows 宿主打开「显示读音」后，`CandidateGlossReadings.h` 的 `candidate_gloss_display` 把读音用两个空格接在释义行尾（`input  /ˈɪnpʊt/`），和 macOS 在 [macOS 候选读音单独一行](2026-10-16-macos-candidate-pronunciation-own-line.md) 之前的版式一样，问题也一样：横排时格宽按「释义 + 读音」这条拼起来的长行量（`CandidateWindow::measure_items` 量的是 `candidate_secondary_text`），每格宽出一倍，读音的末尾紧挨下一格的释义，看上去压在相邻候选上；一页放不下、`candidate_single_line_columns` 把格子挤窄时，这一行又在音标中间折开。macOS 那次已经改成读音单独一行，Windows 留作后续。

## Decision

**读音单独一行。** `candidate_gloss_display` 把每行释义的读音放在它下面单独一行，和 macOS 的 `MSIMECandidateGlossDisplay` 一样；空的释义行不生读音行。读音范围照旧报告，候选窗照旧按释义色的 0.72 倍透明度画读音（选中底色不透明时不减淡）。

**排版走已有的多行释义路径，竖排加一条例外。** 读音多出来的一行就是释义那一段的又一行：`candidate_secondary_lines` 数换行，交给 `CandidateItemWidths::translation_lines`；DirectWrite 量多行文字的宽度取最宽的一行，所以格宽是释义和读音里较宽的那一行。`CandidateItemWidths::reading_lines` 另外记下其中几行是读音（`measure_items` 取 `candidate_secondary_run` 的 `readings` 个数），只用来认出竖排里的那条例外。

- 横排：`candidate_page_layout` 让同一行的格子取最高一格的高度，一页里只要有一个候选有读音，每格都多出一行，候选和释义都在同一高度。没有读音的页、开关关着时高度和从前一样；横排预留的释义行（`reserved_secondary_lines`）按目标语言数算，不为读音预留。
- 竖排：和 macOS 一样，一行释义加它的读音这两行留在候选文字右侧，读音在释义下面（`candidate_translation_beside`：`translation_lines` 为 1，或为 2 且 `reading_lines` 为 1）。右侧框的高度是量出的两行高和候选行高里较大的一个，文字在框里竖直居中；默认 16 DIP 候选字号下两行 12 DIP 的释义（约 32 DIP）放得进一行候选（33.6 DIP），行不变高，小字号或回退字体更高时这一行跟着长高。行宽是候选、间隔和两行里较宽的一行（`candidate_item_natural_width`）；右边放不下时照旧放到候选文字下面。其他两行以上的释义（韩文汉字 훈음 加译文、两种目标语言、插件注释，以及它们再加读音）仍按 Windows 已有的规则放在候选文字下面。

**预选释义列按行起点找。** Tab 预选的释义列在高亮候选上画下划线，`candidate_gloss_column_range` 原来数第 N 个换行找第 N 列。读音单独占一行后，第一列有读音时第二列在第三行，数换行会把下划线画到读音上或者干脆找不到。现在 `candidate_gloss_display` 报出每行释义的起点，`CandidateSecondaryRun::translation_line_starts` 带上偏移，`candidate_gloss_column_range` 按它取。4096 字节的上限放不下读音、只画释义时，行起点跟着画出来的那份文字。

**截图工具带读音。** `tests/tools/candidate_skin_gallery` 的假候选页里前两个候选经 `attach_candidate_readings` 挂上读音，CI 的 `windows-skin-gallery` 产物里每款皮肤的横竖排截图都能看到读音单独一行、横排其他格子跟着留出这一行。

## Alternatives considered

- **竖排也按 Windows 已有的多行规则，释义和读音都放到候选文字下面** — 不用新字段，排法和韩文 훈음 加译文、两种目标语言一样。但竖排每个有读音的候选从一行候选高变成一行候选加两行释义高，9 个都带英文释义的一页高出一倍多；读音挤到相邻格只是横排的问题，竖排付了高度却换不来什么，和 macOS 的差距也是两行而不是一行。这个 PR 的第一版这样做过，评审后改掉。
- **只改横排，竖排保留「释义  /读音/」接在行尾** — 竖排完全不变。但同一个宿主按排列方向给出两种读音版式，`candidate_secondary_run`、`candidate_gloss_column_range` 和测量都要带上方向；行尾的读音在竖排里放不下时照样会整条挪到下面、在音标中间折开，和 macOS 也不一致。
- **两种目标语言的释义在竖排也留在右边，和 macOS 完全一样** — macOS 竖排的两行释义都在右边。但这改的是 Windows 已有的、和读音无关的排法（韩文 훈음 加译文也是两行），不在这次范围里。
- **读音仍接在行尾，只把格宽量准或裁在格子里** — macOS 那次已经否掉：格宽本来就按整行量，问题出在版式；接在行尾每格都宽一倍，更容易被挤窄后在音标中间折开。
- **不改 `candidate_gloss_column_range`，改成在读音行前加别的分隔符** — 用换行以外的符号把读音和释义分在同一行，列号就还等于行号；但那就不是单独一行了，问题照旧。

## Consequences

- **收益**：横排有读音的格子宽度从「释义 + 读音」降到两者里较宽的一行，读音不再贴着相邻格的释义，挤窄时也不会在一条读音中间折开；和 macOS 的版式一致。预选释义列的下划线在有读音时仍然画在第二种目标语言那一行上。
- **代价**：横排有读音的页多一行释义高，同一行没有读音的格子也留着这一行；读音是异步到达的，到达时这一页会长高一行（和 macOS 一样不预留，理由见 macOS 那篇）。竖排在默认字号下行高不变，释义的第一行不再和候选文字居中对齐（和 macOS 一样）；小字号时有读音的行略高一点。竖排里「一行释义加读音」留在右边，而两种目标语言的两行释义在下面，同一个宿主里两种两行的排法不同，靠 `reading_lines` 区分。
- 选中行的释义色后来补上了，见 [Windows 选中行释义跟随选中文字换色](2026-11-01-windows-selected-gloss-contrast.md)：`candidate_row_translation_color` 在不透明选中底色上不再画皮肤包的释义色。

## Verification

- `windows-candidate-card-size`：横排一页只有一个候选有读音时每格等高、多出这一行，有读音的格宽是两行里较宽的一行，没有读音的页高度不变；竖排一行释义加读音留在候选右侧、行高和卡片高度不变，两行比候选行高时只有这一行长高，右侧放不下时放到下面；훈음 加译文、훈음 加译文加读音、两种目标语言加读音仍放在下面。
- `windows-candidate-gloss-readings`：读音在它那一行释义下面单独一行；两行都有读音时各自跟在下面；读音范围和每行释义的起点；空的释义行不生读音行。
- `windows-korean-hanja-presentation`：훈음、两种目标语言加读音时的次要文字、行数、读音范围和行起点；`candidate_gloss_column_range` 在第一列有读音时找到第二列、没有读音时第二列在第二行、超出列数和带 훈음 时不画；4096 字节上限按换行重新算。
- `windows-script-annotation-presentation`：插件注释排在读音那一行之后，起点和行数跟着变。
- `windows-session`（`tests/ui/candidate_mailbox.cpp`）：读音挂到候选上后，次要文字是释义加下一行读音；这个测试要 `windows.h`，只在 Windows 构建里编。
- `windows-candidate-skin-gallery-plan`：截图工具的假候选页前两个候选带读音，次要文字是释义加下一行读音。
- 截图：CI `windows-skin-gallery` 产物里横排、竖排、浅色、深色的基线和各款皮肤。真机（Windows 11）上打开「显示读音」核对横排和竖排，留给维护者。
