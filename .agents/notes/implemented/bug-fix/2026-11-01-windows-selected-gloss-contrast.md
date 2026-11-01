# Agent Note: Windows 选中行释义跟随选中文字换色

Status: implemented

## Problem

Windows 候选窗选中行的释义和读音对选中底色的对比度太低，[Windows 候选读音单独一行](2026-11-01-windows-candidate-pronunciation-own-line.md)（#10461）把它记成后续项。`CandidatePalette.h` 的 `candidate_row_translation_color` 在选中行只认皮肤写的 `selected_translation`，没写时画皮肤包的释义色，再没有时画选中序号色 `selected_number`：

- 皮肤包的释义色是给卡片底色配的。选中底色不透明又和释义色同一色系时（`selected = "#4D6BFE"`、`translation = "#2F5BD3"`），选中行是蓝底蓝字，约 1.4:1。macOS 已经用 `SkinTokens::selectedTranslation` 修过（[macOS 候选读音单独一行，选中行释义按底色换色](2026-10-16-macos-candidate-pronunciation-own-line.md)），Windows 没有。
- 默认皮肤也偏低：选中序号色是白色 0.82，在浅色的 `#2C7A4B` 上约 4.1:1，在深色的 `#5FBF84` 上约 2.0:1，CI 的 `windows-candidate-skin-gallery` 截图 `no-skin__horizontal-dark.png` 里选中行的 `input /ˈɪnpʊt/` 很淡。读音在不透明选中底色上不减淡，和释义同色，所以同样淡。

## Decision

选中行的释义色（读音同色）按下面的顺序取，规则都在 `candidate_row_translation_color`，绘制处 `CandidateWindow.cpp` 不变：

1. 皮肤写了 `selected_translation`（`apply_candidate_skin_extra_slots` 读进来的），照它画，不推导，和 macOS 相同。
2. 选中底色不透明（alpha ≥ 0.85，和 `candidate_selected_foreground`、macOS `ContrastingText` 同一门槛）时：
   - 有皮肤包的释义色：换成从选中文字推导的颜色 `candidate_selected_gloss_color`，包的释义色只留给未选中行。
   - 没有皮肤包的释义色：仍画选中序号色；它对底色不到 4.5:1 时换成推导的颜色。
3. 选中底色半透明时下面透出的是卡片底色，照旧画包的释义色，没有时画选中序号色。

`candidate_selected_gloss_color` 取选中行候选文字的颜色（`selected_text`，它是「保持未选中色」的 alpha 0 时取候选文字色），不透明度从和选中序号一样的 0.82 起，对底色还不到 4.5:1（WCAG AA 正文）时按 0.01 往上加，到够了或者到不透明为止。对比度由 `candidate_contrast_on` 按释义色自己的 alpha 合成到底色上再算。

结果：默认浅色的选中行释义是白色 0.89，约 4.5:1；当时默认深色是不透明白色，约 2.3:1，和选中文字一样，到不了 4.5:1。之后[默认皮肤的选中行达到 WCAG AA](2026-11-01-default-selection-contrast-aa.md)把默认深色的选中底换成 `#17783F`，同一规则下释义是白色 0.86，到 4.5:1。未选中行一律不变。

### 默认深色为什么不到 4.5:1

审查时指出，上一版在默认深色上把释义换成了近黑 `#1A1A1A` 0.82（约 5.5:1），而候选文字和序号仍是白色，同一格两种前景色，次要的释义比主要的候选文字还显眼；这等于改了出厂默认皮肤的选中行设计，AGENTS.md 要求这类改动单独授权。这次的选择是：释义跟着选中文字走，同一行只有一种前景色，只在这个前景色里调不透明度。默认深色因此只从 2.0:1 提到 2.3:1。

要让默认深色选中行整体到 AA，得改的是选中底色 `#5FBF84` 或选中文字的白色，两端（macOS、Windows）的默认皮肤都是绿底白字，这是产品决定，不在这个 PR 里做。维护者批准后由[默认皮肤的选中行达到 WCAG AA](2026-11-01-default-selection-contrast-aa.md)处理：选中底换成 `#17783F`，白字不变。

## 和 macOS 的差别

- macOS 的 `DeriveSelectedForegrounds` 用 `ContrastingText` 按底色亮度挑黑白、固定 0.82，只在皮肤包有释义色时推导。皮肤没写选中文字时 macOS 的选中文字也是同一个挑法推导的，所以两端在这种皮肤上得到同一个颜色；皮肤自己写了选中文字时，Windows 跟着它，macOS 仍按亮度挑黑白。
- Windows 往上加不透明度到 4.5:1，macOS 固定 0.82，只保证约 3:1。
- Windows 在选中序号色不到 4.5:1 时也推导，所以默认皮肤的选中行释义会变（浅色白色 0.82 → 0.89，深色白色 0.82 → 1），颜色本身不变；macOS 默认皮肤仍是白色 0.82，深色约 2.0:1、浅色约 4.1:1。macOS 这次没动，同一个可读性问题留作后续，和上面默认深色的产品决定一起定。

## Alternatives considered

- **照搬 macOS，只在皮肤包有释义色时推导，固定 0.82** — 和 macOS 完全一致、默认皮肤的外观不变。但默认浅色约 4.1:1 差一点到 AA，只调不透明度就能补上，颜色不变，没有理由留着。
- **按底色挑白或近黑（上一版）** — 默认深色能到约 5.5:1。但白字配近黑释义的混排是新的默认外观，次要文字比主要文字显眼，需要单独的产品授权，被审查拦下。
- **把默认深色选中行的候选文字和序号也一起换成近黑，或把选中底色调暗** — 一行颜色统一，也能到 AA。但改的是两端共用的绿底白字设计，不在这次「释义和读音」的范围里，要单独由产品决定。
- **把推导结果写进 `CandidatePalette::selected_translation`** — 绘制处直接读字段。但这个字段现在的意思是「皮肤明确写了」，调色板又分两步合成（`candidate_palette` 再 `apply_candidate_skin_extra_slots`，取色器能让包的选中槽位不生效），在哪一步写都要再判断一次；在取色时推导只有一处。

## Consequences

- **收益**：不透明选中底色的皮肤，选中行的释义和读音不再用给卡片配的释义色，蓝底蓝字这类皮肤读得出来；选中文字能到 4.5:1 的底色上释义也到 4.5:1；默认浅色到 4.5:1。同一行始终只有一种前景色。
- **代价**：选中文字本身对底色不够时，释义也不够，默认深色约 2.3:1 就是这种情况（`#4D6BFE` 上的白色约 4.3:1 同理）。皮肤写了可读的 `selected_number`、没写释义色时照旧用它；写了读不清的 `selected_number` 时释义不再跟着它。
- **测试缺口**：`windows-candidate-palette` 和 `windows-candidate-skin` 在 CI 上只编译（Windows GNU cross build）不执行，Windows 原生作业的 ctest 只跑 `windows7-*` 和 `windows-settings-instance`。这次的断言靠本地运行，退化了 CI 发现不了；这个缺口在本 PR 之前就有，已由[默认皮肤的选中行达到 WCAG AA](2026-11-01-default-selection-contrast-aa.md)补上，两项在 `windows-scripts` 作业里用 MSVC 运行。

## Verification

- `tests/ui/candidate_palette.cpp`：默认浅色、深色的选中行释义和选中文字同色、不透明度不低于 0.82、对比度高于原来的选中序号色；浅色是白色 0.89 左右、至少 4.5:1，深色是不透明白色；未选中行和固定排位行不变；可读的选中序号色照用；`#23262F` 配 `#6B7180`、`#F2C94C` 配 `#B8860B` 的包到 4.5:1，`#4D6BFE` 配 `#2F5BD3` 取不透明白色；半透明选中底色保留包的释义色；写了 `selected_translation` 照它画。
- `tests/ui/candidate_skin.cpp`：选中取色器让包的 `selected_translation` 不生效时，选中行释义从选中文字推导、和它同色，未选中行不变。
- 两项都在 macOS 上用 `clang++ -std=c++17 -Wall -Wextra -Werror` 编译运行通过，`candidate_palette.cpp` 另用 `x86_64-w64-mingw32-g++ -fsyntax-only` 检查；Windows 构建和 `windows-candidate-skin-gallery` 截图交给 PR 的 CI。
